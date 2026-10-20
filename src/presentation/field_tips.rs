//! Control's field tips on the battle screen: a comms band under the stage
//! that shows each authored `FieldTip` the first time its trigger holds in a
//! battle visit, so Act 1 teaches its rules where they come up.
//!
//! Tips are pure presentation. They only queue while the player can act
//! (assets ready, playback idle, Player phase), never touch the domain, and
//! never block input. Enter or the button dismisses one; `H` turns tips off
//! or back on.

use std::collections::VecDeque;

use bevy::{prelude::*, text::LetterSpacing};

use crate::domain::{
    battle::BattleState,
    model::{BattlePhase, WeaponId},
};
use crate::mission::FieldTip;

use super::{
    ActiveMission, BattleEventQueue, BattleRuntime, EventPlayback,
    assets::{AssetLoadStatus, UiAssets, mission_assets_ready},
    interaction::{InteractionMode, InteractionState, StatusMessage},
    layout::battle_stage_rect,
    theme,
};

const BAND_GAP: f32 = 8.0;
const BAND_HEIGHT: f32 = 82.0;
const CONTROLS_WIDTH: f32 = 164.0;

/// Player preference for field tips; on by default so a first campaign is
/// taught, and `H` turns them off for anyone who knows the rules.
#[derive(Resource, Clone, Copy, Debug, Eq, PartialEq)]
pub struct TipSettings {
    pub enabled: bool,
}

impl Default for TipSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// The active battle visit's tips, by authored index. Battle entry resets
/// it; a restart keeps it, so a retry does not repeat what was already said.
#[derive(Resource, Clone, Debug, Default, Eq, PartialEq)]
pub struct FieldTips {
    /// Tips whose trigger has held this visit, shown or skipped.
    triggered: Vec<usize>,
    /// Tips waiting to show; the front one is on screen.
    queue: VecDeque<usize>,
}

impl FieldTips {
    /// The tip on screen.
    pub fn current(&self) -> Option<usize> {
        self.queue.front().copied()
    }

    /// Tips queued behind the one on screen.
    pub fn waiting(&self) -> usize {
        self.queue.len().saturating_sub(1)
    }

    pub fn dismiss(&mut self) {
        self.queue.pop_front();
    }

    /// Drop every queued tip without showing it.
    pub fn skip_all(&mut self) {
        self.queue.clear();
    }

    /// Mark every tip whose trigger now holds for the first time as
    /// triggered, and queue it in authored order when `show` is set. A tip
    /// that triggers while tips are off is skipped for the rest of the visit.
    pub fn observe(
        &mut self,
        tips: &[FieldTip],
        battle: &BattleState,
        armed: Option<WeaponId>,
        show: bool,
    ) {
        for (index, tip) in tips.iter().enumerate() {
            if self.triggered.contains(&index) || !tip.trigger.holds(battle, armed) {
                continue;
            }
            self.triggered.push(index);
            if show {
                self.queue.push_back(index);
            }
        }
    }
}

#[derive(Component)]
pub struct FieldTipBand;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldTipText {
    Title,
    Body,
    Button,
}

#[derive(Component)]
pub struct FieldTipButton;

/// Queue tips as their triggers first hold, and handle Enter and `H`.
/// Runs after the battle's input systems, so a tip about the command the
/// player just gave shows the same frame.
#[allow(clippy::too_many_arguments)]
pub(crate) fn stage_field_tips(
    mut tips: ResMut<FieldTips>,
    mut settings: ResMut<TipSettings>,
    mission: Res<ActiveMission>,
    battle: Res<BattleRuntime>,
    interaction: Res<InteractionState>,
    playback: Res<EventPlayback>,
    queue: Res<BattleEventQueue>,
    asset_status: Res<AssetLoadStatus>,
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    mut status: Option<ResMut<StatusMessage>>,
) {
    let pressed = |key: KeyCode| {
        keyboard
            .as_ref()
            .is_some_and(|input| input.just_pressed(key))
    };
    if pressed(KeyCode::KeyH) {
        settings.enabled = !settings.enabled;
        if !settings.enabled {
            tips.skip_all();
        }
        if let Some(status) = status.as_deref_mut() {
            status.0 = if settings.enabled {
                "Field tips on (H to turn off).".to_owned()
            } else {
                "Field tips off (H to turn on).".to_owned()
            };
        }
    }

    let player_can_act = mission_assets_ready(&asset_status)
        && playback.current.is_none()
        && queue.0.is_empty()
        && battle.0.phase() == BattlePhase::Player
        && battle.0.result().is_none();
    if !player_can_act {
        return;
    }
    if pressed(KeyCode::Enter) {
        tips.dismiss();
    }
    let armed = match interaction.mode {
        InteractionMode::Attack(weapon) => Some(weapon),
        InteractionMode::Inspect | InteractionMode::Move | InteractionMode::AegisTarget => None,
    };
    tips.observe(mission.0.tips, &battle.0, armed, settings.enabled);
}

/// Project the tip on screen onto the band. The band hides while there is
/// nothing to say, tips are off, assets are loading, or the battle is over.
pub(crate) fn sync_field_tip_band(
    tips: Res<FieldTips>,
    settings: Res<TipSettings>,
    mission: Res<ActiveMission>,
    battle: Res<BattleRuntime>,
    asset_status: Res<AssetLoadStatus>,
    mut bands: Query<&mut Visibility, With<FieldTipBand>>,
    mut texts: Query<(&FieldTipText, &mut Text)>,
) {
    let tip = tips
        .current()
        .and_then(|index| mission.0.tips.get(index))
        .filter(|_| {
            settings.enabled && mission_assets_ready(&asset_status) && battle.0.result().is_none()
        });
    let visibility = if tip.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut band in &mut bands {
        band.set_if_neq(visibility);
    }
    let Some(tip) = tip else {
        return;
    };
    for (part, mut text) in &mut texts {
        let copy = match part {
            FieldTipText::Title => tip.title,
            FieldTipText::Body => tip.text,
            FieldTipText::Button if tips.waiting() > 0 => "NEXT",
            FieldTipText::Button => "GOT IT",
        };
        if text.0 != copy {
            copy.clone_into(&mut text.0);
        }
    }
}

fn on_field_tip_button_click(mut click: On<Pointer<Click>>, mut tips: ResMut<FieldTips>) {
    click.propagate(false);
    tips.dismiss();
}

/// Spawn the hidden comms band under the battle stage.
pub fn spawn_field_tip_band(commands: &mut Commands, parent: Entity, ui_assets: &UiAssets) {
    let stage = battle_stage_rect();
    let band = commands
        .spawn((
            Name::new("Field tip band"),
            FieldTipBand,
            Node {
                position_type: PositionType::Absolute,
                left: px(stage.min.x),
                top: px(stage.max.y + BAND_GAP),
                width: px(stage.width()),
                height: px(BAND_HEIGHT),
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(16),
                padding: UiRect::right(px(16)),
                border: UiRect::left(px(3)),
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
            width: px(BAND_HEIGHT),
            height: px(BAND_HEIGHT),
            flex_shrink: 0.0,
            ..default()
        },
        ImageNode::new(ui_assets.control_portrait.clone()),
        Pickable::IGNORE,
        ChildOf(band),
    ));
    let copy = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: px(0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(3),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(band),
        ))
        .id();
    let header = commands
        .spawn((
            Node {
                display: Display::Flex,
                align_items: AlignItems::Center,
                column_gap: px(10),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(copy),
        ))
        .id();
    commands.spawn((
        Text::new("CONTROL"),
        theme::ibm_plex_mono(&ui_assets.fonts, 12.0, FontWeight(400)),
        LetterSpacing::Px(2.4),
        TextColor(theme::MUTED),
        Pickable::IGNORE,
        ChildOf(header),
    ));
    commands.spawn((
        Text::new(""),
        theme::ibm_plex_mono(&ui_assets.fonts, 13.0, FontWeight(600)),
        LetterSpacing::Px(1.3),
        TextColor(theme::ACCENT),
        FieldTipText::Title,
        Pickable::IGNORE,
        ChildOf(header),
    ));
    commands.spawn((
        Text::new(""),
        theme::chakra_petch(&ui_assets.fonts, 15.0, FontWeight(400)),
        TextColor(theme::TEXT),
        FieldTipText::Body,
        Pickable::IGNORE,
        ChildOf(copy),
    ));
    let controls = commands
        .spawn((
            Node {
                width: px(CONTROLS_WIDTH),
                flex_shrink: 0.0,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(6),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(band),
        ))
        .id();
    let button = commands
        .spawn((
            Button,
            FieldTipButton,
            Node {
                width: percent(100),
                height: px(36),
                display: Display::Flex,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: px(8),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(theme::PANEL_RAISED),
            BorderColor::all(theme::ACCENT),
            Pickable::default(),
            ChildOf(controls),
        ))
        .observe(on_field_tip_button_click)
        .id();
    commands.spawn((
        Text::new("GOT IT"),
        theme::ibm_plex_mono(&ui_assets.fonts, 13.0, FontWeight(600)),
        LetterSpacing::Px(1.3),
        TextColor(theme::TEXT),
        FieldTipText::Button,
        Pickable::IGNORE,
        ChildOf(button),
    ));
    commands.spawn((
        Text::new("ENTER  ·  H  TIPS OFF"),
        theme::ibm_plex_mono(&ui_assets.fonts, 11.0, FontWeight(400)),
        TextColor(theme::MUTED),
        Pickable::IGNORE,
        ChildOf(controls),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        board::GridPos,
        model::{BattleEvent, Reaction},
    };
    use crate::mission::{
        MissionId, TipTrigger, mission_definition,
        mission_one::{ids, mission_one},
    };

    const TIPS: [FieldTip; 3] = [
        FieldTip {
            trigger: TipTrigger::Opening,
            title: "FIRST",
            text: "",
        },
        FieldTip {
            trigger: TipTrigger::Opening,
            title: "SECOND",
            text: "",
        },
        FieldTip {
            trigger: TipTrigger::Activation(ids::VANGUARD),
            title: "VANGUARD",
            text: "",
        },
    ];

    fn opened_mission_one() -> BattleState {
        let mut battle = mission_one(7);
        battle.begin_round().unwrap();
        battle
    }

    #[test]
    fn tips_queue_in_authored_order_and_show_once() {
        let mut battle = opened_mission_one();
        let mut tips = FieldTips::default();

        tips.observe(&TIPS, &battle, None, true);
        assert_eq!(tips.current(), Some(0));
        assert_eq!(tips.waiting(), 1);
        tips.dismiss();
        assert_eq!(tips.current(), Some(1));
        tips.dismiss();
        assert_eq!(tips.current(), None);

        tips.observe(&TIPS, &battle, None, true);
        assert_eq!(tips.current(), None, "an Opening tip shows once per visit");

        battle.begin_activation(ids::VANGUARD).unwrap();
        tips.observe(&TIPS, &battle, None, true);
        assert_eq!(tips.current(), Some(2));
        tips.dismiss();
        battle
            .choose_reaction(ids::VANGUARD, Reaction::Guard)
            .unwrap();
        battle.finish_activation(ids::VANGUARD).unwrap();
        tips.observe(&TIPS, &battle, None, true);
        assert_eq!(tips.current(), None);
    }

    #[test]
    fn tips_that_trigger_while_off_are_skipped() {
        let battle = opened_mission_one();
        let mut tips = FieldTips::default();

        tips.observe(&TIPS, &battle, None, false);
        assert_eq!(tips.current(), None);
        tips.observe(&TIPS, &battle, None, true);
        assert_eq!(tips.current(), None);
    }

    #[test]
    fn mission_one_teaches_its_first_turn_in_order() {
        let definition = mission_definition(MissionId::One).unwrap();
        let title = |tips: &FieldTips| tips.current().map(|index| definition.tips[index].title);
        let mut battle = opened_mission_one();
        let mut tips = FieldTips::default();

        tips.observe(definition.tips, &battle, None, true);
        assert_eq!(title(&tips), Some("LOCKED ATTACKS"));
        tips.dismiss();

        battle.begin_activation(ids::VANGUARD).unwrap();
        tips.observe(definition.tips, &battle, Some(ids::REPULSOR_RAM), true);
        let mut shown = Vec::new();
        while let Some(title) = title(&tips) {
            shown.push(title);
            tips.dismiss();
        }
        assert_eq!(shown, ["YOUR MOVE", "STANCES", "PUSH"]);
    }

    fn blank_ui_assets() -> UiAssets {
        UiAssets {
            key_art: Handle::default(),
            briefing_art: Handle::default(),
            vanguard_art: Handle::default(),
            gunner_art: Handle::default(),
            interceptor_art: Handle::default(),
            vanguard_map: Handle::default(),
            gunner_map: Handle::default(),
            interceptor_map: Handle::default(),
            enemy_map: Handle::default(),
            control_portrait: Handle::default(),
            icons: Handle::default(),
            board: Handle::default(),
            terrain: Handle::default(),
            fonts: std::array::from_fn(|_| Handle::default()),
        }
    }

    fn tips_app() -> App {
        let mut app = App::new();
        app.insert_resource(BattleRuntime(opened_mission_one()))
            .insert_resource(ActiveMission(mission_definition(MissionId::One).unwrap()))
            .insert_resource(AssetLoadStatus::Ready)
            .init_resource::<TipSettings>()
            .init_resource::<FieldTips>()
            .init_resource::<InteractionState>()
            .init_resource::<EventPlayback>()
            .init_resource::<BattleEventQueue>()
            .init_resource::<StatusMessage>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, (stage_field_tips, sync_field_tip_band).chain());
        let root = app.world_mut().spawn(Node::default()).id();
        let assets = blank_ui_assets();
        spawn_field_tip_band(&mut app.world_mut().commands(), root, &assets);
        app.world_mut().flush();
        app
    }

    fn tap(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.release(key);
        input.clear();
    }

    fn band(app: &mut App) -> Visibility {
        *app.world_mut()
            .query_filtered::<&Visibility, With<FieldTipBand>>()
            .single(app.world())
            .unwrap()
    }

    fn band_text(app: &mut App, part: FieldTipText) -> String {
        app.world_mut()
            .query::<(&FieldTipText, &Text)>()
            .iter(app.world())
            .find(|(candidate, _)| **candidate == part)
            .map(|(_, text)| text.0.clone())
            .unwrap()
    }

    #[test]
    fn the_band_shows_the_opening_tip_and_enter_dismisses_it() {
        let mut app = tips_app();
        assert_eq!(band(&mut app), Visibility::Hidden);

        app.update();
        assert_eq!(band(&mut app), Visibility::Inherited);
        assert_eq!(band_text(&mut app, FieldTipText::Title), "LOCKED ATTACKS");
        assert!(band_text(&mut app, FieldTipText::Body).starts_with("Red-outlined cells"));
        assert_eq!(band_text(&mut app, FieldTipText::Button), "GOT IT");

        tap(&mut app, KeyCode::Enter);
        app.update();
        assert_eq!(app.world().resource::<FieldTips>().current(), None);
        assert_eq!(band(&mut app), Visibility::Hidden);
    }

    #[test]
    fn the_button_reads_next_while_tips_wait_behind_it() {
        let mut app = tips_app();
        app.update();
        app.world_mut()
            .resource_mut::<BattleRuntime>()
            .0
            .begin_activation(ids::VANGUARD)
            .unwrap();

        app.update();
        assert_eq!(band_text(&mut app, FieldTipText::Title), "LOCKED ATTACKS");
        assert_eq!(band_text(&mut app, FieldTipText::Button), "NEXT");
        tap(&mut app, KeyCode::Enter);
        assert_eq!(band_text(&mut app, FieldTipText::Title), "YOUR MOVE");
    }

    #[test]
    fn tips_wait_for_playback_to_finish() {
        let mut app = tips_app();
        app.world_mut()
            .resource_mut::<BattleEventQueue>()
            .0
            .push_back(BattleEvent::UnitMoved {
                unit: ids::VANGUARD,
                from: GridPos::new(4, 7),
                to: GridPos::new(4, 8),
            });

        app.update();
        assert_eq!(app.world().resource::<FieldTips>().current(), None);
        assert_eq!(band(&mut app), Visibility::Hidden);

        app.world_mut().resource_mut::<BattleEventQueue>().0.clear();
        app.update();
        assert_eq!(app.world().resource::<FieldTips>().current(), Some(0));
    }

    #[test]
    fn h_turns_tips_off_and_skips_what_was_queued() {
        let mut app = tips_app();
        app.update();
        assert_eq!(band(&mut app), Visibility::Inherited);

        tap(&mut app, KeyCode::KeyH);
        assert!(!app.world().resource::<TipSettings>().enabled);
        assert_eq!(app.world().resource::<FieldTips>().current(), None);
        assert_eq!(band(&mut app), Visibility::Hidden);
        assert_eq!(
            app.world().resource::<StatusMessage>().0,
            "Field tips off (H to turn on)."
        );

        // Tips that trigger while off stay skipped once tips come back on.
        app.world_mut()
            .resource_mut::<BattleRuntime>()
            .0
            .begin_activation(ids::VANGUARD)
            .unwrap();
        app.update();
        tap(&mut app, KeyCode::KeyH);
        assert!(app.world().resource::<TipSettings>().enabled);
        assert_eq!(app.world().resource::<FieldTips>().current(), None);
    }

    #[test]
    fn the_band_hides_once_the_battle_is_over() {
        let mut app = tips_app();
        app.update();
        assert_eq!(band(&mut app), Visibility::Inherited);
        {
            let mut battle = app.world_mut().resource_mut::<BattleRuntime>();
            battle.0.clear_reinforcements_for_test();
            for enemy in [
                ids::RIFLEMAN_LEFT,
                ids::RIFLEMAN_RIGHT,
                ids::STRIKER,
                ids::ARTILLERY,
            ] {
                battle.0.apply_direct_damage(
                    enemy,
                    99,
                    crate::domain::combat::DamageSource::PlayerWeapon(ids::PILE_LANCE),
                );
            }
            assert!(battle.0.result().is_some_and(|result| result.victory));
        }

        app.update();
        assert_eq!(band(&mut app), Visibility::Hidden);
    }
}
