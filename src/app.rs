use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::{
    mission::mission_one::mission_one,
    presentation::{
        AttackPreviewCells, BattleEventQueue, BattleRuntime, SelectedCell,
        assets::{AssetLoadStatus, MissionAssets, monitor_mission_assets},
        battlefield::setup_mission_scene,
        interaction::{InteractionState, StatusMessage, handle_keyboard_shortcuts},
        sync::{
            apply_prop_visibility, apply_unit_transforms, attach_intent_line_rendering,
            attach_intent_target_rendering, attach_reaction_rendering, attach_telegraph_rendering,
            pulse_telegraphs, reconcile_intent_guides, reconcile_reaction_markers,
            reconcile_telegraph_markers, sync_auxiliary_transforms, sync_cell_highlights,
        },
        ui::{setup_mission_ui, update_asset_status_text, update_hud},
    },
};

pub struct ScorpiusPlugin;

impl Plugin for ScorpiusPlugin {
    fn build(&self, app: &mut App) {
        let mut battle = mission_one(fresh_seed());
        battle
            .begin_round()
            .expect("authored Mission 1 opening must be valid");

        app.add_plugins((
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").into(),
                    ..default()
                })
                .set(WindowPlugin {
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
        .insert_resource(BattleRuntime(battle))
        .init_resource::<SelectedCell>()
        .init_resource::<BattleEventQueue>()
        .init_resource::<AttackPreviewCells>()
        .init_resource::<InteractionState>()
        .init_resource::<StatusMessage>()
        .init_resource::<MissionAssets>()
        .init_resource::<AssetLoadStatus>()
        .add_systems(
            Startup,
            (center_primary_window, setup_mission_scene, setup_mission_ui),
        )
        .add_systems(Update, monitor_mission_assets)
        .add_systems(Update, stabilize_primary_window_position)
        .add_systems(
            Update,
            (
                reconcile_telegraph_markers,
                reconcile_intent_guides,
                reconcile_reaction_markers,
                attach_telegraph_rendering,
                attach_intent_target_rendering,
                attach_intent_line_rendering,
                attach_reaction_rendering,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                apply_unit_transforms,
                apply_prop_visibility,
                sync_auxiliary_transforms,
                sync_cell_highlights,
                pulse_telegraphs,
                handle_keyboard_shortcuts,
                update_hud,
                update_asset_status_text,
            ),
        );
    }
}

fn fresh_seed() -> u64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    elapsed.as_secs() ^ u64::from(elapsed.subsec_nanos())
}

fn center_primary_window(mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    if let Ok(mut window) = windows.single_mut() {
        window.position.center(MonitorSelection::Primary);
    }
}

fn stabilize_primary_window_position(
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut startup_frames: Local<u16>,
) {
    if *startup_frames >= 120 {
        return;
    }
    if let Ok(mut window) = windows.single_mut() {
        window.position.center(MonitorSelection::Primary);
        *startup_frames += 1;
    }
}
