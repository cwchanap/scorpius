use bevy::prelude::*;

use super::{
    BattleRuntime, EventPlayback,
    assets::UiAssets,
    interaction::{
        CommandAction, CommandButton, InteractionMode, InteractionState, on_command_button_click,
    },
    theme,
    ui::CommandButtonLabel,
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
    );
    spawn_command_row(
        commands,
        root,
        assets,
        CommandAction::FinishUnit,
        theme::ICON_WAIT,
        "WAIT",
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
    spawn_back_row(commands, weapons, assets);
    for slot in 0..3 {
        spawn_command_row_with_label(
            commands,
            weapons,
            assets,
            CommandAction::WeaponSlot(slot),
            theme::ICON_ATTACK,
            "--",
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
    spawn_back_row(commands, stances, assets);
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
        spawn_command_row(commands, stances, assets, action, icon, label);
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
            Pickable::default(),
            ChildOf(targeting),
        ))
        .observe(on_command_button_click)
        .id();
    commands.spawn((
        theme::icon_node(
            assets.icons.clone(),
            theme::ICON_EVADE,
            Color::srgb_u8(255, 179, 170),
        ),
        icon_node(26.0),
        Pickable::IGNORE,
        ChildOf(cancel),
    ));
    commands.spawn((
        Text::new("CANCEL"),
        theme::chakra_petch(&assets.fonts, 18.0, FontWeight(600)),
        TextColor(Color::srgb_u8(255, 179, 170)),
        Pickable::IGNORE,
        ChildOf(cancel),
    ));
    commands.spawn((
        Text::new("TARGET"),
        theme::ibm_plex_mono(&assets.fonts, 12.0, FontWeight(500)),
        TextColor(theme::MUTED),
        Node {
            margin: UiRect::left(Val::Auto),
            ..default()
        },
        TargetingLabel,
        Pickable::IGNORE,
        ChildOf(cancel),
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
pub fn update_battle_menu(
    battle: Res<BattleRuntime>,
    interaction: Res<InteractionState>,
    playback: Res<EventPlayback>,
    mut regions: Query<(&MenuRegion, &mut Visibility, &mut Node), Without<TargetingPanel>>,
    mut targeting: Query<(&TargetingPanel, &mut Visibility, &mut Node), Without<MenuRegion>>,
    mut labels: Query<&mut Text, With<TargetingLabel>>,
    mut buttons: Query<(&MenuButton, &mut Pickable)>,
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
    let target_text = match interaction.mode {
        InteractionMode::Move => "DESTINATION",
        InteractionMode::Attack(_) => "ATTACK TARGET",
        InteractionMode::AegisTarget => "ALLY",
        InteractionMode::Inspect => "TARGET",
    };
    for mut text in &mut labels {
        text.0 = target_text.to_owned();
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
        theme::chakra_petch(&assets.fonts, 19.0, FontWeight(600)),
        TextColor(theme::TEXT),
        Pickable::IGNORE,
        ChildOf(row),
    ));
}

fn spawn_command_row_with_label(
    commands: &mut Commands,
    parent: Entity,
    assets: &UiAssets,
    action: CommandAction,
    icon: Rect,
    label: &'static str,
    slot: usize,
) {
    let row = commands
        .spawn((
            Button,
            CommandButton(action),
            targeting_row_node(72.0),
            BackgroundColor(Color::srgb_u8(10, 26, 38)),
            Pickable::default(),
            ChildOf(parent),
        ))
        .observe(on_command_button_click)
        .id();
    commands.spawn((
        theme::icon_node(assets.icons.clone(), icon, theme::GOLD),
        icon_node(28.0),
        Pickable::IGNORE,
        ChildOf(row),
    ));
    commands.spawn((
        Text::new(label),
        theme::chakra_petch(&assets.fonts, 18.0, FontWeight(600)),
        TextColor(theme::TEXT),
        CommandButtonLabel::WeaponSlot(slot),
        Pickable::IGNORE,
        ChildOf(row),
    ));
}

fn spawn_back_row(commands: &mut Commands, parent: Entity, assets: &UiAssets) {
    let row = commands
        .spawn((
            MenuButton(MenuAction::Back),
            targeting_row_node(44.0),
            BackgroundColor(Color::srgb_u8(10, 26, 38)),
            Pickable::default(),
            ChildOf(parent),
        ))
        .observe(on_menu_button_click)
        .id();
    commands.spawn((
        theme::icon_node(assets.icons.clone(), theme::ICON_BACK, theme::MUTED),
        icon_node(20.0),
        Pickable::IGNORE,
        ChildOf(row),
    ));
    commands.spawn((
        Text::new("BACK"),
        theme::ibm_plex_mono(&assets.fonts, 13.0, FontWeight(500)),
        TextColor(theme::MUTED),
        Pickable::IGNORE,
        ChildOf(row),
    ));
}
