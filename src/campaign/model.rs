//! Serialized campaign state: which mission is next, credit balance, per-mech
//! upgrade levels, and pilot XP.

use crate::campaign::pilots::PilotXp;
use crate::mission::MissionId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PlayerMech {
    Vanguard,
    Gunner,
    Interceptor,
}

impl PlayerMech {
    pub const ALL: [Self; 3] = [Self::Vanguard, Self::Gunner, Self::Interceptor];
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum UpgradeTrack {
    Hp,
    Armor,
    Mobility,
    Weapon,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UpgradeLevels {
    pub hp: u8,
    pub armor: u8,
    pub mobility: u8,
    pub weapon: u8,
}

impl UpgradeLevels {
    pub fn level(&self, track: UpgradeTrack) -> u8 {
        match track {
            UpgradeTrack::Hp => self.hp,
            UpgradeTrack::Armor => self.armor,
            UpgradeTrack::Mobility => self.mobility,
            UpgradeTrack::Weapon => self.weapon,
        }
    }

    pub fn level_mut(&mut self, track: UpgradeTrack) -> &mut u8 {
        match track {
            UpgradeTrack::Hp => &mut self.hp,
            UpgradeTrack::Armor => &mut self.armor,
            UpgradeTrack::Mobility => &mut self.mobility,
            UpgradeTrack::Weapon => &mut self.weapon,
        }
    }
}

/// Everything the campaign projects onto the squad: hangar upgrades per mech
/// and each pilot's XP.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SquadUpgrades {
    pub vanguard: UpgradeLevels,
    pub gunner: UpgradeLevels,
    pub interceptor: UpgradeLevels,
    // Saves written before pilot levels load with every pilot at 0 XP.
    #[serde(default)]
    pub pilots: PilotXp,
}

impl SquadUpgrades {
    pub fn levels(&self, mech: PlayerMech) -> &UpgradeLevels {
        match mech {
            PlayerMech::Vanguard => &self.vanguard,
            PlayerMech::Gunner => &self.gunner,
            PlayerMech::Interceptor => &self.interceptor,
        }
    }

    pub fn levels_mut(&mut self, mech: PlayerMech) -> &mut UpgradeLevels {
        match mech {
            PlayerMech::Vanguard => &mut self.vanguard,
            PlayerMech::Gunner => &mut self.gunner,
            PlayerMech::Interceptor => &mut self.interceptor,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CampaignState {
    pub next_mission: MissionId,
    pub credits: u32,
    pub upgrades: SquadUpgrades,
    // `#[serde(default)]` keeps saves written before `completed` existed
    // loadable, treating them as not yet completed instead of failing with a
    // corrupted-save error.
    #[serde(default)]
    pub completed: bool,
}

impl CampaignState {
    pub fn new_game() -> Self {
        Self {
            next_mission: MissionId::One,
            credits: 0,
            upgrades: SquadUpgrades::default(),
            completed: false,
        }
    }

    /// Whether `mission` has been won in this campaign, so Mission Select may
    /// replay it.
    pub fn has_cleared(&self, mission: MissionId) -> bool {
        self.completed || mission < self.next_mission
    }

    /// The cleared missions, in campaign order.
    pub fn cleared_missions(&self) -> impl Iterator<Item = MissionId> + '_ {
        MissionId::ALL
            .into_iter()
            .filter(|mission| self.has_cleared(*mission))
    }
}
