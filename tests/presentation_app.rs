use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use bevy::prelude::*;
use scorpius::{
    campaign::{model::CampaignState, save::SaveFile, session::CampaignSession},
    domain::{
        battle::BattleState,
        board::GridPos,
        model::{BattlePhase, MissionResult, Reaction, UnitId},
    },
    mission::mission_one::{ids, mission_one},
    mission::{MissionId, mission_definition},
    presentation::{
        ActiveMission, AttackPreviewCells, BattleEventQueue, BattleRuntime, CampaignRuntime,
        EventPlayback, PresentationRoot, SelectedCell, TelegraphVisual, UnitVisual,
        battlefield::mission_grid_cells,
        grid_to_world,
        interaction::{
            CommandAction, InteractionMode, InteractionState, StatusMessage, execute_command,
            handle_viability_cell_click, restart_battle, route_cell_click, update_hover_preview,
        },
        sync::{apply_unit_transforms, reconcile_telegraph_markers},
        ui::{HudSnapshot, result_overlay_copy},
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
    let hud = HudSnapshot::from_battle(&battle, Some(ids::VANGUARD));

    assert_eq!(hud.round_phase, "Round 1 · Player Phase");
    assert_eq!(hud.primary, "Eliminate all enemies · 4 remaining");
    assert_eq!(hud.optional, "Turnabout · Not yet");
    assert_eq!(hud.selected_name, Some("Vanguard"));
    assert_eq!(hud.threats.len(), 4);
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
    assert_eq!(
        result_overlay_copy(MissionResult {
            victory: true,
            turnabout_complete: true,
            rounds: 2,
        }),
        "MISSION COMPLETE\nRelay Nine secured\nTurnabout: Achieved"
    );
    assert_eq!(
        result_overlay_copy(MissionResult {
            victory: false,
            turnabout_complete: false,
            rounds: 3,
        }),
        "MISSION FAILED\nSquad knocked out"
    );
}
