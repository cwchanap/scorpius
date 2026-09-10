use bevy::{
    a11y::AccessibilityNode,
    prelude::*,
    text::{LetterSpacing, LineHeight},
};

use super::{
    BattleRuntime, EventPlayback,
    assets::UiAssets,
    interaction::{
        CommandAction, CommandButton, InteractionMode, InteractionState, on_command_button_click,
    },
    theme,
};

/// Presentation-only drill-down state for the fixed left sidebar.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MenuState {
    #[default]
    Hidden,
    Root,
    Weapons,
    Stances,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    Open(MenuState),
    Back,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct MenuButton(pub MenuAction);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct MenuRegion(pub MenuState);

#[derive(Component)]
pub struct TargetingPanel;

#[derive(Component)]
pub struct TargetingLabel;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct TargetingIcon {
    pub move_mode: bool,
}

#[derive(Component)]
pub struct ResolveButton;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct WeaponRow(pub usize);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct WeaponText {
    pub slot: usize,
    pub kind: WeaponTextKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeaponTextKind {
    Name,
    Damage,
    Hit,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct WeaponMeter {
    pub slot: usize,
    pub kind: WeaponMeterKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeaponMeterKind {
    Range,
    Energy(usize),
    Shape(usize),
}

/// Apply a sidebar navigation action after the shared interaction guard has
/// established that the active pilot owns the command area.
pub fn apply_menu_action(
    battle: &crate::domain::battle::BattleState,
    interaction: &mut InteractionState,
    action: MenuAction,
) -> bool {
    if battle.phase() != crate::domain::model::BattlePhase::Player
        || battle.active_unit().is_none()
        || interaction.mode != InteractionMode::Inspect
    {
        return false;
    }
    match action {
        MenuAction::Open(state) => interaction.menu = state,
        MenuAction::Back => interaction.menu = MenuState::Root,
    }
    true
}

/// Spawn the menu region below the inspector. The region is fixed to the
/// sidebar and all mutation rows keep using the shared command observer.
pub fn spawn_battle_menu(commands: &mut Commands, parent: Entity, assets: &UiAssets) {
    let root = commands
        .spawn((
            MenuRegion(MenuState::Root),
            menu_region_node(),
            Visibility::Hidden,
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    spawn_command_row(
        commands,
        root,
        assets,
        CommandAction::Move,
        theme::ICON_MOVE,
        "MOVE",
        19.0,
    );
    spawn_nav_row(
        commands,
        root,
        assets,
        MenuAction::Open(MenuState::Weapons),
        theme::ICON_ATTACK,
        "ATTACK",
    );
    spawn_nav_row(
        commands,
        root,
        assets,
        MenuAction::Open(MenuState::Stances),
        theme::ICON_GUARD,
        "STANCE",
    );
    spawn_command_row(
        commands,
        root,
        assets,
        CommandAction::PilotSkill,
        theme::ICON_SKILL,
        "SKILL",
        19.0,
    );
    spawn_command_row(
        commands,
        root,
        assets,
        CommandAction::FinishUnit,
        theme::ICON_WAIT,
        "WAIT",
        19.0,
    );

    let weapons = commands
        .spawn((
            MenuRegion(MenuState::Weapons),
            menu_region_node(),
            Visibility::Hidden,
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    spawn_back_row(commands, weapons, assets, theme::MENU_TARGET_RECT);
    for slot in 0..3 {
        spawn_weapon_row(
            commands,
            weapons,
            assets,
            CommandAction::WeaponSlot(slot),
            slot,
        );
    }

    let stances = commands
        .spawn((
            MenuRegion(MenuState::Stances),
            menu_region_node(),
            Visibility::Hidden,
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    spawn_back_row(commands, stances, assets, theme::STANCE_GUARD_RECT);
    for (action, icon, label) in [
        (
            CommandAction::Reaction(crate::domain::model::Reaction::Counter),
            theme::ICON_COUNTER,
            "COUNTER",
        ),
        (
            CommandAction::Reaction(crate::domain::model::Reaction::Guard),
            theme::ICON_GUARD,
            "GUARD",
        ),
        (
            CommandAction::Reaction(crate::domain::model::Reaction::Evade),
            theme::ICON_EVADE,
            "EVADE",
        ),
    ] {
        spawn_command_row(commands, stances, assets, action, icon, label, 18.0);
    }

    let targeting = commands
        .spawn((
            TargetingPanel,
            menu_region_node(),
            Visibility::Hidden,
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    let cancel = commands
        .spawn((
            Button,
            CommandButton(CommandAction::Cancel),
            targeting_row_node(64.0),
            BackgroundColor(Color::srgb_u8(42, 21, 18)),
            BorderColor::all(Color::srgb_u8(110, 51, 43)),
            Pickable::default(),
            ChildOf(targeting),
        ))
        .observe(on_command_button_click)
        .id();
    commands.spawn((
        theme::icon_node(
            assets.icons.clone(),
            theme::TARGETING_CANCEL_RECT,
            Color::srgb_u8(255, 179, 170),
        ),
        icon_node(26.0),
        Pickable::IGNORE,
        ChildOf(cancel),
    ));
    commands.spawn((
        Text::new("CANCEL"),
        theme::chakra_petch(&assets.fonts, 18.0, FontWeight(600)),
        LetterSpacing::Px(2.52),
        TextColor(Color::srgb_u8(255, 179, 170)),
        Pickable::IGNORE,
        ChildOf(cancel),
    ));
    commands.spawn((
        theme::icon_node(
            assets.icons.clone(),
            theme::ICON_MOVE,
            Color::srgb_u8(143, 168, 189),
        ),
        icon_node(24.0),
        TargetingIcon { move_mode: true },
        Visibility::Hidden,
        Pickable::IGNORE,
        ChildOf(cancel),
    ));
    commands.spawn((
        theme::icon_node(assets.icons.clone(), theme::ICON_ATTACK, theme::GOLD),
        TargetingIcon { move_mode: false },
        Visibility::Hidden,
        Node {
            width: px(24),
            height: px(24),
            flex_shrink: 0.0,
            margin: UiRect::left(Val::Auto),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(cancel),
    ));

    let resolve = commands
        .spawn((
            Button,
            CommandButton(CommandAction::ResolveAttacks),
            ResolveButton,
            Node {
                width: percent(100),
                height: px(72),
                flex_shrink: 0.0,
                display: Display::None,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: px(16),
                border: UiRect::all(px(2)),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(63, 42, 6)),
            BorderColor::all(theme::GOLD),
            Visibility::Hidden,
            Pickable::default(),
            ChildOf(parent),
        ))
        .observe(on_command_button_click)
        .id();
    commands.spawn((
        theme::icon_node(assets.icons.clone(), theme::ICON_FORWARD, theme::GOLD),
        icon_node(34.0),
        Pickable::IGNORE,
        ChildOf(resolve),
    ));
    commands.spawn((
        Text::new("RESOLVE"),
        theme::chakra_petch(&assets.fonts, 24.0, FontWeight(600)),
        LetterSpacing::Px(4.8),
        TextColor(Color::srgb_u8(255, 228, 173)),
        Pickable::IGNORE,
        ChildOf(resolve),
    ));
}

/// Route sidebar-only navigation. Domain commands never come through this
/// path, which keeps the existing command validation authoritative.
pub fn on_menu_button_click(
    mut click: On<Pointer<Click>>,
    buttons: Query<&MenuButton>,
    battle: Res<BattleRuntime>,
    mut interaction: ResMut<InteractionState>,
) {
    click.propagate(false);
    let Ok(button) = buttons.get(click.entity) else {
        return;
    };
    apply_menu_action(&battle.0, &mut interaction, button.0);
}

/// Keep the three menu blocks and the source-style targeting row mutually
/// exclusive as interaction state changes.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn update_battle_menu(
    battle: Res<BattleRuntime>,
    interaction: Res<InteractionState>,
    playback: Res<EventPlayback>,
    mut regions: Query<
        (&MenuRegion, &mut Visibility, &mut Node),
        (
            Without<TargetingPanel>,
            Without<ResolveButton>,
            Without<TargetingIcon>,
        ),
    >,
    mut targeting: Query<
        (&TargetingPanel, &mut Visibility, &mut Node),
        (Without<MenuRegion>, Without<ResolveButton>),
    >,
    mut targeting_icons: Query<
        (&TargetingIcon, &mut Visibility),
        (Without<TargetingPanel>, Without<ResolveButton>),
    >,
    mut buttons: Query<(&MenuButton, &mut Pickable)>,
    mut resolve: Query<(&mut Visibility, &mut Node), With<ResolveButton>>,
) {
    let active = battle.0.phase() == crate::domain::model::BattlePhase::Player
        && battle.0.active_unit().is_some()
        && !playback.input_locked;
    let inspect = interaction.mode == InteractionMode::Inspect;
    for (region, mut visibility, mut node) in &mut regions {
        let shown = active && inspect && interaction.menu == region.0;
        *visibility = if shown {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        node.display = if shown { Display::Flex } else { Display::None };
    }
    let targeting_active = active
        && matches!(
            interaction.mode,
            InteractionMode::Move | InteractionMode::Attack(_) | InteractionMode::AegisTarget
        );
    for (_, mut visibility, mut node) in &mut targeting {
        *visibility = if targeting_active {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        node.display = if targeting_active {
            Display::Flex
        } else {
            Display::None
        };
    }
    let resolve_shown = battle.0.ready_to_resolve() && !playback.input_locked;
    for (mut visibility, mut node) in &mut resolve {
        *visibility = if resolve_shown {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        node.display = if resolve_shown {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (icon, mut visibility) in &mut targeting_icons {
        let shown = targeting_active
            && matches!(
                (icon.move_mode, interaction.mode),
                (true, InteractionMode::Move)
                    | (false, InteractionMode::Attack(_))
                    | (false, InteractionMode::AegisTarget)
            );
        *visibility = if shown {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for (button, mut pickable) in &mut buttons {
        let enabled = active
            && inspect
            && match button.0 {
                MenuAction::Open(MenuState::Weapons | MenuState::Stances) => {
                    interaction.menu == MenuState::Root
                }
                MenuAction::Open(MenuState::Hidden | MenuState::Root) => false,
                MenuAction::Back => {
                    matches!(interaction.menu, MenuState::Weapons | MenuState::Stances)
                }
            };
        *pickable = if enabled {
            Pickable::default()
        } else {
            Pickable::IGNORE
        };
    }
}

fn menu_region_node() -> Node {
    Node {
        width: percent(100),
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        row_gap: px(4),
        ..default()
    }
}

fn targeting_row_node(height: f32) -> Node {
    Node {
        width: percent(100),
        height: px(height),
        display: Display::Flex,
        align_items: AlignItems::Center,
        column_gap: px(14),
        padding: UiRect::horizontal(px(18)),
        ..default()
    }
}

fn icon_node(size: f32) -> Node {
    Node {
        width: px(size),
        height: px(size),
        flex_shrink: 0.0,
        ..default()
    }
}

fn spawn_nav_row(
    commands: &mut Commands,
    parent: Entity,
    assets: &UiAssets,
    menu: MenuAction,
    icon: Rect,
    label: &'static str,
) {
    let row = commands
        .spawn((
            MenuButton(menu),
            targeting_row_node(54.0),
            BackgroundColor(Color::srgb_u8(10, 26, 38)),
            Pickable::default(),
            ChildOf(parent),
        ))
        .observe(on_menu_button_click)
        .id();
    commands.spawn((
        theme::icon_node(assets.icons.clone(), icon, theme::ACCENT),
        icon_node(28.0),
        Pickable::IGNORE,
        ChildOf(row),
    ));
    commands.spawn((
        Text::new(label),
        theme::chakra_petch(&assets.fonts, 19.0, FontWeight(600)),
        LetterSpacing::Px(1.9),
        TextColor(theme::TEXT),
        Pickable::IGNORE,
        ChildOf(row),
    ));
    commands.spawn((
        Text::new("›"),
        theme::ibm_plex_mono(&assets.fonts, 24.0, FontWeight(500)),
        TextColor(theme::MUTED),
        Node {
            margin: UiRect::left(Val::Auto),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(row),
    ));
}

fn spawn_command_row(
    commands: &mut Commands,
    parent: Entity,
    assets: &UiAssets,
    action: CommandAction,
    icon: Rect,
    label: &'static str,
    label_size: f32,
) {
    let row = commands
        .spawn((
            Button,
            CommandButton(action),
            targeting_row_node(54.0),
            BackgroundColor(Color::srgb_u8(10, 26, 38)),
            Pickable::default(),
            ChildOf(parent),
        ))
        .observe(on_command_button_click)
        .id();
    commands.spawn((
        theme::icon_node(assets.icons.clone(), icon, theme::ACCENT),
        icon_node(28.0),
        Pickable::IGNORE,
        ChildOf(row),
    ));
    commands.spawn((
        Text::new(label),
        theme::chakra_petch(&assets.fonts, label_size, FontWeight(600)),
        LetterSpacing::Px(label_size * 0.1),
        TextColor(theme::TEXT),
        Pickable::IGNORE,
        ChildOf(row),
    ));
}

fn spawn_weapon_row(
    commands: &mut Commands,
    parent: Entity,
    assets: &UiAssets,
    action: CommandAction,
    slot: usize,
) {
    let row = commands
        .spawn((
            Button,
            CommandButton(action),
            WeaponRow(slot),
            AccessibilityNode::default(),
            Node {
                width: percent(100),
                height: px(72),
                flex_shrink: 0.0,
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(14),
                padding: UiRect::horizontal(px(16)),
                border: UiRect::bottom(px(1)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::srgb_u8(18, 34, 47)),
            Pickable::default(),
            ChildOf(parent),
        ))
        .observe(on_command_button_click)
        .id();
    commands.spawn((
        Text::new("—"),
        theme::ibm_plex_mono(&assets.fonts, 1.0, FontWeight(400)),
        TextColor(Color::NONE),
        Node {
            display: Display::None,
            ..default()
        },
        Visibility::Hidden,
        WeaponText {
            slot,
            kind: WeaponTextKind::Name,
        },
        Pickable::IGNORE,
        ChildOf(row),
    ));
    let shape = commands
        .spawn((
            Node {
                width: px(36),
                height: px(36),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(3),
                flex_shrink: 0.0,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(row),
        ))
        .id();
    for row_index in 0..3 {
        let shape_row = commands
            .spawn((
                Node {
                    width: percent(100),
                    height: px(10),
                    display: Display::Flex,
                    column_gap: px(3),
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(shape),
            ))
            .id();
        for column_index in 0..3 {
            commands.spawn((
                Node {
                    width: px(10),
                    height: px(10),
                    ..default()
                },
                BackgroundColor(theme::BORDER),
                WeaponMeter {
                    slot,
                    kind: WeaponMeterKind::Shape(row_index * 3 + column_index),
                },
                Pickable::IGNORE,
                ChildOf(shape_row),
            ));
        }
    }

    let body = commands
        .spawn((
            Node {
                width: percent(100),
                min_width: px(0),
                flex_grow: 1.0,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(row),
        ))
        .id();
    let numbers = commands
        .spawn((
            Node {
                width: percent(100),
                display: Display::Flex,
                align_items: AlignItems::FlexEnd,
                column_gap: px(8),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(body),
        ))
        .id();
    commands.spawn((
        Text::new("—"),
        theme::ibm_plex_mono(&assets.fonts, 28.0, FontWeight(600)),
        LineHeight::RelativeToFont(0.9),
        TextColor(theme::GOLD),
        WeaponText {
            slot,
            kind: WeaponTextKind::Damage,
        },
        Pickable::IGNORE,
        ChildOf(numbers),
    ));
    commands.spawn((
        Text::new("—"),
        theme::ibm_plex_mono(&assets.fonts, 14.0, FontWeight(400)),
        TextColor(theme::MUTED),
        Node {
            margin: UiRect::bottom(px(2)),
            ..default()
        },
        WeaponText {
            slot,
            kind: WeaponTextKind::Hit,
        },
        Pickable::IGNORE,
        ChildOf(numbers),
    ));
    let range_track = commands
        .spawn((
            Node {
                position_type: PositionType::Relative,
                width: percent(100),
                height: px(4),
                ..default()
            },
            BackgroundColor(theme::BORDER),
            Pickable::IGNORE,
            ChildOf(body),
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
        BackgroundColor(theme::GOLD),
        WeaponMeter {
            slot,
            kind: WeaponMeterKind::Range,
        },
        Pickable::IGNORE,
        ChildOf(range_track),
    ));

    let tags = commands
        .spawn((
            Node {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: px(5),
                flex_shrink: 0.0,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(row),
        ))
        .id();
    let energy = commands
        .spawn((
            Node {
                display: Display::Flex,
                column_gap: px(3),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(tags),
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
            WeaponMeter {
                slot,
                kind: WeaponMeterKind::Energy(index),
            },
            Pickable::IGNORE,
            ChildOf(energy),
        ));
    }
    let tag_row = commands
        .spawn((
            Node {
                display: Display::Flex,
                column_gap: px(5),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(tags),
        ))
        .id();
    for (marker, icon) in [
        (WeaponTag::Push, theme::ICON_MOVE),
        (WeaponTag::Counter, theme::ICON_COUNTER),
    ] {
        commands.spawn((
            theme::icon_node(assets.icons.clone(), icon, theme::MUTED),
            icon_node(15.0),
            WeaponTagIcon { slot, marker },
            Visibility::Hidden,
            Pickable::IGNORE,
            ChildOf(tag_row),
        ));
    }
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct WeaponTagIcon {
    pub slot: usize,
    pub marker: WeaponTag,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeaponTag {
    Push,
    Counter,
}

fn spawn_back_row(commands: &mut Commands, parent: Entity, assets: &UiAssets, companion: Rect) {
    let row = commands
        .spawn((
            MenuButton(MenuAction::Back),
            targeting_row_node(44.0),
            BackgroundColor(Color::srgb_u8(10, 26, 38)),
            BorderColor::all(Color::srgb_u8(29, 50, 68)),
            Pickable::default(),
            ChildOf(parent),
        ))
        .observe(on_menu_button_click)
        .id();
    commands.spawn((
        theme::icon_node(assets.icons.clone(), theme::MENU_BACK_RECT, Color::WHITE),
        icon_node(20.0),
        Pickable::IGNORE,
        ChildOf(row),
    ));
    commands.spawn((
        theme::icon_node(assets.icons.clone(), companion, Color::WHITE),
        icon_node(20.0),
        Pickable::IGNORE,
        ChildOf(row),
    ));
}
