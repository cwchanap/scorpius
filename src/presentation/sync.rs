use bevy::prelude::*;

use super::{BattleRuntime, UnitVisual, grid_to_world};

pub fn apply_unit_transforms(
    battle: Res<BattleRuntime>,
    mut visuals: Query<(&UnitVisual, &mut Transform)>,
) {
    for (visual, mut transform) in &mut visuals {
        if let Some(unit) = battle.0.unit(visual.0) {
            transform.translation = grid_to_world(unit.position);
        }
    }
}
