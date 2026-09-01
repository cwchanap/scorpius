use crate::{campaign::model::SquadUpgrades, domain::battle::BattleState};
use serde::{Deserialize, Serialize};

#[cfg(test)]
use crate::domain::{
    board::GridPos,
    combat::weapon_reaches,
    model::{Faction, UnitId},
};

#[cfg(test)]
use std::collections::HashSet;

pub mod enemies;
pub mod mission_five;
pub mod mission_four;
pub mod mission_one;
pub mod mission_six;
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
    Five,
    Six,
    Seven,
}

impl std::fmt::Display for MissionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let number = match self {
            MissionId::One => 1,
            MissionId::Two => 2,
            MissionId::Three => 3,
            MissionId::Four => 4,
            MissionId::Five => 5,
            MissionId::Six => 6,
            MissionId::Seven => 7,
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
        MissionId::Four => Some(&mission_four::MISSION_FOUR_DEFINITION),
        MissionId::Five => Some(&mission_five::MISSION_FIVE_DEFINITION),
        MissionId::Six => Some(&mission_six::MISSION_SIX_DEFINITION),
        MissionId::Seven => None,
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

    // `apply_authored_opening_movement` applies rows sequentially and
    // `move_enemy_to` writes the destination directly with no occupancy
    // check, so two rows can stack on the same initially-empty cell at
    // runtime. Track seen destinations and opener IDs here so malformed
    // authored data is caught by the validator rather than the board.
    let mut seen_openers: HashSet<UnitId> = HashSet::new();
    let mut seen_destinations: HashSet<GridPos> = HashSet::new();

    for opening in battle.rules().opening_plan {
        assert!(
            seen_openers.insert(opening.unit),
            "opening plan references opener {:?} more than once",
            opening.unit
        );
        assert!(
            seen_destinations.insert(opening.destination),
            "opening plan targets destination {:?} more than once",
            opening.destination
        );

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
            let weapon = crate::domain::enemy::unit_weapon(battle, unit)
                .expect("opening unit has its selected weapon");
            assert!(
                weapon_reaches(weapon, opening.destination, target.position),
                "opening target must be in range and push-aligned"
            );
        }
    }
}
