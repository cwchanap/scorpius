//! Mission completion and upgrade purchase progression rules.

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

impl CampaignState {
    pub fn complete_mission(
        &mut self,
        definition: &MissionDefinition,
        result: MissionResult,
    ) -> Result<CompletionReceipt, CampaignError> {
        if !result.victory {
            return Err(CampaignError::MissionNotWon);
        }
        if self.next_mission != definition.id {
            return Err(CampaignError::AlreadyAdvanced {
                expected: definition.id,
                actual: self.next_mission,
            });
        }
        let optional_reward = if result.turnabout_complete {
            definition.optional_reward
        } else {
            0
        };
        let total_reward = definition.base_reward + optional_reward;
        self.credits += total_reward;
        self.next_mission = definition.unlocks;
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
