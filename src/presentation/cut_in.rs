//! SRW-style battle cut-ins: a short side-view band over the battle stage
//! for each attack, showing the two SD sprites, the weapon, and the outcome.
//!
//! Cut-ins are pure presentation. They hold the `AttackRolled` playback event
//! longer while they play, read the outcome from the events still queued
//! behind it, and never touch the domain. Space, Enter, Escape, or a click
//! skips one; `V` turns them off or back on.

use std::collections::VecDeque;
use std::f32::consts::PI;
use std::time::Duration;

use bevy::prelude::*;

use crate::domain::battle::BattleState;
use crate::domain::combat::DamageSource;
use crate::domain::model::{BattleEvent, Faction, UnitArchetype, UnitId, WeaponId};

use super::{
    BattleEventQueue, BattleRuntime, BattleStage, EventPlayback, assets::UiAssets,
    interaction::StatusMessage, layout::BATTLE_STAGE_SIZE, sound::SoundCues, theme,
};

/// How long one cut-in holds the attack's playback event.
pub const CUT_IN_SECONDS: f32 = 1.8;
/// Share of a cut-in that passes before the blow lands on the target.
pub const CUT_IN_IMPACT: f32 = 0.55;

const BAND_TOP: f32 = 172.0;
const BAND_HEIGHT: f32 = 420.0;
const SPRITE_SIZE: f32 = 300.0;
const SPRITE_TOP: f32 = 80.0;
const ATTACKER_LEFT: f32 = 110.0;
const TARGET_LEFT: f32 = BATTLE_STAGE_SIZE.x - SPRITE_SIZE - 110.0;
const SPEED_LINES: usize = 10;

/// Player preference for battle cut-ins; on by default, like SRW's
/// battle animation setting.
#[derive(Resource, Clone, Copy, Debug, Eq, PartialEq)]
pub struct CutInSettings {
    pub enabled: bool,
}

impl Default for CutInSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Everything one cut-in shows, taken from the attack's `AttackRolled` event,
/// the events queued behind it, and the units' names and stats.
#[derive(Clone, Debug, PartialEq)]
pub struct CutInSnapshot {
    pub attacker_name: &'static str,
    pub attacker_archetype: UnitArchetype,
    pub attacker_faction: Faction,
    pub target_name: &'static str,
    pub target_archetype: UnitArchetype,
    pub weapon_name: &'static str,
    pub counter: bool,
    pub hit: bool,
    pub critical: bool,
    pub damage: i16,
    pub max_hp: i16,
    pub hp_before: i16,
    pub hp_after: i16,
}

impl CutInSnapshot {
    /// `pending` is the playback queue behind `event`. The domain has already
    /// applied every queued event, so the target's HP when this attack lands
    /// is its current HP plus all queued damage to it.
    pub fn from_attack(
        event: &BattleEvent,
        pending: &VecDeque<BattleEvent>,
        battle: &BattleState,
        counter: bool,
    ) -> Option<Self> {
        let BattleEvent::AttackRolled {
            attacker,
            weapon,
            target,
            hit,
            critical,
            ..
        } = event
        else {
            return None;
        };
        let attacking = battle.unit(*attacker)?;
        let defending = battle.unit(*target)?;
        let weapon = battle.weapon(*weapon)?;
        let damage_to_target = |event: &BattleEvent| match event {
            BattleEvent::DamageApplied {
                target: damaged,
                amount,
                ..
            } if damaged == target => Some(*amount),
            _ => None,
        };
        let queued: i16 = pending.iter().filter_map(damage_to_target).sum();
        let max_hp = defending.stats.max_hp;
        let hp_before = (defending.hp.max(0) + queued).clamp(0, max_hp);
        // This attack's own damage carries its weapon as the source. A hit
        // can deal none (a Guarded target), and the collision or hazard
        // damage that follows a push plays after the cut-in on the map.
        let from_this_weapon = |source: &DamageSource| match source {
            DamageSource::PlayerWeapon(used) => used == &weapon.id,
            DamageSource::EnemyWeapon(unit, used) => unit == attacker && used == &weapon.id,
            DamageSource::Collision | DamageSource::Hazard | DamageSource::Explosion => false,
        };
        let damage = if *hit {
            pending
                .iter()
                .take_while(|event| !matches!(event, BattleEvent::AttackRolled { .. }))
                .find_map(|event| match event {
                    BattleEvent::DamageApplied {
                        target: damaged,
                        amount,
                        source,
                        ..
                    } if damaged == target && from_this_weapon(source) => Some(*amount),
                    _ => None,
                })
                .unwrap_or(0)
        } else {
            0
        };
        Some(Self {
            attacker_name: attacking.name,
            attacker_archetype: attacking.archetype,
            attacker_faction: attacking.faction,
            target_name: defending.name,
            target_archetype: defending.archetype,
            weapon_name: weapon.name,
            counter,
            hit: *hit,
            critical: *critical,
            damage,
            max_hp,
            hp_before,
            hp_after: (hp_before - damage).max(0),
        })
    }

    pub fn knocked_out(&self) -> bool {
        self.hit && self.hp_after <= 0
    }

    fn accent(&self) -> Color {
        if self.attacker_faction == Faction::Player {
            theme::ACCENT
        } else {
            theme::ENEMY
        }
    }
}

/// The cut-in on screen, if any, and what playback has shown so far.
#[derive(Resource, Default)]
pub struct ActiveCutIn {
    pub snapshot: Option<CutInSnapshot>,
    /// The playback event the cut-in belongs to, by `EventPlayback::started`.
    event: Option<u64>,
    /// The last event this system saw as current, so each one is judged once.
    seen: Option<u64>,
    /// One cut-in per attack: a blast that rolls on several targets shows
    /// the first, the way SRW skips animations for map weapons. Counters
    /// never replace it, because an enemy blast resolves each target's
    /// counter before it rolls on the next target.
    last_strike: Option<(UnitId, WeaponId)>,
    counter_next: bool,
}

#[derive(Component)]
pub struct CutInRoot;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CutInPart {
    Backdrop,
    Band,
    SpeedLine(usize),
    Attacker,
    Target,
    Strike,
    Flash,
    Impact,
    Result,
    HpFill,
    HpText,
}

/// Start, skip, and end cut-ins around the playback event they hold.
/// Runs after `play_battle_events`, so a new `AttackRolled` gets its longer
/// timer before the timer first ticks.
#[allow(clippy::too_many_arguments)]
pub(crate) fn stage_cut_ins(
    mut commands: Commands,
    mut settings: ResMut<CutInSettings>,
    mut active: ResMut<ActiveCutIn>,
    mut playback: ResMut<EventPlayback>,
    battle: Res<BattleRuntime>,
    queue: Res<BattleEventQueue>,
    ui_assets: Res<UiAssets>,
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    mut status: Option<ResMut<StatusMessage>>,
    mut cues: Option<ResMut<SoundCues>>,
    stages: Query<Entity, With<BattleStage>>,
    roots: Query<Entity, With<CutInRoot>>,
) {
    let pressed = |key: KeyCode| {
        keyboard
            .as_ref()
            .is_some_and(|input| input.just_pressed(key))
    };
    if pressed(KeyCode::KeyV) {
        settings.enabled = !settings.enabled;
        if let Some(status) = status.as_deref_mut() {
            status.0 = if settings.enabled {
                "Battle cut-ins on (V to turn off).".to_owned()
            } else {
                "Battle cut-ins off (V to turn on).".to_owned()
            };
        }
    }

    let current = playback.current.is_some().then_some(playback.started);
    if current.is_some() && current == active.event {
        let skip = !settings.enabled
            || [KeyCode::Space, KeyCode::Enter, KeyCode::Escape]
                .into_iter()
                .any(pressed)
            || mouse
                .as_ref()
                .is_some_and(|input| input.just_pressed(MouseButton::Left));
        if skip && let Some((_, timer)) = playback.current.as_mut() {
            let duration = timer.duration();
            timer.set_elapsed(duration);
            if let Some(cues) = cues.as_deref_mut() {
                cues.release_held();
            }
        }
        return;
    }

    // The held event is over (or playback was reset): close the band.
    if active.event.is_some() {
        for root in &roots {
            commands.entity(root).try_despawn();
        }
        active.snapshot = None;
        active.event = None;
    }
    if current.is_none() && queue.0.is_empty() {
        active.last_strike = None;
        active.counter_next = false;
        active.seen = None;
        return;
    }
    if current == active.seen {
        return;
    }
    active.seen = current;
    let Some(event) = playback.current.as_ref().map(|(event, _)| event.clone()) else {
        return;
    };
    match &event {
        BattleEvent::CounterFired { .. } => active.counter_next = true,
        BattleEvent::AttackRolled {
            attacker, weapon, ..
        } => {
            let strike = (*attacker, *weapon);
            let counter = std::mem::take(&mut active.counter_next);
            let repeat = !counter && active.last_strike == Some(strike);
            if !counter {
                active.last_strike = Some(strike);
            }
            if repeat || !settings.enabled {
                return;
            }
            let Some(snapshot) = CutInSnapshot::from_attack(&event, &queue.0, &battle.0, counter)
            else {
                return;
            };
            let Some(stage) = stages.iter().next() else {
                return;
            };
            spawn_cut_in(&mut commands, stage, &ui_assets, &snapshot);
            if let Some((_, timer)) = playback.current.as_mut() {
                timer.set_duration(Duration::from_secs_f32(CUT_IN_SECONDS));
            }
            if let Some(cues) = cues.as_deref_mut() {
                cues.hold_strike_for_cut_in(CUT_IN_IMPACT * CUT_IN_SECONDS);
            }
            active.snapshot = Some(snapshot);
            active.event = current;
        }
        _ => {}
    }
}

fn spawn_cut_in(
    commands: &mut Commands,
    stage: Entity,
    ui_assets: &UiAssets,
    snapshot: &CutInSnapshot,
) {
    let accent = snapshot.accent();
    let root = commands
        .spawn((
            Name::new("Battle cut-in"),
            CutInRoot,
            CutInPart::Backdrop,
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: px(BATTLE_STAGE_SIZE.x),
                height: px(BATTLE_STAGE_SIZE.y),
                ..default()
            },
            UiTransform::IDENTITY,
            BackgroundColor(Color::BLACK.with_alpha(0.0)),
            ZIndex(3000),
            Pickable::IGNORE,
            ChildOf(stage),
        ))
        .id();
    let band = commands
        .spawn((
            CutInPart::Band,
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(BAND_TOP),
                width: px(BATTLE_STAGE_SIZE.x),
                height: px(BAND_HEIGHT),
                overflow: Overflow::clip(),
                ..default()
            },
            UiTransform::from_scale(Vec2::new(1.0, 0.0)),
            BackgroundColor(Color::srgb_u8(8, 15, 26)),
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    for (top, edge) in [(0.0, "top"), (BAND_HEIGHT - 4.0, "bottom")] {
        commands.spawn((
            Name::new(format!("Cut-in {edge} rule")),
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(top),
                width: percent(100.0),
                height: px(4.0),
                ..default()
            },
            BackgroundColor(accent),
            Pickable::IGNORE,
            ChildOf(band),
        ));
    }
    for line in 0..SPEED_LINES {
        commands.spawn((
            CutInPart::SpeedLine(line),
            Node {
                position_type: PositionType::Absolute,
                left: px(speed_line_left(line, 0.0)),
                top: px(24.0 + 37.0 * line as f32),
                width: px(140.0 + 40.0 * (line % 4) as f32),
                height: px(2.0),
                ..default()
            },
            BackgroundColor(accent.with_alpha(0.22)),
            Pickable::IGNORE,
            ChildOf(band),
        ));
    }
    commands.spawn((
        CutInPart::Strike,
        Node {
            position_type: PositionType::Absolute,
            left: px(ATTACKER_LEFT + SPRITE_SIZE * 0.7),
            top: px(SPRITE_TOP + SPRITE_SIZE * 0.45),
            width: px(0.0),
            height: px(10.0),
            ..default()
        },
        BackgroundColor(theme::GOLD),
        Visibility::Hidden,
        Pickable::IGNORE,
        ChildOf(band),
    ));
    spawn_sprite(
        commands,
        band,
        ui_assets.map_sprite(snapshot.attacker_archetype).clone(),
        CutInPart::Attacker,
        // SD sprites face left; the attacker stands on the left facing right.
        true,
    );
    spawn_sprite(
        commands,
        band,
        ui_assets.map_sprite(snapshot.target_archetype).clone(),
        CutInPart::Target,
        false,
    );
    commands.spawn((
        CutInPart::Impact,
        Node {
            position_type: PositionType::Absolute,
            left: px(TARGET_LEFT + SPRITE_SIZE * 0.5 - 48.0),
            top: px(SPRITE_TOP + SPRITE_SIZE * 0.45 - 48.0),
            width: px(96.0),
            height: px(96.0),
            ..default()
        },
        UiTransform::IDENTITY,
        theme::icon_node(ui_assets.icons.clone(), theme::ICON_ATTACK, theme::GOLD),
        Visibility::Hidden,
        Pickable::IGNORE,
        ChildOf(band),
    ));
    commands.spawn((
        CutInPart::Flash,
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: percent(100.0),
            height: percent(100.0),
            ..default()
        },
        BackgroundColor(Color::WHITE.with_alpha(0.0)),
        Pickable::IGNORE,
        ChildOf(band),
    ));

    // Attacker side: who is attacking, and with what.
    let mut attacker_label = snapshot.attacker_name.to_uppercase();
    if snapshot.counter {
        attacker_label.push_str("  ·  COUNTER");
    }
    spawn_text(
        commands,
        band,
        attacker_label,
        theme::ibm_plex_mono(&ui_assets.fonts, 16.0, FontWeight(500)),
        theme::MUTED,
        (Val::Px(40.0), Val::Px(22.0)),
        None,
    );
    spawn_text(
        commands,
        band,
        snapshot.weapon_name.to_uppercase(),
        theme::chakra_petch(&ui_assets.fonts, 40.0, FontWeight(700)),
        accent,
        (Val::Px(40.0), Val::Px(42.0)),
        None,
    );

    // Target side: name and an HP bar that drains as the hit lands.
    spawn_text(
        commands,
        band,
        snapshot.target_name.to_uppercase(),
        theme::ibm_plex_mono(&ui_assets.fonts, 16.0, FontWeight(500)),
        theme::MUTED,
        (Val::Auto, Val::Px(22.0)),
        Some(40.0),
    );
    let hp_bar = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(40.0),
                top: px(50.0),
                width: px(260.0),
                height: px(10.0),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(theme::BORDER),
            Pickable::IGNORE,
            ChildOf(band),
        ))
        .id();
    commands.spawn((
        CutInPart::HpFill,
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: percent(hp_percent(snapshot.hp_before, snapshot.max_hp)),
            height: percent(100.0),
            ..default()
        },
        BackgroundColor(if snapshot.attacker_faction == Faction::Player {
            theme::ENEMY
        } else {
            theme::MINT
        }),
        Pickable::IGNORE,
        ChildOf(hp_bar),
    ));
    commands.spawn((
        CutInPart::HpText,
        Text::new(format!("HP {}/{}", snapshot.hp_before, snapshot.max_hp)),
        theme::ibm_plex_mono(&ui_assets.fonts, 16.0, FontWeight(600)),
        TextColor(theme::TEXT),
        Node {
            position_type: PositionType::Absolute,
            right: px(40.0),
            top: px(66.0),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(band),
    ));
    commands.spawn((
        CutInPart::Result,
        Text::new(result_text(snapshot, false)),
        theme::chakra_petch(&ui_assets.fonts, 44.0, FontWeight(700)),
        TextColor(if snapshot.hit {
            theme::GOLD
        } else {
            theme::TEXT
        }),
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            left: px(TARGET_LEFT - 40.0),
            width: px(SPRITE_SIZE + 80.0),
            top: px(SPRITE_TOP + 10.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        UiTransform::IDENTITY,
        Visibility::Hidden,
        Pickable::IGNORE,
        ChildOf(band),
    ));
    spawn_text(
        commands,
        band,
        "SPACE  SKIP   ·   V  CUT-INS OFF".to_owned(),
        theme::ibm_plex_mono(&ui_assets.fonts, 13.0, FontWeight(400)),
        theme::MUTED,
        (Val::Auto, Val::Px(BAND_HEIGHT - 30.0)),
        Some(24.0),
    );
}

fn spawn_sprite(
    commands: &mut Commands,
    band: Entity,
    image: Handle<Image>,
    part: CutInPart,
    flip_x: bool,
) {
    let mut sprite = ImageNode::new(image);
    sprite.flip_x = flip_x;
    commands.spawn((
        part,
        Node {
            position_type: PositionType::Absolute,
            left: px(sprite_left(part, 0.0)),
            top: px(SPRITE_TOP),
            width: px(SPRITE_SIZE),
            height: px(SPRITE_SIZE),
            ..default()
        },
        UiTransform::IDENTITY,
        sprite,
        Pickable::IGNORE,
        ChildOf(band),
    ));
}

fn spawn_text(
    commands: &mut Commands,
    band: Entity,
    text: String,
    font: TextFont,
    color: Color,
    (left, top): (Val, Val),
    right: Option<f32>,
) {
    commands.spawn((
        Text::new(text),
        font,
        TextColor(color),
        Node {
            position_type: PositionType::Absolute,
            left,
            right: right.map_or(Val::Auto, px),
            top,
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(band),
    ));
}

fn result_text(snapshot: &CutInSnapshot, settled: bool) -> String {
    if !snapshot.hit {
        return "MISS".to_owned();
    }
    let damage = if snapshot.critical {
        format!("CRITICAL\n-{}", snapshot.damage)
    } else {
        format!("-{}", snapshot.damage)
    };
    if settled && snapshot.knocked_out() {
        format!("{damage}\nDESTROYED")
    } else {
        damage
    }
}

fn hp_percent(hp: i16, max_hp: i16) -> f32 {
    100.0 * f32::from(hp.max(0)) / f32::from(max_hp.max(1))
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 0 → 1 → 0 across `[start, end]`.
fn bump(progress: f32, start: f32, end: f32) -> f32 {
    if progress <= start || progress >= end {
        return 0.0;
    }
    ((progress - start) / (end - start) * PI).sin()
}

fn speed_line_left(line: usize, progress: f32) -> f32 {
    let span = BATTLE_STAGE_SIZE.x + 400.0;
    let start = (line as f32 * 397.0) % span;
    (start - progress * 1400.0).rem_euclid(span) - 300.0
}

fn sprite_left(part: CutInPart, progress: f32) -> f32 {
    let enter = smooth(progress / 0.18);
    match part {
        CutInPart::Attacker => -SPRITE_SIZE + (ATTACKER_LEFT + SPRITE_SIZE) * enter,
        _ => BATTLE_STAGE_SIZE.x + (TARGET_LEFT - BATTLE_STAGE_SIZE.x) * enter,
    }
}

/// Lay out the cut-in for the held event's timer fraction.
#[allow(clippy::type_complexity)]
pub(crate) fn animate_cut_in(
    active: Res<ActiveCutIn>,
    playback: Res<EventPlayback>,
    mut parts: Query<(
        &CutInPart,
        &mut Node,
        Option<&mut UiTransform>,
        Option<&mut BackgroundColor>,
        Option<&mut ImageNode>,
        Option<&mut Text>,
        Option<&mut Visibility>,
    )>,
) {
    let Some(snapshot) = active.snapshot.as_ref() else {
        return;
    };
    let Some((_, timer)) = playback
        .current
        .as_ref()
        .filter(|_| active.event == Some(playback.started))
    else {
        return;
    };
    let p = timer.fraction();
    let open = smooth(p / 0.12).min(1.0 - smooth((p - 0.92) / 0.08));
    let strike = smooth((p - 0.42) / 0.12);
    let landed = p >= CUT_IN_IMPACT;
    let drain = smooth((p - 0.56) / 0.22);
    let hp_now = if snapshot.hit {
        f32::from(snapshot.hp_before)
            + (f32::from(snapshot.hp_after) - f32::from(snapshot.hp_before)) * drain
    } else {
        f32::from(snapshot.hp_before)
    };

    for (part, mut node, transform, background, image, text, visibility) in &mut parts {
        match part {
            CutInPart::Backdrop => {
                if let Some(mut background) = background {
                    background.0 = Color::BLACK.with_alpha(0.85 * open);
                }
            }
            CutInPart::Band => {
                if let Some(mut transform) = transform {
                    transform.scale = Vec2::new(1.0, open);
                }
            }
            CutInPart::SpeedLine(line) => node.left = px(speed_line_left(*line, p)),
            CutInPart::Attacker => {
                let lunge = 80.0 * bump(p, 0.40, 0.62);
                node.left = px(sprite_left(*part, p) + lunge);
            }
            CutInPart::Target => {
                let mut left = sprite_left(*part, p);
                let mut top = SPRITE_TOP;
                if snapshot.hit {
                    left += 10.0 * (p * PI * 14.0).sin() * bump(p, 0.55, 0.78);
                } else {
                    let dodge = bump(p, 0.48, 0.74);
                    left += 70.0 * dodge;
                    top -= 30.0 * dodge;
                }
                node.left = px(left);
                node.top = px(top);
                if let Some(mut image) = image {
                    let fade = if snapshot.knocked_out() {
                        1.0 - smooth((p - 0.72) / 0.16)
                    } else {
                        1.0
                    };
                    image.color = Color::WHITE.with_alpha(fade);
                }
            }
            CutInPart::Strike => {
                let reach = TARGET_LEFT + SPRITE_SIZE * 0.3 - (ATTACKER_LEFT + SPRITE_SIZE * 0.7);
                node.width = px(reach.max(0.0) * strike);
                if let Some(mut visibility) = visibility {
                    *visibility = if (0.42..0.6).contains(&p) {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    };
                }
            }
            CutInPart::Flash => {
                if let Some(mut background) = background {
                    let flash = if snapshot.hit {
                        bump(p, 0.52, 0.64) * if snapshot.critical { 0.8 } else { 0.55 }
                    } else {
                        0.0
                    };
                    background.0 = Color::WHITE.with_alpha(flash);
                }
            }
            CutInPart::Impact => {
                let pulse = bump(p, 0.54, 0.76);
                if let Some(mut transform) = transform {
                    transform.scale = Vec2::splat(0.6 + 0.8 * pulse);
                }
                if let Some(mut visibility) = visibility {
                    *visibility = if snapshot.hit && pulse > 0.0 {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    };
                }
            }
            CutInPart::Result => {
                if let Some(mut transform) = transform {
                    transform.scale = Vec2::splat(1.0 + 0.4 * (1.0 - smooth((p - 0.55) / 0.08)));
                }
                if let Some(mut visibility) = visibility {
                    *visibility = if landed {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    };
                }
                if let Some(mut text) = text {
                    let next = result_text(snapshot, p >= 0.8);
                    if text.0 != next {
                        text.0 = next;
                    }
                }
            }
            CutInPart::HpFill => {
                node.width = percent(100.0 * hp_now / f32::from(snapshot.max_hp.max(1)));
            }
            CutInPart::HpText => {
                if let Some(mut text) = text {
                    let next = format!("HP {}/{}", hp_now.round() as i16, snapshot.max_hp);
                    if text.0 != next {
                        text.0 = next;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::board::GridPos;
    use crate::mission::mission_one::{ids, mission_one};

    /// The Vanguard's opening Pile Lance on the Striker in front of it, with
    /// the events it produced.
    fn pile_lance_on_striker(seed: u64) -> (BattleState, Vec<BattleEvent>) {
        let mut battle = mission_one(seed);
        battle.begin_round().unwrap();
        battle.begin_activation(ids::VANGUARD).unwrap();
        let striker = battle.unit(ids::STRIKER).unwrap().position;
        assert_eq!(striker, GridPos::new(4, 6));
        let events = battle
            .attack(ids::VANGUARD, ids::PILE_LANCE, striker)
            .unwrap();
        (battle, events)
    }

    #[test]
    fn snapshot_reads_the_outcome_from_the_queued_events() {
        for seed in 0..32 {
            let (battle, events) = pile_lance_on_striker(seed);
            let mut pending: VecDeque<_> = events.into();
            let rolled = pending.pop_front().unwrap();
            let BattleEvent::AttackRolled { hit, critical, .. } = rolled else {
                panic!("an attack opens with its roll: {rolled:?}");
            };
            let snapshot = CutInSnapshot::from_attack(&rolled, &pending, &battle, false).unwrap();
            let striker = battle.unit(ids::STRIKER).unwrap();
            assert_eq!(snapshot.attacker_name, "Vanguard");
            assert_eq!(snapshot.target_name, striker.name);
            assert_eq!(snapshot.weapon_name, "Pile Lance");
            assert_eq!(snapshot.attacker_faction, Faction::Player);
            assert_eq!((snapshot.hit, snapshot.critical), (hit, critical));
            assert_eq!(snapshot.hp_before, striker.stats.max_hp);
            assert_eq!(
                snapshot.hp_after,
                striker.hp.max(0),
                "seed {seed}: the bar ends where the domain left the Striker"
            );
            assert_eq!(snapshot.damage > 0, hit);
            assert_eq!(snapshot.knocked_out(), striker.is_knocked_out());
        }
    }

    #[test]
    fn a_hit_that_deals_no_weapon_damage_does_not_claim_the_collision_after_it() {
        let (battle, _) = pile_lance_on_striker(7);
        let striker = battle.unit(ids::STRIKER).unwrap();
        let rolled = BattleEvent::AttackRolled {
            attacker: ids::VANGUARD,
            weapon: ids::PILE_LANCE,
            target: ids::STRIKER,
            roll: 10,
            hit: true,
            critical_roll: Some(5),
            critical: true,
        };
        // A Guarded hit for 0, then the push slams the target into a wall.
        let pending = VecDeque::from([
            BattleEvent::CollisionOccurred {
                unit: ids::STRIKER,
                blocked_at: striker.position,
            },
            BattleEvent::DamageApplied {
                target: ids::STRIKER,
                amount: 3,
                remaining_hp: striker.hp,
                source: DamageSource::Collision,
            },
        ]);
        let snapshot = CutInSnapshot::from_attack(&rolled, &pending, &battle, false).unwrap();
        assert_eq!(snapshot.damage, 0);
        assert_eq!(
            snapshot.hp_before,
            (striker.hp.max(0) + 3).min(striker.stats.max_hp)
        );
        assert_eq!(snapshot.hp_after, snapshot.hp_before);
    }

    #[test]
    fn snapshot_ignores_events_that_are_not_attack_rolls() {
        let (battle, _) = pile_lance_on_striker(7);
        assert_eq!(
            CutInSnapshot::from_attack(
                &BattleEvent::OptionalObjectiveCompleted,
                &VecDeque::new(),
                &battle,
                false
            ),
            None
        );
    }

    #[test]
    fn result_text_reads_like_srw() {
        let (battle, events) = pile_lance_on_striker(7);
        let mut pending: VecDeque<_> = events.into();
        let rolled = pending.pop_front().unwrap();
        let mut snapshot = CutInSnapshot::from_attack(&rolled, &pending, &battle, false).unwrap();
        snapshot.hit = false;
        assert_eq!(result_text(&snapshot, true), "MISS");
        snapshot.hit = true;
        snapshot.critical = true;
        snapshot.damage = 12;
        snapshot.hp_after = 0;
        assert_eq!(result_text(&snapshot, false), "CRITICAL\n-12");
        assert_eq!(result_text(&snapshot, true), "CRITICAL\n-12\nDESTROYED");
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

    fn cut_in_app(battle: BattleState, queue: Vec<BattleEvent>, enabled: bool) -> App {
        use bevy::time::TimeUpdateStrategy;

        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
                0.25,
            )))
            .insert_resource(BattleRuntime(battle))
            .insert_resource(blank_ui_assets())
            .insert_resource(BattleEventQueue(queue.into()))
            .insert_resource(CutInSettings { enabled })
            .init_resource::<ActiveCutIn>()
            .init_resource::<EventPlayback>()
            .init_resource::<super::super::RecentBattleLog>()
            .init_resource::<StatusMessage>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(
                Update,
                (
                    super::super::playback::play_battle_events,
                    stage_cut_ins,
                    animate_cut_in,
                )
                    .chain(),
            );
        app.world_mut().spawn(BattleStage);
        app
    }

    fn cut_in_count(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<CutInRoot>>()
            .iter(app.world())
            .count()
    }

    fn held_seconds(app: &App) -> Option<f32> {
        app.world()
            .resource::<EventPlayback>()
            .current
            .as_ref()
            .map(|(_, timer)| timer.duration().as_secs_f32())
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

    #[test]
    fn an_attack_holds_playback_for_one_cut_in_and_space_skips_it() {
        let (battle, events) = pile_lance_on_striker(7);
        let mut app = cut_in_app(battle, events, true);

        app.update();
        assert_eq!(cut_in_count(&mut app), 1);
        assert_eq!(held_seconds(&app), Some(CUT_IN_SECONDS));
        let snapshot = app.world().resource::<ActiveCutIn>().snapshot.clone();
        assert_eq!(
            snapshot.map(|snapshot| snapshot.weapon_name),
            Some("Pile Lance")
        );

        // Mid cut-in the band is open and still holding the roll.
        app.update();
        app.update();
        assert!(matches!(
            app.world().resource::<EventPlayback>().current,
            Some((BattleEvent::AttackRolled { .. }, _))
        ));
        assert_eq!(cut_in_count(&mut app), 1);

        tap(&mut app, KeyCode::Space);
        app.update();
        assert!(!matches!(
            app.world().resource::<EventPlayback>().current,
            Some((BattleEvent::AttackRolled { .. }, _))
        ));
        assert_eq!(cut_in_count(&mut app), 0);
        assert!(app.world().resource::<ActiveCutIn>().snapshot.is_none());
    }

    #[test]
    fn with_cut_ins_off_attacks_play_at_map_speed() {
        let (battle, events) = pile_lance_on_striker(7);
        let mut app = cut_in_app(battle, events, false);

        app.update();
        assert_eq!(cut_in_count(&mut app), 0);
        assert!(held_seconds(&app).is_some_and(|seconds| seconds < 0.5));
    }

    #[test]
    fn v_toggles_cut_ins_and_closes_one_in_progress() {
        let (battle, events) = pile_lance_on_striker(7);
        let mut app = cut_in_app(battle, events, true);
        app.update();
        assert_eq!(cut_in_count(&mut app), 1);

        tap(&mut app, KeyCode::KeyV);
        assert!(!app.world().resource::<CutInSettings>().enabled);
        assert_eq!(
            app.world().resource::<StatusMessage>().0,
            "Battle cut-ins off (V to turn on)."
        );
        app.update();
        assert_eq!(cut_in_count(&mut app), 0);

        tap(&mut app, KeyCode::KeyV);
        assert!(app.world().resource::<CutInSettings>().enabled);
    }

    #[test]
    fn one_cut_in_per_strike_and_counters_are_labelled() {
        let (battle, _) = pile_lance_on_striker(7);
        let roll = |attacker, weapon, target| BattleEvent::AttackRolled {
            attacker,
            weapon,
            target,
            roll: 99,
            hit: false,
            critical_roll: None,
            critical: false,
        };
        let queue = vec![
            roll(ids::VANGUARD, ids::PILE_LANCE, ids::STRIKER),
            roll(ids::VANGUARD, ids::PILE_LANCE, ids::RIFLEMAN_LEFT),
            BattleEvent::CounterFired {
                defender: ids::STRIKER,
                attacker: ids::VANGUARD,
                weapon: ids::SHOCK_CLAW,
            },
            roll(ids::STRIKER, ids::SHOCK_CLAW, ids::VANGUARD),
        ];
        let mut app = cut_in_app(battle, queue, true);

        assert_eq!(
            shown_cut_ins(&mut app),
            vec![
                ("Vanguard", "Striker", false),
                ("Striker", "Vanguard", true)
            ]
        );
        assert!(app.world().resource::<EventPlayback>().current.is_none());
        assert_eq!(cut_in_count(&mut app), 0);
    }

    #[test]
    fn an_identical_roll_after_a_cut_in_plays_on_its_own() {
        let (battle, _) = pile_lance_on_striker(7);
        let roll = BattleEvent::AttackRolled {
            attacker: ids::VANGUARD,
            weapon: ids::PILE_LANCE,
            target: ids::STRIKER,
            roll: 99,
            hit: false,
            critical_roll: None,
            critical: false,
        };
        let mut app = cut_in_app(battle, vec![roll.clone(), roll], true);

        app.update();
        assert_eq!(cut_in_count(&mut app), 1);
        while app.world().resource::<EventPlayback>().started < 2 {
            app.update();
        }
        // The second roll is a repeat of the same strike: no band, map speed.
        assert_eq!(cut_in_count(&mut app), 0);
        assert!(held_seconds(&app).is_some_and(|seconds| seconds < 0.5));
    }

    #[test]
    fn a_counter_between_blast_rolls_does_not_restart_the_blast() {
        let (battle, _) = pile_lance_on_striker(7);
        let roll = |attacker, weapon, target, hit| BattleEvent::AttackRolled {
            attacker,
            weapon,
            target,
            roll: if hit { 1 } else { 99 },
            hit,
            critical_roll: None,
            critical: false,
        };
        // An enemy blast resolves the counter its first hit draws before it
        // rolls on the next cell of its footprint.
        let queue = vec![
            roll(ids::ARTILLERY, ids::SIEGE_MORTAR, ids::GUNNER, true),
            BattleEvent::CounterFired {
                defender: ids::GUNNER,
                attacker: ids::ARTILLERY,
                weapon: ids::RAIL_RIFLE,
            },
            roll(ids::GUNNER, ids::RAIL_RIFLE, ids::ARTILLERY, false),
            roll(ids::ARTILLERY, ids::SIEGE_MORTAR, ids::VANGUARD, false),
        ];
        let mut app = cut_in_app(battle, queue, true);

        assert_eq!(
            shown_cut_ins(&mut app),
            vec![
                ("Artillery", "Gunner", false),
                ("Gunner", "Artillery", true)
            ]
        );
        assert!(app.world().resource::<EventPlayback>().current.is_none());
    }

    /// Plays the queue out and lists each cut-in as (attacker, target, counter).
    fn shown_cut_ins(app: &mut App) -> Vec<(&'static str, &'static str, bool)> {
        let mut shown = Vec::new();
        let mut last = None;
        for _ in 0..64 {
            app.update();
            let snapshot = app.world().resource::<ActiveCutIn>().snapshot.clone();
            if snapshot != last
                && let Some(snapshot) = &snapshot
            {
                shown.push((
                    snapshot.attacker_name,
                    snapshot.target_name,
                    snapshot.counter,
                ));
            }
            last = snapshot;
        }
        shown
    }

    #[test]
    fn bump_and_speed_lines_stay_in_range() {
        assert_eq!(bump(0.1, 0.2, 0.4), 0.0);
        assert!((bump(0.3, 0.2, 0.4) - 1.0).abs() < 1e-5);
        for line in 0..SPEED_LINES {
            for step in 0..=20 {
                let left = speed_line_left(line, step as f32 / 20.0);
                assert!((-300.0..BATTLE_STAGE_SIZE.x + 100.0).contains(&left));
            }
        }
    }
}
