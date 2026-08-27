//! Save-backed campaign session: new game, continue, mission completion, purchases.

use std::fmt;

use crate::campaign::model::{CampaignState, PlayerMech, UpgradeTrack};
use crate::campaign::progression::{CampaignError, CompletionReceipt};
use crate::campaign::save::{SaveError, SaveFile};
use crate::domain::model::MissionResult;
use crate::mission::{MissionDefinition, MissionId};

pub struct CampaignSession {
    pub state: Option<CampaignState>,
    pub save: SaveFile,
    pub last_completion: Option<CompletionReceipt>,
}

impl CampaignSession {
    pub fn new(save: SaveFile) -> Self {
        Self {
            state: None,
            save,
            last_completion: None,
        }
    }
}

#[derive(Debug)]
pub enum FlowError {
    NoActiveCampaign,
    Save(SaveError),
    Campaign(CampaignError),
}

impl fmt::Display for FlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlowError::NoActiveCampaign => write!(f, "no active campaign"),
            FlowError::Save(error) => write!(f, "{error}"),
            FlowError::Campaign(error) => write!(f, "{error}"),
        }
    }
}

impl From<SaveError> for FlowError {
    fn from(error: SaveError) -> Self {
        FlowError::Save(error)
    }
}

impl From<CampaignError> for FlowError {
    fn from(error: CampaignError) -> Self {
        FlowError::Campaign(error)
    }
}

pub fn start_new_game(session: &mut CampaignSession) -> Result<(), FlowError> {
    let state = CampaignState::new_game();
    session.save.store(&state)?;
    session.state = Some(state);
    Ok(())
}

pub fn continue_game(session: &mut CampaignSession) -> Result<MissionId, FlowError> {
    let state = session.save.load()?.ok_or(FlowError::NoActiveCampaign)?;
    let next_mission = state.next_mission;
    session.state = Some(state);
    Ok(next_mission)
}

pub fn complete_current_mission(
    session: &mut CampaignSession,
    definition: &MissionDefinition,
    result: MissionResult,
) -> Result<CompletionReceipt, FlowError> {
    let mut next = session.state.clone().ok_or(FlowError::NoActiveCampaign)?;
    let receipt = next.complete_mission(definition, result)?;
    session.save.store(&next)?;
    session.state = Some(next);
    session.last_completion = Some(receipt);
    Ok(receipt)
}

pub fn persist_purchase(
    session: &mut CampaignSession,
    mech: PlayerMech,
    track: UpgradeTrack,
) -> Result<(), FlowError> {
    let mut next = session.state.clone().ok_or(FlowError::NoActiveCampaign)?;
    next.purchase_upgrade(mech, track)?;
    session.save.store(&next)?;
    session.state = Some(next);
    Ok(())
}
