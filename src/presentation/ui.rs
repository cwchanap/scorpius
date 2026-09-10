use bevy::{
    a11y::AccessibilityNode,
    ecs::system::SystemParam,
    prelude::*,
    text::{LetterSpacing, LineHeight},
};

use crate::app::GameScreen;
use crate::domain::{
    battle::BattleState,
    board::GridPos,
    model::{
        BattleEvent, BattlePhase, Faction, MissionResult, OptionalObjective, PrimaryObjective,
        Reaction, UnitArchetype, UnitId, WeaponId, WeaponShape,
    },
};
use crate::mission::MissionDefinition;

use super::{
    ActiveMission, BattleRuntime, CampaignRuntime, CanvasRoot, EventPlayback, RecentBattleLog,
    RestartRequest,
    assets::{AssetLoadStatus, UiAssets},
    battle_menu::{
        WeaponMeter, WeaponMeterKind, WeaponRow, WeaponTag, WeaponTagIcon, WeaponText,
        WeaponTextKind, spawn_battle_menu,
    },
    interaction::{
        CommandAction, CommandButton, InteractionMode, InteractionState, StatusMessage,
        on_command_button_click, restart_allowed,
    },
    layout::spawn_canvas_root,
    theme,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThreatSnapshot {
    pub attacker_id: UnitId,
    pub attacker: &'static str,
    pub attacker_archetype: UnitArchetype,
    pub weapon_id: WeaponId,
    pub weapon: &'static str,
    pub shape: WeaponShape,
    pub cells: Vec<crate::domain::board::GridPos>,
    pub intended_occupant_id: Option<UnitId>,
    pub intended_occupant: Option<&'static str>,
    pub normal_damage: i16,
    pub hit_chance: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WeaponSnapshot {
    pub id: WeaponId,
    pub name: &'static str,
    pub shape: WeaponShape,
    pub min_range: u8,
    pub max_range: u8,
    pub base_damage: i16,
    pub hit_modifier: i16,
    pub crit_chance: u8,
    pub en_cost: i16,
    pub push: bool,
    pub counter_weapon: bool,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InspectorSnapshot {
    pub unit_id: Option<UnitId>,
    pub name: Option<&'static str>,
    pub archetype: Option<UnitArchetype>,
    pub faction: Option<Faction>,
    pub hp: Option<i16>,
    pub max_hp: Option<i16>,
    pub en: Option<i16>,
    pub max_en: Option<i16>,
    pub armor: Option<i16>,
    pub movement: Option<u8>,
    pub evasion: Option<i16>,
    pub moved: bool,
    pub acted: bool,
    pub finished: bool,
    pub reaction: Option<Reaction>,
}

impl InspectorSnapshot {
    fn from_unit(unit: &crate::domain::model::UnitState) -> Self {
        Self {
            unit_id: Some(unit.id),
            name: Some(unit.name),
            archetype: Some(unit.archetype),
            faction: Some(unit.faction),
            hp: Some(unit.hp),
            max_hp: Some(unit.stats.max_hp),
            en: Some(unit.en),
            max_en: Some(unit.stats.max_en),
            armor: Some(unit.stats.armor),
            movement: Some(unit.stats.movement),
            evasion: Some(unit.stats.evasion),
            moved: unit.activation.moved,
            acted: unit.activation.acted,
            finished: unit.activation.finished,
            reaction: unit.reaction,
        }
    }

    pub const fn is_empty(self) -> bool {
        self.unit_id.is_none()
    }
}

/// Live tracker derived from the mission's primary objective. It carries the
/// numbers and map cells needed by the right rail and objective header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObjectiveTrackSnapshot {
    EliminateAll {
        remaining: usize,
        total: usize,
    },
    Protect {
        name: &'static str,
        hp: i16,
        max_hp: i16,
        round: u16,
        position: GridPos,
    },
    Intercept {
        name: &'static str,
        distance: u8,
        deadline_round: u16,
        position: GridPos,
        escape: GridPos,
    },
    Target {
        name: &'static str,
        hp: i16,
        max_hp: i16,
        position: GridPos,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionalProgressSnapshot {
    Turnabout {
        complete: bool,
    },
    ProtectTargetAtHalfHp {
        target: UnitId,
        name: &'static str,
        hp: i16,
        max_hp: i16,
        complete: bool,
    },
    VictoryByRound {
        round: u16,
        current_round: u16,
        complete: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HudSnapshot {
    pub round_phase: String,
    pub primary: String,
    pub optional: String,
    pub objective_track: Option<ObjectiveTrackSnapshot>,
    pub optional_progress: OptionalProgressSnapshot,
    pub selected_name: Option<&'static str>,
    pub inspector: InspectorSnapshot,
    pub threats: Vec<ThreatSnapshot>,
    pub ally_count: usize,
    pub enemy_count: usize,
    pub awaiting_count: usize,
    pub weapon_names: [Option<&'static str>; 3],
    pub weapon_specs: [Option<WeaponSnapshot>; 3],
    pub weapon_enabled: [bool; 3],
    pub can_move: bool,
    pub can_pilot: bool,
    pub can_choose_reaction: bool,
    pub can_finish: bool,
    pub can_resolve: bool,
    pub is_terminal: bool,
    pub is_victory: bool,
    pub pilot_label: &'static str,
    pub pilot_aegis: &'static str,
    pub pilot_focus: &'static str,
    pub pilot_overdrive: &'static str,
}

impl HudSnapshot {
    pub const fn optional_progress_complete(&self) -> bool {
        match self.optional_progress {
            OptionalProgressSnapshot::Turnabout { complete }
            | OptionalProgressSnapshot::ProtectTargetAtHalfHp { complete, .. }
            | OptionalProgressSnapshot::VictoryByRound { complete, .. } => complete,
        }
    }

    pub fn from_battle(
        battle: &BattleState,
        selected: Option<UnitId>,
        definition: &MissionDefinition,
    ) -> Self {
        let remaining = battle
            .units()
            .filter(|unit| unit.faction == Faction::Enemy && !unit.is_knocked_out())
            .count();
        let enemy_total = battle
            .units()
            .filter(|unit| unit.faction == Faction::Enemy)
            .count();
        let enemy_count = remaining;
        let ally_count = battle
            .units()
            .filter(|unit| unit.faction == Faction::Player && !unit.is_knocked_out())
            .count();
        let awaiting_count = battle
            .units()
            .filter(|unit| {
                unit.faction == Faction::Player
                    && !unit.is_knocked_out()
                    && !unit.activation.finished
            })
            .count();
        let selected_unit = selected.and_then(|unit| battle.unit(unit));
        let active = battle
            .active_unit()
            .and_then(|unit| battle.unit(unit))
            .filter(|unit| battle.phase() == BattlePhase::Player && !unit.is_knocked_out());
        let mut weapon_names = [None; 3];
        let mut weapon_enabled = [false; 3];
        let mut weapon_specs = std::array::from_fn(|_| None);
        if let Some(unit) = active {
            for (slot, weapon_id) in unit.weapons.iter().take(3).enumerate() {
                if let Some(weapon) = battle.weapon(*weapon_id) {
                    weapon_names[slot] = Some(weapon.name);
                    let enabled = !unit.activation.acted && unit.en >= weapon.en_cost;
                    weapon_enabled[slot] = enabled;
                    weapon_specs[slot] = Some(WeaponSnapshot {
                        id: weapon.id,
                        name: weapon.name,
                        shape: weapon.shape,
                        min_range: weapon.min_range,
                        max_range: weapon.max_range,
                        base_damage: weapon.base_damage,
                        hit_modifier: weapon.hit_modifier,
                        crit_chance: weapon.crit_chance,
                        en_cost: weapon.en_cost,
                        push: weapon.push,
                        counter_weapon: weapon.counter_weapon,
                        enabled,
                    });
                }
            }
        }
        let inspector =
            selected_unit.map_or_else(InspectorSnapshot::default, InspectorSnapshot::from_unit);

        let pilot = battle.pilot_skills();
        let pilot_status = |used: bool, active_now: bool| {
            if active_now {
                "ACTIVE"
            } else if used {
                "USED"
            } else {
                "READY"
            }
        };

        let objective_track = match battle.rules().primary {
            PrimaryObjective::ProtectThroughRound { target, round } => {
                battle
                    .unit(target)
                    .map(|unit| ObjectiveTrackSnapshot::Protect {
                        name: unit.name,
                        hp: unit.hp,
                        max_hp: unit.stats.max_hp,
                        round,
                        position: unit.position,
                    })
            }
            PrimaryObjective::InterceptBeforeEscape {
                target,
                escape,
                deadline_round,
            } => battle
                .unit(target)
                .map(|unit| ObjectiveTrackSnapshot::Intercept {
                    name: unit.name,
                    distance: unit.position.manhattan(escape),
                    deadline_round,
                    position: unit.position,
                    escape,
                }),
            PrimaryObjective::EliminateTarget { target } => {
                battle
                    .unit(target)
                    .map(|unit| ObjectiveTrackSnapshot::Target {
                        name: unit.name,
                        hp: unit.hp,
                        max_hp: unit.stats.max_hp,
                        position: unit.position,
                    })
            }
            PrimaryObjective::EliminateAllEnemies => Some(ObjectiveTrackSnapshot::EliminateAll {
                remaining,
                total: enemy_total,
            }),
        };
        let round_cap = match battle.rules().primary {
            PrimaryObjective::ProtectThroughRound { round, .. } => Some(round),
            PrimaryObjective::InterceptBeforeEscape { deadline_round, .. } => Some(deadline_round),
            PrimaryObjective::EliminateAllEnemies | PrimaryObjective::EliminateTarget { .. } => {
                None
            }
        };

        Self {
            round_phase: match round_cap {
                Some(cap) => format!(
                    "Round {}/{} · {}",
                    battle.round(),
                    cap,
                    phase_label(battle.phase())
                ),
                None => format!("Round {} · {}", battle.round(), phase_label(battle.phase())),
            },
            primary: match battle.rules().primary {
                PrimaryObjective::EliminateAllEnemies => {
                    format!("{} · {remaining} remaining", definition.primary_objective)
                }
                _ => definition.primary_objective.to_owned(),
            },
            optional: format!(
                "{} · {}",
                definition.optional_objective,
                if battle.objectives().optional_complete {
                    "Complete"
                } else {
                    "Not yet"
                }
            ),
            selected_name: selected_unit.map(|unit| unit.name),
            inspector,
            optional_progress: optional_progress_snapshot(battle),
            objective_track,
            threats: battle
                .intents()
                .iter()
                .filter_map(|intent| {
                    let attacker = battle.unit(intent.attacker)?;
                    let weapon = battle.weapon(intent.profile.weapon)?;
                    let intended_occupant = intent
                        .intended_occupant
                        .and_then(|target| battle.unit(target))
                        .map(|unit| unit.name);
                    let normal_damage = intent
                        .intended_preview
                        .as_ref()
                        .map_or(intent.profile.base_damage, |preview| preview.normal_damage);
                    let hit_chance = intent.intended_preview.as_ref().map_or_else(
                        || {
                            (intent.profile.accuracy + intent.profile.hit_modifier).clamp(5, 95)
                                as u8
                        },
                        |preview| preview.hit_chance,
                    );
                    Some(ThreatSnapshot {
                        attacker_id: intent.attacker,
                        attacker: attacker.name,
                        attacker_archetype: attacker.archetype,
                        weapon_id: intent.profile.weapon,
                        weapon: weapon.name,
                        shape: weapon.shape,
                        cells: intent.footprint.to_vec(),
                        intended_occupant_id: intent.intended_occupant,
                        intended_occupant,
                        normal_damage,
                        hit_chance,
                    })
                })
                .collect(),
            can_move: active.is_some_and(|unit| !unit.activation.moved),
            ally_count,
            enemy_count,
            awaiting_count,
            can_pilot: active.is_some_and(|unit| match unit.archetype {
                UnitArchetype::Vanguard => !pilot.aegis_used,
                UnitArchetype::Gunner => !pilot.focus_used,
                UnitArchetype::Interceptor => !pilot.overdrive_used && !unit.activation.moved,
                UnitArchetype::Rifleman
                | UnitArchetype::Striker
                | UnitArchetype::Artillery
                | UnitArchetype::Flanker
                | UnitArchetype::Bulwark
                | UnitArchetype::Controller
                | UnitArchetype::Dreadnought
                | UnitArchetype::Regent => false,
            }),
            can_choose_reaction: active.is_some_and(|unit| !unit.activation.finished),
            can_finish: active.is_some_and(|unit| unit.reaction.is_some()),
            can_resolve: battle.ready_to_resolve(),
            is_terminal: battle.result().is_some(),
            is_victory: battle.result().is_some_and(|result| result.victory),
            pilot_label: active.map_or("[P] PILOT", |unit| match unit.archetype {
                UnitArchetype::Vanguard => "[P] AEGIS",
                UnitArchetype::Gunner => "[P] FOCUS",
                UnitArchetype::Interceptor => "[P] OVERDRIVE",
                UnitArchetype::Rifleman
                | UnitArchetype::Striker
                | UnitArchetype::Artillery
                | UnitArchetype::Flanker
                | UnitArchetype::Bulwark
                | UnitArchetype::Controller
                | UnitArchetype::Dreadnought
                | UnitArchetype::Regent => "[P] PILOT",
            }),
            pilot_aegis: pilot_status(pilot.aegis_used, pilot.aegis_target.is_some()),
            pilot_focus: pilot_status(pilot.focus_used, pilot.focus_pending),
            pilot_overdrive: pilot_status(pilot.overdrive_used, pilot.overdrive_active),
            weapon_names,
            weapon_enabled,
            weapon_specs,
        }
    }
}

fn optional_progress_snapshot(battle: &BattleState) -> OptionalProgressSnapshot {
    let complete = battle.objectives().optional_complete;
    match battle.rules().optional {
        OptionalObjective::Turnabout => OptionalProgressSnapshot::Turnabout { complete },
        OptionalObjective::ProtectTargetAtHalfHp { target } => battle.unit(target).map_or(
            OptionalProgressSnapshot::ProtectTargetAtHalfHp {
                target,
                name: "UNKNOWN",
                hp: 0,
                max_hp: 0,
                complete,
            },
            |unit| OptionalProgressSnapshot::ProtectTargetAtHalfHp {
                target,
                name: unit.name,
                hp: unit.hp,
                max_hp: unit.stats.max_hp,
                complete,
            },
        ),
        OptionalObjective::VictoryByRound { round } => OptionalProgressSnapshot::VictoryByRound {
            round,
            current_round: battle.round(),
            complete,
        },
    }
}

const fn phase_label(phase: BattlePhase) -> &'static str {
    match phase {
        BattlePhase::EnemyPlanning => "Enemy Planning",
        BattlePhase::Player => "Player Phase",
        BattlePhase::EnemyResolution => "Enemy Resolution",
        BattlePhase::Victory => "Victory",
        BattlePhase::Defeat => "Defeat",
    }
}

#[derive(Component)]
pub struct ObjectiveText;

#[derive(Component)]
pub struct ThreatList;

#[derive(Component)]
pub struct UnitSummary;

#[derive(Component)]
pub struct PreviewText;

#[derive(Component)]
pub struct PreviewPanel;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreviewValue(pub PreviewValueKind);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreviewValueKind {
    Damage,
    Critical,
    Hit,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreviewMeter(pub usize);

#[derive(Component)]
pub struct StatusText;

#[derive(Component)]
pub struct PlaybackText;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogEntryText(pub usize);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogEntryRow(pub usize);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogEntryDot(pub usize);

#[derive(Component)]
pub struct ResultOverlay;

#[derive(Component)]
pub struct AssetStatusText;

#[derive(Component)]
pub struct HudRoot;

#[derive(Component)]
pub struct BattleHeader;

#[derive(Component)]
pub struct BattleSidebar;

#[derive(Component)]
pub struct InspectorPanel;

#[derive(Component)]
pub struct BattleRightbar;

#[derive(Component)]
pub struct InspectorPortrait;

#[derive(Component)]
pub struct InspectorTop;

#[derive(Component)]
pub struct InspectorDetails;

#[derive(Component)]
pub struct InspectorEnergyRow;

#[derive(Component)]
pub struct InspectorStats;

#[derive(Component)]
pub struct InspectorEmpty;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct InspectorText(pub InspectorTextKind);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectorTextKind {
    Name,
    Hp,
    En,
    Armor,
    Movement,
    Evasion,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct InspectorIcon(pub InspectorIconKind);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectorIconKind {
    Glyph,
    Reaction,
    Activation,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct InspectorMeter(pub InspectorMeterKind);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectorMeterKind {
    Hp,
    Energy(usize),
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThreatCard(pub usize);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThreatText {
    pub card: usize,
    pub kind: ThreatTextKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThreatTextKind {
    Attacker,
    Weapon,
    Target,
    Damage,
    Hit,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThreatIcon {
    pub card: usize,
    pub target: bool,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThreatMeter(pub ThreatMeterKind);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThreatMeterKind {
    Hit(usize),
    Shape { card: usize, index: usize },
}

#[derive(Component)]
struct HeaderRestart;

#[derive(Component)]
struct HeaderPrimaryPip(usize);

#[derive(Component)]
struct HeaderBonusDot;

#[derive(Component)]
pub struct ResultIcon;

#[derive(Component)]
pub struct ResultCard;

#[derive(Component)]
pub struct ResultRing;

#[derive(Component)]
pub struct ResultHeadline;

#[derive(Component)]
pub struct ResultDetail;

#[derive(Component)]
pub struct ResultStatus;

#[derive(Component, Clone, Copy)]
pub struct ResultMetricIcon {
    pub bonus: bool,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum HeaderValue {
    Round,
    Allies,
    Enemies,
    Awaiting,
    Credits,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HudTextRole {
    Objective,
    ThreatCount,
    Preview,
    Status,
    Result,
    ResultPrimary,
}

pub fn setup_mission_ui(
    mut commands: Commands,
    canvas_roots: Query<Entity, With<CanvasRoot>>,
    ui_assets: Res<UiAssets>,
) {
    let canvas = canvas_roots
        .iter()
        .next()
        .unwrap_or_else(|| spawn_canvas_root(&mut commands));
    let root = commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(canvas),
        ))
        .id();

    let header = commands
        .spawn((
            BattleHeader,
            Node {
                position_type: PositionType::Absolute,
                left: px(22),
                right: px(22),
                top: px(22),
                height: px(78),
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(20),
                padding: UiRect::horizontal(px(22)),
                ..default()
            },
            BackgroundColor(theme::PANEL),
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    let round_phase = commands
        .spawn((
            Node {
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(12),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(header),
        ))
        .id();
    commands.spawn((
        Text::new("01"),
        theme::ibm_plex_mono(&ui_assets.fonts, 34.0, FontWeight(600)),
        TextColor(theme::ACCENT),
        HeaderValue::Round,
        Pickable::IGNORE,
        ChildOf(round_phase),
    ));
    commands.spawn((
        Node {
            width: px(10),
            height: px(10),
            border_radius: BorderRadius::all(percent(50)),
            ..default()
        },
        BackgroundColor(theme::ACCENT),
        Pickable::IGNORE,
        ChildOf(round_phase),
    ));
    commands.spawn((
        Node {
            width: px(1),
            height: px(38),
            margin: UiRect::horizontal(px(2)),
            ..default()
        },
        BackgroundColor(theme::BORDER),
        Pickable::IGNORE,
        ChildOf(header),
    ));
    let metrics = commands
        .spawn((
            Node {
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(20),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(header),
        ))
        .id();
    spawn_header_metric(
        &mut commands,
        metrics,
        &ui_assets,
        theme::BATTLE_ALLY_RECT,
        theme::TEXT,
        HeaderValue::Allies,
        "0",
    );
    spawn_header_metric(
        &mut commands,
        metrics,
        &ui_assets,
        theme::BATTLE_ENEMY_RECT,
        theme::ENEMY,
        HeaderValue::Enemies,
        "0",
    );
    spawn_header_metric(
        &mut commands,
        metrics,
        &ui_assets,
        theme::BATTLE_AWAITING_RECT,
        theme::GOLD,
        HeaderValue::Awaiting,
        "0",
    );
    commands.spawn((
        Node {
            width: px(1),
            height: px(38),
            margin: UiRect::horizontal(px(2)),
            ..default()
        },
        BackgroundColor(theme::BORDER),
        Pickable::IGNORE,
        ChildOf(header),
    ));
    commands.spawn((
        theme::icon_node(
            ui_assets.icons.clone(),
            theme::BATTLE_CYCLE_RECT,
            Color::WHITE,
        ),
        Node {
            width: px(24),
            height: px(24),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(header),
    ));
    let primary = commands
        .spawn((
            Node {
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(10),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(header),
        ))
        .id();
    commands.spawn((
        theme::icon_node(
            ui_assets.icons.clone(),
            theme::BATTLE_PRIMARY_RECT,
            Color::WHITE,
        ),
        Node {
            width: px(24),
            height: px(24),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(primary),
    ));
    let pips = commands
        .spawn((
            Node {
                display: Display::Flex,
                column_gap: px(6),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(primary),
        ))
        .id();
    for index in 0..4 {
        commands.spawn((
            Node {
                width: px(18),
                height: px(8),
                ..default()
            },
            BackgroundColor(theme::BORDER),
            HeaderPrimaryPip(index),
            Pickable::IGNORE,
            ChildOf(pips),
        ));
    }
    commands.spawn((
        Node {
            width: px(12),
            height: px(12),
            ..default()
        },
        BackgroundColor(theme::GOLD),
        HeaderBonusDot,
        Pickable::IGNORE,
        ChildOf(primary),
    ));
    let header_actions = commands
        .spawn((
            Node {
                margin: UiRect::left(Val::Auto),
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(18),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(header),
        ))
        .id();
    spawn_header_metric(
        &mut commands,
        header_actions,
        &ui_assets,
        theme::BATTLE_CREDITS_RECT,
        theme::GOLD,
        HeaderValue::Credits,
        "—",
    );
    commands
        .spawn((
            Button,
            CommandButton(CommandAction::Restart),
            HeaderRestart,
            Node {
                width: px(46),
                height: px(46),
                display: Display::Flex,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgb_u8(14, 26, 38)),
            Pickable::default(),
            ChildOf(header_actions),
        ))
        .observe(on_command_button_click)
        .with_children(|parent| {
            parent.spawn((
                theme::icon_node(
                    ui_assets.icons.clone(),
                    theme::BATTLE_RESTART_RECT,
                    Color::WHITE,
                ),
                Node {
                    width: px(22),
                    height: px(22),
                    ..default()
                },
                Pickable::IGNORE,
            ));
        });

    let sidebar = commands
        .spawn((
            BattleSidebar,
            Node {
                position_type: PositionType::Absolute,
                left: px(22),
                top: px(114),
                width: px(352),
                height: px(944),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(12),
                min_height: px(0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    let inspector = commands
        .spawn((
            InspectorPanel,
            UnitSummary,
            Node {
                width: percent(100),
                height: Val::Auto,
                flex_shrink: 0.0,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                padding: UiRect::ZERO,
                ..default()
            },
            BackgroundColor(theme::PANEL),
            Outline::new(px(2), px(-2), Color::NONE),
            Visibility::Visible,
            Pickable::IGNORE,
            ChildOf(sidebar),
        ))
        .id();
    let inspector_top = commands
        .spawn((
            InspectorTop,
            Node {
                width: percent(100),
                height: px(92),
                flex_shrink: 0.0,
                display: Display::Flex,
                column_gap: px(14),
                ..default()
            },
            Visibility::Hidden,
            Pickable::IGNORE,
            ChildOf(inspector),
        ))
        .id();
    commands.spawn((
        Node {
            width: px(92),
            height: px(92),
            flex_shrink: 0.0,
            ..default()
        },
        ImageNode::new(ui_assets.icons.clone())
            .with_rect(theme::UNIT_GLYPH_HEX_RECT)
            .with_color(theme::MUTED),
        InspectorPortrait,
        Visibility::Visible,
        Pickable::IGNORE,
        ChildOf(inspector_top),
    ));
    let inspector_details = commands
        .spawn((
            InspectorDetails,
            Node {
                width: percent(100),
                min_width: px(0),
                flex_grow: 1.0,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(7),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(inspector_top),
        ))
        .id();
    let name_row = commands
        .spawn((
            Node {
                width: percent(100),
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(9),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(inspector_details),
        ))
        .id();
    commands.spawn((
        theme::icon_node(
            ui_assets.icons.clone(),
            theme::UNIT_GLYPH_HEX_RECT,
            theme::ACCENT,
        ),
        Node {
            width: px(18),
            height: px(18),
            flex_shrink: 0.0,
            ..default()
        },
        InspectorIcon(InspectorIconKind::Glyph),
        Pickable::IGNORE,
        ChildOf(name_row),
    ));
    commands.spawn((
        Text::new("—"),
        theme::chakra_petch(&ui_assets.fonts, 21.0, FontWeight(600)),
        LetterSpacing::Px(1.68),
        TextColor(theme::TEXT),
        InspectorText(InspectorTextKind::Name),
        Pickable::IGNORE,
        ChildOf(name_row),
    ));
    let hp_row = commands
        .spawn((
            Node {
                width: percent(100),
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(9),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(inspector_details),
        ))
        .id();
    commands.spawn((
        theme::icon_node(
            ui_assets.icons.clone(),
            theme::INSPECTOR_HP_RECT,
            Color::WHITE,
        ),
        Node {
            width: px(16),
            height: px(16),
            flex_shrink: 0.0,
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(hp_row),
    ));
    let hp_track = commands
        .spawn((
            Node {
                position_type: PositionType::Relative,
                width: percent(100),
                height: px(14),
                flex_grow: 1.0,
                ..default()
            },
            BackgroundColor(Color::srgb_u8(13, 26, 36)),
            BorderColor::all(Color::srgb_u8(27, 48, 64)),
            Pickable::IGNORE,
            ChildOf(hp_row),
        ))
        .id();
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            bottom: px(0),
            width: percent(0),
            ..default()
        },
        BackgroundColor(theme::MINT),
        InspectorMeter(InspectorMeterKind::Hp),
        Pickable::IGNORE,
        ChildOf(hp_track),
    ));
    commands.spawn((
        Text::new("—"),
        theme::ibm_plex_mono(&ui_assets.fonts, 16.0, FontWeight(400)),
        TextColor(theme::INSPECTOR_HP_TEXT),
        TextLayout::justify(Justify::Right),
        Node {
            min_width: px(58),
            ..default()
        },
        InspectorText(InspectorTextKind::Hp),
        Pickable::IGNORE,
        ChildOf(hp_row),
    ));
    let energy_row = commands
        .spawn((
            Node {
                width: percent(100),
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(9),
                ..default()
            },
            InspectorEnergyRow,
            Pickable::IGNORE,
            ChildOf(inspector_details),
        ))
        .id();
    commands.spawn((
        theme::icon_node(
            ui_assets.icons.clone(),
            theme::INSPECTOR_EN_RECT,
            Color::WHITE,
        ),
        Node {
            width: px(16),
            height: px(16),
            flex_shrink: 0.0,
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(energy_row),
    ));
    let energy_pips = commands
        .spawn((
            Node {
                display: Display::Flex,
                column_gap: px(3),
                flex_grow: 1.0,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(energy_row),
        ))
        .id();
    for index in 0..9 {
        commands.spawn((
            Node {
                width: px(0),
                height: px(10),
                flex_grow: 1.0,
                ..default()
            },
            BackgroundColor(theme::BORDER),
            InspectorMeter(InspectorMeterKind::Energy(index)),
            Pickable::IGNORE,
            ChildOf(energy_pips),
        ));
    }
    commands.spawn((
        Text::new("—"),
        theme::ibm_plex_mono(&ui_assets.fonts, 16.0, FontWeight(400)),
        TextColor(theme::INSPECTOR_EN_TEXT),
        TextLayout::justify(Justify::Right),
        Node {
            min_width: px(58),
            ..default()
        },
        InspectorText(InspectorTextKind::En),
        Pickable::IGNORE,
        ChildOf(energy_row),
    ));

    let inspector_stats = commands
        .spawn((
            InspectorStats,
            Node {
                width: percent(100),
                height: px(36),
                flex_shrink: 0.0,
                margin: UiRect::top(px(12)),
                padding: UiRect::top(px(10)),
                border: UiRect::top(px(1)),
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(12),
                ..default()
            },
            BorderColor::all(theme::BORDER),
            Visibility::Hidden,
            Pickable::IGNORE,
            ChildOf(inspector),
        ))
        .id();
    spawn_inspector_stat(
        &mut commands,
        inspector_stats,
        &ui_assets,
        theme::INSPECTOR_ARMOR_RECT,
        InspectorTextKind::Armor,
        "—",
    );
    spawn_inspector_stat(
        &mut commands,
        inspector_stats,
        &ui_assets,
        theme::INSPECTOR_MOBILITY_RECT,
        InspectorTextKind::Movement,
        "—",
    );
    spawn_inspector_stat(
        &mut commands,
        inspector_stats,
        &ui_assets,
        theme::INSPECTOR_EVASION_RECT,
        InspectorTextKind::Evasion,
        "—",
    );
    commands.spawn((
        theme::icon_node(ui_assets.icons.clone(), theme::ICON_GUARD, theme::MUTED),
        Node {
            width: px(20),
            height: px(20),
            margin: UiRect::left(Val::Auto),
            ..default()
        },
        InspectorIcon(InspectorIconKind::Reaction),
        Pickable::IGNORE,
        ChildOf(inspector_stats),
    ));
    commands.spawn((
        theme::icon_node(
            ui_assets.icons.clone(),
            theme::ICON_FORWARD_COMPACT,
            theme::MUTED,
        ),
        Node {
            width: px(20),
            height: px(20),
            ..default()
        },
        InspectorIcon(InspectorIconKind::Activation),
        Pickable::IGNORE,
        ChildOf(inspector_stats),
    ));
    let empty = commands
        .spawn((
            InspectorEmpty,
            Node {
                width: percent(100),
                height: px(150),
                flex_shrink: 0.0,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(14),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(8, 14, 24)),
            BorderColor::all(Color::srgb_u8(16, 29, 41)),
            Visibility::Visible,
            Pickable::IGNORE,
            ChildOf(inspector),
        ))
        .id();
    commands.spawn((
        theme::icon_node(
            ui_assets.icons.clone(),
            theme::EMPTY_INSPECTOR_RECT,
            Color::WHITE,
        ),
        Node {
            width: px(42),
            height: px(42),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(empty),
    ));
    let empty_pips = commands
        .spawn((
            Node {
                display: Display::Flex,
                column_gap: px(4),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(empty),
        ))
        .id();
    for _ in 0..3 {
        commands.spawn((
            Node {
                width: px(16),
                height: px(6),
                ..default()
            },
            BackgroundColor(theme::BORDER),
            Pickable::IGNORE,
            ChildOf(empty_pips),
        ));
    }
    spawn_battle_menu(&mut commands, sidebar, &ui_assets);

    let log_panel = commands
        .spawn((
            Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                padding: UiRect::all(px(16)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(theme::PANEL),
            BorderColor::all(theme::BORDER),
            Pickable::IGNORE,
            ChildOf(sidebar),
        ))
        .id();
    let log_header = commands
        .spawn((
            Node {
                width: percent(100),
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(10),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(log_panel),
        ))
        .id();
    commands.spawn((
        theme::icon_node(ui_assets.icons.clone(), theme::LOG_RECT, Color::WHITE),
        Node {
            width: px(18),
            height: px(18),
            flex_shrink: 0.0,
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(log_header),
    ));
    commands.spawn((
        Text::new("LOG"),
        theme::ibm_plex_mono(&ui_assets.fonts, 13.0, FontWeight(400)),
        LetterSpacing::Px(2.6),
        TextColor(theme::MUTED),
        Pickable::IGNORE,
        ChildOf(log_header),
    ));
    for index in 0..6 {
        let entry = commands
            .spawn((
                Node {
                    width: percent(100),
                    min_width: px(0),
                    display: Display::None,
                    align_items: AlignItems::Center,
                    column_gap: px(10),
                    ..default()
                },
                Visibility::Hidden,
                LogEntryRow(index),
                Pickable::IGNORE,
                ChildOf(log_panel),
            ))
            .id();
        commands.spawn((
            Node {
                width: px(8),
                height: px(8),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(theme::BORDER),
            LogEntryDot(index),
            Pickable::IGNORE,
            ChildOf(entry),
        ));
        commands.spawn((
            Text::new(""),
            theme::ibm_plex_mono(&ui_assets.fonts, 15.0, FontWeight(400)),
            TextColor(Color::srgb_u8(159, 182, 201)),
            Node {
                min_width: px(0),
                flex_grow: 1.0,
                ..default()
            },
            LogEntryText(index),
            Pickable::IGNORE,
            ChildOf(entry),
        ));
    }
    commands.spawn((
        Text::new(""),
        theme::chakra_petch(&ui_assets.fonts, 13.0, FontWeight(400)),
        TextColor(theme::GOLD),
        Node {
            margin: UiRect::top(px(8)),
            ..default()
        },
        StatusText,
        HudTextRole::Status,
        Pickable::IGNORE,
        ChildOf(log_panel),
    ));

    let rightbar = commands
        .spawn((
            BattleRightbar,
            Node {
                position_type: PositionType::Absolute,
                right: px(22),
                top: px(114),
                width: px(352),
                height: px(944),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(12),
                min_height: px(0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        Text::new("// OBJECTIVES"),
        theme::ibm_plex_mono(&ui_assets.fonts, 13.0, FontWeight(500)),
        TextColor(theme::ACCENT),
        Node {
            width: percent(100),
            display: Display::None,
            ..default()
        },
        BackgroundColor(theme::PANEL),
        ObjectiveText,
        HudTextRole::Objective,
        Visibility::Hidden,
        Pickable::IGNORE,
        ChildOf(rightbar),
    ));
    let locked = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(64),
                flex_shrink: 0.0,
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(12),
                padding: UiRect::horizontal(px(16)),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(22, 13, 13)),
            BorderColor::all(theme::RESULT_DEFEAT_BORDER),
            Pickable::IGNORE,
            ChildOf(rightbar),
        ))
        .id();
    commands.spawn((
        theme::icon_node(ui_assets.icons.clone(), theme::LOCKED_RECT, Color::WHITE),
        Node {
            width: px(24),
            height: px(24),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(locked),
    ));
    commands.spawn((
        Text::new("LOCKED"),
        theme::ibm_plex_mono(&ui_assets.fonts, 14.0, FontWeight(400)),
        LetterSpacing::Px(2.8),
        TextColor(Color::srgb_u8(255, 156, 144)),
        Pickable::IGNORE,
        ChildOf(locked),
    ));
    commands.spawn((
        Text::new("0"),
        theme::ibm_plex_mono(&ui_assets.fonts, 26.0, FontWeight(600)),
        TextColor(theme::ENEMY),
        Node {
            margin: UiRect::left(Val::Auto),
            ..default()
        },
        HudTextRole::ThreatCount,
        Pickable::IGNORE,
        ChildOf(locked),
    ));
    let preview = commands
        .spawn((
            PreviewPanel,
            Node {
                width: percent(100),
                flex_shrink: 0.0,
                display: Display::None,
                flex_direction: FlexDirection::Column,
                row_gap: px(12),
                padding: UiRect::all(px(16)),
                ..default()
            },
            BackgroundColor(theme::PANEL),
            BorderColor::all(Color::srgb_u8(51, 48, 28)),
            Visibility::Hidden,
            Pickable::IGNORE,
            ChildOf(rightbar),
        ))
        .id();
    let preview_header = commands
        .spawn((
            Node {
                width: percent(100),
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(10),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(preview),
        ))
        .id();
    commands.spawn((
        theme::icon_node(ui_assets.icons.clone(), theme::PREVIEW_RECT, Color::WHITE),
        Node {
            width: px(18),
            height: px(18),
            flex_shrink: 0.0,
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(preview_header),
    ));
    commands.spawn((
        Text::new("PREVIEW"),
        theme::ibm_plex_mono(&ui_assets.fonts, 13.0, FontWeight(400)),
        LetterSpacing::Px(2.6),
        TextColor(Color::srgb_u8(138, 124, 78)),
        PreviewText,
        HudTextRole::Preview,
        Pickable::IGNORE,
        ChildOf(preview_header),
    ));
    let preview_values = commands
        .spawn((
            Node {
                width: percent(100),
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(18),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(preview),
        ))
        .id();
    let preview_damage = commands
        .spawn((
            Node {
                display: Display::Flex,
                align_items: AlignItems::FlexEnd,
                column_gap: px(6),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(preview_values),
        ))
        .id();
    commands.spawn((
        Text::new("—"),
        theme::ibm_plex_mono(&ui_assets.fonts, 40.0, FontWeight(600)),
        LineHeight::RelativeToFont(0.9),
        TextColor(theme::GOLD),
        PreviewValue(PreviewValueKind::Damage),
        Pickable::IGNORE,
        ChildOf(preview_damage),
    ));
    commands.spawn((
        Text::new("/—"),
        theme::ibm_plex_mono(&ui_assets.fonts, 16.0, FontWeight(400)),
        TextColor(theme::MUTED),
        Node {
            margin: UiRect::bottom(px(5)),
            ..default()
        },
        PreviewValue(PreviewValueKind::Critical),
        Pickable::IGNORE,
        ChildOf(preview_damage),
    ));
    commands.spawn((
        Node {
            width: px(1),
            height: px(34),
            ..default()
        },
        BackgroundColor(theme::BORDER),
        Pickable::IGNORE,
        ChildOf(preview_values),
    ));
    let preview_hit = commands
        .spawn((
            Node {
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(8),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(preview_values),
        ))
        .id();
    commands.spawn((
        theme::icon_node(
            ui_assets.icons.clone(),
            theme::PREVIEW_HIT_RECT,
            Color::WHITE,
        ),
        Node {
            width: px(18),
            height: px(18),
            flex_shrink: 0.0,
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(preview_hit),
    ));
    commands.spawn((
        Text::new("—"),
        theme::ibm_plex_mono(&ui_assets.fonts, 26.0, FontWeight(400)),
        TextColor(Color::srgb_u8(207, 224, 236)),
        PreviewValue(PreviewValueKind::Hit),
        Pickable::IGNORE,
        ChildOf(preview_hit),
    ));
    let preview_energy = commands
        .spawn((
            Node {
                display: Display::Flex,
                column_gap: px(4),
                margin: UiRect::left(Val::Auto),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(preview_values),
        ))
        .id();
    for index in 0..5 {
        commands.spawn((
            Node {
                width: px(6),
                height: px(10),
                ..default()
            },
            BackgroundColor(theme::BORDER),
            PreviewMeter(index),
            Pickable::IGNORE,
            ChildOf(preview_energy),
        ));
    }
    let threat_list = commands
        .spawn((
            Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(12),
                overflow: Overflow::clip(),
                ..default()
            },
            ThreatList,
            Pickable::IGNORE,
            ChildOf(rightbar),
        ))
        .id();
    for card in 0..8 {
        spawn_threat_card(&mut commands, threat_list, &ui_assets, card);
    }

    let result_overlay = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                display: Display::Flex,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.008, 0.016, 0.04, 0.86)),
            Visibility::Hidden,
            GlobalZIndex(50),
            ResultOverlay,
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    let result_card = commands
        .spawn((
            Node {
                width: px(760),
                padding: UiRect::all(px(52)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(30),
                align_items: AlignItems::Center,
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(theme::RESULT_CARD_BACKGROUND),
            BorderColor::all(theme::RESULT_VICTORY_BORDER),
            ResultCard,
            Pickable::IGNORE,
            ChildOf(result_overlay),
        ))
        .id();
    let result_ring = commands
        .spawn((
            Node {
                width: px(168),
                height: px(168),
                display: Display::Flex,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(percent(50)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(theme::MINT),
            ResultRing,
            Pickable::IGNORE,
            ChildOf(result_card),
        ))
        .id();
    commands.spawn((
        theme::icon_node(
            ui_assets.icons.clone(),
            theme::RESULT_VICTORY_RECT,
            Color::WHITE,
        ),
        Node {
            width: px(86),
            height: px(86),
            ..default()
        },
        ResultIcon,
        Pickable::IGNORE,
        ChildOf(result_ring),
    ));
    commands.spawn((
        Text::new(""),
        theme::chakra_petch(&ui_assets.fonts, 46.0, FontWeight(700)),
        LetterSpacing::Px(9.2),
        TextColor(theme::RESULT_VICTORY_TEXT),
        ResultHeadline,
        HudTextRole::Result,
        Pickable::IGNORE,
        ChildOf(result_card),
    ));
    commands.spawn((
        Text::new(""),
        theme::chakra_petch(&ui_assets.fonts, 26.0, FontWeight(400)),
        TextColor(theme::RESULT_DEFEAT_TEXT),
        TextLayout::justify(Justify::Center),
        Node {
            max_width: px(650),
            ..default()
        },
        ResultDetail,
        Pickable::IGNORE,
        ChildOf(result_card),
    ));
    let result_metrics = commands
        .spawn((
            Node {
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(14),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(result_card),
        ))
        .id();
    for (icon, color) in [
        (theme::RESULT_PRIMARY_RECT, theme::ACCENT),
        (theme::RESULT_BONUS_RECT, theme::GOLD),
    ] {
        let metric = commands
            .spawn((
                Node {
                    padding: UiRect::axes(px(16), px(22)),
                    display: Display::Flex,
                    align_items: AlignItems::Center,
                    column_gap: px(12),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgb_u8(11, 20, 32)),
                BorderColor::all(if color == theme::GOLD {
                    Color::srgb_u8(51, 48, 28)
                } else {
                    theme::BORDER
                }),
                Pickable::IGNORE,
                ChildOf(result_metrics),
            ))
            .id();
        commands.spawn((
            theme::icon_node(ui_assets.icons.clone(), icon, Color::WHITE),
            Node {
                width: px(24),
                height: px(24),
                ..default()
            },
            ResultMetricIcon {
                bonus: color == theme::GOLD,
            },
            Pickable::IGNORE,
            ChildOf(metric),
        ));
        if color == theme::GOLD {
            commands.spawn((
                Node {
                    width: px(14),
                    height: px(14),
                    ..default()
                },
                BackgroundColor(theme::BORDER),
                HeaderBonusDot,
                Pickable::IGNORE,
                ChildOf(metric),
            ));
        } else {
            commands.spawn((
                Text::new("—"),
                theme::ibm_plex_mono(&ui_assets.fonts, 25.0, FontWeight(600)),
                TextColor(theme::TEXT),
                HudTextRole::ResultPrimary,
                Pickable::IGNORE,
                ChildOf(metric),
            ));
        }
    }
    commands.spawn((
        Text::new(""),
        theme::ibm_plex_mono(&ui_assets.fonts, 15.0, FontWeight(400)),
        TextColor(theme::GOLD),
        Node {
            max_width: px(650),
            display: Display::None,
            ..default()
        },
        ResultStatus,
        Pickable::IGNORE,
        ChildOf(result_card),
    ));
    spawn_command_button(
        &mut commands,
        &ui_assets.fonts,
        &ui_assets.icons,
        result_card,
        CommandAction::Restart,
        "RETRY",
        0.0,
    );
    spawn_command_button(
        &mut commands,
        &ui_assets.fonts,
        &ui_assets.icons,
        result_card,
        CommandAction::ContinueVictory,
        "CONTINUE",
        0.0,
    );

    commands.spawn((
        Text::new("Loading battle UI assets..."),
        theme::chakra_petch(&ui_assets.fonts, 18.0, FontWeight(500)),
        TextColor(theme::GOLD),
        BackgroundColor(Color::srgba(0.08, 0.025, 0.025, 0.94)),
        Node {
            position_type: PositionType::Absolute,
            right: px(24),
            top: px(122),
            padding: UiRect::all(px(12)),
            ..default()
        },
        Pickable::IGNORE,
        AssetStatusText,
        ChildOf(root),
    ));
}

fn spawn_header_metric(
    commands: &mut Commands,
    parent: Entity,
    assets: &UiAssets,
    icon: Rect,
    color: Color,
    value: HeaderValue,
    initial: &'static str,
) {
    let metric = commands
        .spawn((
            Node {
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(8),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        theme::icon_node(assets.icons.clone(), icon, Color::WHITE),
        Node {
            width: px(24),
            height: px(24),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(metric),
    ));
    commands.spawn((
        Text::new(initial),
        theme::ibm_plex_mono(&assets.fonts, 24.0, FontWeight(600)),
        TextColor(color),
        value,
        Pickable::IGNORE,
        ChildOf(metric),
    ));
}

fn spawn_inspector_stat(
    commands: &mut Commands,
    parent: Entity,
    assets: &UiAssets,
    icon: Rect,
    kind: InspectorTextKind,
    initial: &'static str,
) {
    let stat = commands
        .spawn((
            Node {
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(7),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        theme::icon_node(assets.icons.clone(), icon, Color::WHITE),
        Node {
            width: px(17),
            height: px(17),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(stat),
    ));
    commands.spawn((
        Text::new(initial),
        theme::ibm_plex_mono(&assets.fonts, 16.0, FontWeight(400)),
        TextColor(theme::INSPECTOR_STAT_TEXT),
        InspectorText(kind),
        Pickable::IGNORE,
        ChildOf(stat),
    ));
}

fn spawn_threat_card(commands: &mut Commands, parent: Entity, assets: &UiAssets, card: usize) {
    let card_entity = commands
        .spawn((
            ThreatCard(card),
            Node {
                width: percent(100),
                min_height: px(0),
                flex_shrink: 0.0,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(16)),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(20, 12, 12)),
            BorderColor::all(Color::srgb_u8(74, 43, 34)),
            Visibility::Hidden,
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    let top = commands
        .spawn((
            Node {
                width: percent(100),
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(12),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(card_entity),
        ))
        .id();
    commands.spawn((
        theme::icon_node(
            assets.icons.clone(),
            theme::UNIT_GLYPH_SQUARE_RECT,
            theme::ENEMY,
        ),
        Node {
            width: px(18),
            height: px(18),
            flex_shrink: 0.0,
            ..default()
        },
        ThreatIcon {
            card,
            target: false,
        },
        Pickable::IGNORE,
        ChildOf(top),
    ));
    commands.spawn((
        Text::new("—"),
        theme::chakra_petch(&assets.fonts, 19.0, FontWeight(600)),
        TextColor(Color::srgb_u8(240, 220, 216)),
        ThreatText {
            card,
            kind: ThreatTextKind::Attacker,
        },
        Node {
            width: percent(100),
            min_width: px(0),
            flex_grow: 1.0,
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(top),
    ));
    commands.spawn((
        theme::icon_node(assets.icons.clone(), theme::THREAT_ARROW_RECT, Color::WHITE),
        Node {
            width: px(22),
            height: px(22),
            flex_shrink: 0.0,
            margin: UiRect::left(Val::Auto),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(top),
    ));
    commands.spawn((
        theme::icon_node(
            assets.icons.clone(),
            theme::UNIT_GLYPH_SQUARE_RECT,
            theme::ENEMY,
        ),
        Node {
            width: px(18),
            height: px(18),
            flex_shrink: 0.0,
            ..default()
        },
        ThreatIcon { card, target: true },
        Pickable::IGNORE,
        ChildOf(top),
    ));

    let details = commands
        .spawn((
            Node {
                width: percent(100),
                display: Display::Flex,
                align_items: AlignItems::FlexEnd,
                column_gap: px(16),
                margin: UiRect::top(px(14)),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(card_entity),
        ))
        .id();
    let shape = commands
        .spawn((
            Node {
                width: px(42),
                height: px(42),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(3),
                flex_shrink: 0.0,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(details),
        ))
        .id();
    for row in 0..3 {
        let shape_row = commands
            .spawn((
                Node {
                    width: percent(100),
                    height: px(12),
                    display: Display::Flex,
                    column_gap: px(3),
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(shape),
            ))
            .id();
        for column in 0..3 {
            let index = row * 3 + column;
            commands.spawn((
                Node {
                    width: px(12),
                    height: px(12),
                    ..default()
                },
                BackgroundColor(theme::BORDER),
                ThreatMeter(ThreatMeterKind::Shape { card, index }),
                Pickable::IGNORE,
                ChildOf(shape_row),
            ));
        }
    }
    let damage = commands
        .spawn((
            Node {
                display: Display::Flex,
                align_items: AlignItems::FlexEnd,
                column_gap: px(6),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(details),
        ))
        .id();
    commands.spawn((
        Text::new("—"),
        theme::ibm_plex_mono(&assets.fonts, 38.0, FontWeight(600)),
        LineHeight::RelativeToFont(0.9),
        TextColor(Color::srgb_u8(255, 143, 128)),
        ThreatText {
            card,
            kind: ThreatTextKind::Damage,
        },
        Pickable::IGNORE,
        ChildOf(damage),
    ));
    commands.spawn((
        theme::icon_node(
            assets.icons.clone(),
            theme::THREAT_DAMAGE_RECT,
            Color::WHITE,
        ),
        Node {
            width: px(20),
            height: px(20),
            margin: UiRect::bottom(px(5)),
            flex_shrink: 0.0,
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(damage),
    ));
    let hit = commands
        .spawn((
            Node {
                display: Display::Flex,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                flex_shrink: 0.0,
                margin: UiRect::left(Val::Auto),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(details),
        ))
        .id();
    let hit_value = commands
        .spawn((
            Node {
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(7),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(hit),
        ))
        .id();
    commands.spawn((
        theme::icon_node(assets.icons.clone(), theme::THREAT_HIT_RECT, Color::WHITE),
        Node {
            width: px(17),
            height: px(17),
            flex_shrink: 0.0,
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(hit_value),
    ));
    commands.spawn((
        Text::new("—"),
        theme::ibm_plex_mono(&assets.fonts, 21.0, FontWeight(400)),
        TextColor(Color::srgb_u8(224, 192, 184)),
        ThreatText {
            card,
            kind: ThreatTextKind::Hit,
        },
        Pickable::IGNORE,
        ChildOf(hit_value),
    ));
    let hit_track = commands
        .spawn((
            Node {
                position_type: PositionType::Relative,
                width: px(92),
                height: px(6),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(42, 21, 18)),
            Pickable::IGNORE,
            ChildOf(hit),
        ))
        .id();
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            bottom: px(0),
            width: percent(0),
            ..default()
        },
        BackgroundColor(theme::ENEMY),
        ThreatMeter(ThreatMeterKind::Hit(card)),
        Pickable::IGNORE,
        ChildOf(hit_track),
    ));
}

#[derive(SystemParam)]
#[allow(clippy::type_complexity)]
pub struct HudQueries<'w, 's> {
    texts: Query<
        'w,
        's,
        (
            &'static HudTextRole,
            &'static mut Text,
            Option<&'static mut Visibility>,
        ),
        (
            Without<ResultOverlay>,
            Without<HeaderValue>,
            Without<InspectorPortrait>,
            Without<InspectorTop>,
            Without<InspectorStats>,
            Without<InspectorEmpty>,
            Without<ResultDetail>,
            Without<ResultStatus>,
            Without<LogEntryText>,
        ),
    >,
    native_texts: Query<
        'w,
        's,
        (
            &'static mut Text,
            Option<&'static InspectorText>,
            Option<&'static WeaponText>,
            Option<&'static ThreatText>,
            Option<&'static PreviewValue>,
        ),
        (
            Or<(
                With<InspectorText>,
                With<WeaponText>,
                With<ThreatText>,
                With<PreviewValue>,
            )>,
            Without<HudTextRole>,
            Without<HeaderValue>,
            Without<ResultDetail>,
            Without<ResultStatus>,
            Without<LogEntryText>,
        ),
    >,
    weapon_rows: Query<'w, 's, (&'static WeaponRow, &'static mut AccessibilityNode)>,
    button_and_inspector: ParamSet<
        'w,
        's,
        (
            Query<
                'w,
                's,
                (
                    &'static CommandButton,
                    Option<&'static HeaderRestart>,
                    &'static mut BackgroundColor,
                    &'static mut BorderColor,
                    &'static mut Pickable,
                    &'static mut Visibility,
                    &'static mut Node,
                ),
                (
                    Without<HudTextRole>,
                    Without<ResultOverlay>,
                    Without<HeaderPrimaryPip>,
                    Without<HeaderBonusDot>,
                    Without<InspectorPortrait>,
                    Without<InspectorTop>,
                    Without<InspectorStats>,
                    Without<InspectorEmpty>,
                    Without<WeaponTagIcon>,
                    Without<ResultDetail>,
                    Without<ResultStatus>,
                    Without<InspectorMeter>,
                    Without<WeaponMeter>,
                    Without<ThreatMeter>,
                    Without<ResultRing>,
                ),
            >,
            Query<
                'w,
                's,
                (
                    &'static mut Visibility,
                    &'static mut Node,
                    Option<&'static InspectorTop>,
                    Option<&'static InspectorStats>,
                    Option<&'static InspectorEmpty>,
                    Option<&'static InspectorPanel>,
                    Option<&'static mut BackgroundColor>,
                    Option<&'static mut Outline>,
                ),
                (
                    Or<(
                        With<InspectorTop>,
                        With<InspectorStats>,
                        With<InspectorEmpty>,
                        With<InspectorPanel>,
                    )>,
                    Without<ResultOverlay>,
                    Without<InspectorMeter>,
                    Without<WeaponMeter>,
                    Without<ThreatMeter>,
                    Without<ThreatCard>,
                    Without<InspectorPortrait>,
                    Without<WeaponTagIcon>,
                    Without<HudTextRole>,
                    Without<InspectorEnergyRow>,
                    Without<PreviewPanel>,
                    Without<LogEntryText>,
                    Without<LogEntryRow>,
                    Without<ResultDetail>,
                    Without<ResultStatus>,
                ),
            >,
        ),
    >,
    result_overlays: Query<
        'w,
        's,
        &'static mut Visibility,
        (
            With<ResultOverlay>,
            Without<HudTextRole>,
            Without<CommandButton>,
            Without<InspectorPortrait>,
            Without<InspectorTop>,
            Without<InspectorStats>,
            Without<InspectorEmpty>,
            Without<ThreatCard>,
            Without<WeaponTagIcon>,
            Without<InspectorEnergyRow>,
            Without<PreviewPanel>,
            Without<LogEntryText>,
            Without<LogEntryRow>,
        ),
    >,
    header_values: Query<
        'w,
        's,
        (&'static HeaderValue, &'static mut Text),
        (
            Without<HudTextRole>,
            Without<ResultDetail>,
            Without<ResultStatus>,
            Without<LogEntryText>,
        ),
    >,
    inspector_portraits: Query<
        'w,
        's,
        (&'static mut ImageNode, &'static mut Visibility),
        (
            With<InspectorPortrait>,
            Without<ResultOverlay>,
            Without<CommandButton>,
            Without<ResultIcon>,
            Without<ResultMetricIcon>,
            Without<WeaponTagIcon>,
            Without<HudTextRole>,
            Without<ThreatCard>,
            Without<InspectorEnergyRow>,
            Without<PreviewPanel>,
            Without<LogEntryText>,
            Without<LogEntryRow>,
        ),
    >,
    primary_pips: Query<
        'w,
        's,
        (&'static HeaderPrimaryPip, &'static mut BackgroundColor),
        (
            Without<HeaderBonusDot>,
            Without<CommandButton>,
            Without<PreviewMeter>,
            Without<LogEntryDot>,
            Without<LogEntryRow>,
            Without<InspectorTop>,
            Without<InspectorStats>,
            Without<InspectorEmpty>,
            Without<InspectorPanel>,
        ),
    >,
    bonus_dots: Query<
        'w,
        's,
        &'static mut BackgroundColor,
        (
            With<HeaderBonusDot>,
            Without<HeaderPrimaryPip>,
            Without<CommandButton>,
            Without<PreviewMeter>,
            Without<LogEntryDot>,
            Without<LogEntryRow>,
            Without<InspectorTop>,
            Without<InspectorStats>,
            Without<InspectorEmpty>,
            Without<InspectorPanel>,
        ),
    >,
    inspector_energy_rows: Query<
        'w,
        's,
        (&'static mut Visibility, &'static mut Node),
        (
            With<InspectorEnergyRow>,
            Without<InspectorPortrait>,
            Without<CommandButton>,
            Without<HudTextRole>,
            Without<ResultOverlay>,
            Without<InspectorTop>,
            Without<InspectorStats>,
            Without<PreviewPanel>,
            Without<LogEntryText>,
            Without<LogEntryRow>,
            Without<InspectorPanel>,
        ),
    >,
    preview_panels: Query<
        'w,
        's,
        (&'static mut Visibility, &'static mut Node),
        (
            With<PreviewPanel>,
            Without<ResultOverlay>,
            Without<InspectorPortrait>,
            Without<InspectorEnergyRow>,
            Without<CommandButton>,
            Without<HudTextRole>,
            Without<LogEntryText>,
            Without<LogEntryRow>,
        ),
    >,
    preview_meters: Query<
        'w,
        's,
        (&'static PreviewMeter, &'static mut BackgroundColor),
        (
            Without<InspectorMeter>,
            Without<WeaponMeter>,
            Without<ThreatMeter>,
            Without<CommandButton>,
            Without<LogEntryDot>,
            Without<InspectorTop>,
            Without<InspectorStats>,
            Without<InspectorEmpty>,
            Without<InspectorPanel>,
        ),
    >,
    log_entries: Query<
        'w,
        's,
        (
            &'static LogEntryText,
            &'static mut Text,
            &'static mut Visibility,
            &'static mut Node,
        ),
        (
            Without<HudTextRole>,
            Without<PlaybackText>,
            Without<InspectorPortrait>,
            Without<CommandButton>,
            Without<LogEntryRow>,
        ),
    >,
    log_rows: Query<
        'w,
        's,
        (
            &'static LogEntryRow,
            &'static mut Visibility,
            &'static mut Node,
        ),
        (
            Without<CommandButton>,
            Without<LogEntryText>,
            Without<HudTextRole>,
        ),
    >,
    log_dots: Query<
        'w,
        's,
        (&'static LogEntryDot, &'static mut BackgroundColor),
        (
            Without<HeaderPrimaryPip>,
            Without<HeaderBonusDot>,
            Without<CommandButton>,
            Without<PreviewMeter>,
            Without<InspectorTop>,
            Without<InspectorStats>,
            Without<InspectorEmpty>,
            Without<InspectorPanel>,
        ),
    >,
    inspector_icons: Query<
        'w,
        's,
        (&'static mut ImageNode, &'static InspectorIcon),
        (
            Without<InspectorPortrait>,
            Without<ResultIcon>,
            Without<ResultMetricIcon>,
            Without<ThreatIcon>,
            Without<WeaponTagIcon>,
        ),
    >,
    inspector_meters: Query<
        'w,
        's,
        (
            &'static InspectorMeter,
            &'static mut Node,
            &'static mut BackgroundColor,
        ),
        (
            Without<HeaderPrimaryPip>,
            Without<HeaderBonusDot>,
            Without<CommandButton>,
            Without<WeaponMeter>,
            Without<ThreatMeter>,
            Without<ThreatCard>,
            Without<ResultOverlay>,
            Without<PreviewMeter>,
            Without<InspectorEnergyRow>,
            Without<PreviewPanel>,
            Without<LogEntryText>,
            Without<LogEntryDot>,
            Without<LogEntryRow>,
        ),
    >,
    threat_cards: Query<
        'w,
        's,
        (
            &'static ThreatCard,
            &'static mut Visibility,
            &'static mut Node,
        ),
        (
            Without<InspectorTop>,
            Without<InspectorStats>,
            Without<InspectorEmpty>,
            Without<InspectorMeter>,
            Without<WeaponMeter>,
            Without<ThreatMeter>,
            Without<ResultOverlay>,
            Without<CommandButton>,
            Without<InspectorPortrait>,
            Without<WeaponTagIcon>,
            Without<HudTextRole>,
            Without<InspectorEnergyRow>,
            Without<PreviewPanel>,
            Without<LogEntryText>,
            Without<LogEntryRow>,
        ),
    >,
    threat_icons: Query<
        'w,
        's,
        (&'static mut ImageNode, &'static ThreatIcon),
        (
            Without<InspectorPortrait>,
            Without<ResultIcon>,
            Without<ResultMetricIcon>,
            Without<InspectorIcon>,
            Without<WeaponTagIcon>,
        ),
    >,
    threat_meters: Query<
        'w,
        's,
        (
            &'static ThreatMeter,
            &'static mut Node,
            &'static mut BackgroundColor,
        ),
        (
            Without<HeaderPrimaryPip>,
            Without<HeaderBonusDot>,
            Without<CommandButton>,
            Without<InspectorMeter>,
            Without<WeaponMeter>,
            Without<ThreatCard>,
            Without<ResultOverlay>,
            Without<InspectorEnergyRow>,
            Without<PreviewPanel>,
            Without<LogEntryText>,
            Without<PreviewMeter>,
            Without<LogEntryDot>,
            Without<LogEntryRow>,
        ),
    >,
    weapon_meters: Query<
        'w,
        's,
        (
            &'static WeaponMeter,
            &'static mut Node,
            &'static mut BackgroundColor,
        ),
        (
            Without<HeaderPrimaryPip>,
            Without<HeaderBonusDot>,
            Without<CommandButton>,
            Without<InspectorMeter>,
            Without<ThreatMeter>,
            Without<ThreatCard>,
            Without<ResultOverlay>,
            Without<InspectorEnergyRow>,
            Without<PreviewPanel>,
            Without<LogEntryText>,
            Without<PreviewMeter>,
            Without<LogEntryDot>,
            Without<LogEntryRow>,
        ),
    >,
    weapon_tags: Query<
        'w,
        's,
        (
            &'static WeaponTagIcon,
            &'static mut ImageNode,
            &'static mut Visibility,
        ),
        (
            Without<InspectorPortrait>,
            Without<InspectorIcon>,
            Without<ThreatIcon>,
            Without<ResultIcon>,
            Without<ResultMetricIcon>,
            Without<CommandButton>,
            Without<HudTextRole>,
            Without<ThreatCard>,
            Without<ResultOverlay>,
            Without<HeaderPrimaryPip>,
            Without<HeaderBonusDot>,
            Without<InspectorEnergyRow>,
            Without<PreviewPanel>,
            Without<LogEntryText>,
            Without<LogEntryRow>,
        ),
    >,
    result_icons:
        Query<'w, 's, &'static mut ImageNode, (With<ResultIcon>, Without<InspectorPortrait>)>,
    result_metric_icons: Query<
        'w,
        's,
        (&'static mut ImageNode, &'static ResultMetricIcon),
        (With<ResultMetricIcon>, Without<ResultIcon>),
    >,
    result_cards: Query<
        'w,
        's,
        (&'static mut BackgroundColor, &'static mut BorderColor),
        (
            With<ResultCard>,
            Without<CommandButton>,
            Without<HeaderPrimaryPip>,
            Without<HeaderBonusDot>,
            Without<InspectorMeter>,
            Without<ThreatMeter>,
            Without<WeaponMeter>,
            Without<PreviewMeter>,
            Without<LogEntryDot>,
            Without<InspectorTop>,
            Without<InspectorStats>,
            Without<InspectorEmpty>,
            Without<InspectorPanel>,
        ),
    >,
    result_rings: Query<'w, 's, &'static mut BorderColor, (With<ResultRing>, Without<ResultCard>)>,
    result_headlines: Query<'w, 's, &'static mut TextColor, With<ResultHeadline>>,
    result_details: Query<
        'w,
        's,
        (&'static mut Text, &'static mut Node),
        (
            With<ResultDetail>,
            Without<ResultStatus>,
            Without<CommandButton>,
            Without<InspectorTop>,
            Without<InspectorStats>,
            Without<InspectorEmpty>,
            Without<InspectorMeter>,
            Without<WeaponMeter>,
            Without<ThreatMeter>,
            Without<ThreatCard>,
            Without<PreviewPanel>,
            Without<InspectorEnergyRow>,
            Without<LogEntryText>,
            Without<LogEntryRow>,
        ),
    >,
    result_status: Query<
        'w,
        's,
        (&'static mut Text, &'static mut Node),
        (
            With<ResultStatus>,
            Without<ResultDetail>,
            Without<CommandButton>,
            Without<InspectorTop>,
            Without<InspectorStats>,
            Without<InspectorEmpty>,
            Without<InspectorMeter>,
            Without<WeaponMeter>,
            Without<ThreatMeter>,
            Without<ThreatCard>,
            Without<PreviewPanel>,
            Without<InspectorEnergyRow>,
            Without<LogEntryText>,
            Without<LogEntryRow>,
        ),
    >,
}

#[allow(clippy::too_many_arguments)]
pub fn update_hud(
    battle: Res<BattleRuntime>,
    interaction: Res<InteractionState>,
    status: Res<StatusMessage>,
    playback: Res<EventPlayback>,
    recent_log: Option<Res<RecentBattleLog>>,
    active_mission: Res<ActiveMission>,
    campaign: Option<Res<CampaignRuntime>>,
    ui_assets: Res<UiAssets>,
    asset_status: Option<Res<AssetLoadStatus>>,
    next_state: Option<Res<NextState<GameScreen>>>,
    restart_request: Option<Res<RestartRequest>>,
    mut queries: HudQueries,
) {
    let hud = HudSnapshot::from_battle(&battle.0, interaction.inspected_unit, active_mission.0);
    let selected_threats: Vec<&ThreatSnapshot> = hud
        .threats
        .iter()
        .filter(|threat| {
            interaction.inspected_unit.is_some_and(|unit| {
                threat.attacker_id == unit || threat.intended_occupant_id == Some(unit)
            })
        })
        .collect();
    let status_text = if playback.input_locked {
        "Resolving committed events...".to_owned()
    } else {
        status.0.clone()
    };

    for (role, mut text, visibility) in &mut queries.texts {
        text.0 = match role {
            HudTextRole::Objective => {
                let mut text = format!(
                    "// OBJECTIVES\n{}\nPRIMARY  {}\nBONUS    {}",
                    ascii_separators(&hud.round_phase),
                    ascii_separators(&hud.primary),
                    ascii_separators(&hud.optional)
                );
                if let Some(track) = &hud.objective_track {
                    text.push_str(&format!(
                        "\nTRACK    {}",
                        ascii_separators(&format_track(track))
                    ));
                }
                text
            }
            HudTextRole::ThreatCount => hud.threats.len().to_string(),
            HudTextRole::Preview => "PREVIEW".to_owned(),
            HudTextRole::Status => status_text.clone(),
            HudTextRole::Result => battle.0.result().map_or_else(String::new, |result| {
                let copy = result_overlay_copy(result, battle.0.rules().primary, active_mission.0);
                copy.split_once('\n')
                    .map_or(copy.as_str(), |(headline, _)| headline)
                    .to_owned()
            }),
            HudTextRole::ResultPrimary => result_primary_progress(hud.objective_track.as_ref()),
        };
        let _ = visibility;
    }

    for (row, mut visibility, mut node) in &mut queries.log_rows {
        let shown = recent_log
            .as_deref()
            .is_some_and(|log| log.0.get(row.0).is_some());
        *visibility = if shown {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        node.display = if shown { Display::Flex } else { Display::None };
    }
    for (entry, mut text, mut visibility, mut node) in &mut queries.log_entries {
        let value = recent_log
            .as_deref()
            .and_then(|log| log.0.get(entry.0))
            .map(|value| value.replace('\n', " · "));
        let shown = value.is_some();
        text.0 = value.unwrap_or_default();
        *visibility = if shown {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        node.display = if shown { Display::Flex } else { Display::None };
    }
    for (dot, mut background) in &mut queries.log_dots {
        let value = recent_log.as_deref().and_then(|log| log.0.get(dot.0));
        background.0 = value.map_or(theme::BORDER, |value| log_dot_color(value));
    }
    for (mut visibility, mut node) in &mut queries.preview_panels {
        let shown = interaction.preview.is_some();
        *visibility = if shown {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        node.display = if shown { Display::Flex } else { Display::None };
    }
    for (meter, mut background) in &mut queries.preview_meters {
        let cost = interaction
            .preview
            .as_ref()
            .map_or(0, |preview| preview.en_cost.max(0) as usize);
        background.0 = if meter.0 < cost {
            theme::GOLD
        } else {
            theme::BORDER
        };
    }

    for (mut text, mut node) in &mut queries.result_details {
        let detail = battle.0.result().and_then(|result| {
            result_overlay_copy(result, battle.0.rules().primary, active_mission.0)
                .split_once('\n')
                .map(|(_, detail)| detail.to_owned())
        });
        text.0 = detail.unwrap_or_default();
        node.display = if battle.0.result().is_some_and(|result| !result.victory) {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (mut text, mut node) in &mut queries.result_status {
        let shown = hud.is_terminal && status.0.starts_with("save file error:");
        text.0 = if shown {
            status.0.clone()
        } else {
            String::new()
        };
        node.display = if shown { Display::Flex } else { Display::None };
    }

    for (mut text, inspector_text, weapon_text, threat_text, preview_value) in
        &mut queries.native_texts
    {
        if let Some(inspector_text) = inspector_text {
            text.0 = match inspector_text.0 {
                InspectorTextKind::Name => hud.inspector.name.unwrap_or("—").to_owned(),
                InspectorTextKind::Hp => match (hud.inspector.hp, hud.inspector.max_hp) {
                    (Some(hp), Some(max_hp)) => format!("{hp}/{max_hp}"),
                    _ => "—".to_owned(),
                },
                InspectorTextKind::En => match (hud.inspector.en, hud.inspector.max_en) {
                    (Some(en), Some(max_en)) => format!("{en}/{max_en}"),
                    _ => "—".to_owned(),
                },
                InspectorTextKind::Armor => hud
                    .inspector
                    .armor
                    .map_or_else(|| "—".to_owned(), |armor| format!("{armor}")),
                InspectorTextKind::Movement => hud
                    .inspector
                    .movement
                    .map_or_else(|| "—".to_owned(), |movement| format!("{movement}")),
                InspectorTextKind::Evasion => hud
                    .inspector
                    .evasion
                    .map_or_else(|| "—".to_owned(), |evasion| format!("{evasion}%")),
            };
        } else if let Some(weapon_text) = weapon_text {
            let weapon = hud
                .weapon_specs
                .get(weapon_text.slot)
                .and_then(Option::as_ref);
            text.0 = match (weapon, weapon_text.kind) {
                (Some(weapon), WeaponTextKind::Name) => weapon.name.to_owned(),
                (Some(weapon), WeaponTextKind::Damage) => weapon.base_damage.to_string(),
                (Some(weapon), WeaponTextKind::Hit) => format_signed(weapon.hit_modifier),
                _ => "—".to_owned(),
            };
        } else if let Some(threat_text) = threat_text {
            let threat = selected_threats.get(threat_text.card).copied();
            text.0 = match (threat, threat_text.kind) {
                (Some(threat), ThreatTextKind::Attacker) => threat.attacker.to_owned(),
                (Some(threat), ThreatTextKind::Weapon) => threat.weapon.to_owned(),
                (Some(threat), ThreatTextKind::Target) => {
                    threat.intended_occupant.unwrap_or("EMPTY").to_owned()
                }
                (Some(threat), ThreatTextKind::Damage) => threat.normal_damage.to_string(),
                (Some(threat), ThreatTextKind::Hit) => {
                    format!("{}%", threat.hit_chance)
                }
                _ => "—".to_owned(),
            };
        } else if let Some(preview_value) = preview_value {
            text.0 = interaction.preview.as_ref().map_or_else(
                || match preview_value.0 {
                    PreviewValueKind::Damage => "—".to_owned(),
                    PreviewValueKind::Critical => "/—".to_owned(),
                    PreviewValueKind::Hit => "—".to_owned(),
                },
                |preview| match preview_value.0 {
                    PreviewValueKind::Damage => preview.normal_damage.to_string(),
                    PreviewValueKind::Critical => format!("/{}", preview.critical_damage),
                    PreviewValueKind::Hit => format!("{}%", preview.hit_chance),
                },
            );
        }
    }

    for (row, mut accessibility) in &mut queries.weapon_rows {
        let label = hud
            .weapon_specs
            .get(row.0)
            .and_then(Option::as_ref)
            .map_or("Weapon", |weapon| weapon.name);
        accessibility.set_label(label.to_owned().into_boxed_str());
    }

    let inspector_selected = !hud.inspector.is_empty();
    for (mut visibility, mut node, top, stats, empty, panel, mut background, mut outline) in
        &mut queries.button_and_inspector.p1()
    {
        let shown = if panel.is_some() {
            node.height = Val::Auto;
            node.padding = if inspector_selected {
                UiRect::all(px(16))
            } else {
                UiRect::ZERO
            };
            if let Some(background) = background.as_deref_mut() {
                background.0 = match hud.inspector.faction {
                    Some(Faction::Player) => theme::INSPECTOR_PLAYER_BACKGROUND,
                    Some(Faction::Enemy) => theme::INSPECTOR_ENEMY_BACKGROUND,
                    None => theme::PANEL,
                };
            }
            if let Some(outline) = outline.as_deref_mut() {
                outline.color = match hud.inspector.faction {
                    Some(Faction::Player) => theme::ACCENT,
                    Some(Faction::Enemy) => theme::INSPECTOR_ENEMY_BORDER,
                    None => Color::NONE,
                };
            }
            true
        } else if top.is_some() || stats.is_some() {
            inspector_selected
        } else {
            !inspector_selected
        };
        *visibility = if shown {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        node.display = if shown { Display::Flex } else { Display::None };
        let _ = empty;
    }
    for (mut visibility, mut node) in &mut queries.inspector_energy_rows {
        let shown = inspector_selected && hud.inspector.faction == Some(Faction::Player);
        *visibility = if shown {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        node.display = if shown { Display::Flex } else { Display::None };
    }

    for (mut image, icon) in &mut queries.inspector_icons {
        let (rect, color) = match icon.0 {
            InspectorIconKind::Glyph => hud.inspector.archetype.map_or(
                (theme::UNIT_GLYPH_HEX_RECT, theme::MUTED),
                |archetype| {
                    let style = theme::unit_archetype_style(archetype);
                    (style.glyph_rect, style.color)
                },
            ),
            InspectorIconKind::Reaction => match hud.inspector.reaction {
                Some(Reaction::Counter) => (theme::ICON_COUNTER, theme::ENEMY),
                Some(Reaction::Guard) => (theme::ICON_GUARD, theme::ACCENT),
                Some(Reaction::Evade) => (theme::ICON_EVADE, theme::MINT),
                None => (theme::ICON_GUARD, theme::MUTED),
            },
            InspectorIconKind::Activation => {
                if hud.inspector.faction == Some(Faction::Enemy) {
                    (theme::ICON_ATTACK, theme::ENEMY)
                } else if hud.inspector.finished {
                    (theme::ICON_WAIT, theme::MINT)
                } else if inspector_selected {
                    (theme::ICON_FORWARD_COMPACT, theme::GOLD)
                } else {
                    (theme::ICON_FORWARD_COMPACT, theme::MUTED)
                }
            }
        };
        image.image = ui_assets.icons.clone();
        image.rect = Some(rect);
        image.color = color;
    }

    for (meter, mut node, mut background) in &mut queries.inspector_meters {
        match meter.0 {
            InspectorMeterKind::Hp => {
                let ratio = match (hud.inspector.hp, hud.inspector.max_hp) {
                    (Some(hp), Some(max_hp)) if max_hp > 0 => {
                        f32::from(hp.max(0)) / f32::from(max_hp)
                    }
                    _ => 0.0,
                };
                node.width = percent((ratio * 100.0).clamp(0.0, 100.0));
                background.0 = match hud.inspector.faction {
                    Some(Faction::Enemy) => theme::ENEMY,
                    Some(Faction::Player) => theme::MINT,
                    None => theme::MUTED,
                };
            }
            InspectorMeterKind::Energy(index) => {
                let en = hud.inspector.en.unwrap_or_default().max(0) as usize;
                background.0 = if index < en {
                    theme::INSPECTOR_EN_PIP_ACTIVE
                } else {
                    theme::INSPECTOR_EN_PIP_INACTIVE
                };
            }
        }
    }

    for (card, mut visibility, mut node) in &mut queries.threat_cards {
        let shown = selected_threats.get(card.0).is_some();
        *visibility = if shown {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        node.display = if shown { Display::Flex } else { Display::None };
    }
    for (mut image, icon) in &mut queries.threat_icons {
        let threat = selected_threats.get(icon.card).copied();
        let unit = threat.and_then(|threat| {
            if icon.target {
                threat.intended_occupant_id
            } else {
                Some(threat.attacker_id)
            }
        });
        let (rect, color) = unit.and_then(|unit| battle.0.unit(unit)).map_or(
            (theme::UNIT_GLYPH_SQUARE_RECT, theme::MUTED),
            |unit| {
                let style = theme::unit_archetype_style(unit.archetype);
                (style.glyph_rect, style.color)
            },
        );
        image.image = ui_assets.icons.clone();
        image.rect = Some(rect);
        image.color = color;
    }
    for (meter, mut node, mut background) in &mut queries.threat_meters {
        match meter.0 {
            ThreatMeterKind::Hit(card) => {
                let chance = selected_threats
                    .get(card)
                    .map_or(0.0, |threat| f32::from(threat.hit_chance));
                node.width = percent(chance);
                background.0 = theme::ENEMY;
            }
            ThreatMeterKind::Shape { card, index } => {
                let active = selected_threats
                    .get(card)
                    .is_some_and(|threat| shape_cell_active(threat.shape, index));
                background.0 = if active { theme::ENEMY } else { theme::BORDER };
            }
        }
    }
    for (meter, mut node, mut background) in &mut queries.weapon_meters {
        let weapon = hud.weapon_specs.get(meter.slot).and_then(Option::as_ref);
        match meter.kind {
            WeaponMeterKind::Range => {
                if let Some(weapon) = weapon {
                    let max_range = f32::from(weapon.max_range.max(1));
                    let min_range = f32::from(weapon.min_range);
                    node.left = percent((min_range / 8.0 * 100.0).clamp(0.0, 100.0));
                    node.width =
                        percent((((max_range - min_range + 1.0) / 8.0) * 100.0).clamp(0.0, 100.0));
                    background.0 = if weapon.enabled {
                        theme::GOLD
                    } else {
                        theme::MUTED
                    };
                } else {
                    node.width = percent(0.0);
                }
            }
            WeaponMeterKind::Energy(index) => {
                background.0 = weapon.map_or(theme::BORDER, |weapon| {
                    if index < weapon.en_cost.max(0) as usize {
                        theme::GOLD
                    } else {
                        theme::BORDER
                    }
                });
            }
            WeaponMeterKind::Shape(index) => {
                let active = weapon.is_some_and(|weapon| shape_cell_active(weapon.shape, index));
                background.0 = if active { theme::GOLD } else { theme::BORDER };
            }
        }
    }
    for (tag, mut image, mut visibility) in &mut queries.weapon_tags {
        let weapon = hud.weapon_specs.get(tag.slot).and_then(Option::as_ref);
        let shown = weapon.is_some_and(|weapon| match tag.marker {
            WeaponTag::Push => weapon.push,
            WeaponTag::Counter => weapon.counter_weapon,
        });
        *visibility = if shown {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        image.image = ui_assets.icons.clone();
        image.rect = Some(match tag.marker {
            WeaponTag::Push => theme::ICON_MOVE,
            WeaponTag::Counter => theme::ICON_COUNTER,
        });
        image.color = match tag.marker {
            WeaponTag::Push => theme::GOLD,
            WeaponTag::Counter => theme::ENEMY,
        };
    }

    for (value, mut text) in &mut queries.header_values {
        text.0 = match value {
            HeaderValue::Round => format!("{:02}", battle.0.round()),
            HeaderValue::Allies => hud.ally_count.to_string(),
            HeaderValue::Enemies => hud.enemy_count.to_string(),
            HeaderValue::Awaiting => hud.awaiting_count.to_string(),
            HeaderValue::Credits => campaign
                .as_ref()
                .and_then(|runtime| runtime.0.state.as_ref())
                .map_or_else(|| "—".to_owned(), |state| state.credits.to_string()),
        };
    }

    let primary_pip_count = hud.objective_track.as_ref().map_or(0, |track| match track {
        ObjectiveTrackSnapshot::EliminateAll { remaining, .. } => (*remaining).min(4),
        ObjectiveTrackSnapshot::Protect { hp, .. } | ObjectiveTrackSnapshot::Target { hp, .. } => {
            usize::from(*hp > 0)
        }
        ObjectiveTrackSnapshot::Intercept { distance, .. } => usize::from(*distance > 0),
    });
    for (pip, mut background) in &mut queries.primary_pips {
        background.0 = if pip.0 < primary_pip_count {
            theme::ACCENT
        } else {
            theme::BORDER
        };
    }
    for mut background in &mut queries.bonus_dots {
        background.0 = if hud.optional_progress_complete() {
            theme::GOLD
        } else {
            theme::BORDER
        };
    }

    for (mut image, mut visibility) in &mut queries.inspector_portraits {
        let (source, rect, color) = match (hud.inspector.faction, hud.inspector.archetype) {
            (Some(Faction::Player), Some(UnitArchetype::Vanguard)) => {
                (ui_assets.vanguard_art.clone(), None, Color::WHITE)
            }
            (Some(Faction::Player), Some(UnitArchetype::Gunner)) => {
                (ui_assets.gunner_art.clone(), None, Color::WHITE)
            }
            (Some(Faction::Player), Some(UnitArchetype::Interceptor)) => {
                (ui_assets.interceptor_art.clone(), None, Color::WHITE)
            }
            (_, Some(archetype)) => {
                let style = theme::unit_archetype_style(archetype);
                (ui_assets.icons.clone(), Some(style.glyph_rect), style.color)
            }
            _ => (
                ui_assets.icons.clone(),
                Some(theme::UNIT_GLYPH_HEX_RECT),
                theme::MUTED,
            ),
        };
        image.image = source;
        image.rect = rect;
        image.color = color;
        *visibility = if inspector_selected {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    for mut image in &mut queries.result_icons {
        if hud.is_victory {
            image.image = ui_assets.icons.clone();
            image.rect = Some(theme::RESULT_VICTORY_RECT);
            image.color = Color::WHITE;
        } else if hud.is_terminal {
            image.image = ui_assets.icons.clone();
            image.rect = Some(theme::RESULT_DEFEAT_RECT);
            image.color = Color::WHITE;
        } else {
            image.image = ui_assets.icons.clone();
            image.rect = Some(theme::RESULT_PRIMARY_RECT);
            image.color = Color::WHITE;
        }
    }
    for (mut image, metric) in &mut queries.result_metric_icons {
        image.image = ui_assets.icons.clone();
        image.rect = Some(if metric.bonus {
            theme::RESULT_BONUS_RECT
        } else {
            theme::RESULT_PRIMARY_RECT
        });
        image.color = Color::WHITE;
    }
    for (mut background, mut border) in &mut queries.result_cards {
        background.0 = theme::RESULT_CARD_BACKGROUND;
        border.set_all(if hud.is_victory {
            theme::RESULT_VICTORY_BORDER
        } else {
            theme::RESULT_DEFEAT_BORDER
        });
    }
    for mut border in &mut queries.result_rings {
        *border = BorderColor::all(if hud.is_victory {
            theme::MINT
        } else {
            theme::ENEMY
        });
    }
    for mut color in &mut queries.result_headlines {
        color.0 = if hud.is_victory {
            theme::RESULT_VICTORY_TEXT
        } else {
            theme::RESULT_DEFEAT_TEXT
        };
    }

    for mut visibility in &mut queries.result_overlays {
        *visibility = if hud.is_terminal && !playback.input_locked {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    let restart_ok = match (
        asset_status.as_ref(),
        next_state.as_ref(),
        restart_request.as_ref(),
    ) {
        (Some(asset_status), Some(next_state), Some(restart_request)) => restart_allowed(
            &battle.0,
            asset_status,
            &playback,
            next_state,
            restart_request.0.is_some(),
        ),
        _ => hud.is_terminal && !hud.is_victory && !playback.input_locked,
    };
    for (
        button,
        header_restart,
        mut background,
        mut border,
        mut pickable,
        mut visibility,
        mut node,
    ) in &mut queries.button_and_inspector.p0()
    {
        let enabled = !playback.input_locked
            && match button.0 {
                CommandAction::Restart => restart_ok,
                action => command_enabled(action, &hud),
            };
        let armed = match (button.0, interaction.mode) {
            (CommandAction::Move, InteractionMode::Move) => true,
            (CommandAction::PilotSkill, InteractionMode::AegisTarget) => true,
            (CommandAction::WeaponSlot(slot), InteractionMode::Attack(weapon)) => {
                hud.weapon_names
                    .get(slot)
                    .is_some_and(|name| name.is_some())
                    && battle
                        .0
                        .active_unit()
                        .and_then(|unit| battle.0.unit(unit))
                        .is_some_and(|unit| unit.weapons.get(slot).copied() == Some(weapon))
            }
            _ => false,
        };
        let terminal_result_action = header_restart.is_none()
            && matches!(
                button.0,
                CommandAction::Restart | CommandAction::ContinueVictory
            );
        background.0 = if terminal_result_action {
            theme::PANEL_RAISED
        } else if button.0 == CommandAction::Cancel {
            theme::TARGETING_CANCEL_BACKGROUND
        } else if button.0 == CommandAction::ResolveAttacks {
            Color::srgb_u8(63, 42, 6)
        } else if armed {
            Color::srgb(0.82, 0.38, 0.08)
        } else if enabled {
            Color::srgb(0.08, 0.25, 0.34)
        } else {
            Color::srgb(0.055, 0.07, 0.09)
        };
        if button.0 == CommandAction::Cancel {
            *border = BorderColor::all(theme::TARGETING_CANCEL_BORDER);
        } else if terminal_result_action {
            *border = BorderColor::all(theme::ACCENT);
        }
        *pickable = if enabled {
            Pickable::default()
        } else {
            Pickable::IGNORE
        };
        // Terminal actions are result-specific: defeat shows Restart only,
        // victory shows Continue only. The overlay itself is hidden mid-battle.
        if matches!(
            button.0,
            CommandAction::Restart | CommandAction::ContinueVictory
        ) {
            let shown = match button.0 {
                CommandAction::Restart => {
                    if header_restart.is_some() {
                        !hud.is_terminal
                    } else {
                        hud.is_terminal && !hud.is_victory
                    }
                }
                _ => hud.is_victory,
            };
            let shown = shown && !playback.input_locked;
            *visibility = if shown {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
            node.display = if shown { Display::Flex } else { Display::None };
        }
    }
}

pub fn update_asset_status_text(
    status: Res<AssetLoadStatus>,
    panel: Single<(&mut Text, &mut Visibility, &mut TextColor), With<AssetStatusText>>,
) {
    let (mut text, mut visibility, mut color) = panel.into_inner();
    match &*status {
        AssetLoadStatus::Loading => {
            text.0 = "Loading battle UI assets...".to_owned();
            *visibility = Visibility::Visible;
            color.0 = Color::srgb(1.0, 0.78, 0.34);
        }
        AssetLoadStatus::Ready => {
            *visibility = Visibility::Hidden;
        }
        AssetLoadStatus::Failed(path) => {
            text.0 = format!("ASSET LOAD FAILED\n{path}");
            *visibility = Visibility::Visible;
            color.0 = Color::srgb(1.0, 0.36, 0.3);
        }
    }
}

fn spawn_command_button(
    commands: &mut Commands,
    fonts: &theme::FontHandles,
    icons: &Handle<Image>,
    parent: Entity,
    action: CommandAction,
    label: &str,
    _width: f32,
) -> Entity {
    let icon = match action {
        CommandAction::ContinueVictory => theme::ICON_FORWARD_COMPACT,
        CommandAction::Restart => theme::BATTLE_RESTART_RECT,
        _ => theme::ICON_WAIT,
    };
    let button = commands
        .spawn((
            Button,
            CommandButton(action),
            Node {
                width: percent(100),
                height: px(84),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: px(16),
                padding: UiRect::horizontal(px(18)),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(theme::PANEL_RAISED),
            BorderColor::all(theme::ACCENT),
            Pickable::default(),
            ChildOf(parent),
        ))
        .observe(on_command_button_click)
        .id();
    commands.spawn((
        Node {
            width: px(32),
            height: px(32),
            ..default()
        },
        theme::icon_node(icons.clone(), icon, Color::WHITE),
        Pickable::IGNORE,
        ChildOf(button),
    ));
    commands.spawn((
        Text::new(label),
        theme::chakra_petch(fonts, 26.0, FontWeight::NORMAL),
        LetterSpacing::Px(5.2),
        TextColor(theme::TEXT),
        Pickable::IGNORE,
        ChildOf(button),
    ));
    button
}

fn command_enabled(action: CommandAction, hud: &HudSnapshot) -> bool {
    match action {
        CommandAction::Move => hud.can_move,
        CommandAction::WeaponSlot(slot) => hud.weapon_enabled.get(slot).copied().unwrap_or(false),
        CommandAction::PilotSkill => hud.can_pilot,
        CommandAction::Reaction(_) => hud.can_choose_reaction,
        CommandAction::FinishUnit => hud.can_finish,
        CommandAction::ResolveAttacks => hud.can_resolve,
        CommandAction::Restart => hud.is_terminal && !hud.is_victory,
        CommandAction::ContinueVictory => hud.is_victory,
        CommandAction::Cancel => true,
    }
}

/// Result-overlay copy, derived from the active mission's authored data — no
/// mission-specific wording is hardcoded here.
fn result_primary_progress(track: Option<&ObjectiveTrackSnapshot>) -> String {
    match track {
        Some(ObjectiveTrackSnapshot::EliminateAll { remaining, total }) => {
            format!("{}/{}", total.saturating_sub(*remaining), total)
        }
        Some(ObjectiveTrackSnapshot::Protect { hp, max_hp, .. })
        | Some(ObjectiveTrackSnapshot::Target { hp, max_hp, .. }) => {
            format!("HP {hp}/{max_hp}")
        }
        Some(ObjectiveTrackSnapshot::Intercept { distance, .. }) => {
            format!("{distance} FROM EXIT")
        }
        None => "—".to_owned(),
    }
}

pub fn result_overlay_copy(
    result: MissionResult,
    primary: PrimaryObjective,
    _definition: &MissionDefinition,
) -> String {
    if result.victory {
        "RELAY SECURED".to_owned()
    } else {
        let reason = match primary {
            PrimaryObjective::EliminateAllEnemies | PrimaryObjective::EliminateTarget { .. } => {
                "Squad knocked out"
            }
            PrimaryObjective::ProtectThroughRound { .. } => "Protect target lost",
            PrimaryObjective::InterceptBeforeEscape { .. } => "Courier not stopped in time",
        };
        format!("MISSION FAILED\n{reason}")
    }
}

/// Format a playback event for the recent battle log.
pub fn format_event(event: &BattleEvent, battle: &BattleState) -> String {
    match event {
        BattleEvent::UnitMoved { unit, .. } => battle.unit(*unit).map_or_else(
            || "UNIT MOVING".to_owned(),
            |unit| format!("{} MOVING", unit.name),
        ),
        BattleEvent::AttackRolled {
            attacker,
            target,
            hit,
            critical,
            ..
        } => {
            let attacker = unit_name(battle, *attacker);
            let target = unit_name(battle, *target);
            let outcome = if *critical {
                "CRITICAL"
            } else if *hit {
                "HIT"
            } else {
                "MISS"
            };
            format!("{attacker} -> {target}\n{outcome}")
        }
        BattleEvent::DamageApplied { target, amount, .. } => {
            format!("{}\n-{amount} HP", unit_name(battle, *target))
        }
        BattleEvent::UnitKnockedOut { unit, .. } => {
            format!("{} KNOCKED OUT", unit_name(battle, *unit))
        }
        BattleEvent::UnitPushed { unit, .. } => {
            format!("{} PUSHED", unit_name(battle, *unit))
        }
        BattleEvent::CollisionOccurred { .. } => "COLLISION  -3 HP".to_owned(),
        BattleEvent::HazardTriggered { .. } => "HAZARD  -3 HP".to_owned(),
        BattleEvent::ExplosiveDamaged { amount, .. } => format!("EXPLOSIVE  -{amount} HP"),
        BattleEvent::ExplosionTriggered { .. } => "EXPLOSION".to_owned(),
        BattleEvent::IntentCommitted { attacker, .. } => {
            format!("{} LOCKED INTENT", unit_name(battle, *attacker))
        }
        BattleEvent::IntentCanceled { attacker } => {
            format!("{} INTENT CANCELED", unit_name(battle, *attacker))
        }
        BattleEvent::AttackHitEmpty { .. } => "ATTACK HIT EMPTY".to_owned(),
        BattleEvent::CounterFired { defender, .. } => {
            format!("{} COUNTER", unit_name(battle, *defender))
        }
        BattleEvent::OptionalObjectiveCompleted => "BONUS ACHIEVED".to_owned(),
        BattleEvent::MissionCompleted { .. } => "MISSION COMPLETE".to_owned(),
        BattleEvent::MissionFailed { .. } => "MISSION FAILED".to_owned(),
    }
}

fn unit_name(battle: &BattleState, unit: UnitId) -> &'static str {
    battle.unit(unit).map_or("UNKNOWN", |unit| unit.name)
}

fn ascii_separators(value: &str) -> String {
    value.replace('·', "/")
}

fn format_track(track: &ObjectiveTrackSnapshot) -> String {
    match track {
        ObjectiveTrackSnapshot::EliminateAll { remaining, total } => {
            format!("{remaining}/{total} ENEMIES REMAIN")
        }
        ObjectiveTrackSnapshot::Protect {
            name, hp, max_hp, ..
        } => format!("{name} HP {hp}/{max_hp}"),
        ObjectiveTrackSnapshot::Intercept { name, distance, .. } => {
            format!("{name} {distance} FROM EXIT")
        }
        ObjectiveTrackSnapshot::Target {
            name, hp, max_hp, ..
        } => {
            format!("TARGET {name} HP {hp}/{max_hp}")
        }
    }
}

fn format_signed(value: i16) -> String {
    if value >= 0 {
        format!("+{value}")
    } else {
        value.to_string()
    }
}

fn log_dot_color(entry: &str) -> Color {
    if entry.contains("MISS") || entry.contains("KNOCKED") || entry.contains("FAILED") {
        theme::ENEMY
    } else if entry.contains("HIT") || entry.contains("HP") || entry.contains("COLLISION") {
        theme::GOLD
    } else {
        theme::MINT
    }
}

const fn shape_cell_active(shape: WeaponShape, index: usize) -> bool {
    match shape {
        WeaponShape::Single => index == 4,
        WeaponShape::Cross1 => matches!(index, 1 | 3 | 4 | 5 | 7),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::combat::DamageSource;
    use crate::domain::model::{MissionRules, OptionalObjective};
    use crate::mission::mission_one::{ids, mission_one};
    use crate::mission::mission_three::mission_three;
    use crate::mission::mission_two::mission_two;
    use crate::mission::{MissionId, mission_definition};
    use crate::presentation::assets::UiAssets;

    fn test_ui_assets() -> UiAssets {
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

    fn terminal_battle(victory: bool) -> BattleState {
        let mut battle = mission_one(7);
        let (casualties, source) = if victory {
            (
                &[
                    ids::RIFLEMAN_LEFT,
                    ids::RIFLEMAN_RIGHT,
                    ids::STRIKER,
                    ids::ARTILLERY,
                ][..],
                DamageSource::PlayerWeapon(ids::PILE_LANCE),
            )
        } else {
            (
                &[ids::VANGUARD, ids::GUNNER, ids::INTERCEPTOR][..],
                DamageSource::Hazard,
            )
        };
        for unit in casualties {
            battle.apply_direct_damage(*unit, 99, source);
        }
        battle
    }

    fn run_terminal_hud(victory: bool) -> App {
        let mut app = App::new();
        app.insert_resource(BattleRuntime(terminal_battle(victory)))
            .insert_resource(ActiveMission(mission_definition(MissionId::One).unwrap()))
            .insert_resource(test_ui_assets())
            .init_resource::<InteractionState>()
            .init_resource::<StatusMessage>()
            .init_resource::<EventPlayback>()
            .add_systems(Update, (setup_mission_ui, update_hud).chain());
        app.update();
        app
    }

    /// Returns `(visibility, is_hoverable)` for the two result-overlay buttons.
    /// Panics if either button lacks `Pickable` — that is the regression:
    /// without `Pickable` on the spawned buttons, `update_hud`'s button loop
    /// matches nothing and terminal visibility/enablement never applies.
    fn terminal_button_states(app: &mut App) -> [(Visibility, bool); 2] {
        let mut buttons = app
            .world_mut()
            .query::<(&CommandButton, &Visibility, &Pickable)>();
        let mut find = |app: &mut App, action: CommandAction| -> (Visibility, bool) {
            buttons
                .iter(app.world())
                .find(|(button, _, _)| button.0 == action)
                .map_or_else(
                    || panic!("{action:?} button not found with Pickable"),
                    |(_, visibility, pickable)| (*visibility, pickable.is_hoverable),
                )
        };
        [
            find(app, CommandAction::Restart),
            find(app, CommandAction::ContinueVictory),
        ]
    }

    #[test]
    fn objective_panel_renders_round_cap_and_protect_tracker() {
        let mut battle = mission_two(7);
        battle.begin_round().unwrap();
        let mut app = App::new();
        app.insert_resource(BattleRuntime(battle))
            .insert_resource(ActiveMission(mission_definition(MissionId::Two).unwrap()))
            .insert_resource(test_ui_assets())
            .init_resource::<InteractionState>()
            .init_resource::<StatusMessage>()
            .init_resource::<EventPlayback>()
            .add_systems(Update, (setup_mission_ui, update_hud).chain());
        app.update();

        let mut texts = app.world_mut().query::<(&HudTextRole, &Text)>();
        let (_, text) = texts
            .iter(app.world())
            .find(|(role, _)| **role == HudTextRole::Objective)
            .expect("objective panel must exist");
        assert!(text.0.contains("Round 1/3"), "objective text: {}", text.0);
        assert!(
            text.0.contains("TRACK    Gunner HP 15/15"),
            "objective text: {}",
            text.0
        );
    }

    #[test]
    fn target_objective_tracks_target_without_enemy_count_and_formats_as_target() {
        let mut battle = mission_one(7);
        battle.set_rules_for_test(MissionRules {
            primary: PrimaryObjective::EliminateTarget {
                target: ids::STRIKER,
            },
            optional: OptionalObjective::Turnabout,
            opening_plan: &[],
        });
        let base = *mission_definition(MissionId::One).unwrap();
        let definition = MissionDefinition {
            primary_objective: "Destroy the Striker.",
            ..base
        };

        let hud = HudSnapshot::from_battle(&battle, None, &definition);
        assert_eq!(
            hud.objective_track,
            Some(ObjectiveTrackSnapshot::Target {
                name: "Striker",
                hp: 12,
                max_hp: 12,
                position: GridPos::new(4, 4),
            })
        );
        assert_eq!(hud.primary, "Destroy the Striker.");
        assert_eq!(
            format_track(hud.objective_track.as_ref().unwrap()),
            "TARGET Striker HP 12/12"
        );
    }

    #[test]
    fn elimination_objective_keeps_the_remaining_enemy_count() {
        let battle = mission_one(7);
        let hud =
            HudSnapshot::from_battle(&battle, None, mission_definition(MissionId::One).unwrap());

        assert!(
            hud.primary.contains("remaining"),
            "primary: {}",
            hud.primary
        );
        assert_eq!(
            hud.objective_track,
            Some(ObjectiveTrackSnapshot::EliminateAll {
                remaining: 4,
                total: 4,
            })
        );
    }

    #[test]
    fn header_enemy_count_tracks_living_enemies_but_objective_total_stays_authored() {
        let mut battle = mission_one(7);
        for id in [ids::RIFLEMAN_LEFT, ids::RIFLEMAN_RIGHT, ids::STRIKER] {
            battle.apply_direct_damage(id, 99, DamageSource::PlayerWeapon(ids::PILE_LANCE));
        }

        let hud =
            HudSnapshot::from_battle(&battle, None, mission_definition(MissionId::One).unwrap());

        assert_eq!(hud.enemy_count, 1);
        assert_eq!(
            hud.objective_track,
            Some(ObjectiveTrackSnapshot::EliminateAll {
                remaining: 1,
                total: 4,
            })
        );
    }

    #[test]
    fn mission_two_and_three_primaries_lose_the_remaining_enemy_count() {
        let m2 = mission_two(7);
        let m2_hud =
            HudSnapshot::from_battle(&m2, None, mission_definition(MissionId::Two).unwrap());
        assert!(
            !m2_hud.primary.contains("remaining"),
            "m2: {}",
            m2_hud.primary
        );

        let m3 = mission_three(7);
        let m3_hud =
            HudSnapshot::from_battle(&m3, None, mission_definition(MissionId::Three).unwrap());
        assert!(
            !m3_hud.primary.contains("remaining"),
            "m3: {}",
            m3_hud.primary
        );
    }

    #[test]
    fn ready_asset_status_hides_a_newly_spawned_battle_panel() {
        let mut app = App::new();
        app.insert_resource(AssetLoadStatus::Ready)
            .add_systems(Update, update_asset_status_text);
        app.world_mut().spawn((
            Text::new("Loading battle UI assets..."),
            Visibility::Visible,
            TextColor(theme::GOLD),
            AssetStatusText,
        ));

        app.update();

        let visibility = app
            .world_mut()
            .query_filtered::<&Visibility, With<AssetStatusText>>()
            .single(app.world())
            .expect("asset status panel exists");
        assert_eq!(*visibility, Visibility::Hidden);
    }

    #[test]
    fn defeat_shows_restart_visible_enabled_and_hides_continue() {
        let mut app = run_terminal_hud(false);
        let [restart, cont] = terminal_button_states(&mut app);
        assert_eq!(restart.0, Visibility::Visible);
        assert!(restart.1, "Restart must stay clickable on defeat");
        assert_eq!(cont.0, Visibility::Hidden);
        assert!(!cont.1, "Continue must be unhoverable on defeat");
    }

    #[test]
    fn victory_shows_continue_visible_enabled_and_hides_restart() {
        let mut app = run_terminal_hud(true);
        let [restart, cont] = terminal_button_states(&mut app);
        assert_eq!(cont.0, Visibility::Visible);
        assert!(cont.1, "Continue must be clickable on victory");
        assert_eq!(restart.0, Visibility::Hidden);
        assert!(!restart.1, "Restart must be unhoverable on victory");
    }
}
