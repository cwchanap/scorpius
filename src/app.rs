use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::{
    campaign::{save::SaveFile, session::CampaignSession},
    mission::mission_definition,
    presentation::{
        ActiveMission, AttackPreviewCells, BattleEventQueue, BattleRuntime, CampaignRuntime,
        EventPlayback, PresentationRoot, RestartRequest, RestartRoundPending, SelectedCell,
        assets::{AssetLoadStatus, MissionAssets, monitor_mission_assets},
        battlefield::{rebuild_mission_scene, setup_mission_scene},
        campaign_ui::{
            CampaignStatus, DialogueCursor, despawn_campaign_screen, setup_aftermath_screen,
            setup_briefing_screen, setup_next_mission_screen, setup_pre_mission_story,
            setup_title_screen, setup_upgrade_screen, update_campaign_status_text,
            update_dialogue_screen, update_upgrade_screen,
        },
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
        ui::{HudRoot, setup_mission_ui, update_asset_status_text, update_hud},
    },
};

pub struct ScorpiusPlugin;

#[derive(States, Default, Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GameScreen {
    #[default]
    Title,
    PreMissionStory,
    Briefing,
    Battle,
    Aftermath,
    Upgrade,
    NextMission,
}

impl Plugin for ScorpiusPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Scorpius".into(),
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
        .insert_resource(CampaignRuntime(CampaignSession::new(
            SaveFile::platform_default(),
        )))
        .init_state::<GameScreen>()
        .init_resource::<SelectedCell>()
        .init_resource::<BattleEventQueue>()
        .init_resource::<EventPlayback>()
        .init_resource::<AttackPreviewCells>()
        .init_resource::<InteractionState>()
        .init_resource::<StatusMessage>()
        .init_resource::<CampaignStatus>()
        .init_resource::<DialogueCursor>()
        .init_resource::<RestartRequest>()
        .init_resource::<RestartRoundPending>()
        .init_resource::<MissionAssets>()
        .init_resource::<AssetLoadStatus>()
        .add_systems(Startup, center_primary_window)
        .add_systems(OnEnter(GameScreen::Title), setup_title_screen)
        .add_systems(OnExit(GameScreen::Title), despawn_campaign_screen)
        .add_systems(
            OnEnter(GameScreen::PreMissionStory),
            setup_pre_mission_story,
        )
        .add_systems(OnExit(GameScreen::PreMissionStory), despawn_campaign_screen)
        .add_systems(OnEnter(GameScreen::Briefing), setup_briefing_screen)
        .add_systems(OnExit(GameScreen::Briefing), despawn_campaign_screen)
        .add_systems(
            Update,
            (
                update_dialogue_screen.run_if(
                    in_state(GameScreen::PreMissionStory).or_else(in_state(GameScreen::Aftermath)),
                ),
                update_campaign_status_text
                    .run_if(in_state(GameScreen::Title).or_else(in_state(GameScreen::Upgrade))),
            ),
        )
        .add_systems(OnExit(GameScreen::Battle), teardown_battle_screen)
        .add_systems(
            OnEnter(GameScreen::Battle),
            (
                teardown_battle_screen,
                enter_battle,
                setup_mission_scene,
                setup_mission_ui,
            )
                .chain(),
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
        )
        .add_systems(OnEnter(GameScreen::Aftermath), setup_aftermath_screen)
        .add_systems(OnExit(GameScreen::Aftermath), despawn_campaign_screen)
        .add_systems(OnEnter(GameScreen::Upgrade), setup_upgrade_screen)
        .add_systems(OnExit(GameScreen::Upgrade), despawn_campaign_screen)
        .add_systems(
            Update,
            update_upgrade_screen.run_if(in_state(GameScreen::Upgrade)),
        )
        .add_systems(OnEnter(GameScreen::NextMission), setup_next_mission_screen)
        .add_systems(OnExit(GameScreen::NextMission), despawn_campaign_screen);
    }
}

/// Despawn battlefield/HUD roots left over from a previous Battle visit so
/// re-entering Battle cannot double-spawn the presentation.
#[allow(clippy::type_complexity)]
pub fn teardown_battle_screen(
    mut commands: Commands,
    stale: Query<
        Entity,
        Or<(
            With<PresentationRoot>,
            With<Camera>,
            With<DirectionalLight>,
            With<HudRoot>,
        )>,
    >,
) {
    for entity in &stale {
        commands.entity(entity).try_despawn();
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
