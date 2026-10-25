use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

use crate::domain::{
    battle::BattleState,
    board::GridPos,
    model::{BattlePhase, Faction, PrimaryObjective, Reaction, UnitId, UnitState},
};

use super::{
    AttackPreviewCells, BattleMap, BattleRuntime, CellInsetVisual, CellOverlayVisual, CellVisual,
    EventPlayback, ExtractionVisual, IntentLineVisual, IntentTargetVisual, PresentationRoot,
    PropVisual, ReactionVisual, TelegraphGlyphVisual, TelegraphVisual, TokenAwaiting, TokenCard,
    TokenFootprintVisual, TokenHpFill, TokenHpText, TokenSelectionInset, TokenSelectionVisual,
    UnitVisual,
    assets::UiAssets,
    interaction::{InteractionMode, InteractionState},
    layout::{
        battle_stage_rect, footprint_top_left, iso_center, selection_top_left, stage_effect_depth,
        token_depth, unit_root_top_left,
    },
    theme,
};

fn stage_point(cell: crate::domain::board::GridPos) -> Vec2 {
    iso_center(cell) - battle_stage_rect().min
}

/// Tall forest/mountain cells sort just under the token slice so the art
/// occludes tokens behind it; a telegraph on such a cell rides one step
/// above the art — the committed footprint stays visible while the token
/// standing on the cell still draws on top.
fn telegraph_depth(
    battle: &crate::domain::battle::BattleState,
    cell: crate::domain::board::GridPos,
) -> i32 {
    if battle
        .board()
        .terrain_at(cell)
        .is_some_and(theme::terrain_occludes)
    {
        token_depth(cell) - 1
    } else {
        5
    }
}

fn marker_parent(
    stage: &Query<Entity, With<BattleMap>>,
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

type UnitTransformQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static UnitVisual,
        &'static mut Node,
        &'static mut ZIndex,
        &'static mut Visibility,
    ),
    (Without<TokenFootprintVisual>, Without<TokenSelectionVisual>),
>;
type FootprintTransformQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static TokenFootprintVisual,
        &'static mut Node,
        &'static mut ZIndex,
        &'static mut Visibility,
    ),
    (Without<UnitVisual>, Without<TokenSelectionVisual>),
>;
type SelectionFootprintQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static TokenSelectionVisual,
        &'static mut Node,
        &'static mut ImageNode,
        &'static mut ZIndex,
        &'static mut Visibility,
    ),
    (Without<UnitVisual>, Without<TokenFootprintVisual>),
>;

type SelectionInsetQuery<'w, 's> = Query<
    'w,
    's,
    (&'static TokenSelectionInset, &'static mut ImageNode),
    Without<TokenSelectionVisual>,
>;

pub fn apply_unit_transforms(
    battle: Res<BattleRuntime>,
    playback: Option<Res<EventPlayback>>,
    mut visuals: UnitTransformQuery,
    mut footprints: FootprintTransformQuery,
    mut selection_footprints: SelectionFootprintQuery,
    mut selection_insets: SelectionInsetQuery,
    interaction: Option<Res<InteractionState>>,
) {
    if playback.is_some_and(|playback| playback.input_locked) {
        return;
    }
    for (visual, mut node, mut zindex, mut visibility) in &mut visuals {
        if let Some(unit) = battle.0.unit(visual.0) {
            let root = unit_root_top_left(unit.position);
            node.left = px(root.x);
            node.top = px(root.y);
            *zindex = ZIndex(token_depth(unit.position));
            *visibility = if unit.is_knocked_out() {
                Visibility::Hidden
            } else {
                Visibility::Visible
            };
        }
    }
    for (footprint, mut node, mut zindex, mut visibility) in &mut footprints {
        if let Some(unit) = battle.0.unit(footprint.0) {
            let top_left = footprint_top_left(unit.position);
            node.left = px(top_left.x);
            node.top = px(top_left.y);
            *zindex = ZIndex(token_depth(unit.position) - 1);
            *visibility = if unit.is_knocked_out() {
                Visibility::Hidden
            } else {
                Visibility::Visible
            };
        }
    }
    let inspected = interaction
        .as_deref()
        .and_then(|interaction| interaction.inspected_unit);
    for (footprint, mut node, mut image, mut zindex, mut visibility) in &mut selection_footprints {
        if let Some(unit) = battle.0.unit(footprint.0) {
            let top_left = selection_top_left(unit.position);
            node.left = px(top_left.x);
            node.top = px(top_left.y);
            *zindex = ZIndex(token_depth(unit.position));
            let tint = if unit.is_knocked_out() {
                None
            } else if battle.0.active_unit() == Some(unit.id) {
                Some(theme::BOARD_SELECTED)
            } else if let Some((ring, _)) = target_ring(&battle.0, interaction.as_deref(), unit) {
                Some(ring)
            } else if inspected == Some(unit.id) {
                Some(theme::BOARD_INSPECTED)
            } else {
                None
            };
            *visibility = if tint.is_some() {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
            if let Some(tint) = tint {
                image.color = tint;
            }
        }
    }
    // A target's ring fills with its color so the token stands out over
    // the shadow; the active and inspected rings keep their dark center.
    for (inset, mut image) in &mut selection_insets {
        let fill = battle
            .0
            .unit(inset.0)
            .filter(|unit| battle.0.active_unit() != Some(unit.id))
            .and_then(|unit| target_ring(&battle.0, interaction.as_deref(), unit))
            .map_or(theme::BOARD_DARK, |(_, fill)| fill);
        if image.color != fill {
            image.color = fill;
        }
    }
}

pub fn sync_token_cards(
    battle: Res<BattleRuntime>,
    playback: Option<Res<EventPlayback>>,
    mut cards: Query<(&TokenCard, &mut ImageNode)>,
    mut hp_fills: Query<(&TokenHpFill, &mut Node)>,
    mut hp_texts: Query<(&TokenHpText, &mut Text)>,
    mut awaiting: Query<(&TokenAwaiting, &mut Visibility)>,
) {
    if playback.is_some_and(|playback| playback.input_locked) {
        return;
    }
    for (card, mut image) in &mut cards {
        if let Some(unit) = battle.0.unit(card.0) {
            let faded = unit.activation.finished && unit.faction == Faction::Player;
            image.color = Color::WHITE.with_alpha(if faded { 0.52 } else { 1.0 });
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
    stages: Query<Entity, With<BattleMap>>,
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
                ZIndex(telegraph_depth(&battle.0, cell)),
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
    stages: Query<Entity, With<BattleMap>>,
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
    // Regional tall terrain would bury a flat-9 guide under trees and peaks;
    // there the guides ride the foreground layer above the whole stage stack.
    // Flat boards keep the authored layering under the tokens.
    let board = battle.0.board();
    let line_depth = if super::map_view::is_regional(board.width(), board.height()) {
        stage_effect_depth(board.width(), board.height())
    } else {
        9
    };
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
                ZIndex(token_depth(target_unit.position) - 1),
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
                ZIndex(line_depth),
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
    stages: Query<Entity, With<BattleMap>>,
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
    stages: Query<Entity, With<BattleMap>>,
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
                ZIndex(token_depth(unit_state.position) + 1),
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
    mut targets: Query<(&IntentTargetVisual, &mut Node, &mut ZIndex), Without<ReactionVisual>>,
    mut reactions: Query<(&ReactionVisual, &mut Node, &mut ZIndex), Without<IntentTargetVisual>>,
) {
    if playback.is_some_and(|playback| playback.input_locked) {
        return;
    }
    for (marker, mut node, mut zindex) in &mut targets {
        if let Some(unit) = battle.0.unit(marker.target) {
            let center = stage_point(unit.position);
            node.left = px(center.x - 56.0);
            node.top = px(center.y - 28.0);
            *zindex = ZIndex(token_depth(unit.position) - 1);
        }
    }
    for (marker, mut node, mut zindex) in &mut reactions {
        if let Some(unit) = battle.0.unit(marker.unit) {
            let center = stage_point(unit.position);
            node.left = px(center.x - 16.0);
            node.top = px(center.y - 48.0 - 16.0);
            *zindex = ZIndex(token_depth(unit.position) + 1);
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

/// What a board cell offers the player right now.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CellHighlight {
    /// Where the hovered, inspected, or active mech could still move before
    /// MOVE is armed.
    MoveRange,
    /// A destination the armed MOVE accepts.
    Move,
    /// Inside the armed weapon's reach, with nothing there to hit.
    AttackReach,
    /// A target the armed weapon accepts.
    Attack,
    /// The hovered target's blast footprint.
    Footprint,
    /// An ally the armed Aegis can shield.
    Ally,
}

impl CellHighlight {
    /// Whether a click here carries out the armed command.
    pub const fn clickable(self) -> bool {
        matches!(self, Self::Move | Self::Attack | Self::Ally)
    }
}

/// Every highlighted cell this frame, plus the hovered cell when a click
/// there would act.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CellHighlights {
    pub cells: BTreeMap<GridPos, CellHighlight>,
    pub hovered: Option<GridPos>,
}

/// Project the armed command (or the mech under inspection) onto the board.
/// Reads only public domain queries, so a highlighted target is exactly a
/// cell the matching command accepts.
pub fn cell_highlights(
    battle: &BattleState,
    interaction: &InteractionState,
    footprint: &BTreeSet<GridPos>,
) -> CellHighlights {
    let mut cells = BTreeMap::new();
    match (interaction.mode, battle.active_unit()) {
        (InteractionMode::Move, Some(unit)) => {
            for cell in battle.reachable_cells(unit).unwrap_or_default() {
                cells.insert(cell, CellHighlight::Move);
            }
        }
        (InteractionMode::Attack(weapon), Some(unit)) => {
            for cell in battle.weapon_reach(unit, weapon).unwrap_or_default() {
                let kind = if battle.preview_attack(unit, weapon, cell).is_ok() {
                    CellHighlight::Attack
                } else {
                    CellHighlight::AttackReach
                };
                cells.insert(cell, kind);
            }
        }
        (InteractionMode::AegisTarget, Some(vanguard)) => {
            for ally in battle
                .units()
                .filter(|ally| aegis_can_shield(battle, vanguard, ally))
            {
                cells.insert(ally.position, CellHighlight::Ally);
            }
        }
        _ => {
            if let Some(unit) = move_preview_unit(battle, interaction) {
                for cell in battle.reachable_cells(unit).unwrap_or_default() {
                    cells.insert(cell, CellHighlight::MoveRange);
                }
            }
        }
    }
    let hovered = interaction
        .hovered_cell
        .filter(|cell| cells.get(cell).is_some_and(|kind| kind.clickable()));
    for cell in footprint {
        cells.insert(*cell, CellHighlight::Footprint);
    }
    CellHighlights { cells, hovered }
}

/// The living ally orthogonally next to the Vanguard that Aegis accepts.
fn aegis_can_shield(battle: &BattleState, vanguard: UnitId, ally: &UnitState) -> bool {
    !battle.pilot_skills().aegis_used
        && battle.unit(vanguard).is_some_and(|vanguard| {
            ally.faction == Faction::Player
                && !ally.is_knocked_out()
                && ally.position.manhattan(vanguard.position) == 1
        })
}

/// The ring and fill a unit's token takes as the armed command's target:
/// amber for an enemy the armed weapon can hit, mint for an ally Aegis can
/// shield.
fn target_ring(
    battle: &BattleState,
    interaction: Option<&InteractionState>,
    unit: &UnitState,
) -> Option<(Color, Color)> {
    match (interaction?.mode, battle.active_unit()?) {
        (InteractionMode::Attack(weapon), attacker) => battle
            .preview_attack(attacker, weapon, unit.position)
            .is_ok()
            .then_some((
                theme::BOARD_ATTACK,
                theme::BOARD_ATTACK_FILL.with_alpha(0.75),
            )),
        (InteractionMode::AegisTarget, vanguard) => aegis_can_shield(battle, vanguard, unit)
            .then_some((theme::MINT, theme::MINT.with_alpha(0.55))),
        _ => None,
    }
}

/// With no command armed, the hovered mech's move range previews first,
/// then the inspected mech's, then the active one's; a mech that has already
/// moved or finished shows none.
fn move_preview_unit(battle: &BattleState, interaction: &InteractionState) -> Option<UnitId> {
    if battle.phase() != BattlePhase::Player || battle.result().is_some() {
        return None;
    }
    let can_move = |id: UnitId| {
        battle.unit(id).filter(|unit| {
            unit.faction == Faction::Player
                && !unit.is_knocked_out()
                && !unit.activation.moved
                && !unit.activation.finished
        })
    };
    interaction
        .hovered_cell
        .and_then(|cell| battle.occupant_at(cell))
        .and_then(can_move)
        .or_else(|| interaction.inspected_unit.and_then(can_move))
        .or_else(|| battle.active_unit().and_then(can_move))
        .map(|unit| unit.id)
}

/// Rim and overlay tints for one cell. `pulse` (0..=1) breathes the
/// clickable cells; the hovered clickable cell holds steady with a white rim.
pub fn highlight_tints(
    highlight: Option<CellHighlight>,
    hovered: bool,
    pulse: f32,
    rest_rim: Color,
) -> (Color, Color) {
    let Some(highlight) = highlight else {
        return (rest_rim, Color::NONE);
    };
    let (rim, fill, alpha) = match highlight {
        CellHighlight::MoveRange => (theme::BOARD_SELECTED, theme::BOARD_MOVE_FILL, 0.16),
        CellHighlight::Move => (
            theme::BOARD_SELECTED,
            theme::BOARD_MOVE_FILL,
            0.34 + 0.16 * pulse,
        ),
        CellHighlight::AttackReach => (theme::BOARD_ATTACK, theme::BOARD_ATTACK_FILL, 0.18),
        CellHighlight::Attack => (
            theme::BOARD_ATTACK,
            theme::BOARD_ATTACK_FILL,
            0.4 + 0.16 * pulse,
        ),
        CellHighlight::Footprint => (theme::BOARD_ATTACK, theme::BOARD_ATTACK_FILL, 0.62),
        CellHighlight::Ally => (theme::MINT, theme::MINT, 0.3 + 0.14 * pulse),
    };
    if hovered {
        (theme::BOARD_HOVER, fill.with_alpha(0.62))
    } else {
        (rim, fill.with_alpha(alpha))
    }
}

/// One layer of every board cell (rim, terrain inset, or highlight overlay);
/// the two `Without`s keep the layers' `ImageNode` borrows disjoint.
type CellLayerQuery<'w, 's, T, A, B> =
    Query<'w, 's, (&'static T, &'static mut ImageNode), (Without<A>, Without<B>)>;

pub fn sync_cell_highlights(
    battle: Res<BattleRuntime>,
    interaction: Option<Res<InteractionState>>,
    attack_preview: Option<Res<AttackPreviewCells>>,
    time: Option<Res<Time>>,
    mut cells: CellLayerQuery<CellVisual, CellInsetVisual, CellOverlayVisual>,
    mut insets: CellLayerQuery<CellInsetVisual, CellVisual, CellOverlayVisual>,
    mut overlays: CellLayerQuery<CellOverlayVisual, CellVisual, CellInsetVisual>,
) {
    let highlights = interaction
        .as_deref()
        .map_or_else(CellHighlights::default, |interaction| {
            let footprint = attack_preview.as_deref().map(|preview| &preview.0);
            cell_highlights(
                &battle.0,
                interaction,
                footprint.unwrap_or(&BTreeSet::new()),
            )
        });
    let pulse = time.map_or(0.5, |time| (time.elapsed_secs() * 3.2).sin() * 0.5 + 0.5);
    let board = battle.0.board();
    let tints_for = |cell: GridPos| {
        highlight_tints(
            highlights.cells.get(&cell).copied(),
            highlights.hovered == Some(cell),
            pulse,
            super::map_view::cell_base_rim(board, cell),
        )
    };
    for (cell, mut image) in &mut cells {
        let tint = tints_for(cell.0).0;
        if image.color != tint {
            image.color = tint;
        }
    }
    for (cell, mut image) in &mut overlays {
        let tint = tints_for(cell.0).1;
        if image.color != tint {
            image.color = tint;
        }
    }
    // Highlights never tint the inset, so terrain art keeps its colors.
    for (cell, mut image) in &mut insets {
        let tint = super::map_view::cell_base_fill(board, cell.0);
        if image.color != tint {
            image.color = tint;
        }
    }
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
    use crate::presentation::battlefield::setup_mission_scene;
    use crate::presentation::layout::BATTLE_STAGE_SIZE;

    fn blank_ui_assets() -> UiAssets {
        UiAssets {
            key_art: Handle::default(),
            briefing_art: Handle::default(),
            vanguard_art: Handle::default(),
            gunner_art: Handle::default(),
            interceptor_art: Handle::default(),
            vanguard_map: Handle::default(),
            gunner_map: Handle::default(),
            interceptor_map: Handle::default(),
            enemy_map: Handle::default(),
            control_portrait: Handle::default(),
            icons: Handle::default(),
            board: Handle::default(),
            terrain: Handle::default(),
            fonts: std::array::from_fn(|_| Handle::default()),
        }
    }

    #[test]
    fn tall_terrain_does_not_bury_telegraphs_or_intent_lines() {
        use crate::domain::board::GridPos;
        use crate::mission::mission_one::{ids, mission_one};

        // The Vanguard stands in forest at (11,6); the striker and a rifleman
        // are committed onto it, so the telegraph diamond and the guide lines
        // must sort above the 70px tree art on that cell.
        let mut battle = mission_one(7);
        battle.move_unit_direct_for_test(ids::VANGUARD, GridPos::new(11, 6));
        battle.move_unit_direct_for_test(ids::RIFLEMAN_LEFT, GridPos::new(11, 5));
        battle.move_unit_direct_for_test(ids::STRIKER, GridPos::new(11, 7));
        battle.set_round_for_test(1);
        battle.begin_round().unwrap();

        let mut app = App::new();
        app.insert_resource(BattleRuntime(battle))
            .insert_resource(blank_ui_assets())
            .add_systems(
                Update,
                (
                    setup_mission_scene,
                    reconcile_telegraph_markers,
                    reconcile_intent_guides,
                )
                    .chain(),
            );
        app.update();

        let forest = GridPos::new(11, 6);
        let mut cells = app.world_mut().query::<(&CellVisual, &ZIndex)>();
        let forest_z = cells
            .iter(app.world())
            .find(|(cell, _)| cell.0 == forest)
            .map(|(_, z)| z.0)
            .expect("the forest cell spawns with tall art");
        assert_eq!(
            forest_z,
            token_depth(forest) - 2,
            "forest art sorts just under the token slice"
        );

        let mut telegraphs = app.world_mut().query::<(&TelegraphVisual, &ZIndex)>();
        let telegraph_z = telegraphs
            .iter(app.world())
            .find(|(marker, _)| marker.cell == forest)
            .map(|(_, z)| z.0)
            .expect("the committed footprint marks the forest cell");
        assert!(
            telegraph_z > forest_z,
            "telegraph clears the tree art: {telegraph_z} vs {forest_z}"
        );
        assert!(
            telegraph_z < token_depth(forest),
            "telegraph stays under the token standing on the cell"
        );

        // Regional guide lines ride above the whole stage stack so no tree
        // or peak along the shot can bury a committed threat.
        let mut lines = app.world_mut().query::<(&IntentLineVisual, &ZIndex)>();
        assert!(
            lines.iter(app.world()).count() > 0,
            "committed intents spawn guide lines"
        );
        for (line, z) in lines.iter(app.world()) {
            assert_eq!(
                z.0,
                crate::presentation::layout::stage_effect_depth(128, 128),
                "line from ({},{}): regional guides overlay the stage stack",
                line.origin.x,
                line.origin.y
            );
        }
    }

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

    #[test]
    fn regional_highlights_paint_the_overlay_and_never_tint_terrain_art() {
        use crate::mission::mission_one::{ids, mission_one};

        let mut battle = mission_one(7);
        battle.begin_round().unwrap();
        battle.begin_activation(ids::VANGUARD).unwrap();
        let mut app = App::new();
        app.insert_resource(BattleRuntime(battle))
            .insert_resource(InteractionState {
                mode: InteractionMode::Move,
                ..InteractionState::default()
            })
            .insert_resource(AttackPreviewCells::default())
            .add_systems(Update, sync_cell_highlights);

        let reachable_cell = GridPos::new(4, 8);
        assert!(
            app.world()
                .resource::<BattleRuntime>()
                .0
                .reachable_cells(ids::VANGUARD)
                .unwrap()
                .contains(&reachable_cell)
        );
        let preview_cell = GridPos::new(5, 7);
        app.world_mut()
            .resource_mut::<AttackPreviewCells>()
            .0
            .insert(preview_cell);
        // A far Plain tile: same atlas art, no highlight.
        let idle_cell = GridPos::new(30, 12);
        for cell in [reachable_cell, preview_cell, idle_cell] {
            app.world_mut()
                .spawn((CellVisual(cell), ImageNode::default()));
            app.world_mut()
                .spawn((CellInsetVisual(cell), ImageNode::default()));
            app.world_mut()
                .spawn((CellOverlayVisual(cell), ImageNode::default()));
        }
        app.update();

        let world = app.world_mut();
        let outer = tints::<CellVisual>(world, |visual| visual.0);
        let inset = tints::<CellInsetVisual>(world, |visual| visual.0);
        let overlay = tints::<CellOverlayVisual>(world, |visual| visual.0);
        // The rim and the translucent overlay carry the highlight; the art
        // underneath is never tinted.
        assert_eq!(outer[&reachable_cell], theme::BOARD_SELECTED);
        assert_eq!(
            overlay[&reachable_cell].with_alpha(1.0),
            theme::BOARD_MOVE_FILL
        );
        assert!(overlay[&reachable_cell].alpha() > 0.3);
        assert_eq!(outer[&preview_cell], theme::BOARD_ATTACK);
        assert_eq!(
            overlay[&preview_cell],
            theme::BOARD_ATTACK_FILL.with_alpha(0.62)
        );
        assert_eq!(
            outer[&idle_cell],
            theme::terrain_rim(crate::domain::board::Terrain::Plain)
        );
        assert_eq!(overlay[&idle_cell], Color::NONE);
        for cell in [reachable_cell, preview_cell, idle_cell] {
            assert_eq!(inset[&cell], Color::WHITE);
        }
    }

    fn tints<T: Component>(
        world: &mut World,
        cell: impl Fn(&T) -> GridPos,
    ) -> BTreeMap<GridPos, Color> {
        world
            .query::<(&T, &ImageNode)>()
            .iter(world)
            .map(|(visual, image)| (cell(visual), image.color))
            .collect()
    }

    #[test]
    fn armed_weapons_split_reach_from_targets_and_hover_only_marks_clickable_cells() {
        use crate::mission::mission_one::{ids, mission_one};

        let mut battle = mission_one(7);
        battle.begin_round().unwrap();
        battle.begin_activation(ids::VANGUARD).unwrap();
        let vanguard = battle.unit(ids::VANGUARD).unwrap().position;
        let striker = GridPos::new(vanguard.x, vanguard.y - 1);
        battle.move_unit_direct_for_test(ids::STRIKER, striker);
        let empty = GridPos::new(vanguard.x - 1, vanguard.y);
        assert_eq!(battle.occupant_at(empty), None);

        let mut interaction = InteractionState {
            mode: InteractionMode::Attack(ids::PILE_LANCE),
            hovered_cell: Some(empty),
            ..InteractionState::default()
        };
        let highlights = cell_highlights(&battle, &interaction, &BTreeSet::new());
        assert_eq!(highlights.cells[&striker], CellHighlight::Attack);
        assert_eq!(highlights.cells[&empty], CellHighlight::AttackReach);
        assert_eq!(highlights.hovered, None, "nothing to hit under the cursor");

        interaction.hovered_cell = Some(striker);
        let footprint = BTreeSet::from([striker]);
        let highlights = cell_highlights(&battle, &interaction, &footprint);
        assert_eq!(highlights.hovered, Some(striker));
        assert_eq!(highlights.cells[&striker], CellHighlight::Footprint);
        assert_eq!(
            highlight_tints(
                Some(CellHighlight::Footprint),
                true,
                0.0,
                theme::BOARD_STROKE
            ),
            (
                theme::BOARD_HOVER,
                theme::BOARD_ATTACK_FILL.with_alpha(0.62)
            )
        );
    }

    #[test]
    fn an_armed_weapon_rings_the_enemies_it_can_hit() {
        use crate::mission::mission_one::{ids, mission_one};

        let mut battle = mission_one(7);
        battle.begin_round().unwrap();
        battle.begin_activation(ids::VANGUARD).unwrap();
        let mut app = App::new();
        app.insert_resource(BattleRuntime(battle))
            .insert_resource(InteractionState {
                mode: InteractionMode::Attack(ids::PILE_LANCE),
                ..InteractionState::default()
            })
            .add_systems(Update, apply_unit_transforms);
        let mut ring = |unit| {
            let ring = app
                .world_mut()
                .spawn((
                    TokenSelectionVisual(unit),
                    Node::default(),
                    ImageNode::default(),
                    ZIndex::default(),
                    Visibility::Hidden,
                ))
                .id();
            let inset = app
                .world_mut()
                .spawn((TokenSelectionInset(unit), ImageNode::default()))
                .id();
            (ring, inset)
        };
        // The opening Striker stands next to the Vanguard; the Riflemen are
        // out of the Pile Lance's one-cell reach.
        let striker = ring(ids::STRIKER);
        let rifleman = ring(ids::RIFLEMAN_LEFT);
        app.update();

        let world = app.world();
        assert_eq!(
            world.get::<Visibility>(striker.0),
            Some(&Visibility::Visible)
        );
        assert_eq!(
            world.get::<ImageNode>(striker.0).unwrap().color,
            theme::BOARD_ATTACK
        );
        assert_eq!(
            world
                .get::<ImageNode>(striker.1)
                .unwrap()
                .color
                .with_alpha(1.0),
            theme::BOARD_ATTACK_FILL
        );
        assert_eq!(
            world.get::<Visibility>(rifleman.0),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            world.get::<ImageNode>(rifleman.1).unwrap().color,
            theme::BOARD_DARK
        );
    }

    #[test]
    fn idle_move_previews_stop_once_the_mech_has_moved() {
        use crate::mission::mission_one::{ids, mission_one};

        let mut battle = mission_one(7);
        battle.begin_round().unwrap();
        battle.begin_activation(ids::VANGUARD).unwrap();
        let interaction = InteractionState::default();
        let preview = cell_highlights(&battle, &interaction, &BTreeSet::new());
        assert!(!preview.cells.is_empty());
        assert!(
            preview
                .cells
                .values()
                .all(|kind| *kind == CellHighlight::MoveRange)
        );
        assert_eq!(preview.hovered, None, "a preview is not clickable");

        battle.move_unit(ids::VANGUARD, GridPos::new(4, 8)).unwrap();
        let moved = cell_highlights(&battle, &interaction, &BTreeSet::new());
        assert!(moved.cells.is_empty());

        // Aegis marks only the living allies next to the Vanguard.
        let mut aegis = InteractionState {
            mode: InteractionMode::AegisTarget,
            ..InteractionState::default()
        };
        let allies = cell_highlights(&battle, &aegis, &BTreeSet::new());
        assert!(!allies.cells.is_empty());
        for (cell, kind) in &allies.cells {
            assert_eq!(*kind, CellHighlight::Ally);
            assert_eq!(cell.manhattan(GridPos::new(4, 8)), 1);
        }
        aegis.mode = InteractionMode::Inspect;
        assert!(
            cell_highlights(&battle, &aegis, &BTreeSet::new())
                .cells
                .is_empty()
        );
    }

    #[test]
    fn a_spent_aegis_marks_no_allies_and_cannot_be_rearmed() {
        use crate::mission::mission_one::{ids, mission_one};
        use crate::presentation::interaction::{CommandAction, execute_command};

        let mut battle = mission_one(7);
        battle.begin_round().unwrap();
        battle.begin_activation(ids::VANGUARD).unwrap();
        battle.move_unit(ids::VANGUARD, GridPos::new(4, 8)).unwrap();
        let mut interaction = InteractionState::default();
        execute_command(&mut battle, &mut interaction, CommandAction::PilotSkill).unwrap();
        assert!(
            cell_highlights(&battle, &interaction, &BTreeSet::new())
                .cells
                .values()
                .any(|kind| *kind == CellHighlight::Ally)
        );

        battle.use_aegis(ids::GUNNER).unwrap();
        assert!(
            cell_highlights(&battle, &interaction, &BTreeSet::new())
                .cells
                .is_empty(),
            "a spent Aegis advertises no targets"
        );
        interaction.mode = InteractionMode::Inspect;
        assert_eq!(
            execute_command(&mut battle, &mut interaction, CommandAction::PilotSkill),
            Err(crate::domain::model::BattleError::PilotSkillAlreadyUsed)
        );
        assert_eq!(interaction.mode, InteractionMode::Inspect);
    }

    #[test]
    fn idle_move_previews_need_an_open_player_phase() {
        use crate::mission::mission_one::{ids, mission_one};

        let mut battle = mission_one(7);
        let interaction = InteractionState {
            inspected_unit: Some(ids::VANGUARD),
            ..InteractionState::default()
        };
        assert_eq!(battle.phase(), BattlePhase::EnemyPlanning);
        assert!(
            cell_highlights(&battle, &interaction, &BTreeSet::new())
                .cells
                .is_empty()
        );

        battle.begin_round().unwrap();
        assert!(
            !cell_highlights(&battle, &interaction, &BTreeSet::new())
                .cells
                .is_empty()
        );

        battle.set_phase_for_test(BattlePhase::Victory);
        assert!(
            cell_highlights(&battle, &interaction, &BTreeSet::new())
                .cells
                .is_empty(),
            "the un-moved Vanguard previews no range after the battle"
        );
    }
}
