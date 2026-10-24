use bevy::{prelude::*, text::LetterSpacing};

use crate::presentation::{CampaignRuntime, CanvasRoot, assets::UiAssets};

use super::super::{
    campaign_ui::{
        CampaignStatus, CampaignStatusText, CampaignUiAction, MissionSelectRow, mission_select_rows,
    },
    theme,
};
use super::shared::{ensure_canvas, spawn_button, spawn_icon, spawn_screen_root};

/// Mission Select: one row per mission, where a cleared mission replays and a
/// locked one stays inert. Rows come from the save that Title's MISSIONS
/// button loaded.
pub fn setup_mission_select_screen(
    mut commands: Commands,
    runtime: Res<CampaignRuntime>,
    status: Res<CampaignStatus>,
    ui_assets: Res<UiAssets>,
    canvas_roots: Query<Entity, With<CanvasRoot>>,
) {
    let Some(state) = runtime.0.state.as_ref() else {
        return;
    };
    let rows = mission_select_rows(state);
    let canvas = ensure_canvas(&mut commands, &canvas_roots);
    super::shared::spawn_campaign_camera(&mut commands);
    let root = spawn_screen_root(
        &mut commands,
        canvas,
        "Mission Select Screen",
        Color::srgb_u8(6, 10, 18),
    );

    let header = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                height: px(120),
                padding: UiRect::horizontal(px(56)),
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(28),
                border: UiRect::bottom(px(1)),
                ..default()
            },
            BorderColor::all(theme::BORDER),
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    spawn_icon(
        &mut commands,
        header,
        &ui_assets.icons,
        theme::BRIEFING_DEPLOY_RECT,
        Color::WHITE,
        48.0,
    );
    let title = commands
        .spawn((
            Node {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(header),
        ))
        .id();
    commands.spawn((
        Text::new("MISSIONS"),
        theme::chakra_petch(&ui_assets.fonts, 44.0, FontWeight(700)),
        LetterSpacing::Px(8.8),
        TextColor(theme::TEXT),
        Pickable::IGNORE,
        ChildOf(title),
    ));
    commands.spawn((
        Text::new("Replay a cleared mission. Replays pay no credits and keep your progress."),
        theme::ibm_plex_mono(&ui_assets.fonts, 16.0, FontWeight(400)),
        TextColor(theme::MUTED),
        Pickable::IGNORE,
        ChildOf(title),
    ));

    let list = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(152),
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
    for row in rows {
        spawn_mission_row(&mut commands, list, &ui_assets, row);
    }

    let back = spawn_button(
        &mut commands,
        root,
        CampaignUiAction::ReturnToTitle,
        true,
        Node {
            position_type: PositionType::Absolute,
            left: px(56),
            bottom: px(40),
            width: px(260),
            height: px(72),
            display: Display::Flex,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: px(16),
            border: UiRect::all(px(1)),
            ..default()
        },
    );
    spawn_icon(
        &mut commands,
        back,
        &ui_assets.icons,
        theme::ENDING_RETURN_RECT,
        Color::WHITE,
        28.0,
    );
    commands.spawn((
        Text::new("TITLE"),
        theme::chakra_petch(&ui_assets.fonts, 24.0, FontWeight(600)),
        LetterSpacing::Px(4.8),
        TextColor(theme::TEXT),
        Pickable::IGNORE,
        ChildOf(back),
    ));
    commands.spawn((
        Text::new(status.0.clone()),
        theme::ibm_plex_mono(&ui_assets.fonts, 14.0, FontWeight(400)),
        TextColor(theme::ENEMY),
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            left: px(360),
            right: px(360),
            bottom: px(66),
            ..default()
        },
        CampaignStatusText,
        Pickable::IGNORE,
        ChildOf(root),
    ));
}

fn spawn_mission_row(
    commands: &mut Commands,
    parent: Entity,
    assets: &UiAssets,
    row: MissionSelectRow,
) {
    let button = spawn_button(
        commands,
        parent,
        CampaignUiAction::ReplayMission(row.mission),
        row.cleared,
        Node {
            width: px(1100),
            height: px(92),
            padding: UiRect::horizontal(px(32)),
            display: Display::Flex,
            align_items: AlignItems::Center,
            column_gap: px(28),
            border: UiRect::all(px(1)),
            ..default()
        },
    );
    commands
        .entity(button)
        .insert(BorderColor::all(theme::BORDER));
    commands.spawn((
        Text::new(format!("{:02}", row.mission)),
        theme::ibm_plex_mono(&assets.fonts, 40.0, FontWeight(600)),
        TextColor(if row.cleared {
            theme::ACCENT
        } else {
            theme::BORDER
        }),
        Pickable::IGNORE,
        ChildOf(button),
    ));
    commands.spawn((
        Text::new(row.title),
        theme::chakra_petch(&assets.fonts, 30.0, FontWeight(600)),
        LetterSpacing::Px(3.0),
        TextColor(if row.cleared {
            theme::TEXT
        } else {
            theme::MUTED
        }),
        Pickable::IGNORE,
        ChildOf(button),
    ));
    let badge = commands
        .spawn((
            Node {
                margin: UiRect::left(Val::Auto),
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(10),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(button),
        ))
        .id();
    spawn_icon(
        commands,
        badge,
        &assets.icons,
        if row.cleared {
            theme::ICON_WAIT
        } else {
            theme::LOCKED_RECT
        },
        if row.cleared {
            theme::MINT
        } else {
            theme::MUTED
        },
        24.0,
    );
    commands.spawn((
        Text::new(if row.cleared { "CLEARED" } else { "LOCKED" }),
        theme::ibm_plex_mono(&assets.fonts, 16.0, FontWeight(400)),
        LetterSpacing::Px(3.2),
        TextColor(if row.cleared {
            theme::MINT
        } else {
            theme::MUTED
        }),
        Pickable::IGNORE,
        ChildOf(badge),
    ));
}
