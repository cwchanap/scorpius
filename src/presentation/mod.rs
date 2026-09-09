pub mod assets;
pub mod battle_menu;
pub mod battlefield;
pub mod campaign_ui;
pub mod interaction;
pub mod layout;
pub mod playback;
pub mod screens;
pub mod sync;
pub mod theme;
pub mod ui;

pub use battle_menu::MenuState;
pub use layout::{CanvasRoot, ViewportRoot};

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

/// The inset fill for a board diamond. The outer `CellVisual` remains the
/// source-style stroke and both nodes stay inert under marker-required UI
/// picking.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellInsetVisual(pub GridPos);

#[derive(Component)]
pub struct PresentationRoot;

/// The single UI node that owns the isometric board and its flat-sorted
/// sibling visuals.
#[derive(Component)]
pub struct BattleStage;

/// Ownership marker for the active campaign screen's UI camera.
#[derive(Component)]
pub struct CampaignCamera;

/// Ownership marker for the battle screen's UI camera. Campaign cleanup only
/// targets `CampaignCamera`, while battle cleanup only targets this marker.
#[derive(Component)]
pub struct BattleCamera2d;

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

/// The clickable upright token card for a unit. The card itself is the only
/// pickable child of the board stage for that unit.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenCard(pub UnitId);

/// Flat sibling footprint/shadow beneath a token card.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenFootprintVisual(pub UnitId);

/// Non-pickable selection/inspection diamond beneath a token card.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenSelectionVisual(pub UnitId);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenHpFill(pub UnitId);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenHpText(pub UnitId);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenAwaiting(pub UnitId);

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

/// Six newest playback messages, stored newest-first.
#[derive(Resource, Clone, Debug, Default, Eq, PartialEq)]
pub struct RecentBattleLog(pub VecDeque<String>);

impl RecentBattleLog {
    pub fn push(&mut self, entry: String) {
        self.0.push_front(entry);
        self.0.truncate(6);
    }
}

#[derive(Resource, Default)]
pub(crate) struct RestartRoundPending(pub bool);

#[derive(Resource, Default)]
pub struct RestartRequest(pub(crate) Option<u64>);

#[derive(Component)]
pub(crate) struct EventEffect;

#[derive(Resource, Default)]
pub struct AttackPreviewCells(pub BTreeSet<GridPos>);
