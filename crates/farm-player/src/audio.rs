//! Sound: the requests a frame produces and a small mixer that plays them.
//!
//! Every sound effect is a synthesized farm-runtime preset ([`SfxPreset::render`]), so games are
//! audible with zero assets. The player hands out [`SoundRequest`]s (cue + gain, the settings'
//! master × effects volume already applied); a host plays them however it likes. [`Mixer`] is the
//! platform-neutral part of the desktop player's audio thread: it sums the playing voices into an
//! interleaved output buffer. [`SoundBank`] renders every preset once, up front, so a real-time
//! audio callback only ever receives ready [`Voice`]s ([`Mixer::start`] neither allocates nor
//! frees). Music and ambience are the built-in loops of `farm_runtime::music`: a frame says which
//! should play ([`MusicCue`]), a [`MusicBank`] renders them once, and the mixer loops them in two
//! slots (music, ambience), fading between loops and volumes.
//!
//! A game's own sounds and music ([`crate::audio_assets`]) arrive decoded at their file's rate:
//! a [`SoundRequest`] carries the sound's [`Clip`], a [`MusicCue`] its music asset. A host
//! resamples a clip to its output rate once ([`resample`], kept in a [`ClipCache`]) and mixes it
//! like the presets and loops; voices and loops may be mono or stereo.

use farm_runtime::audio::{sfx_preset, Clip, SFX_PRESETS};
use farm_runtime::music::{self, MusicCue};
use std::collections::BTreeMap;
use std::sync::Arc;

/// A sound to play now.
#[derive(Debug, Clone, PartialEq)]
pub struct SoundRequest {
    /// A farm-runtime preset name (`ui`, `harvest`, `coin`, …), or the id of the game's sound
    /// asset in `clip`.
    pub cue: String,
    /// Linear gain (0..=1), volumes applied.
    pub gain: f32,
    /// The game's sound asset `cue` (M2), played instead of a preset.
    pub clip: Option<Clip>,
}

/// A sound ready to mix: samples (shared with a [`SoundBank`] or a [`ClipCache`]), mono or
/// interleaved stereo, and a gain.
#[derive(Debug, Clone)]
pub struct Voice {
    samples: Arc<[f32]>,
    stereo: bool,
    position: usize,
    gain: f32,
}

impl Voice {
    /// A voice for samples at the mixer's rate (`stereo`: interleaved left, right); `None` for
    /// gains that are not positive.
    pub fn new(samples: Arc<[f32]>, stereo: bool, gain: f32) -> Option<Self> {
        // Also skips NaN gains.
        if gain.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            return None;
        }
        Some(Self { samples, stereo, position: 0, gain: gain.min(1.0) })
    }
}

/// Every preset rendered at one sample rate, shared by the voices that play them. Keep the bank
/// alive while its voices play: a real-time callback then never frees the last reference.
#[derive(Debug, Clone)]
pub struct SoundBank {
    sample_rate: u32,
    sounds: BTreeMap<&'static str, Arc<[f32]>>,
}

impl SoundBank {
    /// Renders every farm-runtime preset at `sample_rate`.
    pub fn new(sample_rate: u32) -> Self {
        let sample_rate = sample_rate.max(1);
        let sounds = SFX_PRESETS.iter().map(|(name, preset)| (*name, Arc::from(preset.render(sample_rate)))).collect();
        Self { sample_rate, sounds }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// The preset voice for `request`; `None` for unknown cues and gains that are not
    /// positive. (A request's sound asset is the host's to resample: [`ClipCache`].)
    pub fn voice(&self, request: &SoundRequest) -> Option<Voice> {
        let samples = self.sounds.get(request.cue.as_str())?;
        Voice::new(Arc::clone(samples), false, request.gain)
    }
}

/// Every built-in music and ambience loop rendered at one sample rate (several seconds of
/// rendering in all: build it off the audio callback, and off the frame loop where a hitch
/// shows). Keep it alive while the mixer loops its buffers.
#[derive(Debug, Clone)]
pub struct MusicBank {
    sample_rate: u32,
    loops: BTreeMap<&'static str, Arc<[f32]>>,
}

impl MusicBank {
    pub fn new(sample_rate: u32) -> Self {
        let sample_rate = sample_rate.max(1);
        let loops = music::MUSIC_LOOPS
            .iter()
            .chain(music::AMBIENCE_LOOPS)
            .filter_map(|name| music::render_loop(name, sample_rate).map(|samples| (*name, Arc::from(samples))))
            .collect();
        Self { sample_rate, loops }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// The samples of a loop; `None` for an unknown name.
    pub fn get(&self, name: &str) -> Option<Arc<[f32]>> {
        self.loops.get(name).cloned()
    }

    /// The two loop slots' contents for `cue`'s built-in loops: (music samples, gain),
    /// (ambience samples, gain). A cue's music asset is the host's to resample
    /// ([`ClipCache`]).
    pub fn slots(&self, cue: &MusicCue) -> [LoopRequest; 2] {
        let slot = |name: Option<&str>, gain: f32| LoopRequest {
            samples: name.and_then(|name| self.get(name)),
            stereo: false,
            gain,
        };
        [slot(cue.music, cue.music_gain), slot(cue.ambience, cue.ambience_gain)]
    }
}

/// What a loop slot should play: a loop (shared with a [`MusicBank`] or a [`ClipCache`]), mono
/// or interleaved stereo, at a gain, or silence.
#[derive(Debug, Clone, Default)]
pub struct LoopRequest {
    pub samples: Option<Arc<[f32]>>,
    pub stereo: bool,
    pub gain: f32,
}

impl LoopRequest {
    /// Whether `self` and `other` play the same samples at the same gain.
    pub fn same(&self, other: &LoopRequest) -> bool {
        let samples = match (&self.samples, &other.samples) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        samples && self.stereo == other.stereo && self.gain.to_bits() == other.gain.to_bits()
    }
}

/// A loop being played, with its gain moving toward a target.
#[derive(Debug, Clone)]
struct LoopVoice {
    samples: Arc<[f32]>,
    stereo: bool,
    position: usize,
    gain: f32,
    target: f32,
}

/// One loop slot: what plays now and what is fading out.
#[derive(Debug, Clone, Default)]
struct LoopSlot {
    current: Option<LoopVoice>,
    fading: Option<LoopVoice>,
}

/// Loop slots: music and ambience.
pub const LOOP_SLOTS: usize = 2;
/// Seconds a fade between two loops (or two volumes) takes.
pub const LOOP_FADE_SECONDS: f32 = 1.2;

/// Mixes sound-effect voices (mono presets) and the two loop slots into interleaved output
/// frames.
#[derive(Debug, Clone)]
pub struct Mixer {
    sample_rate: u32,
    cache: BTreeMap<String, Arc<[f32]>>,
    clips: ClipCache,
    voices: Vec<Voice>,
    loops: [LoopSlot; LOOP_SLOTS],
}

/// Voices mixed at once; the oldest is dropped beyond this.
pub const MAX_VOICES: usize = 16;

impl Mixer {
    pub fn new(sample_rate: u32) -> Self {
        // Room for every voice up front: the list must never grow in an audio callback.
        Self {
            sample_rate: sample_rate.max(1),
            cache: BTreeMap::new(),
            clips: ClipCache::new(sample_rate),
            voices: Vec::with_capacity(MAX_VOICES + 1),
            loops: Default::default(),
        }
    }

    /// Plays `request` in loop slot `slot` (0 music, 1 ambience): the same loop moves to the new
    /// gain, another loop fades in while the old one fades out, and no loop fades the slot out.
    /// Never allocates, and frees a buffer only when the caller held no other reference to it
    /// (a [`MusicBank`] always does).
    pub fn set_loop(&mut self, slot: usize, request: LoopRequest) {
        let Some(slot) = self.loops.get_mut(slot) else { return };
        let gain = if request.gain.is_finite() { request.gain.clamp(0.0, 1.0) } else { 0.0 };
        let same = match (&slot.current, &request.samples) {
            (Some(current), Some(samples)) => Arc::ptr_eq(&current.samples, samples),
            (None, None) => true,
            _ => false,
        };
        if same {
            if let Some(current) = slot.current.as_mut() {
                current.target = gain;
            }
            return;
        }
        let was_fading = slot.fading.take();
        if let Some(mut old) = slot.current.take() {
            old.target = 0.0;
            slot.fading = Some(old);
        }
        let stereo = request.stereo;
        slot.current = request.samples.filter(|samples| samples.len() >= width(stereo)).map(|samples| {
            // A loop that was fading out comes back from where it is.
            match was_fading.filter(|fading| Arc::ptr_eq(&fading.samples, &samples) && fading.stereo == stereo) {
                Some(mut back) => {
                    back.target = gain;
                    back
                }
                None => LoopVoice { samples, stereo, position: 0, gain: 0.0, target: gain },
            }
        });
    }

    /// Whether a loop plays (or fades) in `slot`.
    pub fn loop_playing(&self, slot: usize) -> bool {
        self.loops.get(slot).is_some_and(|slot| slot.current.is_some() || slot.fading.is_some())
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Starts a prepared voice (from a [`SoundBank`] or a [`ClipCache`]). Never allocates: the
    /// oldest voice makes room beyond [`MAX_VOICES`].
    pub fn start(&mut self, voice: Voice) {
        if self.voices.len() >= MAX_VOICES {
            self.voices.remove(0);
        }
        self.voices.push(voice);
    }

    /// Starts a voice for `request` (unknown cues are ignored), rendering its preset or
    /// resampling its sound asset on first use. Real-time callbacks use [`Mixer::start`] with a
    /// [`SoundBank`] and a [`ClipCache`] instead.
    pub fn play(&mut self, request: &SoundRequest) {
        // Also skips NaN gains.
        if request.gain.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            return;
        }
        if request.clip.is_some() {
            if let Some(voice) = self.clips.voice(request) {
                self.start(voice);
            }
            return;
        }
        let samples = match self.cache.get(&request.cue) {
            Some(samples) => Arc::clone(samples),
            None => {
                let Some(preset) = sfx_preset(&request.cue) else { return };
                let samples: Arc<[f32]> = preset.render(self.sample_rate).into();
                self.cache.insert(request.cue.clone(), Arc::clone(&samples));
                samples
            }
        };
        self.start(Voice { samples, stereo: false, position: 0, gain: request.gain.min(1.0) });
    }

    /// Voices still playing.
    pub fn active(&self) -> usize {
        self.voices.len()
    }

    /// Fills `out` (interleaved, `channels` per frame) with the mix; silence when idle.
    pub fn mix(&mut self, out: &mut [f32], channels: usize) {
        out.fill(0.0);
        let channels = channels.max(1);
        let step = 1.0 / (LOOP_FADE_SECONDS * self.sample_rate as f32);
        for slot in &mut self.loops {
            for voice in [slot.current.as_mut(), slot.fading.as_mut()].into_iter().flatten() {
                let width = width(voice.stereo);
                for frame in out.chunks_exact_mut(channels) {
                    voice.gain = if voice.gain < voice.target {
                        (voice.gain + step).min(voice.target)
                    } else {
                        (voice.gain - step).max(voice.target)
                    };
                    add_frame(frame, &voice.samples[voice.position..voice.position + width], voice.gain);
                    voice.position += width;
                    if voice.position + width > voice.samples.len() {
                        voice.position = 0;
                    }
                }
            }
            if slot.fading.as_ref().is_some_and(|fading| fading.gain <= 0.0) {
                slot.fading = None;
            }
        }
        for voice in &mut self.voices {
            let width = width(voice.stereo);
            for frame in out.chunks_exact_mut(channels) {
                let Some(samples) = voice.samples.get(voice.position..voice.position + width) else { break };
                add_frame(frame, samples, voice.gain);
                voice.position += width;
            }
        }
        self.voices.retain(|voice| voice.position + width(voice.stereo) <= voice.samples.len());
        for sample in out {
            *sample = sample.clamp(-1.0, 1.0);
        }
    }
}

/// Samples per frame: 1 (mono) or 2 (stereo).
fn width(stereo: bool) -> usize {
    if stereo {
        2
    } else {
        1
    }
}

/// Adds one source frame (`[mono]` or `[left, right]`) at `gain` to an output frame: mono goes
/// to every channel; stereo goes left and right, mixed down for a mono output and for any
/// further channels.
fn add_frame(out: &mut [f32], source: &[f32], gain: f32) {
    match *source {
        [left, right] => {
            let (left, right) = (left * gain, right * gain);
            let middle = 0.5 * (left + right);
            match out {
                [only] => *only += middle,
                [first, second, rest @ ..] => {
                    *first += left;
                    *second += right;
                    rest.iter_mut().for_each(|channel| *channel += middle);
                }
                [] => {}
            }
        }
        _ => {
            let value = source.first().copied().unwrap_or(0.0) * gain;
            out.iter_mut().for_each(|channel| *channel += value);
        }
    }
}

/// `clip`'s samples at `rate`: the clip's own buffer when the rates match, else resampled with
/// cubic (Catmull-Rom) interpolation, channel by channel. A `looped` clip interpolates across
/// its end into its start, so the loop stays seamless. Takes a while for long music: do it
/// off the audio callback.
pub fn resample(clip: &Clip, rate: u32, looped: bool) -> Arc<[f32]> {
    let rate = rate.max(1);
    let source_rate = clip.sample_rate.max(1);
    if source_rate == rate || clip.samples.is_empty() {
        return Arc::clone(&clip.samples);
    }
    let channels = usize::from(clip.channels.clamp(1, 2));
    let frames = clip.samples.len() / channels;
    if frames == 0 {
        return Arc::clone(&clip.samples);
    }
    let out_frames = ((frames as u64 * u64::from(rate)).div_ceil(u64::from(source_rate)) as usize).max(1);
    let step = f64::from(source_rate) / f64::from(rate);
    let at = |frame: isize, channel: usize| -> f32 {
        let frame = if looped {
            frame.rem_euclid(frames as isize) as usize
        } else {
            frame.clamp(0, frames as isize - 1) as usize
        };
        clip.samples[frame * channels + channel]
    };
    let mut out = Vec::with_capacity(out_frames * channels);
    for index in 0..out_frames {
        let position = index as f64 * step;
        let whole = position.floor();
        let t = (position - whole) as f32;
        let whole = whole as isize;
        for channel in 0..channels {
            let (y0, y1, y2, y3) =
                (at(whole - 1, channel), at(whole, channel), at(whole + 1, channel), at(whole + 2, channel));
            let c1 = 0.5 * (y2 - y0);
            let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
            let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
            out.push((((c3 * t + c2) * t + c1) * t + y1).clamp(-1.0, 1.0));
        }
    }
    Arc::from(out)
}

/// A game's clips resampled to one output rate, kept as long as the cache lives, so a real-time
/// callback playing them never frees the last reference. Clips are told apart by identity
/// ([`Clip`]'s `PartialEq`); a game has a few dozen at most, so a list does.
#[derive(Debug, Clone)]
pub struct ClipCache {
    rate: u32,
    entries: Vec<(Clip, Arc<[f32]>)>,
}

impl ClipCache {
    pub fn new(rate: u32) -> Self {
        Self { rate: rate.max(1), entries: Vec::new() }
    }

    pub fn sample_rate(&self) -> u32 {
        self.rate
    }

    /// `clip`'s samples at the cache's rate, if they are ready.
    pub fn get(&self, clip: &Clip) -> Option<Arc<[f32]>> {
        self.entries.iter().find(|(source, _)| source == clip).map(|(_, samples)| Arc::clone(samples))
    }

    /// Keeps `samples` (made by [`resample`] at the cache's rate) for `clip`.
    pub fn insert(&mut self, clip: Clip, samples: Arc<[f32]>) {
        if self.get(&clip).is_none() {
            self.entries.push((clip, samples));
        }
    }

    /// `clip`'s samples at the cache's rate, resampled now on first use.
    pub fn prepare(&mut self, clip: &Clip, looped: bool) -> Arc<[f32]> {
        if let Some(samples) = self.get(clip) {
            return samples;
        }
        let samples = resample(clip, self.rate, looped);
        self.entries.push((clip.clone(), Arc::clone(&samples)));
        samples
    }

    /// The voice for a request's sound asset; `None` without one, or for gains that are not
    /// positive.
    pub fn voice(&mut self, request: &SoundRequest) -> Option<Voice> {
        let clip = request.clip.as_ref()?;
        if request.gain.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            return None;
        }
        Voice::new(self.prepare(clip, false), clip.channels == 2, request.gain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixes_presets_until_they_end() {
        let mut mixer = Mixer::new(8000);
        mixer.play(&SoundRequest { cue: "coin".into(), gain: 0.5, clip: None });
        mixer.play(&SoundRequest { cue: "no-such-cue".into(), gain: 1.0, clip: None });
        mixer.play(&SoundRequest { cue: "ui".into(), gain: 0.0, clip: None });
        assert_eq!(mixer.active(), 1);
        let mut out = vec![0.0f32; 2 * 256];
        mixer.mix(&mut out, 2);
        assert!(out.iter().any(|s| *s != 0.0));
        assert_eq!(out[0], out[1], "mono in both channels");
        assert!(out.iter().all(|s| s.abs() <= 0.5 + 1e-6));
        for _ in 0..20 {
            mixer.mix(&mut out, 2);
        }
        assert_eq!(mixer.active(), 0);
        mixer.mix(&mut out, 2);
        assert!(out.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn loops_fade_in_repeat_and_cross_fade() {
        let rate = 1000;
        let up: Arc<[f32]> = Arc::from(vec![0.5f32; 300]);
        let down: Arc<[f32]> = Arc::from(vec![-0.25f32; 200]);
        let mut mixer = Mixer::new(rate);
        mixer.set_loop(0, LoopRequest { samples: Some(Arc::clone(&up)), stereo: false, gain: 1.0 });
        assert!(mixer.loop_playing(0) && !mixer.loop_playing(1));
        // Fades in over LOOP_FADE_SECONDS, then holds: past the loop's end it starts again.
        let mut out = vec![0.0f32; 2 * 2000];
        mixer.mix(&mut out, 2);
        assert!(out[0].abs() < 0.01, "starts silent: {}", out[0]);
        assert!((out[out.len() - 1] - 0.5).abs() < 1e-6, "full volume, looping: {}", out[out.len() - 1]);
        // Same loop, new volume: no restart, just a fade.
        mixer.set_loop(0, LoopRequest { samples: Some(Arc::clone(&up)), stereo: false, gain: 0.5 });
        mixer.mix(&mut out, 2);
        assert!((out[out.len() - 1] - 0.25).abs() < 1e-6);
        // Another loop: the old fades out while the new fades in, then only the new plays.
        mixer.set_loop(0, LoopRequest { samples: Some(Arc::clone(&down)), stereo: false, gain: 1.0 });
        mixer.mix(&mut out, 2);
        assert!((out[out.len() - 1] + 0.25).abs() < 1e-6, "{}", out[out.len() - 1]);
        // Silence fades the slot out and frees it.
        mixer.set_loop(0, LoopRequest::default());
        mixer.mix(&mut out, 2);
        mixer.mix(&mut out, 2);
        assert!(!mixer.loop_playing(0));
        assert!(out.iter().all(|s| *s == 0.0));
        // Unknown slots are ignored.
        mixer.set_loop(7, LoopRequest { samples: Some(up), stereo: false, gain: 1.0 });
    }

    #[test]
    fn the_music_bank_holds_every_loop() {
        let bank = MusicBank::new(4000);
        let cue = MusicCue::new(Some("day"), Some("rain"), 0.5);
        let [music, ambience] = bank.slots(&cue);
        assert!(music.samples.is_some() && ambience.samples.is_some());
        assert_eq!((music.gain, ambience.gain), (0.5, 0.4));
        assert!(bank.get("night").is_some() && bank.get("crickets").is_some() && bank.get("birds").is_some());
        let [silent, _] = bank.slots(&MusicCue::default());
        assert!(silent.samples.is_none());
    }

    #[test]
    fn banks_prepare_voices_up_front_and_mix_like_play() {
        let bank = SoundBank::new(8000);
        assert_eq!(bank.sample_rate(), 8000);
        assert!(bank.voice(&SoundRequest { cue: "no-such-cue".into(), gain: 1.0, clip: None }).is_none());
        assert!(bank.voice(&SoundRequest { cue: "coin".into(), gain: f32::NAN, clip: None }).is_none());
        let request = SoundRequest { cue: "coin".into(), gain: 0.5, clip: None };
        let (mut played, mut started) = (Mixer::new(8000), Mixer::new(8000));
        played.play(&request);
        started.start(bank.voice(&request).unwrap());
        let (mut a, mut b) = (vec![0.0f32; 512], vec![0.0f32; 512]);
        played.mix(&mut a, 2);
        started.mix(&mut b, 2);
        assert_eq!(a, b);
        // Voices share the bank's samples, and the voice list never grows past its capacity.
        let capacity = started.voices.capacity();
        for _ in 0..3 * MAX_VOICES {
            started.start(bank.voice(&request).unwrap());
        }
        assert_eq!(started.active(), MAX_VOICES);
        assert_eq!(started.voices.capacity(), capacity);
        assert!(Arc::strong_count(&bank.sounds["coin"]) > MAX_VOICES);
    }

    fn clip(rate: u32, channels: u16, samples: Vec<f32>) -> Clip {
        Clip { sample_rate: rate, channels, samples: Arc::from(samples) }
    }

    #[test]
    fn stereo_voices_keep_their_sides_and_mix_down_for_mono() {
        let request =
            SoundRequest { cue: "duet".into(), gain: 1.0, clip: Some(clip(8000, 2, vec![0.5, -0.25, 0.5, -0.25])) };
        let mut mixer = Mixer::new(8000);
        mixer.play(&request);
        let mut out = vec![0.0f32; 3 * 4];
        mixer.mix(&mut out, 3);
        // Left, right, then the two mixed for a third channel; two frames, then silence.
        assert_eq!(&out[..6], &[0.5, -0.25, 0.125, 0.5, -0.25, 0.125]);
        assert!(out[6..].iter().all(|s| *s == 0.0));
        assert_eq!(mixer.active(), 0);
        let mut mono = Mixer::new(8000);
        mono.play(&request);
        let mut out = vec![0.0f32; 3];
        mono.mix(&mut out, 1);
        assert_eq!(out, [0.125, 0.125, 0.0]);
    }

    #[test]
    fn stereo_loops_wrap_on_whole_frames() {
        let samples: Arc<[f32]> = Arc::from(vec![0.5f32, -0.5, 0.25, -0.25, 1.0]);
        let mut mixer = Mixer::new(1000);
        mixer.set_loop(0, LoopRequest { samples: Some(samples), stereo: true, gain: 1.0 });
        let mut out = vec![0.0f32; 2 * 4000];
        mixer.mix(&mut out, 2);
        // Faded in by now: frames alternate (0.5, -0.5), (0.25, -0.25); the odd sample is never read.
        let tail = &out[out.len() - 8..];
        assert!(
            tail.chunks(4).all(|pair| pair == [0.5, -0.5, 0.25, -0.25] || pair == [0.25, -0.25, 0.5, -0.5]),
            "{tail:?}"
        );
    }

    #[test]
    fn resampling_keeps_length_level_and_shares_matching_rates() {
        let tone = clip(8000, 1, (0..800).map(|i| (i as f32 * 0.05).sin() * 0.5).collect());
        let same = resample(&tone, 8000, false);
        assert!(Arc::ptr_eq(&same, &tone.samples), "no copy at the clip's own rate");
        let up = resample(&tone, 44_100, false);
        assert_eq!(up.len(), (800 * 44_100usize).div_ceil(8000));
        let peak = up.iter().fold(0.0f32, |max, s| max.max(s.abs()));
        assert!((peak - 0.5).abs() < 0.01, "{peak}");
        // Every source sample lands on an output sample where the ratio is whole.
        let double = resample(&tone, 16_000, false);
        assert_eq!(double.len(), 1600);
        assert!(double.iter().step_by(2).zip(tone.samples.iter()).all(|(a, b)| (a - b).abs() < 1e-6));
        // Stereo stays interleaved, sides apart.
        let sides = resample(&clip(8000, 2, [0.5, -0.5].repeat(100)), 22_050, true);
        assert_eq!(sides.len() % 2, 0);
        assert!(sides.chunks(2).all(|frame| (frame[0] - 0.5).abs() < 1e-6 && (frame[1] + 0.5).abs() < 1e-6));
    }

    #[test]
    fn the_clip_cache_resamples_each_clip_once() {
        let sound = clip(22_050, 1, vec![0.25; 2205]);
        let mut cache = ClipCache::new(44_100);
        assert!(cache.get(&sound).is_none());
        let request = SoundRequest { cue: "chime".into(), gain: 0.5, clip: Some(sound.clone()) };
        let voice = cache.voice(&request).unwrap();
        assert_eq!(voice.samples.len(), 4410);
        let again = cache.get(&sound).unwrap();
        assert!(Arc::ptr_eq(&again, &voice.samples));
        // Another decode of the same samples is another clip.
        assert!(cache.get(&clip(22_050, 1, vec![0.25; 2205])).is_none());
        assert!(cache.voice(&SoundRequest { gain: 0.0, ..request.clone() }).is_none());
        assert!(cache.voice(&SoundRequest { clip: None, ..request }).is_none());
    }
}
