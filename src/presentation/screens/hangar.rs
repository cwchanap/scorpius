use bevy::{
    prelude::*,
    text::LetterSpacing,
    ui::{
        BackgroundGradient, ColorStop, LinearGradient, RadialGradient, RadialGradientShape,
        UiPosition,
    },
};

use crate::campaign::model::{PlayerMech, UpgradeTrack};
use crate::domain::model::UnitArchetype;
use crate::presentation::{CampaignRuntime, CanvasRoot, assets::UiAssets};

use super::super::{
    campaign_ui::{
        CampaignStatus, CampaignStatusText, CampaignUiAction, MECHS, TRACKS, UpgradeCostText,
        UpgradeCreditsText, UpgradePip, UpgradePurchaseIcon, UpgradeRow, UpgradeTrackIcon,
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
    commands.entity(root).insert(BackgroundGradient(vec![
        RadialGradient::new(
            UiPosition::top_left(Val::Percent(50.0), Val::Percent(0.0)),
            RadialGradientShape::Ellipse(Val::Percent(120.0), Val::Percent(80.0)),
            vec![
                ColorStop::percent(Color::srgb_u8(10, 18, 32), 0.0),
                ColorStop::percent(Color::srgb_u8(5, 8, 15), 70.0),
            ],
        )
        .into(),
    ]));

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
        theme::HANGAR_RECT,
        Color::WHITE,
        42.0,
    );
    commands.spawn((
        Text::new("HANGAR"),
        theme::chakra_petch(&ui_assets.fonts, 36.0, FontWeight(600)),
        LetterSpacing::Px(4.32),
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
        theme::CREDITS_LARGE_RECT,
        Color::WHITE,
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
        theme::ICON_FORWARD_COMPACT,
        Color::WHITE,
        36.0,
    );
    commands.spawn((
        Text::new("NEXT DROP"),
        theme::chakra_petch(&ui_assets.fonts, 28.0, FontWeight(600)),
        LetterSpacing::Px(5.6),
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
    let (art, archetype) = match mech {
        PlayerMech::Vanguard => (assets.vanguard_art.clone(), UnitArchetype::Vanguard),
        PlayerMech::Gunner => (assets.gunner_art.clone(), UnitArchetype::Gunner),
        PlayerMech::Interceptor => (assets.interceptor_art.clone(), UnitArchetype::Interceptor),
    };
    let glyph = theme::unit_archetype_style(archetype);
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
        ImageNode::new(art).with_mode(NodeImageMode::Auto),
        Pickable::IGNORE,
        ChildOf(art_panel),
    ));
    let skill_icon = spawn_icon(
        commands,
        art_panel,
        &assets.icons,
        theme::ICON_SKILL,
        theme::GOLD,
        26.0,
    );
    commands.entity(skill_icon).insert(Node {
        position_type: PositionType::Absolute,
        right: px(18),
        bottom: px(18),
        width: px(26),
        height: px(26),
        ..default()
    });
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            bottom: px(0),
            height: px(130),
            ..default()
        },
        BackgroundGradient(vec![
            LinearGradient::to_top(vec![
                ColorStop::percent(
                    Color::srgba(10.0 / 255.0, 20.0 / 255.0, 32.0 / 255.0, 0.95),
                    0.0,
                ),
                ColorStop::percent(Color::NONE, 60.0),
            ])
            .into(),
        ]),
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
        glyph.glyph_rect,
        glyph.color,
        22.0,
    );
    commands.spawn((
        Text::new(label),
        theme::chakra_petch(&assets.fonts, 26.0, FontWeight(600)),
        LetterSpacing::Px(2.6),
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
    let track_icon = spawn_icon(
        commands,
        row,
        &assets.icons,
        super::shared::hangar_track_icon(track),
        theme::MUTED,
        22.0,
    );
    commands
        .entity(track_icon)
        .insert(UpgradeTrackIcon(mech, track));
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
                width: px(26),
                height: px(8),
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
    let purchase_icon = spawn_icon(
        commands,
        button,
        &assets.icons,
        theme::CREDITS_PURCHASE_RECT,
        Color::WHITE,
        18.0,
    );
    commands
        .entity(purchase_icon)
        .insert(UpgradePurchaseIcon(mech, track));
    commands.entity(button).insert(Node {
        min_width: px(100),
        height: px(42),
        padding: UiRect::horizontal(px(12)),
        display: Display::Flex,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        column_gap: px(8),
        ..default()
    });
    commands.spawn((
        Text::new("—"),
        theme::ibm_plex_mono(&assets.fonts, 15.0, FontWeight(600)),
        TextColor(theme::TEXT),
        UpgradeCostText(mech, track),
        Pickable::IGNORE,
        ChildOf(button),
    ));
}
