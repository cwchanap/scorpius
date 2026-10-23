//! Sound effects and music. Effects are Ogg files in `assets/audio/sfx/`,
//! generated with ElevenLabs sound effects; `synth.rs` renders the two music
//! tracks and the victory and defeat jingles at startup, and this module
//! wraps those samples as a Bevy audio source. It plays effects for the cues that battle playback, cut-ins and
//! buttons leave in [`SoundCues`], and keeps the right music track looping
//! for the current screen.
//!
//! Presentation only: no domain state or RNG changes. Bare test apps without
//! [`SoundPlugin`] have no `SoundCues`, so playback stays silent there.

use std::collections::HashMap;
use std::num::NonZero;
use std::sync::Arc;
use std::time::Duration;

use bevy::audio::{
    AddAudioSource, AudioSink, AudioSinkPlayback, AudioSource, ChannelCount, Decodable, SampleRate,
    Source, Volume,
};
use bevy::prelude::*;

use crate::app::GameScreen;
use crate::domain::model::BattleEvent;

use super::interaction::StatusMessage;
use super::synth::{Jingle, SAMPLE_RATE, Track, render_jingle, render_track};
use super::{BattleEventQueue, BattleRuntime};

const SFX_VOLUME: f32 = 0.7;
/// The jingles sit a little above the effects, since nothing plays under them.
const JINGLE_VOLUME: f32 = 1.0;
const MUSIC_VOLUME: f32 = 0.35;

/// One-shot sound effects, each tied to a battle event or UI action.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Sfx {
    Select,
    Hover,
    Move,
    Hit,
    Critical,
    Miss,
    Impact,
    KnockOut,
    Push,
    Collision,
    Hazard,
    Explosion,
    Telegraph,
    Counter,
    CutIn,
    Reinforcement,
    Objective,
    Spirit,
    Victory,
    Defeat,
}

impl Sfx {
    pub const ALL: [Sfx; 20] = [
        Sfx::Select,
        Sfx::Hover,
        Sfx::Move,
        Sfx::Hit,
        Sfx::Critical,
        Sfx::Miss,
        Sfx::Impact,
        Sfx::KnockOut,
        Sfx::Push,
        Sfx::Collision,
        Sfx::Hazard,
        Sfx::Explosion,
        Sfx::Telegraph,
        Sfx::Counter,
        Sfx::CutIn,
        Sfx::Reinforcement,
        Sfx::Objective,
        Sfx::Spirit,
        Sfx::Victory,
        Sfx::Defeat,
    ];

    /// The effect's file, relative to the `assets` folder. Victory and
    /// defeat play a synthesized [`Jingle`] instead.
    pub fn asset_path(self) -> Option<&'static str> {
        Some(match self {
            Sfx::Select => "audio/sfx/select.ogg",
            Sfx::Hover => "audio/sfx/hover.ogg",
            Sfx::Move => "audio/sfx/move.ogg",
            Sfx::Hit => "audio/sfx/hit.ogg",
            Sfx::Critical => "audio/sfx/critical.ogg",
            Sfx::Miss => "audio/sfx/miss.ogg",
            Sfx::Impact => "audio/sfx/impact.ogg",
            Sfx::KnockOut => "audio/sfx/knock_out.ogg",
            Sfx::Push => "audio/sfx/push.ogg",
            Sfx::Collision => "audio/sfx/collision.ogg",
            Sfx::Hazard => "audio/sfx/hazard.ogg",
            Sfx::Explosion => "audio/sfx/explosion.ogg",
            Sfx::Telegraph => "audio/sfx/telegraph.ogg",
            Sfx::Counter => "audio/sfx/counter.ogg",
            Sfx::CutIn => "audio/sfx/cut_in.ogg",
            Sfx::Reinforcement => "audio/sfx/reinforcement.ogg",
            Sfx::Objective => "audio/sfx/objective.ogg",
            Sfx::Spirit => "audio/sfx/spirit.ogg",
            Sfx::Victory | Sfx::Defeat => return None,
        })
    }

    pub fn jingle(self) -> Option<Jingle> {
        match self {
            Sfx::Victory => Some(Jingle::Victory),
            Sfx::Defeat => Some(Jingle::Defeat),
            _ => None,
        }
    }
}

/// Rendered mono music samples, played through Bevy's audio output.
#[derive(Asset, TypePath, Clone, Debug)]
pub struct SynthSound {
    samples: Arc<[f32]>,
}

impl SynthSound {
    pub fn new(samples: Vec<f32>) -> Self {
        Self {
            samples: samples.into(),
        }
    }
}

pub struct SynthDecoder {
    samples: Arc<[f32]>,
    index: usize,
}

impl Iterator for SynthDecoder {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.samples.get(self.index).copied();
        self.index += 1;
        sample
    }
}

impl Source for SynthDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZero::<u16>::MIN
    }

    fn sample_rate(&self) -> SampleRate {
        NonZero::new(SAMPLE_RATE).expect("sample rate is non-zero")
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(
            self.samples.len() as f64 / f64::from(SAMPLE_RATE),
        ))
    }
}

impl Decodable for SynthSound {
    type Decoder = SynthDecoder;

    fn decoder(&self) -> Self::Decoder {
        SynthDecoder {
            samples: self.samples.clone(),
            index: 0,
        }
    }
}

/// Player preference for sound; on by default, and `N` toggles it anywhere.
#[derive(Resource, Clone, Copy, Debug, Eq, PartialEq)]
pub struct SoundSettings {
    pub enabled: bool,
}

impl Default for SoundSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Effects waiting to play, each with the seconds left before it starts.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct SoundCues(pub Vec<(Sfx, f32)>);

impl SoundCues {
    pub fn play(&mut self, sfx: Sfx) {
        self.0.push((sfx, 0.0));
    }

    pub fn battle_event(&mut self, event: &BattleEvent) {
        if let Some(sfx) = sfx_for_event(event) {
            self.play(sfx);
        }
    }

    /// A cut-in opens with its whoosh and holds the strike's hit or miss
    /// until the blow lands in the band, `delay` seconds later.
    pub fn hold_strike_for_cut_in(&mut self, delay: f32) {
        for (sfx, wait) in &mut self.0 {
            if matches!(sfx, Sfx::Hit | Sfx::Critical | Sfx::Miss) && *wait == 0.0 {
                *wait = delay;
            }
        }
        self.play(Sfx::CutIn);
    }

    /// A skipped cut-in plays its held strike right away.
    pub fn release_held(&mut self) {
        for (_, wait) in &mut self.0 {
            *wait = 0.0;
        }
    }
}

/// The effect a battle event plays as playback reaches it, if any.
pub fn sfx_for_event(event: &BattleEvent) -> Option<Sfx> {
    Some(match event {
        BattleEvent::UnitMoved { .. } => Sfx::Move,
        BattleEvent::AttackRolled { hit: false, .. } => Sfx::Miss,
        BattleEvent::AttackRolled { critical: true, .. } => Sfx::Critical,
        BattleEvent::AttackRolled { .. } => Sfx::Hit,
        BattleEvent::AttackHitEmpty { .. } => Sfx::Miss,
        BattleEvent::DamageApplied { .. } | BattleEvent::ExplosiveDamaged { .. } => Sfx::Impact,
        BattleEvent::UnitKnockedOut { .. } => Sfx::KnockOut,
        BattleEvent::UnitPushed { .. } => Sfx::Push,
        BattleEvent::CollisionOccurred { .. } => Sfx::Collision,
        BattleEvent::HazardTriggered { .. } => Sfx::Hazard,
        BattleEvent::ExplosionTriggered { .. } => Sfx::Explosion,
        BattleEvent::IntentCommitted { .. } => Sfx::Telegraph,
        BattleEvent::CounterFired { .. } => Sfx::Counter,
        BattleEvent::ReinforcementArrived { .. } => Sfx::Reinforcement,
        BattleEvent::OptionalObjectiveCompleted => Sfx::Objective,
        BattleEvent::SpiritInvoked { .. } => Sfx::Spirit,
        BattleEvent::MissionCompleted { .. } => Sfx::Victory,
        BattleEvent::MissionFailed { .. } => Sfx::Defeat,
        BattleEvent::IntentCanceled { .. } | BattleEvent::UnitRepaired { .. } => return None,
    })
}

/// The track a screen plays. A finished battle goes quiet once its last
/// event has played, so the victory or defeat sting stands alone.
pub fn track_for(screen: GameScreen, battle_over: bool) -> Option<Track> {
    match screen {
        GameScreen::Battle if battle_over => None,
        GameScreen::Battle => Some(Track::Battle),
        GameScreen::Title
        | GameScreen::PreMissionStory
        | GameScreen::Briefing
        | GameScreen::Aftermath
        | GameScreen::Upgrade
        | GameScreen::Ending => Some(Track::Theme),
    }
}

/// Handles to every effect file, rendered jingle and music track.
#[derive(Resource, Clone, Debug)]
pub struct SoundBank {
    pub sfx: HashMap<Sfx, Handle<AudioSource>>,
    pub jingles: HashMap<Sfx, Handle<SynthSound>>,
    pub tracks: HashMap<Track, Handle<SynthSound>>,
}

impl SoundBank {
    pub fn load(assets: &AssetServer, sounds: &mut Assets<SynthSound>) -> Self {
        Self {
            sfx: Sfx::ALL
                .into_iter()
                .filter_map(|sfx| Some((sfx, assets.load(sfx.asset_path()?))))
                .collect(),
            jingles: Sfx::ALL
                .into_iter()
                .filter_map(|sfx| {
                    let samples = render_jingle(sfx.jingle()?);
                    Some((sfx, sounds.add(SynthSound::new(samples))))
                })
                .collect(),
            tracks: Track::ALL
                .into_iter()
                .map(|track| (track, sounds.add(SynthSound::new(render_track(track)))))
                .collect(),
        }
    }
}

/// The looping music entity and the track it plays.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct MusicPlayer(pub Track);

/// A one-shot effect entity, despawned when it finishes.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct SfxPlayer(pub Sfx);

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<SynthSound>()
            .init_resource::<SoundSettings>()
            .init_resource::<SoundCues>()
            .add_systems(Startup, setup_sound_bank)
            .add_systems(Update, toggle_sound)
            .add_systems(PostUpdate, (play_sound_cues, sync_music))
            .add_observer(cue_button_click)
            .add_observer(cue_button_hover);
    }
}

pub fn setup_sound_bank(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut sounds: ResMut<Assets<SynthSound>>,
) {
    commands.insert_resource(SoundBank::load(&assets, &mut sounds));
}

pub fn toggle_sound(
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    mut settings: ResMut<SoundSettings>,
    mut sinks: Query<&mut AudioSink>,
    mut pending: Query<&mut PlaybackSettings, Without<AudioSink>>,
    mut status: Option<ResMut<StatusMessage>>,
) {
    if !keyboard.is_some_and(|input| input.just_pressed(KeyCode::KeyN)) {
        return;
    }
    settings.enabled = !settings.enabled;
    // A player whose asset is still loading has no sink yet; it starts
    // from its playback settings once the asset is ready.
    for mut playback in &mut pending {
        playback.muted = !settings.enabled;
    }
    for mut sink in &mut sinks {
        if settings.enabled {
            sink.unmute();
        } else {
            sink.mute();
        }
    }
    if let Some(status) = status.as_deref_mut() {
        status.0 = if settings.enabled {
            "Sound on (N to mute).".to_owned()
        } else {
            "Sound off (N to turn on).".to_owned()
        };
    }
}

pub fn play_sound_cues(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<SoundSettings>,
    bank: Option<Res<SoundBank>>,
    mut cues: ResMut<SoundCues>,
) {
    let delta = time.delta_secs();
    let mut due = Vec::new();
    cues.0.retain_mut(|(sfx, wait)| {
        if *wait <= 0.0 {
            due.push(*sfx);
            false
        } else {
            *wait -= delta;
            true
        }
    });
    if !settings.enabled {
        return;
    }
    let Some(bank) = bank else {
        return;
    };
    due.dedup();
    for sfx in due {
        let playback = |volume| PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume));
        let mut player = commands.spawn((Name::new("Sound effect"), SfxPlayer(sfx)));
        if let Some(handle) = bank.sfx.get(&sfx) {
            player.insert((AudioPlayer(handle.clone()), playback(SFX_VOLUME)));
        } else if let Some(handle) = bank.jingles.get(&sfx) {
            player.insert((AudioPlayer(handle.clone()), playback(JINGLE_VOLUME)));
        } else {
            player.despawn();
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn sync_music(
    mut commands: Commands,
    settings: Res<SoundSettings>,
    bank: Option<Res<SoundBank>>,
    screen: Option<Res<State<GameScreen>>>,
    battle: Option<Res<BattleRuntime>>,
    queue: Option<Res<BattleEventQueue>>,
    players: Query<(Entity, &MusicPlayer)>,
    effects: Query<(Entity, &SfxPlayer)>,
) {
    let (Some(bank), Some(screen)) = (bank, screen) else {
        return;
    };
    // The result event is always the last one queued, so an empty queue
    // means its jingle has just started: the battle music stops with it.
    let battle_over = battle.is_some_and(|battle| battle.0.result().is_some())
        && queue.is_none_or(|queue| queue.0.is_empty());
    let wanted = track_for(*screen.get(), battle_over);
    let mut playing = false;
    for (entity, player) in &players {
        if Some(player.0) == wanted && !playing {
            playing = true;
        } else {
            commands.entity(entity).try_despawn();
        }
    }
    if playing {
        return;
    }
    let Some(track) = wanted else {
        return;
    };
    let Some(handle) = bank.tracks.get(&track) else {
        return;
    };
    // New music retires a result jingle, so a quick retry or continue
    // doesn't play the next screen's track over it.
    for (entity, effect) in &effects {
        if effect.0.jingle().is_some() {
            commands.entity(entity).try_despawn();
        }
    }
    let mut playback = PlaybackSettings::LOOP.with_volume(Volume::Linear(MUSIC_VOLUME));
    if !settings.enabled {
        playback = playback.muted();
    }
    commands.spawn((
        Name::new("Music"),
        MusicPlayer(track),
        AudioPlayer(handle.clone()),
        playback,
    ));
}

fn cue_button_click(
    click: On<Pointer<Click>>,
    buttons: Query<(), With<Button>>,
    cues: Option<ResMut<SoundCues>>,
) {
    if buttons.contains(click.entity)
        && let Some(mut cues) = cues
    {
        cues.play(Sfx::Select);
    }
}

fn cue_button_hover(
    over: On<Pointer<Over>>,
    buttons: Query<(), With<Button>>,
    cues: Option<ResMut<SoundCues>>,
) {
    if buttons.contains(over.entity)
        && let Some(mut cues) = cues
    {
        cues.play(Sfx::Hover);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::NormalizedRenderTarget;
    use bevy::picking::backend::HitData;
    use bevy::picking::pointer::{Location, PointerId};

    use crate::domain::board::GridPos;
    use crate::domain::combat::DamageSource;
    use crate::domain::model::{MissionResult, UnitId, WeaponId};

    const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");

    fn attack(hit: bool, critical: bool) -> BattleEvent {
        BattleEvent::AttackRolled {
            attacker: UnitId(1),
            weapon: WeaponId(1),
            target: UnitId(2),
            roll: 10,
            hit,
            critical_roll: None,
            critical,
        }
    }

    fn sound_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: ASSETS.into(),
                ..default()
            },
            bevy::state::app::StatesPlugin,
        ))
        .init_asset::<SynthSound>()
        .init_asset::<AudioSource>()
        .init_asset_loader::<bevy::audio::AudioLoader>()
        .init_resource::<SoundSettings>()
        .init_resource::<SoundCues>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_systems(Startup, setup_sound_bank)
        .add_systems(Update, toggle_sound)
        .add_systems(PostUpdate, (play_sound_cues, sync_music));
        app
    }

    fn effects(app: &mut App) -> Vec<Sfx> {
        let mut query = app.world_mut().query::<&SfxPlayer>();
        query.iter(app.world()).map(|player| player.0).collect()
    }

    fn music(app: &mut App) -> Vec<Track> {
        let mut query = app.world_mut().query::<&MusicPlayer>();
        query.iter(app.world()).map(|player| player.0).collect()
    }

    fn press_n(app: &mut App) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyN);
        app.update();
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.release(KeyCode::KeyN);
        input.clear();
    }

    #[test]
    fn attacks_sound_their_result() {
        assert_eq!(sfx_for_event(&attack(false, false)), Some(Sfx::Miss));
        assert_eq!(sfx_for_event(&attack(true, false)), Some(Sfx::Hit));
        assert_eq!(sfx_for_event(&attack(true, true)), Some(Sfx::Critical));
        assert_eq!(
            sfx_for_event(&BattleEvent::MissionCompleted {
                result: MissionResult {
                    victory: true,
                    optional_complete: false,
                    rounds: 3,
                }
            }),
            Some(Sfx::Victory)
        );
        assert_eq!(
            sfx_for_event(&BattleEvent::IntentCanceled {
                attacker: UnitId(1)
            }),
            None
        );
    }

    #[test]
    fn every_effect_file_is_a_short_mono_ogg() {
        for sfx in Sfx::ALL {
            let Some(file) = sfx.asset_path() else {
                assert!(sfx.jingle().is_some(), "{sfx:?} has no sound");
                continue;
            };
            let path = format!("{ASSETS}/{file}");
            let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("{path} is missing"));
            let decoder = AudioSource {
                bytes: bytes.into(),
            }
            .decoder();
            assert_eq!(decoder.channels().get(), 1, "{sfx:?} is not mono");
            let rate = decoder.sample_rate().get() as f32;
            let samples: Vec<f32> = decoder.collect();
            let seconds = samples.len() as f32 / rate;
            assert!((0.05..2.0).contains(&seconds), "{sfx:?} lasts {seconds}s");
            let peak = samples.iter().fold(0.0f32, |peak, s| peak.max(s.abs()));
            assert!((0.05..=1.0).contains(&peak), "{sfx:?} peaks at {peak}");
        }
    }

    #[test]
    fn every_battle_event_plays_its_effect() {
        let unit = UnitId(1);
        let weapon = WeaponId(1);
        let cell = GridPos { x: 1, y: 2 };
        let result = MissionResult {
            victory: false,
            optional_complete: false,
            rounds: 4,
        };
        let cases = [
            (
                BattleEvent::UnitMoved {
                    unit,
                    from: cell,
                    to: cell,
                },
                Sfx::Move,
            ),
            (
                BattleEvent::AttackHitEmpty {
                    attacker: unit,
                    weapon,
                    cell,
                },
                Sfx::Miss,
            ),
            (
                BattleEvent::DamageApplied {
                    target: unit,
                    amount: 3,
                    remaining_hp: 5,
                    source: DamageSource::Collision,
                },
                Sfx::Impact,
            ),
            (
                BattleEvent::ExplosiveDamaged {
                    position: cell,
                    amount: 3,
                    remaining_hp: 0,
                    source: DamageSource::Hazard,
                },
                Sfx::Impact,
            ),
            (
                BattleEvent::UnitKnockedOut {
                    unit,
                    position: cell,
                },
                Sfx::KnockOut,
            ),
            (
                BattleEvent::UnitPushed {
                    unit,
                    from: cell,
                    to: cell,
                },
                Sfx::Push,
            ),
            (
                BattleEvent::CollisionOccurred {
                    unit,
                    blocked_at: cell,
                },
                Sfx::Collision,
            ),
            (
                BattleEvent::HazardTriggered {
                    unit,
                    position: cell,
                },
                Sfx::Hazard,
            ),
            (
                BattleEvent::ExplosionTriggered {
                    position: cell,
                    footprint: vec![cell],
                },
                Sfx::Explosion,
            ),
            (
                BattleEvent::IntentCommitted {
                    attacker: unit,
                    weapon,
                    footprint: vec![cell],
                    intended_occupant: None,
                },
                Sfx::Telegraph,
            ),
            (
                BattleEvent::CounterFired {
                    defender: unit,
                    attacker: UnitId(2),
                    weapon,
                },
                Sfx::Counter,
            ),
            (
                BattleEvent::ReinforcementArrived {
                    unit,
                    position: cell,
                },
                Sfx::Reinforcement,
            ),
            (BattleEvent::OptionalObjectiveCompleted, Sfx::Objective),
            (
                BattleEvent::SpiritInvoked {
                    unit,
                    command: crate::domain::spirit::SpiritCommand::Valor,
                },
                Sfx::Spirit,
            ),
            (BattleEvent::MissionFailed { result }, Sfx::Defeat),
        ];
        for (event, sfx) in cases {
            assert_eq!(sfx_for_event(&event), Some(sfx), "{event:?}");
        }
    }

    #[test]
    fn buttons_click_and_tick_on_hover() {
        let mut app = sound_app();
        app.add_observer(cue_button_click)
            .add_observer(cue_button_hover);
        app.update();
        let button = app.world_mut().spawn(Button).id();
        let label = app.world_mut().spawn_empty().id();
        let location = Location {
            target: NormalizedRenderTarget::None {
                width: 1920,
                height: 1080,
            },
            position: Vec2::ZERO,
        };
        let hit = HitData::new(Entity::PLACEHOLDER, 0.0, None, None);
        for target in [label, button] {
            app.world_mut().trigger(Pointer::new(
                PointerId::Mouse,
                location.clone(),
                Click {
                    button: PointerButton::Primary,
                    hit: hit.clone(),
                    duration: Duration::ZERO,
                    count: 1,
                },
                target,
            ));
            app.world_mut().trigger(Pointer::new(
                PointerId::Mouse,
                location.clone(),
                Over { hit: hit.clone() },
                target,
            ));
        }
        assert_eq!(
            app.world().resource::<SoundCues>().0,
            vec![(Sfx::Select, 0.0), (Sfx::Hover, 0.0)]
        );
    }

    #[test]
    fn the_plugin_loads_the_bank_and_plays_the_theme() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: ASSETS.into(),
                ..default()
            },
            bevy::state::app::StatesPlugin,
            bevy::audio::AudioPlugin::default(),
            SoundPlugin,
        ))
        .init_state::<GameScreen>();
        app.update();
        app.update();
        let bank = app.world().resource::<SoundBank>();
        assert_eq!(bank.sfx.len() + bank.jingles.len(), Sfx::ALL.len());
        assert_eq!(bank.jingles.len(), Jingle::ALL.len());
        assert_eq!(bank.tracks.len(), Track::ALL.len());
        assert_eq!(music(&mut app), vec![Track::Theme]);
    }

    #[test]
    fn music_decodes_as_mono_at_the_synth_rate() {
        let decoder = SynthSound::new(vec![0.25; SAMPLE_RATE as usize / 2]).decoder();
        assert_eq!(decoder.channels().get(), 1);
        assert_eq!(decoder.sample_rate().get(), SAMPLE_RATE);
        assert_eq!(decoder.current_span_len(), None);
        assert_eq!(decoder.total_duration(), Some(Duration::from_millis(500)));
        assert_eq!(decoder.count(), SAMPLE_RATE as usize / 2);
    }

    #[test]
    fn battle_music_stops_once_the_battle_has_played_out() {
        assert_eq!(track_for(GameScreen::Title, false), Some(Track::Theme));
        assert_eq!(track_for(GameScreen::Upgrade, true), Some(Track::Theme));
        assert_eq!(track_for(GameScreen::Battle, false), Some(Track::Battle));
        assert_eq!(track_for(GameScreen::Battle, true), None);
    }

    #[test]
    fn a_cut_in_holds_the_strike_until_the_blow_lands() {
        let mut cues = SoundCues::default();
        cues.battle_event(&attack(true, false));
        cues.hold_strike_for_cut_in(1.0);
        assert_eq!(cues.0, vec![(Sfx::Hit, 1.0), (Sfx::CutIn, 0.0)]);
        cues.release_held();
        assert_eq!(cues.0, vec![(Sfx::Hit, 0.0), (Sfx::CutIn, 0.0)]);
    }

    #[test]
    fn cues_spawn_effects_and_wait_out_their_delay() {
        let mut app = sound_app();
        app.update();
        {
            let mut cues = app.world_mut().resource_mut::<SoundCues>();
            cues.play(Sfx::Select);
            cues.0.push((Sfx::Hit, 60.0));
        }
        app.update();
        assert_eq!(effects(&mut app), vec![Sfx::Select]);
        assert_eq!(app.world().resource::<SoundCues>().0.len(), 1);
    }

    #[test]
    fn victory_plays_its_jingle_louder_than_the_effects() {
        let mut app = sound_app();
        app.update();
        app.world_mut()
            .resource_mut::<SoundCues>()
            .play(Sfx::Victory);
        app.update();
        let mut query = app
            .world_mut()
            .query::<(&SfxPlayer, &AudioPlayer<SynthSound>, &PlaybackSettings)>();
        let (player, _, settings) = query.single(app.world()).expect("the jingle plays");
        assert_eq!(player.0, Sfx::Victory);
        assert_eq!(settings.volume, Volume::Linear(JINGLE_VOLUME));
    }

    #[test]
    fn muting_drops_effects_and_mutes_new_music() {
        let mut app = sound_app();
        app.init_resource::<StatusMessage>();
        app.update();
        press_n(&mut app);
        assert!(!app.world().resource::<SoundSettings>().enabled);
        assert_eq!(
            app.world().resource::<StatusMessage>().0,
            "Sound off (N to turn on)."
        );
        app.world_mut()
            .resource_mut::<SoundCues>()
            .play(Sfx::Select);
        app.update();
        assert!(effects(&mut app).is_empty());
        assert!(app.world().resource::<SoundCues>().0.is_empty());

        app.insert_state(GameScreen::Title);
        app.update();
        app.update();
        let mut query = app.world_mut().query::<(&MusicPlayer, &PlaybackSettings)>();
        let (player, settings) = query.single(app.world()).expect("theme plays muted");
        assert_eq!(player.0, Track::Theme);
        assert!(settings.muted);

        press_n(&mut app);
        assert!(app.world().resource::<SoundSettings>().enabled);
        assert_eq!(
            app.world().resource::<StatusMessage>().0,
            "Sound on (N to mute)."
        );
    }

    #[test]
    fn toggling_off_mutes_music_that_is_still_loading() {
        let mut app = sound_app();
        app.init_state::<GameScreen>();
        app.update();
        app.update();
        let muted = |app: &mut App| {
            let mut query = app
                .world_mut()
                .query_filtered::<&PlaybackSettings, With<MusicPlayer>>();
            query.single(app.world()).expect("theme is queued").muted
        };
        assert!(!muted(&mut app));
        press_n(&mut app);
        assert!(muted(&mut app));
        press_n(&mut app);
        assert!(!muted(&mut app));
    }

    #[test]
    fn a_result_jingle_plays_alone_until_the_next_music_starts() {
        use crate::mission::mission_one::{ids, mission_one};

        let mut app = sound_app();
        app.init_state::<GameScreen>()
            .insert_resource(BattleRuntime(mission_one(7)))
            .init_resource::<BattleEventQueue>();
        app.world_mut()
            .resource_mut::<NextState<GameScreen>>()
            .set(GameScreen::Battle);
        app.update();
        app.update();
        assert_eq!(music(&mut app), vec![Track::Battle]);

        let mut events = {
            let battle = &mut app.world_mut().resource_mut::<BattleRuntime>().0;
            battle.clear_reinforcements_for_test();
            let mut events = Vec::new();
            for enemy in [
                ids::RIFLEMAN_LEFT,
                ids::RIFLEMAN_RIGHT,
                ids::STRIKER,
                ids::ARTILLERY,
            ] {
                events.extend(battle.apply_direct_damage(enemy, 99, DamageSource::Hazard));
            }
            events
        };
        let result = events.pop().expect("the battle ends");
        assert!(matches!(result, BattleEvent::MissionCompleted { .. }));
        // The battle music keeps playing until the result event plays.
        app.world_mut()
            .resource_mut::<BattleEventQueue>()
            .0
            .push_back(result.clone());
        app.update();
        assert_eq!(music(&mut app), vec![Track::Battle]);

        app.world_mut().resource_mut::<BattleEventQueue>().0.clear();
        app.world_mut()
            .resource_mut::<SoundCues>()
            .battle_event(&result);
        app.update();
        assert!(music(&mut app).is_empty());
        assert_eq!(effects(&mut app), vec![Sfx::Victory]);

        // A quick retry starts the battle music and retires the jingle.
        app.insert_resource(BattleRuntime(mission_one(8)));
        app.world_mut().resource_mut::<SoundCues>().play(Sfx::Hit);
        app.update();
        app.update();
        assert_eq!(music(&mut app), vec![Track::Battle]);
        assert_eq!(effects(&mut app), vec![Sfx::Hit]);
    }

    #[test]
    fn music_follows_the_screen() {
        let mut app = sound_app();
        app.init_state::<GameScreen>();
        app.update();
        app.update();
        assert_eq!(music(&mut app), vec![Track::Theme]);

        let battle = crate::domain::battle::BattleState::viability_fixture();
        app.insert_resource(BattleRuntime(battle))
            .init_resource::<BattleEventQueue>();
        app.world_mut()
            .resource_mut::<NextState<GameScreen>>()
            .set(GameScreen::Battle);
        app.update();
        app.update();
        assert_eq!(music(&mut app), vec![Track::Battle]);
    }
}
