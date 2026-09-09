use bevy::prelude::{NextState, Timer};

use scorpius::{
    app::GameScreen,
    domain::{
        battle::BattleState,
        board::GridPos,
        model::{BattleError, BattlePhase, Reaction},
    },
    mission::mission_one::{ids, mission_one},
    presentation::{
        EventPlayback,
        assets::AssetLoadStatus,
        battle_menu::{MenuAction, MenuState, apply_menu_action},
        interaction::{
            CommandAction, InteractionMode, InteractionState, execute_command, next_ready_unit,
            restart_allowed, route_cell_click,
        },
    },
};

fn active_vanguard() -> BattleState {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    battle.begin_activation(ids::VANGUARD).unwrap();
    battle
}

fn inspect_active_vanguard() -> InteractionState {
    InteractionState {
        inspected_unit: Some(ids::VANGUARD),
        menu: MenuState::Root,
        ..Default::default()
    }
}

#[test]
fn root_weapons_back_and_stances_back_return_to_root() {
    let battle = active_vanguard();
    let mut interaction = inspect_active_vanguard();

    assert!(apply_menu_action(
        &battle,
        &mut interaction,
        MenuAction::Open(MenuState::Weapons)
    ));
    assert_eq!(interaction.menu, MenuState::Weapons);
    assert!(apply_menu_action(
        &battle,
        &mut interaction,
        MenuAction::Back
    ));
    assert_eq!(interaction.menu, MenuState::Root);

    assert!(apply_menu_action(
        &battle,
        &mut interaction,
        MenuAction::Open(MenuState::Stances)
    ));
    assert_eq!(interaction.menu, MenuState::Stances);
    assert!(apply_menu_action(
        &battle,
        &mut interaction,
        MenuAction::Back
    ));
    assert_eq!(interaction.menu, MenuState::Root);
}

#[test]
fn menu_navigation_cannot_change_state_without_active_inspect_authority() {
    let battle = active_vanguard();
    let mut interaction = InteractionState {
        inspected_unit: Some(ids::VANGUARD),
        menu: MenuState::Root,
        mode: InteractionMode::Move,
        ..Default::default()
    };
    assert!(!apply_menu_action(
        &battle,
        &mut interaction,
        MenuAction::Open(MenuState::Weapons)
    ));
    assert_eq!(interaction.menu, MenuState::Root);

    interaction.mode = InteractionMode::Inspect;
    // A fresh planning state is also a useful no-authority guard.
    let no_active = mission_one(7);
    assert!(!apply_menu_action(
        &no_active,
        &mut interaction,
        MenuAction::Open(MenuState::Weapons)
    ));
}

#[test]
fn targeting_hides_menu_invalid_targets_keep_mode_and_cancel_restores_root() {
    let mut battle = active_vanguard();
    let mut interaction = inspect_active_vanguard();

    execute_command(&mut battle, &mut interaction, CommandAction::Move).unwrap();
    assert_eq!(interaction.mode, InteractionMode::Move);
    assert_eq!(interaction.menu, MenuState::Hidden);
    assert_eq!(
        route_cell_click(&mut battle, &mut interaction, GridPos::new(4, 6)),
        Err(BattleError::DestinationOccupied(GridPos::new(4, 6)))
    );
    assert_eq!(interaction.mode, InteractionMode::Move);
    assert_eq!(interaction.menu, MenuState::Hidden);
    execute_command(&mut battle, &mut interaction, CommandAction::Cancel).unwrap();
    assert_eq!(interaction.mode, InteractionMode::Inspect);
    assert_eq!(interaction.menu, MenuState::Root);

    execute_command(&mut battle, &mut interaction, CommandAction::WeaponSlot(0)).unwrap();
    assert!(matches!(interaction.mode, InteractionMode::Attack(_)));
    assert_eq!(interaction.menu, MenuState::Hidden);
    assert!(route_cell_click(&mut battle, &mut interaction, GridPos::new(99, 99)).is_err());
    assert!(matches!(interaction.mode, InteractionMode::Attack(_)));
    execute_command(&mut battle, &mut interaction, CommandAction::Cancel).unwrap();
    assert_eq!(interaction.menu, MenuState::Root);

    execute_command(&mut battle, &mut interaction, CommandAction::PilotSkill).unwrap();
    assert_eq!(interaction.mode, InteractionMode::AegisTarget);
    assert_eq!(interaction.menu, MenuState::Hidden);
    assert!(route_cell_click(&mut battle, &mut interaction, GridPos::new(4, 6)).is_err());
    assert_eq!(interaction.mode, InteractionMode::AegisTarget);
    execute_command(&mut battle, &mut interaction, CommandAction::Cancel).unwrap();
    assert_eq!(interaction.mode, InteractionMode::Inspect);
    assert_eq!(interaction.menu, MenuState::Root);
}

#[test]
fn successful_targeting_reopens_root_and_wait_hands_off_in_order() {
    let mut battle = active_vanguard();
    let mut interaction = inspect_active_vanguard();

    execute_command(&mut battle, &mut interaction, CommandAction::Move).unwrap();
    route_cell_click(&mut battle, &mut interaction, GridPos::new(4, 8)).unwrap();
    assert_eq!(interaction.mode, InteractionMode::Inspect);
    assert_eq!(interaction.menu, MenuState::Root);
    assert_eq!(
        battle.unit(ids::VANGUARD).unwrap().position,
        GridPos::new(4, 8)
    );

    execute_command(
        &mut battle,
        &mut interaction,
        CommandAction::Reaction(Reaction::Guard),
    )
    .unwrap();
    execute_command(&mut battle, &mut interaction, CommandAction::FinishUnit).unwrap();
    assert_eq!(battle.active_unit(), Some(ids::GUNNER));
    assert_eq!(interaction.inspected_unit, Some(ids::GUNNER));
    assert_eq!(interaction.menu, MenuState::Root);

    execute_command(
        &mut battle,
        &mut interaction,
        CommandAction::Reaction(Reaction::Counter),
    )
    .unwrap();
    execute_command(&mut battle, &mut interaction, CommandAction::FinishUnit).unwrap();
    assert_eq!(battle.active_unit(), Some(ids::INTERCEPTOR));
    assert_eq!(interaction.inspected_unit, Some(ids::INTERCEPTOR));
    assert_eq!(interaction.menu, MenuState::Root);

    execute_command(
        &mut battle,
        &mut interaction,
        CommandAction::Reaction(Reaction::Evade),
    )
    .unwrap();
    execute_command(&mut battle, &mut interaction, CommandAction::FinishUnit).unwrap();
    assert_eq!(battle.active_unit(), None);
    assert_eq!(interaction.inspected_unit, None);
    assert_eq!(interaction.menu, MenuState::Hidden);
    assert!(battle.ready_to_resolve());
}

#[test]
fn inspecting_enemy_or_finished_pilot_never_transfers_active_authority() {
    let mut battle = active_vanguard();
    let mut interaction = inspect_active_vanguard();

    route_cell_click(&mut battle, &mut interaction, GridPos::new(4, 6)).unwrap();
    assert_eq!(interaction.inspected_unit, Some(ids::STRIKER));
    assert_eq!(battle.active_unit(), Some(ids::VANGUARD));
    execute_command(&mut battle, &mut interaction, CommandAction::Move).unwrap();
    assert_eq!(interaction.mode, InteractionMode::Move);

    execute_command(&mut battle, &mut interaction, CommandAction::Cancel).unwrap();
    execute_command(
        &mut battle,
        &mut interaction,
        CommandAction::Reaction(Reaction::Guard),
    )
    .unwrap();
    execute_command(&mut battle, &mut interaction, CommandAction::FinishUnit).unwrap();
    assert_eq!(battle.active_unit(), Some(ids::GUNNER));

    route_cell_click(&mut battle, &mut interaction, GridPos::new(4, 7)).unwrap();
    assert_eq!(interaction.inspected_unit, Some(ids::VANGUARD));
    assert_eq!(battle.active_unit(), Some(ids::GUNNER));
    assert_eq!(interaction.menu, MenuState::Root);
}

#[test]
fn next_ready_and_restart_guards_cover_idle_playback_and_pending_states() {
    let mut battle = mission_one(7);
    battle.begin_round().unwrap();
    assert_eq!(next_ready_unit(&battle), Some(ids::VANGUARD));

    let ready = AssetLoadStatus::Ready;
    let loading = AssetLoadStatus::Loading;
    let playback = EventPlayback::default();
    let next = NextState::<GameScreen>::Unchanged;
    assert!(restart_allowed(&battle, &ready, &playback, &next, false));
    assert!(!restart_allowed(&battle, &loading, &playback, &next, false));

    let mut locked = EventPlayback {
        input_locked: true,
        ..Default::default()
    };
    assert!(!restart_allowed(&battle, &ready, &locked, &next, false));
    locked.input_locked = false;
    locked.current = Some((
        scorpius::domain::model::BattleEvent::UnitMoved {
            unit: ids::VANGUARD,
            from: GridPos::new(4, 7),
            to: GridPos::new(4, 8),
        },
        Timer::from_seconds(1.0, bevy::time::TimerMode::Once),
    ));
    assert!(!restart_allowed(&battle, &ready, &locked, &next, false));
    assert!(!restart_allowed(&battle, &ready, &playback, &next, true));

    let mut pending = NextState::<GameScreen>::Unchanged;
    pending.set(GameScreen::Aftermath);
    assert!(!restart_allowed(
        &battle, &ready, &playback, &pending, false
    ));

    battle.begin_activation(ids::VANGUARD).unwrap();
    assert!(!restart_allowed(&battle, &ready, &playback, &next, false));
    assert_eq!(battle.phase(), BattlePhase::Player);
}
