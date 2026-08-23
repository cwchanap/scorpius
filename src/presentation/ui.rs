use bevy::prelude::*;

use super::{
    SelectedCell,
    assets::{AssetLoadStatus, MISSION_ONE_GLTF_DISPLAY_PATH},
};

#[derive(Component)]
pub struct SelectedCellText;

#[derive(Component)]
pub struct AssetStatusText;

pub fn setup_mission_ui(mut commands: Commands) {
    commands.spawn((
        Text::new("MISSION 1 // BREAK THE LINE"),
        TextFont {
            font_size: FontSize::Px(24.0),
            ..default()
        },
        TextColor(Color::srgb(0.72, 0.92, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: px(22),
            left: px(24),
            ..default()
        },
        Pickable::IGNORE,
    ));

    commands.spawn((
        Text::new("Selected cell: none\nInspect the battlefield while mission assets load."),
        TextFont {
            font_size: FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: px(58),
            left: px(24),
            ..default()
        },
        Pickable::IGNORE,
        SelectedCellText,
    ));

    commands.spawn((
        Text::new(format!("Loading {MISSION_ONE_GLTF_DISPLAY_PATH}…")),
        TextFont {
            font_size: FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.78, 0.34)),
        BackgroundColor(Color::srgba(0.08, 0.025, 0.025, 0.94)),
        Node {
            position_type: PositionType::Absolute,
            right: px(24),
            top: px(22),
            padding: UiRect::all(px(12)),
            ..default()
        },
        Pickable::IGNORE,
        AssetStatusText,
    ));
}

pub fn update_selected_cell_text(
    selected: Res<SelectedCell>,
    mut text: Single<&mut Text, With<SelectedCellText>>,
) {
    if !selected.is_changed() {
        return;
    }
    text.0 = match selected.0 {
        Some(cell) => format!(
            "Selected cell: ({}, {})\nCanonical state drives every world-space visual.",
            cell.x, cell.y
        ),
        None => "Selected cell: none\nChoose a mech or battlefield cell.".to_owned(),
    };
}

pub fn update_asset_status_text(
    status: Res<AssetLoadStatus>,
    panel: Single<(&mut Text, &mut Visibility, &mut TextColor), With<AssetStatusText>>,
) {
    if !status.is_changed() {
        return;
    }
    let (mut text, mut visibility, mut color) = panel.into_inner();
    match &*status {
        AssetLoadStatus::Loading => {
            text.0 = format!("Loading {MISSION_ONE_GLTF_DISPLAY_PATH}…");
            *visibility = Visibility::Visible;
            color.0 = Color::srgb(1.0, 0.78, 0.34);
        }
        AssetLoadStatus::Ready => {
            *visibility = Visibility::Hidden;
        }
        AssetLoadStatus::Failed(path) => {
            text.0 = format!("ASSET LOAD FAILED\n{path}");
            *visibility = Visibility::Visible;
            color.0 = Color::srgb(1.0, 0.36, 0.3);
        }
    }
}
