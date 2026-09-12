use std::f32::consts::PI;
use std::time::Duration;

use bevy::prelude::*;

use crate::domain::model::BattleEvent;

use super::{
    BattleEventQueue, BattleRuntime, BattleStage, EventEffect, EventPlayback, PresentationRoot,
    RecentBattleLog, RestartRoundPending, UnitVisual,
    assets::UiAssets,
    interaction::StatusMessage,
    layout::{TOKEN_HEIGHT, TOKEN_WIDTH, battle_stage_rect, iso_center},
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
    ),
    Without<EventEffect>,
>;
type EventEffectQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static mut UiTransform), (With<EventEffect>, Without<UnitVisual>)>;
type DamageNumberQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static DamageNumberEffect, &'static mut Node), Without<UnitVisual>>;

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
    mut unit_visuals: UnitVisualQuery,
    mut effects: EventEffectQuery,
    mut damage_numbers: DamageNumberQuery,
) {
    let finished = if let Some((event, timer)) = playback.current.as_mut() {
        timer.tick(time.delta());
        let progress = timer.fraction();
        animate_unit_event(event, progress, &mut unit_visuals);
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
        spawn_event_effect(&mut commands, parent, &event, &battle, &ui_assets);
    }
    if let BattleEvent::DamageApplied { target, amount, .. } = &event
        && let Some(hud_root) = hud_roots.iter().next()
        && let Some(unit) = battle.0.unit(*target)
    {
        let origin = iso_center(unit.position) + Vec2::new(0.0, -TOKEN_HEIGHT - 10.0);
        spawn_damage_number(&mut commands, hud_root, &ui_assets.fonts, origin, *amount);
    }
    animate_unit_event(&event, 0.0, &mut unit_visuals);
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

fn animate_unit_event(event: &BattleEvent, progress: f32, visuals: &mut UnitVisualQuery<'_, '_>) {
    let eased = progress * progress * (3.0 - 2.0 * progress);
    for (visual, mut node, mut transform, mut visibility) in visuals.iter_mut() {
        match event {
            BattleEvent::UnitMoved { unit, from, to }
            | BattleEvent::UnitPushed { unit, from, to }
                if *unit == visual.0 =>
            {
                let from = node_position(*from);
                let to = node_position(*to);
                let current = from.lerp(to, eased);
                node.left = px(current.x);
                node.top = px(current.y);
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
    ui_assets: &UiAssets,
) {
    let position = match event {
        BattleEvent::AttackHitEmpty { cell, .. }
        | BattleEvent::ExplosionTriggered { position: cell, .. }
        | BattleEvent::HazardTriggered { position: cell, .. }
        | BattleEvent::ExplosiveDamaged { position: cell, .. }
        | BattleEvent::CollisionOccurred {
            blocked_at: cell, ..
        } => Some(*cell),
        BattleEvent::AttackRolled {
            target, hit: true, ..
        }
        | BattleEvent::DamageApplied { target, .. }
        | BattleEvent::UnitKnockedOut { unit: target, .. } => {
            battle.0.unit(*target).map(|unit| unit.position)
        }
        _ => None,
    };
    let Some(position) = position else {
        return;
    };
    let center = stage_point(position);
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
    use crate::mission::mission_one::mission_one;
    use bevy::ecs::system::RunSystemOnce;

    fn animate_damage_numbers_halfway(mut damage_numbers: DamageNumberQuery) {
        animate_damage_numbers(0.5, &mut damage_numbers);
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
