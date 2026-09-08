use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

use crate::domain::model::{Faction, PrimaryObjective, Reaction};

use super::{
    AttackPreviewCells, BattleRuntime, BattleStage, CellVisual, EventPlayback, ExtractionVisual,
    IntentLineVisual, IntentTargetVisual, PresentationRoot, PropVisual, ReactionVisual,
    TelegraphGlyphVisual, TelegraphVisual, TokenAwaiting, TokenCard, TokenHpFill, TokenHpText,
    UnitVisual,
    assets::UiAssets,
    interaction::InteractionState,
    layout::{TOKEN_HEIGHT, TOKEN_WIDTH, battle_stage_rect, iso_center},
    theme,
};

fn stage_point(cell: crate::domain::board::GridPos) -> Vec2 {
    iso_center(cell) - battle_stage_rect().min
}

fn marker_parent(
    stage: &Query<Entity, With<BattleStage>>,
    roots: &Query<Entity, With<PresentationRoot>>,
) -> Option<Entity> {
    stage.iter().next().or_else(|| roots.iter().next())
}

fn marker_node(center: Vec2, width: f32, height: f32, top_offset: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(center.x - width * 0.5),
        top: px(center.y - height * 0.5 + top_offset),
        width: px(width),
        height: px(height),
        ..default()
    }
}

fn insert_board_image(
    commands: &mut Commands,
    entity: Entity,
    ui_assets: Option<&UiAssets>,
    rect: Rect,
    color: Color,
) {
    if let Some(ui_assets) = ui_assets {
        commands
            .entity(entity)
            .insert(theme::board_node(ui_assets.board.clone(), rect, color));
    }
}

fn insert_icon_image(
    commands: &mut Commands,
    entity: Entity,
    ui_assets: Option<&UiAssets>,
    rect: Rect,
    color: Color,
) {
    if let Some(ui_assets) = ui_assets {
        commands
            .entity(entity)
            .insert(theme::icon_node(ui_assets.icons.clone(), rect, color));
    }
}

pub fn apply_unit_transforms(
    battle: Res<BattleRuntime>,
    playback: Option<Res<EventPlayback>>,
    mut visuals: Query<(&UnitVisual, &mut Node, &mut Visibility)>,
) {
    if playback.is_some_and(|playback| playback.input_locked) {
        return;
    }
    for (visual, mut node, mut visibility) in &mut visuals {
        if let Some(unit) = battle.0.unit(visual.0) {
            let center = stage_point(unit.position);
            node.left = px(center.x - TOKEN_WIDTH * 0.5);
            node.top = px(center.y - TOKEN_HEIGHT - 4.0);
            *visibility = if unit.is_knocked_out() {
                Visibility::Hidden
            } else {
                Visibility::Visible
            };
        }
    }
}

pub fn sync_token_cards(
    battle: Res<BattleRuntime>,
    playback: Option<Res<EventPlayback>>,
    mut cards: Query<(&TokenCard, &mut BackgroundColor)>,
    mut hp_fills: Query<(&TokenHpFill, &mut Node)>,
    mut hp_texts: Query<(&TokenHpText, &mut Text)>,
    mut awaiting: Query<(&TokenAwaiting, &mut Visibility)>,
) {
    if playback.is_some_and(|playback| playback.input_locked) {
        return;
    }
    for (card, mut background) in &mut cards {
        if let Some(unit) = battle.0.unit(card.0) {
            let base = if unit.faction == Faction::Player {
                Color::srgba(0.08, 0.20, 0.29, 0.98)
            } else {
                Color::srgba(0.22, 0.08, 0.06, 0.98)
            };
            *background = if unit.activation.finished && unit.faction == Faction::Player {
                BackgroundColor(base.with_alpha(0.52))
            } else {
                BackgroundColor(base)
            };
        }
    }
    for (fill, mut node) in &mut hp_fills {
        if let Some(unit) = battle.0.unit(fill.0) {
            node.width =
                percent(100.0 * f32::from(unit.hp.max(0)) / f32::from(unit.stats.max_hp.max(1)));
        }
    }
    for (text, mut value) in &mut hp_texts {
        if let Some(unit) = battle.0.unit(text.0) {
            value.0 = unit.hp.to_string();
        }
    }
    for (marker, mut visibility) in &mut awaiting {
        *visibility = if battle
            .0
            .unit(marker.0)
            .is_some_and(|unit| unit.faction == Faction::Player && !unit.activation.finished)
        {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
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
    ui_assets: Option<Res<UiAssets>>,
    stages: Query<Entity, With<BattleStage>>,
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
    let parent = marker_parent(&stages, &roots);
    for ((attacker, cell), shape) in expected {
        if present.contains(&(attacker, cell)) {
            continue;
        }
        let entity = commands
            .spawn((
                TelegraphVisual { attacker, cell },
                TelegraphGlyphVisual(shape),
                marker_node(stage_point(cell), 112.0, 56.0, 0.0),
                UiTransform::IDENTITY,
                BackgroundColor(theme::BOARD_TELEGRAPH),
                ZIndex(5),
                Pickable::IGNORE,
            ))
            .id();
        if let Some(parent) = parent {
            commands.entity(entity).insert(ChildOf(parent));
        }
        insert_board_image(
            &mut commands,
            entity,
            ui_assets.as_deref(),
            theme::BOARD_DIAMOND_RECT,
            theme::BOARD_TELEGRAPH,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn reconcile_intent_guides(
    mut commands: Commands,
    battle: Res<BattleRuntime>,
    playback: Option<Res<EventPlayback>>,
    ui_assets: Option<Res<UiAssets>>,
    stages: Query<Entity, With<BattleStage>>,
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
    let parent = marker_parent(&stages, &roots);
    for (attacker, target) in expected_targets {
        if present_targets.contains(&(attacker, target)) {
            continue;
        }
        let Some(target_unit) = battle.0.unit(target) else {
            continue;
        };
        let entity = commands
            .spawn((
                IntentTargetVisual { attacker, target },
                marker_node(stage_point(target_unit.position), 112.0, 56.0, 0.0),
                BackgroundColor(theme::BOARD_ATTACK),
                ZIndex(22),
                Pickable::IGNORE,
            ))
            .id();
        if let Some(parent) = parent {
            commands.entity(entity).insert(ChildOf(parent));
        }
        insert_board_image(
            &mut commands,
            entity,
            ui_assets.as_deref(),
            theme::BOARD_DIAMOND_RECT,
            theme::BOARD_ATTACK,
        );
    }
    for (attacker, line) in expected_lines {
        if present_lines.contains(&attacker) {
            continue;
        }
        let entity = commands
            .spawn((
                line,
                intent_line_node(line.origin, line.center),
                BackgroundColor(theme::ENEMY),
                ZIndex(9),
                Pickable::IGNORE,
            ))
            .id();
        if let Some(parent) = parent {
            commands.entity(entity).insert(ChildOf(parent));
        }
        let _ = entity;
    }
}

/// Keeps one marker at an intercept mission's escape cell and none for the
/// other authored primary objectives.
pub fn reconcile_extraction_marker(
    mut commands: Commands,
    battle: Res<BattleRuntime>,
    playback: Option<Res<EventPlayback>>,
    ui_assets: Option<Res<UiAssets>>,
    stages: Query<Entity, With<BattleStage>>,
    roots: Query<Entity, With<PresentationRoot>>,
    existing: Query<(Entity, &ExtractionVisual)>,
) {
    if playback.is_some_and(|playback| playback.input_locked) {
        return;
    }
    let expected = match battle.0.rules().primary {
        PrimaryObjective::InterceptBeforeEscape { escape, .. } => Some(escape),
        PrimaryObjective::EliminateAllEnemies
        | PrimaryObjective::EliminateTarget { .. }
        | PrimaryObjective::ProtectThroughRound { .. } => None,
    };
    for (entity, marker) in &existing {
        if Some(marker.0) != expected {
            commands.entity(entity).despawn();
        }
    }
    let Some(escape) = expected else {
        return;
    };
    if existing.iter().any(|(_, marker)| marker.0 == escape) {
        return;
    }
    let parent = marker_parent(&stages, &roots);
    let entity = commands
        .spawn((
            ExtractionVisual(escape),
            marker_node(stage_point(escape), 112.0, 56.0, 0.0),
            BackgroundColor(theme::BOARD_EXTRACTION),
            ZIndex(7),
            Pickable::IGNORE,
        ))
        .id();
    if let Some(parent) = parent {
        commands.entity(entity).insert(ChildOf(parent));
    }
    insert_board_image(
        &mut commands,
        entity,
        ui_assets.as_deref(),
        theme::BOARD_DIAMOND_RECT,
        theme::BOARD_EXTRACTION,
    );
}

pub fn reconcile_reaction_markers(
    mut commands: Commands,
    battle: Res<BattleRuntime>,
    playback: Option<Res<EventPlayback>>,
    ui_assets: Option<Res<UiAssets>>,
    stages: Query<Entity, With<BattleStage>>,
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
    let parent = marker_parent(&stages, &roots);
    for (unit, reaction) in expected {
        if present.contains(&unit) {
            continue;
        }
        let Some(unit_state) = battle.0.unit(unit) else {
            continue;
        };
        let entity = commands
            .spawn((
                ReactionVisual { unit, reaction },
                marker_node(stage_point(unit_state.position), 32.0, 32.0, -48.0),
                BackgroundColor(theme::MINT),
                ZIndex(23),
                Pickable::IGNORE,
            ))
            .id();
        if let Some(parent) = parent {
            commands.entity(entity).insert(ChildOf(parent));
        }
        let icon = match reaction {
            Reaction::Guard => theme::ICON_GUARD,
            Reaction::Evade => theme::ICON_EVADE,
            Reaction::Counter => theme::ICON_COUNTER,
        };
        insert_icon_image(
            &mut commands,
            entity,
            ui_assets.as_deref(),
            icon,
            Color::WHITE,
        );
    }
}

pub fn sync_auxiliary_transforms(
    battle: Res<BattleRuntime>,
    playback: Option<Res<EventPlayback>>,
    mut targets: Query<(&IntentTargetVisual, &mut Node), Without<ReactionVisual>>,
    mut reactions: Query<(&ReactionVisual, &mut Node), Without<IntentTargetVisual>>,
) {
    if playback.is_some_and(|playback| playback.input_locked) {
        return;
    }
    for (marker, mut node) in &mut targets {
        if let Some(unit) = battle.0.unit(marker.target) {
            let center = stage_point(unit.position);
            node.left = px(center.x - 56.0);
            node.top = px(center.y - 28.0);
        }
    }
    for (marker, mut node) in &mut reactions {
        if let Some(unit) = battle.0.unit(marker.unit) {
            let center = stage_point(unit.position);
            node.left = px(center.x - 16.0);
            node.top = px(center.y - 48.0 - 16.0);
        }
    }
}

pub fn pulse_telegraphs(
    time: Res<Time>,
    mut telegraphs: Query<(&mut UiTransform, &mut ImageNode), With<TelegraphVisual>>,
) {
    let pulse = (time.elapsed_secs() * 3.2).sin() * 0.045 + 1.0;
    for (mut transform, mut image) in &mut telegraphs {
        transform.scale = Vec2::splat(pulse);
        let mut color = theme::BOARD_TELEGRAPH;
        color.set_alpha(0.28 + (pulse - 0.955) * 2.0);
        image.color = color;
    }
}

pub fn sync_cell_highlights(
    battle: Res<BattleRuntime>,
    interaction: Option<Res<InteractionState>>,
    attack_preview: Option<Res<AttackPreviewCells>>,
    mut cells: Query<(&CellVisual, &mut ImageNode)>,
) {
    let hovered = interaction
        .as_deref()
        .and_then(|interaction| interaction.hovered_cell);
    let selected_unit = interaction
        .as_deref()
        .and_then(inspected_unit)
        .or_else(|| {
            hovered
                .and_then(|cell| battle.0.occupant_at(cell))
                .and_then(|id| battle.0.unit(id))
                .filter(|unit| unit.faction == Faction::Player)
                .map(|unit| unit.id)
        })
        .filter(|id| {
            battle
                .0
                .unit(*id)
                .is_some_and(|unit| unit.faction == Faction::Player)
        });
    let reachable = selected_unit
        .and_then(|unit| battle.0.reachable_cells(unit).ok())
        .unwrap_or_default();
    for (cell, mut image) in &mut cells {
        image.color = if attack_preview
            .as_ref()
            .is_some_and(|preview| preview.0.contains(&cell.0))
        {
            theme::BOARD_ATTACK
        } else if hovered == Some(cell.0) {
            theme::BOARD_SELECTED
        } else if reachable.contains(&cell.0) {
            theme::BOARD_REACHABLE
        } else if (cell.0.x + cell.0.y) % 2 == 0 {
            theme::BOARD_LIGHT
        } else {
            theme::BOARD_DARK
        };
    }
}

fn inspected_unit(interaction: &InteractionState) -> Option<crate::domain::model::UnitId> {
    interaction.inspected_unit
}

fn intent_line_node(
    origin: crate::domain::board::GridPos,
    center: crate::domain::board::GridPos,
) -> (Node, UiTransform) {
    let start = stage_point(origin);
    let end = stage_point(center);
    let delta = end - start;
    let length = delta.length().max(1.0);
    let midpoint = start.midpoint(end);
    (
        Node {
            position_type: PositionType::Absolute,
            left: px(midpoint.x - length * 0.5),
            top: px(midpoint.y - 1.5),
            width: px(length),
            height: px(3.0),
            ..default()
        },
        UiTransform::from_rotation(Rot2::radians(delta.y.atan2(delta.x))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::layout::BATTLE_STAGE_SIZE;

    #[test]
    fn intent_line_is_a_flat_ui_sibling() {
        let (node, transform) = intent_line_node(
            crate::domain::board::GridPos::new(0, 0),
            crate::domain::board::GridPos::new(1, 1),
        );
        assert_eq!(node.height, px(3.0));
        assert!(transform.rotation != Rot2::IDENTITY);
    }

    #[test]
    fn board_stage_fits_the_authored_design_rect() {
        assert_eq!(BATTLE_STAGE_SIZE, Vec2::new(1008.0, 764.0));
    }
}
