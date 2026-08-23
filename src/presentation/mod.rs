pub mod assets;
pub mod battlefield;
pub mod interaction;
pub mod sync;
pub mod ui;

use std::collections::{BTreeSet, VecDeque};

use bevy::prelude::*;

use crate::domain::{
    battle::BattleState,
    board::GridPos,
    model::{BattleEvent, Reaction, UnitId, WeaponShape},
};

#[derive(Resource)]
pub struct BattleRuntime(pub BattleState);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnitVisual(pub UnitId);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellVisual(pub GridPos);

#[derive(Component)]
pub struct PresentationRoot;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct TelegraphVisual {
    pub attacker: UnitId,
    pub cell: GridPos,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct TelegraphGlyphVisual(pub WeaponShape);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntentTargetVisual {
    pub attacker: UnitId,
    pub target: UnitId,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntentLineVisual {
    pub attacker: UnitId,
    pub origin: GridPos,
    pub center: GridPos,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub enum PropVisual {
    Blocking(GridPos),
    Explosive(GridPos),
    Hazard(GridPos),
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReactionVisual {
    pub unit: UnitId,
    pub reaction: Reaction,
}

#[derive(Resource, Default)]
pub struct BattleEventQueue(pub VecDeque<BattleEvent>);

#[derive(Resource, Default)]
pub struct AttackPreviewCells(pub BTreeSet<GridPos>);

#[derive(Resource, Default)]
pub struct SelectedCell(pub Option<GridPos>);

pub fn grid_to_world(pos: GridPos) -> Vec3 {
    const HALF: f32 = 4.0;
    Vec3::new(pos.x as f32 - HALF, 0.2, pos.y as f32 - HALF)
}
