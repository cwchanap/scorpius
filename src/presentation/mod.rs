pub mod assets;
pub mod battlefield;
pub mod campaign_ui;
pub mod interaction;
pub mod playback;
pub mod sync;
pub mod ui;

use std::collections::{BTreeSet, VecDeque};

use bevy::prelude::*;

use crate::campaign::session::CampaignSession;
use crate::domain::{
    battle::BattleState,
    board::GridPos,
    model::{BattleEvent, Reaction, UnitId, WeaponShape},
};
use crate::mission::MissionDefinition;

#[derive(Resource)]
pub struct BattleRuntime(pub BattleState);

/// Save-backed campaign session the battle lifecycle reads from.
#[derive(Resource)]
pub struct CampaignRuntime(pub CampaignSession);

/// Authored definition of the mission currently in play.
#[derive(Resource, Clone, Copy)]
pub struct ActiveMission(pub &'static MissionDefinition);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnitVisual(pub UnitId);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellVisual(pub GridPos);

#[derive(Component)]
pub struct PresentationRoot;

#[derive(Component)]
pub(crate) struct PresentationNeedsRebuild;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct TelegraphVisual {
    pub attacker: UnitId,
    pub cell: GridPos,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct TelegraphGlyphVisual(pub WeaponShape);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntentTargetVisual {
    pub attacker: UnitId,
    pub target: UnitId,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntentLineVisual {
    pub attacker: UnitId,
    pub origin: GridPos,
    pub center: GridPos,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub enum PropVisual {
    Blocking(GridPos),
    Explosive(GridPos),
    Hazard(GridPos),
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReactionVisual {
    pub unit: UnitId,
    pub reaction: Reaction,
}

/// Ground marker at the intercept mission's escape cell; spawned/kept by
/// `sync::reconcile_extraction_marker` only while the primary is intercept.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExtractionVisual(pub GridPos);

#[derive(Resource, Default)]
pub struct BattleEventQueue(pub VecDeque<BattleEvent>);

#[derive(Resource, Default)]
pub struct EventPlayback {
    pub current: Option<(BattleEvent, Timer)>,
    pub input_locked: bool,
}

#[derive(Resource, Default)]
pub(crate) struct RestartRoundPending(pub bool);

#[derive(Resource, Default)]
pub(crate) struct RestartRequest(pub Option<u64>);

#[derive(Component)]
pub(crate) struct EventEffect;

#[derive(Resource, Default)]
pub struct AttackPreviewCells(pub BTreeSet<GridPos>);

#[derive(Resource, Default)]
pub struct SelectedCell(pub Option<GridPos>);

pub fn grid_to_world(pos: GridPos) -> Vec3 {
    const HALF: f32 = 4.0;
    Vec3::new(pos.x as f32 - HALF, 0.2, pos.y as f32 - HALF)
}
