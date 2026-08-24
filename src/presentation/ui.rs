use bevy::prelude::*;

use crate::domain::{
    battle::BattleState,
    combat::AttackPreview,
    model::{BattlePhase, Faction, Reaction, UnitId},
};

use super::{
    BattleRuntime,
    assets::{AssetLoadStatus, MISSION_ONE_GLTF_DISPLAY_PATH},
    interaction::{
        CommandAction, CommandButton, InteractionMode, InteractionState, StatusMessage,
        on_command_button_click,
    },
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThreatSnapshot {
    pub attacker: &'static str,
    pub weapon: &'static str,
    pub cells: String,
    pub intended_occupant: Option<&'static str>,
    pub normal_damage: i16,
    pub hit_chance: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HudSnapshot {
    pub round_phase: String,
    pub primary: String,
    pub optional: String,
    pub selected_name: Option<&'static str>,
    pub selected_summary: String,
    pub threats: Vec<ThreatSnapshot>,
    pub weapon_names: [Option<&'static str>; 3],
    pub weapon_enabled: [bool; 3],
    pub can_move: bool,
    pub can_choose_reaction: bool,
    pub can_finish: bool,
    pub can_resolve: bool,
}

impl HudSnapshot {
    pub fn from_battle(battle: &BattleState, selected: Option<UnitId>) -> Self {
        let remaining = battle
            .units()
            .filter(|unit| unit.faction == Faction::Enemy && !unit.is_knocked_out())
            .count();
        let selected_unit = selected.and_then(|unit| battle.unit(unit));
        let active = selected
            .filter(|unit| battle.active_unit() == Some(*unit))
            .and_then(|unit| battle.unit(unit))
            .filter(|unit| battle.phase() == BattlePhase::Player && !unit.is_knocked_out());
        let mut weapon_names = [None; 3];
        let mut weapon_enabled = [false; 3];
        if let Some(unit) = selected_unit {
            for (slot, weapon_id) in unit.weapons.iter().take(3).enumerate() {
                if let Some(weapon) = battle.weapon(*weapon_id) {
                    weapon_names[slot] = Some(weapon.name);
                    weapon_enabled[slot] = active.is_some_and(|active| {
                        !active.activation.acted && active.en >= weapon.en_cost
                    });
                }
            }
        }
        let selected_summary = selected_unit.map_or_else(
            || "NO MECH SELECTED\nChoose a player unit on the board.".to_owned(),
            |unit| {
                let move_state = if unit.activation.moved {
                    "SPENT"
                } else {
                    "READY"
                };
                let action_state = if unit.activation.acted {
                    "SPENT"
                } else {
                    "READY"
                };
                let stance = unit
                    .reaction
                    .map(|reaction| format!("{reaction:?}"))
                    .unwrap_or_else(|| "--".to_owned());
                format!(
                    "{}\nHP {}/{}   EN {}/{}\nMOVE {}   ACTION {}\nSTANCE {}",
                    unit.name,
                    unit.hp,
                    unit.stats.max_hp,
                    unit.en,
                    unit.stats.max_en,
                    move_state,
                    action_state,
                    stance.to_uppercase()
                )
            },
        );

        Self {
            round_phase: format!("Round {} · {}", battle.round(), phase_label(battle.phase())),
            primary: format!("Eliminate all enemies · {remaining} remaining"),
            optional: format!(
                "Turnabout · {}",
                if battle.objectives().turnabout_complete {
                    "Complete"
                } else {
                    "Not yet"
                }
            ),
            selected_name: selected_unit.map(|unit| unit.name),
            selected_summary,
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
                        attacker: attacker.name,
                        weapon: weapon.name,
                        cells: intent
                            .footprint
                            .iter()
                            .map(|cell| format!("{},{}", cell.x, cell.y))
                            .collect::<Vec<_>>()
                            .join(" "),
                        intended_occupant,
                        normal_damage,
                        hit_chance,
                    })
                })
                .collect(),
            can_move: active.is_some_and(|unit| !unit.activation.moved),
            can_choose_reaction: active.is_some_and(|unit| !unit.activation.finished),
            can_finish: active.is_some_and(|unit| unit.reaction.is_some()),
            can_resolve: battle.ready_to_resolve(),
            weapon_names,
            weapon_enabled,
        }
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
pub struct CommandBar;

#[derive(Component)]
pub struct PreviewText;

#[derive(Component)]
pub struct StatusText;

#[derive(Component)]
pub struct ResultOverlay;

#[derive(Component)]
pub struct AssetStatusText;

#[derive(Component)]
struct HudRoot;

#[derive(Component, Clone, Copy)]
pub(crate) enum HudTextRole {
    Objective,
    Threats,
    Unit,
    Preview,
    Status,
    Result,
}

#[derive(Component)]
pub(crate) struct WeaponButtonLabel(usize);

pub fn setup_mission_ui(mut commands: Commands) {
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
        ))
        .id();

    commands.spawn((
        Text::new("// OBJECTIVES"),
        text_font(16.0),
        TextColor(Color::srgb(0.82, 0.94, 1.0)),
        panel_node(20.0, 18.0, 330.0),
        panel_background(),
        ObjectiveText,
        HudTextRole::Objective,
        Pickable::IGNORE,
        ChildOf(root),
    ));
    commands.spawn((
        Text::new("// LOCKED THREATS"),
        text_font(13.5),
        TextColor(Color::srgb(1.0, 0.76, 0.72)),
        Node {
            position_type: PositionType::Absolute,
            top: px(18),
            right: px(20),
            width: px(390),
            padding: UiRect::all(px(12)),
            ..default()
        },
        panel_background(),
        ThreatList,
        HudTextRole::Threats,
        Pickable::IGNORE,
        ChildOf(root),
    ));
    commands.spawn((
        Text::new("// UNIT"),
        text_font(14.0),
        TextColor(Color::srgb(0.76, 0.93, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            left: px(20),
            bottom: px(20),
            width: px(300),
            padding: UiRect::all(px(12)),
            ..default()
        },
        panel_background(),
        UnitSummary,
        HudTextRole::Unit,
        Pickable::IGNORE,
        ChildOf(root),
    ));
    commands.spawn((
        Text::new("TARGET PREVIEW"),
        text_font(13.0),
        TextColor(Color::srgb(1.0, 0.82, 0.46)),
        Node {
            position_type: PositionType::Absolute,
            left: px(340),
            bottom: px(92),
            width: px(435),
            padding: UiRect::all(px(9)),
            ..default()
        },
        panel_background(),
        PreviewText,
        HudTextRole::Preview,
        Pickable::IGNORE,
        ChildOf(root),
    ));
    commands.spawn((
        Text::new("Select a mech to begin."),
        text_font(12.5),
        TextColor(Color::srgb(0.78, 0.84, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            left: px(340),
            bottom: px(66),
            width: px(900),
            ..default()
        },
        StatusText,
        HudTextRole::Status,
        Pickable::IGNORE,
        ChildOf(root),
    ));
    commands.spawn((
        Text::new(""),
        text_font(28.0),
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: percent(32),
            top: percent(32),
            width: percent(36),
            padding: UiRect::all(px(28)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.025, 0.045, 0.97)),
        Visibility::Hidden,
        ResultOverlay,
        HudTextRole::Result,
        Pickable::IGNORE,
        ChildOf(root),
    ));

    let command_bar = commands
        .spawn((
            CommandBar,
            Node {
                position_type: PositionType::Absolute,
                left: px(340),
                right: px(20),
                bottom: px(14),
                height: px(46),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                column_gap: px(5),
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    spawn_command_button(
        &mut commands,
        command_bar,
        CommandAction::Move,
        "[M] MOVE",
        76.0,
        None,
    );
    for slot in 0..3 {
        spawn_command_button(
            &mut commands,
            command_bar,
            CommandAction::WeaponSlot(slot),
            "--",
            112.0,
            Some(slot),
        );
    }
    spawn_command_button(
        &mut commands,
        command_bar,
        CommandAction::Reaction(Reaction::Counter),
        "[C] COUNTER",
        82.0,
        None,
    );
    spawn_command_button(
        &mut commands,
        command_bar,
        CommandAction::Reaction(Reaction::Guard),
        "[G] GUARD",
        72.0,
        None,
    );
    spawn_command_button(
        &mut commands,
        command_bar,
        CommandAction::Reaction(Reaction::Evade),
        "[E] EVADE",
        72.0,
        None,
    );
    spawn_command_button(
        &mut commands,
        command_bar,
        CommandAction::FinishUnit,
        "[F] FINISH",
        82.0,
        None,
    );
    spawn_command_button(
        &mut commands,
        command_bar,
        CommandAction::ResolveAttacks,
        "[SPACE] RESOLVE",
        112.0,
        None,
    );

    commands.spawn((
        Text::new(format!("Loading {MISSION_ONE_GLTF_DISPLAY_PATH}...")),
        text_font(18.0),
        TextColor(Color::srgb(1.0, 0.78, 0.34)),
        BackgroundColor(Color::srgba(0.08, 0.025, 0.025, 0.94)),
        Node {
            position_type: PositionType::Absolute,
            right: px(24),
            bottom: px(154),
            padding: UiRect::all(px(12)),
            ..default()
        },
        Pickable::IGNORE,
        AssetStatusText,
        ChildOf(root),
    ));
}

pub(crate) fn update_hud(
    battle: Res<BattleRuntime>,
    interaction: Res<InteractionState>,
    status: Res<StatusMessage>,
    mut texts: Query<(&HudTextRole, &mut Text, Option<&mut Visibility>)>,
    mut weapon_labels: Query<(&WeaponButtonLabel, &mut Text), Without<HudTextRole>>,
    mut buttons: Query<(&CommandButton, &mut BackgroundColor)>,
) {
    let hud = HudSnapshot::from_battle(&battle.0, interaction.selected_unit);
    let threat_text = format_threats(&hud);
    let preview_text = interaction.preview.as_ref().map_or_else(
        || "TARGET PREVIEW\nArm a weapon and hover a target.".to_owned(),
        |preview| format_preview(&battle.0, preview),
    );
    let status_text = if status.0.is_empty() {
        "[M] MOVE  [1-3] WEAPONS  [C/G/E] STANCE  [F] FINISH  [SPACE] RESOLVE".to_owned()
    } else {
        status.0.clone()
    };

    for (role, mut text, visibility) in &mut texts {
        text.0 = match role {
            HudTextRole::Objective => format!(
                "// OBJECTIVES\n{}\n[P] {}\n[B] {}",
                ascii_separators(&hud.round_phase),
                ascii_separators(&hud.primary),
                ascii_separators(&hud.optional)
            ),
            HudTextRole::Threats => threat_text.clone(),
            HudTextRole::Unit => format!("// UNIT\n{}", hud.selected_summary),
            HudTextRole::Preview => preview_text.clone(),
            HudTextRole::Status => status_text.clone(),
            HudTextRole::Result => battle.0.result().map_or_else(String::new, |result| {
                format!(
                    "{}\n{}\nROUNDS {}\n\n[R] RESTART",
                    if result.victory {
                        "MISSION COMPLETE"
                    } else {
                        "MISSION FAILED"
                    },
                    if result.turnabout_complete {
                        "TURNABOUT COMPLETE"
                    } else {
                        "TURNABOUT MISSED"
                    },
                    result.rounds
                )
            }),
        };
        if matches!(role, HudTextRole::Result)
            && let Some(mut visibility) = visibility
        {
            *visibility = if battle.0.result().is_some() {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }

    for (label, mut text) in &mut weapon_labels {
        text.0 = hud.weapon_names[label.0]
            .map(|name| format!("[{}] {name}", label.0 + 1))
            .unwrap_or_else(|| format!("[{}] --", label.0 + 1));
    }
    for (button, mut background) in &mut buttons {
        let enabled = command_enabled(button.0, &hud);
        let armed = match (button.0, interaction.mode) {
            (CommandAction::Move, InteractionMode::Move) => true,
            (CommandAction::WeaponSlot(slot), InteractionMode::Attack(weapon)) => {
                hud.weapon_names
                    .get(slot)
                    .is_some_and(|name| name.is_some())
                    && interaction
                        .selected_unit
                        .and_then(|unit| battle.0.unit(unit))
                        .is_some_and(|unit| unit.weapons.get(slot).copied() == Some(weapon))
            }
            _ => false,
        };
        background.0 = if armed {
            Color::srgb(0.82, 0.38, 0.08)
        } else if enabled {
            Color::srgb(0.08, 0.25, 0.34)
        } else {
            Color::srgb(0.055, 0.07, 0.09)
        };
    }
}

pub fn update_asset_status_text(
    status: Res<AssetLoadStatus>,
    panel: Single<(&mut Text, &mut Visibility, &mut TextColor), With<AssetStatusText>>,
) {
    if !status.is_changed() {
        return;
    }
    let (mut text, mut visibility, mut color) = panel.into_inner();
    match &*status {
        AssetLoadStatus::Loading => {
            text.0 = format!("Loading {MISSION_ONE_GLTF_DISPLAY_PATH}...");
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
    parent: Entity,
    action: CommandAction,
    label: &str,
    width: f32,
    weapon_slot: Option<usize>,
) {
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
            ChildOf(parent),
        ))
        .observe(on_command_button_click)
        .id();
    let mut label_entity = commands.spawn((
        Text::new(label),
        text_font(11.5),
        TextColor(Color::srgb(0.88, 0.94, 1.0)),
        Pickable::IGNORE,
        ChildOf(button),
    ));
    if let Some(slot) = weapon_slot {
        label_entity.insert(WeaponButtonLabel(slot));
    }
}

fn command_enabled(action: CommandAction, hud: &HudSnapshot) -> bool {
    match action {
        CommandAction::Move => hud.can_move,
        CommandAction::WeaponSlot(slot) => hud.weapon_enabled.get(slot).copied().unwrap_or(false),
        CommandAction::Reaction(_) => hud.can_choose_reaction,
        CommandAction::FinishUnit => hud.can_finish,
        CommandAction::ResolveAttacks => hud.can_resolve,
        CommandAction::Restart => false,
    }
}

fn format_threats(hud: &HudSnapshot) -> String {
    let mut text = String::from("// LOCKED THREATS");
    if hud.threats.is_empty() {
        text.push_str("\nNONE");
        return text;
    }
    for threat in &hud.threats {
        text.push_str(&format!(
            "\n! {} / {} -> {}\n  {} DMG  {}% HIT  [{}]",
            threat.attacker,
            threat.weapon,
            threat.intended_occupant.unwrap_or("EMPTY"),
            threat.normal_damage,
            threat.hit_chance,
            threat.cells
        ));
    }
    text
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

fn text_font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..default()
    }
}

fn panel_node(left: f32, top: f32, width: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(left),
        top: px(top),
        width: px(width),
        padding: UiRect::all(px(12)),
        ..default()
    }
}

fn panel_background() -> BackgroundColor {
    BackgroundColor(Color::srgba(0.018, 0.035, 0.055, 0.88))
}
