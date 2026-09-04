use std::{f32::consts::PI, time::Duration};

use bevy::prelude::*;

use crate::domain::model::{BattleEvent, UnitArchetype};

use super::{
    BattleEventQueue, BattleRuntime, EventEffect, EventPlayback, PresentationRoot,
    RestartRoundPending, UnitVisual,
    assets::MissionAssets,
    battlefield::BattleCamera,
    grid_to_world,
    interaction::StatusMessage,
    ui::{HudRoot, text_font},
};

const UNIT_SCALE: f32 = 0.72;

type UnitVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static UnitVisual,
        &'static mut Transform,
        &'static mut Visibility,
    ),
    Without<EventEffect>,
>;
type EventEffectQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static mut Transform), (With<EventEffect>, Without<UnitVisual>)>;
type DamageNumberQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static DamageNumberEffect, &'static mut Node)>;

#[derive(Component)]
pub(crate) struct DamageNumberEffect {
    origin: Vec2,
}

#[allow(clippy::type_complexity)]
type CameraQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Camera,
        &'static GlobalTransform,
        &'static BattleCamera,
        &'static mut Transform,
    ),
    (With<Camera3d>, Without<UnitVisual>, Without<EventEffect>),
>;

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

#[allow(clippy::too_many_arguments)]
pub(crate) fn play_battle_events(
    mut commands: Commands,
    time: Res<Time>,
    battle: Res<BattleRuntime>,
    mission_assets: Res<MissionAssets>,
    roots: Query<Entity, With<PresentationRoot>>,
    hud_roots: Query<Entity, With<HudRoot>>,
    mut cameras: CameraQuery,
    mut queue: ResMut<BattleEventQueue>,
    mut playback: ResMut<EventPlayback>,
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
        if is_boss_attack(event, &battle) {
            for (_, _, camera, mut transform) in &mut cameras {
                *transform = boss_camera_transform(camera.rest, progress);
            }
        } else {
            restore_camera(&mut cameras);
        }
        timer.is_finished()
    } else {
        false
    };

    if finished {
        despawn_transient_effects(&mut commands, &mut effects, &mut damage_numbers);
        restore_camera(&mut cameras);
        playback.current = None;
    } else if playback.current.is_some() {
        return;
    }

    let Some(event) = queue.0.pop_front() else {
        playback.input_locked = false;
        return;
    };

    if let Some(root) = roots.iter().next() {
        spawn_event_effect(&mut commands, root, &event, &battle, &mission_assets);
    }
    if let BattleEvent::DamageApplied { target, amount, .. } = &event
        && let Some(hud_root) = hud_roots.iter().next()
        && let Some(unit) = battle.0.unit(*target)
        && let Some((camera, camera_transform, _, _)) = cameras.iter().next()
        && let Ok(viewport) = camera.world_to_viewport(
            camera_transform,
            grid_to_world(unit.position) + Vec3::Y * 0.8,
        )
    {
        spawn_damage_number(&mut commands, hud_root, viewport, *amount);
    }
    animate_unit_event(&event, 0.0, &mut unit_visuals);
    playback.current = Some((
        event.clone(),
        Timer::new(event_duration(&event), TimerMode::Once),
    ));
    playback.input_locked = true;
}

fn restore_camera(cameras: &mut CameraQuery<'_, '_>) {
    for (_, _, camera, mut transform) in cameras {
        *transform = camera.rest;
    }
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

fn animate_unit_event(event: &BattleEvent, progress: f32, visuals: &mut UnitVisualQuery<'_, '_>) {
    let eased = progress * progress * (3.0 - 2.0 * progress);
    for (visual, mut transform, mut visibility) in visuals.iter_mut() {
        match event {
            BattleEvent::UnitMoved { unit, from, to }
            | BattleEvent::UnitPushed { unit, from, to }
                if *unit == visual.0 =>
            {
                transform.translation = grid_to_world(*from).lerp(grid_to_world(*to), eased);
            }
            BattleEvent::AttackRolled {
                attacker,
                target,
                hit,
                ..
            } => {
                if *attacker == visual.0 {
                    transform.scale = Vec3::splat(attack_scale(progress));
                }
                if *target == visual.0 && *hit {
                    let pulse = (progress * PI).sin();
                    transform.scale = Vec3::splat(UNIT_SCALE * (1.0 + pulse * 0.16));
                }
            }
            BattleEvent::DamageApplied { target, .. } if *target == visual.0 => {
                transform.translation.x += (progress * PI * 6.0).sin() * 0.08;
            }
            BattleEvent::UnitKnockedOut { unit, .. } if *unit == visual.0 => {
                *visibility = Visibility::Visible;
                transform.scale = Vec3::splat(UNIT_SCALE * (1.0 - eased).max(0.02));
            }
            BattleEvent::CounterFired { defender, .. } if *defender == visual.0 => {
                let pulse = (progress * PI).sin();
                transform.scale = Vec3::splat(UNIT_SCALE * (1.0 + pulse * 0.12));
            }
            _ => {}
        }
    }
}

fn animate_effects(progress: f32, effects: &mut EventEffectQuery<'_, '_>) {
    let pulse = (progress * PI).sin();
    for (_, mut transform) in effects.iter_mut() {
        transform.scale = Vec3::splat(0.48 + pulse * 0.28);
        transform.translation.y += 0.012;
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

fn boss_camera_transform(rest: Transform, progress: f32) -> Transform {
    let mut transform = rest;
    let pulse = (progress * PI).sin();
    transform.translation.x += pulse * 0.08;
    transform.translation.z -= pulse * 0.05;
    transform
}

fn is_boss_attack(event: &BattleEvent, battle: &BattleRuntime) -> bool {
    matches!(event, BattleEvent::AttackRolled { attacker, .. }
    if battle.0.unit(*attacker).is_some_and(|unit| {
        matches!(
            unit.archetype,
            UnitArchetype::Dreadnought | UnitArchetype::Regent
        )
    }))
}

fn despawn_transient_effects(
    commands: &mut Commands,
    effects: &mut EventEffectQuery<'_, '_>,
    damage_numbers: &mut DamageNumberQuery<'_, '_>,
) {
    for (entity, _) in &mut *effects {
        commands.entity(entity).despawn();
    }
    for (entity, _, _) in &mut *damage_numbers {
        commands.entity(entity).despawn();
    }
}

fn spawn_damage_number(commands: &mut Commands, hud_root: Entity, viewport: Vec2, amount: i16) {
    commands.spawn((
        Text::new(format!("-{amount}")),
        text_font(28.0),
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: px(viewport.x),
            top: px(viewport.y),
            ..default()
        },
        DamageNumberEffect { origin: viewport },
        Pickable::IGNORE,
        ChildOf(hud_root),
    ));
}

fn spawn_event_effect(
    commands: &mut Commands,
    root: Entity,
    event: &BattleEvent,
    battle: &BattleRuntime,
    mission_assets: &MissionAssets,
) {
    let position = match event {
        BattleEvent::AttackHitEmpty { cell, .. }
        | BattleEvent::ExplosionTriggered { position: cell, .. }
        | BattleEvent::HazardTriggered { position: cell, .. }
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

    commands.spawn((
        Name::new("Combat impact"),
        WorldAssetRoot(mission_assets.scene(9)),
        Transform::from_translation(grid_to_world(position) + Vec3::Y * 0.44)
            .with_scale(Vec3::splat(0.48)),
        Visibility::Visible,
        EventEffect,
        Pickable::IGNORE,
        ChildOf(root),
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
    fn boss_camera_shake_peaks_midway_and_returns_to_rest() {
        let rest = Transform::from_xyz(10.8, 12.4, 12.2).looking_at(Vec3::ZERO, Vec3::Y);
        assert_eq!(boss_camera_transform(rest, 0.0), rest);
        let mid = boss_camera_transform(rest, 0.5);
        assert!(mid.translation.x > rest.translation.x);
        assert!(mid.translation.z < rest.translation.z);
        let end = boss_camera_transform(rest, 1.0);
        assert!(end.translation.distance(rest.translation) < 1e-6);
    }

    #[test]
    fn damage_number_lifecycle_spawns_animates_and_despawns() {
        let mut app = App::new();
        let hud_root = app.world_mut().spawn(HudRoot).id();
        let mut commands = app.world_mut().commands();
        spawn_damage_number(&mut commands, hud_root, Vec2::new(320.0, 240.0), 7);
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
        // 240 - 24 * 0.5 = 228: numerically smaller, visually above the origin.
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
}
