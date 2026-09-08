use bevy::prelude::*;

use crate::presentation::{CampaignCamera, CanvasRoot, UiPickingCamera};

use super::super::{
    campaign_ui::{
        CampaignUiAction, DialoguePip, DialoguePortrait, DialogueSpeaker, DialogueText, ScreenRoot,
    },
    theme,
};

pub(crate) fn ensure_canvas(
    commands: &mut Commands,
    canvas_roots: &Query<Entity, With<CanvasRoot>>,
) -> Entity {
    canvas_roots
        .iter()
        .next()
        .unwrap_or_else(|| crate::presentation::layout::spawn_canvas_root(commands))
}

pub(crate) fn spawn_campaign_camera(commands: &mut Commands) {
    commands.spawn((Camera2d, CampaignCamera, UiPickingCamera));
}

pub(crate) fn spawn_screen_root(
    commands: &mut Commands,
    canvas: Entity,
    name: &'static str,
    background: Color,
) -> Entity {
    commands
        .spawn((
            Name::new(name),
            ScreenRoot,
            fullscreen_node(),
            BackgroundColor(background),
            Pickable::IGNORE,
            ChildOf(canvas),
        ))
        .id()
}

pub(crate) fn fullscreen_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: percent(100),
        height: percent(100),
        ..default()
    }
}

pub(crate) fn spawn_image(
    commands: &mut Commands,
    parent: Entity,
    image: Handle<Image>,
    node: Node,
) -> Entity {
    commands
        .spawn((
            node,
            ImageNode::new(image),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id()
}

pub(crate) fn spawn_icon(
    commands: &mut Commands,
    parent: Entity,
    image: &Handle<Image>,
    rect: Rect,
    color: Color,
    size: f32,
) -> Entity {
    commands
        .spawn((
            Node {
                width: px(size),
                height: px(size),
                flex_shrink: 0.0,
                ..default()
            },
            theme::icon_node(image.clone(), rect, color),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id()
}

pub(crate) fn spawn_pip(
    commands: &mut Commands,
    parent: Entity,
    active: bool,
    color: Color,
    width: f32,
    height: f32,
) -> Entity {
    commands
        .spawn((
            Node {
                width: px(width),
                height: px(height),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(if active { color } else { theme::BORDER }),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_pips(
    commands: &mut Commands,
    parent: Entity,
    active: usize,
    total: usize,
    color: Color,
    width: f32,
    height: f32,
    gap: f32,
) {
    let row = commands
        .spawn((
            Node {
                display: Display::Flex,
                column_gap: px(gap),
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    for index in 0..total {
        spawn_pip(commands, row, index < active, color, width, height);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_dialogue_screen(
    commands: &mut Commands,
    asset_server: &AssetServer,
    scene: &crate::mission::DialogueScene,
    advance_action: CampaignUiAction,
    canvas: Entity,
    fonts: &theme::FontHandles,
    icons: &Handle<Image>,
    accent: Color,
    allow_skip: bool,
) -> Entity {
    spawn_campaign_camera(commands);
    let root = spawn_screen_root(commands, canvas, "Dialogue Screen", theme::BACKGROUND);
    let opening = crate::presentation::campaign_ui::dialogue_snapshot(
        scene,
        crate::presentation::campaign_ui::DialogueCursor(0),
    );

    spawn_image(
        commands,
        root,
        asset_server.load(scene.background),
        fullscreen_node(),
    );
    commands.spawn((
        fullscreen_node(),
        BackgroundColor(Color::srgba(5.0 / 255.0, 8.0 / 255.0, 15.0 / 255.0, 0.36)),
        Pickable::IGNORE,
        ChildOf(root),
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(56),
            bottom: px(300),
            width: px(300),
            height: px(300),
            border: UiRect::all(px(2)),
            ..default()
        },
        BorderColor::all(accent),
        ImageNode::new(asset_server.load(opening.portrait)),
        DialoguePortrait,
        Pickable::IGNORE,
        ChildOf(root),
    ));

    let dialogue = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(56),
                right: px(56),
                bottom: px(64),
                height: px(212),
                display: Display::Flex,
                ..default()
            },
            BackgroundColor(Color::srgba(4.0 / 255.0, 9.0 / 255.0, 17.0 / 255.0, 0.9)),
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        Node {
            width: px(12),
            height: percent(100),
            ..default()
        },
        BackgroundColor(accent),
        Pickable::IGNORE,
        ChildOf(dialogue),
    ));
    let content = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: px(0),
                padding: UiRect::axes(px(26), px(34)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(14),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(dialogue),
        ))
        .id();
    let heading = commands
        .spawn((
            Node {
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(16),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(content),
        ))
        .id();
    commands.spawn((
        Text::new(opening.speaker),
        theme::ibm_plex_mono(fonts, 26.0, FontWeight(600)),
        TextColor(theme::GOLD),
        DialogueSpeaker,
        Pickable::IGNORE,
        ChildOf(heading),
    ));
    commands.spawn((
        Node {
            flex_grow: 1.0,
            height: px(1),
            ..default()
        },
        BackgroundColor(theme::BORDER),
        Pickable::IGNORE,
        ChildOf(heading),
    ));
    let pips = commands
        .spawn((
            Node {
                display: Display::Flex,
                column_gap: px(7),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(heading),
        ))
        .id();
    for index in 0..scene.lines.len() {
        commands.spawn((
            Node {
                width: px(if index == 0 { 22.0 } else { 10.0 }),
                height: px(7),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(if index == 0 { accent } else { theme::BORDER }),
            DialoguePip(index),
            Pickable::IGNORE,
            ChildOf(pips),
        ));
    }
    commands.spawn((
        Text::new(opening.text),
        theme::chakra_petch(fonts, 29.0, FontWeight(400)),
        TextColor(theme::TEXT),
        Node {
            max_width: px(1320),
            ..default()
        },
        DialogueText,
        Pickable::IGNORE,
        ChildOf(content),
    ));
    spawn_icon_button(
        commands,
        dialogue,
        advance_action,
        true,
        icons,
        theme::ICON_BACK,
        accent,
        52.0,
        Node {
            width: px(160),
            height: percent(100),
            ..default()
        },
    );
    if allow_skip {
        spawn_icon_button(
            commands,
            root,
            CampaignUiAction::SkipDialogue,
            true,
            icons,
            theme::ICON_SKIP,
            theme::MUTED,
            24.0,
            Node {
                position_type: PositionType::Absolute,
                right: px(56),
                top: px(44),
                width: px(56),
                height: px(48),
                ..default()
            },
        );
    }
    root
}

pub(crate) fn spawn_button(
    commands: &mut Commands,
    parent: Entity,
    action: CampaignUiAction,
    enabled: bool,
    node: Node,
) -> Entity {
    commands
        .spawn((
            Button,
            action,
            node,
            BackgroundColor(if enabled {
                theme::PANEL_RAISED
            } else {
                Color::srgb_u8(11, 17, 24)
            }),
            if enabled {
                Pickable::default()
            } else {
                Pickable::IGNORE
            },
            ChildOf(parent),
        ))
        .observe(crate::presentation::campaign_ui::on_campaign_ui_click)
        .id()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_icon_button(
    commands: &mut Commands,
    parent: Entity,
    action: CampaignUiAction,
    enabled: bool,
    image: &Handle<Image>,
    rect: Rect,
    color: Color,
    size: f32,
    node: Node,
) -> Entity {
    let button = spawn_button(commands, parent, action, enabled, node);
    let content = commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(button),
        ))
        .id();
    spawn_icon(commands, content, image, rect, color, size);
    button
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_image_button_row(
    commands: &mut Commands,
    parent: Entity,
    action: CampaignUiAction,
    label: &'static str,
    enabled: bool,
    fonts: &theme::FontHandles,
    image: &Handle<Image>,
    rect: Rect,
    color: Color,
    icon_size: f32,
    node: Node,
    label_size: f32,
    trailing_pips: Option<(usize, usize, Color, f32, f32)>,
) -> Entity {
    let button = spawn_button(commands, parent, action, enabled, node);
    let content = commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::horizontal(px(30)),
                align_items: AlignItems::Center,
                column_gap: px(24),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(button),
        ))
        .id();
    spawn_icon(commands, content, image, rect, color, icon_size);
    commands.spawn((
        Text::new(label),
        theme::chakra_petch(fonts, label_size, FontWeight(600)),
        TextColor(if enabled { theme::TEXT } else { theme::MUTED }),
        Pickable::IGNORE,
        ChildOf(content),
    ));
    if let Some((active, total, pip_color, width, height)) = trailing_pips {
        let pips = commands
            .spawn((
                Node {
                    margin: UiRect::left(Val::Auto),
                    display: Display::Flex,
                    column_gap: px(5),
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(content),
            ))
            .id();
        for index in 0..total {
            spawn_pip(commands, pips, index < active, pip_color, width, height);
        }
    }
    button
}

pub(crate) fn track_icon(track: crate::campaign::model::UpgradeTrack) -> Rect {
    match track {
        crate::campaign::model::UpgradeTrack::Hp => theme::ICON_GUARD,
        crate::campaign::model::UpgradeTrack::Armor => theme::ICON_COUNTER,
        crate::campaign::model::UpgradeTrack::Mobility => theme::ICON_MOVE,
        crate::campaign::model::UpgradeTrack::Weapon => theme::ICON_ATTACK,
    }
}
