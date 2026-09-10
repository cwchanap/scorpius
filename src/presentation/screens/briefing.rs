use bevy::{
    prelude::*,
    text::LetterSpacing,
    ui::{BackgroundGradient, ColorStop, LinearGradient},
};

use crate::presentation::{CampaignRuntime, CanvasRoot, assets::UiAssets};

use super::super::{
    campaign_ui::{BriefingSnapshot, CampaignUiAction, active_definition, briefing_snapshot},
    theme,
};
use super::shared::{ensure_canvas, spawn_button, spawn_icon, spawn_pips, spawn_screen_root};

pub fn setup_briefing_screen(
    mut commands: Commands,
    runtime: Res<CampaignRuntime>,
    ui_assets: Res<UiAssets>,
    canvas_roots: Query<Entity, With<CanvasRoot>>,
) {
    let Some(definition) = active_definition(&runtime) else {
        return;
    };
    let Some(campaign) = runtime.0.state.as_ref() else {
        return;
    };
    let snapshot = briefing_snapshot(definition, campaign);
    let canvas = ensure_canvas(&mut commands, &canvas_roots);
    super::shared::spawn_campaign_camera(&mut commands);
    let root = spawn_screen_root(
        &mut commands,
        canvas,
        "Briefing Screen",
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
    commands.spawn((
        Text::new(format!("{:02}", snapshot.mission)),
        theme::ibm_plex_mono(&ui_assets.fonts, 76.0, FontWeight(600)),
        TextColor(theme::ACCENT),
        Node::default(),
        Pickable::IGNORE,
        ChildOf(header),
    ));
    commands.spawn((
        Node {
            width: px(1),
            height: px(64),
            ..default()
        },
        BackgroundColor(theme::BORDER),
        Pickable::IGNORE,
        ChildOf(header),
    ));
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
    let title_copy = snapshot
        .title
        .split_once('—')
        .map_or(snapshot.title, |(_, title)| title.trim());
    commands.spawn((
        Text::new(title_copy),
        theme::chakra_petch(&ui_assets.fonts, 34.0, FontWeight(600)),
        LetterSpacing::Px(2.04),
        TextColor(theme::TEXT),
        Pickable::IGNORE,
        ChildOf(title),
    ));
    spawn_pips(
        &mut commands,
        title,
        snapshot.mission.to_string().parse::<usize>().unwrap_or(1),
        7,
        theme::ACCENT,
        10.0,
        7.0,
        8.0,
    );
    let credits = commands
        .spawn((
            Node {
                margin: UiRect::left(Val::Auto),
                padding: UiRect::axes(px(14), px(20)),
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(12),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(theme::PANEL),
            BorderColor::all(theme::BORDER),
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
        Text::new(snapshot.credits.to_string()),
        theme::ibm_plex_mono(&ui_assets.fonts, 30.0, FontWeight(600)),
        TextColor(theme::GOLD),
        Pickable::IGNORE,
        ChildOf(credits),
    ));

    let body = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(120),
                bottom: px(0),
                display: Display::Flex,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    spawn_briefing_art(&mut commands, body, &ui_assets, &snapshot);
    spawn_briefing_panel(&mut commands, body, &ui_assets, &snapshot);
}

fn spawn_briefing_art(
    commands: &mut Commands,
    parent: Entity,
    assets: &UiAssets,
    snapshot: &BriefingSnapshot,
) {
    let art = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                position_type: PositionType::Relative,
                min_width: px(0),
                overflow: Overflow::clip(),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    super::shared::spawn_image(
        commands,
        art,
        assets.briefing_art.clone(),
        Node {
            position_type: PositionType::Absolute,
            left: px(-75),
            right: px(-75),
            top: px(0),
            bottom: px(0),
            ..default()
        },
    );
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            bottom: px(0),
            height: px(180),
            ..default()
        },
        BackgroundGradient(vec![
            LinearGradient::to_bottom(vec![
                ColorStop::percent(Color::NONE, 0.0),
                ColorStop::percent(
                    Color::srgba(6.0 / 255.0, 10.0 / 255.0, 18.0 / 255.0, 0.95),
                    100.0,
                ),
            ])
            .into(),
        ]),
        Pickable::IGNORE,
        ChildOf(art),
    ));
    let badges = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(40),
                bottom: px(40),
                display: Display::Flex,
                column_gap: px(14),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(art),
        ))
        .id();
    spawn_badge(
        commands,
        badges,
        assets,
        theme::BRIEFING_GRID_RECT,
        theme::MUTED,
        "9×9",
    );
    spawn_badge(
        commands,
        badges,
        assets,
        theme::BRIEFING_ENEMY_RECT,
        theme::ENEMY,
        &snapshot.enemy_count.to_string(),
    );
    spawn_badge(
        commands,
        badges,
        assets,
        theme::BRIEFING_HAZARD_RECT,
        theme::GOLD,
        &snapshot.hazard_count.to_string(),
    );
}

fn spawn_badge(
    commands: &mut Commands,
    parent: Entity,
    assets: &UiAssets,
    icon: Rect,
    color: Color,
    label: &str,
) {
    let badge = commands
        .spawn((
            Node {
                padding: UiRect::axes(px(12), px(16)),
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(10),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(Color::srgba(4.0 / 255.0, 9.0 / 255.0, 17.0 / 255.0, 0.85)),
            BorderColor::all(theme::BORDER),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    spawn_icon(commands, badge, &assets.icons, icon, Color::WHITE, 24.0);
    commands.spawn((
        Text::new(label),
        theme::ibm_plex_mono(&assets.fonts, 20.0, FontWeight(400)),
        TextColor(color),
        Pickable::IGNORE,
        ChildOf(badge),
    ));
}

fn spawn_briefing_panel(
    commands: &mut Commands,
    parent: Entity,
    assets: &UiAssets,
    snapshot: &BriefingSnapshot,
) {
    let panel = commands
        .spawn((
            Node {
                width: px(640),
                padding: UiRect::axes(px(44), px(56)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(26),
                min_width: px(640),
                border: UiRect::left(px(1)),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(8, 13, 22)),
            BorderColor::all(theme::BORDER),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    spawn_objective_card(
        commands,
        panel,
        assets,
        theme::BRIEFING_PRIMARY_RECT,
        theme::ACCENT,
        "PRIMARY",
        snapshot.primary,
        Some(snapshot.enemy_count),
    );
    spawn_objective_card(
        commands,
        panel,
        assets,
        theme::BRIEFING_BONUS_RECT,
        theme::GOLD,
        &format!("BONUS · {}", snapshot.bonus_title.to_ascii_uppercase()),
        snapshot.optional,
        None,
    );
    let rewards = commands
        .spawn((
            Node {
                display: Display::Flex,
                column_gap: px(16),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(panel),
        ))
        .id();
    spawn_reward(
        commands,
        rewards,
        assets,
        "BASE",
        snapshot.base_reward,
        theme::TEXT,
    );
    spawn_reward(
        commands,
        rewards,
        assets,
        "BONUS",
        snapshot.optional_reward,
        theme::GOLD,
    );
    let deploy = spawn_button(
        commands,
        panel,
        CampaignUiAction::StartMission,
        true,
        Node {
            height: px(96),
            margin: UiRect::top(Val::Auto),
            display: Display::Flex,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: px(20),
            border: UiRect::all(px(1)),
            ..default()
        },
    );
    spawn_icon(
        commands,
        deploy,
        &assets.icons,
        theme::BRIEFING_DEPLOY_RECT,
        Color::WHITE,
        40.0,
    );
    commands.spawn((
        Text::new("DEPLOY"),
        theme::chakra_petch(&assets.fonts, 30.0, FontWeight(600)),
        LetterSpacing::Px(6.0),
        TextColor(theme::TEXT),
        Pickable::IGNORE,
        ChildOf(deploy),
    ));
}

#[allow(clippy::too_many_arguments)]
fn spawn_objective_card(
    commands: &mut Commands,
    parent: Entity,
    assets: &UiAssets,
    icon: Rect,
    color: Color,
    heading: &str,
    description: &str,
    pips: Option<usize>,
) {
    let card = commands
        .spawn((
            Node {
                padding: UiRect::all(px(24)),
                display: Display::Flex,
                column_gap: px(20),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(theme::PANEL),
            BorderColor::all(if color == theme::GOLD {
                Color::srgb_u8(51, 48, 28)
            } else {
                theme::BORDER
            }),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    spawn_icon(commands, card, &assets.icons, icon, Color::WHITE, 46.0);
    let content = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: px(0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(card),
        ))
        .id();
    commands.spawn((
        Text::new(heading),
        theme::ibm_plex_mono(&assets.fonts, 15.0, FontWeight(400)),
        LetterSpacing::Px(3.0),
        TextColor(color),
        Pickable::IGNORE,
        ChildOf(content),
    ));
    commands.spawn((
        Text::new(description),
        theme::chakra_petch(&assets.fonts, 24.0, FontWeight(400)),
        TextColor(theme::TEXT),
        Node {
            max_width: px(480),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(content),
    ));
    if let Some(total) = pips {
        spawn_pips(commands, content, total, total, color, 8.0, 8.0, 8.0);
    }
}

fn spawn_reward(
    commands: &mut Commands,
    parent: Entity,
    assets: &UiAssets,
    heading: &'static str,
    amount: u32,
    color: Color,
) {
    let card = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                padding: UiRect::all(px(22)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(theme::PANEL),
            BorderColor::all(if color == theme::GOLD {
                Color::srgb_u8(51, 48, 28)
            } else {
                theme::BORDER
            }),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        Text::new(heading),
        theme::ibm_plex_mono(&assets.fonts, 14.0, FontWeight(400)),
        LetterSpacing::Px(2.52),
        TextColor(theme::MUTED),
        Pickable::IGNORE,
        ChildOf(card),
    ));
    let amount_row = commands
        .spawn((
            Node {
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(8),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(card),
        ))
        .id();
    commands.spawn((
        Text::new(format!(
            "{}{}",
            if color == theme::GOLD { "+" } else { "" },
            amount
        )),
        theme::ibm_plex_mono(&assets.fonts, 44.0, FontWeight(600)),
        TextColor(color),
        Pickable::IGNORE,
        ChildOf(amount_row),
    ));
    spawn_icon(
        commands,
        amount_row,
        &assets.icons,
        if color == theme::GOLD {
            theme::BRIEFING_BONUS_REWARD_RECT
        } else {
            theme::BRIEFING_BASE_REWARD_RECT
        },
        Color::WHITE,
        20.0,
    );
}
