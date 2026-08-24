use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;

use crate::domain::{
    battle::BattleState,
    board::GridPos,
    combat::AttackPreview,
    model::{BattleError, BattleEvent, BattlePhase, Faction, Reaction, UnitId, WeaponId},
};

use super::{
    AttackPreviewCells, BattleEventQueue, BattleRuntime, CellVisual, SelectedCell,
    assets::{AssetLoadStatus, mission_assets_ready},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InteractionMode {
    #[default]
    Inspect,
    Move,
    Attack(WeaponId),
}

#[derive(Resource, Default)]
pub struct InteractionState {
    pub selected_unit: Option<UnitId>,
    pub hovered_cell: Option<GridPos>,
    pub mode: InteractionMode,
    pub preview: Option<AttackPreview>,
}

#[derive(Resource, Default)]
pub struct StatusMessage(pub String);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandAction {
    Move,
    WeaponSlot(usize),
    Reaction(Reaction),
    FinishUnit,
    ResolveAttacks,
    Restart,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandButton(pub CommandAction);

pub fn route_cell_click(
    battle: &mut BattleState,
    interaction: &mut InteractionState,
    clicked: GridPos,
) -> Result<Vec<BattleEvent>, BattleError> {
    match interaction.mode {
        InteractionMode::Move => {
            let unit = interaction
                .selected_unit
                .ok_or(BattleError::NoUnitSelected)?;
            let events = battle.move_unit(unit, clicked)?;
            interaction.mode = InteractionMode::Inspect;
            interaction.hovered_cell = Some(clicked);
            interaction.preview = None;
            Ok(events)
        }
        InteractionMode::Attack(weapon) => {
            let unit = interaction
                .selected_unit
                .ok_or(BattleError::NoUnitSelected)?;
            let events = battle.attack(unit, weapon, clicked)?;
            interaction.mode = InteractionMode::Inspect;
            interaction.hovered_cell = Some(clicked);
            interaction.preview = None;
            Ok(events)
        }
        InteractionMode::Inspect => {
            if let Some(unit_id) = battle.occupant_at(clicked) {
                let unit = battle
                    .unit(unit_id)
                    .ok_or(BattleError::UnknownUnit(unit_id))?;
                if unit.faction == Faction::Player {
                    let should_begin = battle.phase() == BattlePhase::Player
                        && battle.active_unit() != Some(unit_id)
                        && !unit.activation.finished
                        && !unit.is_knocked_out();
                    if should_begin {
                        battle.begin_activation(unit_id)?;
                    }
                    interaction.selected_unit = Some(unit_id);
                }
            }
            interaction.hovered_cell = Some(clicked);
            interaction.preview = None;
            Ok(Vec::new())
        }
    }
}

pub fn execute_command(
    battle: &mut BattleState,
    interaction: &mut InteractionState,
    action: CommandAction,
) -> Result<Vec<BattleEvent>, BattleError> {
    match action {
        CommandAction::Move => {
            let unit_id = require_selected_active_unit(battle, interaction)?;
            let unit = battle
                .unit(unit_id)
                .ok_or(BattleError::UnknownUnit(unit_id))?;
            if unit.activation.moved {
                return Err(BattleError::MoveAlreadySpent(unit_id));
            }
            interaction.mode = InteractionMode::Move;
            interaction.preview = None;
            Ok(Vec::new())
        }
        CommandAction::WeaponSlot(slot) => {
            let unit_id = require_selected_active_unit(battle, interaction)?;
            let unit = battle
                .unit(unit_id)
                .ok_or(BattleError::UnknownUnit(unit_id))?;
            if unit.activation.acted {
                return Err(BattleError::ActionAlreadySpent(unit_id));
            }
            let weapon_id = unit
                .weapons
                .get(slot)
                .copied()
                .ok_or(BattleError::UnknownWeapon(WeaponId(0)))?;
            let weapon = battle
                .weapon(weapon_id)
                .ok_or(BattleError::UnknownWeapon(weapon_id))?;
            if unit.en < weapon.en_cost {
                return Err(BattleError::InsufficientEn {
                    unit: unit_id,
                    required: weapon.en_cost,
                    available: unit.en,
                });
            }
            interaction.mode = InteractionMode::Attack(weapon_id);
            interaction.preview = interaction
                .hovered_cell
                .and_then(|cell| battle.preview_attack(unit_id, weapon_id, cell).ok());
            Ok(Vec::new())
        }
        CommandAction::Reaction(reaction) => {
            let unit = require_selected_active_unit(battle, interaction)?;
            battle.choose_reaction(unit, reaction)?;
            Ok(Vec::new())
        }
        CommandAction::FinishUnit => {
            let unit = require_selected_active_unit(battle, interaction)?;
            battle.finish_activation(unit)?;
            interaction.selected_unit = None;
            interaction.mode = InteractionMode::Inspect;
            interaction.preview = None;
            Ok(Vec::new())
        }
        CommandAction::ResolveAttacks => {
            let events = battle.resolve_enemy_phase()?;
            interaction.selected_unit = None;
            interaction.hovered_cell = None;
            interaction.mode = InteractionMode::Inspect;
            interaction.preview = None;
            Ok(events)
        }
        CommandAction::Restart => {
            if battle.result().is_none() {
                return Err(BattleError::WrongPhase {
                    expected: BattlePhase::Victory,
                    actual: battle.phase(),
                });
            }
            battle.restart_mission(fresh_seed());
            let events = battle.begin_round()?;
            *interaction = InteractionState::default();
            Ok(events)
        }
    }
}

pub fn update_hover_preview(
    battle: &BattleState,
    interaction: &mut InteractionState,
    cell: GridPos,
) {
    interaction.hovered_cell = Some(cell);
    interaction.preview = match (interaction.selected_unit, interaction.mode) {
        (Some(attacker), InteractionMode::Attack(weapon)) => {
            battle.preview_attack(attacker, weapon, cell).ok()
        }
        _ => None,
    };
}

pub fn handle_viability_cell_click(
    battle: &mut BattleState,
    selected: &mut Option<GridPos>,
    clicked: GridPos,
) -> Result<Vec<BattleEvent>, BattleError> {
    let unit_position = battle
        .unit(UnitId(1))
        .ok_or(BattleError::UnknownUnit(UnitId(1)))?
        .position;

    let events = if *selected == Some(unit_position) && clicked != unit_position {
        battle.move_unit(UnitId(1), clicked)?
    } else {
        Vec::new()
    };
    *selected = Some(clicked);
    Ok(events)
}

#[allow(clippy::too_many_arguments)]
pub fn on_battlefield_cell_click(
    click: On<Pointer<Click>>,
    cells: Query<&CellVisual>,
    mut battle: ResMut<BattleRuntime>,
    mut selected: ResMut<SelectedCell>,
    mut interaction: ResMut<InteractionState>,
    mut status: ResMut<StatusMessage>,
    mut event_queue: ResMut<BattleEventQueue>,
    mut preview_cells: ResMut<AttackPreviewCells>,
    asset_status: Res<AssetLoadStatus>,
) {
    if !mission_assets_ready(asset_status) {
        return;
    }
    let Ok(cell) = cells.get(click.entity) else {
        return;
    };
    selected.0 = Some(cell.0);
    match route_cell_click(&mut battle.0, &mut interaction, cell.0) {
        Ok(events) => {
            event_queue.0.extend(events);
            status.0.clear();
        }
        Err(error) => status.0 = error.to_string(),
    }
    copy_preview_cells(&interaction, &mut preview_cells);
}

pub fn on_battlefield_cell_over(
    over: On<Pointer<Over>>,
    cells: Query<&CellVisual>,
    battle: Res<BattleRuntime>,
    mut interaction: ResMut<InteractionState>,
    mut preview_cells: ResMut<AttackPreviewCells>,
    asset_status: Res<AssetLoadStatus>,
) {
    if !mission_assets_ready(asset_status) {
        return;
    }
    let Ok(cell) = cells.get(over.entity) else {
        return;
    };
    update_hover_preview(&battle.0, &mut interaction, cell.0);
    copy_preview_cells(&interaction, &mut preview_cells);
}

pub fn on_battlefield_cell_out(
    out: On<Pointer<Out>>,
    cells: Query<&CellVisual>,
    mut interaction: ResMut<InteractionState>,
    mut preview_cells: ResMut<AttackPreviewCells>,
) {
    let Ok(cell) = cells.get(out.entity) else {
        return;
    };
    if interaction.hovered_cell == Some(cell.0) {
        interaction.hovered_cell = None;
        interaction.preview = None;
        preview_cells.0.clear();
    }
}

fn copy_preview_cells(interaction: &InteractionState, cells: &mut AttackPreviewCells) {
    cells.0.clear();
    if let Some(preview) = &interaction.preview {
        cells.0.extend(preview.footprint.iter().copied());
    }
}

#[allow(clippy::too_many_arguments)]
pub fn on_command_button_click(
    click: On<Pointer<Click>>,
    buttons: Query<&CommandButton>,
    mut battle: ResMut<BattleRuntime>,
    mut interaction: ResMut<InteractionState>,
    mut status: ResMut<StatusMessage>,
    mut event_queue: ResMut<BattleEventQueue>,
    mut preview_cells: ResMut<AttackPreviewCells>,
    mut selected_cell: ResMut<SelectedCell>,
    asset_status: Res<AssetLoadStatus>,
) {
    if !mission_assets_ready(asset_status) {
        return;
    }
    let Ok(button) = buttons.get(click.entity) else {
        return;
    };
    run_command(
        button.0,
        &mut battle.0,
        &mut interaction,
        &mut status,
        &mut event_queue,
        &mut preview_cells,
        &mut selected_cell,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn handle_keyboard_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut battle: ResMut<BattleRuntime>,
    mut interaction: ResMut<InteractionState>,
    mut status: ResMut<StatusMessage>,
    mut event_queue: ResMut<BattleEventQueue>,
    mut preview_cells: ResMut<AttackPreviewCells>,
    mut selected_cell: ResMut<SelectedCell>,
    asset_status: Res<AssetLoadStatus>,
) {
    if !mission_assets_ready(asset_status) {
        return;
    }
    let action = if keyboard.just_pressed(KeyCode::KeyM) {
        Some(CommandAction::Move)
    } else if keyboard.just_pressed(KeyCode::Digit1) {
        Some(CommandAction::WeaponSlot(0))
    } else if keyboard.just_pressed(KeyCode::Digit2) {
        Some(CommandAction::WeaponSlot(1))
    } else if keyboard.just_pressed(KeyCode::Digit3) {
        Some(CommandAction::WeaponSlot(2))
    } else if keyboard.just_pressed(KeyCode::KeyC) {
        Some(CommandAction::Reaction(Reaction::Counter))
    } else if keyboard.just_pressed(KeyCode::KeyG) {
        Some(CommandAction::Reaction(Reaction::Guard))
    } else if keyboard.just_pressed(KeyCode::KeyE) {
        Some(CommandAction::Reaction(Reaction::Evade))
    } else if keyboard.just_pressed(KeyCode::KeyF) {
        Some(CommandAction::FinishUnit)
    } else if keyboard.just_pressed(KeyCode::Space) {
        Some(CommandAction::ResolveAttacks)
    } else if keyboard.just_pressed(KeyCode::KeyR) {
        Some(CommandAction::Restart)
    } else {
        None
    };
    let Some(action) = action else {
        return;
    };
    run_command(
        action,
        &mut battle.0,
        &mut interaction,
        &mut status,
        &mut event_queue,
        &mut preview_cells,
        &mut selected_cell,
    );
}

fn run_command(
    action: CommandAction,
    battle: &mut BattleState,
    interaction: &mut InteractionState,
    status: &mut StatusMessage,
    event_queue: &mut BattleEventQueue,
    preview_cells: &mut AttackPreviewCells,
    selected_cell: &mut SelectedCell,
) {
    match execute_command(battle, interaction, action) {
        Ok(events) => {
            event_queue.0.extend(events);
            status.0 = command_success_message(action).to_owned();
            if interaction.selected_unit.is_none() {
                selected_cell.0 = None;
            }
        }
        Err(error) => status.0 = error.to_string(),
    }
    copy_preview_cells(interaction, preview_cells);
}

fn require_selected_active_unit(
    battle: &BattleState,
    interaction: &InteractionState,
) -> Result<UnitId, BattleError> {
    if battle.phase() != BattlePhase::Player {
        return Err(BattleError::WrongPhase {
            expected: BattlePhase::Player,
            actual: battle.phase(),
        });
    }
    let unit = interaction
        .selected_unit
        .ok_or(BattleError::NoUnitSelected)?;
    if battle.active_unit() != Some(unit) {
        return Err(BattleError::UnitNotActive(unit));
    }
    Ok(unit)
}

const fn command_success_message(action: CommandAction) -> &'static str {
    match action {
        CommandAction::Move => "MOVE ARMED — choose a cyan destination.",
        CommandAction::WeaponSlot(_) => "WEAPON ARMED — choose an amber target.",
        CommandAction::Reaction(Reaction::Counter) => "COUNTER stance selected.",
        CommandAction::Reaction(Reaction::Guard) => "GUARD stance selected.",
        CommandAction::Reaction(Reaction::Evade) => "EVADE stance selected.",
        CommandAction::FinishUnit => "Unit finished.",
        CommandAction::ResolveAttacks => "Committed enemy attacks resolved.",
        CommandAction::Restart => "Mission restarted.",
    }
}

fn fresh_seed() -> u64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    elapsed.as_secs() ^ u64::from(elapsed.subsec_nanos())
}
