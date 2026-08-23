use crate::domain::{
    battle::BattleState,
    board::GridPos,
    model::{
        BattleError, BattleEvent, BattlePhase, Faction, UnitId, UnitState, WeaponId, WeaponShape,
        WeaponSpec,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DamageSource {
    PlayerWeapon(WeaponId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttackPreview {
    pub attacker: UnitId,
    pub weapon: WeaponId,
    pub target: GridPos,
    pub footprint: Vec<GridPos>,
    pub hit_chance: u8,
    pub normal_damage: i16,
    pub critical_damage: i16,
    pub en_cost: i16,
    pub push_destination: Option<GridPos>,
}

#[derive(Clone)]
struct AttackContext {
    attacker: UnitState,
    weapon: WeaponSpec,
    footprint: Vec<GridPos>,
    target_unit: UnitState,
    push_destination: Option<GridPos>,
}

#[derive(Clone, Copy)]
struct AttackValues {
    hit_chance: u8,
    normal_damage: i16,
    critical_damage: i16,
}

impl BattleState {
    pub fn preview_attack(
        &self,
        attacker: UnitId,
        weapon: WeaponId,
        target: GridPos,
    ) -> Result<AttackPreview, BattleError> {
        let context = self.attack_context(attacker, weapon, target)?;
        let values = attack_values(&context.attacker, &context.weapon, &context.target_unit);

        Ok(AttackPreview {
            attacker,
            weapon,
            target,
            footprint: context.footprint,
            hit_chance: values.hit_chance,
            normal_damage: values.normal_damage,
            critical_damage: values.critical_damage,
            en_cost: context.weapon.en_cost,
            push_destination: context.push_destination,
        })
    }

    pub fn attack(
        &mut self,
        attacker: UnitId,
        weapon: WeaponId,
        target: GridPos,
    ) -> Result<Vec<BattleEvent>, BattleError> {
        let context = self.attack_context(attacker, weapon, target)?;
        let targets: Vec<_> = context
            .footprint
            .iter()
            .filter_map(|position| self.occupant_at(*position))
            .filter(|id| {
                self.unit(*id)
                    .is_some_and(|unit| unit.faction == Faction::Enemy)
            })
            .collect();

        let acting_unit = self
            .unit_mut(attacker)
            .expect("validated attacking unit must exist");
        acting_unit.en -= context.weapon.en_cost;
        acting_unit.activation.acted = true;

        let mut events = Vec::new();
        for target_id in targets {
            let defender = self
                .unit(target_id)
                .expect("target collected from living units")
                .clone();
            let values = attack_values(&context.attacker, &context.weapon, &defender);
            let roll = self.roll_percent();
            let hit = roll <= values.hit_chance;
            let critical_roll = hit.then(|| self.roll_percent());
            let critical = critical_roll.is_some_and(|roll| roll <= context.weapon.crit_chance);

            events.push(BattleEvent::AttackRolled {
                attacker,
                weapon,
                target: target_id,
                roll,
                hit,
                critical_roll,
                critical,
            });

            if !hit {
                continue;
            }

            let damage = if critical {
                values.critical_damage
            } else {
                values.normal_damage
            };
            events.extend(self.apply_damage(
                target_id,
                damage,
                DamageSource::PlayerWeapon(weapon),
            )?);
            if context.weapon.push
                && self
                    .unit(target_id)
                    .is_some_and(|unit| !unit.is_knocked_out())
            {
                events.push(BattleEvent::PushRequested {
                    attacker,
                    target: target_id,
                });
            }
        }

        Ok(events)
    }

    pub(super) fn apply_damage(
        &mut self,
        target: UnitId,
        damage: i16,
        source: DamageSource,
    ) -> Result<Vec<BattleEvent>, BattleError> {
        let unit = self
            .unit_mut(target)
            .ok_or(BattleError::UnknownUnit(target))?;
        if unit.is_knocked_out() || damage <= 0 {
            return Ok(Vec::new());
        }

        let previous_hp = unit.hp;
        unit.hp = (unit.hp - damage).max(0);
        let applied = previous_hp - unit.hp;
        let position = unit.position;
        let remaining_hp = unit.hp;

        let mut events = vec![BattleEvent::DamageApplied {
            target,
            amount: applied,
            remaining_hp,
            source,
        }];
        if remaining_hp == 0 {
            events.push(BattleEvent::UnitKnockedOut {
                unit: target,
                position,
            });
        }
        Ok(events)
    }

    fn attack_context(
        &self,
        attacker: UnitId,
        weapon: WeaponId,
        target: GridPos,
    ) -> Result<AttackContext, BattleError> {
        if self.phase() != BattlePhase::Player {
            return Err(BattleError::WrongPhase {
                expected: BattlePhase::Player,
                actual: self.phase(),
            });
        }

        let attacking_unit = self
            .unit(attacker)
            .ok_or(BattleError::UnknownUnit(attacker))?;
        if attacking_unit.faction != Faction::Player {
            return Err(BattleError::UnitNotPlayer(attacker));
        }
        if attacking_unit.is_knocked_out() {
            return Err(BattleError::UnitKnockedOut(attacker));
        }
        if self.active_unit() != Some(attacker) {
            return Err(BattleError::UnitNotActive(attacker));
        }
        if attacking_unit.activation.acted {
            return Err(BattleError::ActionAlreadySpent(attacker));
        }

        let weapon_spec = self
            .weapon(weapon)
            .ok_or(BattleError::UnknownWeapon(weapon))?;
        if !attacking_unit.weapons.contains(&weapon) {
            return Err(BattleError::WeaponNotOwned {
                unit: attacker,
                weapon,
            });
        }
        if attacking_unit.en < weapon_spec.en_cost {
            return Err(BattleError::InsufficientEn {
                unit: attacker,
                required: weapon_spec.en_cost,
                available: attacking_unit.en,
            });
        }
        if !self.board().contains(target) {
            return Err(BattleError::OutOfBounds(target));
        }

        let distance = attacking_unit.position.manhattan(target);
        if distance < weapon_spec.min_range || distance > weapon_spec.max_range {
            return Err(BattleError::TargetOutOfRange {
                attacker,
                weapon,
                target,
            });
        }
        if weapon_spec.push
            && attacking_unit.position.x != target.x
            && attacking_unit.position.y != target.y
        {
            return Err(BattleError::PushTargetNotAligned {
                attacker: attacking_unit.position,
                target,
            });
        }

        let target_id = self
            .occupant_at(target)
            .ok_or(BattleError::InvalidTarget(target))?;
        let target_unit = self
            .unit(target_id)
            .filter(|unit| unit.faction == Faction::Enemy)
            .ok_or(BattleError::InvalidTarget(target))?;
        let footprint = attack_footprint(self, weapon_spec.shape, target);
        let push_destination = weapon_spec
            .push
            .then(|| push_destination(self, attacking_unit.position, target))
            .flatten();

        Ok(AttackContext {
            attacker: attacking_unit.clone(),
            weapon: weapon_spec.clone(),
            footprint,
            target_unit: target_unit.clone(),
            push_destination,
        })
    }
}

fn attack_values(attacker: &UnitState, weapon: &WeaponSpec, defender: &UnitState) -> AttackValues {
    let hit_chance =
        (attacker.stats.accuracy + weapon.hit_modifier - defender.stats.evasion).clamp(5, 95) as u8;
    let normal_damage = (weapon.base_damage - defender.stats.armor).max(1);
    let critical_raw = weapon.base_damage + weapon.base_damage / 2;
    let critical_damage = (critical_raw - defender.stats.armor).max(1);
    AttackValues {
        hit_chance,
        normal_damage,
        critical_damage,
    }
}

fn attack_footprint(battle: &BattleState, shape: WeaponShape, target: GridPos) -> Vec<GridPos> {
    let mut footprint = vec![target];
    if shape == WeaponShape::Cross1 {
        footprint
            .extend(target.orthogonal_neighbors(battle.board().width(), battle.board().height()));
    }
    footprint
}

fn push_destination(battle: &BattleState, attacker: GridPos, target: GridPos) -> Option<GridPos> {
    let delta_x = (i16::from(target.x) - i16::from(attacker.x)).signum();
    let delta_y = (i16::from(target.y) - i16::from(attacker.y)).signum();
    let x = i16::from(target.x) + delta_x;
    let y = i16::from(target.y) + delta_y;
    let destination = GridPos::new(u8::try_from(x).ok()?, u8::try_from(y).ok()?);
    battle.board().contains(destination).then_some(destination)
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::{
            board::GridPos,
            model::{BattleError, BattleEvent},
        },
        mission::mission_one::{ids, mission_one},
    };

    #[test]
    fn preview_and_resolution_share_inputs_and_charge_en_once() {
        let mut battle = adjacent_vanguard_and_striker(2);
        battle.begin_activation(ids::VANGUARD).unwrap();
        let preview = battle
            .preview_attack(ids::VANGUARD, ids::REPULSOR_RAM, GridPos::new(4, 6))
            .unwrap();
        let en_before = battle.unit(ids::VANGUARD).unwrap().en;
        let events = battle
            .attack(ids::VANGUARD, ids::REPULSOR_RAM, GridPos::new(4, 6))
            .unwrap();

        assert_eq!(preview.hit_chance, 83);
        assert_eq!(preview.normal_damage, 3);
        assert_eq!(battle.unit(ids::VANGUARD).unwrap().en, en_before - 2);
        assert!(events.iter().any(|event| matches!(
            event,
            BattleEvent::AttackRolled {
                roll: 11,
                hit: true,
                ..
            }
        )));
        assert_eq!(
            battle.attack(ids::VANGUARD, ids::PILE_LANCE, GridPos::new(4, 6)),
            Err(BattleError::ActionAlreadySpent(ids::VANGUARD))
        );
    }

    #[test]
    fn seeded_miss_crit_and_knockout_are_deterministic() {
        let mut miss = adjacent_vanguard_and_striker(6);
        miss.begin_activation(ids::VANGUARD).unwrap();
        miss.attack(ids::VANGUARD, ids::PILE_LANCE, GridPos::new(4, 6))
            .unwrap();
        assert_eq!(miss.unit(ids::STRIKER).unwrap().hp, 12);

        let mut crit = low_hp_striker_fixture(0);
        crit.begin_activation(ids::VANGUARD).unwrap();
        crit.attack(ids::VANGUARD, ids::PILE_LANCE, GridPos::new(4, 6))
            .unwrap();
        assert!(crit.unit(ids::STRIKER).unwrap().is_knocked_out());
        assert_eq!(crit.occupant_at(GridPos::new(4, 6)), None);
    }

    #[test]
    fn move_and_action_work_in_either_order_once() {
        let mut action_first = adjacent_vanguard_and_striker(2);
        action_first.begin_activation(ids::VANGUARD).unwrap();
        action_first
            .attack(ids::VANGUARD, ids::REPULSOR_RAM, GridPos::new(4, 6))
            .unwrap();
        action_first
            .move_unit(ids::VANGUARD, GridPos::new(3, 7))
            .unwrap();

        let mut move_first = adjacent_vanguard_and_striker(2);
        move_first.begin_activation(ids::VANGUARD).unwrap();
        move_first
            .move_unit(ids::VANGUARD, GridPos::new(4, 8))
            .unwrap();
        move_first
            .attack(ids::VANGUARD, ids::ANCHOR_CANNON, GridPos::new(4, 6))
            .unwrap();

        assert!(action_first.unit(ids::VANGUARD).unwrap().activation.moved);
        assert!(move_first.unit(ids::VANGUARD).unwrap().activation.acted);
    }

    #[test]
    fn rejected_attack_does_not_spend_action_or_en() {
        let mut battle = adjacent_vanguard_and_striker(2);
        battle.begin_activation(ids::VANGUARD).unwrap();
        let before = battle.unit(ids::VANGUARD).unwrap().clone();

        assert_eq!(
            battle.attack(ids::VANGUARD, ids::RAIL_RIFLE, GridPos::new(4, 6)),
            Err(BattleError::WeaponNotOwned {
                unit: ids::VANGUARD,
                weapon: ids::RAIL_RIFLE,
            })
        );
        assert_eq!(battle.unit(ids::VANGUARD).unwrap(), &before);
    }

    #[test]
    fn cross_footprint_is_unique_and_stably_ordered() {
        let mut battle = mission_one(7);
        battle.enter_player_phase_for_test();
        battle.move_unit_direct_for_test(ids::STRIKER, GridPos::new(4, 5));
        battle.begin_activation(ids::GUNNER).unwrap();

        let preview = battle
            .preview_attack(ids::GUNNER, ids::BURST_MISSILE, GridPos::new(4, 5))
            .unwrap();

        assert_eq!(
            preview.footprint,
            vec![
                GridPos::new(4, 5),
                GridPos::new(4, 4),
                GridPos::new(3, 5),
                GridPos::new(5, 5),
                GridPos::new(4, 6),
            ]
        );
    }

    fn adjacent_vanguard_and_striker(seed: u64) -> crate::domain::battle::BattleState {
        let mut battle = mission_one(seed);
        battle.enter_player_phase_for_test();
        battle.move_unit_direct_for_test(ids::STRIKER, GridPos::new(4, 6));
        battle
    }

    fn low_hp_striker_fixture(seed: u64) -> crate::domain::battle::BattleState {
        let mut battle = adjacent_vanguard_and_striker(seed);
        battle.unit_mut_for_test(ids::STRIKER).unwrap().hp = 10;
        battle
    }
}
