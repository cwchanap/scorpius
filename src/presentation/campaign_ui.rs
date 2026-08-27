//! Title, pre-mission story, and briefing screens for the campaign loop.
//!
//! Every screen here roots under [`ScreenRoot`], deliberately separate from
//! [`super::PresentationRoot`]: leaving a campaign screen despawns only its own
//! UI and 2D camera, never the 3D battlefield.

use bevy::prelude::*;

use crate::app::GameScreen;
use crate::campaign::session::{FlowError, continue_game, start_new_game};
use crate::mission::{DialogueScene, MissionDefinition, MissionId, mission_definition};
use crate::presentation::CampaignRuntime;

/// Root of a campaign-flow screen (Title / pre-mission story / briefing, and
/// the aftermath, upgrade, and next-mission screens to come).
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
) {
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
/// walks the pre-mission scene, and START MISSION sets Battle only.
pub fn apply_campaign_action(
    action: CampaignUiAction,
    runtime: &mut CampaignRuntime,
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
            Ok(MissionId::Two) => next_state.set(GameScreen::Upgrade),
            Err(error) => status.0 = error.to_string(),
        },
        CampaignUiAction::AdvanceDialogue => {
            let line_count = active_definition(runtime)
                .map_or(0, |definition| definition.pre_mission.lines.len());
            if cursor.0 + 1 < line_count {
                cursor.0 += 1;
            } else {
                cursor.0 = line_count.saturating_sub(1);
                next_state.set(GameScreen::Briefing);
            }
        }
        CampaignUiAction::StartMission => next_state.set(GameScreen::Battle),
    }
}

fn on_campaign_ui_click(
    click: On<Pointer<Click>>,
    actions: Query<&CampaignUiAction>,
    mut runtime: ResMut<CampaignRuntime>,
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
        &mut cursor,
        &mut status,
        &mut next_state,
    );
}

pub fn update_dialogue_screen(
    runtime: Res<CampaignRuntime>,
    cursor: Res<DialogueCursor>,
    asset_server: Res<AssetServer>,
    mut portrait: Single<&mut ImageNode, With<DialoguePortrait>>,
    mut speaker: Single<&mut Text, (With<DialogueSpeaker>, Without<DialogueText>)>,
    mut text: Single<&mut Text, (With<DialogueText>, Without<DialogueSpeaker>)>,
) {
    let Some(definition) = active_definition(&runtime) else {
        return;
    };
    let snapshot = dialogue_snapshot(&definition.pre_mission, *cursor);
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
