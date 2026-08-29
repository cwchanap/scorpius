use crate::{campaign::model::SquadUpgrades, domain::battle::BattleState};
use serde::{Deserialize, Serialize};

pub mod enemies;
pub mod mission_one;
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
        // Three is authored by the Mission 3 task; Four is the terminal
        // handoff state with no battle content.
        MissionId::Three | MissionId::Four => None,
    }
}
