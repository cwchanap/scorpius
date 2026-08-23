use crate::domain::{
    battle::BattleState,
    board::GridPos,
    combat::DamageSource,
    model::{BattleError, BattleEvent, UnitId},
};

impl BattleState {
    pub fn resolve_push(
        &mut self,
        attacker: UnitId,
        target: UnitId,
    ) -> Result<Vec<BattleEvent>, BattleError> {
        let attacker_state = self
            .unit(attacker)
            .ok_or(BattleError::UnknownUnit(attacker))?;
        if attacker_state.is_knocked_out() {
            return Err(BattleError::UnitKnockedOut(attacker));
        }
        let target_state = self.unit(target).ok_or(BattleError::UnknownUnit(target))?;
        if target_state.is_knocked_out() {
            return Err(BattleError::UnitKnockedOut(target));
        }

        let attacker_position = attacker_state.position;
        let target_position = target_state.position;
        if attacker_position == target_position
            || (attacker_position.x != target_position.x
                && attacker_position.y != target_position.y)
        {
            return Err(BattleError::PushTargetNotAligned {
                attacker: attacker_position,
                target: target_position,
            });
        }

        let destination = displacement_destination(attacker_position, target_position)
            .filter(|position| self.board().contains(*position));
        let Some(destination) = destination.filter(|position| self.is_open_for(target, *position))
        else {
            let blocked_at = destination.unwrap_or(target_position);
            let mut events = vec![BattleEvent::CollisionOccurred {
                unit: target,
                blocked_at,
            }];
            events.extend(self.apply_damage(target, 3, DamageSource::Collision)?);
            events.extend(self.check_terminal_state());
            return Ok(events);
        };

        self.unit_mut(target)
            .expect("validated push target must exist")
            .position = destination;
        let mut events = vec![BattleEvent::UnitPushed {
            unit: target,
            from: target_position,
            to: destination,
        }];
        events.extend(self.resolve_hazard_if_present(target)?);
        events.extend(self.check_terminal_state());
        Ok(events)
    }

    pub fn damage_explosive(
        &mut self,
        position: GridPos,
        damage: i16,
        source: DamageSource,
    ) -> Result<Vec<BattleEvent>, BattleError> {
        let mut events = self.damage_explosive_raw(position, damage, source)?;
        events.extend(self.check_terminal_state());
        Ok(events)
    }

    pub(super) fn damage_explosive_raw(
        &mut self,
        position: GridPos,
        damage: i16,
        source: DamageSource,
    ) -> Result<Vec<BattleEvent>, BattleError> {
        if damage <= 0 {
            return Ok(Vec::new());
        }

        let (applied, remaining_hp, exploded_now) = {
            let explosive = self
                .board_mut()
                .explosive_at_mut(position)
                .ok_or(BattleError::ExplosiveNotFound(position))?;
            if explosive.exploded {
                return Ok(Vec::new());
            }
            let previous_hp = explosive.hp;
            explosive.hp = (explosive.hp - damage).max(0);
            let exploded_now = explosive.hp == 0;
            if exploded_now {
                explosive.exploded = true;
            }
            (previous_hp - explosive.hp, explosive.hp, exploded_now)
        };

        let mut events = vec![BattleEvent::ExplosiveDamaged {
            position,
            amount: applied,
            remaining_hp,
            source,
        }];
        if exploded_now {
            events.extend(self.resolve_explosion(position)?);
        }
        Ok(events)
    }

    pub(super) fn resolve_hazard_if_present(
        &mut self,
        unit: UnitId,
    ) -> Result<Vec<BattleEvent>, BattleError> {
        let state = self.unit(unit).ok_or(BattleError::UnknownUnit(unit))?;
        if state.is_knocked_out() || !self.board().is_hazard(state.position) {
            return Ok(Vec::new());
        }
        let position = state.position;
        let mut events = vec![BattleEvent::HazardTriggered { unit, position }];
        events.extend(self.apply_damage(unit, 3, DamageSource::Hazard)?);
        events.extend(self.check_terminal_state());
        Ok(events)
    }

    fn resolve_explosion(&mut self, position: GridPos) -> Result<Vec<BattleEvent>, BattleError> {
        let mut footprint = vec![position];
        footprint
            .extend(position.orthogonal_neighbors(self.board().width(), self.board().height()));
        let targets: Vec<_> = footprint
            .iter()
            .filter_map(|cell| self.occupant_at(*cell))
            .collect();

        let mut events = vec![BattleEvent::ExplosionTriggered {
            position,
            footprint,
        }];
        for target in targets {
            events.extend(self.apply_damage(target, 4, DamageSource::Explosion)?);
        }
        Ok(events)
    }
}

fn displacement_destination(attacker: GridPos, target: GridPos) -> Option<GridPos> {
    let delta_x = (i16::from(target.x) - i16::from(attacker.x)).signum();
    let delta_y = (i16::from(target.y) - i16::from(attacker.y)).signum();
    let x = i16::from(target.x) + delta_x;
    let y = i16::from(target.y) + delta_y;
    Some(GridPos::new(u8::try_from(x).ok()?, u8::try_from(y).ok()?))
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::{board::GridPos, combat::DamageSource, model::BattleEvent},
        mission::mission_one::{ids, mission_one},
    };

    #[test]
    fn push_moves_once_then_hazard_damages_once() {
        let mut battle = hazard_push_fixture();
        let events = battle
            .resolve_push(ids::INTERCEPTOR, ids::RIFLEMAN_LEFT)
            .unwrap();

        assert_eq!(
            battle.unit(ids::RIFLEMAN_LEFT).unwrap().position,
            GridPos::new(2, 6)
        );
        assert_eq!(battle.unit(ids::RIFLEMAN_LEFT).unwrap().hp, 6);
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, BattleEvent::HazardTriggered { .. }))
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event,
                    BattleEvent::DamageApplied {
                        source: DamageSource::Hazard,
                        ..
                    }
                ))
                .count(),
            1
        );
    }

    #[test]
    fn blocked_push_deals_collision_only_to_pushed_unit() {
        let mut battle = collision_fixture();
        battle.resolve_push(ids::VANGUARD, ids::STRIKER).unwrap();

        assert_eq!(
            battle.unit(ids::STRIKER).unwrap().position,
            GridPos::new(3, 5)
        );
        assert_eq!(battle.unit(ids::STRIKER).unwrap().hp, 9);
        assert_eq!(battle.unit(ids::RIFLEMAN_LEFT).unwrap().hp, 9);
    }

    #[test]
    fn explosive_applies_one_cross_event_and_cannot_repeat() {
        let mut battle = explosive_fixture();
        let first = battle
            .damage_explosive(
                GridPos::new(6, 6),
                4,
                DamageSource::PlayerWeapon(ids::RAIL_RIFLE),
            )
            .unwrap();
        let second = battle
            .damage_explosive(
                GridPos::new(6, 6),
                4,
                DamageSource::PlayerWeapon(ids::RAIL_RIFLE),
            )
            .unwrap();

        assert_eq!(
            first
                .iter()
                .filter(|event| matches!(event, BattleEvent::ExplosionTriggered { .. }))
                .count(),
            1
        );
        assert!(second.is_empty());
        assert_eq!(battle.unit(ids::RIFLEMAN_RIGHT).unwrap().hp, 5);
    }

    #[test]
    fn ordinary_movement_resolves_hazard_once() {
        let mut battle = mission_one(7);
        battle.enter_player_phase_for_test();
        battle.move_unit_direct_for_test(ids::INTERCEPTOR, GridPos::new(2, 7));
        battle.begin_activation(ids::INTERCEPTOR).unwrap();

        let events = battle
            .move_unit(ids::INTERCEPTOR, GridPos::new(2, 6))
            .unwrap();

        assert_eq!(battle.unit(ids::INTERCEPTOR).unwrap().hp, 12);
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, BattleEvent::HazardTriggered { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn player_weapon_automatically_hits_and_explodes_prop() {
        let mut battle = explosive_fixture();
        battle.enter_player_phase_for_test();
        battle.begin_activation(ids::GUNNER).unwrap();

        let events = battle
            .attack(ids::GUNNER, ids::RAIL_RIFLE, GridPos::new(6, 6))
            .unwrap();

        let explosive = battle.board().explosive_at(GridPos::new(6, 6)).unwrap();
        assert!(explosive.exploded);
        assert_eq!(explosive.hp, 0);
        assert_eq!(battle.unit(ids::RIFLEMAN_RIGHT).unwrap().hp, 5);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, BattleEvent::ExplosionTriggered { .. }))
        );
    }

    fn hazard_push_fixture() -> crate::domain::battle::BattleState {
        let mut battle = mission_one(7);
        battle.move_unit_direct_for_test(ids::INTERCEPTOR, GridPos::new(2, 4));
        battle.move_unit_direct_for_test(ids::RIFLEMAN_LEFT, GridPos::new(2, 5));
        battle
    }

    fn collision_fixture() -> crate::domain::battle::BattleState {
        let mut battle = mission_one(7);
        battle.move_unit_direct_for_test(ids::VANGUARD, GridPos::new(4, 5));
        battle.move_unit_direct_for_test(ids::STRIKER, GridPos::new(3, 5));
        battle.move_unit_direct_for_test(ids::RIFLEMAN_LEFT, GridPos::new(2, 5));
        battle
    }

    fn explosive_fixture() -> crate::domain::battle::BattleState {
        let mut battle = mission_one(7);
        battle.move_unit_direct_for_test(ids::RIFLEMAN_RIGHT, GridPos::new(6, 5));
        battle
    }
}
