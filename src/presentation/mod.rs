pub mod assets;
pub mod battlefield;
pub mod interaction;
pub mod sync;
pub mod ui;

use bevy::prelude::*;

use crate::domain::{battle::BattleState, board::GridPos, model::UnitId};

#[derive(Resource)]
pub struct BattleRuntime(pub BattleState);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnitVisual(pub UnitId);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellVisual(pub GridPos);

#[derive(Resource, Default)]
pub struct SelectedCell(pub Option<GridPos>);

pub fn grid_to_world(pos: GridPos) -> Vec3 {
    Vec3::new(pos.x as f32 - 1.0, 0.2, pos.y as f32 - 1.0)
}
