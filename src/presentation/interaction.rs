use std::time::{SystemTime, UNIX_EPOCH};

use bevy::{picking::backend::HitData, prelude::*};

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
    EventPlayback, PresentationNeedsRebuild, PresentationRoot, RecentBattleLog, RestartRequest,
    RestartRoundPending, TokenCard,
    assets::{AssetLoadStatus, mission_assets_ready},
    campaign_ui::screen_transition_pending,
    layout::{BATTLE_STAGE_SIZE, grid_from_stage_point},
};

pub use super::battle_menu::MenuState;

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
    /// The unit currently inspected by the operator. During an activation,
    /// commands still require `BattleState::active_unit`; inspection never
    /// grants a second unit command authority.
    pub inspected_unit: Option<UnitId>,
    pub hovered_cell: Option<GridPos>,
    pub mode: InteractionMode,
    pub menu: MenuState,
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
    Cancel,
    Restart,
    ContinueVictory,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandButton(pub CommandAction);

/// Convert the UI picking backend's normalized node-local position into the
/// stage's fixed design pixels. Token hits must never use this helper because
/// their positions are local to the token card.
pub fn stage_point_from_hit(hit: &HitData) -> Option<Vec2> {
    let normalized = hit.position?.truncate();
    if !normalized.is_finite() {
        return None;
    }
    Some((normalized + Vec2::splat(0.5)) * BATTLE_STAGE_SIZE)
}

/// Resolve a direct stage hit through the authored diamond projection.
pub fn grid_from_hit(hit: &HitData) -> Option<GridPos> {
    grid_from_stage_point(stage_point_from_hit(hit)?)
}

fn set_inspected_unit(interaction: &mut InteractionState, unit: Option<UnitId>) {
    interaction.inspected_unit = unit;
}

pub fn route_cell_click(
    battle: &mut BattleState,
    interaction: &mut InteractionState,
    clicked: GridPos,
) -> Result<Vec<BattleEvent>, BattleError> {
    match interaction.mode {
        InteractionMode::Move => {
            let unit = require_active_unit(battle)?;
            let events = battle.move_unit(unit, clicked)?;
            interaction.mode = InteractionMode::Inspect;
            interaction.menu = MenuState::Root;
            interaction.hovered_cell = Some(clicked);
            interaction.preview = None;
            Ok(events)
        }
        InteractionMode::Attack(weapon) => {
            let unit = require_active_unit(battle)?;
            let events = battle.attack(unit, weapon, clicked)?;
            interaction.mode = InteractionMode::Inspect;
            interaction.menu = MenuState::Root;
            interaction.hovered_cell = Some(clicked);
            interaction.preview = None;
            Ok(events)
        }
        InteractionMode::AegisTarget => {
            let ally = battle
                .occupant_at(clicked)
                .ok_or(BattleError::NoUnitSelected)?;
            battle.use_aegis(ally)?;
            interaction.mode = InteractionMode::Inspect;
            interaction.menu = MenuState::Root;
            interaction.hovered_cell = Some(clicked);
            interaction.preview = None;
            Ok(Vec::new())
        }
        InteractionMode::Inspect => {
            if let Some(unit_id) = battle.occupant_at(clicked) {
                let unit = battle
                    .unit(unit_id)
                    .ok_or(BattleError::UnknownUnit(unit_id))?;
                if unit.faction == Faction::Player {
                    let should_begin = battle.phase() == BattlePhase::Player
                        && battle.active_unit().is_none()
                        && !unit.activation.finished
                        && !unit.is_knocked_out();
                    if should_begin {
                        battle.begin_activation(unit_id)?;
                        interaction.menu = MenuState::Root;
                    }
                }
                set_inspected_unit(interaction, Some(unit_id));
            }
            interaction.hovered_cell = Some(clicked);
            interaction.preview = None;
            Ok(Vec::new())
        }
    }
}

/// Inspect a token using its domain ID. This is intentionally separate from
/// stage picking because a token's `HitData.position` is token-local.
pub fn route_token_click(
    battle: &mut BattleState,
    interaction: &mut InteractionState,
    unit_id: UnitId,
) -> Result<Vec<BattleEvent>, BattleError> {
    let position = battle
        .unit(unit_id)
        .ok_or(BattleError::UnknownUnit(unit_id))?
        .position;
    // Token-local picking stops propagation, but the cell route remains the
    // single source of inspect/activation behavior for every board target.
    route_cell_click(battle, interaction, position)
}

pub fn update_hover_preview(
    battle: &BattleState,
    interaction: &mut InteractionState,
    cell: GridPos,
) {
    interaction.hovered_cell = Some(cell);
    interaction.preview = match (battle.active_unit(), interaction.mode) {
        (Some(attacker), InteractionMode::Attack(weapon)) => {
            battle.preview_attack(attacker, weapon, cell).ok()
        }
        _ => None,
    };
}

fn copy_preview_cells(interaction: &InteractionState, cells: &mut AttackPreviewCells) {
    cells.0.clear();
    if let Some(preview) = &interaction.preview {
        cells.0.extend(preview.footprint.iter().copied());
    }
}

fn clear_hover_preview(interaction: &mut InteractionState, preview_cells: &mut AttackPreviewCells) {
    interaction.hovered_cell = None;
    interaction.preview = None;
    preview_cells.0.clear();
}

fn stage_event_ready(status: &AssetLoadStatus, playback: &EventPlayback) -> bool {
    mission_assets_ready(status) && !playback.input_locked
}

#[allow(clippy::too_many_arguments)]
fn route_pointer_result(
    result: Result<Vec<BattleEvent>, BattleError>,
    interaction: &InteractionState,
    status: &mut StatusMessage,
    event_queue: &mut BattleEventQueue,
    playback: &mut EventPlayback,
    preview_cells: &mut AttackPreviewCells,
) {
    match result {
        Ok(events) => {
            playback.input_locked |= !events.is_empty();
            event_queue.0.extend(events);
            status.0.clear();
        }
        Err(error) => status.0 = error.to_string(),
    }
    copy_preview_cells(interaction, preview_cells);
}

/// The only stage click route. Board decorations are not pickable, so this
/// observer receives a direct stage-local hit and routes exactly one cell.
#[allow(clippy::too_many_arguments)]
pub fn on_battlefield_stage_click(
    _click: On<Pointer<Click>>,
    mut battle: ResMut<BattleRuntime>,
    mut interaction: ResMut<InteractionState>,
    mut status: ResMut<StatusMessage>,
    mut event_queue: ResMut<BattleEventQueue>,
    mut playback: ResMut<EventPlayback>,
    mut preview_cells: ResMut<AttackPreviewCells>,
    asset_status: Res<AssetLoadStatus>,
) {
    let click = _click;
    if !stage_event_ready(&asset_status, &playback) {
        return;
    }
    let Some(cell) = grid_from_hit(&click.event.hit) else {
        return;
    };
    route_pointer_result(
        route_cell_click(&mut battle.0, &mut interaction, cell),
        &interaction,
        &mut status,
        &mut event_queue,
        &mut playback,
        &mut preview_cells,
    );
}

pub fn on_battlefield_stage_move(
    event: On<Pointer<Move>>,
    battle: Res<BattleRuntime>,
    mut interaction: ResMut<InteractionState>,
    mut preview_cells: ResMut<AttackPreviewCells>,
    playback: Res<EventPlayback>,
    asset_status: Res<AssetLoadStatus>,
) {
    if !stage_event_ready(&asset_status, &playback) {
        return;
    }
    let Some(cell) = grid_from_hit(&event.event.hit) else {
        clear_hover_preview(&mut interaction, &mut preview_cells);
        return;
    };
    update_hover_preview(&battle.0, &mut interaction, cell);
    copy_preview_cells(&interaction, &mut preview_cells);
}

pub fn on_battlefield_stage_out(
    _event: On<Pointer<Out>>,
    mut interaction: ResMut<InteractionState>,
    mut preview_cells: ResMut<AttackPreviewCells>,
) {
    clear_hover_preview(&mut interaction, &mut preview_cells);
}

/// Token events stop at the token card. In targeting modes the current domain
/// position is routed once; in Inspect mode the token is simply inspected.
#[allow(clippy::too_many_arguments)]
pub fn on_battlefield_token_click(
    mut click: On<Pointer<Click>>,
    tokens: Query<&TokenCard>,
    mut battle: ResMut<BattleRuntime>,
    mut interaction: ResMut<InteractionState>,
    mut status: ResMut<StatusMessage>,
    mut event_queue: ResMut<BattleEventQueue>,
    mut playback: ResMut<EventPlayback>,
    mut preview_cells: ResMut<AttackPreviewCells>,
    asset_status: Res<AssetLoadStatus>,
) {
    click.propagate(false);
    if !stage_event_ready(&asset_status, &playback) {
        return;
    }
    let Ok(token) = tokens.get(click.entity) else {
        return;
    };
    route_pointer_result(
        route_token_click(&mut battle.0, &mut interaction, token.0),
        &interaction,
        &mut status,
        &mut event_queue,
        &mut playback,
        &mut preview_cells,
    );
}

pub fn on_battlefield_token_move(
    mut event: On<Pointer<Move>>,
    tokens: Query<&TokenCard>,
    battle: Res<BattleRuntime>,
    mut interaction: ResMut<InteractionState>,
    mut preview_cells: ResMut<AttackPreviewCells>,
    playback: Res<EventPlayback>,
    asset_status: Res<AssetLoadStatus>,
) {
    event.propagate(false);
    if !stage_event_ready(&asset_status, &playback) {
        return;
    }
    let Ok(token) = tokens.get(event.entity) else {
        return;
    };
    let Some(position) = battle.0.unit(token.0).map(|unit| unit.position) else {
        return;
    };
    update_hover_preview(&battle.0, &mut interaction, position);
    copy_preview_cells(&interaction, &mut preview_cells);
}

pub fn on_battlefield_token_out(
    mut event: On<Pointer<Out>>,
    tokens: Query<&TokenCard>,
    battle: Res<BattleRuntime>,
    mut interaction: ResMut<InteractionState>,
    mut preview_cells: ResMut<AttackPreviewCells>,
) {
    event.propagate(false);
    let Ok(token) = tokens.get(event.entity) else {
        return;
    };
    if interaction.hovered_cell == battle.0.unit(token.0).map(|unit| unit.position) {
        interaction.hovered_cell = None;
        interaction.preview = None;
        preview_cells.0.clear();
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
    mut playback: ResMut<EventPlayback>,
    mut restart_request: ResMut<RestartRequest>,
    mut campaign: ResMut<CampaignRuntime>,
    active_mission: Res<ActiveMission>,
    mut next_state: ResMut<NextState<GameScreen>>,
    asset_status: Res<AssetLoadStatus>,
) {
    if !stage_event_ready(&asset_status, &playback) {
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
            playback: &mut playback,
            restart_request: &mut restart_request,
            asset_status: &asset_status,
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
    mut playback: ResMut<EventPlayback>,
    mut restart_request: ResMut<RestartRequest>,
    mut campaign: ResMut<CampaignRuntime>,
    active_mission: Res<ActiveMission>,
    mut next_state: ResMut<NextState<GameScreen>>,
    asset_status: Res<AssetLoadStatus>,
) {
    if !stage_event_ready(&asset_status, &playback) {
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
    } else if keyboard.just_pressed(KeyCode::Escape) {
        Some(CommandAction::Cancel)
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
            playback: &mut playback,
            restart_request: &mut restart_request,
            asset_status: &asset_status,
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
    playback: &'a mut EventPlayback,
    restart_request: &'a mut RestartRequest,
    asset_status: &'a AssetLoadStatus,
}

pub fn execute_command(
    battle: &mut BattleState,
    interaction: &mut InteractionState,
    action: CommandAction,
) -> Result<Vec<BattleEvent>, BattleError> {
    match action {
        CommandAction::Move => {
            let unit_id = require_active_unit(battle)?;
            let unit = battle
                .unit(unit_id)
                .ok_or(BattleError::UnknownUnit(unit_id))?;
            if unit.activation.moved {
                return Err(BattleError::MoveAlreadySpent(unit_id));
            }
            interaction.mode = InteractionMode::Move;
            interaction.menu = MenuState::Hidden;
            interaction.preview = None;
            Ok(Vec::new())
        }
        CommandAction::WeaponSlot(slot) => {
            let unit_id = require_active_unit(battle)?;
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
            interaction.menu = MenuState::Hidden;
            interaction.preview = interaction
                .hovered_cell
                .and_then(|cell| battle.preview_attack(unit_id, weapon_id, cell).ok());
            Ok(Vec::new())
        }
        CommandAction::PilotSkill => {
            let unit_id = require_active_unit(battle)?;
            let unit = battle
                .unit(unit_id)
                .ok_or(BattleError::UnknownUnit(unit_id))?;
            match unit.archetype {
                UnitArchetype::Vanguard => {
                    interaction.mode = InteractionMode::AegisTarget;
                    interaction.menu = MenuState::Hidden;
                    interaction.preview = None;
                }
                UnitArchetype::Gunner => {
                    battle.use_focus()?;
                    interaction.menu = MenuState::Root;
                }
                UnitArchetype::Interceptor => {
                    battle.use_overdrive()?;
                    interaction.menu = MenuState::Root;
                }
                UnitArchetype::Rifleman
                | UnitArchetype::Striker
                | UnitArchetype::Artillery
                | UnitArchetype::Flanker
                | UnitArchetype::Bulwark
                | UnitArchetype::Controller
                | UnitArchetype::Dreadnought
                | UnitArchetype::Regent => {
                    return Err(BattleError::PilotSkillWrongUnit(unit_id));
                }
            }
            Ok(Vec::new())
        }
        CommandAction::Reaction(reaction) => {
            let unit = require_active_unit(battle)?;
            battle.choose_reaction(unit, reaction)?;
            interaction.menu = MenuState::Root;
            Ok(Vec::new())
        }
        CommandAction::FinishUnit => {
            let unit = require_active_unit(battle)?;
            battle.finish_activation(unit)?;
            interaction.mode = InteractionMode::Inspect;
            interaction.preview = None;
            if let Some(next) = next_ready_unit(battle) {
                battle.begin_activation(next)?;
                set_inspected_unit(interaction, Some(next));
                interaction.menu = MenuState::Root;
            } else {
                set_inspected_unit(interaction, None);
                interaction.menu = MenuState::Hidden;
            }
            Ok(Vec::new())
        }
        CommandAction::ResolveAttacks => {
            let events = battle.resolve_enemy_phase()?;
            set_inspected_unit(interaction, None);
            interaction.hovered_cell = None;
            interaction.mode = InteractionMode::Inspect;
            interaction.menu = MenuState::Hidden;
            interaction.preview = None;
            Ok(events)
        }
        CommandAction::Cancel => {
            interaction.mode = InteractionMode::Inspect;
            interaction.menu = if battle.active_unit().is_some() {
                MenuState::Root
            } else {
                MenuState::Hidden
            };
            interaction.preview = None;
            Ok(Vec::new())
        }
        CommandAction::Restart => {
            let idle_player =
                battle.phase() == BattlePhase::Player && battle.active_unit().is_none();
            let defeat = battle.result().is_some_and(|result| !result.victory);
            if !idle_player && !defeat {
                return Err(BattleError::WrongPhase {
                    expected: BattlePhase::Defeat,
                    actual: battle.phase(),
                });
            }
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

fn run_command(action: CommandAction, mut context: CommandContext<'_>) {
    if action == CommandAction::ContinueVictory {
        run_continue_victory(&mut context);
        return;
    }
    if action == CommandAction::Restart
        && !restart_allowed(
            context.battle,
            context.asset_status,
            context.playback,
            context.next_state,
            context.restart_request.0.is_some(),
        )
    {
        context.status.0 = "Restart is unavailable right now.".to_owned();
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
        }
        Err(error) => context.status.0 = error.to_string(),
    }
    copy_preview_cells(context.interaction, context.preview_cells);
}

fn run_continue_victory(context: &mut CommandContext<'_>) {
    if screen_transition_pending(context.next_state) {
        return;
    }
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

fn require_active_unit(battle: &BattleState) -> Result<UnitId, BattleError> {
    if battle.phase() != BattlePhase::Player {
        return Err(BattleError::WrongPhase {
            expected: BattlePhase::Player,
            actual: battle.phase(),
        });
    }
    battle.active_unit().ok_or(BattleError::NoUnitSelected)
}

/// Return the next living, unfinished player in the authored activation order.
/// The fixed pilot order keeps hand-off stable even when map insertion order
/// changes.
pub fn next_ready_unit(battle: &BattleState) -> Option<UnitId> {
    const ORDER: [UnitArchetype; 3] = [
        UnitArchetype::Vanguard,
        UnitArchetype::Gunner,
        UnitArchetype::Interceptor,
    ];
    ORDER
        .into_iter()
        .find_map(|archetype| {
            battle.units().find(|unit| {
                unit.faction == Faction::Player
                    && unit.archetype == archetype
                    && !unit.is_knocked_out()
                    && !unit.activation.finished
            })
        })
        .map(|unit| unit.id)
}

/// Restart is available only while the authored mission is idle, after assets
/// finish loading, with no playback or queued transition/request in flight.
pub fn restart_allowed(
    battle: &BattleState,
    asset_status: &AssetLoadStatus,
    playback: &EventPlayback,
    next_state: &NextState<GameScreen>,
    restart_pending: bool,
) -> bool {
    let idle_player = battle.phase() == BattlePhase::Player && battle.active_unit().is_none();
    let defeat = battle.result().is_some_and(|result| !result.victory);
    (idle_player || defeat)
        && mission_assets_ready(asset_status)
        && !playback.input_locked
        && playback.current.is_none()
        && !screen_transition_pending(next_state)
        && !restart_pending
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
        CommandAction::Cancel => "Command cancelled.",
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

    let root = world
        .spawn((
            Name::new("Mission Presentation"),
            PresentationRoot,
            PresentationNeedsRebuild,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            Pickable::IGNORE,
            Visibility::Visible,
        ))
        .id();
    if let Some(canvas) = world
        .query_filtered::<Entity, With<super::CanvasRoot>>()
        .iter(world)
        .next()
    {
        world.entity_mut(root).insert(ChildOf(canvas));
    }
}

/// Clear the interaction/playback/preview/selection state shared by restart and battle entry.
pub(crate) fn reset_transient_battle_state(world: &mut World) {
    *world.resource_mut::<InteractionState>() = InteractionState::default();
    *world.resource_mut::<StatusMessage>() = StatusMessage::default();
    world.resource_mut::<BattleEventQueue>().0.clear();
    *world.resource_mut::<EventPlayback>() = EventPlayback::default();
    world.resource_mut::<AttackPreviewCells>().0.clear();
    if let Some(mut log) = world.get_resource_mut::<RecentBattleLog>() {
        log.0.clear();
    }
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::campaign::model::CampaignState;
    use crate::campaign::save::SaveFile;
    use crate::campaign::session::CampaignSession;
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
            battle.apply_direct_damage(
                enemy,
                99,
                crate::domain::combat::DamageSource::PlayerWeapon(ids::PILE_LANCE),
            );
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
        let mut playback = EventPlayback::default();
        let mut restart_request = RestartRequest::default();
        let asset_status = AssetLoadStatus::Ready;
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
                playback: &mut playback,
                restart_request: &mut restart_request,
                asset_status: &asset_status,
            },
        );
    }

    #[test]
    fn restart_is_rejected_on_victory() {
        let mut battle = terminal_victory_battle();
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
        assert!(runtime.0.last_completion.is_some());
        assert_eq!(pending(&next), Some(GameScreen::Aftermath));
        assert_eq!(status.0, "Campaign progress saved.");
    }

    #[test]
    fn stage_hit_uses_normalized_stage_coordinates() {
        let local = crate::presentation::layout::iso_center(GridPos::new(4, 4))
            - crate::presentation::layout::battle_stage_rect().min;
        let normalized = local / BATTLE_STAGE_SIZE - Vec2::splat(0.5);
        let hit = HitData::new(Entity::PLACEHOLDER, 0.0, Some(normalized.extend(0.0)), None);
        let resolved = stage_point_from_hit(&hit).unwrap();
        assert!(resolved.distance(local) < 0.001);
        assert_eq!(grid_from_hit(&hit), Some(GridPos::new(4, 4)));
    }
}
