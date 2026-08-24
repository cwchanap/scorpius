use std::collections::{BTreeMap, BTreeSet};
use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;

use crate::domain::model::{Faction, WeaponShape};

use super::{
    AttackPreviewCells, BattleRuntime, CellVisual, EventPlayback, IntentLineVisual,
    IntentTargetVisual, PresentationRoot, PropVisual, ReactionVisual, SelectedCell,
    TelegraphGlyphVisual, TelegraphVisual, UnitVisual, battlefield::BattlefieldVisualAssets,
    grid_to_world,
};

pub fn apply_unit_transforms(
    battle: Res<BattleRuntime>,
    playback: Option<Res<EventPlayback>>,
    mut visuals: Query<(&UnitVisual, &mut Transform, Option<&mut Visibility>)>,
) {
    if playback.is_some_and(|playback| playback.input_locked) {
        return;
    }
    for (visual, mut transform, visibility) in &mut visuals {
        if let Some(unit) = battle.0.unit(visual.0) {
            transform.translation = grid_to_world(unit.position);
            transform.scale = Vec3::splat(0.72);
            if let Some(mut visibility) = visibility {
                *visibility = if unit.is_knocked_out() {
                    Visibility::Hidden
                } else {
                    Visibility::Visible
                };
            }
        }
    }
}

pub fn apply_prop_visibility(
    battle: Res<BattleRuntime>,
    playback: Option<Res<EventPlayback>>,
    mut props: Query<(&PropVisual, &mut Visibility)>,
) {
    if playback.is_some_and(|playback| playback.input_locked) {
        return;
    }
    for (prop, mut visibility) in &mut props {
        if let PropVisual::Explosive(position) = prop {
            let live = battle.0.board().has_live_explosive(*position);
            *visibility = if live {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
}

pub fn reconcile_telegraph_markers(
    mut commands: Commands,
    battle: Res<BattleRuntime>,
    playback: Option<Res<EventPlayback>>,
    roots: Query<Entity, With<PresentationRoot>>,
    existing: Query<(Entity, &TelegraphVisual)>,
) {
    if playback.is_some_and(|playback| playback.input_locked) {
        return;
    }
    let expected: BTreeMap<_, _> = battle
        .0
        .intents()
        .iter()
        .filter_map(|intent| {
            battle
                .0
                .weapon(intent.profile.weapon)
                .map(|weapon| (intent, weapon.shape))
        })
        .flat_map(|(intent, shape)| {
            intent
                .footprint
                .iter()
                .copied()
                .map(move |cell| ((intent.attacker, cell), shape))
        })
        .collect();
    let mut present = BTreeSet::new();

    for (entity, marker) in &existing {
        let key = (marker.attacker, marker.cell);
        if !expected.contains_key(&key) || !present.insert(key) {
            commands.entity(entity).despawn();
        }
    }

    let root = roots.iter().next();
    for ((attacker, cell), shape) in expected {
        if present.contains(&(attacker, cell)) {
            continue;
        }
        let mut marker = commands.spawn((
            TelegraphVisual { attacker, cell },
            TelegraphGlyphVisual(shape),
            Transform::from_translation(grid_to_world(cell) + Vec3::Y * 0.13),
            Visibility::Visible,
        ));
        if let Some(root) = root {
            marker.insert(ChildOf(root));
        }
    }
}

pub fn reconcile_intent_guides(
    mut commands: Commands,
    battle: Res<BattleRuntime>,
    playback: Option<Res<EventPlayback>>,
    roots: Query<Entity, With<PresentationRoot>>,
    existing_targets: Query<(Entity, &IntentTargetVisual)>,
    existing_lines: Query<(Entity, &IntentLineVisual)>,
) {
    if playback.is_some_and(|playback| playback.input_locked) {
        return;
    }
    let expected_targets: BTreeSet<_> = battle
        .0
        .intents()
        .iter()
        .filter_map(|intent| {
            intent.intended_occupant.and_then(|target| {
                battle
                    .0
                    .unit(target)
                    .filter(|unit| !unit.is_knocked_out())
                    .map(|_| (intent.attacker, target))
            })
        })
        .collect();
    let expected_lines: BTreeMap<_, _> = battle
        .0
        .intents()
        .iter()
        .filter_map(|intent| {
            intent.footprint.first().map(|center| {
                (
                    intent.attacker,
                    IntentLineVisual {
                        attacker: intent.attacker,
                        origin: intent.origin,
                        center: *center,
                    },
                )
            })
        })
        .collect();
    let mut present_targets = BTreeSet::new();
    let mut present_lines = BTreeSet::new();

    for (entity, marker) in &existing_targets {
        let key = (marker.attacker, marker.target);
        if !expected_targets.contains(&key) || !present_targets.insert(key) {
            commands.entity(entity).despawn();
        }
    }
    for (entity, line) in &existing_lines {
        if expected_lines.get(&line.attacker) != Some(line) || !present_lines.insert(line.attacker)
        {
            commands.entity(entity).despawn();
        }
    }

    let root = roots.iter().next();
    for (attacker, target) in expected_targets {
        if present_targets.contains(&(attacker, target)) {
            continue;
        }
        let Some(target_unit) = battle.0.unit(target) else {
            continue;
        };
        let mut marker = commands.spawn((
            IntentTargetVisual { attacker, target },
            Transform::from_translation(grid_to_world(target_unit.position) + Vec3::Y * 0.17),
            Visibility::Visible,
        ));
        if let Some(root) = root {
            marker.insert(ChildOf(root));
        }
    }
    for (attacker, line) in expected_lines {
        if present_lines.contains(&attacker) {
            continue;
        }
        let mut marker = commands.spawn((
            line,
            intent_line_transform(line.origin, line.center),
            Visibility::Visible,
        ));
        if let Some(root) = root {
            marker.insert(ChildOf(root));
        }
    }
}

pub fn reconcile_reaction_markers(
    mut commands: Commands,
    battle: Res<BattleRuntime>,
    playback: Option<Res<EventPlayback>>,
    roots: Query<Entity, With<PresentationRoot>>,
    existing: Query<(Entity, &ReactionVisual)>,
) {
    if playback.is_some_and(|playback| playback.input_locked) {
        return;
    }
    let expected: BTreeMap<_, _> = battle
        .0
        .units()
        .filter(|unit| unit.faction == Faction::Player && !unit.is_knocked_out())
        .filter_map(|unit| unit.reaction.map(|reaction| (unit.id, reaction)))
        .collect();
    let mut present = BTreeSet::new();

    for (entity, marker) in &existing {
        if expected.get(&marker.unit) != Some(&marker.reaction) || !present.insert(marker.unit) {
            commands.entity(entity).despawn();
        }
    }

    let root = roots.iter().next();
    for (unit, reaction) in expected {
        if present.contains(&unit) {
            continue;
        }
        let Some(unit_state) = battle.0.unit(unit) else {
            continue;
        };
        let mut marker = commands.spawn((
            ReactionVisual { unit, reaction },
            Transform::from_translation(grid_to_world(unit_state.position) + Vec3::Y * 0.62)
                .with_scale(Vec3::splat(0.55)),
            Visibility::Visible,
        ));
        if let Some(root) = root {
            marker.insert(ChildOf(root));
        }
    }
}

pub fn sync_auxiliary_transforms(
    battle: Res<BattleRuntime>,
    playback: Option<Res<EventPlayback>>,
    mut targets: Query<(&IntentTargetVisual, &mut Transform), Without<ReactionVisual>>,
    mut reactions: Query<(&ReactionVisual, &mut Transform), Without<IntentTargetVisual>>,
) {
    if playback.is_some_and(|playback| playback.input_locked) {
        return;
    }
    for (marker, mut transform) in &mut targets {
        if let Some(unit) = battle.0.unit(marker.target) {
            transform.translation = grid_to_world(unit.position) + Vec3::Y * 0.17;
        }
    }
    for (marker, mut transform) in &mut reactions {
        if let Some(unit) = battle.0.unit(marker.unit) {
            transform.translation = grid_to_world(unit.position) + Vec3::Y * 0.62;
        }
    }
}

pub(crate) fn attach_telegraph_rendering(
    mut commands: Commands,
    visual_assets: Option<Res<BattlefieldVisualAssets>>,
    telegraphs: Query<(Entity, &TelegraphGlyphVisual), Added<TelegraphVisual>>,
) {
    let Some(visual_assets) = visual_assets else {
        return;
    };

    for (entity, glyph) in &telegraphs {
        commands.entity(entity).insert((
            Mesh3d(visual_assets.telegraph_mesh.clone()),
            MeshMaterial3d(visual_assets.telegraph.clone()),
            Pickable::IGNORE,
        ));
        match glyph.0 {
            WeaponShape::Single => {
                commands.spawn((
                    Mesh3d(visual_assets.ring_mesh.clone()),
                    MeshMaterial3d(visual_assets.telegraph_edge.clone()),
                    Transform::from_xyz(0.0, 0.055, 0.0).with_scale(Vec3::splat(0.82)),
                    Pickable::IGNORE,
                    ChildOf(entity),
                ));
            }
            WeaponShape::Cross1 => {
                for rotation in [0.0, FRAC_PI_2] {
                    commands.spawn((
                        Mesh3d(visual_assets.glyph_bar_mesh.clone()),
                        MeshMaterial3d(visual_assets.telegraph_edge.clone()),
                        Transform::from_xyz(0.0, 0.055, 0.0)
                            .with_rotation(Quat::from_rotation_y(rotation)),
                        Pickable::IGNORE,
                        ChildOf(entity),
                    ));
                }
            }
        }
    }
}

pub(crate) fn attach_intent_target_rendering(
    mut commands: Commands,
    visual_assets: Option<Res<BattlefieldVisualAssets>>,
    targets: Query<Entity, Added<IntentTargetVisual>>,
) {
    let Some(visual_assets) = visual_assets else {
        return;
    };
    for entity in &targets {
        commands.entity(entity).insert((
            Mesh3d(visual_assets.ring_mesh.clone()),
            MeshMaterial3d(visual_assets.intended_target.clone()),
            Pickable::IGNORE,
        ));
    }
}

pub(crate) fn attach_intent_line_rendering(
    mut commands: Commands,
    visual_assets: Option<Res<BattlefieldVisualAssets>>,
    lines: Query<Entity, Added<IntentLineVisual>>,
) {
    let Some(visual_assets) = visual_assets else {
        return;
    };
    for entity in &lines {
        commands.entity(entity).insert((
            Mesh3d(visual_assets.line_mesh.clone()),
            MeshMaterial3d(visual_assets.intent_line.clone()),
            Pickable::IGNORE,
        ));
    }
}

pub(crate) fn attach_reaction_rendering(
    mut commands: Commands,
    visual_assets: Option<Res<BattlefieldVisualAssets>>,
    reactions: Query<(Entity, &ReactionVisual), Added<ReactionVisual>>,
) {
    let Some(visual_assets) = visual_assets else {
        return;
    };
    for (entity, marker) in &reactions {
        commands.entity(entity).insert((
            Mesh3d(visual_assets.ring_mesh.clone()),
            MeshMaterial3d(visual_assets.reaction_material(marker.reaction)),
            Pickable::IGNORE,
        ));
        let rotations: &[f32] = match marker.reaction {
            crate::domain::model::Reaction::Guard => &[0.0],
            crate::domain::model::Reaction::Evade => &[-0.48, 0.48],
            crate::domain::model::Reaction::Counter => &[0.0, FRAC_PI_2],
        };
        for rotation in rotations {
            commands.spawn((
                Mesh3d(visual_assets.glyph_bar_mesh.clone()),
                MeshMaterial3d(visual_assets.reaction_material(marker.reaction)),
                Transform::from_xyz(0.0, 0.05, 0.0).with_rotation(Quat::from_rotation_y(*rotation)),
                Pickable::IGNORE,
                ChildOf(entity),
            ));
        }
    }
}

pub fn pulse_telegraphs(
    time: Res<Time>,
    visual_assets: Option<Res<BattlefieldVisualAssets>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
    mut telegraphs: Query<&mut Transform, With<TelegraphVisual>>,
) {
    let pulse = (time.elapsed_secs() * 3.2).sin() * 0.045 + 1.0;
    for mut transform in &mut telegraphs {
        transform.scale = Vec3::splat(pulse);
    }

    let (Some(visual_assets), Some(materials)) = (visual_assets, materials.as_mut()) else {
        return;
    };
    if let Some(mut material) = materials.get_mut(&visual_assets.telegraph) {
        material.base_color.set_alpha(0.28 + (pulse - 0.955) * 2.0);
    }
}

pub fn sync_cell_highlights(
    battle: Res<BattleRuntime>,
    selected: Res<SelectedCell>,
    attack_preview: Option<Res<AttackPreviewCells>>,
    visual_assets: Option<Res<BattlefieldVisualAssets>>,
    mut cells: Query<(&CellVisual, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let Some(visual_assets) = visual_assets else {
        return;
    };
    let selected_unit = selected
        .0
        .and_then(|cell| battle.0.occupant_at(cell))
        .and_then(|id| battle.0.unit(id))
        .filter(|unit| unit.faction == Faction::Player)
        .map(|unit| unit.id);
    let reachable = selected_unit
        .and_then(|unit| battle.0.reachable_cells(unit).ok())
        .unwrap_or_default();

    for (cell, mut material) in &mut cells {
        material.0 = if attack_preview
            .as_ref()
            .is_some_and(|preview| preview.0.contains(&cell.0))
        {
            visual_assets.tile_attack_preview.clone()
        } else if selected.0 == Some(cell.0) {
            visual_assets.tile_selected.clone()
        } else if reachable.contains(&cell.0) {
            visual_assets.tile_reachable.clone()
        } else if (cell.0.x + cell.0.y) % 2 == 0 {
            visual_assets.tile_light.clone()
        } else {
            visual_assets.tile_dark.clone()
        };
    }
}

fn intent_line_transform(
    origin: crate::domain::board::GridPos,
    center: crate::domain::board::GridPos,
) -> Transform {
    let start = grid_to_world(origin) + Vec3::Y * 0.18;
    let end = grid_to_world(center) + Vec3::Y * 0.18;
    let delta = end - start;
    let length = Vec2::new(delta.x, delta.z).length().max(0.08);
    Transform::from_translation(start.midpoint(end))
        .with_rotation(Quat::from_rotation_y(-delta.z.atan2(delta.x)))
        .with_scale(Vec3::new(length, 1.0, 1.0))
}
