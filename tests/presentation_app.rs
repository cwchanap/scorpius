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
    mission::mission_five::mission_five,
    mission::mission_four::mission_four,
    mission::mission_one::{ids, mission_one},
    mission::mission_seven::mission_seven,
    mission::mission_three::{self, mission_three},
    mission::mission_two::mission_two,
    mission::{MissionId, mission_definition},
    presentation::{
        ActiveMission, AttackPreviewCells, BattleEventQueue, BattleRuntime, CampaignRuntime,
        CellInsetVisual, CellVisual, EventPlayback, ExtractionVisual, PresentationRoot,
        TelegraphVisual, TokenFootprintVisual, TokenSelectionVisual, UnitVisual,
        assets::UiAssets,
        battlefield::{mission_grid_cells, setup_mission_scene},
        interaction::{
            CommandAction, InteractionMode, InteractionState, StatusMessage, execute_command,
            handle_viability_cell_click, restart_battle, route_cell_click, update_hover_preview,
        },
        sync::{
            apply_unit_transforms, reconcile_extraction_marker, reconcile_telegraph_markers,
            sync_cell_highlights,
        },
        theme,
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
        .init_resource::<AttackPreviewCells>();
    app.world_mut().spawn(PresentationRoot);
    app
}

fn blank_ui_assets() -> UiAssets {
    UiAssets {
        key_art: Handle::default(),
        briefing_art: Handle::default(),
        vanguard_art: Handle::default(),
        gunner_art: Handle::default(),
        interceptor_art: Handle::default(),
        icons: Handle::default(),
        board: Handle::default(),
        fonts: std::array::from_fn(|_| Handle::default()),
    }
}

#[test]
fn canonical_move_drives_visual_transform_without_renderer() {
    let mut app = App::new();
    app.insert_resource(BattleRuntime(BattleState::viability_fixture()))
        .add_systems(Update, apply_unit_transforms);
    app.world_mut().spawn((
        UnitVisual(UnitId(1)),
        Node {
            position_type: PositionType::Absolute,
            ..default()
        },
        Visibility::Visible,
    ));

    app.world_mut()
        .resource_mut::<BattleRuntime>()
        .0
        .move_unit(UnitId(1), GridPos::new(1, 2))
        .unwrap();
    app.update();

    let mut nodes = app.world_mut().query::<&Node>();
    let node = nodes.single(app.world()).unwrap();
    let center = scorpius::presentation::layout::iso_center(GridPos::new(1, 2))
        - scorpius::presentation::layout::battle_stage_rect().min;
    assert_eq!(node.left, px(center.x - 38.0));
    assert_eq!(node.top, px(center.y - 68.0));
}

#[test]
fn token_shadow_and_selection_footprints_follow_domain_positions() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    battle.begin_activation(ids::VANGUARD).unwrap();

    let mut app = App::new();
    app.insert_resource(BattleRuntime(battle))
        .insert_resource(InteractionState {
            inspected_unit: Some(ids::STRIKER),
            ..default()
        })
        .add_systems(Update, apply_unit_transforms);
    let shadow = app
        .world_mut()
        .spawn((
            TokenFootprintVisual(ids::VANGUARD),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            Visibility::Hidden,
        ))
        .id();
    let active_ring = app
        .world_mut()
        .spawn((
            TokenSelectionVisual(ids::VANGUARD),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            ImageNode::default(),
            Visibility::Hidden,
        ))
        .id();
    let inspected_ring = app
        .world_mut()
        .spawn((
            TokenSelectionVisual(ids::STRIKER),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            ImageNode::default(),
            Visibility::Hidden,
        ))
        .id();

    app.update();
    let moved_to = GridPos::new(4, 8);
    app.world_mut()
        .resource_mut::<BattleRuntime>()
        .0
        .move_unit(ids::VANGUARD, moved_to)
        .unwrap();
    app.update();

    let center = scorpius::presentation::layout::iso_center(moved_to)
        - scorpius::presentation::layout::battle_stage_rect().min;
    let shadow_node = app.world().get::<Node>(shadow).unwrap();
    assert_eq!(shadow_node.left, px(center.x - 56.0));
    assert_eq!(shadow_node.top, px(center.y - 68.0));
    assert_eq!(
        app.world().get::<Visibility>(shadow),
        Some(&Visibility::Visible)
    );

    let active_node = app.world().get::<Node>(active_ring).unwrap();
    assert_eq!(active_node.left, px(center.x - 56.0));
    assert_eq!(active_node.top, px(center.y - 28.0));
    assert_eq!(
        app.world().get::<Visibility>(active_ring),
        Some(&Visibility::Visible)
    );
    assert_eq!(
        app.world().get::<ImageNode>(active_ring).unwrap().color,
        theme::BOARD_SELECTED
    );

    assert_eq!(
        app.world().get::<Visibility>(inspected_ring),
        Some(&Visibility::Visible)
    );
    assert_eq!(
        app.world().get::<ImageNode>(inspected_ring).unwrap().color,
        theme::BOARD_INSPECTED
    );
}

#[test]
fn mission_cells_have_source_stroke_and_inset_layers() {
    let mut app = App::new();
    app.insert_resource(BattleRuntime(mission_one(7)))
        .insert_resource(blank_ui_assets())
        .add_systems(Update, setup_mission_scene);
    app.update();

    let mut cells = app
        .world_mut()
        .query::<(&CellVisual, &Node, &ImageNode, Option<&Pickable>)>();
    let cell = cells
        .iter(app.world())
        .find(|(visual, ..)| visual.0 == GridPos::new(0, 0))
        .expect("authored cell root");
    assert_eq!(cell.1.width, px(112.0));
    assert_eq!(cell.1.height, px(56.0));
    assert_eq!(cell.2.color, theme::BOARD_STROKE);
    assert!(cell.3.is_none(), "outer stroke must not be pickable");

    let mut insets = app
        .world_mut()
        .query::<(&CellInsetVisual, &Node, &ImageNode, &Pickable)>();
    let inset = insets
        .iter(app.world())
        .find(|(visual, ..)| visual.0 == GridPos::new(0, 0))
        .expect("authored cell inset");
    assert_eq!(inset.1.left, px(3.0));
    assert_eq!(inset.1.top, px(3.0));
    assert_eq!(inset.1.width, px(106.0));
    assert_eq!(inset.1.height, px(50.0));
    assert_eq!(inset.2.color, theme::BOARD_LIGHT);
    assert_eq!(inset.3, &Pickable::IGNORE);
}

#[test]
fn inspecting_enemy_keeps_active_unit_commands_and_preview_authority() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    battle.begin_activation(ids::VANGUARD).unwrap();
    let mut interaction = InteractionState::default();
    let striker_position = battle.unit(ids::STRIKER).unwrap().position;

    route_cell_click(&mut battle, &mut interaction, striker_position).unwrap();
    assert_eq!(interaction.inspected_unit, Some(ids::STRIKER));
    interaction.mode = InteractionMode::Attack(ids::REPULSOR_RAM);
    update_hover_preview(&battle, &mut interaction, striker_position);
    assert_eq!(
        interaction.preview.as_ref().map(|preview| preview.attacker),
        Some(ids::VANGUARD)
    );

    interaction.mode = InteractionMode::Move;
    execute_command(&mut battle, &mut interaction, CommandAction::Move).unwrap();

    let reachable_cell = battle
        .reachable_cells(ids::VANGUARD)
        .unwrap()
        .into_iter()
        .next()
        .expect("the active Vanguard must have a reachable cell");
    let mut highlight_app = App::new();
    highlight_app
        .insert_resource(BattleRuntime(battle.clone()))
        .insert_resource(InteractionState {
            inspected_unit: interaction.inspected_unit,
            hovered_cell: interaction.hovered_cell,
            mode: interaction.mode,
            preview: None,
        })
        .add_systems(Update, sync_cell_highlights);
    let outer = highlight_app
        .world_mut()
        .spawn((CellVisual(reachable_cell), ImageNode::default()))
        .id();
    let inset = highlight_app
        .world_mut()
        .spawn((CellInsetVisual(reachable_cell), ImageNode::default()))
        .id();
    highlight_app.update();
    assert_eq!(
        highlight_app.world().get::<ImageNode>(outer).unwrap().color,
        theme::BOARD_SELECTED,
        "Move highlights must follow the active ally after inspecting an enemy"
    );
    assert_eq!(
        highlight_app.world().get::<ImageNode>(inset).unwrap().color,
        theme::BOARD_REACHABLE,
    );

    route_cell_click(&mut battle, &mut interaction, GridPos::new(4, 8)).unwrap();
    assert_eq!(
        battle.unit(ids::VANGUARD).unwrap().position,
        GridPos::new(4, 8)
    );
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
        .insert_resource(blank_ui_assets())
        .add_systems(Update, reconcile_telegraph_markers);
    app.update();

    let actual: BTreeSet<_> = app
        .world_mut()
        .query::<&TelegraphVisual>()
        .iter(app.world())
        .map(|marker| (marker.attacker, marker.cell))
        .collect();
    assert_eq!(actual, expected);

    let mut markers = app
        .world_mut()
        .query::<(&TelegraphVisual, &ImageNode, Option<&BackgroundColor>)>();
    for (_, image, background) in markers.iter(app.world()) {
        assert_eq!(image.color, theme::BOARD_TELEGRAPH);
        assert!(
            background.is_none_or(|background| background.0 == Color::NONE),
            "telegraph background must stay transparent; color comes from ImageNode"
        );
    }
}

#[test]
fn selected_unit_can_move_then_arm_a_weapon() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    let mut interaction = InteractionState::default();

    route_cell_click(&mut battle, &mut interaction, GridPos::new(5, 8)).unwrap();
    assert_eq!(interaction.inspected_unit, Some(ids::INTERCEPTOR));
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
    assert_eq!(interaction.inspected_unit, None);
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
    app.insert_resource(BattleRuntime(battle))
        .insert_resource(blank_ui_assets())
        .add_systems(Update, reconcile_extraction_marker);
    app.world_mut().spawn(PresentationRoot);
    app.update();

    let mut markers = app.world_mut().query::<(
        &ExtractionVisual,
        &Node,
        &ImageNode,
        Option<&BackgroundColor>,
    )>();
    let markers: Vec<_> = markers.iter(app.world()).collect();
    assert_eq!(markers.len(), 1, "exactly one extraction ring");
    let (marker, node, image, background) = markers[0];
    assert_eq!(marker.0, GridPos::new(8, 0));
    assert_eq!(marker.0, mission_three::EXTRACTION);
    assert_eq!(node.width, px(112.0));
    assert_eq!(image.color, scorpius::presentation::theme::BOARD_EXTRACTION);
    assert!(
        background.is_none_or(|background| background.0 == Color::NONE),
        "extraction background must stay transparent; color comes from ImageNode"
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
    assert_eq!(interaction.inspected_unit, Some(ids::VANGUARD));
    execute_command(&mut battle, &mut interaction, CommandAction::Move).unwrap();
    route_cell_click(&mut battle, &mut interaction, GridPos::new(4, 8)).unwrap();

    execute_command(&mut battle, &mut interaction, CommandAction::PilotSkill).unwrap();
    assert_eq!(interaction.mode, InteractionMode::AegisTarget);

    // Clicking the enemy Striker is rejected while keeping Aegis targeting armed.
    route_cell_click(&mut battle, &mut interaction, GridPos::new(4, 4)).unwrap_err();
    assert_eq!(interaction.mode, InteractionMode::AegisTarget);
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
    assert_eq!(interaction.inspected_unit, Some(ids::GUNNER));
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
    assert_eq!(interaction.inspected_unit, Some(ids::INTERCEPTOR));
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
        .inspected_unit = Some(ids::VANGUARD);
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
        app.world().resource::<InteractionState>().inspected_unit,
        None
    );
    assert!(app.world().resource::<BattleEventQueue>().0.is_empty());
    assert!(!app.world().resource::<EventPlayback>().input_locked);
    assert!(app.world().resource::<StatusMessage>().0.is_empty());
    assert!(app.world().resource::<AttackPreviewCells>().0.is_empty());
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
