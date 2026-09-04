use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use bevy::prelude::*;
use scorpius::{
    campaign::{model::CampaignState, save::SaveFile, session::CampaignSession},
    domain::{
        battle::BattleState,
        board::GridPos,
        model::{BattlePhase, MissionResult, Reaction, UnitArchetype, UnitId},
    },
    mission::mission_five::mission_five,
    mission::mission_four::mission_four,
    mission::mission_one::{ids, mission_one},
    mission::mission_seven::mission_seven,
    mission::mission_three::{self, mission_three},
    mission::mission_two::mission_two,
    mission::{MissionId, mission_definition},
    presentation::{
        ActiveMission, AttackPreviewCells, BattleEventQueue, BattleRuntime, CampaignRuntime,
        EventPlayback, ExtractionVisual, PresentationRoot, SelectedCell, TelegraphVisual,
        UnitVisual,
        battlefield::{create_visual_assets, mission_grid_cells, scene_index},
        grid_to_world,
        interaction::{
            CommandAction, InteractionMode, InteractionState, StatusMessage, execute_command,
            handle_viability_cell_click, restart_battle, route_cell_click, update_hover_preview,
        },
        sync::{
            apply_unit_transforms, attach_extraction_rendering, reconcile_extraction_marker,
            reconcile_telegraph_markers,
        },
        ui::{HudSnapshot, ObjectiveTrackSnapshot, result_overlay_copy},
    },
};

static NEXT_ID: AtomicU32 = AtomicU32::new(0);

fn temp_save_path(label: &str) -> PathBuf {
    let n = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "scorpius-presentation-{label}-{}-{n}.json",
        std::process::id()
    ))
}

fn presentation_fixture_app() -> App {
    let mut app = App::new();
    app.insert_resource(BattleRuntime(mission_one(7)))
        .insert_resource(CampaignRuntime(CampaignSession {
            state: Some(CampaignState::new_game()),
            save: SaveFile::new(temp_save_path("presentation-restart")),
            last_completion: None,
        }))
        .insert_resource(ActiveMission(mission_definition(MissionId::One).unwrap()))
        .init_resource::<InteractionState>()
        .init_resource::<StatusMessage>()
        .init_resource::<BattleEventQueue>()
        .init_resource::<EventPlayback>()
        .init_resource::<AttackPreviewCells>()
        .init_resource::<SelectedCell>();
    app.world_mut().spawn(PresentationRoot);
    app
}

#[test]
fn canonical_move_drives_visual_transform_without_renderer() {
    let mut app = App::new();
    app.insert_resource(BattleRuntime(BattleState::viability_fixture()))
        .add_systems(Update, apply_unit_transforms);
    app.world_mut().spawn((
        UnitVisual(UnitId(1)),
        Transform::from_translation(grid_to_world(GridPos::new(1, 1))),
    ));

    app.world_mut()
        .resource_mut::<BattleRuntime>()
        .0
        .move_unit(UnitId(1), GridPos::new(1, 2))
        .unwrap();
    app.update();

    let mut transforms = app.world_mut().query::<&Transform>();
    let transform = transforms.single(app.world()).unwrap();
    assert_eq!(transform.translation, grid_to_world(GridPos::new(1, 2)));
}

#[test]
fn adjacent_cell_click_moves_canonical_unit() {
    let mut battle = BattleState::viability_fixture();
    let mut selected = None;

    handle_viability_cell_click(&mut battle, &mut selected, GridPos::new(1, 1)).unwrap();
    assert_eq!(selected, Some(GridPos::new(1, 1)));

    handle_viability_cell_click(&mut battle, &mut selected, GridPos::new(2, 1)).unwrap();
    assert_eq!(selected, Some(GridPos::new(2, 1)));
    assert_eq!(battle.unit(UnitId(1)).unwrap().position, GridPos::new(2, 1));
}

#[test]
fn mission_board_exposes_all_eighty_one_logical_cells() {
    let cells = mission_grid_cells(9, 9);

    assert_eq!(cells.len(), 81);
    assert_eq!(cells.first(), Some(&GridPos::new(0, 0)));
    assert_eq!(cells.last(), Some(&GridPos::new(8, 8)));
}

#[test]
fn committed_footprints_create_one_marker_per_unique_cell() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    let expected: BTreeSet<_> = battle
        .intents()
        .iter()
        .flat_map(|intent| {
            intent
                .footprint
                .iter()
                .copied()
                .map(move |cell| (intent.attacker, cell))
        })
        .collect();

    let mut app = App::new();
    app.insert_resource(BattleRuntime(battle))
        .add_systems(Update, reconcile_telegraph_markers);
    app.update();

    let actual: BTreeSet<_> = app
        .world_mut()
        .query::<&TelegraphVisual>()
        .iter(app.world())
        .map(|marker| (marker.attacker, marker.cell))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn selected_unit_can_move_then_arm_a_weapon() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    let mut interaction = InteractionState::default();

    route_cell_click(&mut battle, &mut interaction, GridPos::new(5, 8)).unwrap();
    assert_eq!(interaction.selected_unit, Some(ids::INTERCEPTOR));
    execute_command(&mut battle, &mut interaction, CommandAction::Move).unwrap();
    route_cell_click(&mut battle, &mut interaction, GridPos::new(5, 7)).unwrap();
    assert!(battle.unit(ids::INTERCEPTOR).unwrap().activation.moved);

    execute_command(&mut battle, &mut interaction, CommandAction::WeaponSlot(1)).unwrap();
    assert_eq!(
        interaction.mode,
        InteractionMode::Attack(ids::PULSE_CARBINE)
    );

    update_hover_preview(&battle, &mut interaction, GridPos::new(4, 6));
    let preview = interaction.preview.as_ref().unwrap();
    assert_eq!(preview.target, GridPos::new(4, 6));
    assert_eq!(preview.footprint, vec![GridPos::new(4, 6)]);
    assert_eq!(preview.en_cost, 1);
}

#[test]
fn command_routing_finishes_the_squad_then_resolves_enemy_attacks() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    let mut interaction = InteractionState::default();

    for id in [ids::VANGUARD, ids::GUNNER, ids::INTERCEPTOR] {
        let position = battle.unit(id).unwrap().position;
        route_cell_click(&mut battle, &mut interaction, position).unwrap();
        execute_command(
            &mut battle,
            &mut interaction,
            CommandAction::Reaction(Reaction::Guard),
        )
        .unwrap();
        execute_command(&mut battle, &mut interaction, CommandAction::FinishUnit).unwrap();
    }

    assert!(battle.ready_to_resolve());
    let events =
        execute_command(&mut battle, &mut interaction, CommandAction::ResolveAttacks).unwrap();
    assert!(!events.is_empty());
    assert!(matches!(
        battle.phase(),
        BattlePhase::Player | BattlePhase::Defeat
    ));
    assert_eq!(interaction.selected_unit, None);
}

#[test]
fn hud_snapshot_reports_objectives_unit_allowances_and_threats() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    battle.begin_activation(ids::VANGUARD).unwrap();
    let hud = HudSnapshot::from_battle(
        &battle,
        Some(ids::VANGUARD),
        mission_definition(MissionId::One).unwrap(),
    );

    assert_eq!(hud.round_phase, "Round 1 · Player Phase");
    assert_eq!(hud.primary, "Eliminate all enemies. · 4 remaining");
    assert_eq!(hud.objective_track, None, "M1 has no tracked unit");
    assert_eq!(
        hud.optional,
        "Turnabout: damage an enemy with enemy fire, collision, hazard, or explosion. · Not yet"
    );
    assert_eq!(hud.selected_name, Some("Vanguard"));
    assert_eq!(hud.threats.len(), 4);
    assert_eq!(hud.pilot_label, "[P] AEGIS");
    assert!(hud.can_pilot);
    assert_eq!(hud.pilot_aegis, "READY");
}

#[test]
fn hud_reports_pilot_skill_states_and_dynamic_pilot_label() {
    let definition = mission_definition(MissionId::One).unwrap();
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();

    battle.begin_activation(ids::GUNNER).unwrap();
    battle.use_focus().unwrap();
    battle
        .choose_reaction(ids::GUNNER, Reaction::Guard)
        .unwrap();
    battle.finish_activation(ids::GUNNER).unwrap();

    battle.begin_activation(ids::INTERCEPTOR).unwrap();
    battle.use_overdrive().unwrap();
    let hud = HudSnapshot::from_battle(&battle, Some(ids::INTERCEPTOR), definition);
    assert_eq!(hud.pilot_label, "[P] OVERDRIVE");
    assert_eq!(hud.pilot_aegis, "READY");
    assert_eq!(hud.pilot_focus, "ACTIVE");
    assert_eq!(hud.pilot_overdrive, "ACTIVE");

    battle
        .choose_reaction(ids::INTERCEPTOR, Reaction::Guard)
        .unwrap();
    battle.finish_activation(ids::INTERCEPTOR).unwrap();
    let hud = HudSnapshot::from_battle(&battle, None, definition);
    assert_eq!(hud.pilot_label, "[P] PILOT");
    assert_eq!(hud.pilot_aegis, "READY");
    assert_eq!(hud.pilot_focus, "ACTIVE");
    assert_eq!(hud.pilot_overdrive, "USED");
}

#[test]
fn hud_tracks_protect_mission_round_cap_and_gunner_hp() {
    let mut battle = mission_two(7);
    battle.begin_round().unwrap();
    let hud = HudSnapshot::from_battle(&battle, None, mission_definition(MissionId::Two).unwrap());

    assert_eq!(hud.round_phase, "Round 1/3 · Player Phase");
    assert_eq!(
        hud.objective_track,
        Some(ObjectiveTrackSnapshot::Protect {
            name: "Gunner",
            hp: 15,
            max_hp: 15
        })
    );
}

#[test]
fn hud_tracks_intercept_mission_round_cap_and_courier_distance_to_exit() {
    let mut battle = mission_three(7);
    battle.begin_round().unwrap();
    let hud =
        HudSnapshot::from_battle(&battle, None, mission_definition(MissionId::Three).unwrap());

    assert_eq!(hud.round_phase, "Round 1/5 · Player Phase");
    assert_eq!(
        hud.objective_track,
        Some(ObjectiveTrackSnapshot::Intercept {
            name: "Courier",
            distance: 14
        })
    );
}

#[test]
fn mission_four_target_hud_pins_the_gate_bulwark() {
    let mut battle = mission_four(7);
    battle.begin_round().unwrap();
    let hud = HudSnapshot::from_battle(&battle, None, mission_definition(MissionId::Four).unwrap());

    assert_eq!(
        hud.objective_track,
        Some(ObjectiveTrackSnapshot::Target {
            name: "Gate Bulwark",
            hp: 16,
            max_hp: 16
        })
    );
}

#[test]
fn mission_seven_target_hud_pins_the_regent() {
    let mut battle = mission_seven(7);
    battle.begin_round().unwrap();
    let hud =
        HudSnapshot::from_battle(&battle, None, mission_definition(MissionId::Seven).unwrap());

    assert_eq!(
        hud.objective_track,
        Some(ObjectiveTrackSnapshot::Target {
            name: "Regent",
            hp: 52,
            max_hp: 52
        })
    );
}

#[test]
fn bulwark_and_controller_use_permanent_scene_indices() {
    assert_eq!(scene_index(UnitArchetype::Flanker), 10);
    assert_eq!(scene_index(UnitArchetype::Bulwark), 11);
    assert_eq!(scene_index(UnitArchetype::Controller), 12);
}

#[test]
fn mission_five_hud_lists_both_artillery_threats_and_remaining_count() {
    let mut battle = mission_five(7);
    battle.begin_round().unwrap();
    let hud = HudSnapshot::from_battle(&battle, None, mission_definition(MissionId::Five).unwrap());

    let attackers: BTreeSet<_> = hud.threats.iter().map(|threat| threat.attacker).collect();
    assert!(attackers.contains(&"Siege Artillery A"));
    assert!(attackers.contains(&"Siege Artillery B"));
    assert!(
        hud.primary.contains("remaining"),
        "EliminateAllEnemies primary carries the remaining count: {}",
        hud.primary
    );
}

#[test]
fn intercept_mission_spawns_one_white_extraction_ring_at_the_escape_cell() {
    let mut battle = mission_three(7);
    battle.begin_round().unwrap();
    let mut app = App::new();
    let mut meshes = Assets::<Mesh>::default();
    let mut materials = Assets::<StandardMaterial>::default();
    let visuals = create_visual_assets(&mut meshes, &mut materials);
    let (ring_mesh, white_material) = (visuals.ring_mesh.clone(), visuals.intended_target.clone());
    app.insert_resource(meshes)
        .insert_resource(materials)
        .insert_resource(visuals)
        .insert_resource(BattleRuntime(battle))
        .add_systems(
            Update,
            (reconcile_extraction_marker, attach_extraction_rendering).chain(),
        );
    app.world_mut().spawn(PresentationRoot);
    app.update();

    let mut markers = app.world_mut().query::<(
        &ExtractionVisual,
        &Mesh3d,
        &MeshMaterial3d<StandardMaterial>,
    )>();
    let markers: Vec<_> = markers.iter(app.world()).collect();
    assert_eq!(markers.len(), 1, "exactly one extraction ring");
    let (marker, mesh, material) = markers[0];
    assert_eq!(marker.0, GridPos::new(8, 0));
    assert_eq!(marker.0, mission_three::EXTRACTION);
    assert_eq!(mesh.0.id(), ring_mesh.id());
    assert_eq!(
        material.0.id(),
        white_material.id(),
        "ring uses the existing white material"
    );
}

#[test]
fn missions_without_an_intercept_primary_spawn_no_extraction_marker() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    let mut app = App::new();
    app.insert_resource(BattleRuntime(battle))
        .add_systems(Update, reconcile_extraction_marker);
    app.world_mut().spawn(PresentationRoot);
    app.update();

    let mut markers = app.world_mut().query::<&ExtractionVisual>();
    assert_eq!(markers.iter(app.world()).count(), 0);
}

#[test]
fn vanguard_pilot_arms_aegis_and_shields_an_adjacent_ally() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    let mut interaction = InteractionState::default();

    // The authored deployment has no orthogonal adjacency, so step the
    // Vanguard next to the Gunner before arming the pilot skill.
    route_cell_click(&mut battle, &mut interaction, GridPos::new(4, 7)).unwrap();
    assert_eq!(interaction.selected_unit, Some(ids::VANGUARD));
    execute_command(&mut battle, &mut interaction, CommandAction::Move).unwrap();
    route_cell_click(&mut battle, &mut interaction, GridPos::new(4, 8)).unwrap();

    execute_command(&mut battle, &mut interaction, CommandAction::PilotSkill).unwrap();
    assert_eq!(interaction.mode, InteractionMode::AegisTarget);

    // Clicking the enemy Striker is rejected and returns to Inspect.
    route_cell_click(&mut battle, &mut interaction, GridPos::new(4, 4)).unwrap_err();
    assert_eq!(interaction.mode, InteractionMode::Inspect);
    assert_eq!(battle.pilot_skills().aegis_target, None);

    execute_command(&mut battle, &mut interaction, CommandAction::PilotSkill).unwrap();
    route_cell_click(&mut battle, &mut interaction, GridPos::new(3, 8)).unwrap();
    assert_eq!(interaction.mode, InteractionMode::Inspect);
    assert_eq!(battle.pilot_skills().aegis_target, Some(ids::GUNNER));
    assert!(battle.pilot_skills().aegis_used);
}

#[test]
fn gunner_pilot_sets_focus_pending() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    let mut interaction = InteractionState::default();

    route_cell_click(&mut battle, &mut interaction, GridPos::new(3, 8)).unwrap();
    assert_eq!(interaction.selected_unit, Some(ids::GUNNER));
    execute_command(&mut battle, &mut interaction, CommandAction::PilotSkill).unwrap();

    assert_eq!(interaction.mode, InteractionMode::Inspect);
    let pilot = battle.pilot_skills();
    assert!(pilot.focus_used);
    assert!(pilot.focus_pending);
}

#[test]
fn interceptor_pilot_overdrive_raises_movement_allowance() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    let mut interaction = InteractionState::default();

    route_cell_click(&mut battle, &mut interaction, GridPos::new(5, 8)).unwrap();
    assert_eq!(interaction.selected_unit, Some(ids::INTERCEPTOR));
    assert_eq!(battle.movement_allowance(ids::INTERCEPTOR).unwrap(), 4);

    execute_command(&mut battle, &mut interaction, CommandAction::PilotSkill).unwrap();

    assert_eq!(battle.movement_allowance(ids::INTERCEPTOR).unwrap(), 6);
    assert!(battle.pilot_skills().overdrive_used);
}

#[test]
fn restart_replaces_presentation_root_and_transient_state() {
    let mut app = presentation_fixture_app();
    app.update();
    let old_root = app
        .world_mut()
        .query_filtered::<Entity, With<PresentationRoot>>()
        .single(app.world())
        .unwrap();
    let stale_child = app.world_mut().spawn(ChildOf(old_root)).id();

    app.world_mut()
        .resource_mut::<InteractionState>()
        .selected_unit = Some(ids::VANGUARD);
    app.world_mut()
        .resource_mut::<BattleEventQueue>()
        .0
        .push_back(scorpius::domain::model::BattleEvent::OptionalObjectiveCompleted);
    app.world_mut().resource_mut::<EventPlayback>().input_locked = true;
    app.world_mut().resource_mut::<StatusMessage>().0 = "stale".to_owned();
    app.world_mut()
        .resource_mut::<AttackPreviewCells>()
        .0
        .insert(GridPos::new(4, 4));
    app.world_mut().resource_mut::<SelectedCell>().0 = Some(GridPos::new(4, 4));
    restart_battle(app.world_mut(), 11);
    app.update();

    let new_root = app
        .world_mut()
        .query_filtered::<Entity, With<PresentationRoot>>()
        .single(app.world())
        .unwrap();
    assert_ne!(new_root, old_root);
    assert!(app.world().get_entity(stale_child).is_err());
    assert_eq!(
        app.world().resource::<InteractionState>().selected_unit,
        None
    );
    assert!(app.world().resource::<BattleEventQueue>().0.is_empty());
    assert!(!app.world().resource::<EventPlayback>().input_locked);
    assert!(app.world().resource::<StatusMessage>().0.is_empty());
    assert!(app.world().resource::<AttackPreviewCells>().0.is_empty());
    assert_eq!(app.world().resource::<SelectedCell>().0, None);
    assert_eq!(app.world().resource::<BattleRuntime>().0.round(), 0);
}

#[test]
fn terminal_overlay_copy_matches_the_mission_result() {
    let definition = mission_definition(MissionId::One).unwrap();
    assert_eq!(
        result_overlay_copy(
            MissionResult {
                victory: true,
                optional_complete: true,
                rounds: 2,
            },
            mission_one(7).rules().primary,
            definition,
        ),
        "MISSION COMPLETE\nMission 1 — Turnabout at Relay Nine\nBONUS Achieved"
    );
    assert_eq!(
        result_overlay_copy(
            MissionResult {
                victory: false,
                optional_complete: false,
                rounds: 3,
            },
            mission_one(7).rules().primary,
            definition,
        ),
        "MISSION FAILED\nSquad knocked out"
    );
}

fn defeat_result() -> MissionResult {
    MissionResult {
        victory: false,
        optional_complete: false,
        rounds: 3,
    }
}

#[test]
fn defeat_overlay_names_the_lost_protect_target() {
    let definition = mission_definition(MissionId::Two).unwrap();
    assert_eq!(
        result_overlay_copy(defeat_result(), mission_two(7).rules().primary, definition),
        "MISSION FAILED\nProtect target lost"
    );
}

#[test]
fn defeat_overlay_names_the_escaped_courier() {
    let definition = mission_definition(MissionId::Three).unwrap();
    assert_eq!(
        result_overlay_copy(
            defeat_result(),
            mission_three(7).rules().primary,
            definition
        ),
        "MISSION FAILED\nCourier not stopped in time"
    );
}
