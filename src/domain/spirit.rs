//! Spirit Commands: the SRW-style pilot spells. Each player pilot carries an
//! SP pool for the battle and spends it on one closed, typed set of commands.
//! Unlike the once-per-mission pilot skills, any pilot can cast any command
//! as often as its SP allows.

use super::{
    battle::BattleState,
    model::{BattleError, BattleEvent, Faction, UnitId},
};

/// SP every pilot starts a battle with.
pub const SP_START: i16 = 30;
/// The most SP a pilot can hold.
pub const SP_MAX: i16 = 60;
/// SP every living pilot regains at the start of each round after the first.
pub const SP_PER_ROUND: i16 = 5;
/// HP that Guts restores, capped at the mech's max HP.
pub const GUTS_REPAIR: i16 = 12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpiritCommand {
    /// The pilot's next Action attack on an enemy deals double damage.
    Valor,
    /// The next enemy shot that would hit the pilot misses instead.
    Alert,
    /// Restore [`GUTS_REPAIR`] HP to the pilot's own mech now.
    Guts,
}

impl SpiritCommand {
    pub const ALL: [Self; 3] = [Self::Valor, Self::Alert, Self::Guts];

    pub const fn cost(self) -> i16 {
        match self {
            Self::Valor => 30,
            Self::Alert => 15,
            Self::Guts => 25,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Valor => "VALOR",
            Self::Alert => "ALERT",
            Self::Guts => "GUTS",
        }
    }
}

/// One pilot's battle-local spirit state: never serialized, reset by
/// rebuilding the mission's `BattleState`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpiritState {
    pub sp: i16,
    /// Valor waits for the pilot's next committed Action attack on an enemy.
    pub valor: bool,
    /// Alert waits for the next enemy shot that would hit the pilot.
    pub alert: bool,
}

impl Default for SpiritState {
    fn default() -> Self {
        Self {
            sp: SP_START,
            valor: false,
            alert: false,
        }
    }
}

impl BattleState {
    /// The pilot's spirit state, for player units only.
    pub fn spirit(&self, unit: UnitId) -> Option<SpiritState> {
        self.spirits.get(&unit).copied()
    }

    /// Whether the active pilot could cast `command` right now.
    pub fn can_use_spirit(&self, command: SpiritCommand) -> bool {
        self.spirit_check(command).is_ok()
    }

    /// Spend the active pilot's SP on `command`.
    pub fn use_spirit(&mut self, command: SpiritCommand) -> Result<Vec<BattleEvent>, BattleError> {
        let unit = self.spirit_check(command)?;
        let spirit = self
            .spirits
            .get_mut(&unit)
            .expect("spirit_check found the pilot's spirit state");
        spirit.sp -= command.cost();
        match command {
            SpiritCommand::Valor => spirit.valor = true,
            SpiritCommand::Alert => spirit.alert = true,
            SpiritCommand::Guts => {}
        }
        let mut events = vec![BattleEvent::SpiritInvoked { unit, command }];
        if command == SpiritCommand::Guts {
            let mech = self.unit_mut(unit).expect("the active pilot exists");
            let before = mech.hp;
            mech.hp = (mech.hp + GUTS_REPAIR).min(mech.stats.max_hp);
            events.push(BattleEvent::UnitRepaired {
                unit,
                amount: mech.hp - before,
                remaining_hp: mech.hp,
            });
        }
        Ok(events)
    }

    fn spirit_check(&self, command: SpiritCommand) -> Result<UnitId, BattleError> {
        self.require_player_phase()?;
        let id = self.active_unit().ok_or(BattleError::NoUnitSelected)?;
        let unit = self.unit(id).ok_or(BattleError::UnknownUnit(id))?;
        let spirit = self.spirit(id).ok_or(BattleError::UnitNotPlayer(id))?;
        let already = match command {
            SpiritCommand::Valor => spirit.valor,
            SpiritCommand::Alert => spirit.alert,
            SpiritCommand::Guts => unit.hp >= unit.stats.max_hp,
        };
        if already {
            return Err(BattleError::SpiritNoEffect { unit: id, command });
        }
        if spirit.sp < command.cost() {
            return Err(BattleError::InsufficientSp {
                unit: id,
                required: command.cost(),
                available: spirit.sp,
            });
        }
        Ok(id)
    }

    /// Start-of-round SP for every living pilot.
    pub(super) fn regain_round_sp(&mut self) {
        let living: Vec<_> = self
            .units()
            .filter(|unit| unit.faction == Faction::Player && !unit.is_knocked_out())
            .map(|unit| unit.id)
            .collect();
        for id in living {
            if let Some(spirit) = self.spirits.get_mut(&id) {
                spirit.sp = (spirit.sp + SP_PER_ROUND).min(SP_MAX);
            }
        }
    }

    pub(super) fn valor_pending(&self, unit: UnitId) -> bool {
        self.spirit(unit).is_some_and(|spirit| spirit.valor)
    }

    pub(super) fn clear_valor(&mut self, unit: UnitId) {
        if let Some(spirit) = self.spirits.get_mut(&unit) {
            spirit.valor = false;
        }
    }

    /// Spend a waiting Alert, reporting whether there was one.
    pub(super) fn take_alert(&mut self, unit: UnitId) -> bool {
        self.spirits
            .get_mut(&unit)
            .is_some_and(|spirit| std::mem::take(&mut spirit.alert))
    }

    #[cfg(test)]
    pub(crate) fn set_sp_for_test(&mut self, unit: UnitId, sp: i16) {
        self.spirits.get_mut(&unit).expect("a player pilot").sp = sp;
    }
}

#[cfg(test)]
mod tests {
    use super::{GUTS_REPAIR, SP_MAX, SP_PER_ROUND, SP_START, SpiritCommand};
    use crate::{
        domain::{
            board::GridPos,
            model::{BattleError, BattleEvent, Reaction},
        },
        mission::mission_one::{ids, mission_one},
    };

    fn active(unit: crate::domain::model::UnitId) -> crate::domain::battle::BattleState {
        let mut battle = mission_one(7);
        battle.begin_round().unwrap();
        battle.begin_activation(unit).unwrap();
        battle
    }

    #[test]
    fn pilots_start_with_sp_and_enemies_have_none() {
        let battle = mission_one(7);
        for unit in battle.units() {
            let spirit = battle.spirit(unit.id);
            match unit.faction {
                crate::domain::model::Faction::Player => {
                    assert_eq!(spirit.map(|spirit| spirit.sp), Some(SP_START));
                }
                crate::domain::model::Faction::Enemy => assert_eq!(spirit, None),
            }
        }
    }

    #[test]
    fn spirits_need_an_active_pilot_in_the_player_phase() {
        let mut battle = mission_one(7);
        assert!(matches!(
            battle.use_spirit(SpiritCommand::Alert),
            Err(BattleError::WrongPhase { .. })
        ));
        battle.begin_round().unwrap();
        assert_eq!(
            battle.use_spirit(SpiritCommand::Alert),
            Err(BattleError::NoUnitSelected)
        );
    }

    #[test]
    fn a_spirit_spends_its_cost_and_cannot_overdraw() {
        let mut battle = active(ids::VANGUARD);
        assert_eq!(
            battle.use_spirit(SpiritCommand::Alert).unwrap(),
            vec![BattleEvent::SpiritInvoked {
                unit: ids::VANGUARD,
                command: SpiritCommand::Alert,
            }]
        );
        let spirit = battle.spirit(ids::VANGUARD).unwrap();
        assert_eq!(spirit.sp, SP_START - SpiritCommand::Alert.cost());
        assert!(spirit.alert);
        assert_eq!(
            battle.use_spirit(SpiritCommand::Valor),
            Err(BattleError::InsufficientSp {
                unit: ids::VANGUARD,
                required: SpiritCommand::Valor.cost(),
                available: spirit.sp,
            })
        );
        assert!(!battle.can_use_spirit(SpiritCommand::Valor));
        assert_eq!(battle.spirit(ids::VANGUARD), Some(spirit));
    }

    #[test]
    fn a_waiting_spirit_cannot_be_cast_again() {
        let mut battle = active(ids::VANGUARD);
        battle.set_sp_for_test(ids::VANGUARD, SP_MAX);
        battle.use_spirit(SpiritCommand::Valor).unwrap();
        assert_eq!(
            battle.use_spirit(SpiritCommand::Valor),
            Err(BattleError::SpiritNoEffect {
                unit: ids::VANGUARD,
                command: SpiritCommand::Valor,
            })
        );
        assert_eq!(battle.spirit(ids::VANGUARD).unwrap().sp, SP_MAX - 30);
    }

    #[test]
    fn guts_repairs_up_to_max_hp_and_refuses_a_full_mech() {
        let mut battle = active(ids::VANGUARD);
        assert_eq!(
            battle.use_spirit(SpiritCommand::Guts),
            Err(BattleError::SpiritNoEffect {
                unit: ids::VANGUARD,
                command: SpiritCommand::Guts,
            })
        );
        let max_hp = battle.unit(ids::VANGUARD).unwrap().stats.max_hp;
        battle.unit_mut_for_test(ids::VANGUARD).unwrap().hp = max_hp - 5;
        let events = battle.use_spirit(SpiritCommand::Guts).unwrap();
        assert_eq!(
            events[1],
            BattleEvent::UnitRepaired {
                unit: ids::VANGUARD,
                amount: 5,
                remaining_hp: max_hp,
            }
        );
        battle.set_sp_for_test(ids::VANGUARD, SP_MAX);
        battle.unit_mut_for_test(ids::VANGUARD).unwrap().hp = 1;
        battle.use_spirit(SpiritCommand::Guts).unwrap();
        assert_eq!(battle.unit(ids::VANGUARD).unwrap().hp, 1 + GUTS_REPAIR);
    }

    #[test]
    fn valor_doubles_the_preview_and_the_next_attack_only() {
        let mut plain = adjacent_vanguard_and_striker(2);
        plain.begin_activation(ids::VANGUARD).unwrap();
        let base = plain
            .preview_attack(ids::VANGUARD, ids::PILE_LANCE, GridPos::new(4, 6))
            .unwrap();

        let mut battle = adjacent_vanguard_and_striker(2);
        battle.begin_activation(ids::VANGUARD).unwrap();
        battle.use_spirit(SpiritCommand::Valor).unwrap();
        let preview = battle
            .preview_attack(ids::VANGUARD, ids::PILE_LANCE, GridPos::new(4, 6))
            .unwrap();
        assert_eq!(preview.normal_damage, base.normal_damage * 2);
        assert_eq!(preview.critical_damage, base.critical_damage * 2);
        assert_eq!(preview.hit_chance, base.hit_chance);

        let plain_events = plain
            .attack(ids::VANGUARD, ids::PILE_LANCE, GridPos::new(4, 6))
            .unwrap();
        let events = battle
            .attack(ids::VANGUARD, ids::PILE_LANCE, GridPos::new(4, 6))
            .unwrap();
        let dealt = |events: &[BattleEvent]| {
            events.iter().find_map(|event| match event {
                BattleEvent::DamageApplied { amount, .. } => Some(*amount),
                _ => None,
            })
        };
        let plain_hit = dealt(&plain_events).expect("seed 2 lands the Pile Lance");
        let striker_hp = battle.unit(ids::STRIKER).unwrap().stats.max_hp;
        assert_eq!(
            dealt(&events),
            Some((plain_hit * 2).min(striker_hp)),
            "the same seeded roll deals double"
        );
        assert!(!battle.spirit(ids::VANGUARD).unwrap().valor);
    }

    #[test]
    fn alert_dodges_the_next_enemy_hit_against_the_pilot() {
        let rolled = |events: &[BattleEvent], unit| {
            events
                .iter()
                .find_map(|event| match event {
                    BattleEvent::AttackRolled {
                        target, roll, hit, ..
                    } if *target == unit => Some((*roll, *hit)),
                    _ => None,
                })
                .expect("an enemy shoots at the pilot")
        };
        // The first seed whose opening shot lands without Alert.
        let (seed, unit, plain_roll) = (1..100)
            .find_map(|seed| {
                let (mut plain, unit) = targeted_player(seed, SpiritCommand::Guts);
                let events = plain.resolve_enemy_phase().unwrap();
                let (roll, hit) = rolled(&events, unit);
                hit.then_some((seed, unit, roll))
            })
            .expect("some seed lands the opening shot");
        let (mut alert, _) = targeted_player(seed, SpiritCommand::Alert);
        let events = alert.resolve_enemy_phase().unwrap();
        assert_eq!(
            rolled(&events, unit),
            (plain_roll, false),
            "same roll, a miss"
        );
        assert!(!alert.spirit(unit).unwrap().alert, "Alert is spent");
    }

    #[test]
    fn alert_waits_through_a_shot_that_misses_anyway() {
        // An untouchable pilot, and a seed where every shot at it misses.
        let dodgy = |seed, command| {
            let (mut battle, unit) = targeted_player(seed, command);
            battle.unit_mut_for_test(unit).unwrap().stats.evasion = 200;
            (battle, unit)
        };
        let (seed, unit) = (1..100)
            .find_map(|seed| {
                let (mut plain, unit) = dodgy(seed, SpiritCommand::Guts);
                let events = plain.resolve_enemy_phase().unwrap();
                let all_miss = events.iter().all(|event| {
                    !matches!(event, BattleEvent::AttackRolled { target, hit: true, .. }
                        if *target == unit)
                });
                all_miss.then_some((seed, unit))
            })
            .expect("some seed misses the pilot");
        let (mut alert, _) = dodgy(seed, SpiritCommand::Alert);
        alert.resolve_enemy_phase().unwrap();
        assert!(
            alert.spirit(unit).unwrap().alert,
            "a miss leaves Alert waiting"
        );
    }

    #[test]
    fn living_pilots_regain_sp_each_round_up_to_the_cap() {
        let mut battle = mission_one(7);
        battle.begin_round().unwrap();
        assert_eq!(battle.spirit(ids::GUNNER).unwrap().sp, SP_START);
        battle.set_sp_for_test(ids::VANGUARD, SP_MAX - 2);
        battle.unit_mut_for_test(ids::INTERCEPTOR).unwrap().hp = 0;
        finish_all(&mut battle);
        battle.resolve_enemy_phase().unwrap();
        assert_eq!(battle.round(), 2);
        assert_eq!(battle.spirit(ids::VANGUARD).unwrap().sp, SP_MAX);
        assert_eq!(
            battle.spirit(ids::GUNNER).unwrap().sp,
            SP_START + SP_PER_ROUND
        );
        assert_eq!(battle.spirit(ids::INTERCEPTOR).unwrap().sp, SP_START);
    }

    #[test]
    fn spirits_never_touch_the_rng() {
        let mut spent = active(ids::GUNNER);
        let mut idle = active(ids::GUNNER);
        spent.set_sp_for_test(ids::GUNNER, SP_MAX);
        spent.unit_mut_for_test(ids::GUNNER).unwrap().hp -= 1;
        spent.use_spirit(SpiritCommand::Valor).unwrap();
        spent.use_spirit(SpiritCommand::Guts).unwrap();
        spent.use_spirit(SpiritCommand::Alert).unwrap_err();
        assert_eq!(spent.roll_percent(), idle.roll_percent());
    }

    fn finish_all(battle: &mut crate::domain::battle::BattleState) {
        let players: Vec<_> = battle
            .units()
            .filter(|unit| {
                unit.faction == crate::domain::model::Faction::Player
                    && !unit.is_knocked_out()
                    && !unit.activation.finished
            })
            .map(|unit| unit.id)
            .collect();
        for unit in players {
            battle.begin_activation(unit).unwrap();
            battle.choose_reaction(unit, Reaction::Guard).unwrap();
            battle.finish_activation(unit).unwrap();
        }
    }

    /// Mission 1 after the opening, with `command` cast by a pilot an
    /// enemy intent targets, and every pilot finished.
    fn targeted_player(
        seed: u64,
        command: SpiritCommand,
    ) -> (
        crate::domain::battle::BattleState,
        crate::domain::model::UnitId,
    ) {
        let mut battle = mission_one(seed);
        battle.begin_round().unwrap();
        let unit = battle
            .intents()
            .iter()
            .find_map(|intent| intent.intended_occupant)
            .expect("the opening targets a pilot");
        battle.begin_activation(unit).unwrap();
        if command == SpiritCommand::Guts {
            battle.unit_mut_for_test(unit).unwrap().hp -= 1;
        }
        battle.use_spirit(command).unwrap();
        battle.choose_reaction(unit, Reaction::Guard).unwrap();
        battle.finish_activation(unit).unwrap();
        finish_all(&mut battle);
        (battle, unit)
    }

    fn adjacent_vanguard_and_striker(seed: u64) -> crate::domain::battle::BattleState {
        let mut battle = mission_one(seed);
        battle.enter_player_phase_for_test();
        battle.move_unit_direct_for_test(ids::STRIKER, GridPos::new(4, 6));
        battle
    }
}
