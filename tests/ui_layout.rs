use bevy::{
    app::{Startup, TaskPoolPlugin},
    asset::{AssetPlugin, Assets},
    camera::{Camera2d, RenderTarget, RenderTargetInfo, Viewport},
    image::{Image, ImagePlugin, TextureAtlasPlugin},
    input::InputPlugin,
    picking::{
        PickingSystems,
        events::{Click, Move, Pointer},
        hover::HoverMap,
        pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput},
        prelude::{InteractionPlugin, Pickable, PickingPlugin},
    },
    prelude::{
        App, Button, ChildOf, Commands, Component, Entity, Handle, InheritedVisibility,
        IntoScheduleConfigs, Node, On, Query, Rect, Res, ResMut, Resource, Text, TransformPlugin,
        UiGlobalTransform, UiPickingSettings, UiScale, Val, Vec2, Visibility, Window, With,
    },
    text::TextPlugin,
    time::TimePlugin,
    ui::{UiPlugin, prelude::UiPickingCamera},
    window::PrimaryWindow,
};
use scorpius::{
    domain::board::GridPos,
    domain::model::BattleEvent,
    mission::mission_one::{ids, mission_one},
    presentation::layout::{
        BATTLE_STAGE_SIZE, CanvasLayout, battle_stage_rect, grid_from_stage_point, iso_center,
        setup_canvas, update_canvas_scale,
    },
    presentation::{
        AttackPreviewCells, BattleEventQueue, BattleRuntime, BattleStage, CanvasRoot,
        EventPlayback, TokenCard, ViewportRoot,
        assets::{AssetLoadStatus, UiAssets},
        interaction::{
            InteractionMode, InteractionState, StatusMessage, on_battlefield_stage_click,
            on_battlefield_stage_move, on_battlefield_stage_out, on_battlefield_token_click,
            on_battlefield_token_move, on_battlefield_token_out,
        },
        map_view::{MapView, spawn_map_controls},
    },
};

fn blank_ui_assets() -> UiAssets {
    UiAssets {
        key_art: Handle::default(),
        briefing_art: Handle::default(),
        vanguard_art: Handle::default(),
        gunner_art: Handle::default(),
        interceptor_art: Handle::default(),
        vanguard_map: Handle::default(),
        gunner_map: Handle::default(),
        interceptor_map: Handle::default(),
        enemy_map: Handle::default(),
        icons: Handle::default(),
        board: Handle::default(),
        terrain: Handle::default(),
        fonts: std::array::from_fn(|_| Handle::default()),
    }
}

#[test]
fn all_screens_share_one_letterbox_transform() {
    for (window_size, expected_scale, expected_offset) in [
        (Vec2::new(1280.0, 720.0), 2.0 / 3.0, Vec2::ZERO),
        (Vec2::new(1600.0, 900.0), 5.0 / 6.0, Vec2::ZERO),
        (Vec2::new(1600.0, 1000.0), 5.0 / 6.0, Vec2::new(0.0, 50.0)),
    ] {
        let fit = CanvasLayout::fit(window_size);
        assert!((fit.scale - expected_scale).abs() < 0.00001);
        assert!((fit.offset - expected_offset).length() < 0.001);
    }
    let fit = CanvasLayout::fit(Vec2::new(1600.0, 1000.0));
    assert!(fit.to_design(Vec2::new(800.0, 10.0)).is_none());
    assert!(
        (fit.to_design(Vec2::new(800.0, 500.0)).unwrap() - Vec2::new(960.0, 540.0)).length()
            < 0.001
    );
}

#[derive(Component)]
struct StageProbe;

fn setup_headless_ui_stage(mut commands: bevy::prelude::Commands) {
    let canvas = scorpius::presentation::layout::spawn_canvas_root(&mut commands);
    commands.spawn((
        StageProbe,
        Pickable::default(),
        bevy::prelude::InheritedVisibility::VISIBLE,
        Node {
            width: Val::Px(BATTLE_STAGE_SIZE.x),
            height: Val::Px(BATTLE_STAGE_SIZE.y),
            position_type: bevy::prelude::PositionType::Absolute,
            left: Val::Px(battle_stage_rect().min.x),
            top: Val::Px(battle_stage_rect().min.y),
            ..Default::default()
        },
        ChildOf(canvas),
    ));
}

fn hovered_stage_cell(app: &App, stage: Entity) -> GridPos {
    let hit = app
        .world()
        .resource::<HoverMap>()
        .get(&PointerId::Mouse)
        .and_then(|hits| hits.get(&stage))
        .expect("stage must be present in the UI picking hit map");
    let normalized = hit
        .position
        .expect("UI picking hit must include normalized node-local position")
        .truncate();
    let node_size = app
        .world()
        .get::<bevy::prelude::ComputedNode>(stage)
        .expect("stage must have computed UI geometry")
        .size;
    let node_local =
        (normalized + Vec2::splat(0.5)) * node_size / app.world().resource::<UiScale>().0;
    grid_from_stage_point(node_local).expect("stage hit must resolve to an authored grid cell")
}

fn sync_headless_camera_to_window(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<&mut bevy::prelude::Camera, With<UiPickingCamera>>,
) {
    let Some(window) = windows.iter().next() else {
        return;
    };
    let size = window.resolution.physical_size();
    for mut camera in &mut cameras {
        camera.computed.target_info = Some(RenderTargetInfo {
            physical_size: size,
            scale_factor: window.scale_factor(),
        });
        camera.viewport = Some(Viewport {
            physical_size: size,
            ..Default::default()
        });
    }
}

fn send_headless_pointer_move(app: &mut App, window: Entity, position: Vec2) {
    send_headless_pointer_action(
        app,
        window,
        position,
        PointerAction::Move {
            delta: Vec2::new(1.0, 0.0),
        },
    );
}

fn send_headless_pointer_action(
    app: &mut App,
    window: Entity,
    position: Vec2,
    action: PointerAction,
) {
    let target = RenderTarget::Window(bevy::window::WindowRef::Entity(window))
        .normalize(Some(window))
        .expect("headless test window should normalize");
    app.world_mut().write_message(PointerInput::new(
        PointerId::Mouse,
        Location { target, position },
        action,
    ));
}

#[test]
fn updated_battle_stage_and_iso_centers_are_pinned() {
    assert_eq!(
        battle_stage_rect(),
        Rect::from_corners(Vec2::new(456.0, 204.0), Vec2::new(1464.0, 968.0))
    );
    assert_eq!(iso_center(GridPos::new(0, 0)), Vec2::new(960.0, 394.0));
    assert_eq!(iso_center(GridPos::new(8, 0)), Vec2::new(1408.0, 618.0));
    assert_eq!(iso_center(GridPos::new(0, 8)), Vec2::new(512.0, 618.0));
    assert_eq!(iso_center(GridPos::new(8, 8)), Vec2::new(960.0, 842.0));
}

#[test]
fn inverse_iso_hit_test_returns_one_cell_or_none() {
    let stage = battle_stage_rect();
    let local = iso_center(GridPos::new(4, 7)) - stage.min;
    assert_eq!(grid_from_stage_point(local), Some(GridPos::new(4, 7)));
    assert_eq!(grid_from_stage_point(Vec2::new(4.0, 4.0)), None);
}

#[test]
fn every_cell_center_and_quadrant_maps_to_its_single_cell() {
    let stage = battle_stage_rect();
    for y in 0..9 {
        for x in 0..9 {
            let cell = GridPos::new(x, y);
            let center = iso_center(cell) - stage.min;
            assert_eq!(grid_from_stage_point(center), Some(cell));
            for dx in [-20.0, 20.0] {
                for dy in [-10.0, 10.0] {
                    assert_eq!(
                        grid_from_stage_point(center + Vec2::new(dx, dy)),
                        Some(cell)
                    );
                }
            }
        }
    }
}

#[test]
fn shared_edges_choose_one_cell_and_outside_points_are_rejected() {
    let stage = battle_stage_rect();
    let top = iso_center(GridPos::new(4, 4));
    let right = iso_center(GridPos::new(5, 4));
    let shared_edge = (top + right) / 2.0 - stage.min;
    assert!(grid_from_stage_point(shared_edge).is_some());
    assert_eq!(grid_from_stage_point(Vec2::new(-1.0, 0.0)), None);
    assert_eq!(grid_from_stage_point(Vec2::new(1009.0, 0.0)), None);
    assert_eq!(grid_from_stage_point(Vec2::new(0.0, 765.0)), None);
    assert_eq!(grid_from_stage_point(Vec2::new(1008.0, 764.0)), None);
}

#[test]
fn projection_rejects_cells_outside_the_authored_nine_by_nine_board() {
    let stage = battle_stage_rect();
    for cell in [GridPos::new(9, 0), GridPos::new(0, 9), GridPos::new(9, 9)] {
        assert_eq!(grid_from_stage_point(iso_center(cell) - stage.min), None);
    }
}

#[test]
fn shared_canvas_is_fixed_and_centered_by_the_viewport_root() {
    let mut app = App::new();
    app.add_systems(Startup, setup_canvas);
    app.update();

    let viewport = app
        .world_mut()
        .query_filtered::<Entity, bevy::prelude::With<ViewportRoot>>()
        .single(app.world())
        .expect("one viewport root");
    let canvas = app
        .world_mut()
        .query_filtered::<Entity, bevy::prelude::With<CanvasRoot>>()
        .single(app.world())
        .expect("one canvas root");
    assert_eq!(
        app.world().get::<ChildOf>(canvas).unwrap().parent(),
        viewport
    );

    let mut nodes = app.world_mut().query::<(bevy::prelude::Entity, &Node)>();
    let viewport_node = nodes
        .iter(app.world())
        .find(|(entity, _)| *entity == viewport)
        .map(|(_, node)| node)
        .unwrap();
    assert_eq!(viewport_node.width, Val::Percent(100.0));
    assert_eq!(viewport_node.height, Val::Percent(100.0));
    let canvas_node = nodes
        .iter(app.world())
        .find(|(entity, _)| *entity == canvas)
        .map(|(_, node)| node)
        .unwrap();
    assert_eq!(canvas_node.width, Val::Px(1920.0));
    assert_eq!(canvas_node.height, Val::Px(1080.0));
}

fn spawn_initial_screen_canvas(mut commands: bevy::prelude::Commands) {
    scorpius::presentation::layout::spawn_canvas_root(&mut commands);
}

#[test]
fn startup_canvas_setup_reuses_a_canvas_created_by_initial_state_entry() {
    let mut app = App::new();
    app.add_systems(Startup, (spawn_initial_screen_canvas, setup_canvas).chain());
    app.update();

    let canvas_count = app
        .world_mut()
        .query_filtered::<Entity, With<CanvasRoot>>()
        .iter(app.world())
        .count();
    assert_eq!(canvas_count, 1);
}

#[test]
fn resize_recomputes_scale_before_picking_and_preserves_stage_cell_hits() {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ImagePlugin::default(),
        TextureAtlasPlugin,
        InputPlugin,
        TimePlugin,
        TransformPlugin,
        TextPlugin,
        UiPlugin,
        PickingPlugin,
        InteractionPlugin,
    ))
    .init_resource::<UiScale>()
    .insert_resource(UiPickingSettings {
        require_markers: true,
    })
    .add_systems(
        bevy::app::PreUpdate,
        sync_headless_camera_to_window
            .before(update_canvas_scale)
            .before(PickingSystems::Backend),
    )
    .add_systems(
        bevy::app::PreUpdate,
        update_canvas_scale.before(PickingSystems::Backend),
    )
    .add_systems(Startup, setup_headless_ui_stage);
    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: (1920, 1080).into(),
                ..Default::default()
            },
            PrimaryWindow,
        ))
        .id();
    let camera = app.world_mut().spawn((Camera2d, UiPickingCamera)).id();
    let pointer = app.world_mut().spawn(PointerId::Mouse).id();

    // The first update lays out the fixed canvas and stage on the synthetic camera.
    app.update();
    let initial_fit = CanvasLayout::fit(Vec2::new(1920.0, 1080.0));
    assert_eq!(app.world().resource::<UiScale>().0, 1.0);
    let stage = app
        .world_mut()
        .query_filtered::<Entity, With<StageProbe>>()
        .single(app.world())
        .expect("one stage probe");
    let canvas = app.world().get::<ChildOf>(stage).unwrap().parent();
    assert!(app.world().get::<CanvasRoot>(canvas).is_some());
    assert!(
        app.world()
            .get::<bevy::prelude::ComputedNode>(stage)
            .unwrap()
            .size
            .x
            > 0.0
    );
    assert!(
        app.world()
            .get::<bevy::prelude::ComputedNode>(stage)
            .unwrap()
            .size
            .y
            > 0.0
    );
    assert!(app.world().get_entity(camera).is_ok());
    assert!(app.world().get_entity(pointer).is_ok());

    let expected_cell = GridPos::new(4, 7);
    let stage_design_point = iso_center(expected_cell);
    let initial_window_point = initial_fit.offset + stage_design_point * initial_fit.scale;
    send_headless_pointer_move(&mut app, window, initial_window_point);
    app.update();
    assert_eq!(hovered_stage_cell(&app, stage), expected_cell);

    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .resolution
        .set(1600.0, 1000.0);
    let resized_fit = CanvasLayout::fit(Vec2::new(1600.0, 1000.0));
    let resized_window_point = resized_fit.offset + stage_design_point * resized_fit.scale;
    send_headless_pointer_move(&mut app, window, resized_window_point);
    app.update();
    assert_eq!(app.world().resource::<UiScale>().0, resized_fit.scale);
    // Layout refreshes in PostUpdate, so the next frame's backend consumes the resized geometry.
    send_headless_pointer_move(&mut app, window, resized_window_point);
    app.update();
    assert_eq!(hovered_stage_cell(&app, stage), expected_cell);
}

#[derive(Debug, Resource, Default)]
struct StagePointerCounts {
    clicks: usize,
    moves: usize,
}

fn count_stage_click(_event: On<Pointer<Click>>, mut counts: ResMut<StagePointerCounts>) {
    counts.clicks += 1;
}

fn count_stage_move(_event: On<Pointer<Move>>, mut counts: ResMut<StagePointerCounts>) {
    counts.moves += 1;
}

fn setup_production_picker_scene(mut commands: Commands, battle: Res<BattleRuntime>) {
    let canvas = scorpius::presentation::layout::spawn_canvas_root(&mut commands);
    let stage = commands
        .spawn((
            BattleStage,
            Pickable::default(),
            InheritedVisibility::VISIBLE,
            Node {
                width: Val::Px(BATTLE_STAGE_SIZE.x),
                height: Val::Px(BATTLE_STAGE_SIZE.y),
                position_type: bevy::prelude::PositionType::Absolute,
                left: Val::Px(battle_stage_rect().min.x),
                top: Val::Px(battle_stage_rect().min.y),
                ..Default::default()
            },
            ChildOf(canvas),
        ))
        .observe(on_battlefield_stage_click)
        .observe(on_battlefield_stage_move)
        .observe(on_battlefield_stage_out)
        .observe(count_stage_click)
        .observe(count_stage_move)
        .id();

    for unit_id in [ids::STRIKER, ids::VANGUARD] {
        let token_cell = battle.0.unit(unit_id).unwrap().position;
        let root = scorpius::presentation::layout::unit_root_top_left(token_cell);
        commands
            .spawn((
                TokenCard(unit_id),
                Pickable::default(),
                Visibility::Visible,
                InheritedVisibility::VISIBLE,
                Node {
                    width: Val::Px(scorpius::presentation::layout::MAP_UNIT_WIDTH),
                    height: Val::Px(scorpius::presentation::layout::MAP_UNIT_HEIGHT),
                    position_type: bevy::prelude::PositionType::Absolute,
                    left: Val::Px(root.x),
                    top: Val::Px(root.y),
                    ..Default::default()
                },
                ChildOf(stage),
            ))
            .observe(on_battlefield_token_click)
            .observe(on_battlefield_token_move)
            .observe(on_battlefield_token_out);
    }

    let blocker_cell = GridPos::new(3, 5);
    let blocker_center = iso_center(blocker_cell) - battle_stage_rect().min;
    commands.spawn((
        Node {
            width: Val::Px(112.0),
            height: Val::Px(82.0),
            position_type: bevy::prelude::PositionType::Absolute,
            left: Val::Px(blocker_center.x - 56.0),
            top: Val::Px(blocker_center.y - 54.0),
            ..Default::default()
        },
        Pickable::IGNORE,
        ChildOf(stage),
    ));
    // Production always installs a MapView; the identity view keeps this
    // fixture's raw stage-local spawns resolving through the same projection.
    commands.insert_resource(MapView::new(battle.0.board()));
}

fn setup_production_map_controls_scene(
    mut commands: Commands,
    battle: Res<BattleRuntime>,
    assets: Res<UiAssets>,
    mut images: ResMut<Assets<Image>>,
) {
    let canvas = scorpius::presentation::layout::spawn_canvas_root(&mut commands);
    let stage = commands
        .spawn((
            BattleStage,
            Pickable::default(),
            InheritedVisibility::VISIBLE,
            Node {
                width: Val::Px(BATTLE_STAGE_SIZE.x),
                height: Val::Px(BATTLE_STAGE_SIZE.y),
                position_type: bevy::prelude::PositionType::Absolute,
                left: Val::Px(battle_stage_rect().min.x),
                top: Val::Px(battle_stage_rect().min.y),
                ..Default::default()
            },
            ChildOf(canvas),
        ))
        .observe(on_battlefield_stage_click)
        .observe(on_battlefield_stage_move)
        .observe(on_battlefield_stage_out)
        .observe(count_stage_click)
        .observe(count_stage_move)
        .id();

    // Mirror populate_mission_root: regional boards install the map chrome and
    // a zoomed-out MapView resource.
    let mut view = MapView::new(battle.0.board());
    if view.is_regional() {
        view.zoom = 0.7;
        view.focus(GridPos::new(6, 6));
        spawn_map_controls(
            &mut commands,
            stage,
            &assets,
            battle.0.board(),
            &battle,
            &mut images,
        );
    }
    commands.insert_resource(view);
}

fn production_app_base() -> App {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin::default(),
        ImagePlugin::default(),
        TextureAtlasPlugin,
        InputPlugin,
        TimePlugin,
        TransformPlugin,
        TextPlugin,
        UiPlugin,
        PickingPlugin,
        InteractionPlugin,
    ))
    .insert_resource(BattleRuntime(battle))
    .insert_resource(blank_ui_assets())
    .init_resource::<Assets<Image>>()
    .init_resource::<InteractionState>()
    .init_resource::<StatusMessage>()
    .init_resource::<BattleEventQueue>()
    .init_resource::<EventPlayback>()
    .init_resource::<AttackPreviewCells>()
    .insert_resource(AssetLoadStatus::Ready)
    .insert_resource(StagePointerCounts::default())
    .init_resource::<UiScale>()
    .insert_resource(UiPickingSettings {
        require_markers: true,
    })
    .add_systems(
        bevy::app::PreUpdate,
        sync_headless_camera_to_window
            .before(update_canvas_scale)
            .before(PickingSystems::Backend),
    )
    .add_systems(
        bevy::app::PreUpdate,
        update_canvas_scale.before(PickingSystems::Backend),
    );
    app
}

fn production_windowed_app(mut app: App) -> (App, Entity) {
    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: (1920, 1080).into(),
                ..Default::default()
            },
            PrimaryWindow,
        ))
        .id();
    app.world_mut().spawn((Camera2d, UiPickingCamera));
    app.world_mut().spawn(PointerId::Mouse);
    app.update();
    (app, window)
}

fn production_picker_app() -> (App, Entity) {
    let mut app = production_app_base();
    app.add_systems(Startup, setup_production_picker_scene);
    production_windowed_app(app)
}

fn production_map_controls_app() -> (App, Entity) {
    let mut app = production_app_base();
    app.add_systems(Startup, setup_production_map_controls_scene);
    production_windowed_app(app)
}

#[test]
fn production_picker_resolves_panned_zoomed_regional_cells_and_ignores_middle_click() {
    let (mut app, window) = production_picker_app();
    // Keep this test's original token fixtures away from the center hit.
    let mut view = MapView::new(app.world().resource::<BattleRuntime>().0.board());
    view.focus(GridPos::new(120, 120));
    view.zoom = 0.5;
    app.insert_resource(view);
    let point = battle_stage_rect().min + BATTLE_STAGE_SIZE * 0.5;
    send_headless_pointer_move(&mut app, window, point);
    app.update();
    assert_eq!(
        app.world().resource::<InteractionState>().hovered_cell,
        Some(GridPos::new(120, 120))
    );
    app.world_mut()
        .resource_mut::<InteractionState>()
        .hovered_cell = None;
    for action in [
        PointerAction::Press(PointerButton::Middle),
        PointerAction::Release(PointerButton::Middle),
    ] {
        send_headless_pointer_action(&mut app, window, point, action);
        app.update();
    }
    assert_eq!(
        app.world().resource::<InteractionState>().hovered_cell,
        None
    );
    for action in [
        PointerAction::Press(PointerButton::Primary),
        PointerAction::Release(PointerButton::Primary),
    ] {
        send_headless_pointer_action(&mut app, window, point, action);
        app.update();
    }
    assert_eq!(
        app.world().resource::<InteractionState>().hovered_cell,
        Some(GridPos::new(120, 120))
    );
}

#[test]
fn production_stage_observers_route_blockers_tokens_and_targets_once() {
    let (mut app, window) = production_picker_app();
    let fit = CanvasLayout::fit(Vec2::new(1920.0, 1080.0));

    let blocker = GridPos::new(3, 5);
    // Probe the blocker diamond's right tip: the tall Striker sprite at (4,6)
    // and the Vanguard sprite at (4,7) cover the rest of the tile face.
    let blocker_point = fit.offset + (iso_center(blocker) + Vec2::new(52.0, 0.0)) * fit.scale;
    send_headless_pointer_move(&mut app, window, blocker_point);
    app.update();
    assert_eq!(
        app.world().resource::<InteractionState>().hovered_cell,
        Some(blocker),
        "an ignored blocker must leave the stage as the hover target",
    );
    assert_eq!(app.world().resource::<StagePointerCounts>().clicks, 0);
    assert_eq!(app.world().resource::<StagePointerCounts>().moves, 1);

    let striker_cell = app
        .world()
        .resource::<BattleRuntime>()
        .0
        .unit(ids::STRIKER)
        .unwrap()
        .position;
    let token_point = fit.offset + (iso_center(striker_cell) + Vec2::new(0.0, -10.0)) * fit.scale;
    send_headless_pointer_move(&mut app, window, token_point);
    app.update();
    assert_eq!(
        app.world().resource::<InteractionState>().hovered_cell,
        Some(striker_cell),
        "feet-zone token hover must resolve to the mech's own diamond",
    );
    assert_eq!(
        app.world().resource::<StagePointerCounts>().moves,
        1,
        "token move must stop before the stage observer"
    );

    // Inspect-mode head-zone hover stays on the mech's own cell so the
    // highlight matches the unit a click at the same point would inspect.
    let overhang = fit.offset + (iso_center(striker_cell) + Vec2::new(0.0, -80.0)) * fit.scale;
    send_headless_pointer_move(&mut app, window, overhang);
    app.update();
    assert_eq!(
        app.world().resource::<InteractionState>().hovered_cell,
        Some(striker_cell),
        "inspect-mode token overhang must hover the mech itself",
    );

    send_headless_pointer_action(
        &mut app,
        window,
        token_point,
        PointerAction::Press(PointerButton::Primary),
    );
    app.update();
    send_headless_pointer_action(
        &mut app,
        window,
        token_point,
        PointerAction::Release(PointerButton::Primary),
    );
    app.update();
    assert_eq!(
        app.world().resource::<InteractionState>().inspected_unit,
        Some(ids::STRIKER)
    );
    assert_eq!(app.world().resource::<StagePointerCounts>().clicks, 0);

    app.world_mut()
        .resource_mut::<BattleRuntime>()
        .0
        .begin_activation(ids::VANGUARD)
        .unwrap();
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mode = InteractionMode::Attack(scorpius::mission::squad::ids::REPULSOR_RAM);
    }
    // Targeting-mode head-zone hover resolves the diamond behind the mech.
    let behind = striker_cell
        .x
        .checked_sub(1)
        .zip(striker_cell.y.checked_sub(1))
        .map(|(x, y)| GridPos::new(x, y));
    if let Some(behind_cell) = behind {
        send_headless_pointer_move(&mut app, window, overhang);
        app.update();
        assert_eq!(
            app.world().resource::<InteractionState>().hovered_cell,
            Some(behind_cell),
            "targeting-mode token overhang must hover the underlying diamond",
        );
    }
    send_headless_pointer_move(&mut app, window, token_point);
    app.update();
    send_headless_pointer_action(
        &mut app,
        window,
        token_point,
        PointerAction::Press(PointerButton::Primary),
    );
    app.update();
    send_headless_pointer_action(
        &mut app,
        window,
        token_point,
        PointerAction::Release(PointerButton::Primary),
    );
    app.update();

    assert!(
        app.world()
            .resource::<BattleRuntime>()
            .0
            .unit(ids::VANGUARD)
            .unwrap()
            .activation
            .acted
    );
    let attacks = app
        .world()
        .resource::<BattleEventQueue>()
        .0
        .iter()
        .filter(|event| {
            matches!(
                event,
                BattleEvent::AttackRolled {
                    attacker: ids::VANGUARD,
                    ..
                }
            )
        })
        .count();
    assert_eq!(attacks, 1, "one token click routes one attack");
    assert_eq!(app.world().resource::<StagePointerCounts>().clicks, 0);
}

#[test]
fn production_ready_token_click_starts_activation_without_bubbling_to_stage() {
    let (mut app, window) = production_picker_app();
    let fit = CanvasLayout::fit(Vec2::new(1920.0, 1080.0));
    let vanguard_cell = app
        .world()
        .resource::<BattleRuntime>()
        .0
        .unit(ids::VANGUARD)
        .unwrap()
        .position;
    let token_point = fit.offset + (iso_center(vanguard_cell) + Vec2::new(0.0, -36.0)) * fit.scale;
    send_headless_pointer_move(&mut app, window, token_point);
    app.update();
    send_headless_pointer_action(
        &mut app,
        window,
        token_point,
        PointerAction::Press(PointerButton::Primary),
    );
    app.update();
    send_headless_pointer_action(
        &mut app,
        window,
        token_point,
        PointerAction::Release(PointerButton::Primary),
    );
    app.update();

    assert_eq!(
        app.world().resource::<BattleRuntime>().0.active_unit(),
        Some(ids::VANGUARD)
    );
    assert_eq!(
        app.world().resource::<InteractionState>().inspected_unit,
        Some(ids::VANGUARD)
    );
    assert_eq!(app.world().resource::<StagePointerCounts>().clicks, 0);
}

#[test]
fn production_pointer_out_clears_hover_and_preview_while_playback_locked() {
    let (mut app, window) = production_picker_app();
    let fit = CanvasLayout::fit(Vec2::new(1920.0, 1080.0));
    app.world_mut()
        .resource_mut::<BattleRuntime>()
        .0
        .begin_activation(ids::VANGUARD)
        .unwrap();

    let blocker = GridPos::new(3, 5);
    // Right diamond tip, clear of the Striker/Vanguard sprites overlapping
    // this tile.
    let blocker_point = fit.offset + (iso_center(blocker) + Vec2::new(52.0, 0.0)) * fit.scale;
    send_headless_pointer_move(&mut app, window, blocker_point);
    app.update();
    assert_eq!(
        app.world().resource::<InteractionState>().hovered_cell,
        Some(blocker)
    );

    let target = app
        .world()
        .resource::<BattleRuntime>()
        .0
        .unit(ids::STRIKER)
        .unwrap()
        .position;
    let preview = app
        .world()
        .resource::<BattleRuntime>()
        .0
        .preview_attack(
            ids::VANGUARD,
            scorpius::mission::squad::ids::REPULSOR_RAM,
            target,
        )
        .unwrap();
    {
        let mut interaction = app.world_mut().resource_mut::<InteractionState>();
        interaction.mode = InteractionMode::Attack(scorpius::mission::squad::ids::REPULSOR_RAM);
        interaction.hovered_cell = Some(target);
        interaction.preview = Some(preview);
    }
    app.world_mut()
        .resource_mut::<AttackPreviewCells>()
        .0
        .insert(target);
    app.world_mut().resource_mut::<EventPlayback>().input_locked = true;

    let outside = fit.offset + Vec2::new(100.0, 100.0) * fit.scale;
    send_headless_pointer_move(&mut app, window, outside);
    app.update();
    assert_eq!(
        app.world().resource::<InteractionState>().hovered_cell,
        None
    );
    assert!(app.world().resource::<InteractionState>().preview.is_none());
    assert!(app.world().resource::<AttackPreviewCells>().0.is_empty());

    app.world_mut().resource_mut::<EventPlayback>().input_locked = false;
    let token_point = fit.offset + (iso_center(target) + Vec2::new(0.0, -10.0)) * fit.scale;
    send_headless_pointer_move(&mut app, window, token_point);
    app.update();
    assert_eq!(
        app.world().resource::<InteractionState>().hovered_cell,
        Some(target)
    );
    assert!(app.world().resource::<InteractionState>().preview.is_some());
    app.world_mut().resource_mut::<EventPlayback>().input_locked = true;

    send_headless_pointer_move(&mut app, window, outside);
    app.update();
    assert_eq!(
        app.world().resource::<InteractionState>().hovered_cell,
        None
    );
    assert!(app.world().resource::<InteractionState>().preview.is_none());
    assert!(app.world().resource::<AttackPreviewCells>().0.is_empty());
}

#[test]
fn production_stage_move_to_blank_inside_rectangle_clears_preview_without_changing_targeting() {
    let (mut app, window) = production_picker_app();
    let fit = CanvasLayout::fit(Vec2::new(1920.0, 1080.0));
    app.world_mut()
        .resource_mut::<BattleRuntime>()
        .0
        .begin_activation(ids::GUNNER)
        .unwrap();
    app.world_mut().resource_mut::<InteractionState>().mode =
        InteractionMode::Attack(scorpius::mission::squad::ids::RAIL_RIFLE);

    let target = GridPos::new(6, 6);
    let target_point = fit.offset + iso_center(target) * fit.scale;
    send_headless_pointer_move(&mut app, window, target_point);
    app.update();
    assert_eq!(
        app.world().resource::<InteractionState>().hovered_cell,
        Some(target)
    );
    assert!(app.world().resource::<InteractionState>().preview.is_some());
    assert!(!app.world().resource::<AttackPreviewCells>().0.is_empty());
    assert_eq!(
        app.world().resource::<BattleRuntime>().0.active_unit(),
        Some(ids::GUNNER)
    );

    let blank = fit.offset + (battle_stage_rect().min + Vec2::new(8.0, 8.0)) * fit.scale;
    assert_eq!(grid_from_stage_point(Vec2::new(8.0, 8.0)), None);
    send_headless_pointer_move(&mut app, window, blank);
    app.update();
    assert_eq!(
        app.world().resource::<InteractionState>().hovered_cell,
        None
    );
    assert!(app.world().resource::<InteractionState>().preview.is_none());
    assert!(app.world().resource::<AttackPreviewCells>().0.is_empty());
    assert_eq!(
        app.world().resource::<InteractionState>().mode,
        InteractionMode::Attack(scorpius::mission::squad::ids::RAIL_RIFLE)
    );
    assert_eq!(
        app.world().resource::<BattleRuntime>().0.active_unit(),
        Some(ids::GUNNER)
    );
}

#[test]
fn production_map_chrome_never_leaks_moves_or_clicks_into_cell_routing() {
    let (mut app, window) = production_map_controls_app();
    let fit = CanvasLayout::fit(Vec2::new(1920.0, 1080.0));
    let to_window = |design: Vec2| fit.offset + design * fit.scale;

    // The minimap covers the stage's top-right corner.
    let minimap_center =
        battle_stage_rect().min + Vec2::new(BATTLE_STAGE_SIZE.x - 16.0 - 88.0, 16.0 + 88.0);
    send_headless_pointer_move(&mut app, window, to_window(minimap_center));
    app.update();
    assert_eq!(
        app.world().resource::<InteractionState>().hovered_cell,
        None,
        "a bubbled minimap-local move must not paint a stage cell",
    );

    // Clicking the minimap navigates without ever reaching the stage.
    let before = *app.world().resource::<MapView>();
    for action in [
        PointerAction::Press(PointerButton::Primary),
        PointerAction::Release(PointerButton::Primary),
    ] {
        send_headless_pointer_action(&mut app, window, to_window(minimap_center), action);
        app.update();
    }
    assert_ne!(
        *app.world().resource::<MapView>(),
        before,
        "a minimap click recenters the regional view",
    );
    assert_eq!(app.world().resource::<StagePointerCounts>().clicks, 0);
    assert_eq!(
        app.world().resource::<InteractionState>().hovered_cell,
        None
    );

    // Zoom buttons are pickable chrome: pointer moves and clicks on them must
    // never fall through to the stage.
    let button_center = {
        let mut buttons = app
            .world_mut()
            .query_filtered::<(&Text, &UiGlobalTransform), With<Button>>();
        buttons
            .iter(app.world())
            .find(|(text, _)| text.0 == "+")
            .map(|(_, transform)| transform.affine().transform_point2(Vec2::ZERO))
            .expect("zoom-in button exists")
    };
    send_headless_pointer_move(&mut app, window, button_center);
    app.update();
    assert_eq!(
        app.world().resource::<InteractionState>().hovered_cell,
        None,
        "a bubbled button-local move must not paint a stage cell",
    );
    assert_eq!(app.world().resource::<MapView>().zoom, 0.7);
    for action in [
        PointerAction::Press(PointerButton::Primary),
        PointerAction::Release(PointerButton::Primary),
    ] {
        send_headless_pointer_action(&mut app, window, button_center, action);
        app.update();
    }
    assert!(
        (app.world().resource::<MapView>().zoom - 0.7 * 1.25).abs() < 0.001,
        "the zoom button applies its map action",
    );
    assert_eq!(
        app.world().resource::<StagePointerCounts>().clicks,
        0,
        "button clicks must not fall through to the stage",
    );

    // The help panel and the terrain readout are pickable chrome too: clicks
    // on them must never route to the diamond hidden underneath.
    let chrome_point = |design: Vec2| fit.offset + design * fit.scale;
    for (label, point) in [
        (
            "help panel",
            battle_stage_rect().min + Vec2::new(BATTLE_STAGE_SIZE.x - 16.0 - 70.0, 246.0 + 22.0),
        ),
        (
            "terrain readout",
            battle_stage_rect().min + Vec2::new(12.0 + 60.0, BATTLE_STAGE_SIZE.y - 12.0 - 16.0),
        ),
    ] {
        for action in [
            PointerAction::Press(PointerButton::Primary),
            PointerAction::Release(PointerButton::Primary),
        ] {
            send_headless_pointer_action(&mut app, window, chrome_point(point), action);
            app.update();
        }
        assert_eq!(
            app.world().resource::<StagePointerCounts>().clicks,
            0,
            "{label} clicks must not fall through to the stage",
        );
        assert_eq!(
            app.world().resource::<InteractionState>().hovered_cell,
            None,
            "{label} clicks must not route a stage cell",
        );
    }
    assert_eq!(
        app.world().resource::<InteractionState>().hovered_cell,
        None
    );
}
