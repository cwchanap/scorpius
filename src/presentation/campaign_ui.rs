//! Title, pre-mission story, and briefing screens for the campaign loop.
//!
//! Every screen here roots under [`ScreenRoot`], deliberately separate from
//! [`super::PresentationRoot`]: leaving a campaign screen despawns only its own
//! UI and 2D camera, never the 3D battlefield.

use bevy::prelude::*;

use crate::app::GameScreen;
use crate::campaign::model::{CampaignState, PlayerMech, UpgradeTrack};
use crate::campaign::progression::{CompletionReceipt, UPGRADE_COSTS};
use crate::campaign::session::{FlowError, continue_game, persist_purchase, start_new_game};
use crate::mission::{DialogueScene, MissionDefinition, MissionId, mission_definition};
use crate::presentation::CampaignRuntime;

use super::ActiveMission;

/// Root of a campaign-flow screen (Title / pre-mission story / briefing /
/// aftermath / upgrade / next-mission): despawned when the screen changes.
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

pub fn dialogue_snapshot(scene: &DialogueScene, cursor: DialogueCursor) -> DialogueSnapshot {
    let line = &scene.lines[cursor.0.min(scene.lines.len() - 1)];
    DialogueSnapshot {
        speaker: line.speaker,
        text: line.text,
        portrait: line.portrait,
    }
}

pub fn briefing_copy(definition: &MissionDefinition) -> String {
    format!(
        "{}\n\nPRIMARY\n{}\n\nBONUS\n{}\n\nREWARD\n{} credits\nBONUS +{} credits",
        definition.title,
        definition.primary_objective,
        definition.optional_objective,
        definition.base_reward,
        definition.optional_reward,
    )
}

#[derive(Component)]
pub struct DialoguePortrait;

#[derive(Component)]
pub struct DialogueSpeaker;

#[derive(Component)]
pub struct DialogueText;

#[derive(Component)]
pub struct CampaignStatusText;

/// Spawn one dialogue screen for `scene`: background, portrait, speaker,
/// dialogue text, and a single advance button emitting `advance_action`.
///
/// Task 8 reuses this helper for the aftermath scene; it is deliberately not a
/// dialogue engine.
fn spawn_dialogue_screen(
    commands: &mut Commands,
    asset_server: &AssetServer,
    scene: &DialogueScene,
    advance_action: CampaignUiAction,
) -> Entity {
    commands.spawn(Camera2d);
    let root = commands
        .spawn((
            Name::new("Dialogue Screen"),
            ScreenRoot,
            fullscreen_node(),
            Pickable::IGNORE,
        ))
        .id();
    let opening = dialogue_snapshot(scene, DialogueCursor(0));

    commands.spawn((
        Node {
            width: percent(100),
            height: percent(100),
            ..default()
        },
        ImageNode::new(asset_server.load(scene.background)),
        Pickable::IGNORE,
        ChildOf(root),
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(28),
            bottom: px(196),
            width: px(240),
            height: px(240),
            border: UiRect::all(px(2)),
            ..default()
        },
        BorderColor::all(Color::srgb(0.24, 0.78, 0.86)),
        ImageNode::new(asset_server.load(opening.portrait)),
        DialoguePortrait,
        Pickable::IGNORE,
        ChildOf(root),
    ));
    commands.spawn((
        Text::new(opening.speaker),
        text_font(21.0),
        TextColor(Color::srgb(1.0, 0.82, 0.46)),
        Node {
            position_type: PositionType::Absolute,
            left: px(30),
            bottom: px(152),
            ..default()
        },
        DialogueSpeaker,
        Pickable::IGNORE,
        ChildOf(root),
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(24),
            right: px(24),
            bottom: px(24),
            height: px(118),
            ..default()
        },
        BackgroundColor(Color::srgba(0.012, 0.02, 0.035, 0.82)),
        Pickable::IGNORE,
        ChildOf(root),
    ));
    commands.spawn((
        Text::new(opening.text),
        text_font(16.0),
        TextColor(Color::srgb(0.9, 0.94, 0.98)),
        Node {
            position_type: PositionType::Absolute,
            left: px(48),
            bottom: px(48),
            width: px(760),
            ..default()
        },
        DialogueText,
        Pickable::IGNORE,
        ChildOf(root),
    ));
    spawn_action_button(
        commands,
        root,
        advance_action,
        "CONTINUE",
        true,
        Node {
            position_type: PositionType::Absolute,
            right: px(44),
            bottom: px(46),
            width: px(170),
            height: px(44),
            ..default()
        },
    );
    root
}

pub fn setup_title_screen(
    mut commands: Commands,
    runtime: Res<CampaignRuntime>,
    mut status: ResMut<CampaignStatus>,
) {
    status.0.clear();
    let continue_enabled = match runtime.0.save.load() {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(error) => {
            status.0 = FlowError::from(error).to_string();
            false
        }
    };
    commands.spawn(Camera2d);
    let root = commands
        .spawn((
            Name::new("Title Screen"),
            ScreenRoot,
            fullscreen_node(),
            BackgroundColor(Color::srgb(0.012, 0.016, 0.028)),
            Pickable::IGNORE,
        ))
        .id();
    commands.spawn((
        Text::new("SCORPIUS"),
        text_font(76.0),
        TextColor(Color::srgb(0.78, 0.92, 1.0)),
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: px(112),
            width: percent(100),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(root),
    ));
    commands.spawn((
        Text::new("// SQUAD-LEVEL TURN-BASED TACTICS"),
        text_font(15.0),
        TextColor(Color::srgb(0.5, 0.62, 0.72)),
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: px(214),
            width: percent(100),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(root),
    ));
    let menu = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(312),
                width: percent(100),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(14),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    spawn_action_button(
        &mut commands,
        menu,
        CampaignUiAction::NewGame,
        "NEW GAME",
        true,
        Node {
            width: px(300),
            height: px(50),
            ..default()
        },
    );
    spawn_action_button(
        &mut commands,
        menu,
        CampaignUiAction::Continue,
        "CONTINUE",
        continue_enabled,
        Node {
            width: px(300),
            height: px(50),
            ..default()
        },
    );
    commands.spawn((
        Text::new(status.0.clone()),
        text_font(14.0),
        TextColor(Color::srgb(1.0, 0.42, 0.36)),
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(36),
            width: percent(100),
            ..default()
        },
        CampaignStatusText,
        Pickable::IGNORE,
        ChildOf(root),
    ));
}

/// Aftermath reward panel contents, rendered from the persisted receipt only.
pub fn aftermath_reward_copy(receipt: Option<CompletionReceipt>) -> String {
    receipt.map_or_else(String::new, |receipt| {
        format!(
            "MISSION REWARD\nBase {}\nBonus +{}\nTotal {}\nCredits {}",
            receipt.base_reward,
            receipt.optional_reward,
            receipt.total_reward,
            receipt.credits_after
        )
    })
}

/// NextMission handoff copy; the heading follows the runtime's next mission —
/// authored missions and the terminal Mission 7 handoff alike announce the unlock.
/// `Vanguard/Gunner/Interceptor` lines
/// list each mech's HP/ARMOR/MOBILITY/WEAPON levels from the persisted state.
pub fn next_mission_copy(state: &CampaignState) -> String {
    let levels = |mech: PlayerMech| {
        let l = state.upgrades.levels(mech);
        format!("{} {} {} {}", l.hp, l.armor, l.mobility, l.weapon)
    };
    let heading = format!("MISSION {} UNLOCKED", state.next_mission);
    format!(
        "{heading}\n\nCampaign progress saved.\n\nCredits: {}\n\nVanguard {}\nGunner {}\nInterceptor {}\n\nHP / ARMOR / MOBILITY / WEAPON",
        state.credits,
        levels(PlayerMech::Vanguard),
        levels(PlayerMech::Gunner),
        levels(PlayerMech::Interceptor),
    )
}

/// One upgrade row's `level / current effect / next effect / cost / MAX` text,
/// read from the persisted campaign state and `UPGRADE_COSTS`.
pub fn upgrade_row_copy(state: &CampaignState, mech: PlayerMech, track: UpgradeTrack) -> String {
    let level = state.upgrades.levels(mech).level(track);
    let maxed = level >= 3;
    let next = if maxed {
        "MAX".to_owned()
    } else {
        track_effect(track, level + 1)
    };
    let cost = if maxed {
        "MAX".to_owned()
    } else {
        format!("{} CR", UPGRADE_COSTS[level as usize])
    };
    format!(
        "{}   LV {}   {}  ->  {}   {}",
        track_label(track),
        level,
        track_effect(track, level),
        next,
        cost
    )
}

fn track_label(track: UpgradeTrack) -> &'static str {
    match track {
        UpgradeTrack::Hp => "HP",
        UpgradeTrack::Armor => "ARMOR",
        UpgradeTrack::Mobility => "MOBILITY",
        UpgradeTrack::Weapon => "WEAPON",
    }
}

fn track_effect(track: UpgradeTrack, level: u8) -> String {
    match track {
        UpgradeTrack::Hp => format!("+{} MAX HP", 3 * u32::from(level)),
        UpgradeTrack::Armor => format!("+{} ARMOR", level),
        UpgradeTrack::Mobility => format!("+{} EVASION", 5 * u32::from(level)),
        UpgradeTrack::Weapon => format!("+{} WEAPON DMG", level),
    }
}

/// Display-only affordability check; `persist_purchase` owns the real rules.
fn purchase_enabled(state: &CampaignState, mech: PlayerMech, track: UpgradeTrack) -> bool {
    let level = state.upgrades.levels(mech).level(track);
    level < 3 && state.credits >= UPGRADE_COSTS[level as usize]
}

#[derive(Component)]
pub struct UpgradeCreditsText;

#[derive(Component, Clone, Copy)]
pub struct UpgradeRow(pub PlayerMech, pub UpgradeTrack);

const MECHS: [(PlayerMech, &str); 3] = [
    (PlayerMech::Vanguard, "VANGUARD"),
    (PlayerMech::Gunner, "GUNNER"),
    (PlayerMech::Interceptor, "INTERCEPTOR"),
];

const TRACKS: [UpgradeTrack; 4] = [
    UpgradeTrack::Hp,
    UpgradeTrack::Armor,
    UpgradeTrack::Mobility,
    UpgradeTrack::Weapon,
];

pub fn setup_aftermath_screen(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    runtime: Res<CampaignRuntime>,
    active_mission: Res<ActiveMission>,
    mut cursor: ResMut<DialogueCursor>,
) {
    *cursor = DialogueCursor(0);
    let root = spawn_dialogue_screen(
        &mut commands,
        &asset_server,
        &active_mission.0.aftermath,
        CampaignUiAction::AdvanceAftermath,
    );
    commands.spawn((
        Text::new(aftermath_reward_copy(runtime.0.last_completion)),
        text_font(15.0),
        TextColor(Color::srgb(0.78, 0.92, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: px(28),
            right: px(28),
            width: px(300),
            padding: UiRect::all(px(14)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.012, 0.02, 0.035, 0.82)),
        Pickable::IGNORE,
        ChildOf(root),
    ));
}

pub fn setup_upgrade_screen(mut commands: Commands, mut status: ResMut<CampaignStatus>) {
    status.0.clear();
    commands.spawn(Camera2d);
    let root = commands
        .spawn((
            Name::new("Upgrade Screen"),
            ScreenRoot,
            fullscreen_node(),
            BackgroundColor(Color::srgb(0.014, 0.02, 0.032)),
            Pickable::IGNORE,
        ))
        .id();
    commands.spawn((
        Text::new("// HANGAR — SQUAD UPGRADES"),
        text_font(24.0),
        TextColor(Color::srgb(1.0, 0.82, 0.46)),
        Node {
            position_type: PositionType::Absolute,
            left: px(28),
            top: px(24),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(root),
    ));
    commands.spawn((
        Text::new(String::new()),
        text_font(20.0),
        TextColor(Color::srgb(0.78, 0.92, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            right: px(28),
            top: px(24),
            ..default()
        },
        UpgradeCreditsText,
        Pickable::IGNORE,
        ChildOf(root),
    ));
    let rows = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(28),
                top: px(84),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(18),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    for (mech, mech_label) in MECHS {
        commands.spawn((
            Text::new(mech_label),
            text_font(18.0),
            TextColor(Color::srgb(0.82, 0.94, 1.0)),
            Pickable::IGNORE,
            ChildOf(rows),
        ));
        let mech_rows = commands
            .spawn((
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6),
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(rows),
            ))
            .id();
        for track in TRACKS {
            let row = commands
                .spawn((
                    Node {
                        display: Display::Flex,
                        flex_direction: FlexDirection::Row,
                        column_gap: px(14),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    Pickable::IGNORE,
                    ChildOf(mech_rows),
                ))
                .id();
            commands.spawn((
                Text::new(String::new()),
                text_font(14.0),
                TextColor(Color::srgb(0.85, 0.9, 0.95)),
                Node {
                    width: px(520),
                    ..default()
                },
                UpgradeRow(mech, track),
                Pickable::IGNORE,
                ChildOf(row),
            ));
            spawn_action_button(
                &mut commands,
                row,
                CampaignUiAction::PurchaseUpgrade(mech, track),
                "BUY",
                true,
                Node {
                    width: px(90),
                    height: px(30),
                    ..default()
                },
            );
        }
    }
    spawn_action_button(
        &mut commands,
        root,
        CampaignUiAction::Proceed,
        "PROCEED",
        true,
        Node {
            position_type: PositionType::Absolute,
            left: px(28),
            bottom: px(36),
            width: px(260),
            height: px(52),
            ..default()
        },
    );
    commands.spawn((
        Text::new(String::new()),
        text_font(14.0),
        TextColor(Color::srgb(1.0, 0.42, 0.36)),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(36),
            left: px(320),
            width: px(900),
            ..default()
        },
        CampaignStatusText,
        Pickable::IGNORE,
        ChildOf(root),
    ));
}

/// Re-read the persisted campaign state whenever it changes: rows, credits,
/// and purchase affordability never hold UI-local copies, so a failed
/// purchase leaves the display unchanged.
#[allow(clippy::type_complexity)]
pub fn update_upgrade_screen(
    runtime: Res<CampaignRuntime>,
    mut rows: Query<
        (&UpgradeRow, &mut Text),
        (Without<UpgradeCreditsText>, Without<CampaignStatusText>),
    >,
    mut credits: Single<&mut Text, (With<UpgradeCreditsText>, Without<CampaignStatusText>)>,
    mut buttons: Query<
        (&CampaignUiAction, &mut BackgroundColor, &mut Pickable),
        Without<UpgradeRow>,
    >,
) {
    let Some(state) = runtime.0.state.as_ref() else {
        return;
    };
    credits.0 = format!("CREDITS {}", state.credits);
    for (row, mut text) in &mut rows {
        text.0 = upgrade_row_copy(state, row.0, row.1);
    }
    for (action, mut background, mut pickable) in &mut buttons {
        let CampaignUiAction::PurchaseUpgrade(mech, track) = *action else {
            continue;
        };
        let enabled = purchase_enabled(state, mech, track);
        background.0 = if enabled {
            Color::srgb(0.07, 0.22, 0.3)
        } else {
            Color::srgb(0.045, 0.055, 0.065)
        };
        *pickable = if enabled {
            Pickable::default()
        } else {
            Pickable::IGNORE
        };
    }
}

pub fn setup_next_mission_screen(mut commands: Commands, runtime: Res<CampaignRuntime>) {
    commands.spawn(Camera2d);
    let root = commands
        .spawn((
            Name::new("Next Mission Screen"),
            ScreenRoot,
            fullscreen_node(),
            BackgroundColor(Color::srgb(0.012, 0.016, 0.028)),
            Pickable::IGNORE,
        ))
        .id();
    commands.spawn((
        Text::new(
            runtime
                .0
                .state
                .as_ref()
                .map_or_else(String::new, next_mission_copy),
        ),
        text_font(20.0),
        TextColor(Color::srgb(0.85, 0.9, 0.95)),
        Node {
            position_type: PositionType::Absolute,
            left: px(28),
            top: px(28),
            width: px(820),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(root),
    ));
    spawn_action_button(
        &mut commands,
        root,
        CampaignUiAction::ReturnToTitle,
        "RETURN TO TITLE",
        true,
        Node {
            position_type: PositionType::Absolute,
            left: px(28),
            bottom: px(36),
            width: px(260),
            height: px(52),
            ..default()
        },
    );
}

pub fn setup_pre_mission_story(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    runtime: Res<CampaignRuntime>,
    mut cursor: ResMut<DialogueCursor>,
) {
    *cursor = DialogueCursor(0);
    let Some(definition) = active_definition(&runtime) else {
        return;
    };
    spawn_dialogue_screen(
        &mut commands,
        &asset_server,
        &definition.pre_mission,
        CampaignUiAction::AdvanceDialogue,
    );
}

pub fn setup_briefing_screen(mut commands: Commands, runtime: Res<CampaignRuntime>) {
    let Some(definition) = active_definition(&runtime) else {
        return;
    };
    commands.spawn(Camera2d);
    let root = commands
        .spawn((
            Name::new("Briefing Screen"),
            ScreenRoot,
            fullscreen_node(),
            BackgroundColor(Color::srgb(0.014, 0.02, 0.032)),
            Pickable::IGNORE,
        ))
        .id();
    commands.spawn((
        Text::new("// MISSION BRIEFING"),
        text_font(22.0),
        TextColor(Color::srgb(1.0, 0.82, 0.46)),
        Node {
            position_type: PositionType::Absolute,
            left: px(28),
            top: px(28),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(root),
    ));
    commands.spawn((
        Text::new(briefing_copy(definition)),
        text_font(17.0),
        TextColor(Color::srgb(0.85, 0.9, 0.95)),
        Node {
            position_type: PositionType::Absolute,
            left: px(28),
            top: px(96),
            width: px(820),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(root),
    ));
    spawn_action_button(
        &mut commands,
        root,
        CampaignUiAction::StartMission,
        "START MISSION",
        true,
        Node {
            position_type: PositionType::Absolute,
            left: px(28),
            bottom: px(36),
            width: px(260),
            height: px(52),
            ..default()
        },
    );
}

/// Shared campaign-screen cleanup: despawn the leaving screen's UI root and its
/// 2D camera. Never touches `PresentationRoot` or the battle camera.
#[allow(clippy::type_complexity)]
pub fn despawn_campaign_screen(
    mut commands: Commands,
    screens: Query<Entity, Or<(With<ScreenRoot>, With<Camera2d>)>>,
) {
    for entity in &screens {
        commands.entity(entity).try_despawn();
    }
}

/// Pure campaign-action routing shared by the button observer and tests:
/// Title actions go through the unified `FlowError` API, dialogue advancing
/// walks the active scene, purchases persist via the session, and
/// PROCEED/RETURN only change `GameScreen`.
pub fn apply_campaign_action(
    action: CampaignUiAction,
    runtime: &mut CampaignRuntime,
    active_mission: Option<&ActiveMission>,
    cursor: &mut DialogueCursor,
    status: &mut CampaignStatus,
    next_state: &mut NextState<GameScreen>,
) {
    match action {
        CampaignUiAction::NewGame => match start_new_game(&mut runtime.0) {
            Ok(()) => next_state.set(GameScreen::PreMissionStory),
            Err(error) => status.0 = error.to_string(),
        },
        CampaignUiAction::Continue => match continue_game(&mut runtime.0) {
            Ok(MissionId::One) => next_state.set(GameScreen::PreMissionStory),
            Ok(id) => next_state.set(if mission_definition(id).is_some() {
                GameScreen::Upgrade
            } else {
                GameScreen::NextMission
            }),
            Err(error) => status.0 = error.to_string(),
        },
        CampaignUiAction::AdvanceDialogue => {
            let line_count = active_definition(runtime)
                .map_or(0, |definition| definition.pre_mission.lines.len());
            advance_dialogue(cursor, line_count, GameScreen::Briefing, next_state);
        }
        CampaignUiAction::StartMission => next_state.set(GameScreen::Battle),
        CampaignUiAction::AdvanceAftermath => {
            // Aftermath walks `ActiveMission` — after completion the runtime
            // already points at the unlocked mission, which has no definition.
            let Some(mission) = active_mission else {
                return;
            };
            advance_dialogue(
                cursor,
                mission.0.aftermath.lines.len(),
                GameScreen::Upgrade,
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
            // Authored next mission: straight into its pre-mission story.
            // Seven is the terminal handoff state.
            let authored = runtime
                .0
                .state
                .as_ref()
                .is_some_and(|state| mission_definition(state.next_mission).is_some());
            next_state.set(if authored {
                GameScreen::PreMissionStory
            } else {
                GameScreen::NextMission
            });
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

fn on_campaign_ui_click(
    click: On<Pointer<Click>>,
    actions: Query<&CampaignUiAction>,
    mut runtime: ResMut<CampaignRuntime>,
    active_mission: Option<Res<ActiveMission>>,
    mut cursor: ResMut<DialogueCursor>,
    mut status: ResMut<CampaignStatus>,
    mut next_state: ResMut<NextState<GameScreen>>,
) {
    let Ok(action) = actions.get(click.entity) else {
        return;
    };
    apply_campaign_action(
        *action,
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
) {
    // Aftermath reads `ActiveMission` — the runtime has already advanced past
    // the completed mission — while pre-mission resolves `next_mission`.
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
}

pub fn update_campaign_status_text(
    status: Res<CampaignStatus>,
    mut text: Single<&mut Text, With<CampaignStatusText>>,
) {
    text.0 = status.0.clone();
}

fn active_definition(runtime: &CampaignRuntime) -> Option<&'static MissionDefinition> {
    let state = runtime.0.state.as_ref()?;
    mission_definition(state.next_mission)
}

fn spawn_action_button(
    commands: &mut Commands,
    parent: Entity,
    action: CampaignUiAction,
    label: &str,
    enabled: bool,
    node: Node,
) {
    let button = commands
        .spawn((
            Button,
            action,
            node,
            BackgroundColor(if enabled {
                Color::srgb(0.07, 0.22, 0.3)
            } else {
                Color::srgb(0.045, 0.055, 0.065)
            }),
            if enabled {
                Pickable::default()
            } else {
                Pickable::IGNORE
            },
            ChildOf(parent),
        ))
        .observe(on_campaign_ui_click)
        .id();
    commands.spawn((
        Text::new(label),
        text_font(17.0),
        TextColor(if enabled {
            Color::srgb(0.88, 0.94, 1.0)
        } else {
            Color::srgb(0.4, 0.45, 0.5)
        }),
        Pickable::IGNORE,
        ChildOf(button),
    ));
}

fn fullscreen_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: percent(100),
        height: percent(100),
        ..default()
    }
}

fn text_font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..default()
    }
}
