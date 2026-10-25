//! The next-step prompt above the battle stage: one line that says what the
//! player can do right now and which key does it, so the command flow reads
//! without a manual. Pure presentation over public domain queries.

use std::collections::BTreeSet;

use bevy::prelude::*;

use crate::domain::{battle::BattleState, model::BattlePhase};

use super::{
    BattleRuntime, EventPlayback, MenuState,
    assets::{AssetLoadStatus, UiAssets, mission_assets_ready},
    interaction::{InteractionMode, InteractionState},
    layout::battle_stage_rect,
    sync::cell_highlights,
    theme,
};

const PROMPT_GAP: f32 = 12.0;
const PROMPT_HEIGHT: f32 = 52.0;

/// Which highlight the prompt points at; picks the band's icon and accent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromptTone {
    Info,
    Move,
    Attack,
    Ally,
    Resolve,
}

impl PromptTone {
    /// The tone's white icon mask, and whether to mirror it: the Back
    /// arrow, flipped, points to the next step.
    const fn icon(self) -> (Rect, bool) {
        match self {
            Self::Info => (theme::ICON_BACK, true),
            Self::Move => (theme::ICON_MOVE, false),
            Self::Attack => (theme::ICON_ATTACK, false),
            Self::Ally => (theme::ICON_GUARD, false),
            Self::Resolve => (theme::ICON_WAIT, false),
        }
    }

    const fn color(self) -> Color {
        match self {
            Self::Info => theme::ACCENT,
            Self::Move => theme::BOARD_SELECTED,
            Self::Attack | Self::Resolve => theme::GOLD,
            Self::Ally => theme::MINT,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BattlePrompt {
    pub tone: PromptTone,
    pub text: String,
}

impl BattlePrompt {
    fn new(tone: PromptTone, text: String) -> Self {
        Self { tone, text }
    }
}

/// The player's next step, or `None` while they cannot act (enemy phases,
/// a finished battle).
pub fn battle_prompt(battle: &BattleState, interaction: &InteractionState) -> Option<BattlePrompt> {
    if battle.phase() != BattlePhase::Player || battle.result().is_some() {
        return None;
    }
    let Some(active) = battle.active_unit().and_then(|id| battle.unit(id)) else {
        return Some(if battle.ready_to_resolve() {
            BattlePrompt::new(
                PromptTone::Resolve,
                "Every mech is set. RESOLVE [Space] plays the enemy turn.".to_owned(),
            )
        } else {
            BattlePrompt::new(
                PromptTone::Info,
                "Click one of your mechs to give it orders.".to_owned(),
            )
        });
    };
    let name = active.name;
    let prompt = match interaction.mode {
        InteractionMode::Move => BattlePrompt::new(
            PromptTone::Move,
            format!("Click a blue cell to move {name}.   [Esc] cancel"),
        ),
        InteractionMode::Attack(weapon) => {
            let weapon_name = battle.weapon(weapon).map_or("the weapon", |spec| spec.name);
            if has_target(battle, interaction) {
                BattlePrompt::new(
                    PromptTone::Attack,
                    format!("Click an amber target to fire {weapon_name}.   [Esc] cancel"),
                )
            } else {
                BattlePrompt::new(
                    PromptTone::Attack,
                    format!(
                        "Nothing to hit in {weapon_name}'s range. Move closer or pick another weapon.   [Esc] cancel"
                    ),
                )
            }
        }
        InteractionMode::AegisTarget if has_target(battle, interaction) => BattlePrompt::new(
            PromptTone::Ally,
            format!("Click a green ally next to {name} to shield it.   [Esc] cancel"),
        ),
        InteractionMode::AegisTarget => BattlePrompt::new(
            PromptTone::Ally,
            format!(
                "No ally stands next to {name} to shield. Move beside one first.   [Esc] cancel"
            ),
        ),
        InteractionMode::Inspect => match interaction.menu {
            MenuState::Weapons => BattlePrompt::new(
                PromptTone::Info,
                format!("Pick a weapon [1-3] for {name}.   [Backspace] back"),
            ),
            MenuState::Stances => BattlePrompt::new(
                PromptTone::Info,
                format!("Pick how {name} answers enemy fire this round.   [Backspace] back"),
            ),
            MenuState::Spirits => BattlePrompt::new(
                PromptTone::Info,
                format!("Spend {name}'s SP on a spirit.   [Backspace] back"),
            ),
            MenuState::Root | MenuState::Hidden => {
                let can_move = !active.activation.moved;
                let can_attack = !active.activation.acted;
                let text = match (active.reaction.is_some(), can_move, can_attack) {
                    (true, _, _) => format!("{name} is set. WAIT [F] ends its turn."),
                    (false, true, true) => {
                        format!("{name}: MOVE [M] or ATTACK [A], then set a STANCE [S].")
                    }
                    (false, true, false) => {
                        format!("{name}: MOVE [M], or set a STANCE [S] to end its turn.")
                    }
                    (false, false, true) => {
                        format!("{name}: ATTACK [A], or set a STANCE [S] to end its turn.")
                    }
                    (false, false, false) => format!("{name}: set a STANCE [S] to end its turn."),
                };
                BattlePrompt::new(PromptTone::Info, text)
            }
        },
    };
    Some(prompt)
}

/// Whether the armed command has a cell a click would act on, read from the
/// same projection the board highlights.
fn has_target(battle: &BattleState, interaction: &InteractionState) -> bool {
    cell_highlights(battle, interaction, &BTreeSet::new())
        .cells
        .values()
        .any(|kind| kind.clickable())
}

#[derive(Component)]
pub struct BattlePromptBand;

#[derive(Component)]
pub struct BattlePromptIcon;

#[derive(Component)]
pub struct BattlePromptText;

/// Spawn the hidden prompt band in the strip above the battle stage.
pub fn spawn_battle_prompt(commands: &mut Commands, parent: Entity, ui_assets: &UiAssets) {
    let stage = battle_stage_rect();
    let band = commands
        .spawn((
            Name::new("Battle prompt"),
            BattlePromptBand,
            Node {
                position_type: PositionType::Absolute,
                left: px(stage.min.x),
                top: px(stage.min.y - PROMPT_GAP - PROMPT_HEIGHT),
                width: px(stage.width()),
                height: px(PROMPT_HEIGHT),
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(14),
                padding: UiRect::horizontal(px(18)),
                border: UiRect::left(px(4)),
                ..default()
            },
            BackgroundColor(theme::PANEL),
            BorderColor::all(theme::ACCENT),
            Visibility::Hidden,
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        Node {
            width: px(24),
            height: px(24),
            flex_shrink: 0.0,
            ..default()
        },
        theme::icon_node(ui_assets.icons.clone(), theme::ICON_BACK, theme::ACCENT),
        BattlePromptIcon,
        Pickable::IGNORE,
        ChildOf(band),
    ));
    commands.spawn((
        Text::new(""),
        theme::chakra_petch(&ui_assets.fonts, 19.0, FontWeight(500)),
        TextColor(theme::TEXT),
        BattlePromptText,
        Pickable::IGNORE,
        ChildOf(band),
    ));
}

/// Show the prompt for the current state; hide it during playback, loading,
/// and every phase where the player has nothing to do.
#[allow(clippy::type_complexity)]
pub fn sync_battle_prompt(
    battle: Res<BattleRuntime>,
    interaction: Res<InteractionState>,
    playback: Res<EventPlayback>,
    asset_status: Option<Res<AssetLoadStatus>>,
    mut bands: Query<(&mut Visibility, &mut BorderColor), With<BattlePromptBand>>,
    mut icons: Query<&mut ImageNode, With<BattlePromptIcon>>,
    mut texts: Query<&mut Text, With<BattlePromptText>>,
) {
    let ready = asset_status.as_deref().is_none_or(mission_assets_ready);
    let prompt = (ready && !playback.input_locked)
        .then(|| battle_prompt(&battle.0, &interaction))
        .flatten();
    for (mut visibility, mut border) in &mut bands {
        visibility.set_if_neq(if prompt.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        if let Some(prompt) = &prompt {
            border.set_if_neq(BorderColor::all(prompt.tone.color()));
        }
    }
    let Some(prompt) = prompt else {
        return;
    };
    for mut icon in &mut icons {
        let (rect, flip) = prompt.tone.icon();
        if icon.rect != Some(rect) || icon.flip_x != flip || icon.color != prompt.tone.color() {
            icon.rect = Some(rect);
            icon.flip_x = flip;
            icon.color = prompt.tone.color();
        }
    }
    for mut text in &mut texts {
        if text.0 != prompt.text {
            text.0.clone_from(&prompt.text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::Reaction;
    use crate::mission::mission_one::{ids, mission_one};
    use crate::presentation::interaction::{CommandAction, execute_command};

    fn opened() -> BattleState {
        let mut battle = mission_one(7);
        battle.begin_round().unwrap();
        battle
    }

    fn prompt(battle: &BattleState, interaction: &InteractionState) -> BattlePrompt {
        battle_prompt(battle, interaction).expect("the player can act")
    }

    #[test]
    fn idle_player_phase_asks_for_a_mech_then_for_resolve() {
        let mut battle = opened();
        let interaction = InteractionState::default();
        let idle = prompt(&battle, &interaction);
        assert_eq!(idle.tone, PromptTone::Info);
        assert!(idle.text.contains("Click one of your mechs"));

        for unit in [ids::VANGUARD, ids::GUNNER, ids::INTERCEPTOR] {
            battle.begin_activation(unit).unwrap();
            battle.choose_reaction(unit, Reaction::Guard).unwrap();
            battle.finish_activation(unit).unwrap();
        }
        let resolve = prompt(&battle, &interaction);
        assert_eq!(resolve.tone, PromptTone::Resolve);
        assert!(resolve.text.contains("RESOLVE [Space]"));
    }

    #[test]
    fn root_menu_walks_the_activation_from_orders_to_wait() {
        let mut battle = opened();
        let mut interaction = InteractionState {
            menu: MenuState::Root,
            ..default()
        };
        battle.begin_activation(ids::VANGUARD).unwrap();
        assert_eq!(
            prompt(&battle, &interaction).text,
            "Vanguard: MOVE [M] or ATTACK [A], then set a STANCE [S]."
        );

        battle
            .move_unit(ids::VANGUARD, crate::domain::board::GridPos::new(4, 8))
            .unwrap();
        assert!(
            prompt(&battle, &interaction)
                .text
                .starts_with("Vanguard: ATTACK [A]")
        );

        interaction.menu = MenuState::Stances;
        assert!(
            prompt(&battle, &interaction)
                .text
                .contains("[Backspace] back")
        );
        interaction.menu = MenuState::Root;
        battle
            .choose_reaction(ids::VANGUARD, Reaction::Guard)
            .unwrap();
        assert_eq!(
            prompt(&battle, &interaction).text,
            "Vanguard is set. WAIT [F] ends its turn."
        );
    }

    #[test]
    fn armed_commands_name_their_highlight_color() {
        let mut battle = opened();
        let mut interaction = InteractionState::default();
        battle.begin_activation(ids::VANGUARD).unwrap();

        execute_command(&mut battle, &mut interaction, CommandAction::Move).unwrap();
        let armed = prompt(&battle, &interaction);
        assert_eq!(armed.tone, PromptTone::Move);
        assert!(armed.text.contains("blue cell"));

        execute_command(&mut battle, &mut interaction, CommandAction::Cancel).unwrap();
        execute_command(&mut battle, &mut interaction, CommandAction::PilotSkill).unwrap();
        let aegis = prompt(&battle, &interaction);
        assert_eq!(aegis.tone, PromptTone::Ally);
        assert!(aegis.text.starts_with("No ally stands next to Vanguard"));
        battle
            .move_unit(ids::VANGUARD, crate::domain::board::GridPos::new(4, 8))
            .unwrap();
        assert!(prompt(&battle, &interaction).text.contains("green ally"));
    }

    #[test]
    fn an_armed_weapon_says_when_nothing_is_in_range() {
        // Before the opening moves, the Striker stands three cells from the
        // Vanguard, out of the Pile Lance's one-cell reach.
        let mut battle = mission_one(7);
        battle.enter_player_phase_for_test();
        let mut interaction = InteractionState::default();
        battle.begin_activation(ids::VANGUARD).unwrap();
        execute_command(&mut battle, &mut interaction, CommandAction::WeaponSlot(0)).unwrap();
        let empty = prompt(&battle, &interaction);
        assert_eq!(empty.tone, PromptTone::Attack);
        assert!(
            empty
                .text
                .starts_with("Nothing to hit in Pile Lance's range")
        );

        battle.move_unit_direct_for_test(ids::STRIKER, crate::domain::board::GridPos::new(4, 6));
        assert!(
            prompt(&battle, &interaction)
                .text
                .starts_with("Click an amber target to fire Pile Lance")
        );
    }

    #[test]
    fn the_band_shows_the_prompt_and_hides_during_playback() {
        let mut app = App::new();
        app.insert_resource(BattleRuntime(opened()))
            .init_resource::<InteractionState>()
            .init_resource::<EventPlayback>()
            .add_systems(Update, sync_battle_prompt);
        let band = app
            .world_mut()
            .spawn((BattlePromptBand, Visibility::Hidden, BorderColor::default()))
            .id();
        let icon = app
            .world_mut()
            .spawn((BattlePromptIcon, ImageNode::default()))
            .id();
        let text = app
            .world_mut()
            .spawn((BattlePromptText, Text::default()))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<Visibility>(band),
            Some(&Visibility::Inherited)
        );
        assert_eq!(
            app.world().get::<Text>(text).unwrap().0,
            "Click one of your mechs to give it orders."
        );
        let image = app.world().get::<ImageNode>(icon).unwrap();
        assert_eq!(image.rect, Some(theme::ICON_BACK));
        assert!(image.flip_x);

        app.world_mut().resource_mut::<EventPlayback>().input_locked = true;
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(band),
            Some(&Visibility::Hidden)
        );
    }

    #[test]
    fn enemy_phases_and_finished_battles_show_no_prompt() {
        let battle = mission_one(7);
        assert_eq!(battle.phase(), BattlePhase::EnemyPlanning);
        assert_eq!(battle_prompt(&battle, &InteractionState::default()), None);
    }
}
