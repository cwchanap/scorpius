use bevy::prelude::*;
use scorpius::{
    domain::{battle::BattleState, board::GridPos, model::UnitId},
    presentation::{
        BattleRuntime, UnitVisual, battlefield::viability_grid_cells, grid_to_world,
        interaction::handle_viability_cell_click, sync::apply_unit_transforms,
    },
};

#[test]
fn canonical_move_drives_visual_transform_without_renderer() {
    let mut app = App::new();
    app.insert_resource(BattleRuntime(BattleState::viability_fixture()))
        .add_systems(Update, apply_unit_transforms);
    app.world_mut().spawn((
        UnitVisual(UnitId(1)),
        Transform::from_translation(grid_to_world(GridPos::new(1, 1))),
    ));

    app.world_mut()
        .resource_mut::<BattleRuntime>()
        .0
        .move_unit(UnitId(1), GridPos::new(1, 2))
        .unwrap();
    app.update();

    let mut transforms = app.world_mut().query::<&Transform>();
    let transform = transforms.single(app.world()).unwrap();
    assert_eq!(transform.translation, grid_to_world(GridPos::new(1, 2)));
}

#[test]
fn adjacent_cell_click_moves_canonical_unit() {
    let mut battle = BattleState::viability_fixture();
    let mut selected = None;

    handle_viability_cell_click(&mut battle, &mut selected, GridPos::new(1, 1)).unwrap();
    assert_eq!(selected, Some(GridPos::new(1, 1)));

    handle_viability_cell_click(&mut battle, &mut selected, GridPos::new(2, 1)).unwrap();
    assert_eq!(selected, Some(GridPos::new(2, 1)));
    assert_eq!(battle.unit(UnitId(1)).unwrap().position, GridPos::new(2, 1));
}

#[test]
fn viability_board_exposes_nine_logical_cells() {
    assert_eq!(
        viability_grid_cells(),
        vec![
            GridPos::new(0, 0),
            GridPos::new(1, 0),
            GridPos::new(2, 0),
            GridPos::new(0, 1),
            GridPos::new(1, 1),
            GridPos::new(2, 1),
            GridPos::new(0, 2),
            GridPos::new(1, 2),
            GridPos::new(2, 2),
        ]
    );
}
