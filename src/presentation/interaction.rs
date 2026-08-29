use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;

use crate::app::GameScreen;
use crate::campaign::session::complete_current_mission;
use crate::domain::{
    battle::BattleState,
    board::GridPos,
    combat::AttackPreview,
    model::{
        BattleError, BattleEvent, BattlePhase, Faction, Reaction, UnitArchetype, UnitId, WeaponId,
    },
};

use super::{
    ActiveMission, AttackPreviewCells, BattleEventQueue, BattleRuntime, CampaignRuntime,
    CellVisual, EventPlayback, PresentationNeedsRebuild, PresentationRoot, RestartRequest,
    RestartRoundPending, SelectedCell,
    assets::{AssetLoadStatus, mission_assets_ready},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InteractionMode {
    #[default]
    Inspect,
    Move,
    Attack(WeaponId),
    AegisTarget,
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
    PilotSkill,
    Reaction(Reaction),
    FinishUnit,
    ResolveAttacks,
    Restart,
    ContinueVictory,
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
        InteractionMode::AegisTarget => {
            interaction.mode = InteractionMode::Inspect;
            interaction.hovered_cell = Some(clicked);
            interaction.preview = None;
            let ally = battle
                .occupant_at(clicked)
                .ok_or(BattleError::NoUnitSelected)?;
            battle.use_aegis(ally)?;
            Ok(Vec::new())
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
        CommandAction::PilotSkill => {
            let unit_id = require_selected_active_unit(battle, interaction)?;
            let unit = battle
                .unit(unit_id)
                .ok_or(BattleError::UnknownUnit(unit_id))?;
            match unit.archetype {
                UnitArchetype::Vanguard => {
                    interaction.mode = InteractionMode::AegisTarget;
                    interaction.preview = None;
                }
                UnitArchetype::Gunner => battle.use_focus()?,
                UnitArchetype::Interceptor => battle.use_overdrive()?,
                UnitArchetype::Rifleman
                | UnitArchetype::Striker
                | UnitArchetype::Artillery
                | UnitArchetype::Flanker => {
                    return Err(BattleError::PilotSkillWrongUnit(unit_id));
                }
            }
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
            // Restart is a defeat-only escape hatch; victory must continue
            // the campaign (see README "Controls").
            battle
                .result()
                .filter(|result| !result.victory)
                .ok_or(BattleError::WrongPhase {
                    expected: BattlePhase::Defeat,
                    actual: battle.phase(),
                })?;
            Ok(Vec::new())
        }
        CommandAction::ContinueVictory => {
            battle
                .result()
                .filter(|result| result.victory)
                .ok_or(BattleError::WrongPhase {
                    expected: BattlePhase::Victory,
                    actual: battle.phase(),
                })?;
            Ok(Vec::new())
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

pub fn restart_battle(world: &mut World, seed: u64) {
    let definition = world.resource::<ActiveMission>().0;
    let upgrades = world
        .resource::<CampaignRuntime>()
        .0
        .state
        .as_ref()
        .expect("restart requires active campaign")
        .upgrades
        .clone();
    world.resource_mut::<BattleRuntime>().0 = (definition.build)(seed, &upgrades);

    let roots: Vec<_> = world
        .query_filtered::<Entity, With<PresentationRoot>>()
        .iter(world)
        .collect();
    for root in roots {
        world.entity_mut(root).despawn();
    }

    reset_transient_battle_state(world);
    if let Some(mut pending) = world.get_resource_mut::<RestartRoundPending>() {
        pending.0 = true;
    }

    world.spawn((
        Name::new("Mission 1 Presentation"),
        PresentationRoot,
        PresentationNeedsRebuild,
        Transform::default(),
        Visibility::Visible,
    ));
}

/// Clear the interaction/playback/preview/selection state shared by restart and battle entry.
pub(crate) fn reset_transient_battle_state(world: &mut World) {
    *world.resource_mut::<InteractionState>() = InteractionState::default();
    *world.resource_mut::<StatusMessage>() = StatusMessage::default();
    world.resource_mut::<BattleEventQueue>().0.clear();
    *world.resource_mut::<EventPlayback>() = EventPlayback::default();
    world.resource_mut::<AttackPreviewCells>().0.clear();
    world.resource_mut::<SelectedCell>().0 = None;
}

pub(crate) fn process_restart_request(world: &mut World) {
    let seed = world.resource_mut::<RestartRequest>().0.take();
    if let Some(seed) = seed {
        restart_battle(world, seed);
    }
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
    mut playback: ResMut<EventPlayback>,
    mut preview_cells: ResMut<AttackPreviewCells>,
    asset_status: Res<AssetLoadStatus>,
) {
    if !mission_assets_ready(asset_status) || playback.input_locked {
        return;
    }
    let Ok(cell) = cells.get(click.entity) else {
        return;
    };
    selected.0 = Some(cell.0);
    match route_cell_click(&mut battle.0, &mut interaction, cell.0) {
        Ok(events) => {
            playback.input_locked |= !events.is_empty();
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
    playback: Res<EventPlayback>,
    asset_status: Res<AssetLoadStatus>,
) {
    if !mission_assets_ready(asset_status) || playback.input_locked {
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
    playback: Res<EventPlayback>,
) {
    if playback.input_locked {
        return;
    }
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
pub(crate) fn on_command_button_click(
    click: On<Pointer<Click>>,
    buttons: Query<&CommandButton>,
    mut battle: ResMut<BattleRuntime>,
    mut interaction: ResMut<InteractionState>,
    mut status: ResMut<StatusMessage>,
    mut event_queue: ResMut<BattleEventQueue>,
    mut preview_cells: ResMut<AttackPreviewCells>,
    mut selected_cell: ResMut<SelectedCell>,
    mut playback: ResMut<EventPlayback>,
    mut restart_request: ResMut<RestartRequest>,
    mut campaign: ResMut<CampaignRuntime>,
    active_mission: Res<ActiveMission>,
    mut next_state: ResMut<NextState<GameScreen>>,
    asset_status: Res<AssetLoadStatus>,
) {
    if !mission_assets_ready(asset_status) || playback.input_locked {
        return;
    }
    let Ok(button) = buttons.get(click.entity) else {
        return;
    };
    run_command(
        button.0,
        CommandContext {
            battle: &mut battle.0,
            campaign: &mut campaign,
            active_mission: &active_mission,
            next_state: &mut next_state,
            interaction: &mut interaction,
            status: &mut status,
            event_queue: &mut event_queue,
            preview_cells: &mut preview_cells,
            selected_cell: &mut selected_cell,
            playback: &mut playback,
            restart_request: &mut restart_request,
        },
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_keyboard_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut battle: ResMut<BattleRuntime>,
    mut interaction: ResMut<InteractionState>,
    mut status: ResMut<StatusMessage>,
    mut event_queue: ResMut<BattleEventQueue>,
    mut preview_cells: ResMut<AttackPreviewCells>,
    mut selected_cell: ResMut<SelectedCell>,
    mut playback: ResMut<EventPlayback>,
    mut restart_request: ResMut<RestartRequest>,
    mut campaign: ResMut<CampaignRuntime>,
    active_mission: Res<ActiveMission>,
    mut next_state: ResMut<NextState<GameScreen>>,
    asset_status: Res<AssetLoadStatus>,
) {
    if !mission_assets_ready(asset_status) || playback.input_locked {
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
    } else if keyboard.just_pressed(KeyCode::KeyP) {
        Some(CommandAction::PilotSkill)
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
        CommandContext {
            battle: &mut battle.0,
            campaign: &mut campaign,
            active_mission: &active_mission,
            next_state: &mut next_state,
            interaction: &mut interaction,
            status: &mut status,
            event_queue: &mut event_queue,
            preview_cells: &mut preview_cells,
            selected_cell: &mut selected_cell,
            playback: &mut playback,
            restart_request: &mut restart_request,
        },
    );
}

struct CommandContext<'a> {
    battle: &'a mut BattleState,
    campaign: &'a mut CampaignRuntime,
    active_mission: &'a ActiveMission,
    next_state: &'a mut NextState<GameScreen>,
    interaction: &'a mut InteractionState,
    status: &'a mut StatusMessage,
    event_queue: &'a mut BattleEventQueue,
    preview_cells: &'a mut AttackPreviewCells,
    selected_cell: &'a mut SelectedCell,
    playback: &'a mut EventPlayback,
    restart_request: &'a mut RestartRequest,
}

fn run_command(action: CommandAction, mut context: CommandContext<'_>) {
    if action == CommandAction::ContinueVictory {
        run_continue_victory(&mut context);
        return;
    }
    match execute_command(context.battle, context.interaction, action) {
        Ok(events) => {
            context.playback.input_locked |= !events.is_empty();
            context.event_queue.0.extend(events);
            context.status.0 = command_success_message(action, context.interaction.mode).to_owned();
            if action == CommandAction::Restart {
                context.restart_request.0 = Some(fresh_seed());
            }
            if context.interaction.selected_unit.is_none() {
                context.selected_cell.0 = None;
            }
        }
        Err(error) => context.status.0 = error.to_string(),
    }
    copy_preview_cells(context.interaction, context.preview_cells);
}

/// Save-backed victory continue: complete the mission identified by
/// [`ActiveMission`], persist it, and open the aftermath. Any failure keeps
/// Battle and the current campaign state untouched; the mission is never
/// re-derived from `runtime.0.state.next_mission` after completion.
fn run_continue_victory(context: &mut CommandContext<'_>) {
    let result = context.battle.result().filter(|result| result.victory);
    match result.map(|result| {
        complete_current_mission(&mut context.campaign.0, context.active_mission.0, result)
    }) {
        Some(Ok(_)) => {
            context.status.0 =
                command_success_message(CommandAction::ContinueVictory, context.interaction.mode)
                    .to_owned();
            context.next_state.set(GameScreen::Aftermath);
        }
        Some(Err(error)) => context.status.0 = error.to_string(),
        None => {
            context.status.0 = BattleError::WrongPhase {
                expected: BattlePhase::Victory,
                actual: context.battle.phase(),
            }
            .to_string();
        }
    }
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

fn command_success_message(action: CommandAction, mode: InteractionMode) -> &'static str {
    match action {
        CommandAction::Move => "MOVE ARMED — choose a cyan destination.",
        CommandAction::WeaponSlot(_) => "WEAPON ARMED — choose an amber target.",
        CommandAction::PilotSkill => {
            if mode == InteractionMode::AegisTarget {
                "AEGIS ARMED — click an adjacent ally to shield."
            } else {
                "Pilot skill engaged."
            }
        }
        CommandAction::Reaction(Reaction::Counter) => "COUNTER stance selected.",
        CommandAction::Reaction(Reaction::Guard) => "GUARD stance selected.",
        CommandAction::Reaction(Reaction::Evade) => "EVADE stance selected.",
        CommandAction::FinishUnit => "Unit finished.",
        CommandAction::ResolveAttacks => "Committed enemy attacks resolved.",
        CommandAction::Restart => "Mission restarted.",
        CommandAction::ContinueVictory => "Campaign progress saved.",
    }
}

fn fresh_seed() -> u64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    elapsed.as_secs() ^ u64::from(elapsed.subsec_nanos())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::campaign::model::CampaignState;
    use crate::campaign::save::SaveFile;
    use crate::campaign::session::CampaignSession;
    use crate::domain::combat::DamageSource;
    use crate::mission::mission_one::{ids, mission_one};
    use crate::mission::{MissionId, mission_definition};

    static NEXT_ID: AtomicU32 = AtomicU32::new(0);

    fn temp_save_path(label: &str) -> PathBuf {
        let n = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "scorpius-interaction-{label}-{}-{n}.json",
            std::process::id()
        ))
    }

    fn pending(next: &NextState<GameScreen>) -> Option<GameScreen> {
        match next {
            NextState::Unchanged => None,
            NextState::Pending(state) | NextState::PendingIfNeq(state) => Some(*state),
        }
    }

    fn terminal_victory_battle() -> BattleState {
        let mut battle = mission_one(7);
        for enemy in [
            ids::RIFLEMAN_LEFT,
            ids::RIFLEMAN_RIGHT,
            ids::STRIKER,
            ids::ARTILLERY,
        ] {
            battle.apply_direct_damage(enemy, 99, DamageSource::PlayerWeapon(ids::PILE_LANCE));
        }
        battle
    }

    #[allow(clippy::too_many_arguments)]
    fn run_continue(
        battle: &mut BattleState,
        runtime: &mut CampaignRuntime,
        active_mission: &ActiveMission,
        status: &mut StatusMessage,
        next: &mut NextState<GameScreen>,
    ) {
        let mut interaction = InteractionState::default();
        let mut event_queue = BattleEventQueue::default();
        let mut preview_cells = AttackPreviewCells::default();
        let mut selected_cell = SelectedCell::default();
        let mut playback = EventPlayback::default();
        let mut restart_request = RestartRequest::default();
        run_command(
            CommandAction::ContinueVictory,
            CommandContext {
                battle,
                campaign: runtime,
                active_mission,
                next_state: next,
                interaction: &mut interaction,
                status,
                event_queue: &mut event_queue,
                preview_cells: &mut preview_cells,
                selected_cell: &mut selected_cell,
                playback: &mut playback,
                restart_request: &mut restart_request,
            },
        );
    }

    #[test]
    fn restart_is_rejected_on_victory() {
        let mut battle = terminal_victory_battle();
        assert!(battle.result().is_some_and(|result| result.victory));
        let mut interaction = InteractionState::default();
        assert!(matches!(
            execute_command(&mut battle, &mut interaction, CommandAction::Restart),
            Err(BattleError::WrongPhase { .. })
        ));
    }

    #[test]
    fn continue_victory_completes_the_mission_and_opens_aftermath() {
        let mut session = CampaignSession::new(SaveFile::new(temp_save_path("continue-ok")));
        session.state = Some(CampaignState::new_game());
        session.save.store(&CampaignState::new_game()).unwrap();
        let mut runtime = CampaignRuntime(session);
        let mut battle = terminal_victory_battle();
        let active_mission = ActiveMission(mission_definition(MissionId::One).unwrap());
        let mut status = StatusMessage::default();
        let mut next = NextState::Unchanged;

        run_continue(
            &mut battle,
            &mut runtime,
            &active_mission,
            &mut status,
            &mut next,
        );

        let disk = runtime.0.save.load().unwrap().unwrap();
        assert_eq!(disk.next_mission, MissionId::Two);
        assert_eq!(disk.credits, 300);
        let state = runtime.0.state.as_ref().unwrap();
        assert_eq!(state.next_mission, MissionId::Two);
        assert_eq!(state.credits, 300);
        assert!(runtime.0.last_completion.is_some());
        assert_eq!(active_mission.0.id, MissionId::One);
        assert_eq!(pending(&next), Some(GameScreen::Aftermath));
        assert_eq!(status.0, "Campaign progress saved.");
    }

    #[test]
    fn continue_victory_save_failure_keeps_battle_and_campaign_state() {
        // A SaveFile whose parent path is an ordinary file makes store() fail
        // (create_dir_all on a file path errors), so write the file first.
        let blocker = temp_save_path("continue-blocker");
        std::fs::write(&blocker, b"not a directory").unwrap();
        let mut session = CampaignSession::new(SaveFile::new(blocker.join("campaign.json")));
        let mut original = CampaignState::new_game();
        original.credits = 120;
        session.state = Some(original);
        let mut runtime = CampaignRuntime(session);
        let mut battle = terminal_victory_battle();
        let active_mission = ActiveMission(mission_definition(MissionId::One).unwrap());
        let mut status = StatusMessage::default();
        let mut next = NextState::Unchanged;

        run_continue(
            &mut battle,
            &mut runtime,
            &active_mission,
            &mut status,
            &mut next,
        );

        assert_eq!(pending(&next), None);
        let state = runtime.0.state.as_ref().unwrap();
        assert_eq!(state.next_mission, MissionId::One);
        assert_eq!(state.credits, 120);
        assert!(runtime.0.last_completion.is_none());
        assert_eq!(active_mission.0.id, MissionId::One);
        assert!(status.0.contains("save file error"));
        assert!(battle.result().is_some_and(|result| result.victory));
    }
}
