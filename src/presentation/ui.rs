use bevy::prelude::*;

use super::SelectedCell;

#[derive(Component)]
pub struct SelectedCellText;

pub fn setup_viability_ui(mut commands: Commands) {
    commands.spawn((
        Text::new("HPA-632 · BEVY VIABILITY"),
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
        Text::new("Selected cell: none\nClick the center tile, then an adjacent tile to move."),
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
            "Selected cell: ({}, {})\nClick an adjacent tile to move canonical state.",
            cell.x, cell.y
        ),
        None => {
            "Selected cell: none\nClick the center tile, then an adjacent tile to move.".to_owned()
        }
    };
}
