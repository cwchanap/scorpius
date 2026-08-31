use crate::{campaign::model::SquadUpgrades, domain::battle::BattleState};
use serde::{Deserialize, Serialize};

#[cfg(test)]
use crate::domain::{combat::weapon_reaches, model::Faction};

pub mod enemies;
pub mod mission_one;
pub mod mission_three;
pub mod mission_two;
pub mod squad;

pub type MissionBuilder = fn(u64, &SquadUpgrades) -> BattleState;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum MissionId {
    One,
    Two,
    Three,
    Four,
}

impl std::fmt::Display for MissionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let number = match self {
            MissionId::One => 1,
            MissionId::Two => 2,
            MissionId::Three => 3,
            MissionId::Four => 4,
        };
        write!(f, "{number}")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DialogueLine {
    pub speaker: &'static str,
    pub text: &'static str,
    pub portrait: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DialogueScene {
    pub background: &'static str,
    pub lines: &'static [DialogueLine],
}

#[derive(Clone, Copy, Debug)]
pub struct MissionDefinition {
    pub id: MissionId,
    pub unlocks: MissionId,
    pub build: MissionBuilder,
    pub title: &'static str,
    pub primary_objective: &'static str,
    pub optional_objective: &'static str,
    pub base_reward: u32,
    pub optional_reward: u32,
    pub pre_mission: DialogueScene,
    pub aftermath: DialogueScene,
}

impl PartialEq for MissionDefinition {
    fn eq(&self, other: &Self) -> bool {
        // The builder function pointer is exercised, not compared.
        self.id == other.id
            && self.unlocks == other.unlocks
            && self.title == other.title
            && self.primary_objective == other.primary_objective
            && self.optional_objective == other.optional_objective
            && self.base_reward == other.base_reward
            && self.optional_reward == other.optional_reward
            && self.pre_mission == other.pre_mission
            && self.aftermath == other.aftermath
    }
}

pub fn mission_definition(id: MissionId) -> Option<&'static MissionDefinition> {
    match id {
        MissionId::One => Some(&mission_one::MISSION_ONE_DEFINITION),
        MissionId::Two => Some(&mission_two::MISSION_TWO_DEFINITION),
        MissionId::Three => Some(&mission_three::MISSION_THREE_DEFINITION),
        // Four is the terminal handoff state with no battle content.
        MissionId::Four => None,
    }
}

/// One shared opening-legality assertion for every authored mission. Each
/// mission's own tests still pin the exact opening rows; this helper covers
/// only the generic invariants they used to duplicate.
#[cfg(test)]
pub(crate) fn assert_opening_plan_is_legal(battle: &BattleState) {
    let enemies: Vec<_> = battle
        .units()
        .filter(|unit| unit.faction == Faction::Enemy)
        .map(|unit| unit.id)
        .collect();
    assert_eq!(battle.rules().opening_plan.len(), enemies.len());

    for opening in battle.rules().opening_plan {
        let unit = battle.unit(opening.unit).expect("opening refs a real unit");
        assert_eq!(unit.faction, Faction::Enemy);
        assert!(opening.destination.manhattan(unit.position) <= unit.stats.movement);
        assert!(battle.board().contains(opening.destination));
        assert!(!battle.board().is_blocking(opening.destination));
        assert!(!battle.board().is_hazard(opening.destination));
        assert!(
            battle
                .units()
                .all(|other| { other.id == opening.unit || other.position != opening.destination })
        );

        if let Some(target_id) = opening.target {
            let target = battle.unit(target_id).expect("opening target exists");
            assert_eq!(target.faction, Faction::Player);
            let weapon = unit
                .weapons
                .first()
                .and_then(|weapon| battle.weapon(*weapon))
                .expect("opening unit has first weapon");
            assert!(
                weapon_reaches(weapon, opening.destination, target.position),
                "opening target must be in range and push-aligned"
            );
        }
    }
}
