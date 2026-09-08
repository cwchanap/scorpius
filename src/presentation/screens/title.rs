use bevy::prelude::*;

use crate::campaign::session::FlowError;
use crate::presentation::{CampaignRuntime, CanvasRoot};

use super::super::{assets::UiAssets, campaign_ui::CampaignStatus};
use super::shared::{
    ensure_canvas, spawn_campaign_camera, spawn_icon, spawn_image_button_row, spawn_screen_root,
};

pub fn setup_title_screen(
    mut commands: Commands,
    runtime: Res<CampaignRuntime>,
    mut status: ResMut<CampaignStatus>,
    ui_assets: Res<UiAssets>,
    canvas_roots: Query<Entity, With<CanvasRoot>>,
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
    let canvas = ensure_canvas(&mut commands, &canvas_roots);
    spawn_campaign_camera(&mut commands);
    let root = spawn_screen_root(
        &mut commands,
        canvas,
        "Title Screen",
        Color::srgb_u8(5, 8, 15),
    );

    super::shared::spawn_image(
        &mut commands,
        root,
        ui_assets.key_art.clone(),
        super::shared::fullscreen_node(),
    );
    commands.spawn((
        super::shared::fullscreen_node(),
        BackgroundColor(Color::srgba(5.0 / 255.0, 8.0 / 255.0, 15.0 / 255.0, 0.62)),
        Pickable::IGNORE,
        ChildOf(root),
    ));

    let heading = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(196),
                width: percent(100),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(22),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    let wordmark = commands
        .spawn((
            Node {
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(26),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(heading),
        ))
        .id();
    spawn_icon(
        &mut commands,
        wordmark,
        &ui_assets.icons,
        crate::presentation::theme::UNIT_GLYPH_HEX_RECT,
        crate::presentation::theme::ACCENT,
        56.0,
    );
    commands.spawn((
        Text::new("SCORPIUS"),
        crate::presentation::theme::chakra_petch(&ui_assets.fonts, 132.0, FontWeight(700)),
        TextColor(Color::srgb_u8(238, 246, 255)),
        TextShadow {
            offset: Vec2::ZERO,
            color: Color::srgba(62.0 / 255.0, 199.0 / 255.0, 219.0 / 255.0, 0.45),
        },
        Pickable::IGNORE,
        ChildOf(wordmark),
    ));
    spawn_icon(
        &mut commands,
        wordmark,
        &ui_assets.icons,
        crate::presentation::theme::UNIT_GLYPH_HEX_RECT,
        crate::presentation::theme::ACCENT,
        56.0,
    );
    let ornament = commands
        .spawn((
            Node {
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(10),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(heading),
        ))
        .id();
    commands.spawn((
        Node {
            width: px(120),
            height: px(1),
            ..default()
        },
        BackgroundColor(crate::presentation::theme::ACCENT),
        Pickable::IGNORE,
        ChildOf(ornament),
    ));
    let ornament_icons = commands
        .spawn((
            Node {
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(10),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(ornament),
        ))
        .id();
    for _ in 0..3 {
        spawn_icon(
            &mut commands,
            ornament_icons,
            &ui_assets.icons,
            crate::presentation::theme::ICON_SKILL,
            Color::srgb_u8(159, 182, 201),
            26.0,
        );
    }
    commands.spawn((
        Node {
            width: px(120),
            height: px(1),
            ..default()
        },
        BackgroundColor(crate::presentation::theme::ACCENT),
        Pickable::IGNORE,
        ChildOf(ornament),
    ));

    let menu = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                bottom: px(172),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(18),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    spawn_image_button_row(
        &mut commands,
        menu,
        crate::presentation::campaign_ui::CampaignUiAction::NewGame,
        "NEW GAME",
        true,
        &ui_assets.fonts,
        &ui_assets.icons,
        crate::presentation::theme::ICON_MOVE,
        crate::presentation::theme::ACCENT,
        40.0,
        Node {
            width: px(420),
            height: px(88),
            ..default()
        },
        30.0,
        None,
    );
    spawn_image_button_row(
        &mut commands,
        menu,
        crate::presentation::campaign_ui::CampaignUiAction::Continue,
        "CONTINUE",
        continue_enabled,
        &ui_assets.fonts,
        &ui_assets.icons,
        crate::presentation::theme::ICON_BACK,
        Color::srgb_u8(143, 168, 189),
        40.0,
        Node {
            width: px(420),
            height: px(88),
            ..default()
        },
        30.0,
        Some((
            usize::from(continue_enabled),
            7,
            Color::srgb_u8(70, 105, 125),
            10.0,
            10.0,
        )),
    );
    commands.spawn((
        Text::new(status.0.clone()),
        crate::presentation::theme::ibm_plex_mono(&ui_assets.fonts, 14.0, FontWeight(400)),
        TextColor(crate::presentation::theme::ENEMY),
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            left: px(28),
            right: px(28),
            bottom: px(36),
            ..default()
        },
        super::super::campaign_ui::CampaignStatusText,
        Pickable::IGNORE,
        ChildOf(root),
    ));
}
