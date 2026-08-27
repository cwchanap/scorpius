//! Serialized campaign state: which mission is next, credit balance, and per-mech upgrade levels.

use crate::mission::MissionId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PlayerMech {
    Vanguard,
    Gunner,
    Interceptor,
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

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SquadUpgrades {
    pub vanguard: UpgradeLevels,
    pub gunner: UpgradeLevels,
    pub interceptor: UpgradeLevels,
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
}

impl CampaignState {
    pub fn new_game() -> Self {
        Self {
            next_mission: MissionId::One,
            credits: 0,
            upgrades: SquadUpgrades::default(),
        }
    }
}
