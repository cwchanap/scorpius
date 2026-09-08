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
    mission::{MissionId, mission_definition},
    presentation::{
        ActiveMission, CampaignCamera, CampaignRuntime, CanvasRoot,
        assets::UiAssets,
        campaign_ui::{
            CampaignStatus, CampaignUiAction, DialogueCursor, DialoguePip, ScreenRoot, UpgradePip,
            UpgradeRow,
        },
        screens::{
            setup_aftermath_screen, setup_briefing_screen, setup_ending_screen,
            setup_pre_mission_story, setup_title_screen, setup_upgrade_screen,
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
