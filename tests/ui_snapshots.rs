use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use bevy::{
    app::TaskPoolPlugin, asset::AssetPlugin, prelude::*, state::app::StatesPlugin,
    ui::prelude::UiPickingCamera,
};
use scorpius::{
    campaign::{
        model::{CampaignState, SquadUpgrades},
        progression::CompletionReceipt,
        save::SaveFile,
        session::CampaignSession,
    },
    domain::{
        battle::BattleState,
        model::{Reaction, UnitId},
    },
    mission::mission_one::{ids, mission_one},
    mission::{MissionId, mission_definition},
    presentation::{
        ActiveMission, BattleRuntime, CampaignCamera, CampaignRuntime, CanvasRoot, EventPlayback,
        assets::UiAssets,
        battle_menu::{MenuRegion, MenuState, TargetingPanel, WeaponRow},
        campaign_ui::{
            CampaignStatus, CampaignUiAction, DialogueCursor, DialoguePip, ScreenRoot, UpgradePip,
            UpgradeRow,
        },
        interaction::{CommandAction, CommandButton, InteractionState, StatusMessage},
        screens::{
            setup_aftermath_screen, setup_briefing_screen, setup_ending_screen,
            setup_pre_mission_story, setup_title_screen, setup_upgrade_screen,
        },
        ui::{
            BattleHeader, BattleRightbar, BattleSidebar, InspectorPanel, PreviewText, ResultIcon,
            ResultOverlay, ThreatCard, setup_mission_ui, update_hud,
        },
    },
};

static NEXT_ID: AtomicU32 = AtomicU32::new(0);

fn test_save_path() -> PathBuf {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "scorpius-ui-snapshot-{}-{id}.json",
        std::process::id()
    ))
}

fn test_assets() -> UiAssets {
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

fn fixture_app(state: CampaignState) -> App {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        StatesPlugin,
    ))
    .init_asset::<Image>()
    .init_asset::<Font>();
    app.insert_resource(CampaignRuntime(CampaignSession {
        state: Some(state),
        save: SaveFile::new(test_save_path()),
        last_completion: None,
    }))
    .insert_resource(test_assets())
    .insert_resource(CampaignStatus::default())
    .insert_resource(DialogueCursor(0))
    .init_resource::<UiScale>()
    .init_state::<scorpius::app::GameScreen>();
    app.world_mut().spawn((
        CanvasRoot,
        Node {
            width: px(1920),
            height: px(1080),
            ..default()
        },
    ));
    app
}

fn battle_fixture_app(
    battle: scorpius::domain::battle::BattleState,
    inspected: Option<scorpius::domain::model::UnitId>,
) -> App {
    let mut interaction = InteractionState {
        inspected_unit: inspected,
        menu: MenuState::Root,
        ..Default::default()
    };
    if inspected.is_none() {
        interaction.menu = MenuState::Hidden;
    }
    let mut app = App::new();
    app.insert_resource(BattleRuntime(battle))
        .insert_resource(ActiveMission(mission_definition(MissionId::One).unwrap()))
        .insert_resource(test_assets())
        .insert_resource(interaction)
        .insert_resource(StatusMessage::default())
        .insert_resource(EventPlayback::default())
        .add_systems(Update, (setup_mission_ui, update_hud).chain());
    app
}

fn assert_campaign_root_and_camera(app: &mut App) {
    let roots = app
        .world_mut()
        .query_filtered::<Entity, With<ScreenRoot>>()
        .iter(app.world())
        .count();
    assert_eq!(roots, 1);
    let cameras = app
        .world_mut()
        .query_filtered::<Entity, (With<CampaignCamera>, With<UiPickingCamera>)>()
        .iter(app.world())
        .count();
    assert_eq!(cameras, 1);
}

fn actions(app: &mut App) -> Vec<CampaignUiAction> {
    app.world_mut()
        .query_filtered::<&CampaignUiAction, With<Pickable>>()
        .iter(app.world())
        .copied()
        .collect()
}

#[test]
fn title_and_dialogue_use_marked_campaign_controls() {
    let mut app = fixture_app(CampaignState::new_game());
    app.add_systems(Update, setup_title_screen);
    app.update();
    assert_campaign_root_and_camera(&mut app);
    let title_actions = actions(&mut app);
    assert!(title_actions.contains(&CampaignUiAction::NewGame));
    assert!(title_actions.contains(&CampaignUiAction::Continue));
    assert!(
        title_actions
            .iter()
            .all(|action| !matches!(action, CampaignUiAction::SkipDialogue))
    );

    let mut app = fixture_app(CampaignState::new_game());
    app.add_systems(Update, setup_pre_mission_story);
    app.update();
    assert_campaign_root_and_camera(&mut app);
    let story_actions = actions(&mut app);
    assert!(story_actions.contains(&CampaignUiAction::AdvanceDialogue));
    assert!(story_actions.contains(&CampaignUiAction::SkipDialogue));
    assert_eq!(
        app.world_mut()
            .query::<&DialoguePip>()
            .iter(app.world())
            .count(),
        3
    );
}

#[test]
fn briefing_and_aftermath_keep_authored_data_in_the_screen_tree() {
    let mut app = fixture_app(CampaignState::new_game());
    app.add_systems(Update, setup_briefing_screen);
    app.update();
    assert_campaign_root_and_camera(&mut app);
    assert!(actions(&mut app).contains(&CampaignUiAction::StartMission));
    assert!(
        app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .any(|text| text.0.contains("Turnabout at Relay Nine"))
    );

    let mut app = fixture_app(CampaignState {
        next_mission: MissionId::Two,
        ..CampaignState::new_game()
    });
    app.world_mut()
        .resource_mut::<CampaignRuntime>()
        .0
        .last_completion = Some(CompletionReceipt {
        mission: MissionId::One,
        base_reward: 300,
        optional_reward: 100,
        total_reward: 400,
        credits_after: 400,
    });
    app.insert_resource(ActiveMission(mission_definition(MissionId::One).unwrap()));
    app.add_systems(Update, setup_aftermath_screen);
    app.update();
    assert_campaign_root_and_camera(&mut app);
    let aftermath_actions = actions(&mut app);
    assert!(aftermath_actions.contains(&CampaignUiAction::AdvanceAftermath));
    assert!(!aftermath_actions.contains(&CampaignUiAction::SkipDialogue));
    assert!(
        app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .any(|text| text.0 == "400")
    );
}

#[test]
fn hangar_and_ending_render_typed_upgrade_controls() {
    let mut app = fixture_app(CampaignState {
        next_mission: MissionId::Two,
        credits: 500,
        upgrades: SquadUpgrades::default(),
        completed: false,
    });
    app.add_systems(
        Update,
        (
            setup_upgrade_screen,
            scorpius::presentation::campaign_ui::update_upgrade_screen,
        )
            .chain(),
    );
    app.update();
    assert_campaign_root_and_camera(&mut app);
    let upgrade_actions = actions(&mut app);
    assert_eq!(
        upgrade_actions
            .iter()
            .filter(|action| matches!(action, CampaignUiAction::PurchaseUpgrade(..)))
            .count(),
        12
    );
    assert_eq!(
        app.world_mut()
            .query::<&UpgradeRow>()
            .iter(app.world())
            .count(),
        12
    );
    assert_eq!(
        app.world_mut()
            .query::<&UpgradePip>()
            .iter(app.world())
            .count(),
        36
    );

    let mut app = fixture_app(CampaignState {
        next_mission: MissionId::Seven,
        completed: true,
        ..CampaignState::new_game()
    });
    app.add_systems(Update, setup_ending_screen);
    app.update();
    assert_campaign_root_and_camera(&mut app);
    assert!(actions(&mut app).contains(&CampaignUiAction::ReturnToTitle));
    assert_eq!(
        app.world_mut()
            .query::<&UpgradePip>()
            .iter(app.world())
            .count(),
        0
    );
}

#[test]
fn campaign_controls_are_marked_for_required_ui_picking() {
    let mut app = fixture_app(CampaignState::new_game());
    app.add_systems(Update, setup_title_screen);
    app.update();
    let mut query = app.world_mut().query::<(&CampaignUiAction, &Pickable)>();
    for (action, pickable) in query.iter(app.world()) {
        if matches!(action, CampaignUiAction::NewGame) {
            assert!(
                !matches!(*pickable, Pickable::IGNORE),
                "enabled campaign action {action:?} must remain pickable"
            );
        }
    }
}

#[test]
fn battle_snapshot_spawns_source_fixed_header_sidebar_and_menu_regions() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    battle.begin_activation(ids::VANGUARD).unwrap();
    let mut app = battle_fixture_app(battle, Some(ids::VANGUARD));
    app.update();

    let header = app
        .world_mut()
        .query_filtered::<&Node, With<BattleHeader>>()
        .single(app.world())
        .expect("fixed header must be present");
    assert_eq!(header.left, Val::Px(22.0));
    assert_eq!(header.top, Val::Px(22.0));
    assert_eq!(header.height, Val::Px(78.0));

    let sidebar = app
        .world_mut()
        .query_filtered::<&Node, With<BattleSidebar>>()
        .single(app.world())
        .expect("fixed left sidebar must be present");
    assert_eq!(sidebar.left, Val::Px(22.0));
    assert_eq!(sidebar.width, Val::Px(352.0));
    assert_eq!(sidebar.top, Val::Px(114.0));
    assert_eq!(sidebar.height, Val::Px(944.0));

    let rightbar = app
        .world_mut()
        .query_filtered::<&Node, With<BattleRightbar>>()
        .single(app.world())
        .expect("fixed right sidebar must be present");
    assert_eq!(rightbar.right, Val::Px(22.0));
    assert_eq!(rightbar.width, Val::Px(352.0));
    assert_eq!(
        app.world_mut()
            .query::<&MenuRegion>()
            .iter(app.world())
            .count(),
        3
    );
    assert_eq!(
        app.world_mut()
            .query::<&TargetingPanel>()
            .iter(app.world())
            .count(),
        1
    );
    assert_eq!(
        app.world_mut()
            .query::<&InspectorPanel>()
            .iter(app.world())
            .count(),
        1
    );
    assert!(
        app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .any(|text| text.0 == "Player Phase")
    );
}

#[test]
fn battle_snapshot_renders_inspector_art_source_preview_and_selected_threats() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    battle.begin_activation(ids::VANGUARD).unwrap();
    let mut app = battle_fixture_app(battle, Some(ids::VANGUARD));
    app.update();

    let texts: Vec<_> = app
        .world_mut()
        .query::<&Text>()
        .iter(app.world())
        .map(|text| text.0.clone())
        .collect();
    assert!(
        texts.iter().any(|text| text.contains("Vanguard")),
        "{texts:?}"
    );
    assert!(texts.iter().any(|text| text.contains("HP")), "{texts:?}");

    let threat_cards: Vec<_> = app
        .world_mut()
        .query::<(&ThreatCard, &Visibility)>()
        .iter(app.world())
        .filter(|(_, visibility)| **visibility == Visibility::Visible)
        .map(|(card, _)| card.0)
        .collect();
    assert_eq!(threat_cards, vec![0, 1]);
    assert_eq!(
        app.world_mut()
            .query::<&ThreatCard>()
            .iter(app.world())
            .count(),
        8,
        "threat cards keep stable source slots for dynamic intents"
    );
    assert!(
        texts.iter().any(|text| text.contains("Striker")),
        "{texts:?}"
    );
    assert!(
        texts.iter().any(|text| text.contains("Artillery")),
        "{texts:?}"
    );
    assert!(texts.iter().any(|text| text.contains("DMG")), "{texts:?}");
    assert_eq!(
        app.world_mut()
            .query::<&WeaponRow>()
            .iter(app.world())
            .count(),
        3
    );

    let preview = app
        .world_mut()
        .query_filtered::<&Text, With<PreviewText>>()
        .single(app.world())
        .expect("preview panel must be present");
    assert!(preview.0.contains("TARGET PREVIEW"));
}

fn terminal_battle(victory: bool) -> scorpius::domain::battle::BattleState {
    if victory {
        let mut battle = BattleState::viability_fixture();
        battle
            .choose_reaction(UnitId(1), Reaction::Guard)
            .expect("fixture pilot can choose a reaction");
        battle
            .finish_activation(UnitId(1))
            .expect("fixture pilot can finish");
        battle
            .resolve_enemy_phase()
            .expect("empty enemy phase resolves terminal victory");
        assert!(battle.result().is_some_and(|result| result.victory));
        battle
    } else {
        let mut battle = mission_one(7);
        battle
            .begin_round()
            .expect("source opening enters player phase");
        for _ in 0..8 {
            battle
                .resolve_push(ids::STRIKER, ids::VANGUARD)
                .expect("source push path damages the pilot at the board edge");
            if battle.result().is_some() {
                break;
            }
            if battle
                .unit(ids::VANGUARD)
                .is_some_and(|unit| unit.is_knocked_out())
            {
                break;
            }
        }
        for (id, destination, collisions) in [
            (ids::GUNNER, scorpius::domain::board::GridPos::new(4, 7), 5),
            (
                ids::INTERCEPTOR,
                scorpius::domain::board::GridPos::new(4, 7),
                6,
            ),
        ] {
            battle
                .begin_activation(id)
                .expect("remaining pilot activates");
            battle
                .move_unit(id, destination)
                .expect("remaining pilot moves into the source push lane");
            battle
                .choose_reaction(id, Reaction::Guard)
                .expect("remaining pilot guards");
            battle
                .finish_activation(id)
                .expect("remaining pilot finishes");
            battle
                .resolve_push(ids::STRIKER, id)
                .expect("source push starts the edge collision sequence");
            for _ in 0..collisions {
                if battle.unit(id).is_some_and(|unit| unit.is_knocked_out()) {
                    break;
                }
                battle
                    .resolve_push(ids::STRIKER, id)
                    .expect("source push collision damages the pilot");
            }
            if battle.result().is_some() {
                break;
            }
        }
        assert!(battle.result().is_some_and(|result| !result.victory));
        battle
    }
}

fn result_button_state(app: &mut App, action: CommandAction) -> (Visibility, bool) {
    app.world_mut()
        .query::<(&CommandButton, &Visibility, &Pickable)>()
        .iter(app.world())
        .find(|(button, _, _)| button.0 == action)
        .map(|(_, visibility, pickable)| (*visibility, pickable.is_hoverable))
        .expect("result action must have a pickable button")
}

#[test]
fn battle_result_snapshot_renders_terminal_victory_overlay_and_metrics() {
    let mut app = battle_fixture_app(terminal_battle(true), None);
    app.update();

    let overlay = app
        .world_mut()
        .query_filtered::<&Visibility, With<ResultOverlay>>()
        .single(app.world())
        .expect("result overlay must be present");
    assert_eq!(*overlay, Visibility::Visible);
    let texts: Vec<_> = app
        .world_mut()
        .query::<&Text>()
        .iter(app.world())
        .map(|text| text.0.clone())
        .collect();
    assert!(texts.iter().any(|text| text.contains("MISSION COMPLETE")));
    assert!(texts.iter().any(|text| text == "CLEAR"));
    assert!(texts.iter().any(|text| text == "MISSED"));
    let icon = app
        .world_mut()
        .query_filtered::<&ImageNode, With<ResultIcon>>()
        .single(app.world())
        .expect("victory icon must be rendered");
    assert_eq!(icon.rect, Some(scorpius::presentation::theme::ICON_WAIT));
    assert_eq!(icon.color, scorpius::presentation::theme::MINT);
    assert_eq!(
        result_button_state(&mut app, CommandAction::ContinueVictory),
        (Visibility::Visible, true)
    );
    assert_eq!(
        result_button_state(&mut app, CommandAction::Restart),
        (Visibility::Hidden, false)
    );
}

#[test]
fn battle_result_snapshot_renders_terminal_defeat_overlay_and_metrics() {
    let mut app = battle_fixture_app(terminal_battle(false), None);
    app.update();

    let overlay = app
        .world_mut()
        .query_filtered::<&Visibility, With<ResultOverlay>>()
        .single(app.world())
        .expect("result overlay must be present");
    assert_eq!(*overlay, Visibility::Visible);
    let texts: Vec<_> = app
        .world_mut()
        .query::<&Text>()
        .iter(app.world())
        .map(|text| text.0.clone())
        .collect();
    assert!(texts.iter().any(|text| text.contains("MISSION FAILED")));
    assert!(texts.iter().any(|text| text == "FAILED"));
    assert!(texts.iter().any(|text| text == "MISSED"));
    let icon = app
        .world_mut()
        .query_filtered::<&ImageNode, With<ResultIcon>>()
        .single(app.world())
        .expect("defeat icon must be rendered");
    assert_eq!(icon.rect, Some(scorpius::presentation::theme::ICON_COUNTER));
    assert_eq!(icon.color, scorpius::presentation::theme::ENEMY);
    assert_eq!(
        result_button_state(&mut app, CommandAction::Restart),
        (Visibility::Visible, true)
    );
    assert_eq!(
        result_button_state(&mut app, CommandAction::ContinueVictory),
        (Visibility::Hidden, false)
    );
}
