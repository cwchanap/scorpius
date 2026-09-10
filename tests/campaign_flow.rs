use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use scorpius::app::{GameScreen, enter_battle, teardown_battle_screen};
use scorpius::campaign::model::{
    CampaignState, PlayerMech, SquadUpgrades, UpgradeLevels, UpgradeTrack,
};
use scorpius::campaign::progression::CompletionReceipt;
use scorpius::campaign::save::SaveFile;
use scorpius::campaign::session::{CampaignSession, complete_current_mission, persist_purchase};
use scorpius::domain::board::GridPos;
use scorpius::domain::model::{MissionResult, OptionalObjective, PrimaryObjective};
use scorpius::mission::mission_definition;
use scorpius::mission::mission_five;
use scorpius::mission::mission_four;
use scorpius::mission::mission_one::ids;
use scorpius::mission::mission_three;
use scorpius::mission::{DialogueLine, MissionId};
use scorpius::presentation::assets::UiAssets;
use scorpius::presentation::battlefield::setup_mission_scene;
use scorpius::presentation::campaign_ui::{
    BriefingSnapshot, CampaignStatus, CampaignUiAction, DialogueCursor, ScreenRoot,
    apply_campaign_action as apply_campaign_action_for_screen, briefing_snapshot,
    dialogue_snapshot, ending_snapshot, upgrade_row_snapshot,
};
use scorpius::presentation::screens::setup_title_screen;
use scorpius::presentation::ui::HudRoot;
use scorpius::presentation::{
    ActiveMission, AttackPreviewCells, BattleCamera2d, BattleEventQueue, BattleRuntime,
    CampaignCamera, CampaignRuntime, EventPlayback, PresentationRoot,
    interaction::{InteractionState, StatusMessage, restart_battle},
};

static NEXT_ID: AtomicU32 = AtomicU32::new(0);

fn temp_save_path(label: &str) -> PathBuf {
    let n = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "scorpius-flow-{label}-{}-{n}.json",
        std::process::id()
    ))
}

fn init_battle_transients(app: &mut App) {
    app.init_resource::<InteractionState>()
        .init_resource::<StatusMessage>()
        .init_resource::<BattleEventQueue>()
        .init_resource::<EventPlayback>()
        .init_resource::<AttackPreviewCells>();
}

fn blank_ui_assets() -> UiAssets {
    UiAssets {
        key_art: Handle::default(),
        briefing_art: Handle::default(),
        vanguard_art: Handle::default(),
        gunner_art: Handle::default(),
        interceptor_art: Handle::default(),
        icons: Handle::default(),
        board: Handle::default(),
        fonts: std::array::from_fn(|_| Handle::default()),
    }
}

fn entity_count<T: Component>(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<Entity, With<T>>()
        .iter(app.world())
        .count()
}

fn pending(next: &NextState<GameScreen>) -> Option<GameScreen> {
    match next {
        NextState::Unchanged => None,
        NextState::Pending(state) | NextState::PendingIfNeq(state) => Some(*state),
    }
}

fn apply_campaign_action(
    action: CampaignUiAction,
    runtime: &mut CampaignRuntime,
    active_mission: Option<&ActiveMission>,
    cursor: &mut DialogueCursor,
    status: &mut CampaignStatus,
    next_state: &mut NextState<GameScreen>,
) {
    let current_screen = match action {
        CampaignUiAction::NewGame | CampaignUiAction::Continue => GameScreen::Title,
        CampaignUiAction::AdvanceDialogue | CampaignUiAction::SkipDialogue => {
            GameScreen::PreMissionStory
        }
        CampaignUiAction::StartMission => GameScreen::Briefing,
        CampaignUiAction::AdvanceAftermath => GameScreen::Aftermath,
        CampaignUiAction::PurchaseUpgrade(PlayerMech::Vanguard, _)
        | CampaignUiAction::PurchaseUpgrade(PlayerMech::Gunner, _)
        | CampaignUiAction::PurchaseUpgrade(PlayerMech::Interceptor, _)
        | CampaignUiAction::Proceed => GameScreen::Upgrade,
        CampaignUiAction::ReturnToTitle => GameScreen::Ending,
    };
    apply_campaign_action_for_screen(
        action,
        current_screen,
        runtime,
        active_mission,
        cursor,
        status,
        next_state,
    );
}

fn walk_story_to_briefing(
    runtime: &mut CampaignRuntime,
    cursor: &mut DialogueCursor,
    status: &mut CampaignStatus,
) {
    loop {
        let mut next = NextState::Unchanged;
        apply_campaign_action(
            CampaignUiAction::AdvanceDialogue,
            runtime,
            None,
            cursor,
            status,
            &mut next,
        );
        if pending(&next) == Some(GameScreen::Briefing) {
            return;
        }
    }
}

fn start_mission(
    runtime: &mut CampaignRuntime,
    cursor: &mut DialogueCursor,
    status: &mut CampaignStatus,
) {
    let mut next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::StartMission,
        runtime,
        None,
        cursor,
        status,
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::Battle));
}

#[test]
fn battle_entry_builds_the_active_mission_with_campaign_upgrades() {
    let mut app = App::new();
    app.insert_resource(CampaignRuntime(CampaignSession {
        state: Some(CampaignState {
            next_mission: MissionId::One,
            credits: 0,
            upgrades: SquadUpgrades {
                vanguard: UpgradeLevels {
                    hp: 1,
                    ..Default::default()
                },
                ..Default::default()
            },
            completed: false,
        }),
        save: SaveFile::new(temp_save_path("battle-entry")),
        last_completion: None,
    }));
    init_battle_transients(&mut app);
    app.add_systems(Update, enter_battle);

    app.update();

    let active = app.world().resource::<ActiveMission>().0;
    assert_eq!(active.id, MissionId::One);
    let battle = &app.world().resource::<BattleRuntime>().0;
    assert_eq!(battle.unit(ids::VANGUARD).unwrap().stats.max_hp, 23);
    assert_eq!(battle.round(), 1);
}

#[test]
fn mission_two_entry_and_restart_run_the_shared_definition_path_with_upgrades() {
    let mut app = App::new();
    app.insert_resource(CampaignRuntime(CampaignSession {
        state: Some(CampaignState {
            next_mission: MissionId::Two,
            credits: 800,
            upgrades: SquadUpgrades {
                gunner: UpgradeLevels {
                    hp: 1,
                    ..Default::default()
                },
                ..Default::default()
            },
            completed: false,
        }),
        save: SaveFile::new(temp_save_path("entry-restart-two")),
        last_completion: None,
    }));
    init_battle_transients(&mut app);
    app.add_systems(Update, enter_battle);

    app.update();

    let active = app.world().resource::<ActiveMission>().0;
    assert_eq!(active, mission_definition(MissionId::Two).unwrap());
    {
        let battle = &app.world().resource::<BattleRuntime>().0;
        assert_eq!(battle.round(), 1, "entry must run the authored opening");
        let gunner = battle.unit(ids::GUNNER).unwrap();
        assert_eq!(
            gunner.stats.max_hp, 18,
            "campaign HP upgrade must project into Mission 2"
        );
    }

    // Fight into the mission: move the Vanguard off its authored deployment.
    {
        let battle = &mut app.world_mut().resource_mut::<BattleRuntime>().0;
        battle.begin_activation(ids::VANGUARD).unwrap();
        battle.move_unit(ids::VANGUARD, GridPos::new(3, 6)).unwrap();
    }
    assert_eq!(
        app.world()
            .resource::<BattleRuntime>()
            .0
            .unit(ids::VANGUARD)
            .unwrap()
            .position,
        GridPos::new(3, 6)
    );

    restart_battle(app.world_mut(), 4242);

    let battle = &app.world().resource::<BattleRuntime>().0;
    assert_eq!(battle.round(), 0, "rebuild waits for the opening round");
    assert_eq!(
        battle.rules().primary,
        PrimaryObjective::ProtectThroughRound {
            target: ids::GUNNER,
            round: 3,
        },
        "restart must rebuild the authored Mission 2 rules"
    );
    assert_eq!(
        battle.rules().optional,
        OptionalObjective::ProtectTargetAtHalfHp {
            target: ids::GUNNER
        }
    );
    let gunner = battle.unit(ids::GUNNER).unwrap();
    assert_eq!(gunner.stats.max_hp, 18, "upgrades must survive restart");
    assert_eq!(gunner.hp, 18, "restart rebuilds at full authored HP");
    assert_eq!(gunner.position, GridPos::new(4, 6), "authored deployment");
    assert_eq!(
        battle.unit(ids::VANGUARD).unwrap().position,
        GridPos::new(3, 7),
        "mid-battle movement must be undone by the rebuild"
    );
    assert_eq!(app.world().resource::<ActiveMission>().0.id, MissionId::Two);
}

#[test]
fn mission_three_entry_builds_through_the_shared_definition_path_with_upgrades() {
    let mut app = App::new();
    app.insert_resource(CampaignRuntime(CampaignSession {
        state: Some(CampaignState {
            next_mission: MissionId::Three,
            credits: 800,
            upgrades: SquadUpgrades {
                vanguard: UpgradeLevels {
                    hp: 1,
                    ..Default::default()
                },
                ..Default::default()
            },
            completed: false,
        }),
        save: SaveFile::new(temp_save_path("entry-three")),
        last_completion: None,
    }));
    init_battle_transients(&mut app);
    app.add_systems(Update, enter_battle);

    app.update();

    let active = app.world().resource::<ActiveMission>().0;
    assert_eq!(active, mission_definition(MissionId::Three).unwrap());
    let battle = &app.world().resource::<BattleRuntime>().0;
    assert_eq!(battle.round(), 1, "entry must run the authored opening");
    assert_eq!(
        battle.rules().primary,
        PrimaryObjective::InterceptBeforeEscape {
            target: mission_three::ids::COURIER,
            escape: mission_three::EXTRACTION,
            deadline_round: 5,
        }
    );
    assert_eq!(
        battle.rules().optional,
        OptionalObjective::VictoryByRound { round: 2 }
    );
    assert_eq!(
        battle.unit(ids::VANGUARD).unwrap().stats.max_hp,
        23,
        "campaign HP upgrade must project into Mission 3"
    );
    assert_eq!(
        battle.unit(mission_three::ids::COURIER).unwrap().position,
        GridPos::new(0, 6)
    );
}

#[test]
fn mission_four_entry_builds_through_the_shared_definition_path_with_upgrades() {
    let mut app = App::new();
    app.insert_resource(CampaignRuntime(CampaignSession {
        state: Some(CampaignState {
            next_mission: MissionId::Four,
            credits: 1200,
            upgrades: SquadUpgrades {
                vanguard: UpgradeLevels {
                    hp: 1,
                    ..Default::default()
                },
                ..Default::default()
            },
            completed: false,
        }),
        save: SaveFile::new(temp_save_path("entry-four")),
        last_completion: None,
    }));
    init_battle_transients(&mut app);
    app.add_systems(Update, enter_battle);

    app.update();

    let active = app.world().resource::<ActiveMission>().0;
    assert_eq!(active, mission_definition(MissionId::Four).unwrap());
    let battle = &app.world().resource::<BattleRuntime>().0;
    assert_eq!(battle.round(), 1, "entry must run the authored opening");
    assert_eq!(
        battle.rules().primary,
        PrimaryObjective::EliminateTarget {
            target: mission_four::ids::BULWARK,
        }
    );
    assert_eq!(battle.rules().optional, OptionalObjective::Turnabout);
    assert_eq!(
        battle.unit(ids::VANGUARD).unwrap().stats.max_hp,
        23,
        "campaign HP upgrade must project into Mission 4"
    );
    assert_eq!(
        battle.unit(mission_four::ids::BULWARK).unwrap().position,
        GridPos::new(4, 4),
        "the authored opening steps the Bulwark into the breach"
    );
}

#[test]
fn mission_five_entry_builds_through_the_shared_definition_path_with_upgrades() {
    let mut app = App::new();
    app.insert_resource(CampaignRuntime(CampaignSession {
        state: Some(CampaignState {
            next_mission: MissionId::Five,
            credits: 2000,
            upgrades: SquadUpgrades {
                vanguard: UpgradeLevels {
                    hp: 1,
                    ..Default::default()
                },
                ..Default::default()
            },
            completed: false,
        }),
        save: SaveFile::new(temp_save_path("entry-five")),
        last_completion: None,
    }));
    init_battle_transients(&mut app);
    app.add_systems(Update, enter_battle);

    app.update();

    let active = app.world().resource::<ActiveMission>().0;
    assert_eq!(active, mission_definition(MissionId::Five).unwrap());
    let battle = &app.world().resource::<BattleRuntime>().0;
    assert_eq!(battle.round(), 1, "entry must run the authored opening");
    assert_eq!(
        battle.rules().primary,
        PrimaryObjective::EliminateAllEnemies
    );
    assert_eq!(
        battle.rules().optional,
        OptionalObjective::VictoryByRound { round: 4 }
    );
    assert_eq!(
        battle.unit(ids::VANGUARD).unwrap().stats.max_hp,
        23,
        "campaign HP upgrade must project into Mission 5"
    );
    assert_eq!(
        battle
            .unit(mission_five::ids::ARTILLERY_A)
            .unwrap()
            .position,
        GridPos::new(3, 0),
        "the first battery stays put on its firing solution"
    );
    assert_eq!(
        battle.unit(mission_five::ids::CONTROLLER).unwrap().position,
        GridPos::new(3, 6),
        "the authored opening steps the Controller into the push lane"
    );
}

#[test]
fn four_five_campaign_continuity_from_persisted_save_to_the_six_handoff() {
    // Seed the save: base-only One–Three (1200 credits) minus one 200-credit
    // Vanguard HP purchase, persisted at the Mission 4 handoff.
    let path = temp_save_path("four-five-continuity");
    {
        let mut session = CampaignSession {
            state: Some(CampaignState::new_game()),
            save: SaveFile::new(path.clone()),
            last_completion: None,
        };
        for id in [MissionId::One, MissionId::Two, MissionId::Three] {
            complete_current_mission(
                &mut session,
                mission_definition(id).unwrap(),
                MissionResult {
                    victory: true,
                    optional_complete: false,
                    rounds: 2,
                },
            )
            .unwrap();
        }
        persist_purchase(&mut session, PlayerMech::Vanguard, UpgradeTrack::Hp).unwrap();
    }

    let mut cursor = DialogueCursor(0);
    let mut status = CampaignStatus::default();

    // 1–2. Continue loads the save and routes Mission 4 to the Upgrade
    // screen with the non-zero upgrade intact.
    let mut runtime = CampaignRuntime(CampaignSession {
        state: None,
        save: SaveFile::new(path.clone()),
        last_completion: None,
    });
    let mut next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::Continue,
        &mut runtime,
        None,
        &mut cursor,
        &mut status,
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::Upgrade));
    {
        let state = runtime.0.state.as_ref().unwrap();
        assert_eq!(state.next_mission, MissionId::Four);
        assert_eq!(state.upgrades.vanguard.hp, 1);
        assert_eq!(
            state.credits, 1000,
            "base One–Three rewards minus the purchase"
        );
    }

    // 3. Four is authored: PROCEED opens its story directly.
    let mut next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::Proceed,
        &mut runtime,
        None,
        &mut cursor,
        &mut status,
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::PreMissionStory));
    walk_story_to_briefing(&mut runtime, &mut cursor, &mut status);
    start_mission(&mut runtime, &mut cursor, &mut status);

    // 4. Start Mission: the shared definition path builds M4 with the
    // persisted upgrade projected.
    let mut app = App::new();
    init_battle_transients(&mut app);
    app.insert_resource(runtime);
    app.add_systems(Update, enter_battle);
    app.update();

    let m4 = app.world().resource::<ActiveMission>().0;
    assert_eq!(m4, mission_definition(MissionId::Four).unwrap());
    {
        let battle = &app.world().resource::<BattleRuntime>().0;
        assert_eq!(battle.round(), 1, "entry runs the authored opening");
        assert_eq!(
            battle.unit(ids::VANGUARD).unwrap().stats.max_hp,
            23,
            "persisted HP upgrade projects into Mission 4"
        );
    }

    // 5. Restart rebuilds the same M4 definition; campaign state untouched.
    {
        let battle = &mut app.world_mut().resource_mut::<BattleRuntime>().0;
        battle.begin_activation(ids::VANGUARD).unwrap();
        battle.move_unit(ids::VANGUARD, GridPos::new(4, 6)).unwrap();
    }
    restart_battle(app.world_mut(), 4242);
    assert_eq!(
        app.world().resource::<ActiveMission>().0,
        m4,
        "restart keeps the same Mission 4 definition"
    );
    {
        let battle = &app.world().resource::<BattleRuntime>().0;
        assert_eq!(battle.round(), 0, "rebuild waits for the opening round");
        let vanguard = battle.unit(ids::VANGUARD).unwrap();
        assert_eq!(
            vanguard.position,
            GridPos::new(4, 7),
            "authored deployment restored"
        );
        assert_eq!(vanguard.stats.max_hp, 23, "upgrade survives restart");
    }
    {
        let state = app
            .world()
            .resource::<CampaignRuntime>()
            .0
            .state
            .as_ref()
            .unwrap();
        assert_eq!(state.next_mission, MissionId::Four);
        assert_eq!(
            state.upgrades.vanguard.hp, 1,
            "restart reads, never mutates"
        );
        assert_eq!(state.credits, 1000);
    }

    // 6. Completing M4 (base only) banks its 600 base reward and persists
    // the Mission 5 handoff.
    let mut runtime = app
        .world_mut()
        .remove_resource::<CampaignRuntime>()
        .unwrap();
    let receipt = complete_current_mission(
        &mut runtime.0,
        m4,
        MissionResult {
            victory: true,
            optional_complete: false,
            rounds: 3,
        },
    )
    .unwrap();
    assert_eq!((receipt.base_reward, receipt.optional_reward), (600, 0));
    assert_eq!(
        runtime.0.state.as_ref().unwrap().next_mission,
        MissionId::Five
    );
    assert_eq!(
        runtime.0.save.load().unwrap().unwrap().next_mission,
        MissionId::Five,
        "completion persists the Mission 5 handoff"
    );

    // 7. Continue/Proceed route Five through Upgrade into its story; the
    // same upgrade still projects.
    runtime.0.state = None;
    let mut next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::Continue,
        &mut runtime,
        None,
        &mut cursor,
        &mut status,
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::Upgrade));
    {
        let state = runtime.0.state.as_ref().unwrap();
        assert_eq!(state.next_mission, MissionId::Five);
        assert_eq!(state.upgrades.vanguard.hp, 1);
        assert_eq!(state.credits, 1600, "600 base reward persisted");
    }
    let mut next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::Proceed,
        &mut runtime,
        None,
        &mut cursor,
        &mut status,
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::PreMissionStory));
    cursor = DialogueCursor(0);
    walk_story_to_briefing(&mut runtime, &mut cursor, &mut status);
    start_mission(&mut runtime, &mut cursor, &mut status);
    app.insert_resource(runtime);
    app.update();

    let m5 = app.world().resource::<ActiveMission>().0;
    assert_eq!(m5, mission_definition(MissionId::Five).unwrap());
    {
        let battle = &app.world().resource::<BattleRuntime>().0;
        assert_eq!(battle.round(), 1);
        assert_eq!(
            battle.unit(ids::VANGUARD).unwrap().stats.max_hp,
            23,
            "the same upgrade projects into Mission 5"
        );
    }

    // 8. Completing M5 (base only) persists the Six handoff. Base rewards
    // through Five: 1200 (One–Three) + 600 + 700 = 2500 before optional
    // rewards; the one 200-credit purchase leaves 2300 banked.
    let mut runtime = app
        .world_mut()
        .remove_resource::<CampaignRuntime>()
        .unwrap();
    let receipt = complete_current_mission(
        &mut runtime.0,
        m5,
        MissionResult {
            victory: true,
            optional_complete: false,
            rounds: 3,
        },
    )
    .unwrap();
    assert_eq!((receipt.base_reward, receipt.optional_reward), (700, 0));
    assert_eq!(
        runtime.0.state.as_ref().unwrap().next_mission,
        MissionId::Six
    );
    assert_eq!(runtime.0.state.as_ref().unwrap().credits, 2500 - 200);
    assert_eq!(
        runtime.0.save.load().unwrap().unwrap().next_mission,
        MissionId::Six,
        "the Six handoff persists"
    );

    // 9. Continue at Six routes to the Upgrade screen — Six is now authored.
    runtime.0.state = None;
    let mut next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::Continue,
        &mut runtime,
        None,
        &mut cursor,
        &mut status,
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::Upgrade));
    assert_eq!(
        runtime.0.state.as_ref().unwrap().next_mission,
        MissionId::Six
    );
}

#[test]
fn completing_three_four_and_five_advances_to_six_then_six_advances_to_seven_at_3300() {
    let mut session = CampaignSession {
        state: Some(CampaignState {
            next_mission: MissionId::Three,
            // Base rewards of One and Two already banked.
            credits: 300 + 400,
            upgrades: SquadUpgrades::default(),
            completed: false,
        }),
        save: SaveFile::new(temp_save_path("completion-through-four")),
        last_completion: None,
    };
    let victory = |optional| MissionResult {
        victory: true,
        optional_complete: optional,
        rounds: 1,
    };

    complete_current_mission(
        &mut session,
        mission_definition(MissionId::Three).unwrap(),
        victory(false),
    )
    .unwrap();
    assert_eq!(
        session.state.as_ref().unwrap().next_mission,
        MissionId::Four
    );

    let receipt = complete_current_mission(
        &mut session,
        mission_definition(MissionId::Four).unwrap(),
        victory(false),
    )
    .unwrap();
    assert_eq!((receipt.base_reward, receipt.optional_reward), (600, 0));
    assert_eq!(
        session.state.as_ref().unwrap().next_mission,
        MissionId::Five
    );

    let receipt = complete_current_mission(
        &mut session,
        mission_definition(MissionId::Five).unwrap(),
        victory(false),
    )
    .unwrap();
    assert_eq!((receipt.base_reward, receipt.optional_reward), (700, 0));
    assert_eq!(session.state.as_ref().unwrap().next_mission, MissionId::Six);

    // Base rewards through Five: 300 + 400 + 500 + 600 + 700 = 2500.
    assert_eq!(session.state.as_ref().unwrap().credits, 2500);

    // Completing Six (base only) advances to the Seven handoff at 3300.
    let receipt = complete_current_mission(
        &mut session,
        mission_definition(MissionId::Six).unwrap(),
        MissionResult {
            victory: true,
            optional_complete: false,
            rounds: 4,
        },
    )
    .unwrap();
    assert_eq!((receipt.base_reward, receipt.optional_reward), (800, 0));
    assert_eq!(
        session.state.as_ref().unwrap().next_mission,
        MissionId::Seven
    );
    assert_eq!(session.state.as_ref().unwrap().credits, 3300);

    // Turnabout complete: the 250 bonus rides on top of the base reward.
    let mut session = CampaignSession {
        state: Some(CampaignState {
            next_mission: MissionId::Six,
            credits: 2500,
            upgrades: SquadUpgrades::default(),
            completed: false,
        }),
        save: SaveFile::new(temp_save_path("completion-through-six-optional")),
        last_completion: None,
    };
    let receipt = complete_current_mission(
        &mut session,
        mission_definition(MissionId::Six).unwrap(),
        MissionResult {
            victory: true,
            optional_complete: true,
            rounds: 4,
        },
    )
    .unwrap();
    assert_eq!((receipt.base_reward, receipt.optional_reward), (800, 250));
    assert_eq!(session.state.as_ref().unwrap().credits, 3300 + 250);
    assert_eq!(
        session.state.as_ref().unwrap().next_mission,
        MissionId::Seven
    );
}

#[test]
fn title_is_the_default_screen_and_battle_waits_for_entry() {
    let mut app = App::new();
    let mut campaign = CampaignSession::new(SaveFile::new(temp_save_path("state-entry")));
    campaign.state = Some(CampaignState::new_game());
    app.add_plugins(StatesPlugin)
        .insert_resource(CampaignRuntime(campaign));
    init_battle_transients(&mut app);
    app.init_state::<GameScreen>()
        .add_systems(OnEnter(GameScreen::Battle), enter_battle);

    app.update();

    assert_eq!(
        app.world().resource::<State<GameScreen>>().get(),
        &GameScreen::Title
    );
    assert!(app.world().get_resource::<BattleRuntime>().is_none());

    app.world_mut()
        .resource_mut::<NextState<GameScreen>>()
        .set(GameScreen::Battle);
    app.update();

    assert_eq!(
        app.world().resource::<State<GameScreen>>().get(),
        &GameScreen::Battle
    );
    assert_eq!(app.world().resource::<ActiveMission>().0.id, MissionId::One);
    assert_eq!(app.world().resource::<BattleRuntime>().0.round(), 1);
}

#[test]
fn briefing_snapshot_lists_objectives_and_rewards() {
    let definition = mission_definition(MissionId::One).unwrap();
    let snapshot = briefing_snapshot(definition, &CampaignState::new_game());
    assert_eq!(
        snapshot,
        BriefingSnapshot {
            mission: MissionId::One,
            title: definition.title,
            bonus_title: "Turnabout",
            enemy_count: 4,
            primary: definition.primary_objective,
            optional: definition.optional_objective,
            base_reward: 300,
            optional_reward: 100,
            credits: 0,
        }
    );
    let mission_four = mission_definition(MissionId::Four).unwrap();
    assert_eq!(
        briefing_snapshot(mission_four, &CampaignState::new_game()).bonus_title,
        "Chain Reaction"
    );
}

#[test]
fn pre_mission_story_swaps_control_expression_between_lines() {
    let scene = &mission_definition(MissionId::One).unwrap().pre_mission;

    let opening = dialogue_snapshot(scene, DialogueCursor(0));
    let warning = dialogue_snapshot(scene, DialogueCursor(2));

    assert_eq!(opening.speaker, "Control");
    assert_eq!(opening.portrait, "vn/control_neutral.png");
    assert_eq!(opening.text, scene.lines[0].text);
    assert_eq!(warning.speaker, "Control");
    assert_eq!(warning.portrait, "vn/control_alert.png");
    assert_eq!(warning.text, scene.lines[2].text);
}

#[test]
fn title_new_game_persists_and_enters_pre_mission_story() {
    let save = SaveFile::new(temp_save_path("new-game"));
    let mut runtime = CampaignRuntime(CampaignSession::new(save));
    let mut cursor = DialogueCursor(0);
    let mut status = CampaignStatus::default();
    let mut next = NextState::Unchanged;

    apply_campaign_action(
        CampaignUiAction::NewGame,
        &mut runtime,
        None,
        &mut cursor,
        &mut status,
        &mut next,
    );

    assert_eq!(pending(&next), Some(GameScreen::PreMissionStory));
    assert!(status.0.is_empty());
    assert_eq!(
        runtime.0.state.as_ref().unwrap().next_mission,
        MissionId::One
    );
    assert!(runtime.0.save.load().unwrap().is_some());
}

#[test]
fn title_continue_routes_by_the_saved_next_mission() {
    let mut runtime = CampaignRuntime(CampaignSession {
        state: None,
        save: SaveFile::new(temp_save_path("continue-two")),
        last_completion: None,
    });
    runtime
        .0
        .save
        .store(&CampaignState {
            next_mission: MissionId::Two,
            credits: 400,
            upgrades: SquadUpgrades::default(),
            completed: false,
        })
        .unwrap();
    let mut next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::Continue,
        &mut runtime,
        None,
        &mut DialogueCursor(0),
        &mut CampaignStatus::default(),
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::Upgrade));

    let mut runtime = CampaignRuntime(CampaignSession {
        state: None,
        save: SaveFile::new(temp_save_path("continue-one")),
        last_completion: None,
    });
    runtime
        .0
        .save
        .store(&CampaignState {
            next_mission: MissionId::One,
            credits: 0,
            upgrades: SquadUpgrades::default(),
            completed: false,
        })
        .unwrap();
    let mut next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::Continue,
        &mut runtime,
        None,
        &mut DialogueCursor(0),
        &mut CampaignStatus::default(),
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::PreMissionStory));

    let mut runtime = CampaignRuntime(CampaignSession {
        state: None,
        save: SaveFile::new(temp_save_path("continue-missing")),
        last_completion: None,
    });
    let mut status = CampaignStatus::default();
    let mut next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::Continue,
        &mut runtime,
        None,
        &mut DialogueCursor(0),
        &mut status,
        &mut next,
    );
    assert_eq!(pending(&next), None);
    assert!(status.0.contains("no active campaign"));
    assert!(runtime.0.state.is_none());

    let corrupt_path = temp_save_path("continue-corrupt");
    std::fs::write(&corrupt_path, b"{ not valid campaign json").unwrap();
    let mut runtime = CampaignRuntime(CampaignSession {
        state: None,
        save: SaveFile::new(corrupt_path),
        last_completion: None,
    });
    let mut status = CampaignStatus::default();
    let mut next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::Continue,
        &mut runtime,
        None,
        &mut DialogueCursor(0),
        &mut status,
        &mut next,
    );
    assert_eq!(pending(&next), None);
    assert!(status.0.contains("corrupted save file"));
}

#[test]
fn advancing_dialogue_walks_all_lines_then_opens_briefing() {
    let mut runtime = CampaignRuntime(CampaignSession {
        state: Some(CampaignState::new_game()),
        save: SaveFile::new(temp_save_path("advance-story")),
        last_completion: None,
    });
    let mut cursor = DialogueCursor(0);
    let mut status = CampaignStatus::default();
    let mut next = NextState::Unchanged;

    for expected_cursor in [1, 2] {
        apply_campaign_action(
            CampaignUiAction::AdvanceDialogue,
            &mut runtime,
            None,
            &mut cursor,
            &mut status,
            &mut next,
        );
        assert_eq!(pending(&next), None);
        assert_eq!(cursor, DialogueCursor(expected_cursor));
    }

    apply_campaign_action(
        CampaignUiAction::AdvanceDialogue,
        &mut runtime,
        None,
        &mut cursor,
        &mut status,
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::Briefing));
    assert_eq!(cursor, DialogueCursor(2));
    assert!(status.0.is_empty());

    next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::StartMission,
        &mut runtime,
        None,
        &mut cursor,
        &mut status,
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::Battle));
}

#[test]
fn battle_reentry_despawns_stale_battlefield_and_hud_roots() {
    let mut app = App::new();
    app.add_plugins(StatesPlugin)
        .insert_resource(CampaignRuntime(CampaignSession {
            state: Some(CampaignState::new_game()),
            save: SaveFile::new(temp_save_path("reentry")),
            last_completion: None,
        }));
    init_battle_transients(&mut app);
    app.init_state::<GameScreen>().add_systems(
        OnEnter(GameScreen::Battle),
        (teardown_battle_screen, enter_battle).chain(),
    );
    app.update();
    assert_eq!(
        app.world().resource::<State<GameScreen>>().get(),
        &GameScreen::Title
    );

    let stale_root = app.world_mut().spawn(PresentationRoot).id();
    let stale_child = app.world_mut().spawn(ChildOf(stale_root)).id();
    let stale_battle_camera = app.world_mut().spawn((Camera2d, BattleCamera2d)).id();
    let unrelated_camera = app.world_mut().spawn(Camera2d).id();
    app.world_mut().spawn(HudRoot);

    app.world_mut()
        .resource_mut::<NextState<GameScreen>>()
        .set(GameScreen::Battle);
    app.update();

    assert!(
        app.world_mut()
            .query_filtered::<(), With<PresentationRoot>>()
            .iter(app.world())
            .next()
            .is_none()
    );
    assert!(app.world_mut().get_entity(stale_battle_camera).is_err());
    assert!(app.world().get_entity(unrelated_camera).is_ok());
    assert!(
        app.world_mut()
            .query_filtered::<(), With<HudRoot>>()
            .iter(app.world())
            .next()
            .is_none()
    );
    assert!(app.world().get_entity(stale_child).is_err());
    assert_eq!(app.world().resource::<BattleRuntime>().0.round(), 1);
}

#[test]
fn title_battle_exit_battle_restart_keeps_one_owned_camera_and_root() {
    let mut app = App::new();
    app.add_plugins(StatesPlugin)
        .insert_resource(CampaignRuntime(CampaignSession {
            state: Some(CampaignState::new_game()),
            save: SaveFile::new(temp_save_path("lifecycle")),
            last_completion: None,
        }))
        .insert_resource(blank_ui_assets())
        .init_resource::<CampaignStatus>()
        .init_state::<GameScreen>()
        .add_systems(OnEnter(GameScreen::Title), setup_title_screen)
        .add_systems(
            OnExit(GameScreen::Title),
            scorpius::presentation::campaign_ui::despawn_campaign_screen,
        )
        .add_systems(
            OnEnter(GameScreen::Battle),
            (
                teardown_battle_screen,
                enter_battle,
                setup_mission_scene,
                scorpius::presentation::ui::setup_mission_ui,
            )
                .chain(),
        )
        .add_systems(OnExit(GameScreen::Battle), teardown_battle_screen);
    init_battle_transients(&mut app);

    app.update();
    assert_eq!(entity_count::<CampaignCamera>(&mut app), 1);
    assert_eq!(entity_count::<BattleCamera2d>(&mut app), 0);
    assert_eq!(entity_count::<ScreenRoot>(&mut app), 1);
    assert_eq!(entity_count::<PresentationRoot>(&mut app), 0);

    app.world_mut()
        .resource_mut::<NextState<GameScreen>>()
        .set(GameScreen::Battle);
    app.update();
    assert_eq!(entity_count::<CampaignCamera>(&mut app), 0);
    assert_eq!(entity_count::<BattleCamera2d>(&mut app), 1);
    assert_eq!(entity_count::<ScreenRoot>(&mut app), 0);
    assert_eq!(entity_count::<PresentationRoot>(&mut app), 1);

    app.world_mut()
        .resource_mut::<NextState<GameScreen>>()
        .set(GameScreen::Title);
    app.update();
    assert_eq!(entity_count::<CampaignCamera>(&mut app), 1);
    assert_eq!(entity_count::<BattleCamera2d>(&mut app), 0);
    assert_eq!(entity_count::<ScreenRoot>(&mut app), 1);
    assert_eq!(entity_count::<PresentationRoot>(&mut app), 0);

    app.world_mut()
        .resource_mut::<NextState<GameScreen>>()
        .set(GameScreen::Battle);
    app.update();
    assert_eq!(entity_count::<CampaignCamera>(&mut app), 0);
    assert_eq!(entity_count::<BattleCamera2d>(&mut app), 1);
    assert_eq!(entity_count::<PresentationRoot>(&mut app), 1);

    let old_root = app
        .world_mut()
        .query_filtered::<Entity, With<PresentationRoot>>()
        .single(app.world())
        .unwrap();
    restart_battle(app.world_mut(), 4_242);
    assert!(app.world().get_entity(old_root).is_err());
    app.update();
    assert_eq!(entity_count::<BattleCamera2d>(&mut app), 1);
    assert_eq!(entity_count::<PresentationRoot>(&mut app), 1);
    assert_eq!(entity_count::<CampaignCamera>(&mut app), 0);
    assert_eq!(entity_count::<ScreenRoot>(&mut app), 0);
    assert_eq!(
        app.world()
            .resource::<BattleRuntime>()
            .0
            .unit(ids::VANGUARD)
            .unwrap()
            .position,
        GridPos::new(4, 7)
    );
}

static AFTERMATH_FIXTURE_RECEIPTS: [CompletionReceipt; 2] = [
    CompletionReceipt {
        mission: MissionId::One,
        base_reward: 300,
        optional_reward: 0,
        total_reward: 300,
        credits_after: 300,
    },
    CompletionReceipt {
        mission: MissionId::One,
        base_reward: 300,
        optional_reward: 100,
        total_reward: 400,
        credits_after: 400,
    },
];

#[test]
fn aftermath_receipt_exposes_the_persisted_reward_values() {
    let base_only = AFTERMATH_FIXTURE_RECEIPTS[0];
    assert_eq!((base_only.base_reward, base_only.optional_reward), (300, 0));
    assert_eq!(
        (base_only.total_reward, base_only.credits_after),
        (300, 300)
    );

    let bonus = AFTERMATH_FIXTURE_RECEIPTS[1];
    assert_eq!((bonus.base_reward, bonus.optional_reward), (300, 100));
    assert_eq!((bonus.total_reward, bonus.credits_after), (400, 400));
}

#[test]
fn advancing_aftermath_walks_lines_then_opens_upgrade() {
    let mut runtime = CampaignRuntime(CampaignSession {
        state: Some(CampaignState {
            next_mission: MissionId::Two,
            credits: 300,
            upgrades: SquadUpgrades::default(),
            completed: false,
        }),
        save: SaveFile::new(temp_save_path("advance-aftermath")),
        last_completion: Some(AFTERMATH_FIXTURE_RECEIPTS[0]),
    });
    // Aftermath must use `ActiveMission`, not `state.next_mission` (Two has
    // no definition).
    let active_mission = ActiveMission(mission_definition(MissionId::One).unwrap());
    let mut cursor = DialogueCursor(0);
    let mut status = CampaignStatus::default();
    let mut next = NextState::Unchanged;

    apply_campaign_action(
        CampaignUiAction::AdvanceAftermath,
        &mut runtime,
        Some(&active_mission),
        &mut cursor,
        &mut status,
        &mut next,
    );
    assert_eq!(pending(&next), None);
    assert_eq!(cursor, DialogueCursor(1));

    apply_campaign_action(
        CampaignUiAction::AdvanceAftermath,
        &mut runtime,
        Some(&active_mission),
        &mut cursor,
        &mut status,
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::Upgrade));
    assert_eq!(cursor, DialogueCursor(1));
    assert!(status.0.is_empty());
}

#[test]
fn purchase_upgrade_action_persists_and_reports_failures() {
    let mut runtime = CampaignRuntime(CampaignSession {
        state: Some(CampaignState {
            next_mission: MissionId::Two,
            credits: 500,
            upgrades: SquadUpgrades::default(),
            completed: false,
        }),
        save: SaveFile::new(temp_save_path("purchase")),
        last_completion: None,
    });
    let mut cursor = DialogueCursor(0);
    let mut status = CampaignStatus::default();
    let mut next = NextState::Unchanged;

    apply_campaign_action(
        CampaignUiAction::PurchaseUpgrade(PlayerMech::Vanguard, UpgradeTrack::Hp),
        &mut runtime,
        None,
        &mut cursor,
        &mut status,
        &mut next,
    );
    let state = runtime.0.state.as_ref().unwrap();
    assert_eq!(state.credits, 300);
    assert_eq!(
        state
            .upgrades
            .levels(PlayerMech::Vanguard)
            .level(UpgradeTrack::Hp),
        1
    );
    let disk = runtime.0.save.load().unwrap().unwrap();
    assert_eq!(disk.credits, 300);
    assert!(status.0.contains("300 credits remaining"));
    assert_eq!(pending(&next), None);

    // HP level 2 costs 400 but only 300 remain: no-op plus FlowError.
    apply_campaign_action(
        CampaignUiAction::PurchaseUpgrade(PlayerMech::Vanguard, UpgradeTrack::Hp),
        &mut runtime,
        None,
        &mut cursor,
        &mut status,
        &mut next,
    );
    let state = runtime.0.state.as_ref().unwrap();
    assert_eq!(state.credits, 300);
    assert_eq!(
        state
            .upgrades
            .levels(PlayerMech::Vanguard)
            .level(UpgradeTrack::Hp),
        1
    );
    assert!(status.0.contains("insufficient credits"));
    assert_eq!(pending(&next), None);
}

#[test]
fn upgrade_row_snapshot_lists_level_effects_cost_and_max() {
    let state = CampaignState {
        next_mission: MissionId::Two,
        credits: 300,
        upgrades: SquadUpgrades {
            vanguard: UpgradeLevels {
                hp: 1,
                ..Default::default()
            },
            gunner: UpgradeLevels::default(),
            interceptor: UpgradeLevels {
                hp: 3,
                armor: 3,
                mobility: 3,
                weapon: 3,
            },
        },
        completed: false,
    };

    let row = upgrade_row_snapshot(&state, PlayerMech::Vanguard, UpgradeTrack::Hp);
    assert_eq!(row.level, 1);
    assert_eq!(row.current_effect, "+3 MAX HP");
    assert_eq!(row.next_effect, "+6 MAX HP");
    assert_eq!(row.cost, Some(400));
    assert!(!row.affordable);

    let row = upgrade_row_snapshot(&state, PlayerMech::Gunner, UpgradeTrack::Mobility);
    assert_eq!(row.level, 0);
    assert_eq!(row.current_effect, "+0 EVASION");
    assert_eq!(row.next_effect, "+5 EVASION");
    assert_eq!(row.cost, Some(200));
    assert!(row.affordable);

    let row = upgrade_row_snapshot(&state, PlayerMech::Interceptor, UpgradeTrack::Weapon);
    assert_eq!(row.level, 3);
    assert_eq!(row.current_effect, "+3 WEAPON DMG");
    assert_eq!(row.next_effect, "MAX");
    assert_eq!(row.cost, None);
    assert!(row.maxed);
    assert!(!row.affordable);
}

#[test]
fn proceed_with_an_authored_next_mission_opens_its_story_and_return_never_writes_save() {
    let mut runtime = CampaignRuntime(CampaignSession {
        state: Some(CampaignState {
            next_mission: MissionId::Two,
            credits: 400,
            upgrades: SquadUpgrades::default(),
            completed: false,
        }),
        save: SaveFile::new(temp_save_path("handoff")),
        last_completion: None,
    });
    runtime
        .0
        .save
        .store(&CampaignState {
            next_mission: MissionId::Two,
            credits: 400,
            upgrades: SquadUpgrades::default(),
            completed: false,
        })
        .unwrap();
    let mut cursor = DialogueCursor(0);
    let mut status = CampaignStatus::default();
    let mut next = NextState::Unchanged;

    // Mission 2 is authored: PROCEED skips the handoff and opens its story.
    apply_campaign_action(
        CampaignUiAction::Proceed,
        &mut runtime,
        None,
        &mut cursor,
        &mut status,
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::PreMissionStory));

    next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::ReturnToTitle,
        &mut runtime,
        None,
        &mut cursor,
        &mut status,
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::Title));
    assert!(status.0.is_empty());

    let disk = runtime.0.save.load().unwrap().unwrap();
    assert_eq!(disk.next_mission, MissionId::Two);
    assert_eq!(disk.credits, 400);
    assert!(disk.upgrades == SquadUpgrades::default());
}

#[test]
fn continue_and_proceed_route_unfinished_missions_to_upgrade_and_story() {
    let route_continue = |next_mission| {
        let mut runtime = CampaignRuntime(CampaignSession {
            state: None,
            save: SaveFile::new(temp_save_path("continue-route")),
            last_completion: None,
        });
        runtime
            .0
            .save
            .store(&CampaignState {
                next_mission,
                credits: 0,
                upgrades: SquadUpgrades::default(),
                completed: false,
            })
            .unwrap();
        let mut next = NextState::Unchanged;
        apply_campaign_action(
            CampaignUiAction::Continue,
            &mut runtime,
            None,
            &mut DialogueCursor(0),
            &mut CampaignStatus::default(),
            &mut next,
        );
        pending(&next)
    };
    assert_eq!(route_continue(MissionId::Three), Some(GameScreen::Upgrade));
    assert_eq!(route_continue(MissionId::Four), Some(GameScreen::Upgrade));
    assert_eq!(route_continue(MissionId::Five), Some(GameScreen::Upgrade));
    assert_eq!(route_continue(MissionId::Six), Some(GameScreen::Upgrade));
    // Seven is authored: an unfinished save continues into its Upgrade pass.
    assert_eq!(route_continue(MissionId::Seven), Some(GameScreen::Upgrade));

    // PROCEED at the Three handoff opens Mission 3's story.
    let mut runtime = CampaignRuntime(CampaignSession {
        state: Some(CampaignState {
            next_mission: MissionId::Three,
            credits: 900,
            upgrades: SquadUpgrades::default(),
            completed: false,
        }),
        save: SaveFile::new(temp_save_path("proceed-three")),
        last_completion: None,
    });
    let mut next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::Proceed,
        &mut runtime,
        None,
        &mut DialogueCursor(0),
        &mut CampaignStatus::default(),
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::PreMissionStory));

    // PROCEED at the Four handoff opens Mission 4's story — Four is authored.
    let mut runtime = CampaignRuntime(CampaignSession {
        state: Some(CampaignState {
            next_mission: MissionId::Four,
            credits: 1200,
            upgrades: SquadUpgrades::default(),
            completed: false,
        }),
        save: SaveFile::new(temp_save_path("proceed-four")),
        last_completion: None,
    });
    let mut next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::Proceed,
        &mut runtime,
        None,
        &mut DialogueCursor(0),
        &mut CampaignStatus::default(),
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::PreMissionStory));

    // PROCEED at the Five handoff opens Mission 5's story — Five is authored.
    let mut runtime = CampaignRuntime(CampaignSession {
        state: Some(CampaignState {
            next_mission: MissionId::Five,
            credits: 2000,
            upgrades: SquadUpgrades::default(),
            completed: false,
        }),
        save: SaveFile::new(temp_save_path("proceed-five")),
        last_completion: None,
    });
    let mut next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::Proceed,
        &mut runtime,
        None,
        &mut DialogueCursor(0),
        &mut CampaignStatus::default(),
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::PreMissionStory));

    // PROCEED at the Six handoff opens Mission 6's story — Six is authored.
    let mut runtime = CampaignRuntime(CampaignSession {
        state: Some(CampaignState {
            next_mission: MissionId::Six,
            credits: 1200,
            upgrades: SquadUpgrades::default(),
            completed: false,
        }),
        save: SaveFile::new(temp_save_path("proceed-six")),
        last_completion: None,
    });
    let mut next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::Proceed,
        &mut runtime,
        None,
        &mut DialogueCursor(0),
        &mut CampaignStatus::default(),
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::PreMissionStory));

    // PROCEED at the Seven handoff opens Mission 7's story — Seven is authored.
    let mut runtime = CampaignRuntime(CampaignSession {
        state: Some(CampaignState {
            next_mission: MissionId::Seven,
            credits: 1200,
            upgrades: SquadUpgrades::default(),
            completed: false,
        }),
        save: SaveFile::new(temp_save_path("proceed-seven")),
        last_completion: None,
    });
    let mut next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::Proceed,
        &mut runtime,
        None,
        &mut DialogueCursor(0),
        &mut CampaignStatus::default(),
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::PreMissionStory));
}

#[test]
fn completed_campaign_routes_continue_aftermath_and_proceed_to_ending() {
    // Seed and persist a completed campaign through the real Seven completion.
    let path = temp_save_path("completed-ending");
    {
        let mut session = CampaignSession {
            state: Some(CampaignState {
                next_mission: MissionId::Seven,
                credits: 3300,
                upgrades: SquadUpgrades::default(),
                completed: false,
            }),
            save: SaveFile::new(path.clone()),
            last_completion: None,
        };
        complete_current_mission(
            &mut session,
            mission_definition(MissionId::Seven).unwrap(),
            MissionResult {
                victory: true,
                optional_complete: false,
                rounds: 4,
            },
        )
        .unwrap();
        let state = session.state.as_ref().unwrap();
        assert!(state.completed);
        assert_eq!(state.next_mission, MissionId::Seven);
    }

    let mut cursor = DialogueCursor(0);
    let mut status = CampaignStatus::default();

    // Completed Continue -> Ending.
    let mut runtime = CampaignRuntime(CampaignSession {
        state: None,
        save: SaveFile::new(path.clone()),
        last_completion: None,
    });
    let mut next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::Continue,
        &mut runtime,
        None,
        &mut cursor,
        &mut status,
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::Ending));

    // Completed AdvanceAftermath -> Ending: the final aftermath walks its
    // three lines, then the last advance lands on Ending.
    let active_mission = ActiveMission(mission_definition(MissionId::Seven).unwrap());
    for expected_cursor in [1, 2] {
        next = NextState::Unchanged;
        apply_campaign_action(
            CampaignUiAction::AdvanceAftermath,
            &mut runtime,
            Some(&active_mission),
            &mut cursor,
            &mut status,
            &mut next,
        );
        assert_eq!(pending(&next), None);
        assert_eq!(cursor, DialogueCursor(expected_cursor));
    }
    next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::AdvanceAftermath,
        &mut runtime,
        Some(&active_mission),
        &mut cursor,
        &mut status,
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::Ending));

    // Completed Proceed -> Ending, never back into Mission 7 story.
    next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::Proceed,
        &mut runtime,
        None,
        &mut cursor,
        &mut status,
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::Ending));

    // Ending -> Title.
    next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::ReturnToTitle,
        &mut runtime,
        None,
        &mut cursor,
        &mut status,
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::Title));
}

#[test]
fn mission_six_briefing_and_dialogue_match_the_spec() {
    let definition = mission_definition(MissionId::Six).unwrap();
    let snapshot = briefing_snapshot(
        definition,
        &CampaignState {
            credits: 0,
            ..CampaignState::new_game()
        },
    );
    assert_eq!(snapshot.title, definition.title);
    assert_eq!(snapshot.primary, definition.primary_objective);
    assert_eq!(snapshot.optional, definition.optional_objective);
    assert_eq!(snapshot.base_reward, 800);
    assert_eq!(snapshot.optional_reward, 250);

    // Pre-mission: the spec's exact lines over existing VN portraits.
    let scene = &definition.pre_mission;
    assert_eq!(scene.background, "vn/relay_nine_bg.png");
    assert_eq!(scene.lines.len(), 3);
    assert_eq!(
        scene.lines[0].text,
        "A Dreadnought is anchoring the line. Its main battery commits before we move."
    );
    assert_eq!(scene.lines[1].text, "Then the escorts are ammunition.");
    assert_eq!(
        scene.lines[2].text,
        "Exactly. Below half integrity the battery overloads and the Dreadnought will close in."
    );
    let opening = dialogue_snapshot(scene, DialogueCursor(0));
    assert_eq!(opening.speaker, "Control");
    assert_eq!(opening.portrait, "vn/control_neutral.png");
    let middle = dialogue_snapshot(scene, DialogueCursor(1));
    assert_eq!(middle.speaker, "Vanguard");
    assert_eq!(middle.portrait, "vn/vanguard_neutral.png");
    let closing = dialogue_snapshot(scene, DialogueCursor(2));
    assert_eq!(closing.speaker, "Control");
    assert_eq!(closing.portrait, "vn/control_alert.png");

    // Aftermath: two lines, announcing the Mission 7 handoff.
    assert_eq!(definition.aftermath.background, "vn/relay_nine_bg.png");
    assert_eq!(definition.aftermath.lines.len(), 2);
    assert_eq!(
        definition.aftermath.lines[0],
        DialogueLine {
            speaker: "Vanguard",
            text: "Dreadnought down. Their line is collapsing.",
            portrait: "vn/vanguard_neutral.png",
        }
    );
    assert_eq!(
        definition.aftermath.lines[1],
        DialogueLine {
            speaker: "Control",
            text: "One command unit remains. Mission 7 is the final push.",
            portrait: "vn/control_neutral.png",
        }
    );
}

#[test]
fn dialogue_skip_is_story_only_and_duplicate_transition_is_a_noop() {
    let mut runtime = CampaignRuntime(CampaignSession {
        state: Some(CampaignState::new_game()),
        save: SaveFile::new(temp_save_path("skip-guard")),
        last_completion: None,
    });
    let before = runtime.0.state.clone();
    let mut cursor = DialogueCursor(0);
    let mut status = CampaignStatus("ready".into());
    let mut next = NextState::Unchanged;

    apply_campaign_action_for_screen(
        CampaignUiAction::SkipDialogue,
        GameScreen::Briefing,
        &mut runtime,
        None,
        &mut cursor,
        &mut status,
        &mut next,
    );
    assert_eq!(cursor, DialogueCursor(0));
    assert_eq!(status.0, "Skip is only available during pre-mission story.");
    assert_eq!(runtime.0.state, before);
    assert_eq!(pending(&next), None);

    status.0.clear();
    apply_campaign_action_for_screen(
        CampaignUiAction::SkipDialogue,
        GameScreen::PreMissionStory,
        &mut runtime,
        None,
        &mut cursor,
        &mut status,
        &mut next,
    );
    assert_eq!(cursor, DialogueCursor(2));
    assert_eq!(status.0, "");
    assert_eq!(pending(&next), Some(GameScreen::Briefing));

    let skipped_cursor = cursor;
    let skipped_state = runtime.0.state.clone();
    apply_campaign_action_for_screen(
        CampaignUiAction::AdvanceDialogue,
        GameScreen::PreMissionStory,
        &mut runtime,
        None,
        &mut cursor,
        &mut status,
        &mut next,
    );
    assert_eq!(cursor, skipped_cursor);
    assert_eq!(runtime.0.state, skipped_state);
    assert_eq!(pending(&next), Some(GameScreen::Briefing));
}

fn assert_pending_campaign_action_is_unchanged(
    label: &str,
    action: CampaignUiAction,
    current_screen: GameScreen,
    state: CampaignState,
    cursor_position: usize,
    pending_screen: GameScreen,
) {
    let path = temp_save_path(label);
    let save = SaveFile::new(path.clone());
    save.store(&state).unwrap();
    let receipt = CompletionReceipt {
        mission: MissionId::One,
        base_reward: 300,
        optional_reward: 100,
        total_reward: 400,
        credits_after: 400,
    };
    let mut runtime = CampaignRuntime(CampaignSession {
        state: Some(state),
        save,
        last_completion: Some(receipt),
    });
    let before_state = runtime.0.state.clone();
    let before_receipt = runtime.0.last_completion;
    let before_bytes = std::fs::read(&path).unwrap();
    let mut cursor = DialogueCursor(cursor_position);
    let before_cursor = cursor;
    let mut status = CampaignStatus("ready".into());
    let before_status = status.clone();
    let mut next = NextState::Pending(pending_screen);

    apply_campaign_action_for_screen(
        action,
        current_screen,
        &mut runtime,
        None,
        &mut cursor,
        &mut status,
        &mut next,
    );

    assert_eq!(cursor, before_cursor, "{label}: cursor changed");
    assert_eq!(runtime.0.state, before_state, "{label}: state changed");
    assert_eq!(
        runtime.0.last_completion, before_receipt,
        "{label}: receipt changed"
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before_bytes,
        "{label}: save changed"
    );
    assert_eq!(status, before_status, "{label}: status changed");
    assert_eq!(
        pending(&next),
        Some(pending_screen),
        "{label}: transition changed"
    );
}

#[test]
fn pending_campaign_actions_preserve_campaign_state_receipt_and_disk_bytes() {
    assert_pending_campaign_action_is_unchanged(
        "advance-last-line",
        CampaignUiAction::AdvanceDialogue,
        GameScreen::PreMissionStory,
        CampaignState::new_game(),
        2,
        GameScreen::Briefing,
    );
    assert_pending_campaign_action_is_unchanged(
        "skip-dialogue",
        CampaignUiAction::SkipDialogue,
        GameScreen::PreMissionStory,
        CampaignState::new_game(),
        0,
        GameScreen::Briefing,
    );
    assert_pending_campaign_action_is_unchanged(
        "proceed",
        CampaignUiAction::Proceed,
        GameScreen::Upgrade,
        CampaignState {
            next_mission: MissionId::Seven,
            completed: true,
            ..CampaignState::new_game()
        },
        0,
        GameScreen::PreMissionStory,
    );
}

#[test]
fn ending_snapshot_announces_campaign_complete_state() {
    // The Ending screen is reachable only when the campaign is complete
    // (a completed save always has next_mission == Mission Seven).
    let state = CampaignState {
        next_mission: MissionId::Seven,
        completed: true,
        ..CampaignState::new_game()
    };
    let snapshot = ending_snapshot(&state);
    assert_eq!(snapshot.credits, 0);
    assert_eq!(snapshot.vanguard, UpgradeLevels::default());
    assert_eq!(snapshot.gunner, UpgradeLevels::default());
    assert_eq!(snapshot.interceptor, UpgradeLevels::default());
}

#[test]
fn ending_snapshot_lists_credits_and_all_upgrade_levels() {
    let state = CampaignState {
        next_mission: MissionId::Two,
        credits: 400,
        upgrades: SquadUpgrades {
            vanguard: UpgradeLevels {
                hp: 1,
                mobility: 2,
                ..Default::default()
            },
            gunner: UpgradeLevels {
                armor: 1,
                ..Default::default()
            },
            interceptor: UpgradeLevels {
                weapon: 3,
                ..Default::default()
            },
        },
        completed: false,
    };

    let snapshot = ending_snapshot(&state);
    assert_eq!(snapshot.credits, 400);
    assert_eq!(snapshot.vanguard.hp, 1);
    assert_eq!(snapshot.vanguard.mobility, 2);
    assert_eq!(snapshot.gunner.armor, 1);
    assert_eq!(snapshot.interceptor.weapon, 3);
}

#[test]
fn vn_assets_ship_exactly_the_four_specified_png_dimensions() {
    let expected = [
        ("relay_nine_bg.png", 1280, 720),
        ("control_neutral.png", 512, 512),
        ("control_alert.png", 512, 512),
        ("vanguard_neutral.png", 512, 512),
    ];
    for (file, width, height) in expected {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets/vn")
            .join(file);
        let bytes =
            std::fs::read(&path).unwrap_or_else(|error| panic!("missing vn asset {file}: {error}"));
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "{file} is not a PNG");
        let ihdr = &bytes[16..24];
        let (actual_w, actual_h) = (
            u32::from_be_bytes(ihdr[..4].try_into().unwrap()),
            u32::from_be_bytes(ihdr[4..].try_into().unwrap()),
        );
        assert_eq!((actual_w, actual_h), (width, height), "{file}");
    }
}
