//! Sound effects and music. Effects are Ogg files in `assets/audio/sfx/`,
//! generated with ElevenLabs sound effects; `synth.rs` renders the two music
//! tracks at startup, and this module wraps those samples as a Bevy audio
//! source. It plays effects for the cues that battle playback, cut-ins and
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
use super::synth::{SAMPLE_RATE, Track, render_track};
use super::{BattleEventQueue, BattleRuntime, EventPlayback};

const SFX_VOLUME: f32 = 0.7;
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
    Victory,
    Defeat,
}

impl Sfx {
    pub const ALL: [Sfx; 19] = [
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
        Sfx::Victory,
        Sfx::Defeat,
    ];

    /// The effect's file, relative to the `assets` folder.
    pub fn asset_path(self) -> &'static str {
        match self {
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
            Sfx::Victory => "audio/sfx/victory.ogg",
            Sfx::Defeat => "audio/sfx/defeat.ogg",
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
        BattleEvent::MissionCompleted { .. } => Sfx::Victory,
        BattleEvent::MissionFailed { .. } => Sfx::Defeat,
        BattleEvent::IntentCanceled { .. } => return None,
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

/// Handles to every effect file and rendered music track.
#[derive(Resource, Clone, Debug)]
pub struct SoundBank {
    pub sfx: HashMap<Sfx, Handle<AudioSource>>,
    pub tracks: HashMap<Track, Handle<SynthSound>>,
}

impl SoundBank {
    pub fn load(assets: &AssetServer, sounds: &mut Assets<SynthSound>) -> Self {
        Self {
            sfx: Sfx::ALL
                .into_iter()
                .map(|sfx| (sfx, assets.load(sfx.asset_path())))
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
    mut status: Option<ResMut<StatusMessage>>,
) {
    if !keyboard.is_some_and(|input| input.just_pressed(KeyCode::KeyN)) {
        return;
    }
    settings.enabled = !settings.enabled;
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
        if let Some(handle) = bank.sfx.get(&sfx) {
            commands.spawn((
                Name::new("Sound effect"),
                SfxPlayer(sfx),
                AudioPlayer(handle.clone()),
                PlaybackSettings::DESPAWN.with_volume(Volume::Linear(SFX_VOLUME)),
            ));
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
    playback: Option<Res<EventPlayback>>,
    players: Query<(Entity, &MusicPlayer)>,
) {
    let (Some(bank), Some(screen)) = (bank, screen) else {
        return;
    };
    let battle_over = battle.is_some_and(|battle| battle.0.result().is_some())
        && queue.is_none_or(|queue| queue.0.is_empty())
        && playback.is_none_or(|playback| playback.current.is_none());
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
            let path = format!("{ASSETS}/{}", sfx.asset_path());
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
            .init_resource::<BattleEventQueue>()
            .init_resource::<EventPlayback>();
        app.world_mut()
            .resource_mut::<NextState<GameScreen>>()
            .set(GameScreen::Battle);
        app.update();
        app.update();
        assert_eq!(music(&mut app), vec![Track::Battle]);
    }
}
