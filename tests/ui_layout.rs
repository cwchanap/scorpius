use bevy::{
    app::{Startup, TaskPoolPlugin},
    asset::AssetPlugin,
    camera::{Camera2d, RenderTarget, RenderTargetInfo, Viewport},
    image::{ImagePlugin, TextureAtlasPlugin},
    input::InputPlugin,
    picking::{
        PickingSystems,
        events::{Click, Move, Pointer},
        hover::HoverMap,
        pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput},
        prelude::{InteractionPlugin, Pickable, PickingPlugin},
    },
    prelude::{
        App, ChildOf, Commands, Component, Entity, InheritedVisibility, IntoScheduleConfigs, Node,
        On, Query, Rect, Res, ResMut, Resource, TransformPlugin, UiPickingSettings, UiScale, Val,
        Vec2, Visibility, Window, With,
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
        assets::AssetLoadStatus,
        interaction::{
            InteractionMode, InteractionState, StatusMessage, on_battlefield_stage_click,
            on_battlefield_stage_move, on_battlefield_stage_out, on_battlefield_token_click,
            on_battlefield_token_move, on_battlefield_token_out,
        },
    },
};

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

    let token_cell = battle.0.unit(ids::STRIKER).unwrap().position;
    let token_center = iso_center(token_cell) - battle_stage_rect().min;
    commands
        .spawn((
            TokenCard(ids::STRIKER),
            Pickable::default(),
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
            Node {
                width: Val::Px(76.0),
                height: Val::Px(64.0),
                position_type: bevy::prelude::PositionType::Absolute,
                left: Val::Px(token_center.x - 38.0),
                top: Val::Px(token_center.y - 68.0),
                ..Default::default()
            },
            ChildOf(stage),
        ))
        .observe(on_battlefield_token_click)
        .observe(on_battlefield_token_move)
        .observe(on_battlefield_token_out);

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
}

fn production_picker_app() -> (App, Entity) {
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
    )
    .add_systems(Startup, setup_production_picker_scene);
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

#[test]
fn production_stage_observers_route_blockers_tokens_and_targets_once() {
    let (mut app, window) = production_picker_app();
    let fit = CanvasLayout::fit(Vec2::new(1920.0, 1080.0));

    let blocker = GridPos::new(3, 5);
    let blocker_point = fit.offset + (iso_center(blocker) + Vec2::new(0.0, -13.0)) * fit.scale;
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
    let token_point = fit.offset + (iso_center(striker_cell) + Vec2::new(0.0, -36.0)) * fit.scale;
    send_headless_pointer_move(&mut app, window, token_point);
    app.update();
    assert_eq!(
        app.world().resource::<InteractionState>().hovered_cell,
        Some(striker_cell),
        "token-local hover must resolve through the domain position",
    );
    assert_eq!(
        app.world().resource::<StagePointerCounts>().moves,
        1,
        "token move must stop before the stage observer"
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
fn production_pointer_out_clears_hover_and_preview_while_playback_locked() {
    let (mut app, window) = production_picker_app();
    let fit = CanvasLayout::fit(Vec2::new(1920.0, 1080.0));
    app.world_mut()
        .resource_mut::<BattleRuntime>()
        .0
        .begin_activation(ids::VANGUARD)
        .unwrap();

    let blocker = GridPos::new(3, 5);
    let blocker_point = fit.offset + (iso_center(blocker) + Vec2::new(0.0, -13.0)) * fit.scale;
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
    let token_point = fit.offset + (iso_center(target) + Vec2::new(0.0, -36.0)) * fit.scale;
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
