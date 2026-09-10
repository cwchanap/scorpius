use bevy::{ecs::system::SystemParam, prelude::*};

use crate::app::GameScreen;
use crate::domain::{
    battle::BattleState,
    board::GridPos,
    combat::AttackPreview,
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
        WeaponMeter, WeaponMeterKind, WeaponTag, WeaponTagIcon, WeaponText, WeaponTextKind,
        spawn_battle_menu,
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
pub struct StatusText;

#[derive(Component)]
pub struct PlaybackText;

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

#[derive(Component, Clone, Copy)]
pub(crate) enum HeaderValue {
    Round,
    Phase,
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
    Playback,
    Result,
    ResultPrimary,
    ResultBonus,
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
    commands.spawn((
        Text::new("01"),
        theme::ibm_plex_mono(&ui_assets.fonts, 34.0, FontWeight(600)),
        TextColor(theme::ACCENT),
        HeaderValue::Round,
        Pickable::IGNORE,
        ChildOf(header),
    ));
    commands.spawn((
        Text::new("PLAYER PHASE"),
        theme::ibm_plex_mono(&ui_assets.fonts, 13.0, FontWeight(500)),
        TextColor(theme::MUTED),
        HeaderValue::Phase,
        Pickable::IGNORE,
        ChildOf(header),
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
    spawn_header_metric(
        &mut commands,
        header,
        &ui_assets,
        theme::ICON_FORWARD_COMPACT,
        theme::ACCENT,
        HeaderValue::Allies,
        "0",
    );
    spawn_header_metric(
        &mut commands,
        header,
        &ui_assets,
        theme::ICON_ATTACK,
        theme::ENEMY,
        HeaderValue::Enemies,
        "0",
    );
    spawn_header_metric(
        &mut commands,
        header,
        &ui_assets,
        theme::ICON_WAIT,
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
        theme::icon_node(ui_assets.icons.clone(), theme::ICON_FORWARD, theme::MUTED),
        Node {
            width: px(24),
            height: px(24),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(header),
    ));
    commands.spawn((
        Text::new("SCORPIUS // COMBAT LINK"),
        theme::ibm_plex_mono(&ui_assets.fonts, 13.0, FontWeight(500)),
        TextColor(theme::MUTED),
        Node {
            margin: UiRect::left(Val::Auto),
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
        theme::icon_node(ui_assets.icons.clone(), theme::ICON_ATTACK, theme::ACCENT),
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
                column_gap: px(4),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(primary),
        ))
        .id();
    for index in 0..4 {
        commands.spawn((
            Node {
                width: px(14),
                height: px(6),
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
            width: px(8),
            height: px(8),
            ..default()
        },
        BackgroundColor(theme::GOLD),
        HeaderBonusDot,
        Pickable::IGNORE,
        ChildOf(primary),
    ));
    spawn_header_metric(
        &mut commands,
        header,
        &ui_assets,
        theme::ICON_GUARD,
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
                margin: UiRect::left(px(8)),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(14, 26, 38)),
            Pickable::default(),
            ChildOf(header),
        ))
        .observe(on_command_button_click)
        .with_children(|parent| {
            parent.spawn((
                theme::icon_node(ui_assets.icons.clone(), theme::icon_rect(5), theme::MUTED),
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
                height: px(190),
                flex_shrink: 0.0,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(16)),
                ..default()
            },
            BackgroundColor(theme::PANEL),
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
                column_gap: px(8),
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
        theme::chakra_petch(&ui_assets.fonts, 18.0, FontWeight(600)),
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
                column_gap: px(8),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(inspector_details),
        ))
        .id();
    commands.spawn((
        theme::icon_node(
            ui_assets.icons.clone(),
            theme::ICON_FORWARD_COMPACT,
            theme::MINT,
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
                height: px(10),
                flex_grow: 1.0,
                ..default()
            },
            BackgroundColor(theme::BORDER),
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
        theme::ibm_plex_mono(&ui_assets.fonts, 15.0, FontWeight(600)),
        TextColor(theme::TEXT),
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
                column_gap: px(8),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(inspector_details),
        ))
        .id();
    commands.spawn((
        theme::icon_node(ui_assets.icons.clone(), theme::ICON_SKILL, theme::GOLD),
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
                width: px(7),
                height: px(5),
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
        theme::ibm_plex_mono(&ui_assets.fonts, 13.0, FontWeight(600)),
        TextColor(theme::GOLD),
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
        theme::ICON_GUARD,
        InspectorTextKind::Armor,
        "—",
    );
    spawn_inspector_stat(
        &mut commands,
        inspector_stats,
        &ui_assets,
        theme::ICON_MOVE,
        InspectorTextKind::Movement,
        "—",
    );
    spawn_inspector_stat(
        &mut commands,
        inspector_stats,
        &ui_assets,
        theme::ICON_EVADE,
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
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(12),
                ..default()
            },
            Visibility::Visible,
            Pickable::IGNORE,
            ChildOf(inspector),
        ))
        .id();
    commands.spawn((
        theme::icon_node(
            ui_assets.icons.clone(),
            theme::UNIT_GLYPH_HEX_RECT,
            theme::MUTED,
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
    commands.spawn((
        Text::new("SELECT A MECH"),
        theme::ibm_plex_mono(&ui_assets.fonts, 12.0, FontWeight(500)),
        TextColor(theme::MUTED),
        Pickable::IGNORE,
        ChildOf(empty),
    ));
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
            Pickable::IGNORE,
            ChildOf(sidebar),
        ))
        .id();
    commands.spawn((
        Text::new("LOG"),
        theme::ibm_plex_mono(&ui_assets.fonts, 13.0, FontWeight(500)),
        TextColor(theme::MUTED),
        Pickable::IGNORE,
        ChildOf(log_panel),
    ));
    commands.spawn((
        Text::new(""),
        theme::ibm_plex_mono(&ui_assets.fonts, 14.0, FontWeight(400)),
        TextColor(theme::MUTED),
        Node {
            min_height: px(0),
            overflow: Overflow::clip(),
            ..default()
        },
        PlaybackText,
        HudTextRole::Playback,
        Pickable::IGNORE,
        ChildOf(log_panel),
    ));
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
            Pickable::IGNORE,
            ChildOf(rightbar),
        ))
        .id();
    commands.spawn((
        theme::icon_node(ui_assets.icons.clone(), theme::ICON_ATTACK, theme::ENEMY),
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
        theme::ibm_plex_mono(&ui_assets.fonts, 14.0, FontWeight(500)),
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
    commands.spawn((
        Text::new("TARGET PREVIEW\nArm a weapon and hover a target."),
        theme::ibm_plex_mono(&ui_assets.fonts, 13.0, FontWeight(500)),
        TextColor(theme::GOLD),
        Node {
            width: percent(100),
            min_height: px(106),
            padding: UiRect::all(px(16)),
            ..default()
        },
        BackgroundColor(theme::PANEL),
        PreviewText,
        HudTextRole::Preview,
        Pickable::IGNORE,
        ChildOf(rightbar),
    ));
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
                width: px(690),
                padding: UiRect::all(px(28)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(18),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme::PANEL),
            Pickable::IGNORE,
            ChildOf(result_overlay),
        ))
        .id();
    commands.spawn((
        theme::icon_node(ui_assets.icons.clone(), theme::ICON_WAIT, theme::MINT),
        Node {
            width: px(86),
            height: px(86),
            ..default()
        },
        ResultIcon,
        Pickable::IGNORE,
        ChildOf(result_card),
    ));
    commands.spawn((
        Text::new(""),
        theme::chakra_petch(&ui_assets.fonts, 28.0, FontWeight(600)),
        TextColor(theme::TEXT),
        HudTextRole::Result,
        Pickable::IGNORE,
        ChildOf(result_card),
    ));
    let result_metrics = commands
        .spawn((
            Node {
                width: percent(100),
                display: Display::Flex,
                column_gap: px(14),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(result_card),
        ))
        .id();
    for (label, role, color) in [
        ("PRIMARY", HudTextRole::ResultPrimary, theme::ACCENT),
        ("BONUS", HudTextRole::ResultBonus, theme::GOLD),
    ] {
        let metric = commands
            .spawn((
                Node {
                    width: percent(50),
                    padding: UiRect::all(px(16)),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(8),
                    ..default()
                },
                BackgroundColor(Color::srgb_u8(11, 20, 32)),
                Pickable::IGNORE,
                ChildOf(result_metrics),
            ))
            .id();
        commands.spawn((
            Text::new(label),
            theme::ibm_plex_mono(&ui_assets.fonts, 13.0, FontWeight(500)),
            TextColor(color),
            Pickable::IGNORE,
            ChildOf(metric),
        ));
        commands.spawn((
            Text::new("—"),
            theme::ibm_plex_mono(&ui_assets.fonts, 25.0, FontWeight(600)),
            TextColor(theme::TEXT),
            role,
            Pickable::IGNORE,
            ChildOf(metric),
        ));
    }
    spawn_command_button(
        &mut commands,
        &ui_assets.fonts,
        result_card,
        CommandAction::Restart,
        "RESTART MISSION",
        260.0,
    );
    spawn_command_button(
        &mut commands,
        &ui_assets.fonts,
        result_card,
        CommandAction::ContinueVictory,
        "CONTINUE",
        260.0,
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
        theme::icon_node(assets.icons.clone(), icon, color),
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
                column_gap: px(5),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        theme::icon_node(assets.icons.clone(), icon, theme::MUTED),
        Node {
            width: px(16),
            height: px(16),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(stat),
    ));
    commands.spawn((
        Text::new(initial),
        theme::ibm_plex_mono(&assets.fonts, 12.0, FontWeight(600)),
        TextColor(theme::TEXT),
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
                min_height: px(148),
                flex_shrink: 0.0,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                padding: UiRect::all(px(14)),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(22, 13, 13)),
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
                column_gap: px(7),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(card_entity),
        ))
        .id();
    for target in [false, true] {
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
            ThreatIcon { card, target },
            Pickable::IGNORE,
            ChildOf(top),
        ));
        commands.spawn((
            Text::new("—"),
            theme::chakra_petch(&assets.fonts, 13.0, FontWeight(600)),
            TextColor(theme::TEXT),
            ThreatText {
                card,
                kind: if target {
                    ThreatTextKind::Target
                } else {
                    ThreatTextKind::Attacker
                },
            },
            Pickable::IGNORE,
            ChildOf(top),
        ));
        if !target {
            commands.spawn((
                Text::new("→"),
                theme::ibm_plex_mono(&assets.fonts, 16.0, FontWeight(500)),
                TextColor(theme::ENEMY),
                Pickable::IGNORE,
                ChildOf(top),
            ));
        }
    }
    commands.spawn((
        Text::new("—"),
        theme::ibm_plex_mono(&assets.fonts, 11.0, FontWeight(500)),
        TextColor(theme::MUTED),
        ThreatText {
            card,
            kind: ThreatTextKind::Weapon,
        },
        Node {
            margin: UiRect::left(px(4)),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(card_entity),
    ));

    let details = commands
        .spawn((
            Node {
                width: percent(100),
                display: Display::Flex,
                align_items: AlignItems::FlexEnd,
                column_gap: px(12),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(card_entity),
        ))
        .id();
    let shape = commands
        .spawn((
            Node {
                width: px(54),
                height: px(54),
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
                    height: px(16),
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
                    width: px(16),
                    height: px(16),
                    ..default()
                },
                BackgroundColor(theme::BORDER),
                ThreatMeter(ThreatMeterKind::Shape { card, index }),
                Pickable::IGNORE,
                ChildOf(shape_row),
            ));
        }
    }
    let numbers = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: px(0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: px(7),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(details),
        ))
        .id();
    commands.spawn((
        Text::new("— DMG"),
        theme::chakra_petch(&assets.fonts, 18.0, FontWeight(600)),
        TextColor(theme::ENEMY),
        ThreatText {
            card,
            kind: ThreatTextKind::Damage,
        },
        Pickable::IGNORE,
        ChildOf(numbers),
    ));
    let hit = commands
        .spawn((
            Node {
                width: percent(100),
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(6),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(numbers),
        ))
        .id();
    commands.spawn((
        Text::new("—"),
        theme::ibm_plex_mono(&assets.fonts, 12.0, FontWeight(600)),
        TextColor(theme::TEXT),
        ThreatText {
            card,
            kind: ThreatTextKind::Hit,
        },
        Pickable::IGNORE,
        ChildOf(hit),
    ));
    let hit_track = commands
        .spawn((
            Node {
                position_type: PositionType::Relative,
                width: percent(100),
                height: px(6),
                flex_grow: 1.0,
                ..default()
            },
            BackgroundColor(theme::BORDER),
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
        ),
        (
            Or<(With<InspectorText>, With<WeaponText>, With<ThreatText>)>,
            Without<HudTextRole>,
            Without<HeaderValue>,
        ),
    >,
    buttons: Query<
        'w,
        's,
        (
            &'static CommandButton,
            Option<&'static HeaderRestart>,
            &'static mut BackgroundColor,
            &'static mut Pickable,
            &'static mut Visibility,
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
            Without<ThreatCard>,
            Without<WeaponTagIcon>,
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
        ),
    >,
    header_values: Query<'w, 's, (&'static HeaderValue, &'static mut Text), Without<HudTextRole>>,
    inspector_portraits: Query<
        'w,
        's,
        (&'static mut ImageNode, &'static mut Visibility),
        (
            With<InspectorPortrait>,
            Without<ResultOverlay>,
            Without<CommandButton>,
            Without<ResultIcon>,
            Without<WeaponTagIcon>,
            Without<HudTextRole>,
            Without<ThreatCard>,
        ),
    >,
    primary_pips: Query<
        'w,
        's,
        (&'static HeaderPrimaryPip, &'static mut BackgroundColor),
        (Without<HeaderBonusDot>, Without<CommandButton>),
    >,
    bonus_dots: Query<
        'w,
        's,
        &'static mut BackgroundColor,
        (
            With<HeaderBonusDot>,
            Without<HeaderPrimaryPip>,
            Without<CommandButton>,
        ),
    >,
    inspector_visibility: Query<
        'w,
        's,
        (
            &'static mut Visibility,
            &'static mut Node,
            Option<&'static InspectorTop>,
            Option<&'static InspectorStats>,
            Option<&'static InspectorEmpty>,
        ),
        (
            Or<(
                With<InspectorTop>,
                With<InspectorStats>,
                With<InspectorEmpty>,
            )>,
            Without<CommandButton>,
            Without<ResultOverlay>,
            Without<InspectorMeter>,
            Without<WeaponMeter>,
            Without<ThreatMeter>,
            Without<ThreatCard>,
            Without<InspectorPortrait>,
            Without<WeaponTagIcon>,
            Without<HudTextRole>,
        ),
    >,
    inspector_icons: Query<
        'w,
        's,
        (&'static mut ImageNode, &'static InspectorIcon),
        (
            Without<InspectorPortrait>,
            Without<ResultIcon>,
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
        ),
    >,
    threat_icons: Query<
        'w,
        's,
        (&'static mut ImageNode, &'static ThreatIcon),
        (
            Without<InspectorPortrait>,
            Without<ResultIcon>,
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
            Without<CommandButton>,
            Without<HudTextRole>,
            Without<ThreatCard>,
            Without<ResultOverlay>,
            Without<HeaderPrimaryPip>,
            Without<HeaderBonusDot>,
        ),
    >,
    result_icons:
        Query<'w, 's, &'static mut ImageNode, (With<ResultIcon>, Without<InspectorPortrait>)>,
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
    let preview_text = interaction.preview.as_ref().map_or_else(
        || "TARGET PREVIEW\nArm a weapon and hover a target.".to_owned(),
        |preview| format_preview(&battle.0, preview),
    );
    let status_text = if playback.input_locked {
        "Resolving committed events...".to_owned()
    } else if status.0.is_empty() {
        "[M] MOVE  [1-3] WEAPONS  [P] PILOT  [C/G/E] STANCE  [F] FINISH  [SPACE] RESOLVE  [ESC] CANCEL".to_owned()
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
            HudTextRole::Preview => preview_text.clone(),
            HudTextRole::Status => status_text.clone(),
            HudTextRole::Playback => recent_log.as_deref().map_or_else(String::new, |log| {
                log.0.iter().cloned().collect::<Vec<_>>().join("\n")
            }),
            HudTextRole::Result => battle.0.result().map_or_else(String::new, |result| {
                result_overlay_copy(result, battle.0.rules().primary, active_mission.0)
            }),
            HudTextRole::ResultPrimary => battle.0.result().map_or_else(
                || "—".to_owned(),
                |result| if result.victory { "CLEAR" } else { "FAILED" }.to_owned(),
            ),
            HudTextRole::ResultBonus => battle.0.result().map_or_else(
                || "—".to_owned(),
                |result| {
                    if result.optional_complete {
                        "ACHIEVED"
                    } else {
                        "MISSED"
                    }
                    .to_owned()
                },
            ),
        };
        if matches!(role, HudTextRole::Playback)
            && let Some(mut visibility) = visibility
        {
            let has_log = recent_log.as_deref().is_some_and(|log| !log.0.is_empty());
            *visibility = if playback.current.is_some() || has_log {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }

    for (mut text, inspector_text, weapon_text, threat_text) in &mut queries.native_texts {
        if let Some(inspector_text) = inspector_text {
            text.0 = match inspector_text.0 {
                InspectorTextKind::Name => hud.inspector.name.unwrap_or("—").to_owned(),
                InspectorTextKind::Hp => match (hud.inspector.hp, hud.inspector.max_hp) {
                    (Some(hp), Some(max_hp)) => format!("HP {hp}/{max_hp}"),
                    _ => "HP —".to_owned(),
                },
                InspectorTextKind::En => match (hud.inspector.en, hud.inspector.max_en) {
                    (Some(en), Some(max_en)) => format!("EN {en}/{max_en}"),
                    _ => "EN —".to_owned(),
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
                (Some(weapon), WeaponTextKind::Damage) => {
                    format!("{} DMG", weapon.base_damage)
                }
                (Some(weapon), WeaponTextKind::Hit) => {
                    let accuracy = battle
                        .0
                        .active_unit()
                        .and_then(|unit| battle.0.unit(unit))
                        .map_or(weapon.hit_modifier, |unit| {
                            (unit.stats.accuracy + weapon.hit_modifier).clamp(5, 95)
                        });
                    format!("{accuracy}% HIT")
                }
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
                (Some(threat), ThreatTextKind::Damage) => {
                    format!("{} DMG", threat.normal_damage)
                }
                (Some(threat), ThreatTextKind::Hit) => {
                    format!("{}% HIT", threat.hit_chance)
                }
                _ => "—".to_owned(),
            };
        }
    }

    let inspector_selected = !hud.inspector.is_empty();
    for (mut visibility, mut node, top, stats, empty) in &mut queries.inspector_visibility {
        let shown = if top.is_some() || stats.is_some() {
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
                    theme::GOLD
                } else {
                    theme::BORDER
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
            HeaderValue::Phase => phase_label(battle.0.phase()).to_owned(),
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
            image.rect = Some(theme::ICON_WAIT);
            image.color = theme::MINT;
        } else if hud.is_terminal {
            image.image = ui_assets.icons.clone();
            image.rect = Some(theme::ICON_COUNTER);
            image.color = theme::ENEMY;
        } else {
            image.image = ui_assets.icons.clone();
            image.rect = Some(theme::ICON_ATTACK);
            image.color = theme::GOLD;
        }
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
    for (button, header_restart, mut background, mut pickable, mut visibility) in
        &mut queries.buttons
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
        background.0 = if button.0 == CommandAction::ResolveAttacks {
            Color::srgb_u8(63, 42, 6)
        } else if armed {
            Color::srgb(0.82, 0.38, 0.08)
        } else if enabled {
            Color::srgb(0.08, 0.25, 0.34)
        } else {
            Color::srgb(0.055, 0.07, 0.09)
        };
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
    parent: Entity,
    action: CommandAction,
    label: &str,
    width: f32,
) -> Entity {
    let button = commands
        .spawn((
            Button,
            CommandButton(action),
            Node {
                width: px(width),
                height: px(42),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(5), px(3)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.055, 0.07, 0.09)),
            Pickable::default(),
            ChildOf(parent),
        ))
        .observe(on_command_button_click)
        .id();
    commands
        .spawn((
            Text::new(label),
            theme::chakra_petch(fonts, 11.5, FontWeight::NORMAL),
            TextColor(Color::srgb(0.88, 0.94, 1.0)),
            Pickable::IGNORE,
            ChildOf(button),
        ))
        .id()
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
pub fn result_overlay_copy(
    result: MissionResult,
    primary: PrimaryObjective,
    definition: &MissionDefinition,
) -> String {
    if result.victory {
        format!(
            "MISSION COMPLETE\n{}\nBONUS {}",
            definition.title,
            if result.optional_complete {
                "Achieved"
            } else {
                "Missed"
            }
        )
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

pub(crate) fn format_event(event: &BattleEvent, battle: &BattleState) -> String {
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

fn format_preview(battle: &BattleState, preview: &AttackPreview) -> String {
    let target = battle
        .occupant_at(preview.target)
        .and_then(|unit| battle.unit(unit))
        .map(|unit| unit.name.to_owned())
        .or_else(|| {
            battle
                .board()
                .has_live_explosive(preview.target)
                .then(|| "EXPLOSIVE".to_owned())
        })
        .unwrap_or_else(|| format!("CELL {},{}", preview.target.x, preview.target.y));
    let footprint = preview
        .footprint
        .iter()
        .map(|cell| format!("{},{}", cell.x, cell.y))
        .collect::<Vec<_>>()
        .join(" ");
    let push = battle
        .weapon(preview.weapon)
        .filter(|weapon| weapon.push)
        .map_or_else(String::new, |_| match preview.push_destination {
            None => " | PUSH: EDGE COLLISION".to_owned(),
            Some(destination)
                if battle.board().is_blocking(destination)
                    || battle.board().has_live_explosive(destination)
                    || battle.occupant_at(destination).is_some() =>
            {
                " | PUSH: COLLISION 3".to_owned()
            }
            Some(destination) if battle.board().is_hazard(destination) => {
                " | PUSH: HAZARD 3".to_owned()
            }
            Some(destination) => format!(" | PUSH -> {},{}", destination.x, destination.y),
        });
    format!(
        "TARGET {} | {}% HIT | {} / {} CRIT DMG | EN {}{}\nCELLS [{}]",
        target,
        preview.hit_chance,
        preview.normal_damage,
        preview.critical_damage,
        preview.en_cost,
        push,
        footprint
    )
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
