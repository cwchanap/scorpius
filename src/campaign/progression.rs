//! Mission completion and upgrade purchase progression rules.

use std::fmt;

use crate::campaign::model::{CampaignState, PlayerMech, UpgradeTrack};
use crate::domain::model::MissionResult;
use crate::mission::{MissionDefinition, MissionId};

pub const UPGRADE_COSTS: [u32; 3] = [200, 400, 600];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompletionReceipt {
    pub mission: MissionId,
    pub base_reward: u32,
    pub optional_reward: u32,
    pub total_reward: u32,
    pub credits_after: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CampaignError {
    MissionNotWon,
    CampaignComplete,
    AlreadyAdvanced {
        expected: MissionId,
        actual: MissionId,
    },
    MaxLevel,
    InsufficientCredits {
        required: u32,
        available: u32,
    },
}

impl fmt::Display for CampaignError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CampaignError::MissionNotWon => write!(f, "mission was not won"),
            CampaignError::CampaignComplete => write!(f, "campaign already complete"),
            CampaignError::AlreadyAdvanced { expected, actual } => write!(
                f,
                "campaign already advanced past {expected:?} (now at {actual:?})"
            ),
            CampaignError::MaxLevel => write!(f, "upgrade track already at max level"),
            CampaignError::InsufficientCredits {
                required,
                available,
            } => write!(
                f,
                "insufficient credits: {required} required, {available} available"
            ),
        }
    }
}

impl CampaignState {
    pub fn complete_mission(
        &mut self,
        definition: &MissionDefinition,
        result: MissionResult,
    ) -> Result<CompletionReceipt, CampaignError> {
        if !result.victory {
            return Err(CampaignError::MissionNotWon);
        }
        if self.completed {
            return Err(CampaignError::CampaignComplete);
        }
        if self.next_mission != definition.id {
            return Err(CampaignError::AlreadyAdvanced {
                expected: definition.id,
                actual: self.next_mission,
            });
        }
        let optional_reward = if result.optional_complete {
            definition.optional_reward
        } else {
            0
        };
        let total_reward = definition.base_reward + optional_reward;
        self.credits += total_reward;
        match definition.unlocks {
            Some(next) => self.next_mission = next,
            None => self.completed = true,
        }
        Ok(CompletionReceipt {
            mission: definition.id,
            base_reward: definition.base_reward,
            optional_reward,
            total_reward,
            credits_after: self.credits,
        })
    }

    pub fn purchase_upgrade(
        &mut self,
        mech: PlayerMech,
        track: UpgradeTrack,
    ) -> Result<(), CampaignError> {
        let current = self.upgrades.levels(mech).level(track);
        if current >= 3 {
            return Err(CampaignError::MaxLevel);
        }
        let cost = UPGRADE_COSTS[current as usize];
        if self.credits < cost {
            return Err(CampaignError::InsufficientCredits {
                required: cost,
                available: self.credits,
            });
        }
        self.credits -= cost;
        *self.upgrades.levels_mut(mech).level_mut(track) = current + 1;
        Ok(())
    }
}
