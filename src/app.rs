use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::{
    domain::battle::BattleState,
    presentation::{
        BattleRuntime, SelectedCell,
        battlefield::setup_viability_scene,
        sync::apply_unit_transforms,
        ui::{setup_viability_ui, update_selected_cell_text},
    },
};

pub struct ScorpiusPlugin;

impl Plugin for ScorpiusPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Scorpius — Mission 1".into(),
                    resolution: (1280, 720).into(),
                    position: WindowPosition::Centered(MonitorSelection::Primary),
                    ..default()
                }),
                ..default()
            }),
            MeshPickingPlugin,
        ))
        .insert_resource(ClearColor(Color::srgb(0.025, 0.035, 0.055)))
        .insert_resource(MeshPickingSettings {
            require_markers: true,
            ..default()
        })
        .insert_resource(BattleRuntime(BattleState::viability_fixture()))
        .init_resource::<SelectedCell>()
        .add_systems(
            Startup,
            (
                center_primary_window,
                setup_viability_scene,
                setup_viability_ui,
            ),
        )
        .add_systems(Update, (apply_unit_transforms, update_selected_cell_text));
    }
}

fn center_primary_window(mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    if let Ok(mut window) = windows.single_mut() {
        window.position.center(MonitorSelection::Primary);
    }
}
