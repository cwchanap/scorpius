use std::f32::consts::PI;
use std::time::Duration;

use bevy::prelude::*;

use crate::domain::model::{BattleEvent, UnitId};

use super::{
    BattleEventQueue, BattleRuntime, BattleStage, EventEffect, EventPlayback, IntentTargetVisual,
    PresentationRoot, ReactionVisual, RecentBattleLog, RestartRoundPending, TokenFootprintVisual,
    TokenSelectionVisual, UnitVisual,
    assets::UiAssets,
    interaction::StatusMessage,
    layout::{
        STAGE_EFFECT_DEPTH, TOKEN_HEIGHT, TOKEN_WIDTH, battle_stage_rect, iso_center, token_depth,
    },
    theme,
    ui::{HudRoot, format_event},
};

const UNIT_SCALE: f32 = 1.0;

type UnitVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static UnitVisual,
        &'static mut Node,
        &'static mut UiTransform,
        &'static mut Visibility,
        Option<&'static mut ZIndex>,
    ),
    Without<EventEffect>,
>;
type EventEffectQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static mut UiTransform), (With<EventEffect>, Without<UnitVisual>)>;
type DamageNumberQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static DamageNumberEffect, &'static mut Node), Without<UnitVisual>>;
type FootprintVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static TokenFootprintVisual,
        &'static mut Node,
        Option<&'static mut ZIndex>,
    ),
    (
        Without<UnitVisual>,
        Without<TokenSelectionVisual>,
        Without<DamageNumberEffect>,
    ),
>;
type SelectionVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static TokenSelectionVisual,
        &'static mut Node,
        Option<&'static mut ZIndex>,
    ),
    (
        Without<UnitVisual>,
        Without<TokenFootprintVisual>,
        Without<DamageNumberEffect>,
    ),
>;
type IntentTargetVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static IntentTargetVisual,
        &'static mut Node,
        Option<&'static mut ZIndex>,
    ),
    (
        Without<UnitVisual>,
        Without<TokenFootprintVisual>,
        Without<TokenSelectionVisual>,
        Without<DamageNumberEffect>,
    ),
>;
type ReactionVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ReactionVisual,
        &'static mut Node,
        Option<&'static mut ZIndex>,
    ),
    (
        Without<UnitVisual>,
        Without<TokenFootprintVisual>,
        Without<TokenSelectionVisual>,
        Without<DamageNumberEffect>,
        Without<IntentTargetVisual>,
    ),
>;

/// Unit-bound stage visuals that travel together during move/push playback.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct UnitPlaybackQueries<'w, 's> {
    units: UnitVisualQuery<'w, 's>,
    footprints: FootprintVisualQuery<'w, 's>,
    selections: SelectionVisualQuery<'w, 's>,
    intent_targets: IntentTargetVisualQuery<'w, 's>,
    reactions: ReactionVisualQuery<'w, 's>,
}

#[derive(Component)]
pub(crate) struct DamageNumberEffect {
    origin: Vec2,
}

pub(crate) fn begin_restarted_round(
    mut pending: ResMut<RestartRoundPending>,
    mut battle: ResMut<BattleRuntime>,
    mut queue: ResMut<BattleEventQueue>,
    mut playback: ResMut<EventPlayback>,
    mut status: ResMut<StatusMessage>,
) {
    if !pending.0 {
        return;
    }

    let events = battle
        .0
        .begin_round()
        .expect("restarted authored mission must begin from enemy planning");
    playback.input_locked = !events.is_empty();
    queue.0.extend(events);
    status.0 = "Mission restarted.".to_owned();
    pending.0 = false;
}

fn stage_point(cell: crate::domain::board::GridPos) -> Vec2 {
    iso_center(cell) - battle_stage_rect().min
}

fn stage_parent(
    stages: &Query<Entity, With<BattleStage>>,
    roots: &Query<Entity, With<PresentationRoot>>,
) -> Option<Entity> {
    stages.iter().next().or_else(|| roots.iter().next())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn play_battle_events(
    mut commands: Commands,
    time: Res<Time>,
    battle: Res<BattleRuntime>,
    ui_assets: Res<UiAssets>,
    stages: Query<Entity, With<BattleStage>>,
    roots: Query<Entity, With<PresentationRoot>>,
    hud_roots: Query<Entity, With<HudRoot>>,
    mut queue: ResMut<BattleEventQueue>,
    mut playback: ResMut<EventPlayback>,
    mut recent_log: ResMut<RecentBattleLog>,
    mut unit_queries: UnitPlaybackQueries,
    mut effects: EventEffectQuery,
    mut damage_numbers: DamageNumberQuery,
) {
    let finished = if let Some((event, timer)) = playback.current.as_mut() {
        timer.tick(time.delta());
        let progress = timer.fraction();
        animate_unit_event(event, progress, &mut unit_queries);
        animate_effects(progress, &mut effects);
        animate_damage_numbers(progress, &mut damage_numbers);
        timer.is_finished()
    } else {
        false
    };

    if finished {
        despawn_transient_effects(&mut commands, &mut effects, &mut damage_numbers);
        playback.current = None;
    } else if playback.current.is_some() {
        return;
    }

    let Some(event) = queue.0.pop_front() else {
        playback.input_locked = false;
        return;
    };

    // The queue is the single playback boundary. Logging here means every
    // event is recorded once, even when a timer spans multiple frames.
    recent_log.push(format_event(&event, &battle.0));

    if let Some(parent) = stage_parent(&stages, &roots) {
        spawn_event_effect(
            &mut commands,
            parent,
            &event,
            &battle,
            &unit_queries.units,
            &ui_assets,
        );
    }
    if let BattleEvent::DamageApplied { target, amount, .. } = &event
        && let Some(hud_root) = hud_roots.iter().next()
        && let Some(unit) = battle.0.unit(*target)
    {
        let center = rendered_stage_center(&unit_queries.units, *target)
            .map(|stage_center| stage_center + battle_stage_rect().min)
            .unwrap_or_else(|| iso_center(unit.position));
        let origin = center + Vec2::new(0.0, -TOKEN_HEIGHT - 10.0);
        spawn_damage_number(&mut commands, hud_root, &ui_assets.fonts, origin, *amount);
    }
    animate_unit_event(&event, 0.0, &mut unit_queries);
    playback.current = Some((
        event.clone(),
        Timer::new(event_duration(&event), TimerMode::Once),
    ));
    playback.input_locked = true;
}

fn event_duration(event: &BattleEvent) -> Duration {
    let seconds = match event {
        BattleEvent::UnitMoved { .. } | BattleEvent::UnitPushed { .. } => 0.30,
        BattleEvent::AttackRolled { .. }
        | BattleEvent::DamageApplied { .. }
        | BattleEvent::UnitKnockedOut { .. }
        | BattleEvent::ExplosionTriggered { .. } => 0.22,
        BattleEvent::CollisionOccurred { .. }
        | BattleEvent::HazardTriggered { .. }
        | BattleEvent::ExplosiveDamaged { .. }
        | BattleEvent::AttackHitEmpty { .. }
        | BattleEvent::CounterFired { .. } => 0.18,
        BattleEvent::IntentCommitted { .. } | BattleEvent::IntentCanceled { .. } => 0.12,
        BattleEvent::OptionalObjectiveCompleted
        | BattleEvent::MissionCompleted { .. }
        | BattleEvent::MissionFailed { .. } => 0.18,
    };
    Duration::from_secs_f32(seconds)
}

fn node_position(position: crate::domain::board::GridPos) -> Vec2 {
    let center = stage_point(position);
    Vec2::new(center.x - TOKEN_WIDTH * 0.5, center.y - TOKEN_HEIGHT - 4.0)
}

fn footprint_position(position: crate::domain::board::GridPos) -> Vec2 {
    let center = stage_point(position);
    Vec2::new(center.x - 56.0, center.y - 68.0)
}

fn selection_position(position: crate::domain::board::GridPos) -> Vec2 {
    let center = stage_point(position);
    Vec2::new(center.x - 56.0, center.y - 28.0)
}

/// `IntentTargetVisual` markers sit on the same 112x56 diamond footprint as
/// the selection ring.
fn intent_target_position(position: crate::domain::board::GridPos) -> Vec2 {
    selection_position(position)
}

fn reaction_position(position: crate::domain::board::GridPos) -> Vec2 {
    let center = stage_point(position);
    Vec2::new(center.x - 16.0, center.y - 64.0)
}

/// Continuous sibling depth along the from/to slide; `token_depth` is linear
/// in `x + y`, so lerping the endpoint depths tracks the rendered position.
fn animated_token_depth(
    from: crate::domain::board::GridPos,
    to: crate::domain::board::GridPos,
    eased: f32,
) -> i32 {
    let from = token_depth(from) as f32;
    (from + (token_depth(to) as f32 - from) * eased).round() as i32
}

/// Stage-local center of the card currently rendered for `unit`. Combat
/// feedback must land where the token is drawn; the domain position has
/// already advanced past pushes queued behind the event being played.
fn rendered_stage_center(visuals: &UnitVisualQuery<'_, '_>, unit: UnitId) -> Option<Vec2> {
    visuals.iter().find_map(|(visual, node, ..)| {
        if visual.0 != unit {
            return None;
        }
        match (node.left, node.top) {
            (Val::Px(left), Val::Px(top)) => Some(Vec2::new(
                left + TOKEN_WIDTH * 0.5,
                top + TOKEN_HEIGHT + 4.0,
            )),
            _ => None,
        }
    })
}

fn animate_unit_event(
    event: &BattleEvent,
    progress: f32,
    queries: &mut UnitPlaybackQueries<'_, '_>,
) {
    let eased = progress * progress * (3.0 - 2.0 * progress);
    if let BattleEvent::UnitMoved { unit, from, to } | BattleEvent::UnitPushed { unit, from, to } =
        event
    {
        // Footprint, selection, and unit-attached markers are flat stage
        // siblings, not card children, so they must travel the same from/to
        // path while input is locked. ZIndex follows the animated depth so
        // the moving token re-sorts against blockers mid-slide instead of
        // snapping when the resting sync unlocks.
        let depth = animated_token_depth(*from, *to, eased);
        for (footprint, mut node, zindex) in queries.footprints.iter_mut() {
            if footprint.0 == *unit {
                let current = footprint_position(*from).lerp(footprint_position(*to), eased);
                node.left = px(current.x);
                node.top = px(current.y);
                if let Some(mut zindex) = zindex {
                    *zindex = ZIndex(depth - 1);
                }
            }
        }
        for (selection, mut node, zindex) in queries.selections.iter_mut() {
            if selection.0 == *unit {
                let current = selection_position(*from).lerp(selection_position(*to), eased);
                node.left = px(current.x);
                node.top = px(current.y);
                if let Some(mut zindex) = zindex {
                    *zindex = ZIndex(depth);
                }
            }
        }
        for (marker, mut node, zindex) in queries.intent_targets.iter_mut() {
            if marker.target == *unit {
                let current =
                    intent_target_position(*from).lerp(intent_target_position(*to), eased);
                node.left = px(current.x);
                node.top = px(current.y);
                if let Some(mut zindex) = zindex {
                    *zindex = ZIndex(depth - 1);
                }
            }
        }
        for (marker, mut node, zindex) in queries.reactions.iter_mut() {
            if marker.unit == *unit {
                let current = reaction_position(*from).lerp(reaction_position(*to), eased);
                node.left = px(current.x);
                node.top = px(current.y);
                if let Some(mut zindex) = zindex {
                    *zindex = ZIndex(depth + 1);
                }
            }
        }
    }
    for (visual, mut node, mut transform, mut visibility, zindex) in queries.units.iter_mut() {
        match event {
            BattleEvent::UnitMoved { unit, from, to }
            | BattleEvent::UnitPushed { unit, from, to }
                if *unit == visual.0 =>
            {
                let current = node_position(*from).lerp(node_position(*to), eased);
                node.left = px(current.x);
                node.top = px(current.y);
                if let Some(mut zindex) = zindex {
                    *zindex = ZIndex(animated_token_depth(*from, *to, eased));
                }
            }
            BattleEvent::AttackRolled {
                attacker,
                target,
                hit,
                ..
            } => {
                if *attacker == visual.0 {
                    transform.scale = Vec2::splat(attack_scale(progress));
                }
                if *target == visual.0 && *hit {
                    let pulse = (progress * PI).sin();
                    transform.scale = Vec2::splat(UNIT_SCALE * (1.0 + pulse * 0.16));
                }
            }
            BattleEvent::DamageApplied { target, .. } if *target == visual.0 => {
                transform.translation.x = px((progress * PI * 6.0).sin() * 6.0);
            }
            BattleEvent::UnitKnockedOut { unit, .. } if *unit == visual.0 => {
                *visibility = Visibility::Visible;
                transform.scale = Vec2::splat(UNIT_SCALE * (1.0 - eased).max(0.02));
            }
            BattleEvent::CounterFired { defender, .. } if *defender == visual.0 => {
                let pulse = (progress * PI).sin();
                transform.scale = Vec2::splat(UNIT_SCALE * (1.0 + pulse * 0.12));
            }
            _ => {}
        }
    }
}

fn animate_effects(progress: f32, effects: &mut EventEffectQuery<'_, '_>) {
    let pulse = (progress * PI).sin();
    for (_, mut transform) in effects.iter_mut() {
        transform.scale = Vec2::splat(0.48 + pulse * 0.28);
        transform.translation.y = px(-8.0 * progress);
    }
}

fn animate_damage_numbers(progress: f32, damage_numbers: &mut DamageNumberQuery<'_, '_>) {
    for (_, effect, mut node) in damage_numbers.iter_mut() {
        node.top = px(effect.origin.y - 24.0 * progress);
    }
}

fn attack_scale(progress: f32) -> f32 {
    let pulse = (progress * PI).sin();
    UNIT_SCALE * (1.0 + pulse * 0.10)
}

fn despawn_transient_effects(
    commands: &mut Commands,
    effects: &mut EventEffectQuery<'_, '_>,
    damage_numbers: &mut DamageNumberQuery<'_, '_>,
) {
    for (entity, _) in effects.iter_mut() {
        commands.entity(entity).despawn();
    }
    for (entity, _, _) in damage_numbers.iter_mut() {
        commands.entity(entity).despawn();
    }
}

fn spawn_damage_number(
    commands: &mut Commands,
    hud_root: Entity,
    fonts: &super::theme::FontHandles,
    origin: Vec2,
    amount: i16,
) {
    commands.spawn((
        Text::new(format!("-{amount}")),
        theme::ibm_plex_mono(fonts, 28.0, FontWeight(600)),
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: px(origin.x),
            top: px(origin.y),
            ..default()
        },
        DamageNumberEffect { origin },
        Pickable::IGNORE,
        ChildOf(hud_root),
    ));
}

fn spawn_event_effect(
    commands: &mut Commands,
    parent: Entity,
    event: &BattleEvent,
    battle: &BattleRuntime,
    visuals: &UnitVisualQuery<'_, '_>,
    ui_assets: &UiAssets,
) {
    let center = match event {
        BattleEvent::AttackHitEmpty { cell, .. }
        | BattleEvent::ExplosionTriggered { position: cell, .. }
        | BattleEvent::HazardTriggered { position: cell, .. }
        | BattleEvent::ExplosiveDamaged { position: cell, .. }
        | BattleEvent::CollisionOccurred {
            blocked_at: cell, ..
        } => Some(stage_point(*cell)),
        BattleEvent::AttackRolled {
            target, hit: true, ..
        }
        | BattleEvent::DamageApplied { target, .. }
        | BattleEvent::UnitKnockedOut { unit: target, .. } => {
            rendered_stage_center(visuals, *target).or_else(|| {
                battle
                    .0
                    .unit(*target)
                    .map(|unit| stage_point(unit.position))
            })
        }
        _ => None,
    };
    let Some(center) = center else {
        return;
    };
    let icon = match event {
        BattleEvent::ExplosionTriggered { .. } => theme::ICON_ATTACK,
        BattleEvent::HazardTriggered { .. } => theme::ICON_GUARD,
        BattleEvent::CollisionOccurred { .. } => theme::ICON_COUNTER,
        _ => theme::ICON_ATTACK,
    };
    let effect = commands
        .spawn((
            Name::new("Combat impact"),
            Node {
                position_type: PositionType::Absolute,
                left: px(center.x - 32.0),
                top: px(center.y - 32.0),
                width: px(64.0),
                height: px(64.0),
                ..default()
            },
            UiTransform::IDENTITY,
            EventEffect,
            ZIndex(STAGE_EFFECT_DEPTH),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    commands.entity(effect).insert(theme::icon_node(
        ui_assets.icons.clone(),
        icon,
        Color::WHITE,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::combat::DamageSource;
    use crate::domain::model::WeaponId;
    use crate::mission::mission_one::{ids, mission_one};
    use bevy::ecs::system::RunSystemOnce;
    use bevy::time::TimeUpdateStrategy;

    fn animate_damage_numbers_halfway(mut damage_numbers: DamageNumberQuery) {
        animate_damage_numbers(0.5, &mut damage_numbers);
    }

    fn animate_move_halfway(mut queries: UnitPlaybackQueries) {
        animate_unit_event(
            &BattleEvent::UnitMoved {
                unit: crate::domain::model::UnitId(1),
                from: crate::domain::board::GridPos::new(1, 1),
                to: crate::domain::board::GridPos::new(3, 1),
            },
            0.5,
            &mut queries,
        );
    }

    fn despawn_via_playback_cleanup(
        mut commands: Commands,
        mut effects: EventEffectQuery,
        mut damage_numbers: DamageNumberQuery,
    ) {
        despawn_transient_effects(&mut commands, &mut effects, &mut damage_numbers);
    }

    #[test]
    fn attack_pulse_pins_base_scale_at_start_and_end_and_peaks_midway() {
        assert_eq!(attack_scale(0.0), UNIT_SCALE);
        assert!((attack_scale(1.0) - UNIT_SCALE).abs() < 1e-6);
        assert!(attack_scale(0.5) > UNIT_SCALE);
    }

    #[test]
    fn damage_number_lifecycle_spawns_animates_and_despawns() {
        let mut app = App::new();
        let hud_root = app.world_mut().spawn(HudRoot).id();
        let fonts = std::array::from_fn(|_| Handle::default());
        let mut commands = app.world_mut().commands();
        spawn_damage_number(&mut commands, hud_root, &fonts, Vec2::new(320.0, 240.0), 7);
        app.world_mut().flush();

        let mut query = app
            .world_mut()
            .query::<(&DamageNumberEffect, &Text, &Node)>();
        let (effect, text, node) = query.single(app.world()).unwrap();
        assert_eq!(effect.origin, Vec2::new(320.0, 240.0));
        assert_eq!(text.0, "-7");
        assert_eq!(node.top, px(240.0));

        app.world_mut()
            .run_system_once(animate_damage_numbers_halfway)
            .unwrap();

        let mut query = app.world_mut().query::<&Node>();
        let node = query.single(app.world()).unwrap();
        assert_eq!(node.top, px(228.0));

        app.world_mut()
            .run_system_once(despawn_via_playback_cleanup)
            .unwrap();

        let mut query = app
            .world_mut()
            .query_filtered::<Entity, With<DamageNumberEffect>>();
        assert!(query.iter(app.world()).next().is_none());
    }

    #[test]
    fn move_animation_carries_footprint_and_selection_with_the_card() {
        let mut app = App::new();
        let unit = crate::domain::model::UnitId(1);
        let from = crate::domain::board::GridPos::new(1, 1);
        let to = crate::domain::board::GridPos::new(3, 1);
        let card = app
            .world_mut()
            .spawn((
                UnitVisual(unit),
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
                UiTransform::IDENTITY,
                ZIndex(token_depth(from)),
            ))
            .id();
        let footprint = app
            .world_mut()
            .spawn((
                TokenFootprintVisual(unit),
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
                ZIndex(token_depth(from) - 1),
            ))
            .id();
        let selection = app
            .world_mut()
            .spawn((
                TokenSelectionVisual(unit),
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
                ZIndex(token_depth(from)),
            ))
            .id();
        let intent_target = app
            .world_mut()
            .spawn((
                IntentTargetVisual {
                    attacker: crate::domain::model::UnitId(2),
                    target: unit,
                },
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
                ZIndex(token_depth(from) - 1),
            ))
            .id();
        let reaction = app
            .world_mut()
            .spawn((
                ReactionVisual {
                    unit,
                    reaction: crate::domain::model::Reaction::Guard,
                },
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
                ZIndex(token_depth(from) + 1),
            ))
            .id();

        app.world_mut()
            .run_system_once(animate_move_halfway)
            .unwrap();

        let eased = 0.5_f32;
        let card_mid = node_position(from).lerp(node_position(to), eased);
        let card_node = app.world().get::<Node>(card).unwrap();
        assert_eq!(card_node.left, px(card_mid.x));
        assert_eq!(card_node.top, px(card_mid.y));
        let footprint_mid = footprint_position(from).lerp(footprint_position(to), eased);
        let footprint_node = app.world().get::<Node>(footprint).unwrap();
        assert_eq!(footprint_node.left, px(footprint_mid.x));
        assert_eq!(footprint_node.top, px(footprint_mid.y));
        let selection_mid = selection_position(from).lerp(selection_position(to), eased);
        let selection_node = app.world().get::<Node>(selection).unwrap();
        assert_eq!(selection_node.left, px(selection_mid.x));
        assert_eq!(selection_node.top, px(selection_mid.y));

        let depth = animated_token_depth(from, to, eased);
        assert_eq!(app.world().get::<ZIndex>(card), Some(&ZIndex(depth)));
        assert_eq!(
            app.world().get::<ZIndex>(footprint),
            Some(&ZIndex(depth - 1))
        );
        assert_eq!(app.world().get::<ZIndex>(selection), Some(&ZIndex(depth)));

        let intent_mid = intent_target_position(from).lerp(intent_target_position(to), eased);
        let intent_node = app.world().get::<Node>(intent_target).unwrap();
        assert_eq!(intent_node.left, px(intent_mid.x));
        assert_eq!(intent_node.top, px(intent_mid.y));
        assert_eq!(
            app.world().get::<ZIndex>(intent_target),
            Some(&ZIndex(depth - 1))
        );
        let reaction_mid = reaction_position(from).lerp(reaction_position(to), eased);
        let reaction_node = app.world().get::<Node>(reaction).unwrap();
        assert_eq!(reaction_node.left, px(reaction_mid.x));
        assert_eq!(reaction_node.top, px(reaction_mid.y));
        assert_eq!(
            app.world().get::<ZIndex>(reaction),
            Some(&ZIndex(depth + 1))
        );
    }

    #[test]
    fn unit_target_feedback_anchors_to_the_rendered_token_position() {
        // The domain applies an attack's damage and push before playback sees
        // the events, so `unit.position` is already the post-push cell. The
        // on-stage card still renders at the pre-push cell; impact icons and
        // damage numbers must anchor to that rendered position.
        let mut battle = mission_one(7);
        battle.begin_round().unwrap();
        battle.begin_activation(ids::VANGUARD).unwrap();
        let rendered_cell = battle.unit(ids::VANGUARD).unwrap().position;
        battle
            .move_unit(ids::VANGUARD, crate::domain::board::GridPos::new(4, 8))
            .unwrap();

        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
                0.25,
            )))
            .insert_resource(BattleRuntime(battle))
            .insert_resource(UiAssets {
                key_art: Handle::default(),
                briefing_art: Handle::default(),
                vanguard_art: Handle::default(),
                gunner_art: Handle::default(),
                interceptor_art: Handle::default(),
                icons: Handle::default(),
                board: Handle::default(),
                fonts: std::array::from_fn(|_| Handle::default()),
            })
            .insert_resource(BattleEventQueue(std::collections::VecDeque::from([
                BattleEvent::AttackRolled {
                    attacker: ids::VANGUARD,
                    weapon: WeaponId(0),
                    target: ids::VANGUARD,
                    roll: 40,
                    hit: true,
                    critical_roll: None,
                    critical: false,
                },
                BattleEvent::DamageApplied {
                    target: ids::VANGUARD,
                    amount: 4,
                    remaining_hp: 9,
                    source: DamageSource::Collision,
                },
            ])))
            .init_resource::<EventPlayback>()
            .init_resource::<RecentBattleLog>()
            .add_systems(Update, play_battle_events);
        app.world_mut().spawn(BattleStage);
        app.world_mut().spawn(HudRoot);
        let rendered = node_position(rendered_cell);
        app.world_mut().spawn((
            UnitVisual(ids::VANGUARD),
            Node {
                position_type: PositionType::Absolute,
                left: px(rendered.x),
                top: px(rendered.y),
                ..default()
            },
            UiTransform::IDENTITY,
        ));

        app.update();

        let center = stage_point(rendered_cell);
        let mut effects = app
            .world_mut()
            .query_filtered::<(&Node, &ZIndex), With<EventEffect>>();
        let (effect_node, effect_depth) = effects.single(app.world()).unwrap();
        assert_eq!(effect_node.left, px(center.x - 32.0));
        assert_eq!(effect_node.top, px(center.y - 32.0));
        assert_eq!(*effect_depth, ZIndex(STAGE_EFFECT_DEPTH));

        app.update();

        let origin = iso_center(rendered_cell) + Vec2::new(0.0, -TOKEN_HEIGHT - 10.0);
        let mut numbers = app.world_mut().query::<(&DamageNumberEffect, &Node)>();
        let (number, node) = numbers.single(app.world()).unwrap();
        assert_eq!(number.origin, origin);
        assert_eq!(node.left, px(origin.x));
        assert_eq!(node.top, px(origin.y));
        let (effect_node, _) = effects.single(app.world()).unwrap();
        assert_eq!(effect_node.left, px(center.x - 32.0));
        assert_eq!(effect_node.top, px(center.y - 32.0));
    }

    #[test]
    fn restarted_round_queues_authored_events_before_unlocking_input() {
        let mut app = App::new();
        app.insert_resource(BattleRuntime(mission_one(11)))
            .insert_resource(RestartRoundPending(true))
            .init_resource::<BattleEventQueue>()
            .init_resource::<EventPlayback>()
            .init_resource::<StatusMessage>()
            .add_systems(Update, begin_restarted_round);

        app.update();

        assert_eq!(app.world().resource::<BattleRuntime>().0.round(), 1);
        assert!(!app.world().resource::<BattleEventQueue>().0.is_empty());
        assert!(app.world().resource::<EventPlayback>().input_locked);
        assert!(!app.world().resource::<RestartRoundPending>().0);
    }

    #[test]
    fn playback_records_each_event_in_the_recent_battle_log() {
        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin)
            .insert_resource(BattleRuntime(mission_one(7)))
            .insert_resource(UiAssets {
                key_art: Handle::default(),
                briefing_art: Handle::default(),
                vanguard_art: Handle::default(),
                gunner_art: Handle::default(),
                interceptor_art: Handle::default(),
                icons: Handle::default(),
                board: Handle::default(),
                fonts: std::array::from_fn(|_| Handle::default()),
            })
            .insert_resource(BattleEventQueue(std::collections::VecDeque::from([
                BattleEvent::OptionalObjectiveCompleted,
            ])))
            .init_resource::<EventPlayback>()
            .init_resource::<RecentBattleLog>()
            .add_systems(Update, play_battle_events);

        app.update();

        assert_eq!(
            app.world()
                .resource::<RecentBattleLog>()
                .0
                .front()
                .map(String::as_str),
            Some("BONUS ACHIEVED")
        );
    }
}
