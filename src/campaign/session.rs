//! Save-backed campaign session: new game, continue, mission completion,
//! purchases, and replays of cleared missions.

use std::fmt;

use crate::campaign::model::{CampaignState, PlayerMech, UpgradeTrack};
use crate::campaign::progression::{CampaignError, CompletionReceipt};
use crate::campaign::save::{SaveError, SaveFile};
use crate::domain::model::{MissionResult, Takedown};
use crate::mission::{MissionDefinition, MissionId};

pub struct CampaignSession {
    pub state: Option<CampaignState>,
    pub save: SaveFile,
    pub last_completion: Option<CompletionReceipt>,
    /// A cleared mission picked on Mission Select. A replay never awards
    /// credits, advances the campaign, or writes the save.
    pub replay: Option<MissionId>,
}

impl CampaignSession {
    pub fn new(save: SaveFile) -> Self {
        Self {
            state: None,
            save,
            last_completion: None,
            replay: None,
        }
    }

    /// The mission the briefing and battle play: the replay, else the
    /// campaign's next mission.
    pub fn active_mission(&self) -> Option<MissionId> {
        self.replay
            .or_else(|| self.state.as_ref().map(|state| state.next_mission))
    }
}

#[derive(Debug)]
pub enum FlowError {
    NoActiveCampaign,
    /// A replayed mission never completes the campaign's current mission.
    Replay,
    Save(SaveError),
    Campaign(CampaignError),
}

impl fmt::Display for FlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlowError::NoActiveCampaign => write!(f, "no active campaign"),
            FlowError::Replay => write!(f, "a replay does not change campaign progress"),
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
    session.last_completion = None;
    session.replay = None;
    Ok(())
}

pub fn continue_game(session: &mut CampaignSession) -> Result<MissionId, FlowError> {
    let state = session.save.load()?.ok_or(FlowError::NoActiveCampaign)?;
    let next_mission = state.next_mission;
    session.state = Some(state);
    session.last_completion = None;
    session.replay = None;
    Ok(next_mission)
}

/// Loads the save for Mission Select. Fails like Continue when there is no
/// save, and leaves any replay behind.
pub fn open_mission_select(session: &mut CampaignSession) -> Result<(), FlowError> {
    let state = session.save.load()?.ok_or(FlowError::NoActiveCampaign)?;
    session.state = Some(state);
    session.last_completion = None;
    session.replay = None;
    Ok(())
}

/// Picks a cleared mission to replay with the squad's current upgrades.
pub fn start_replay(session: &mut CampaignSession, mission: MissionId) -> Result<(), FlowError> {
    let state = session.state.as_ref().ok_or(FlowError::NoActiveCampaign)?;
    if !state.has_cleared(mission) {
        return Err(CampaignError::MissionLocked(mission).into());
    }
    session.replay = Some(mission);
    session.last_completion = None;
    Ok(())
}

/// Complete the current mission with no takedowns credited.
pub fn complete_current_mission(
    session: &mut CampaignSession,
    definition: &MissionDefinition,
    result: MissionResult,
) -> Result<CompletionReceipt, FlowError> {
    complete_current_mission_with_takedowns(session, definition, result, &[])
}

/// Complete the current mission, award its pilot XP, and save.
pub fn complete_current_mission_with_takedowns(
    session: &mut CampaignSession,
    definition: &MissionDefinition,
    result: MissionResult,
    takedowns: &[Takedown],
) -> Result<CompletionReceipt, FlowError> {
    if session.replay.is_some() {
        return Err(FlowError::Replay);
    }
    let mut next = session.state.clone().ok_or(FlowError::NoActiveCampaign)?;
    let receipt = next.complete_mission_with_takedowns(definition, result, takedowns)?;
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
