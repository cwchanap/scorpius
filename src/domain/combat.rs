use crate::domain::{
    battle::BattleState,
    board::GridPos,
    enemy::AttackProfile,
    model::{
        BattleError, BattleEvent, BattlePhase, Faction, Reaction, UnitArchetype, UnitId, UnitState,
        WeaponId, WeaponShape, WeaponSpec,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DamageSource {
    PlayerWeapon(WeaponId),
    EnemyWeapon(UnitId, WeaponId),
    Collision,
    Hazard,
    Explosion,
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
    target_unit: Option<UnitState>,
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
        let force_hit = self.focus_forces_hit(&context.attacker);
        let values = context
            .target_unit
            .as_ref()
            .map(|target| attack_values(&context.attacker, &context.weapon, target, force_hit))
            .unwrap_or_else(|| prop_attack_values(&context.weapon));

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
        let force_hit = self.focus_forces_hit(&context.attacker);
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
        if force_hit {
            self.clear_focus_pending();
        }

        let mut events = Vec::new();
        for target_id in targets {
            let defender = self
                .unit(target_id)
                .expect("target collected from living units")
                .clone();
            let values = attack_values(&context.attacker, &context.weapon, &defender, force_hit);
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
                events.extend(self.resolve_push(attacker, target_id)?);
            }
        }

        let explosive_positions: Vec<_> = context
            .footprint
            .iter()
            .copied()
            .filter(|position| self.board().has_live_explosive(*position))
            .collect();
        for position in explosive_positions {
            events.extend(self.damage_explosive_raw(
                position,
                context.weapon.base_damage,
                DamageSource::PlayerWeapon(weapon),
            )?);
        }
        events.extend(self.check_terminal_state());

        Ok(events)
    }

    pub(super) fn resolve_enemy_profile_against(
        &mut self,
        attacker: UnitId,
        profile: &AttackProfile,
        target: UnitId,
    ) -> Result<(Vec<BattleEvent>, bool), BattleError> {
        let defender = self
            .unit(target)
            .ok_or(BattleError::UnknownUnit(target))?
            .clone();
        let values = incoming_attack_values(
            profile,
            &defender,
            self.pilot_skills().aegis_target == Some(target),
        );
        let roll = self.roll_percent();
        let hit = roll <= values.hit_chance;
        let critical_roll = hit.then(|| self.roll_percent());
        let critical = critical_roll.is_some_and(|roll| roll <= profile.crit_chance);
        let mut events = vec![BattleEvent::AttackRolled {
            attacker,
            weapon: profile.weapon,
            target,
            roll,
            hit,
            critical_roll,
            critical,
        }];

        if !hit {
            return Ok((events, false));
        }

        let damage = if critical {
            values.critical_damage
        } else {
            values.normal_damage
        };
        events.extend(self.apply_damage(
            target,
            damage,
            DamageSource::EnemyWeapon(attacker, profile.weapon),
        )?);
        if profile.push
            && self
                .unit(attacker)
                .is_some_and(|unit| !unit.is_knocked_out())
            && self.unit(target).is_some_and(|unit| !unit.is_knocked_out())
        {
            events.extend(self.resolve_push(attacker, target)?);
        }

        Ok((events, true))
    }

    pub(super) fn resolve_counter(
        &mut self,
        defender: UnitId,
        attacker: UnitId,
    ) -> Result<Vec<BattleEvent>, BattleError> {
        let countering_unit = self
            .unit(defender)
            .ok_or(BattleError::UnknownUnit(defender))?
            .clone();
        let target = self
            .unit(attacker)
            .ok_or(BattleError::UnknownUnit(attacker))?
            .clone();
        if countering_unit.is_knocked_out()
            || countering_unit.reaction != Some(Reaction::Counter)
            || target.is_knocked_out()
        {
            return Ok(Vec::new());
        }

        let weapon = countering_unit
            .weapons
            .iter()
            .filter_map(|weapon| self.weapon(*weapon))
            .find(|weapon| {
                weapon.counter_weapon
                    && weapon_reaches(weapon, countering_unit.position, target.position)
            })
            .cloned();
        let Some(weapon) = weapon else {
            return Ok(Vec::new());
        };
        if countering_unit.en < weapon.en_cost {
            return Ok(Vec::new());
        }

        self.unit_mut(defender)
            .expect("validated countering unit must exist")
            .en -= weapon.en_cost;
        let values = attack_values(&countering_unit, &weapon, &target, false);
        let roll = self.roll_percent();
        let hit = roll <= values.hit_chance;
        let critical_roll = hit.then(|| self.roll_percent());
        let critical = critical_roll.is_some_and(|roll| roll <= weapon.crit_chance);
        let mut events = vec![
            BattleEvent::CounterFired {
                defender,
                attacker,
                weapon: weapon.id,
            },
            BattleEvent::AttackRolled {
                attacker: defender,
                weapon: weapon.id,
                target: attacker,
                roll,
                hit,
                critical_roll,
                critical,
            },
        ];

        if !hit {
            return Ok(events);
        }
        let damage = if critical {
            values.critical_damage
        } else {
            values.normal_damage
        };
        events.extend(self.apply_damage(
            attacker,
            damage,
            DamageSource::PlayerWeapon(weapon.id),
        )?);
        if weapon.push
            && self
                .unit(attacker)
                .is_some_and(|unit| !unit.is_knocked_out())
        {
            events.extend(self.resolve_push(defender, attacker)?);
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
        let knocked_out = remaining_hp == 0;

        if knocked_out {
            self.clear_active_unit_if(target);
        }

        let mut events = vec![BattleEvent::DamageApplied {
            target,
            amount: applied,
            remaining_hp,
            source,
        }];
        if knocked_out {
            events.push(BattleEvent::UnitKnockedOut {
                unit: target,
                position,
            });
        }
        events.extend(self.observe_damage_for_objectives(target, applied, source));
        Ok(events)
    }

    #[cfg(test)]
    pub(crate) fn apply_direct_damage(
        &mut self,
        target: UnitId,
        damage: i16,
        source: DamageSource,
    ) -> Vec<BattleEvent> {
        let mut events = self
            .apply_damage(target, damage, source)
            .expect("direct test damage must target a known unit");
        events.extend(self.check_terminal_state());
        events
    }

    fn focus_forces_hit(&self, attacker: &UnitState) -> bool {
        self.pilot_skills().focus_pending && attacker.archetype == UnitArchetype::Gunner
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

        let target_unit = self
            .occupant_at(target)
            .and_then(|target_id| self.unit(target_id))
            .filter(|unit| unit.faction == Faction::Enemy)
            .cloned();
        if target_unit.is_none() && !self.board().has_live_explosive(target) {
            return Err(BattleError::InvalidTarget(target));
        }
        let footprint = attack_footprint(self, weapon_spec.shape, target);
        let push_destination = weapon_spec
            .push
            .then(|| push_destination(self, attacking_unit.position, target))
            .flatten();

        Ok(AttackContext {
            attacker: attacking_unit.clone(),
            weapon: weapon_spec.clone(),
            footprint,
            target_unit,
            push_destination,
        })
    }
}

fn attack_values(
    attacker: &UnitState,
    weapon: &WeaponSpec,
    defender: &UnitState,
    force_hit: bool,
) -> AttackValues {
    let hit_chance = if force_hit {
        100
    } else {
        (attacker.stats.accuracy + weapon.hit_modifier - defender.stats.evasion).clamp(5, 95) as u8
    };
    let normal_damage = (weapon.base_damage - defender.stats.armor).max(1);
    let critical_raw = weapon.base_damage + weapon.base_damage / 2;
    let critical_damage = (critical_raw - defender.stats.armor).max(1);
    AttackValues {
        hit_chance,
        normal_damage,
        critical_damage,
    }
}

fn incoming_attack_values(
    profile: &AttackProfile,
    defender: &UnitState,
    aegis_guarded: bool,
) -> AttackValues {
    let evasion_bonus = if defender.reaction == Some(Reaction::Evade) {
        25
    } else {
        0
    };
    let hit_chance =
        (profile.accuracy + profile.hit_modifier - defender.stats.evasion - evasion_bonus)
            .clamp(5, 95) as u8;
    let guarded = defender.reaction == Some(Reaction::Guard) || aegis_guarded;
    let guard_reduction = if guarded { 3 } else { 0 };
    let normal_damage =
        ((profile.base_damage - defender.stats.armor).max(1) - guard_reduction).max(0);
    let critical_raw = profile.base_damage + profile.base_damage / 2;
    let critical_damage = ((critical_raw - defender.stats.armor).max(1) - guard_reduction).max(0);
    AttackValues {
        hit_chance,
        normal_damage,
        critical_damage,
    }
}

fn prop_attack_values(weapon: &WeaponSpec) -> AttackValues {
    AttackValues {
        hit_chance: 100,
        normal_damage: weapon.base_damage,
        critical_damage: weapon.base_damage,
    }
}

pub(super) fn preview_for_profile(
    attacker: UnitId,
    profile: &AttackProfile,
    target: GridPos,
    footprint: Vec<GridPos>,
    defender: &UnitState,
    aegis_guarded: bool,
) -> AttackPreview {
    let values = incoming_attack_values(profile, defender, aegis_guarded);
    AttackPreview {
        attacker,
        weapon: profile.weapon,
        target,
        footprint,
        hit_chance: values.hit_chance,
        normal_damage: values.normal_damage,
        critical_damage: values.critical_damage,
        en_cost: 0,
        push_destination: None,
    }
}

pub(super) fn attack_footprint(
    battle: &BattleState,
    shape: WeaponShape,
    target: GridPos,
) -> Vec<GridPos> {
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

fn weapon_reaches(weapon: &WeaponSpec, attacker: GridPos, target: GridPos) -> bool {
    let distance = attacker.manhattan(target);
    distance >= weapon.min_range
        && distance <= weapon.max_range
        && (!weapon.push || attacker.x == target.x || attacker.y == target.y)
}

#[cfg(test)]
mod tests {
    use super::{AttackPreview, incoming_attack_values, preview_for_profile};
    use crate::{
        domain::{
            board::GridPos,
            model::{BattleError, BattleEvent, Reaction, UnitId},
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

    #[test]
    fn guard_reduces_post_armor_damage_and_evade_changes_hit_chance() {
        let guard = incoming_preview_fixture(Some(Reaction::Guard));
        assert_eq!(guard.normal_damage, 1);

        let evade = incoming_preview_fixture(Some(Reaction::Evade));
        let none = incoming_preview_fixture(None);
        assert_eq!(none.hit_chance.saturating_sub(evade.hit_chance), 25);
    }

    #[test]
    fn counter_uses_authored_weapon_and_en_without_recursion() {
        let mut battle = counter_fixture(2);
        let en_before = battle.unit(ids::INTERCEPTOR).unwrap().en;
        let events = battle.resolve_intent_for_test(ids::RIFLEMAN_RIGHT).unwrap();

        assert_eq!(battle.unit(ids::INTERCEPTOR).unwrap().en, en_before - 1);
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, BattleEvent::CounterFired { .. }))
                .count(),
            1
        );
        assert!(!events.iter().any(|event| matches!(
            event,
            BattleEvent::CounterFired { defender, .. }
                if *defender == ids::RIFLEMAN_RIGHT
        )));
    }

    #[test]
    fn counter_requires_current_range_and_sufficient_en() {
        let mut out_of_range = counter_fixture(2);
        let en_before = out_of_range.unit(ids::INTERCEPTOR).unwrap().en;
        out_of_range.move_unit_direct_for_test(ids::RIFLEMAN_RIGHT, GridPos::new(0, 0));
        let range_events = out_of_range
            .resolve_intent_for_test(ids::RIFLEMAN_RIGHT)
            .unwrap();
        assert_eq!(out_of_range.unit(ids::INTERCEPTOR).unwrap().en, en_before);
        assert!(
            !range_events
                .iter()
                .any(|event| matches!(event, BattleEvent::CounterFired { .. }))
        );

        let mut no_en = counter_fixture(2);
        no_en.unit_mut_for_test(ids::INTERCEPTOR).unwrap().en = 0;
        let en_events = no_en.resolve_intent_for_test(ids::RIFLEMAN_RIGHT).unwrap();
        assert!(
            !en_events
                .iter()
                .any(|event| matches!(event, BattleEvent::CounterFired { .. }))
        );
    }

    #[test]
    fn aegis_shields_the_gunner_once_on_the_public_path() {
        // Deterministic sweep first; only fall back to a zero-evasion fixture
        // with the next fixed seed if no seed in 0..64 lands the control hit.
        let (seed, zero_evasion) = match aegis_control_hit_seed() {
            Some(seed) => (seed, false),
            None => (64, true),
        };

        let control = resolve_rifleman_left_against_gunner(seed, zero_evasion, false);
        let aegis = resolve_rifleman_left_against_gunner(seed, zero_evasion, true);

        assert_eq!(
            damage_applied_to(&control, ids::GUNNER),
            Some(4),
            "control hit must be Service Rifle 5 - Gunner armor 1 (seed {seed}, zero_evasion {zero_evasion})"
        );
        assert_eq!(
            damage_applied_to(&aegis, ids::GUNNER),
            Some(1),
            "Aegis applies one 3-point reduction (seed {seed}, zero_evasion {zero_evasion})"
        );
    }

    #[test]
    fn guard_and_aegis_share_one_reduction() {
        let mut battle = mission_one(7);
        battle.begin_round().unwrap();
        let intent = battle.intent_for(ids::RIFLEMAN_LEFT).unwrap().clone();
        let mut gunner = battle.unit(ids::GUNNER).unwrap().clone();
        gunner.reaction = Some(Reaction::Guard);

        let guard_only = incoming_attack_values(&intent.profile, &gunner, false);
        let guard_and_aegis = incoming_attack_values(&intent.profile, &gunner, true);

        assert_eq!(guard_only.normal_damage, guard_and_aegis.normal_damage);
        assert_eq!(guard_only.critical_damage, guard_and_aegis.critical_damage);
    }

    #[test]
    fn focused_gunner_action_hits_and_consumes_pending_only_on_commit() {
        let mut battle = mission_one(7);
        battle.enter_player_phase_for_test();
        battle.begin_activation(ids::GUNNER).unwrap();
        battle.use_focus().unwrap();

        let preview = battle
            .preview_attack(ids::GUNNER, ids::RAIL_RIFLE, GridPos::new(2, 3))
            .unwrap();
        assert_eq!(preview.hit_chance, 100);

        assert_eq!(
            battle.attack(ids::GUNNER, ids::RAIL_RIFLE, GridPos::new(3, 6)),
            Err(BattleError::TargetOutOfRange {
                attacker: ids::GUNNER,
                weapon: ids::RAIL_RIFLE,
                target: GridPos::new(3, 6),
            })
        );
        assert!(
            battle.pilot_skills().focus_pending,
            "a rejected attack must not consume Focus"
        );

        let events = battle
            .attack(ids::GUNNER, ids::RAIL_RIFLE, GridPos::new(2, 3))
            .unwrap();
        assert!(events.iter().any(|event| matches!(
            event,
            BattleEvent::AttackRolled {
                attacker: ids::GUNNER,
                hit: true,
                ..
            }
        )));
        assert!(!battle.pilot_skills().focus_pending);
        assert!(battle.pilot_skills().focus_used);
    }

    #[test]
    fn counter_neither_receives_nor_consumes_focus() {
        let (seed, zero_evasion) = match aegis_control_hit_seed() {
            Some(seed) => (seed, false),
            None => (64, true),
        };

        let (_, plain_events) = gunner_counters_rifleman_left(seed, zero_evasion, false);
        let (focused, focused_events) = gunner_counters_rifleman_left(seed, zero_evasion, true);

        let plain_roll = gunner_counter_roll(&plain_events).expect("counter must fire");
        let focused_roll = gunner_counter_roll(&focused_events).expect("counter must fire");
        assert_eq!(plain_roll, focused_roll);
        assert!(focused.pilot_skills().focus_pending);
    }

    fn damage_applied_to(events: &[BattleEvent], target: UnitId) -> Option<i16> {
        events.iter().find_map(|event| match event {
            BattleEvent::DamageApplied {
                target: damaged,
                amount,
                ..
            } if *damaged == target => Some(*amount),
            _ => None,
        })
    }

    fn gunner_counter_roll(events: &[BattleEvent]) -> Option<(u8, bool, Option<u8>, bool)> {
        events.iter().find_map(|event| match event {
            BattleEvent::AttackRolled {
                attacker,
                roll,
                hit,
                critical_roll,
                critical,
                ..
            } if *attacker == ids::GUNNER => Some((*roll, *hit, *critical_roll, *critical)),
            _ => None,
        })
    }

    fn aegis_control_hit_seed() -> Option<u64> {
        (0..64).find(|&seed| {
            let mut battle = mission_one(seed);
            battle.begin_round().unwrap();
            let events = battle.resolve_intent_for_test(ids::RIFLEMAN_LEFT).unwrap();
            damage_applied_to(&events, ids::GUNNER) == Some(4)
        })
    }

    fn resolve_rifleman_left_against_gunner(
        seed: u64,
        zero_evasion: bool,
        aegis: bool,
    ) -> Vec<BattleEvent> {
        let mut battle = mission_one(seed);
        if zero_evasion {
            battle.unit_mut_for_test(ids::GUNNER).unwrap().stats.evasion = 0;
        }
        battle.begin_round().unwrap();
        if aegis {
            battle.begin_activation(ids::VANGUARD).unwrap();
            battle.move_unit(ids::VANGUARD, GridPos::new(4, 8)).unwrap();
            battle.use_aegis(ids::GUNNER).unwrap();
        }
        battle.resolve_intent_for_test(ids::RIFLEMAN_LEFT).unwrap()
    }

    fn gunner_counters_rifleman_left(
        seed: u64,
        zero_evasion: bool,
        focus: bool,
    ) -> (crate::domain::battle::BattleState, Vec<BattleEvent>) {
        let mut battle = mission_one(seed);
        if zero_evasion {
            battle.unit_mut_for_test(ids::GUNNER).unwrap().stats.evasion = 0;
        }
        battle.begin_round().unwrap();
        battle.begin_activation(ids::GUNNER).unwrap();
        if focus {
            battle.use_focus().unwrap();
        }
        battle
            .choose_reaction(ids::GUNNER, Reaction::Counter)
            .unwrap();
        let events = battle.resolve_intent_for_test(ids::RIFLEMAN_LEFT).unwrap();
        (battle, events)
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

    fn incoming_preview_fixture(reaction: Option<Reaction>) -> AttackPreview {
        let mut battle = mission_one(7);
        battle.begin_round().unwrap();
        battle.unit_mut_for_test(ids::VANGUARD).unwrap().reaction = reaction;

        let intent = battle.intent_for(ids::STRIKER).unwrap();
        let defender = battle.unit(ids::VANGUARD).unwrap();
        preview_for_profile(
            intent.attacker,
            &intent.profile,
            defender.position,
            intent.footprint.clone(),
            defender,
            false,
        )
    }

    fn counter_fixture(seed: u64) -> crate::domain::battle::BattleState {
        let mut battle = mission_one(seed);
        battle.begin_round().unwrap();
        battle.unit_mut_for_test(ids::INTERCEPTOR).unwrap().reaction = Some(Reaction::Counter);
        battle
    }
}
