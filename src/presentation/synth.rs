//! Plain-Rust synthesizer for the game's music.
//!
//! Both looping tracks and the victory and defeat jingles are typed recipes
//! rendered to mono samples at startup, so no music files ship and the output
//! is identical on every run. Nothing here touches Bevy; `sound.rs` wraps the
//! samples as an asset. Sound effects are recorded files instead
//! (`assets/audio/sfx/`).

use std::f32::consts::TAU;

pub const SAMPLE_RATE: u32 = 44_100;

/// Looping music: a calm theme for the campaign screens, a driving one for battle.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Track {
    Theme,
    Battle,
}

impl Track {
    pub const ALL: [Track; 2] = [Track::Theme, Track::Battle];

    /// Exact loop length in samples, so the seam lands on a bar line.
    pub fn samples(self) -> usize {
        let song = self.song();
        song.bars() * 16 * song.step_samples()
    }
}

/// One-shot music for a battle's end, about five seconds long.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Jingle {
    /// "Turnabout"'s opening phrase over Bb and C, landing on a bright D major.
    Victory,
    /// "Relay Nine"'s opening phrase slowed down, sinking to a low A minor.
    Defeat,
}

impl Jingle {
    pub const ALL: [Jingle; 2] = [Jingle::Victory, Jingle::Defeat];
}

#[derive(Clone, Copy)]
enum Wave {
    Sine,
    Triangle,
    Square,
    Saw,
}

fn wave(shape: Wave, phase: f32) -> f32 {
    match shape {
        Wave::Sine => (phase * TAU).sin(),
        Wave::Triangle => 4.0 * (phase - (phase + 0.5).floor()).abs() - 1.0,
        Wave::Square => {
            if phase.fract() < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Wave::Saw => 2.0 * phase.fract() - 1.0,
    }
}

#[derive(Clone, Copy)]
enum Envelope {
    /// Rises over `attack` seconds, then decays to silence by the note's end.
    Pluck { attack: f32 },
    /// Rises over `attack`, holds, and fades over the last `release` seconds.
    Pad { attack: f32, release: f32 },
}

impl Envelope {
    fn gain(self, t: f32, len: f32) -> f32 {
        // Every note fades its last 4 ms so it never ends on a click.
        let tail = ((len - t) / 0.004).clamp(0.0, 1.0);
        let shape = match self {
            Envelope::Pluck { attack } => {
                if t < attack {
                    t / attack
                } else {
                    (-6.9 * (t - attack) / (len - attack).max(1e-3)).exp()
                }
            }
            Envelope::Pad { attack, release } => {
                (t / attack).min(1.0) * ((len - t) / release).clamp(0.0, 1.0)
            }
        };
        shape * tail
    }
}

#[derive(Clone, Copy)]
struct Voice {
    wave: Wave,
    from: f32,
    to: f32,
    gain: f32,
    envelope: Envelope,
    /// Vibrato rate in Hz and depth in semitones.
    vibrato: (f32, f32),
    /// One-pole low-pass cutoff in Hz, or none.
    cutoff: Option<f32>,
}

impl Voice {
    fn new(wave: Wave, from: f32, to: f32, gain: f32) -> Self {
        Self {
            wave,
            from,
            to,
            gain,
            envelope: Envelope::Pluck { attack: 0.002 },
            vibrato: (0.0, 0.0),
            cutoff: None,
        }
    }

    fn envelope(mut self, envelope: Envelope) -> Self {
        self.envelope = envelope;
        self
    }

    fn vibrato(mut self, rate: f32, semitones: f32) -> Self {
        self.vibrato = (rate, semitones);
        self
    }

    fn cutoff(mut self, hz: f32) -> Self {
        self.cutoff = Some(hz);
        self
    }
}

#[derive(Clone, Copy)]
enum Filter {
    Low(f32),
    High(f32),
}

/// A mono buffer; music buffers wrap note tails past the end back to the
/// start so the loop seam carries the release of the last bar.
struct Buffer {
    samples: Vec<f32>,
    wrap: bool,
}

impl Buffer {
    fn new(len: usize, wrap: bool) -> Self {
        Self {
            samples: vec![0.0; len],
            wrap,
        }
    }

    fn add(&mut self, index: usize, value: f32) {
        let len = self.samples.len();
        if index < len {
            self.samples[index] += value;
        } else if self.wrap {
            self.samples[index % len] += value;
        }
    }

    fn tone(&mut self, at: f32, len: f32, voice: Voice) {
        let rate = SAMPLE_RATE as f32;
        let start = (at * rate) as usize;
        let count = (len * rate) as usize;
        let alpha = voice.cutoff.map(one_pole);
        let mut phase = 0.0_f32;
        let mut low = 0.0_f32;
        for i in 0..count {
            let t = i as f32 / rate;
            let progress = t / len;
            // Exponential glide sounds even across octaves; held notes skip it.
            let mut freq = if voice.from == voice.to {
                voice.from
            } else {
                voice.from * (voice.to / voice.from).powf(progress)
            };
            let (vib_rate, vib_depth) = voice.vibrato;
            if vib_depth > 0.0 {
                // Vibrato eases in so short notes stay steady.
                let depth = vib_depth * (t / 0.15).min(1.0);
                freq *= 2f32.powf(depth * (t * vib_rate * TAU).sin() / 12.0);
            }
            phase = (phase + freq / rate).fract();
            let mut sample = wave(voice.wave, phase);
            if let Some(alpha) = alpha {
                low += alpha * (sample - low);
                sample = low;
            }
            self.add(start + i, sample * voice.gain * voice.envelope.gain(t, len));
        }
    }

    fn noise(
        &mut self,
        at: f32,
        len: f32,
        gain: f32,
        envelope: Envelope,
        filter: Filter,
        seed: u32,
    ) {
        let rate = SAMPLE_RATE as f32;
        let start = (at * rate) as usize;
        let count = (len * rate) as usize;
        let mut rng = seed.max(1);
        let mut low = 0.0_f32;
        let (alpha, high_pass) = match filter {
            Filter::Low(hz) => (one_pole(hz), false),
            Filter::High(hz) => (one_pole(hz), true),
        };
        for i in 0..count {
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            let white = rng as f32 / u32::MAX as f32 * 2.0 - 1.0;
            low += alpha * (white - low);
            let sample = if high_pass { white - low } else { low };
            let t = i as f32 / rate;
            self.add(start + i, sample * gain * envelope.gain(t, len));
        }
    }

    /// Soft-clips the mix so stacked voices never exceed full scale.
    fn finish(self) -> Vec<f32> {
        self.samples.into_iter().map(f32::tanh).collect()
    }
}

fn one_pole(cutoff: f32) -> f32 {
    1.0 - (-TAU * cutoff / SAMPLE_RATE as f32).exp()
}

fn midi(note: i32) -> f32 {
    440.0 * 2f32.powf((note - 69) as f32 / 12.0)
}

/// A chord by its bass root (MIDI, octave 1–2) and quality.
#[derive(Clone, Copy)]
struct Chord {
    root: i32,
    minor: bool,
}

const fn minor(root: i32) -> Chord {
    Chord { root, minor: true }
}

const fn major(root: i32) -> Chord {
    Chord { root, minor: false }
}

impl Chord {
    /// Root, third and fifth, `octaves` above the bass root.
    fn triad(self, octaves: i32) -> [i32; 3] {
        let root = self.root + 12 * octaves;
        [root, root + if self.minor { 3 } else { 4 }, root + 7]
    }
}

/// A melody note: bar, start step (16ths), length in steps, MIDI pitch.
type Note = (usize, usize, usize, i32);

struct Song {
    bpm: f32,
    chords: &'static [Chord],
    melody: &'static [Note],
}

impl Song {
    fn bars(&self) -> usize {
        self.chords.len()
    }

    fn step_samples(&self) -> usize {
        (60.0 / self.bpm / 4.0 * SAMPLE_RATE as f32).round() as usize
    }
}

const A1: i32 = 33;
const BB1: i32 = 34;
const C2: i32 = 36;
const D2: i32 = 38;
const E2: i32 = 40;
const F2: i32 = 41;
const G2: i32 = 43;

/// "Relay Nine": the campaign theme, A minor at an easy walk.
const THEME: Song = Song {
    bpm: 92.0,
    chords: &[
        minor(A1 + 12),
        major(F2),
        major(C2),
        major(G2),
        minor(A1 + 12),
        major(F2),
        major(G2),
        major(E2),
    ],
    melody: &[
        (0, 0, 6, 76),
        (0, 6, 2, 74),
        (0, 8, 4, 72),
        (0, 12, 4, 71),
        (1, 0, 8, 69),
        (1, 8, 4, 72),
        (1, 12, 4, 77),
        (2, 0, 6, 76),
        (2, 6, 2, 74),
        (2, 8, 4, 76),
        (2, 12, 4, 79),
        (3, 0, 12, 74),
        (3, 12, 4, 71),
        (4, 0, 6, 76),
        (4, 6, 2, 74),
        (4, 8, 4, 72),
        (4, 12, 4, 76),
        (5, 0, 8, 81),
        (5, 8, 4, 79),
        (5, 12, 4, 77),
        (6, 0, 4, 79),
        (6, 4, 4, 77),
        (6, 8, 4, 74),
        (6, 12, 4, 71),
        (7, 0, 8, 68),
        (7, 8, 8, 71),
    ],
};

/// "Turnabout": the battle theme, D minor and driving.
const BATTLE: Song = Song {
    bpm: 144.0,
    chords: &[
        minor(D2),
        major(BB1),
        major(C2),
        major(A1),
        minor(D2),
        major(BB1),
        major(C2),
        major(A1),
        minor(G2),
        minor(D2),
        major(BB1),
        major(C2),
        minor(G2),
        minor(D2),
        major(BB1),
        major(A1),
    ],
    melody: &[
        (0, 0, 4, 74),
        (0, 4, 4, 77),
        (0, 8, 6, 81),
        (0, 14, 2, 79),
        (1, 0, 4, 77),
        (1, 4, 4, 74),
        (1, 8, 8, 70),
        (2, 0, 4, 72),
        (2, 4, 4, 76),
        (2, 8, 6, 79),
        (2, 14, 2, 77),
        (3, 0, 12, 76),
        (3, 12, 4, 73),
        (4, 0, 4, 74),
        (4, 4, 4, 77),
        (4, 8, 4, 81),
        (4, 12, 4, 86),
        (5, 0, 4, 84),
        (5, 4, 4, 82),
        (5, 8, 4, 81),
        (5, 12, 4, 77),
        (6, 0, 4, 79),
        (6, 4, 4, 81),
        (6, 8, 4, 82),
        (6, 12, 4, 79),
        (7, 0, 12, 81),
        (8, 0, 8, 79),
        (8, 8, 4, 82),
        (8, 12, 4, 81),
        (9, 0, 8, 77),
        (9, 8, 8, 74),
        (10, 0, 4, 74),
        (10, 4, 4, 77),
        (10, 8, 8, 82),
        (11, 0, 8, 84),
        (11, 8, 8, 79),
        (12, 0, 6, 82),
        (12, 6, 2, 81),
        (12, 8, 8, 79),
        (13, 0, 6, 81),
        (13, 6, 2, 77),
        (13, 8, 8, 74),
        (14, 0, 4, 74),
        (14, 4, 4, 77),
        (14, 8, 4, 82),
        (14, 12, 4, 86),
        (15, 0, 8, 85),
        (15, 8, 8, 88),
    ],
};

impl Track {
    fn song(self) -> &'static Song {
        match self {
            Track::Theme => &THEME,
            Track::Battle => &BATTLE,
        }
    }
}

/// Renders one seamless loop of a track.
pub fn render_track(track: Track) -> Vec<f32> {
    use Envelope::{Pad, Pluck};
    use Wave::{Saw, Sine, Square, Triangle};
    let song = track.song();
    let step = song.step_samples() as f32 / SAMPLE_RATE as f32;
    let bar = step * 16.0;
    let mut b = Buffer::new(track.samples(), true);
    let note = |pitch: i32| midi(pitch);

    for (index, chord) in song.chords.iter().enumerate() {
        let start = index as f32 * bar;
        match track {
            Track::Theme => {
                for pitch in chord.triad(3) {
                    b.tone(
                        start,
                        bar + 0.3,
                        Voice::new(Triangle, note(pitch), note(pitch), 0.05).envelope(Pad {
                            attack: 0.35,
                            release: 0.6,
                        }),
                    );
                }
                for half in 0..2 {
                    let at = start + half as f32 * 8.0 * step;
                    b.tone(
                        at,
                        8.0 * step,
                        Voice::new(Saw, note(chord.root), note(chord.root), 0.16)
                            .envelope(Pad {
                                attack: 0.02,
                                release: 0.25,
                            })
                            .cutoff(500.0),
                    );
                }
                // Eighth-note arpeggio up and back down the chord.
                let [r, t, f] = chord.triad(4);
                for (i, pitch) in [r, t, f, r + 12, f, t, r, t].into_iter().enumerate() {
                    b.tone(
                        start + i as f32 * 2.0 * step,
                        2.0 * step,
                        Voice::new(Square, note(pitch), note(pitch), 0.035).cutoff(2200.0),
                    );
                }
                for i in 0..8 {
                    b.noise(
                        start + i as f32 * 2.0 * step,
                        0.04,
                        0.025,
                        Pluck { attack: 0.001 },
                        Filter::High(7000.0),
                        101 + i,
                    );
                }
            }
            Track::Battle => {
                // Octave-jumping eighth-note bass.
                let offsets = [0, 0, 12, 0, 0, 12, 0, 7];
                for (i, offset) in offsets.into_iter().enumerate() {
                    let pitch = chord.root + offset;
                    b.tone(
                        start + i as f32 * 2.0 * step,
                        2.0 * step,
                        Voice::new(Saw, note(pitch), note(pitch), 0.2).cutoff(900.0),
                    );
                }
                // Power-chord stabs on the syncopated hits.
                for hit in [0, 6, 12] {
                    for pitch in [chord.root + 24, chord.root + 31] {
                        b.tone(
                            start + hit as f32 * step,
                            2.5 * step,
                            Voice::new(Square, note(pitch), note(pitch), 0.045).cutoff(1800.0),
                        );
                    }
                }
                for pitch in chord.triad(3) {
                    b.tone(
                        start,
                        bar,
                        Voice::new(Saw, note(pitch), note(pitch), 0.018)
                            .envelope(Pad {
                                attack: 0.08,
                                release: 0.2,
                            })
                            .cutoff(1500.0),
                    );
                }
                for kick in [0, 8, 10] {
                    b.tone(
                        start + kick as f32 * step,
                        0.16,
                        Voice::new(Sine, 140.0, 42.0, 0.5),
                    );
                }
                for snare in [4, 12] {
                    let at = start + snare as f32 * step;
                    b.noise(
                        at,
                        0.14,
                        0.3,
                        Pluck { attack: 0.001 },
                        Filter::Low(5000.0),
                        211 + snare,
                    );
                    b.tone(at, 0.08, Voice::new(Triangle, 220.0, 160.0, 0.15));
                }
                for hat in (0..16).step_by(2) {
                    b.noise(
                        start + hat as f32 * step,
                        0.03,
                        0.05,
                        Pluck { attack: 0.001 },
                        Filter::High(8000.0),
                        307 + hat as u32,
                    );
                }
            }
        }
    }

    let (lead, gain) = match track {
        Track::Theme => (Triangle, 0.11),
        Track::Battle => (Square, 0.055),
    };
    for &(bar_index, at, len, pitch) in song.melody {
        let start = bar_index as f32 * bar + at as f32 * step;
        let mut voice = Voice::new(lead, note(pitch), note(pitch), gain)
            .envelope(Pad {
                attack: 0.01,
                release: 0.06,
            })
            .vibrato(5.5, 0.2);
        if track == Track::Battle {
            voice = voice.cutoff(3200.0);
        }
        b.tone(start, len as f32 * step, voice);
    }
    b.finish()
}

/// Renders a jingle, with a tail that rings out to silence.
pub fn render_jingle(jingle: Jingle) -> Vec<f32> {
    use Envelope::{Pad, Pluck};
    use Wave::{Saw, Sine, Square, Triangle};
    let mut b = Buffer::new((5.2 * SAMPLE_RATE as f32) as usize, false);
    match jingle {
        Jingle::Victory => {
            // Battle tempo: a sixteenth is 1/9.6 s, so the pickup bar lasts 1.67 s.
            let step = 60.0 / BATTLE.bpm / 4.0;
            let bar = 16.0 * step;
            let pickup = [(0.0, major(BB1)), (8.0 * step, major(C2))];
            for (at, chord) in pickup {
                for hit in [0.0, 3.0, 6.0] {
                    for pitch in [chord.root + 24, chord.root + 31] {
                        b.tone(
                            at + hit * step,
                            2.5 * step,
                            Voice::new(Square, midi(pitch), midi(pitch), 0.05).cutoff(1800.0),
                        );
                    }
                }
                for (i, offset) in [0, 12, 0, 12].into_iter().enumerate() {
                    let pitch = chord.root + offset;
                    b.tone(
                        at + i as f32 * 2.0 * step,
                        2.0 * step,
                        Voice::new(Saw, midi(pitch), midi(pitch), 0.2).cutoff(900.0),
                    );
                }
                for kick in [0.0, 4.0] {
                    b.tone(at + kick * step, 0.16, Voice::new(Sine, 140.0, 42.0, 0.5));
                }
            }
            // A snare roll into the landing.
            for i in 0..8 {
                let gain = 0.12 + 0.03 * i as f32;
                b.noise(
                    bar - (8 - i) as f32 * step / 2.0,
                    0.07,
                    gain,
                    Pluck { attack: 0.001 },
                    Filter::Low(5000.0),
                    401 + i,
                );
            }
            // The battle theme's opening line, then up to the high tonic.
            let lead = [
                (0.0, 4.0, 74),
                (4.0, 4.0, 77),
                (8.0, 4.0, 81),
                (12.0, 4.0, 84),
            ];
            for (at, len, pitch) in lead {
                b.tone(
                    at * step,
                    len * step,
                    Voice::new(Square, midi(pitch), midi(pitch), 0.06)
                        .envelope(Pad {
                            attack: 0.01,
                            release: 0.05,
                        })
                        .cutoff(3200.0),
                );
            }
            // The landing: D major, held and ringing out.
            let ring = 5.2 - bar;
            b.tone(bar, 0.4, Voice::new(Sine, 120.0, 38.0, 0.6));
            b.noise(
                bar,
                1.2,
                0.12,
                Pluck { attack: 0.002 },
                Filter::High(6000.0),
                499,
            );
            for pitch in [D2, D2 + 12] {
                b.tone(
                    bar,
                    ring,
                    Voice::new(Saw, midi(pitch), midi(pitch), 0.16)
                        .envelope(Pluck { attack: 0.01 })
                        .cutoff(700.0),
                );
            }
            for pitch in major(D2).triad(2).into_iter().chain([D2 + 36]) {
                b.tone(
                    bar,
                    ring,
                    Voice::new(Square, midi(pitch), midi(pitch), 0.035)
                        .envelope(Pad {
                            attack: 0.01,
                            release: 2.2,
                        })
                        .cutoff(2400.0),
                );
            }
            // A bright arpeggio climbs over the held chord.
            for (i, pitch) in [74, 78, 81, 86, 90, 93].into_iter().enumerate() {
                b.tone(
                    bar + i as f32 * step,
                    0.5,
                    Voice::new(Triangle, midi(pitch), midi(pitch), 0.06),
                );
            }
            b.tone(
                bar + 6.0 * step,
                ring - 6.0 * step,
                Voice::new(Square, midi(86), midi(86), 0.06)
                    .envelope(Pad {
                        attack: 0.02,
                        release: 2.0,
                    })
                    .vibrato(5.5, 0.25)
                    .cutoff(3200.0),
            );
        }
        Jingle::Defeat => {
            // The theme's own tempo: each note of the phrase is a quarter.
            let step = 60.0 / THEME.bpm / 4.0;
            let chords = [
                (0.0, 8.0, minor(A1 + 12)),
                (8.0, 8.0, major(F2)),
                (16.0, 8.0, minor(D2)),
            ];
            for (at, len, chord) in chords {
                for pitch in chord.triad(3) {
                    b.tone(
                        at * step,
                        len * step + 0.3,
                        Voice::new(Triangle, midi(pitch), midi(pitch), 0.05).envelope(Pad {
                            attack: 0.3,
                            release: 0.5,
                        }),
                    );
                }
                b.tone(
                    at * step,
                    len * step,
                    Voice::new(Saw, midi(chord.root), midi(chord.root), 0.15)
                        .envelope(Pad {
                            attack: 0.03,
                            release: 0.3,
                        })
                        .cutoff(450.0),
                );
            }
            // "Relay Nine" opens E D C B; here it keeps falling, A then G#.
            let lead = [
                (0.0, 4.0, 76),
                (4.0, 4.0, 74),
                (8.0, 4.0, 72),
                (12.0, 4.0, 71),
                (16.0, 4.0, 69),
                (20.0, 4.0, 68),
            ];
            for (at, len, pitch) in lead {
                b.tone(
                    at * step,
                    len * step,
                    Voice::new(Triangle, midi(pitch), midi(pitch), 0.11)
                        .envelope(Pad {
                            attack: 0.02,
                            release: 0.12,
                        })
                        .vibrato(5.0, 0.2),
                );
            }
            // It settles on a dark, low A minor that fades away.
            let end = 24.0 * step;
            let ring = 5.2 - end;
            for pitch in minor(A1).triad(1) {
                b.tone(
                    end,
                    ring,
                    Voice::new(Triangle, midi(pitch), midi(pitch), 0.07).envelope(Pad {
                        attack: 0.05,
                        release: ring - 0.1,
                    }),
                );
            }
            b.tone(
                end,
                ring,
                Voice::new(Saw, midi(A1), midi(A1 - 1), 0.18)
                    .envelope(Pad {
                        attack: 0.02,
                        release: ring - 0.1,
                    })
                    .cutoff(300.0),
            );
            b.tone(
                end,
                ring,
                Voice::new(Triangle, midi(69), midi(69), 0.08)
                    .envelope(Pad {
                        attack: 0.05,
                        release: ring - 0.1,
                    })
                    .vibrato(4.0, 0.15),
            );
            b.tone(end, 0.5, Voice::new(Sine, 90.0, 35.0, 0.45));
        }
    }
    b.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peak(samples: &[f32]) -> f32 {
        samples.iter().fold(0.0, |peak, s| peak.max(s.abs()))
    }

    #[test]
    fn tracks_loop_on_a_bar_line_without_a_seam_click() {
        for track in Track::ALL {
            let samples = render_track(track);
            assert_eq!(samples.len(), track.samples());
            assert!(samples.iter().all(|s| s.is_finite()));
            let loudest = peak(&samples);
            assert!(
                (0.2..1.0).contains(&loudest),
                "{track:?} peaks at {loudest}"
            );
            // The jump across the seam is no bigger than a typical step inside.
            let seam = (samples[samples.len() - 1] - samples[0]).abs();
            assert!(seam < 0.2, "{track:?} seam jumps {seam}");
        }
        let theme = Track::Theme.samples() as f32 / SAMPLE_RATE as f32;
        let battle = Track::Battle.samples() as f32 / SAMPLE_RATE as f32;
        assert!((20.0..22.0).contains(&theme), "theme loops every {theme}s");
        assert!(
            (26.0..28.0).contains(&battle),
            "battle loops every {battle}s"
        );
    }

    #[test]
    fn jingles_last_about_five_seconds_and_ring_out() {
        for jingle in Jingle::ALL {
            let samples = render_jingle(jingle);
            let seconds = samples.len() as f32 / SAMPLE_RATE as f32;
            assert!((4.5..5.5).contains(&seconds), "{jingle:?} lasts {seconds}s");
            assert!(samples.iter().all(|s| s.is_finite()));
            let loudest = peak(&samples);
            assert!(
                (0.2..1.0).contains(&loudest),
                "{jingle:?} peaks at {loudest}"
            );
            let tail = peak(&samples[samples.len() - SAMPLE_RATE as usize / 20..]);
            assert!(tail < 0.02, "{jingle:?} is still ringing at {tail}");
            assert!(samples[0].abs() < 0.05, "{jingle:?} starts with a click");
        }
    }

    #[test]
    fn rendering_is_deterministic() {
        assert_eq!(render_track(Track::Theme), render_track(Track::Theme));
        assert_eq!(
            render_jingle(Jingle::Victory),
            render_jingle(Jingle::Victory)
        );
    }
}
