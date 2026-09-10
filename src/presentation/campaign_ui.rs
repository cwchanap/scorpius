//! Campaign actions, state derived presentation data, and small UI adapters.
//!
//! Layout construction lives in [`super::screens`]. This module keeps the
//! campaign session as the source of truth and exposes typed values to each
//! screen rather than building screen-sized copy strings.

use bevy::prelude::*;

use crate::app::GameScreen;
use crate::campaign::model::{CampaignState, PlayerMech, UpgradeLevels, UpgradeTrack};
use crate::campaign::progression::UPGRADE_COSTS;
use crate::campaign::session::{continue_game, persist_purchase, start_new_game};
use crate::domain::model::Faction;
use crate::mission::{DialogueScene, MissionDefinition, MissionId, mission_definition};
use crate::presentation::CampaignRuntime;

use super::{ActiveMission, CampaignCamera};

/// Root of a campaign-flow screen. The root and its marked camera are both
/// owned by the active `GameScreen` and removed on exit.
#[derive(Component)]
pub struct ScreenRoot;

/// Campaign-flow status line; save/flow errors are surfaced here verbatim.
#[derive(Resource, Clone, Debug, Default, Eq, PartialEq)]
pub struct CampaignStatus(pub String);

/// Index of the line currently shown on a dialogue screen.
#[derive(Resource, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DialogueCursor(pub usize);

/// Action emitted by campaign-flow buttons.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub enum CampaignUiAction {
    NewGame,
    Continue,
    AdvanceDialogue,
    SkipDialogue,
    StartMission,
    AdvanceAftermath,
    PurchaseUpgrade(PlayerMech, UpgradeTrack),
    Proceed,
    ReturnToTitle,
}

/// Exact speaker/text/portrait values for the current dialogue line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DialogueSnapshot {
    pub speaker: &'static str,
    pub text: &'static str,
    pub portrait: &'static str,
}

/// Mission facts rendered by the briefing screen. The enemy count is derived
/// from one deterministic authored build at screen entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BriefingSnapshot {
    pub mission: MissionId,
    pub title: &'static str,
    pub bonus_title: &'static str,
    pub enemy_count: usize,
    pub primary: &'static str,
    pub optional: &'static str,
    pub base_reward: u32,
    pub optional_reward: u32,
    pub credits: u32,
}

/// Upgrade facts rendered by one hangar row. Purchase actions still route
/// through the campaign session; this value is display-only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeRowSnapshot {
    pub mech: PlayerMech,
    pub track: UpgradeTrack,
    pub level: u8,
    pub current_effect: String,
    pub next_effect: String,
    pub cost: Option<u32>,
    pub maxed: bool,
    pub affordable: bool,
}

/// Persisted campaign values rendered by the ending screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EndingSnapshot {
    pub credits: u32,
    pub vanguard: UpgradeLevels,
    pub gunner: UpgradeLevels,
    pub interceptor: UpgradeLevels,
}

#[derive(Component)]
pub struct DialoguePortrait;

#[derive(Component)]
pub struct DialogueSpeaker;

#[derive(Component)]
pub struct DialogueText;

#[derive(Component)]
pub struct DialoguePip(pub usize);

#[derive(Component)]
pub struct CampaignStatusText;

#[derive(Component)]
pub struct UpgradeCreditsText;

#[derive(Component, Clone, Copy)]
pub struct UpgradeRow(pub PlayerMech, pub UpgradeTrack);

#[derive(Component, Clone, Copy)]
pub struct UpgradePurchaseIcon(pub PlayerMech, pub UpgradeTrack);

#[derive(Component, Clone, Copy)]
pub struct UpgradeTrackIcon(pub PlayerMech, pub UpgradeTrack);

#[derive(Component, Clone, Copy)]
pub struct UpgradeCostText(pub PlayerMech, pub UpgradeTrack);

#[derive(Component, Clone, Copy)]
pub struct UpgradePip {
    pub mech: PlayerMech,
    pub track: UpgradeTrack,
    pub index: u8,
}

pub const MECHS: [(PlayerMech, &str); 3] = [
    (PlayerMech::Vanguard, "VANGUARD"),
    (PlayerMech::Gunner, "GUNNER"),
    (PlayerMech::Interceptor, "INTERCEPTOR"),
];

pub const TRACKS: [UpgradeTrack; 4] = [
    UpgradeTrack::Hp,
    UpgradeTrack::Armor,
    UpgradeTrack::Mobility,
    UpgradeTrack::Weapon,
];

pub fn dialogue_snapshot(scene: &DialogueScene, cursor: DialogueCursor) -> DialogueSnapshot {
    let line = &scene.lines[cursor.0.min(scene.lines.len().saturating_sub(1))];
    DialogueSnapshot {
        speaker: line.speaker,
        text: line.text,
        portrait: line.portrait,
    }
}

pub fn briefing_snapshot(
    definition: &MissionDefinition,
    campaign: &CampaignState,
) -> BriefingSnapshot {
    let battle = (definition.build)(0, &campaign.upgrades);
    let enemy_count = battle
        .units()
        .filter(|unit| unit.faction == Faction::Enemy)
        .count();
    BriefingSnapshot {
        mission: definition.id,
        title: definition.title,
        bonus_title: definition
            .optional_objective
            .split_once(':')
            .map_or(definition.optional_objective, |(title, _)| title),
        enemy_count,
        primary: definition.primary_objective,
        optional: definition.optional_objective,
        base_reward: definition.base_reward,
        optional_reward: definition.optional_reward,
        credits: campaign.credits,
    }
}

pub fn upgrade_row_snapshot(
    state: &CampaignState,
    mech: PlayerMech,
    track: UpgradeTrack,
) -> UpgradeRowSnapshot {
    let level = state.upgrades.levels(mech).level(track);
    let maxed = level >= 3;
    let cost = (!maxed)
        .then(|| UPGRADE_COSTS.get(usize::from(level)).copied())
        .flatten();
    UpgradeRowSnapshot {
        mech,
        track,
        level,
        current_effect: track_effect(track, level),
        next_effect: if maxed {
            "MAX".to_owned()
        } else {
            track_effect(track, level.saturating_add(1))
        },
        cost,
        maxed,
        affordable: cost.is_some_and(|cost| state.credits >= cost),
    }
}

pub fn ending_snapshot(state: &CampaignState) -> EndingSnapshot {
    EndingSnapshot {
        credits: state.credits,
        vanguard: *state.upgrades.levels(PlayerMech::Vanguard),
        gunner: *state.upgrades.levels(PlayerMech::Gunner),
        interceptor: *state.upgrades.levels(PlayerMech::Interceptor),
    }
}

pub fn track_label(track: UpgradeTrack) -> &'static str {
    match track {
        UpgradeTrack::Hp => "HP",
        UpgradeTrack::Armor => "ARMOR",
        UpgradeTrack::Mobility => "MOBILITY",
        UpgradeTrack::Weapon => "WEAPON",
    }
}

pub fn track_effect(track: UpgradeTrack, level: u8) -> String {
    match track {
        UpgradeTrack::Hp => format!("+{} MAX HP", 3 * u32::from(level)),
        UpgradeTrack::Armor => format!("+{} ARMOR", level),
        UpgradeTrack::Mobility => format!("+{} EVASION", 5 * u32::from(level)),
        UpgradeTrack::Weapon => format!("+{} WEAPON DMG", level),
    }
}

pub fn format_upgrade_row(snapshot: &UpgradeRowSnapshot) -> String {
    snapshot.next_effect.clone()
}

/// Shared campaign-screen cleanup. The explicit markers keep one screen from
/// touching another screen's camera during transitions.
#[allow(clippy::type_complexity)]
pub fn despawn_campaign_screen(
    mut commands: Commands,
    screens: Query<Entity, Or<(With<ScreenRoot>, With<CampaignCamera>)>>,
) {
    for entity in &screens {
        commands.entity(entity).try_despawn();
    }
}

/// Pure terminal routing shared by Continue, final Aftermath advance, and
/// Proceed. `None` means the action does not route a campaign screen.
fn campaign_destination(action: CampaignUiAction, state: &CampaignState) -> Option<GameScreen> {
    match action {
        CampaignUiAction::Continue if state.completed => Some(GameScreen::Ending),
        CampaignUiAction::Continue if state.next_mission == MissionId::One => {
            Some(GameScreen::PreMissionStory)
        }
        CampaignUiAction::Continue => Some(GameScreen::Upgrade),
        CampaignUiAction::AdvanceAftermath if state.completed => Some(GameScreen::Ending),
        CampaignUiAction::AdvanceAftermath => Some(GameScreen::Upgrade),
        CampaignUiAction::Proceed if state.completed => Some(GameScreen::Ending),
        CampaignUiAction::Proceed => Some(GameScreen::PreMissionStory),
        _ => None,
    }
}

/// Whether the state machine already has a queued transition this frame.
pub fn screen_transition_pending(next_state: &NextState<GameScreen>) -> bool {
    !matches!(next_state, NextState::Unchanged)
}

pub fn apply_campaign_action(
    action: CampaignUiAction,
    current_screen: GameScreen,
    runtime: &mut CampaignRuntime,
    active_mission: Option<&ActiveMission>,
    cursor: &mut DialogueCursor,
    status: &mut CampaignStatus,
    next_state: &mut NextState<GameScreen>,
) {
    if screen_transition_pending(next_state) {
        return;
    }
    match action {
        CampaignUiAction::NewGame => match start_new_game(&mut runtime.0) {
            Ok(()) => next_state.set(GameScreen::PreMissionStory),
            Err(error) => status.0 = error.to_string(),
        },
        CampaignUiAction::Continue => match continue_game(&mut runtime.0) {
            Ok(_) => {
                if let Some(screen) = runtime
                    .0
                    .state
                    .as_ref()
                    .and_then(|state| campaign_destination(CampaignUiAction::Continue, state))
                {
                    next_state.set(screen);
                }
            }
            Err(error) => status.0 = error.to_string(),
        },
        CampaignUiAction::AdvanceDialogue => {
            if current_screen != GameScreen::PreMissionStory {
                status.0 = "Advance is only available during pre-mission story.".into();
                return;
            }
            let line_count = active_definition(runtime)
                .map_or(0, |definition| definition.pre_mission.lines.len());
            advance_dialogue(cursor, line_count, GameScreen::Briefing, next_state);
        }
        CampaignUiAction::SkipDialogue => {
            if current_screen != GameScreen::PreMissionStory {
                status.0 = "Skip is only available during pre-mission story.".into();
                return;
            }
            let line_count = active_definition(runtime)
                .map_or(0, |definition| definition.pre_mission.lines.len());
            cursor.0 = line_count.saturating_sub(1);
            next_state.set(GameScreen::Briefing);
        }
        CampaignUiAction::StartMission => next_state.set(GameScreen::Battle),
        CampaignUiAction::AdvanceAftermath => {
            let Some(mission) = active_mission else {
                return;
            };
            let destination = runtime
                .0
                .state
                .as_ref()
                .and_then(|state| campaign_destination(CampaignUiAction::AdvanceAftermath, state))
                .unwrap_or(GameScreen::Upgrade);
            advance_dialogue(
                cursor,
                mission.0.aftermath.lines.len(),
                destination,
                next_state,
            );
        }
        CampaignUiAction::PurchaseUpgrade(mech, track) => {
            match persist_purchase(&mut runtime.0, mech, track) {
                Ok(()) => {
                    status.0 = format!(
                        "Upgrade purchased — {} credits remaining.",
                        runtime.0.state.as_ref().map_or(0, |state| state.credits)
                    );
                }
                Err(error) => status.0 = error.to_string(),
            }
        }
        CampaignUiAction::Proceed => {
            if let Some(screen) = runtime
                .0
                .state
                .as_ref()
                .and_then(|state| campaign_destination(CampaignUiAction::Proceed, state))
            {
                next_state.set(screen);
            }
        }
        CampaignUiAction::ReturnToTitle => next_state.set(GameScreen::Title),
    }
}

fn advance_dialogue(
    cursor: &mut DialogueCursor,
    line_count: usize,
    next_screen: GameScreen,
    next_state: &mut NextState<GameScreen>,
) {
    if cursor.0 + 1 < line_count {
        cursor.0 += 1;
    } else {
        cursor.0 = line_count.saturating_sub(1);
        next_state.set(next_screen);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn on_campaign_ui_click(
    click: On<Pointer<Click>>,
    actions: Query<&CampaignUiAction>,
    mut runtime: ResMut<CampaignRuntime>,
    active_mission: Option<Res<ActiveMission>>,
    mut cursor: ResMut<DialogueCursor>,
    mut status: ResMut<CampaignStatus>,
    mut next_state: ResMut<NextState<GameScreen>>,
    current_screen: Res<State<GameScreen>>,
) {
    let Ok(action) = actions.get(click.entity) else {
        return;
    };
    apply_campaign_action(
        *action,
        *current_screen.get(),
        &mut runtime,
        active_mission.as_deref(),
        &mut cursor,
        &mut status,
        &mut next_state,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn update_dialogue_screen(
    current: Res<State<GameScreen>>,
    runtime: Res<CampaignRuntime>,
    active_mission: Option<Res<ActiveMission>>,
    cursor: Res<DialogueCursor>,
    asset_server: Res<AssetServer>,
    mut portrait: Single<&mut ImageNode, With<DialoguePortrait>>,
    mut speaker: Single<&mut Text, (With<DialogueSpeaker>, Without<DialogueText>)>,
    mut text: Single<&mut Text, (With<DialogueText>, Without<DialogueSpeaker>)>,
    mut pips: Query<(&DialoguePip, &mut Node, &mut BackgroundColor)>,
) {
    let scene = match current.get() {
        GameScreen::Aftermath => active_mission
            .as_deref()
            .map(|mission| &mission.0.aftermath),
        _ => active_definition(&runtime).map(|definition| &definition.pre_mission),
    };
    let Some(scene) = scene else {
        return;
    };
    let snapshot = dialogue_snapshot(scene, *cursor);
    speaker.0 = snapshot.speaker.to_owned();
    text.0 = snapshot.text.to_owned();
    portrait.image = asset_server.load(snapshot.portrait);
    for (pip, mut node, mut background) in &mut pips {
        let active = pip.0 == cursor.0;
        node.width = px(if active { 22.0 } else { 10.0 });
        background.0 = if active {
            super::theme::ACCENT
        } else {
            super::theme::BORDER
        };
    }
}

pub fn update_campaign_status_text(
    status: Res<CampaignStatus>,
    mut text: Single<&mut Text, With<CampaignStatusText>>,
) {
    text.0 = status.0.clone();
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update_upgrade_screen(
    runtime: Res<CampaignRuntime>,
    mut rows: Query<
        (&UpgradeRow, &mut Text),
        (
            Without<UpgradeCreditsText>,
            Without<CampaignStatusText>,
            Without<UpgradeCostText>,
        ),
    >,
    mut credits: Single<&mut Text, (With<UpgradeCreditsText>, Without<CampaignStatusText>)>,
    mut buttons: Query<
        (&CampaignUiAction, &mut BackgroundColor, &mut Pickable),
        (
            Without<UpgradeRow>,
            Without<UpgradePip>,
            Without<UpgradeCostText>,
        ),
    >,
    mut pips: Query<(&UpgradePip, &mut BackgroundColor)>,
    mut track_icons: Query<(&UpgradeTrackIcon, &mut ImageNode), Without<UpgradePurchaseIcon>>,
    mut purchase_icons: Query<(&UpgradePurchaseIcon, &mut ImageNode), Without<UpgradeTrackIcon>>,
    mut costs: Query<
        (&UpgradeCostText, &mut Text),
        (Without<UpgradeCreditsText>, Without<UpgradeRow>),
    >,
) {
    let Some(state) = runtime.0.state.as_ref() else {
        return;
    };
    credits.0 = state.credits.to_string();
    for (row, mut text) in &mut rows {
        text.0 = format_upgrade_row(&upgrade_row_snapshot(state, row.0, row.1));
    }
    for (action, mut background, mut pickable) in &mut buttons {
        let CampaignUiAction::PurchaseUpgrade(mech, track) = *action else {
            continue;
        };
        let snapshot = upgrade_row_snapshot(state, mech, track);
        let enabled = snapshot.affordable;
        background.0 = if enabled {
            super::theme::PANEL_RAISED
        } else {
            Color::srgb_u8(11, 17, 24)
        };
        *pickable = if enabled {
            Pickable::default()
        } else {
            Pickable::IGNORE
        };
    }
    for (icon, mut image) in &mut purchase_icons {
        let snapshot = upgrade_row_snapshot(state, icon.0, icon.1);
        image.rect = Some(if snapshot.maxed {
            super::theme::ICON_WAIT
        } else {
            super::theme::CREDITS_PURCHASE_RECT
        });
        image.color = if snapshot.maxed {
            super::theme::MINT
        } else if snapshot.affordable {
            Color::WHITE
        } else {
            Color::srgb_u8(51, 69, 90)
        };
    }
    for (icon, mut image) in &mut track_icons {
        let snapshot = upgrade_row_snapshot(state, icon.0, icon.1);
        image.color = if snapshot.maxed {
            super::theme::MINT
        } else {
            super::theme::MUTED
        };
    }
    for (cost, mut text) in &mut costs {
        let snapshot = upgrade_row_snapshot(state, cost.0, cost.1);
        text.0 = snapshot
            .cost
            .map_or_else(|| "—".to_owned(), |value| value.to_string());
    }
    for (pip, mut background) in &mut pips {
        let level = state.upgrades.levels(pip.mech).level(pip.track);
        background.0 = if pip.index < level {
            if level >= 3 {
                super::theme::MINT
            } else {
                super::theme::ACCENT
            }
        } else {
            super::theme::BORDER
        };
    }
}

pub fn active_definition(runtime: &CampaignRuntime) -> Option<&'static MissionDefinition> {
    let state = runtime.0.state.as_ref()?;
    mission_definition(state.next_mission)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign::model::SquadUpgrades;

    fn state(completed: bool, next_mission: MissionId) -> CampaignState {
        CampaignState {
            next_mission,
            credits: 0,
            upgrades: SquadUpgrades::default(),
            completed,
        }
    }

    #[test]
    fn campaign_destination_pins_the_terminal_routing_truth_table() {
        let completed = state(true, MissionId::Seven);
        let unfinished_seven = state(false, MissionId::Seven);
        let unfinished_one = state(false, MissionId::One);

        assert_eq!(
            campaign_destination(CampaignUiAction::Continue, &completed),
            Some(GameScreen::Ending)
        );
        assert_eq!(
            campaign_destination(CampaignUiAction::Continue, &unfinished_one),
            Some(GameScreen::PreMissionStory)
        );
        assert_eq!(
            campaign_destination(CampaignUiAction::Continue, &unfinished_seven),
            Some(GameScreen::Upgrade)
        );
        assert_eq!(
            campaign_destination(CampaignUiAction::AdvanceAftermath, &completed),
            Some(GameScreen::Ending)
        );
        assert_eq!(
            campaign_destination(CampaignUiAction::AdvanceAftermath, &unfinished_seven),
            Some(GameScreen::Upgrade)
        );
        assert_eq!(
            campaign_destination(CampaignUiAction::Proceed, &completed),
            Some(GameScreen::Ending)
        );
        assert_eq!(
            campaign_destination(CampaignUiAction::Proceed, &unfinished_seven),
            Some(GameScreen::PreMissionStory)
        );
    }

    #[test]
    fn upgrade_row_format_is_the_next_effect_only() {
        let row = UpgradeRowSnapshot {
            mech: PlayerMech::Vanguard,
            track: UpgradeTrack::Hp,
            level: 1,
            current_effect: "+3 MAX HP".to_owned(),
            next_effect: "+6 MAX HP".to_owned(),
            cost: Some(100),
            maxed: false,
            affordable: true,
        };
        assert_eq!(format_upgrade_row(&row), "+6 MAX HP");
    }
}
