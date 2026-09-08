use bevy::{
    picking::PickingSystems,
    prelude::{
        App, ChildOf, Entity, IntoScheduleConfigs, Node, Rect, Startup, UiPickingSettings, UiScale,
        Val, Vec2, Window,
    },
    window::PrimaryWindow,
};
use scorpius::{
    domain::board::GridPos,
    presentation::layout::{
        CanvasLayout, battle_stage_rect, grid_from_stage_point, iso_center, setup_canvas,
        update_canvas_scale,
    },
    presentation::{CanvasRoot, ViewportRoot},
};

#[test]
fn all_screens_share_one_letterbox_transform() {
    let fit = CanvasLayout::fit(Vec2::new(1600.0, 1000.0));
    assert!((fit.scale - 5.0 / 6.0).abs() < 0.00001);
    assert!((fit.offset - Vec2::new(0.0, 50.0)).length() < 0.001);
    assert!(fit.to_design(Vec2::new(800.0, 10.0)).is_none());
    assert!(
        (fit.to_design(Vec2::new(800.0, 500.0)).unwrap() - Vec2::new(960.0, 540.0)).length()
            < 0.001
    );
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
    app.init_resource::<UiScale>()
        .insert_resource(UiPickingSettings {
            require_markers: true,
        })
        .add_systems(
            bevy::app::PreUpdate,
            update_canvas_scale.before(PickingSystems::Backend),
        );
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

    app.update();
    let design_cell = GridPos::new(4, 7);
    let initial_fit = CanvasLayout::fit(Vec2::new(1920.0, 1080.0));
    let initial_window_point = initial_fit.offset
        + (iso_center(design_cell) - battle_stage_rect().min) * initial_fit.scale;
    assert_eq!(app.world().resource::<UiScale>().0, 1.0);
    assert_eq!(
        grid_from_stage_point((initial_window_point - initial_fit.offset) / initial_fit.scale),
        Some(design_cell)
    );

    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .resolution
        .set(1600.0, 1000.0);
    app.update();
    let resized_fit = CanvasLayout::fit(Vec2::new(1600.0, 1000.0));
    let resized_window_point = resized_fit.offset
        + (iso_center(design_cell) - battle_stage_rect().min) * resized_fit.scale;
    assert_eq!(app.world().resource::<UiScale>().0, resized_fit.scale);
    assert_eq!(
        grid_from_stage_point((resized_window_point - resized_fit.offset) / resized_fit.scale),
        Some(design_cell)
    );
}
