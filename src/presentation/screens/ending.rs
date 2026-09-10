use bevy::{
    prelude::*,
    text::LetterSpacing,
    ui::{BackgroundGradient, ColorStop, RadialGradient, RadialGradientShape, UiPosition},
};

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
    commands.entity(root).insert(BackgroundGradient(vec![
        RadialGradient::new(
            UiPosition::top_left(Val::Percent(50.0), Val::Percent(30.0)),
            RadialGradientShape::Ellipse(Val::Percent(90.0), Val::Percent(70.0)),
            vec![
                ColorStop::percent(Color::srgb_u8(13, 34, 48), 0.0),
                ColorStop::percent(Color::srgb_u8(4, 7, 13), 75.0),
            ],
        )
        .into(),
    ]));
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
        theme::ENDING_EMBLEM_RECT,
        Color::WHITE,
        96.0,
    );
    commands.spawn((
        Text::new("CAMPAIGN"),
        theme::chakra_petch(&ui_assets.fonts, 76.0, FontWeight(700)),
        LetterSpacing::Px(15.2),
        TextColor(Color::srgb_u8(238, 246, 255)),
        TextShadow {
            offset: Vec2::ZERO,
            color: Color::srgba(62.0 / 255.0, 199.0 / 255.0, 219.0 / 255.0, 0.4),
        },
        Pickable::IGNORE,
        ChildOf(heading),
    ));
    spawn_pips(&mut commands, heading, 7, 7, theme::MINT, 26.0, 8.0, 6.0);

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
        theme::ENDING_RETURN_RECT,
        Color::WHITE,
        34.0,
    );
    commands.spawn((
        Text::new("TITLE"),
        theme::chakra_petch(&ui_assets.fonts, 26.0, FontWeight(600)),
        LetterSpacing::Px(5.2),
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
    let archetype = match mech {
        PlayerMech::Vanguard => crate::domain::model::UnitArchetype::Vanguard,
        PlayerMech::Gunner => crate::domain::model::UnitArchetype::Gunner,
        PlayerMech::Interceptor => crate::domain::model::UnitArchetype::Interceptor,
    };
    let glyph = theme::unit_archetype_style(archetype);
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
    spawn_icon(
        commands,
        heading,
        &assets.icons,
        glyph.glyph_rect,
        glyph.color,
        20.0,
    );
    commands.spawn((
        Text::new(label),
        theme::chakra_petch(&assets.fonts, 22.0, FontWeight(600)),
        LetterSpacing::Px(2.2),
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
            super::shared::ending_track_icon(track),
            theme::MUTED,
            20.0,
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
            spawn_pip(commands, pips, index < level, theme::ACCENT, 26.0, 8.0);
        }
    }
}
