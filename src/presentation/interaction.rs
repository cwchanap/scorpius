use bevy::prelude::*;

use crate::domain::{
    battle::BattleState,
    board::GridPos,
    model::{BattleError, BattleEvent, UnitId},
};

use super::{
    BattleRuntime, CellVisual, SelectedCell,
    assets::{AssetLoadStatus, mission_assets_ready},
};

pub fn handle_viability_cell_click(
    battle: &mut BattleState,
    selected: &mut Option<GridPos>,
    clicked: GridPos,
) -> Result<Vec<BattleEvent>, BattleError> {
    let unit_position = battle
        .unit(UnitId(1))
        .ok_or(BattleError::UnknownUnit(UnitId(1)))?
        .position;

    let events = if *selected == Some(unit_position) && clicked != unit_position {
        battle.move_unit(UnitId(1), clicked)?
    } else {
        Vec::new()
    };
    *selected = Some(clicked);
    Ok(events)
}

pub fn on_battlefield_cell_click(
    click: On<Pointer<Click>>,
    cells: Query<&CellVisual>,
    mut battle: ResMut<BattleRuntime>,
    mut selected: ResMut<SelectedCell>,
    asset_status: Res<AssetLoadStatus>,
) {
    if !mission_assets_ready(asset_status) {
        return;
    }
    let Ok(cell) = cells.get(click.entity) else {
        return;
    };
    if handle_viability_cell_click(&mut battle.0, &mut selected.0, cell.0).is_err() {
        selected.0 = Some(cell.0);
    }
}
