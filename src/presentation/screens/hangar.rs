use bevy::prelude::*;

use crate::campaign::model::{PlayerMech, UpgradeTrack};
use crate::presentation::{CampaignRuntime, CanvasRoot, assets::UiAssets};

use super::super::{
    campaign_ui::{
        CampaignStatus, CampaignStatusText, CampaignUiAction, MECHS, TRACKS, UpgradeCreditsText,
        UpgradePip, UpgradeRow,
    },
    theme,
};
use super::shared::{ensure_canvas, spawn_button, spawn_icon, spawn_screen_root};

pub fn setup_upgrade_screen(
    mut commands: Commands,
    runtime: Res<CampaignRuntime>,
    mut status: ResMut<CampaignStatus>,
    ui_assets: Res<UiAssets>,
    canvas_roots: Query<Entity, With<CanvasRoot>>,
) {
    status.0.clear();
    let canvas = ensure_canvas(&mut commands, &canvas_roots);
    super::shared::spawn_campaign_camera(&mut commands);
    let root = spawn_screen_root(
        &mut commands,
        canvas,
        "Hangar Screen",
        Color::srgb_u8(5, 8, 15),
    );

    let header = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(48),
                right: px(48),
                top: px(36),
                height: px(64),
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(22),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    spawn_icon(
        &mut commands,
        header,
        &ui_assets.icons,
        theme::ICON_GUARD,
        theme::ACCENT,
        42.0,
    );
    commands.spawn((
        Text::new("HANGAR"),
        theme::chakra_petch(&ui_assets.fonts, 36.0, FontWeight(600)),
        TextColor(theme::TEXT),
        Pickable::IGNORE,
        ChildOf(header),
    ));
    let credits = commands
        .spawn((
            Node {
                margin: UiRect::left(Val::Auto),
                padding: UiRect::axes(px(16), px(24)),
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(14),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(theme::PANEL),
            BorderColor::all(Color::srgb_u8(51, 48, 28)),
            Pickable::IGNORE,
            ChildOf(header),
        ))
        .id();
    spawn_icon(
        &mut commands,
        credits,
        &ui_assets.icons,
        theme::ICON_SKILL,
        theme::GOLD,
        30.0,
    );
    commands.spawn((
        Text::new(
            runtime
                .0
                .state
                .as_ref()
                .map_or_else(|| "0".to_owned(), |state| state.credits.to_string()),
        ),
        theme::ibm_plex_mono(&ui_assets.fonts, 36.0, FontWeight(600)),
        TextColor(theme::GOLD),
        UpgradeCreditsText,
        Pickable::IGNORE,
        ChildOf(credits),
    ));

    let columns = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(48),
                right: px(48),
                top: px(126),
                bottom: px(150),
                display: Display::Flex,
                column_gap: px(24),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    for (mech, label) in MECHS {
        spawn_mech_column(&mut commands, columns, &ui_assets, mech, label);
    }

    let next = spawn_button(
        &mut commands,
        root,
        CampaignUiAction::Proceed,
        true,
        Node {
            position_type: PositionType::Absolute,
            left: px(48),
            right: px(48),
            bottom: px(32),
            height: px(92),
            display: Display::Flex,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: UiRect::all(px(1)),
            ..default()
        },
    );
    spawn_icon(
        &mut commands,
        next,
        &ui_assets.icons,
        theme::ICON_BACK,
        theme::ACCENT,
        36.0,
    );
    commands.spawn((
        Text::new("NEXT DROP"),
        theme::chakra_petch(&ui_assets.fonts, 28.0, FontWeight(600)),
        TextColor(theme::TEXT),
        Node {
            margin: UiRect::left(px(20)),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(next),
    ));
    commands.spawn((
        Text::new(String::new()),
        theme::ibm_plex_mono(&ui_assets.fonts, 14.0, FontWeight(400)),
        TextColor(theme::ENEMY),
        Node {
            position_type: PositionType::Absolute,
            left: px(48),
            bottom: px(8),
            ..default()
        },
        CampaignStatusText,
        Pickable::IGNORE,
        ChildOf(root),
    ));
}

fn spawn_mech_column(
    commands: &mut Commands,
    parent: Entity,
    assets: &UiAssets,
    mech: PlayerMech,
    label: &'static str,
) {
    let column = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: px(0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(theme::PANEL),
            BorderColor::all(Color::srgb_u8(22, 40, 58)),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    let (art, glyph) = match mech {
        PlayerMech::Vanguard => (assets.vanguard_art.clone(), theme::UNIT_GLYPH_HEX_RECT),
        PlayerMech::Gunner => (assets.gunner_art.clone(), theme::UNIT_GLYPH_DIAMOND_RECT),
        PlayerMech::Interceptor => (
            assets.interceptor_art.clone(),
            theme::UNIT_GLYPH_TRIANGLE_RECT,
        ),
    };
    let art_panel = commands
        .spawn((
            Node {
                position_type: PositionType::Relative,
                height: px(300),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::srgb_u8(6, 12, 21)),
            Pickable::IGNORE,
            ChildOf(column),
        ))
        .id();
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            ..default()
        },
        ImageNode::new(art),
        Pickable::IGNORE,
        ChildOf(art_panel),
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            bottom: px(0),
            height: px(130),
            ..default()
        },
        BackgroundColor(Color::srgba(10.0 / 255.0, 20.0 / 255.0, 32.0 / 255.0, 0.9)),
        Pickable::IGNORE,
        ChildOf(art_panel),
    ));
    let name_row = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(20),
                right: px(20),
                bottom: px(16),
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(12),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(art_panel),
        ))
        .id();
    spawn_icon(
        commands,
        name_row,
        &assets.icons,
        glyph,
        theme::ACCENT,
        32.0,
    );
    commands.spawn((
        Text::new(label),
        theme::chakra_petch(&assets.fonts, 26.0, FontWeight(600)),
        TextColor(theme::TEXT),
        Pickable::IGNORE,
        ChildOf(name_row),
    ));

    let tracks = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                padding: UiRect::all(px(20)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(14),
                min_height: px(0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(column),
        ))
        .id();
    for track in TRACKS {
        spawn_upgrade_row(commands, tracks, assets, mech, track);
    }
}

fn spawn_upgrade_row(
    commands: &mut Commands,
    parent: Entity,
    assets: &UiAssets,
    mech: PlayerMech,
    track: UpgradeTrack,
) {
    let row = commands
        .spawn((
            Node {
                min_height: px(62),
                padding: UiRect::all(px(14)),
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(14),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(13, 24, 38)),
            BorderColor::all(Color::srgb_u8(26, 44, 60)),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    spawn_icon(
        commands,
        row,
        &assets.icons,
        super::shared::track_icon(track),
        theme::ACCENT,
        28.0,
    );
    let data = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: px(0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(8),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(row),
        ))
        .id();
    let pips = commands
        .spawn((
            Node {
                display: Display::Flex,
                column_gap: px(5),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(data),
        ))
        .id();
    for index in 0..3 {
        commands.spawn((
            Node {
                width: px(10),
                height: px(10),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(if index == 0 {
                theme::ACCENT
            } else {
                theme::BORDER
            }),
            UpgradePip { mech, track, index },
            Pickable::IGNORE,
            ChildOf(pips),
        ));
    }
    commands.spawn((
        Text::new(String::new()),
        theme::ibm_plex_mono(&assets.fonts, 14.0, FontWeight(400)),
        TextColor(theme::TEXT),
        UpgradeRow(mech, track),
        Pickable::IGNORE,
        ChildOf(data),
    ));
    let action = CampaignUiAction::PurchaseUpgrade(mech, track);
    let button = spawn_button(
        commands,
        row,
        action,
        true,
        Node {
            min_width: px(100),
            height: px(42),
            padding: UiRect::horizontal(px(10)),
            display: Display::Flex,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: px(8),
            ..default()
        },
    );
    spawn_icon(
        commands,
        button,
        &assets.icons,
        theme::ICON_SKILL,
        theme::GOLD,
        20.0,
    );
    commands.spawn((
        Text::new("BUY"),
        theme::ibm_plex_mono(&assets.fonts, 15.0, FontWeight(600)),
        TextColor(theme::TEXT),
        Pickable::IGNORE,
        ChildOf(button),
    ));
}
