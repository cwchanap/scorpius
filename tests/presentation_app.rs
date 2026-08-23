use std::collections::BTreeSet;

use bevy::prelude::*;
use scorpius::{
    domain::{battle::BattleState, board::GridPos, model::UnitId},
    mission::mission_one::mission_one,
    presentation::{
        BattleRuntime, TelegraphVisual, UnitVisual, battlefield::mission_grid_cells, grid_to_world,
        interaction::handle_viability_cell_click, sync::apply_unit_transforms,
        sync::reconcile_telegraph_markers,
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
fn mission_board_exposes_all_eighty_one_logical_cells() {
    let cells = mission_grid_cells(9, 9);

    assert_eq!(cells.len(), 81);
    assert_eq!(cells.first(), Some(&GridPos::new(0, 0)));
    assert_eq!(cells.last(), Some(&GridPos::new(8, 8)));
}

#[test]
fn committed_footprints_create_one_marker_per_unique_cell() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    let expected: BTreeSet<_> = battle
        .intents()
        .iter()
        .flat_map(|intent| {
            intent
                .footprint
                .iter()
                .copied()
                .map(move |cell| (intent.attacker, cell))
        })
        .collect();

    let mut app = App::new();
    app.insert_resource(BattleRuntime(battle))
        .add_systems(Update, reconcile_telegraph_markers);
    app.update();

    let actual: BTreeSet<_> = app
        .world_mut()
        .query::<&TelegraphVisual>()
        .iter(app.world())
        .map(|marker| (marker.attacker, marker.cell))
        .collect();
    assert_eq!(actual, expected);
}
