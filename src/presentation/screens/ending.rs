use bevy::prelude::*;

use crate::campaign::model::{PlayerMech, UpgradeLevels};
use crate::presentation::{CampaignRuntime, CanvasRoot, assets::UiAssets};

use super::super::{
    campaign_ui::{CampaignUiAction, MECHS, TRACKS, ending_snapshot},
    theme,
};
use super::shared::{
    ensure_canvas, spawn_button, spawn_icon, spawn_pip, spawn_pips, spawn_screen_root,
};

pub fn setup_ending_screen(
    mut commands: Commands,
    runtime: Res<CampaignRuntime>,
    ui_assets: Res<UiAssets>,
    canvas_roots: Query<Entity, With<CanvasRoot>>,
) {
    let Some(state) = runtime.0.state.as_ref() else {
        return;
    };
    let snapshot = ending_snapshot(state);
    let canvas = ensure_canvas(&mut commands, &canvas_roots);
    super::shared::spawn_campaign_camera(&mut commands);
    let root = spawn_screen_root(
        &mut commands,
        canvas,
        "Ending Screen",
        Color::srgb_u8(4, 7, 13),
    );
    let content = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(52),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    let heading = commands
        .spawn((
            Node {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(24),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(content),
        ))
        .id();
    spawn_icon(
        &mut commands,
        heading,
        &ui_assets.icons,
        theme::UNIT_GLYPH_HEX_RECT,
        theme::ACCENT,
        96.0,
    );
    commands.spawn((
        Text::new("CAMPAIGN"),
        theme::chakra_petch(&ui_assets.fonts, 76.0, FontWeight(700)),
        TextColor(Color::srgb_u8(238, 246, 255)),
        TextShadow {
            offset: Vec2::ZERO,
            color: Color::srgba(62.0 / 255.0, 199.0 / 255.0, 219.0 / 255.0, 0.4),
        },
        Pickable::IGNORE,
        ChildOf(heading),
    ));
    spawn_pips(
        &mut commands,
        heading,
        7,
        7,
        theme::ACCENT,
        10.0,
        10.0,
        10.0,
    );

    let cards = commands
        .spawn((
            Node {
                display: Display::Flex,
                column_gap: px(24),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(content),
        ))
        .id();
    for (mech, label) in MECHS {
        let levels = match mech {
            PlayerMech::Vanguard => snapshot.vanguard,
            PlayerMech::Gunner => snapshot.gunner,
            PlayerMech::Interceptor => snapshot.interceptor,
        };
        spawn_mech_card(&mut commands, cards, &ui_assets, mech, label, levels);
    }

    let button = spawn_button(
        &mut commands,
        content,
        CampaignUiAction::ReturnToTitle,
        true,
        Node {
            width: px(420),
            height: px(88),
            display: Display::Flex,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: px(20),
            border: UiRect::all(px(1)),
            ..default()
        },
    );
    spawn_icon(
        &mut commands,
        button,
        &ui_assets.icons,
        theme::ICON_BACK,
        theme::ACCENT,
        34.0,
    );
    commands.spawn((
        Text::new("TITLE"),
        theme::chakra_petch(&ui_assets.fonts, 26.0, FontWeight(600)),
        TextColor(theme::TEXT),
        Pickable::IGNORE,
        ChildOf(button),
    ));
}

fn spawn_mech_card(
    commands: &mut Commands,
    parent: Entity,
    assets: &UiAssets,
    mech: PlayerMech,
    label: &'static str,
    levels: UpgradeLevels,
) {
    let card = commands
        .spawn((
            Node {
                width: px(360),
                padding: UiRect::all(px(24)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(18),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(theme::PANEL),
            BorderColor::all(Color::srgb_u8(22, 40, 58)),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    let glyph = match mech {
        PlayerMech::Vanguard => theme::UNIT_GLYPH_HEX_RECT,
        PlayerMech::Gunner => theme::UNIT_GLYPH_DIAMOND_RECT,
        PlayerMech::Interceptor => theme::UNIT_GLYPH_TRIANGLE_RECT,
    };
    let heading = commands
        .spawn((
            Node {
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(12),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(card),
        ))
        .id();
    spawn_icon(commands, heading, &assets.icons, glyph, theme::ACCENT, 30.0);
    commands.spawn((
        Text::new(label),
        theme::chakra_petch(&assets.fonts, 22.0, FontWeight(600)),
        TextColor(theme::TEXT),
        Pickable::IGNORE,
        ChildOf(heading),
    ));
    for track in TRACKS {
        let level = levels.level(track).min(3);
        let row = commands
            .spawn((
                Node {
                    display: Display::Flex,
                    align_items: AlignItems::Center,
                    column_gap: px(14),
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(card),
            ))
            .id();
        spawn_icon(
            commands,
            row,
            &assets.icons,
            super::shared::track_icon(track),
            theme::ACCENT,
            22.0,
        );
        let pips = commands
            .spawn((
                Node {
                    display: Display::Flex,
                    column_gap: px(5),
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(row),
            ))
            .id();
        for index in 0..3 {
            spawn_pip(commands, pips, index < level, theme::ACCENT, 10.0, 10.0);
        }
    }
}
