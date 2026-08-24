use std::{f32::consts::PI, time::Duration};

use bevy::prelude::*;

use crate::domain::model::BattleEvent;

use super::{
    BattleEventQueue, BattleRuntime, EventEffect, EventPlayback, PresentationRoot,
    RestartRoundPending, UnitVisual, assets::MissionAssets, grid_to_world,
    interaction::StatusMessage,
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
    mut queue: ResMut<BattleEventQueue>,
    mut playback: ResMut<EventPlayback>,
    mut unit_visuals: UnitVisualQuery,
    mut effects: EventEffectQuery,
) {
    let finished = if let Some((event, timer)) = playback.current.as_mut() {
        timer.tick(time.delta());
        animate_unit_event(event, timer.fraction(), &mut unit_visuals);
        animate_effects(timer.fraction(), &mut effects);
        timer.is_finished()
    } else {
        false
    };

    if finished {
        for (entity, _) in &mut effects {
            commands.entity(entity).despawn();
        }
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
            BattleEvent::AttackRolled { target, hit, .. } if *target == visual.0 && *hit => {
                let pulse = (progress * PI).sin();
                transform.scale = Vec3::splat(UNIT_SCALE * (1.0 + pulse * 0.16));
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
