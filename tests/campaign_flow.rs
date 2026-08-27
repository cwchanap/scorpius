use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use scorpius::app::{GameScreen, enter_battle, teardown_battle_screen};
use scorpius::campaign::model::{CampaignState, SquadUpgrades, UpgradeLevels};
use scorpius::campaign::save::SaveFile;
use scorpius::campaign::session::CampaignSession;
use scorpius::mission::MissionId;
use scorpius::mission::mission_definition;
use scorpius::mission::mission_one::ids;
use scorpius::presentation::campaign_ui::{
    CampaignStatus, CampaignUiAction, DialogueCursor, apply_campaign_action, briefing_copy,
    dialogue_snapshot,
};
use scorpius::presentation::ui::HudRoot;
use scorpius::presentation::{
    ActiveMission, AttackPreviewCells, BattleEventQueue, BattleRuntime, CampaignRuntime,
    EventPlayback, PresentationRoot, SelectedCell,
    interaction::{InteractionState, StatusMessage},
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
        .init_resource::<AttackPreviewCells>()
        .init_resource::<SelectedCell>();
}

fn pending(next: &NextState<GameScreen>) -> Option<GameScreen> {
    match next {
        NextState::Unchanged => None,
        NextState::Pending(state) | NextState::PendingIfNeq(state) => Some(*state),
    }
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
fn briefing_copy_lists_objectives_and_rewards() {
    let definition = mission_definition(MissionId::One).unwrap();
    let copy = briefing_copy(definition);

    for expected in [
        definition.title,
        "PRIMARY",
        definition.primary_objective,
        "BONUS",
        definition.optional_objective,
        "300 credits",
        "+100 credits",
    ] {
        assert!(copy.contains(expected), "briefing copy missing {expected}");
    }
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
        })
        .unwrap();
    let mut next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::Continue,
        &mut runtime,
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
        })
        .unwrap();
    let mut next = NextState::Unchanged;
    apply_campaign_action(
        CampaignUiAction::Continue,
        &mut runtime,
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
        &mut cursor,
        &mut status,
        &mut next,
    );
    assert_eq!(pending(&next), Some(GameScreen::Briefing));
    assert_eq!(cursor, DialogueCursor(2));
    assert!(status.0.is_empty());

    apply_campaign_action(
        CampaignUiAction::StartMission,
        &mut runtime,
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
    app.world_mut().spawn(Camera3d::default());
    app.world_mut().spawn(DirectionalLight::default());
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
    assert!(
        app.world_mut()
            .query_filtered::<(), With<Camera3d>>()
            .iter(app.world())
            .next()
            .is_none()
    );
    assert!(
        app.world_mut()
            .query_filtered::<(), With<DirectionalLight>>()
            .iter(app.world())
            .next()
            .is_none()
    );
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
