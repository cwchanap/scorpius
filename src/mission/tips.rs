//! Control's field tips: short in-battle transmissions that teach each Act 1
//! mechanic the first time it matters. Missions author them as typed data
//! next to their dialogue; presentation decides when one shows.

use crate::domain::{
    battle::BattleState,
    model::{UnitId, WeaponId},
};

/// The longest tip text the battle's comms band fits on three lines.
pub const TIP_TEXT_MAX_CHARS: usize = 200;

/// One short Control transmission about a single mechanic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FieldTip {
    pub trigger: TipTrigger,
    pub title: &'static str,
    pub text: &'static str,
}

/// When a tip first shows: the first idle Player-phase moment of a battle
/// visit at which its condition holds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TipTrigger {
    /// The battle's first Player phase.
    Opening,
    /// Any mech is the active unit.
    AnyActivation,
    /// This mech is the active unit.
    Activation(UnitId),
    /// Any mech is the active unit in this round or a later one.
    ActivationFromRound(u16),
    /// The active mech stands in a committed enemy footprint.
    ActiveUnitTargeted,
    /// The active mech stands in this enemy's committed footprint.
    ActiveUnitTargetedBy(UnitId),
    /// The active mech has armed a push weapon.
    PushArmed,
    /// Every surviving mech has finished, so RESOLVE is ready.
    ReadyToResolve,
    /// This enemy is on the board, so a reinforcement carrying it has arrived.
    Deployed(UnitId),
    /// This unit is alive at or below half its max HP, the bosses' weapon
    /// threshold.
    AtHalfHp(UnitId),
}

impl TipTrigger {
    /// Whether the condition holds in `battle`, with `armed` the weapon the
    /// player is targeting with, if any.
    pub fn holds(self, battle: &BattleState, armed: Option<WeaponId>) -> bool {
        let active = || battle.active_unit().and_then(|id| battle.unit(id));
        let active_targeted_by = |attacker: Option<UnitId>| {
            active().is_some_and(|unit| {
                battle.intents().iter().any(|intent| {
                    attacker.is_none_or(|attacker| intent.attacker == attacker)
                        && battle
                            .unit(intent.attacker)
                            .is_some_and(|enemy| !enemy.is_knocked_out())
                        && intent.footprint.contains(&unit.position)
                })
            })
        };
        match self {
            Self::Opening => true,
            Self::AnyActivation => active().is_some(),
            Self::Activation(unit) => battle.active_unit() == Some(unit),
            Self::ActivationFromRound(round) => active().is_some() && battle.round() >= round,
            Self::ActiveUnitTargeted => active_targeted_by(None),
            Self::ActiveUnitTargetedBy(attacker) => active_targeted_by(Some(attacker)),
            Self::PushArmed => armed
                .and_then(|weapon| battle.weapon(weapon))
                .is_some_and(|weapon| weapon.push),
            Self::ReadyToResolve => battle.ready_to_resolve(),
            Self::Deployed(unit) => battle.unit(unit).is_some(),
            Self::AtHalfHp(unit) => battle
                .unit(unit)
                .is_some_and(|unit| !unit.is_knocked_out() && unit.hp * 2 <= unit.stats.max_hp),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::combat::DamageSource;
    use crate::domain::model::{Faction, Reaction};
    use crate::mission::{
        MissionId, mission_definition,
        mission_one::{ids, mission_one},
        mission_six::{self, mission_six},
    };

    const MISSIONS: [MissionId; 7] = [
        MissionId::One,
        MissionId::Two,
        MissionId::Three,
        MissionId::Four,
        MissionId::Five,
        MissionId::Six,
        MissionId::Seven,
    ];

    fn opened_mission_one() -> BattleState {
        let mut battle = mission_one(7);
        battle.begin_round().unwrap();
        battle
    }

    #[test]
    fn every_mission_opens_with_a_tip_that_fits_the_band() {
        for id in MISSIONS {
            let definition = mission_definition(id).unwrap();
            assert_eq!(
                definition.tips.first().map(|tip| tip.trigger),
                Some(TipTrigger::Opening),
                "Mission {id} must open with a tip"
            );
            for tip in definition.tips {
                assert!(!tip.title.is_empty() && tip.title == tip.title.to_uppercase());
                assert!(
                    tip.text.chars().count() <= TIP_TEXT_MAX_CHARS,
                    "Mission {id} tip {:?} is {} chars",
                    tip.title,
                    tip.text.chars().count()
                );
            }
        }
    }

    #[test]
    fn every_authored_tip_names_units_of_its_own_mission() {
        for id in MISSIONS {
            let definition = mission_definition(id).unwrap();
            let battle = (definition.build)(7, &Default::default());
            for tip in definition.tips {
                let unit = match tip.trigger {
                    TipTrigger::Activation(unit)
                    | TipTrigger::ActiveUnitTargetedBy(unit)
                    | TipTrigger::Deployed(unit)
                    | TipTrigger::AtHalfHp(unit) => unit,
                    _ => continue,
                };
                assert!(
                    battle.unit(unit).is_some()
                        || battle
                            .pending_reinforcements()
                            .any(|pending| pending.id == unit),
                    "Mission {id} tip {:?} names a unit the mission never fields",
                    tip.title
                );
            }
        }
    }

    #[test]
    fn activation_triggers_follow_the_active_unit() {
        let mut battle = opened_mission_one();
        assert!(TipTrigger::Opening.holds(&battle, None));
        assert!(!TipTrigger::AnyActivation.holds(&battle, None));
        assert!(!TipTrigger::Activation(ids::VANGUARD).holds(&battle, None));

        battle.begin_activation(ids::VANGUARD).unwrap();

        assert!(TipTrigger::AnyActivation.holds(&battle, None));
        assert!(TipTrigger::Activation(ids::VANGUARD).holds(&battle, None));
        assert!(!TipTrigger::Activation(ids::GUNNER).holds(&battle, None));
        assert!(TipTrigger::ActivationFromRound(1).holds(&battle, None));
        assert!(!TipTrigger::ActivationFromRound(2).holds(&battle, None));
        battle.set_round_for_test(2);
        assert!(TipTrigger::ActivationFromRound(2).holds(&battle, None));
    }

    #[test]
    fn targeted_triggers_read_the_committed_footprints() {
        let battle = opened_mission_one();
        let (attacker, victim) = battle
            .intents()
            .iter()
            .find_map(|intent| {
                intent
                    .footprint
                    .iter()
                    .find_map(|cell| battle.occupant_at(*cell))
                    .filter(|unit| battle.unit(*unit).unwrap().faction == Faction::Player)
                    .map(|unit| (intent.attacker, unit))
            })
            .expect("Mission 1 opens with a mech in a locked footprint");
        let other_attacker = battle
            .intents()
            .iter()
            .map(|intent| intent.attacker)
            .find(|id| {
                battle.intent_for(*id).is_some_and(|intent| {
                    !intent
                        .footprint
                        .contains(&battle.unit(victim).unwrap().position)
                })
            })
            .expect("another enemy aims elsewhere");

        let mut targeted = battle.clone();
        targeted.begin_activation(victim).unwrap();
        assert!(TipTrigger::ActiveUnitTargeted.holds(&targeted, None));
        assert!(TipTrigger::ActiveUnitTargetedBy(attacker).holds(&targeted, None));
        assert!(!TipTrigger::ActiveUnitTargetedBy(other_attacker).holds(&targeted, None));

        // A destroyed attacker's footprint no longer threatens anyone.
        targeted.apply_direct_damage(attacker, 99, DamageSource::PlayerWeapon(ids::PILE_LANCE));
        assert!(!TipTrigger::ActiveUnitTargetedBy(attacker).holds(&targeted, None));
    }

    #[test]
    fn push_trigger_reads_the_armed_weapon() {
        let battle = opened_mission_one();
        assert!(TipTrigger::PushArmed.holds(&battle, Some(ids::REPULSOR_RAM)));
        assert!(!TipTrigger::PushArmed.holds(&battle, Some(ids::PILE_LANCE)));
        assert!(!TipTrigger::PushArmed.holds(&battle, None));
    }

    #[test]
    fn ready_trigger_waits_for_every_mech() {
        let mut battle = opened_mission_one();
        for mech in [ids::VANGUARD, ids::GUNNER] {
            battle.begin_activation(mech).unwrap();
            battle.choose_reaction(mech, Reaction::Guard).unwrap();
            battle.finish_activation(mech).unwrap();
            assert!(!TipTrigger::ReadyToResolve.holds(&battle, None));
        }
        battle.begin_activation(ids::INTERCEPTOR).unwrap();
        battle
            .choose_reaction(ids::INTERCEPTOR, Reaction::Guard)
            .unwrap();
        battle.finish_activation(ids::INTERCEPTOR).unwrap();
        assert!(TipTrigger::ReadyToResolve.holds(&battle, None));
    }

    #[test]
    fn deployed_trigger_waits_for_the_reinforcement() {
        let battle = opened_mission_one();
        assert!(TipTrigger::Deployed(ids::STRIKER).holds(&battle, None));
        assert!(!TipTrigger::Deployed(ids::ROAD_RIFLEMAN_NORTH).holds(&battle, None));
    }

    #[test]
    fn half_hp_trigger_matches_the_boss_threshold() {
        let mut battle = mission_six(7);
        let boss = mission_six::ids::DREADNOUGHT;
        let max_hp = battle.unit(boss).unwrap().stats.max_hp;
        let source = DamageSource::PlayerWeapon(ids::PILE_LANCE);

        battle.apply_direct_damage(boss, max_hp - max_hp / 2 - 1, source);
        assert!(!TipTrigger::AtHalfHp(boss).holds(&battle, None));
        battle.apply_direct_damage(boss, 1, source);
        assert!(TipTrigger::AtHalfHp(boss).holds(&battle, None));
        battle.apply_direct_damage(boss, max_hp, source);
        assert!(!TipTrigger::AtHalfHp(boss).holds(&battle, None));
    }
}
