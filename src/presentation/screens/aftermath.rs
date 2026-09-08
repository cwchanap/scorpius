use bevy::prelude::*;

use crate::campaign::progression::CompletionReceipt;
use crate::presentation::{ActiveMission, CampaignRuntime, CanvasRoot, assets::UiAssets};

use super::super::{
    campaign_ui::{CampaignUiAction, DialogueCursor},
    theme,
};
use super::shared::{ensure_canvas, spawn_dialogue_screen, spawn_icon};

pub fn setup_aftermath_screen(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    runtime: Res<CampaignRuntime>,
    active_mission: Res<ActiveMission>,
    mut cursor: ResMut<DialogueCursor>,
    ui_assets: Res<UiAssets>,
    canvas_roots: Query<Entity, With<CanvasRoot>>,
) {
    *cursor = DialogueCursor(0);
    let canvas = ensure_canvas(&mut commands, &canvas_roots);
    let root = spawn_dialogue_screen(
        &mut commands,
        &asset_server,
        &active_mission.0.aftermath,
        CampaignUiAction::AdvanceAftermath,
        canvas,
        &ui_assets.fonts,
        &ui_assets.icons,
        theme::GOLD,
        false,
    );
    if let Some(receipt) = runtime.0.last_completion {
        spawn_receipt_panel(&mut commands, root, &ui_assets, receipt);
    }
}

fn spawn_receipt_panel(
    commands: &mut Commands,
    parent: Entity,
    assets: &UiAssets,
    receipt: CompletionReceipt,
) {
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(56),
                top: px(56),
                width: px(420),
                padding: UiRect::all(px(28)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(20),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(Color::srgba(4.0 / 255.0, 9.0 / 255.0, 17.0 / 255.0, 0.92)),
            BorderColor::all(theme::BORDER),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    let heading = commands
        .spawn((
            Node {
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(12),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(panel),
        ))
        .id();
    spawn_icon(
        commands,
        heading,
        &assets.icons,
        super::super::theme::ICON_SKILL,
        theme::GOLD,
        26.0,
    );
    commands.spawn((
        Text::new("SALVAGE"),
        theme::ibm_plex_mono(&assets.fonts, 14.0, FontWeight(400)),
        TextColor(theme::GOLD),
        Pickable::IGNORE,
        ChildOf(heading),
    ));
    spawn_receipt_row(
        commands,
        panel,
        assets,
        "MISSION",
        receipt.mission.to_string(),
        theme::TEXT,
    );
    spawn_receipt_row(
        commands,
        panel,
        assets,
        "BASE",
        receipt.base_reward.to_string(),
        theme::TEXT,
    );
    spawn_receipt_row(
        commands,
        panel,
        assets,
        "BONUS",
        format!("+{}", receipt.optional_reward),
        theme::GOLD,
    );
    spawn_receipt_row(
        commands,
        panel,
        assets,
        "TOTAL",
        receipt.total_reward.to_string(),
        theme::ACCENT,
    );
    spawn_receipt_row(
        commands,
        panel,
        assets,
        "CREDITS",
        receipt.credits_after.to_string(),
        theme::GOLD,
    );
}

fn spawn_receipt_row(
    commands: &mut Commands,
    parent: Entity,
    assets: &UiAssets,
    label: &'static str,
    value: String,
    color: Color,
) {
    let row = commands
        .spawn((
            Node {
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(14),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        Node {
            width: px(8),
            height: px(8),
            ..default()
        },
        BackgroundColor(color),
        Pickable::IGNORE,
        ChildOf(row),
    ));
    commands.spawn((
        Text::new(label),
        theme::ibm_plex_mono(&assets.fonts, 15.0, FontWeight(400)),
        TextColor(theme::MUTED),
        Node {
            flex_grow: 1.0,
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(row),
    ));
    commands.spawn((
        Text::new(value),
        theme::ibm_plex_mono(&assets.fonts, 22.0, FontWeight(600)),
        TextColor(color),
        Pickable::IGNORE,
        ChildOf(row),
    ));
}
