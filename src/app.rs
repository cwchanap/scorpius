use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::{
    campaign::{model::CampaignState, save::SaveFile, session::CampaignSession},
    mission::mission_definition,
    presentation::{
        ActiveMission, AttackPreviewCells, BattleEventQueue, BattleRuntime, CampaignRuntime,
        EventPlayback, RestartRequest, RestartRoundPending, SelectedCell,
        assets::{AssetLoadStatus, MissionAssets, monitor_mission_assets},
        battlefield::{rebuild_mission_scene, setup_mission_scene},
        interaction::{
            InteractionState, StatusMessage, handle_keyboard_shortcuts, process_restart_request,
            reset_transient_battle_state,
        },
        playback::{begin_restarted_round, play_battle_events},
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

#[derive(States, Default, Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GameScreen {
    Title,
    PreMissionStory,
    Briefing,
    #[default]
    Battle,
    Aftermath,
    Upgrade,
    NextMission,
}

impl Plugin for ScorpiusPlugin {
    fn build(&self, app: &mut App) {
        // ponytail: Task-4 migration checkpoint — Battle is default and a fresh
        // campaign is seeded in memory; Task 5 restores Title as the entry state.
        let mut campaign = CampaignSession::new(SaveFile::platform_default());
        campaign.state = Some(CampaignState::new_game());

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
        .insert_resource(CampaignRuntime(campaign))
        .init_state::<GameScreen>()
        .init_resource::<SelectedCell>()
        .init_resource::<BattleEventQueue>()
        .init_resource::<EventPlayback>()
        .init_resource::<AttackPreviewCells>()
        .init_resource::<InteractionState>()
        .init_resource::<StatusMessage>()
        .init_resource::<RestartRequest>()
        .init_resource::<RestartRoundPending>()
        .init_resource::<MissionAssets>()
        .init_resource::<AssetLoadStatus>()
        .add_systems(Startup, center_primary_window)
        .add_systems(
            OnEnter(GameScreen::Battle),
            (enter_battle, setup_mission_scene, setup_mission_ui).chain(),
        )
        .add_systems(Update, monitor_mission_assets)
        .add_systems(Update, stabilize_primary_window_position)
        .add_systems(
            Update,
            (
                process_restart_request,
                rebuild_mission_scene,
                begin_restarted_round,
            )
                .chain()
                .run_if(in_state(GameScreen::Battle)),
        )
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
                .chain()
                .after(begin_restarted_round)
                .run_if(in_state(GameScreen::Battle)),
        )
        .add_systems(
            Update,
            (
                apply_unit_transforms,
                apply_prop_visibility,
                sync_auxiliary_transforms,
                sync_cell_highlights,
                pulse_telegraphs,
            )
                .after(reconcile_reaction_markers)
                .run_if(in_state(GameScreen::Battle)),
        )
        .add_systems(
            Update,
            play_battle_events
                .after(apply_unit_transforms)
                .run_if(in_state(GameScreen::Battle)),
        )
        .add_systems(
            Update,
            handle_keyboard_shortcuts
                .after(play_battle_events)
                .run_if(in_state(GameScreen::Battle)),
        )
        .add_systems(
            Update,
            (update_hud, update_asset_status_text)
                .after(play_battle_events)
                .run_if(in_state(GameScreen::Battle)),
        );
    }
}

/// Resolve the campaign's next mission and construct the battle it defines.
///
/// Exclusive so `ActiveMission`/`BattleRuntime` exist before the chained scene
/// and HUD setup systems run.
pub fn enter_battle(world: &mut World) {
    let (next_mission, upgrades) = {
        let state = world
            .resource::<CampaignRuntime>()
            .0
            .state
            .as_ref()
            .expect("Battle requires active campaign");
        (state.next_mission, state.upgrades.clone())
    };
    let definition =
        mission_definition(next_mission).expect("current mission must have authored definition");
    let mut battle = (definition.build)(fresh_seed(), &upgrades);
    battle
        .begin_round()
        .expect("authored mission opening must be valid");
    world.insert_resource(ActiveMission(definition));
    world.insert_resource(BattleRuntime(battle));
    reset_transient_battle_state(world);
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
