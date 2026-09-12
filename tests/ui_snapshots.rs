use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use bevy::{
    a11y::AccessibilityNode,
    app::TaskPoolPlugin,
    asset::AssetPlugin,
    image::{ImagePlugin, TextureAtlasPlugin},
    input::InputPlugin,
    picking::{InteractionPlugin, PickingPlugin},
    prelude::*,
    state::app::StatesPlugin,
    text::{FontSize, TextPlugin},
    time::TimePlugin,
    ui::{UiGlobalTransform, UiPlugin, prelude::UiPickingCamera},
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
        board::GridPos,
        model::{Reaction, UnitId},
    },
    mission::mission_one::{ids, mission_one},
    mission::{MissionId, mission_definition},
    presentation::{
        ActiveMission, BattleRuntime, CampaignCamera, CampaignRuntime, CanvasRoot, EventPlayback,
        RecentBattleLog,
        assets::UiAssets,
        battle_menu::{
            MenuRegion, MenuState, ResolveButton, TargetingIcon, TargetingPanel, WeaponRow,
            WeaponText, WeaponTextKind, update_battle_menu,
        },
        campaign_ui::{
            CampaignStatus, CampaignUiAction, DialogueCursor, DialoguePip, ScreenRoot,
            UpgradeCostText, UpgradePip, UpgradePurchaseIcon, UpgradeRow, UpgradeTrackIcon,
        },
        interaction::{
            CommandAction, CommandButton, InteractionMode, InteractionState, StatusMessage,
        },
        screens::{
            setup_aftermath_screen, setup_briefing_screen, setup_ending_screen,
            setup_pre_mission_story, setup_title_screen, setup_upgrade_screen,
        },
        theme,
        ui::{
            BattleHeader, BattleRightbar, BattleSidebar, InspectorEmpty, InspectorMeter,
            InspectorMeterKind, InspectorPanel, InspectorStats, InspectorText, InspectorTextKind,
            InspectorTop, LogEntryText, PreviewMeter, PreviewPanel, PreviewText, PreviewValue,
            PreviewValueKind, ResultHeadline, ResultIcon, ResultMetricIcon, ResultOverlay,
            ResultRing, ResultStatus, ThreatCard, ThreatMeter, ThreatMeterKind, ThreatText,
            ThreatTextKind, setup_mission_ui, update_hud,
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
        .init_resource::<scorpius::presentation::RecentBattleLog>()
        .add_systems(Startup, setup_mission_ui)
        .add_systems(Update, (update_hud, update_battle_menu).chain());
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
    let story_labels: Vec<_> = app
        .world_mut()
        .query::<(&CampaignUiAction, &AccessibilityNode)>()
        .iter(app.world())
        .map(|(action, node)| (*action, node.label().map(str::to_owned)))
        .collect();
    assert!(story_labels.contains(&(
        CampaignUiAction::AdvanceDialogue,
        Some("Next dialogue".to_owned()),
    )));
    assert!(story_labels.contains(&(
        CampaignUiAction::SkipDialogue,
        Some("Skip dialogue".to_owned()),
    )));
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
    let aftermath_texts: Vec<_> = app
        .world_mut()
        .query::<&Text>()
        .iter(app.world())
        .map(|text| text.0.clone())
        .collect();
    for label in ["BASE", "BONUS", "TOTAL", "CREDITS"] {
        assert!(
            aftermath_texts.iter().any(|text| text.as_str() == label),
            "aftermath receipt must show {label}: {aftermath_texts:?}"
        );
    }
    assert!(aftermath_texts.iter().any(|text| text.as_str() == "300"));
    assert!(aftermath_texts.iter().any(|text| text.as_str() == "+100"));
    assert_eq!(
        aftermath_texts
            .iter()
            .filter(|text| text.as_str() == "400")
            .count(),
        2,
        "aftermath TOTAL and CREDITS both read 400: {aftermath_texts:?}"
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
        optional_reward: 0,
        total_reward: 300,
        credits_after: 300,
    });
    app.insert_resource(ActiveMission(mission_definition(MissionId::One).unwrap()));
    app.add_systems(Update, setup_aftermath_screen);
    app.update();
    let base_texts: Vec<_> = app
        .world_mut()
        .query::<&Text>()
        .iter(app.world())
        .map(|text| text.0.clone())
        .collect();
    assert!(base_texts.iter().any(|text| text.as_str() == "+0"));
    assert_eq!(
        base_texts
            .iter()
            .filter(|text| text.as_str() == "300")
            .count(),
        3,
        "base-only aftermath BASE, TOTAL, and CREDITS all read 300: {base_texts:?}"
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
    let row_texts: Vec<_> = app
        .world_mut()
        .query::<(&UpgradeRow, &Text)>()
        .iter(app.world())
        .map(|(_, text)| text.0.clone())
        .collect();
    assert_eq!(row_texts.len(), 12);
    assert!(row_texts.iter().all(|text| !text.contains("→")));
    assert!(row_texts.iter().all(|text| text == "+3 MAX HP"
        || text == "+1 ARMOR"
        || text == "+5 EVASION"
        || text == "+1 WEAPON DMG"));
    let costs: Vec<_> = app
        .world_mut()
        .query::<(&UpgradeCostText, &Text)>()
        .iter(app.world())
        .map(|(_, text)| text.0.clone())
        .collect();
    assert_eq!(costs.len(), 12);
    assert!(costs.iter().all(|cost| cost == "200"));
    assert_eq!(
        app.world_mut()
            .query::<&UpgradePurchaseIcon>()
            .iter(app.world())
            .count(),
        12
    );
    assert_eq!(
        app.world_mut()
            .query::<&UpgradeTrackIcon>()
            .iter(app.world())
            .count(),
        12
    );
    let pip_sizes: Vec<_> = app
        .world_mut()
        .query::<(&UpgradePip, &Node)>()
        .iter(app.world())
        .map(|(_, node)| (node.width, node.height))
        .collect();
    assert!(
        pip_sizes
            .iter()
            .all(|(width, height)| { *width == Val::Px(26.0) && *height == Val::Px(8.0) })
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
    let texts: Vec<_> = app
        .world_mut()
        .query::<&Text>()
        .iter(app.world())
        .map(|text| text.0.clone())
        .collect();
    assert!(!texts.iter().any(|text| text == "PLAYER PHASE"));
    assert!(
        !texts
            .iter()
            .any(|text| text.contains("SCORPIUS // COMBAT LINK"))
    );
    assert!(!texts.iter().any(|text| text.contains("[M] MOVE")));
}

#[test]
fn battle_snapshot_shows_resolve_after_every_pilot_finishes() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    for id in [ids::VANGUARD, ids::GUNNER, ids::INTERCEPTOR] {
        battle.begin_activation(id).unwrap();
        battle.choose_reaction(id, Reaction::Guard).unwrap();
        battle.finish_activation(id).unwrap();
    }
    assert!(battle.ready_to_resolve());

    let mut app = battle_fixture_app(battle, None);
    app.update();

    let (visibility, node, background, pickable) = app
        .world_mut()
        .query_filtered::<(&Visibility, &Node, &BackgroundColor, &Pickable), With<ResolveButton>>()
        .single(app.world())
        .expect("resolve button must be spawned");
    assert_eq!(*visibility, Visibility::Visible);
    assert_eq!(node.display, Display::Flex);
    assert_eq!(background.0, Color::srgb_u8(63, 42, 6));
    assert!(pickable.is_hoverable);
}

#[test]
fn battle_snapshot_renders_recent_playback_log_entries() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    battle.begin_activation(ids::VANGUARD).unwrap();
    let mut app = battle_fixture_app(battle, Some(ids::VANGUARD));
    app.world_mut()
        .resource_mut::<RecentBattleLog>()
        .push("VANGUARD -> STRIKER\nHIT".to_owned());

    app.update();

    let playback = app
        .world_mut()
        .query::<(&LogEntryText, &Text)>()
        .iter(app.world())
        .find(|(entry, _)| entry.0 == 0)
        .map(|(_, text)| text)
        .expect("playback log text must be spawned");
    assert_eq!(playback.0, "VANGUARD -> STRIKER · HIT");
}

#[test]
fn battle_log_text_stays_visible_inside_the_sidebar_layout() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    battle.begin_activation(ids::VANGUARD).unwrap();
    let mut app = battle_fixture_app(battle, Some(ids::VANGUARD));
    app.world_mut()
        .resource_mut::<RecentBattleLog>()
        .push("VANGUARD -> STRIKER\nHIT".to_owned());
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ImagePlugin::default(),
        TextureAtlasPlugin,
        InputPlugin,
        PickingPlugin,
        InteractionPlugin,
        TimePlugin,
        TextPlugin,
        TransformPlugin,
        UiPlugin,
    ));
    let font = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(
            std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/fonts/ibm-plex-mono-400.ttf"
            ))
            .expect("test layout font must be present"),
        ));
    let mut assets = test_assets();
    assets.fonts = std::array::from_fn(|_| font.clone());
    app.insert_resource(assets);

    app.update();
    app.update();

    let log = app
        .world_mut()
        .query_filtered::<(Entity, &LogEntryText), With<LogEntryText>>()
        .iter(app.world())
        .find_map(|(entity, entry)| (entry.0 == 0).then_some(entity))
        .expect("playback log node must be spawned");
    let playback_node = app
        .world()
        .get::<Node>(log)
        .expect("playback log node style must be present");
    let log_entry = app
        .world()
        .get::<ChildOf>(log)
        .expect("playback log must stay under its entry row")
        .parent();
    let log_panel = app
        .world()
        .get::<ChildOf>(log_entry)
        .expect("playback log entry must stay under its clipping panel")
        .parent();
    let panel_node = app
        .world()
        .get::<Node>(log_panel)
        .expect("log panel style must be present");
    assert_eq!(playback_node.min_height, Val::Auto);
    assert_eq!(playback_node.overflow, Overflow::visible());
    assert_eq!(panel_node.overflow, Overflow::clip());
    let playback_rect = computed_rect(&app, log);
    let playback_width = app
        .world()
        .get::<ComputedNode>(log)
        .expect("playback log layout must be computed")
        .size
        .x;
    let local_visible = app
        .world()
        .get::<Visibility>(log)
        .is_some_and(|visibility| *visibility == Visibility::Visible);
    let sidebar = app
        .world_mut()
        .query_filtered::<Entity, With<BattleSidebar>>()
        .single(app.world())
        .expect("sidebar must be present");
    let sidebar_rect = computed_rect(&app, sidebar);

    assert!(playback_width > 0.0);
    // The headless fixture does not rasterize glyph height, but it still
    // computes the live width, inherited visibility, and hierarchy bounds.
    assert!(playback_width > 0.0);
    assert!(local_visible);
    assert!(playback_rect.min.x >= sidebar_rect.min.x - 0.5);
    assert!(playback_rect.max.x <= sidebar_rect.max.x + 0.5);
    assert!(playback_rect.min.y >= sidebar_rect.min.y - 0.5);
    assert!(playback_rect.max.y <= sidebar_rect.max.y + 0.5);
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
    assert!(texts.iter().any(|text| text == "20/20"), "{texts:?}");

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
    assert!(!texts.iter().any(|text| text.contains("DMG")), "{texts:?}");
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
    assert_eq!(preview.0, "PREVIEW");
    let preview_panel = app
        .world_mut()
        .query_filtered::<(&Visibility, &Node), With<PreviewPanel>>()
        .single(app.world())
        .expect("preview panel must be present");
    assert_eq!(*preview_panel.0, Visibility::Hidden);
    assert_eq!(preview_panel.1.display, Display::None);
}

#[test]
fn battle_sidebar_binds_typed_preview_threat_and_icon_only_weapon_rows() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    battle.begin_activation(ids::VANGUARD).unwrap();
    let mut app = battle_fixture_app(battle, Some(ids::VANGUARD));
    app.update();

    let weapon_names: Vec<_> = app
        .world_mut()
        .query::<(&WeaponText, &Text, &Node)>()
        .iter(app.world())
        .filter(|(weapon, _, _)| weapon.kind == WeaponTextKind::Name)
        .map(|(_, text, node)| (text.0.clone(), node.display))
        .collect();
    assert_eq!(
        weapon_names,
        vec![
            ("Pile Lance".to_owned(), Display::None),
            ("Repulsor Ram".to_owned(), Display::None),
            ("Anchor Cannon".to_owned(), Display::None),
        ]
    );
    let accessible_weapon_names: Vec<_> = app
        .world_mut()
        .query::<(&WeaponRow, &AccessibilityNode)>()
        .iter(app.world())
        .map(|(_, node)| node.label().map(str::to_owned))
        .collect();
    assert_eq!(
        accessible_weapon_names,
        vec![
            Some("Pile Lance".to_owned()),
            Some("Repulsor Ram".to_owned()),
            Some("Anchor Cannon".to_owned()),
        ],
        "icon-only weapon rows retain native button names"
    );

    let inspector_values: Vec<_> = app
        .world_mut()
        .query::<(&InspectorText, &Text, &TextFont)>()
        .iter(app.world())
        .filter_map(|(kind, text, font)| match kind.0 {
            InspectorTextKind::Hp | InspectorTextKind::En => {
                Some((kind.0, text.0.clone(), font.font_size))
            }
            _ => None,
        })
        .collect();
    assert!(inspector_values.iter().any(|(kind, value, size)| {
        *kind == InspectorTextKind::Hp
            && value == "20/20"
            && matches!(size, FontSize::Px(value) if (value - 16.0).abs() < f32::EPSILON)
    }));
    assert!(inspector_values.iter().any(|(kind, value, size)| {
        *kind == InspectorTextKind::En
            && value == "7/7"
            && matches!(size, FontSize::Px(value) if (value - 16.0).abs() < f32::EPSILON)
    }));
    let en_color = app
        .world_mut()
        .query::<(&InspectorText, &TextColor)>()
        .iter(app.world())
        .find(|(kind, _)| kind.0 == InspectorTextKind::En)
        .map(|(_, color)| color.0)
        .expect("inspector EN value must keep its source color");
    assert_eq!(en_color, theme::INSPECTOR_EN_TEXT);
    let en_pips: Vec<_> = app
        .world_mut()
        .query::<(&InspectorMeter, &BackgroundColor)>()
        .iter(app.world())
        .filter_map(|(meter, background)| match meter.0 {
            InspectorMeterKind::Energy(index) => Some((index, background.0)),
            InspectorMeterKind::Hp => None,
        })
        .collect();
    assert_eq!(en_pips.len(), 9);
    assert!(en_pips.iter().all(|(index, color)| {
        *color
            == if *index < 7 {
                theme::INSPECTOR_EN_PIP_ACTIVE
            } else {
                theme::INSPECTOR_EN_PIP_INACTIVE
            }
    }));
    let inspector_style = app
        .world_mut()
        .query_filtered::<(&BackgroundColor, &Outline), With<InspectorPanel>>()
        .single(app.world())
        .expect("selected inspector must keep its source card style");
    assert_eq!(inspector_style.0.0, theme::INSPECTOR_PLAYER_BACKGROUND);
    assert_eq!(inspector_style.1.color, theme::ACCENT);
    let inspector_field_colors: Vec<_> = app
        .world_mut()
        .query::<(&InspectorText, &TextColor)>()
        .iter(app.world())
        .map(|(kind, color)| (kind.0, color.0))
        .collect();
    assert!(inspector_field_colors.contains(&(InspectorTextKind::Hp, theme::INSPECTOR_HP_TEXT)));
    assert!(inspector_field_colors.contains(&(InspectorTextKind::En, theme::INSPECTOR_EN_TEXT)));
    assert!(
        inspector_field_colors.contains(&(InspectorTextKind::Armor, theme::INSPECTOR_STAT_TEXT))
    );
    assert!(
        inspector_field_colors.contains(&(InspectorTextKind::Movement, theme::INSPECTOR_STAT_TEXT))
    );
    assert!(
        inspector_field_colors.contains(&(InspectorTextKind::Evasion, theme::INSPECTOR_STAT_TEXT))
    );
    app.world_mut()
        .resource_mut::<InteractionState>()
        .inspected_unit = Some(ids::STRIKER);
    app.update();
    let enemy_inspector_style = app
        .world_mut()
        .query_filtered::<(&BackgroundColor, &Outline), With<InspectorPanel>>()
        .single(app.world())
        .expect("enemy inspector must keep its source card style");
    assert_eq!(enemy_inspector_style.0.0, theme::INSPECTOR_ENEMY_BACKGROUND);
    assert_eq!(enemy_inspector_style.1.color, theme::INSPECTOR_ENEMY_BORDER);
    app.world_mut()
        .resource_mut::<InteractionState>()
        .inspected_unit = Some(ids::VANGUARD);
    app.update();

    let threat_texts: Vec<_> = app
        .world_mut()
        .query::<(&ThreatText, &Text, &TextFont)>()
        .iter(app.world())
        .filter(|(threat, _, _)| threat.card == 0)
        .map(|(threat, text, font)| (threat.kind, text.0.clone(), font.font_size))
        .collect();
    assert_eq!(threat_texts.len(), 3);
    assert!(threat_texts.iter().any(|(kind, value, size)| {
        *kind == ThreatTextKind::Attacker
            && value == "Striker"
            && matches!(size, FontSize::Px(value) if (value - 19.0).abs() < f32::EPSILON)
    }));
    assert!(threat_texts.iter().any(|(kind, value, size)| {
        *kind == ThreatTextKind::Damage
            && value == "4"
            && matches!(size, FontSize::Px(value) if (value - 38.0).abs() < f32::EPSILON)
    }));
    assert!(threat_texts.iter().any(|(kind, value, size)| {
        *kind == ThreatTextKind::Hit
            && value.ends_with('%')
            && matches!(size, FontSize::Px(value) if (value - 21.0).abs() < f32::EPSILON)
    }));
    assert!(
        !threat_texts.iter().any(|(kind, _, _)| {
            matches!(kind, ThreatTextKind::Weapon | ThreatTextKind::Target)
        })
    );
    let threat_shapes: Vec<_> = app
        .world_mut()
        .query::<(&ThreatMeter, &Node)>()
        .iter(app.world())
        .filter(|(meter, _)| matches!(meter.0, ThreatMeterKind::Shape { card: 0, .. }))
        .collect();
    assert_eq!(threat_shapes.len(), 9);
    assert!(
        threat_shapes
            .iter()
            .all(|(_, node)| { node.width == px(12.0) && node.height == px(12.0) })
    );

    let preview = app
        .world()
        .resource::<BattleRuntime>()
        .0
        .preview_attack(ids::VANGUARD, ids::REPULSOR_RAM, GridPos::new(4, 6))
        .unwrap();
    let expected_preview = (
        preview.normal_damage.to_string(),
        format!("/{}", preview.critical_damage),
        format!("{}%", preview.hit_chance),
        preview.en_cost.max(0) as usize,
    );
    app.world_mut().resource_mut::<InteractionState>().preview = Some(preview);
    app.update();

    let preview_panel = app
        .world_mut()
        .query_filtered::<(&Visibility, &Node), With<PreviewPanel>>()
        .single(app.world())
        .unwrap();
    assert_eq!(*preview_panel.0, Visibility::Visible);
    assert_eq!(preview_panel.1.display, Display::Flex);
    let preview_values: Vec<_> = app
        .world_mut()
        .query::<(&PreviewValue, &Text)>()
        .iter(app.world())
        .map(|(value, text)| (value.0, text.0.clone()))
        .collect();
    assert!(preview_values.contains(&(PreviewValueKind::Damage, expected_preview.0)));
    assert!(preview_values.contains(&(PreviewValueKind::Critical, expected_preview.1)));
    assert!(preview_values.contains(&(PreviewValueKind::Hit, expected_preview.2)));
    let preview_meters: Vec<_> = app
        .world_mut()
        .query::<(&PreviewMeter, &BackgroundColor)>()
        .iter(app.world())
        .collect();
    assert_eq!(preview_meters.len(), 5);
    assert!(preview_meters.iter().all(|(meter, background)| {
        background.0
            == if meter.0 < expected_preview.3 {
                theme::GOLD
            } else {
                theme::BORDER
            }
    }));

    app.world_mut().resource_mut::<InteractionState>().mode = InteractionMode::Move;
    app.update();
    let targeting_icons: Vec<_> = app
        .world_mut()
        .query::<(&TargetingIcon, &Visibility)>()
        .iter(app.world())
        .map(|(icon, visibility)| (icon.move_mode, *visibility))
        .collect();
    assert!(targeting_icons.contains(&(true, Visibility::Visible)));
    assert!(targeting_icons.contains(&(false, Visibility::Hidden)));
    app.world_mut().resource_mut::<InteractionState>().mode =
        InteractionMode::Attack(ids::REPULSOR_RAM);
    app.update();
    let targeting_icons: Vec<_> = app
        .world_mut()
        .query::<(&TargetingIcon, &Visibility)>()
        .iter(app.world())
        .map(|(icon, visibility)| (icon.move_mode, *visibility))
        .collect();
    assert!(targeting_icons.contains(&(true, Visibility::Hidden)));
    assert!(targeting_icons.contains(&(false, Visibility::Visible)));
    let cancel = app
        .world_mut()
        .query::<(&CommandButton, &BackgroundColor, &BorderColor, &Node)>()
        .iter(app.world())
        .find(|(button, _, _, _)| button.0 == CommandAction::Cancel)
        .map(|(_, background, border, node)| (background.0, *border, node.border))
        .expect("targeting cancel button must be present");
    assert_eq!(cancel.0, theme::TARGETING_CANCEL_BACKGROUND);
    assert_eq!(cancel.1, BorderColor::all(theme::TARGETING_CANCEL_BORDER));
    assert_eq!(cancel.2, UiRect::all(px(2.0)));

    let mut empty_app = battle_fixture_app(mission_one(7), None);
    empty_app.update();
    let empty_panel = empty_app
        .world_mut()
        .query_filtered::<&Node, With<InspectorPanel>>()
        .single(empty_app.world())
        .unwrap();
    assert_eq!(empty_panel.padding, UiRect::ZERO);
    let empty_state = empty_app
        .world_mut()
        .query_filtered::<(&Node, &Visibility), With<InspectorEmpty>>()
        .single(empty_app.world())
        .unwrap();
    assert_eq!(empty_state.0.height, px(150));
    assert_eq!(empty_state.0.display, Display::Flex);
    assert_eq!(*empty_state.1, Visibility::Visible);
    assert!(
        empty_app
            .world_mut()
            .query::<&Text>()
            .iter(empty_app.world())
            .all(|text| text.0 != "SELECT A MECH")
    );

    empty_app
        .world_mut()
        .resource_mut::<InteractionState>()
        .inspected_unit = Some(ids::VANGUARD);
    empty_app.update();
    let active_panel = empty_app
        .world_mut()
        .query_filtered::<&Node, With<InspectorPanel>>()
        .single(empty_app.world())
        .unwrap();
    assert_eq!(active_panel.padding, UiRect::all(px(16)));
    let active_empty = empty_app
        .world_mut()
        .query_filtered::<(&Node, &Visibility), With<InspectorEmpty>>()
        .single(empty_app.world())
        .unwrap();
    assert_eq!(active_empty.0.display, Display::None);
    assert_eq!(*active_empty.1, Visibility::Hidden);
}

#[test]
fn inspector_children_stay_within_the_fixed_panel_bounds() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    battle.begin_activation(ids::VANGUARD).unwrap();
    let mut app = battle_fixture_app(battle, Some(ids::VANGUARD));
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ImagePlugin::default(),
        TextureAtlasPlugin,
        InputPlugin,
        PickingPlugin,
        InteractionPlugin,
        TimePlugin,
        TextPlugin,
        TransformPlugin,
        UiPlugin,
    ));
    app.update();

    let panel = app
        .world_mut()
        .query_filtered::<Entity, With<InspectorPanel>>()
        .single(app.world())
        .expect("one inspector panel");
    let panel_rect = computed_rect(&app, panel);
    let mut children = app.world_mut().query_filtered::<Entity, Or<(
        With<InspectorTop>,
        With<InspectorStats>,
        With<InspectorEmpty>,
    )>>();
    let direct_children: Vec<_> = children
        .iter(app.world())
        .filter(|entity| {
            app.world()
                .get::<ChildOf>(*entity)
                .is_some_and(|parent| parent.parent() == panel)
        })
        .collect();
    assert_eq!(direct_children.len(), 3);

    for child in direct_children {
        let rect = computed_rect(&app, child);
        assert!(
            rect.min.x >= panel_rect.min.x - 0.5,
            "inspector child starts outside panel: {rect:?} vs {panel_rect:?}"
        );
        assert!(
            rect.max.x <= panel_rect.max.x + 0.5,
            "inspector child ends outside panel: {rect:?} vs {panel_rect:?}"
        );
    }
}

fn computed_rect(app: &App, entity: Entity) -> Rect {
    let node = app
        .world()
        .get::<ComputedNode>(entity)
        .expect("layout must compute every inspector node");
    let transform = app
        .world()
        .get::<UiGlobalTransform>(entity)
        .expect("layout must compute every inspector transform");
    let (_, _, center) = transform.to_scale_angle_translation();
    Rect::from_center_size(center, node.size)
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

fn result_button_state(app: &mut App, action: CommandAction) -> (Visibility, bool, Display) {
    let overlay = app
        .world_mut()
        .query_filtered::<Entity, With<ResultOverlay>>()
        .single(app.world())
        .expect("one result overlay");
    let result_card = app
        .world_mut()
        .query::<(Entity, &ChildOf)>()
        .iter(app.world())
        .find_map(|(entity, parent)| (parent.parent() == overlay).then_some(entity))
        .expect("result card must be a child of the result overlay");
    app.world_mut()
        .query::<(&CommandButton, &Visibility, &Pickable, &Node, &ChildOf)>()
        .iter(app.world())
        .find(|(button, _, _, _, parent)| button.0 == action && parent.parent() == result_card)
        .map(|(_, visibility, pickable, node, _)| {
            (*visibility, pickable.is_hoverable, node.display)
        })
        .expect("result action must have a pickable button")
}

fn result_button_style(app: &mut App, action: CommandAction) -> (Color, BorderColor, UiRect) {
    let overlay = app
        .world_mut()
        .query_filtered::<Entity, With<ResultOverlay>>()
        .single(app.world())
        .expect("one result overlay");
    let result_card = app
        .world_mut()
        .query::<(Entity, &ChildOf)>()
        .iter(app.world())
        .find_map(|(entity, parent)| (parent.parent() == overlay).then_some(entity))
        .expect("result card must be a child of the result overlay");
    app.world_mut()
        .query::<(
            &CommandButton,
            &BackgroundColor,
            &BorderColor,
            &Node,
            &ChildOf,
        )>()
        .iter(app.world())
        .find(|(button, _, _, _, parent)| button.0 == action && parent.parent() == result_card)
        .map(|(_, background, border, node, _)| (background.0, *border, node.border))
        .expect("result action style must be present")
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
    assert!(texts.iter().any(|text| text == "RELAY SECURED"));
    assert!(texts.iter().any(|text| text == "0/0"));
    let icon = app
        .world_mut()
        .query_filtered::<&ImageNode, With<ResultIcon>>()
        .single(app.world())
        .expect("victory icon must be rendered");
    assert_eq!(
        icon.rect,
        Some(scorpius::presentation::theme::RESULT_VICTORY_RECT)
    );
    assert_eq!(icon.color, Color::WHITE);
    let ring = app
        .world_mut()
        .query_filtered::<&Node, With<ResultRing>>()
        .single(app.world())
        .expect("result ring must be present");
    assert_eq!(ring.width, Val::Px(168.0));
    assert_eq!(ring.height, Val::Px(168.0));
    let headline = app
        .world_mut()
        .query_filtered::<&TextFont, With<ResultHeadline>>()
        .single(app.world())
        .expect("result headline must be present");
    assert_eq!(headline.font_size, FontSize::Px(46.0));
    assert_eq!(
        app.world_mut()
            .query::<&ResultMetricIcon>()
            .iter(app.world())
            .count(),
        2
    );
    assert_eq!(
        result_button_state(&mut app, CommandAction::ContinueVictory),
        (Visibility::Visible, true, Display::Flex)
    );
    assert_eq!(
        result_button_style(&mut app, CommandAction::ContinueVictory),
        (
            theme::PANEL_RAISED,
            BorderColor::all(theme::ACCENT),
            UiRect::all(px(1.0))
        )
    );
    assert_eq!(
        result_button_state(&mut app, CommandAction::Restart),
        (Visibility::Hidden, false, Display::None)
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
    assert!(texts.iter().any(|text| text == "0/4"));
    assert_eq!(
        app.world_mut()
            .query_filtered::<&Text, With<ResultHeadline>>()
            .single(app.world())
            .expect("defeat headline must be present")
            .0,
        "MISSION FAILED"
    );
    let icon = app
        .world_mut()
        .query_filtered::<&ImageNode, With<ResultIcon>>()
        .single(app.world())
        .expect("defeat icon must be rendered");
    assert_eq!(
        icon.rect,
        Some(scorpius::presentation::theme::RESULT_DEFEAT_RECT)
    );
    assert_eq!(icon.color, Color::WHITE);
    assert_eq!(
        result_button_state(&mut app, CommandAction::Restart),
        (Visibility::Visible, true, Display::Flex)
    );
    assert_eq!(
        result_button_style(&mut app, CommandAction::Restart),
        (
            theme::PANEL_RAISED,
            BorderColor::all(theme::ACCENT),
            UiRect::all(px(1.0))
        )
    );
    assert_eq!(
        result_button_state(&mut app, CommandAction::ContinueVictory),
        (Visibility::Hidden, false, Display::None)
    );
}

#[test]
fn battle_result_snapshot_surfaces_save_errors_beside_continue() {
    let mut app = battle_fixture_app(terminal_battle(true), None);
    app.world_mut().resource_mut::<StatusMessage>().0 =
        "save file error: Is a directory (os error 21)".to_owned();
    app.update();

    let (error, node) = app
        .world_mut()
        .query_filtered::<(&Text, &Node), With<ResultStatus>>()
        .single(app.world())
        .expect("result save status must be present");
    assert_eq!(error.0, "save file error: Is a directory (os error 21)");
    assert_eq!(node.display, Display::Flex);
    assert_eq!(
        result_button_state(&mut app, CommandAction::ContinueVictory),
        (Visibility::Visible, true, Display::Flex)
    );
}
