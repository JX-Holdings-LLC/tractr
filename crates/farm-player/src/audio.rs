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
//! A game's own sounds and music ([`crate::audio_assets`]) arrive as [`AudioFile`]s: a
//! [`SoundRequest`] carries the sound's file, a [`MusicCue`] its music asset. A short sound comes
//! decoded at its file's rate; a host resamples it to its output rate on first use ([`resample`])
//! and keeps that in a [`ClipCache`], which drops the clips played least recently once it holds
//! more than its budget, never one that is playing. Music and long sounds are decoded and
//! resampled while they play ([`crate::audio_stream`]), and the mixer plays them from a
//! [`StreamHandle`] like any other samples. Voices and loops may be mono or stereo.

use crate::audio_stream::{Pop, StreamHandle};
use farm_runtime::audio::{sfx_preset, AudioFile, Clip, FileBytes, SFX_PRESETS};
use farm_runtime::music::{self, MusicCue};
use std::collections::BTreeMap;
use std::sync::{Arc, Weak};

/// A sound to play now.
#[derive(Debug, Clone, PartialEq)]
pub struct SoundRequest {
    /// A farm-runtime preset name (`ui`, `harvest`, `coin`, …), or the id of the game's sound
    /// asset in `file`.
    pub cue: String,
    /// Linear gain (0..=1), volumes applied.
    pub gain: f32,
    /// The game's sound asset `cue` (M2), played instead of a preset.
    pub file: Option<AudioFile>,
}

/// What a voice or a loop plays: samples at the mixer's rate, mono or interleaved stereo, in a
/// buffer (shared with a [`SoundBank`], a [`MusicBank`] or a [`ClipCache`]) or arriving from a
/// stream.
#[derive(Debug, Clone)]
pub enum Samples {
    Buffer(Arc<[f32]>),
    Stream(StreamHandle),
}

impl Samples {
    /// Whether `self` and `other` are the same buffer or stream.
    pub fn same(&self, other: &Samples) -> bool {
        match (self, other) {
            (Samples::Buffer(a), Samples::Buffer(b)) => Arc::ptr_eq(a, b),
            (Samples::Stream(a), Samples::Stream(b)) => a.same(b),
            _ => false,
        }
    }

    /// Whether there is a whole frame to play (a stream always may have).
    fn playable(&self, stereo: bool) -> bool {
        match self {
            Samples::Buffer(samples) => samples.len() >= width(stereo),
            Samples::Stream(stream) => stream.channels() == width(stereo),
        }
    }
}

/// Where a voice or loop is in its samples.
#[derive(Debug, Clone)]
struct Playhead {
    samples: Samples,
    position: usize,
}

impl Playhead {
    fn new(samples: Samples) -> Self {
        Self { samples, position: 0 }
    }

    /// Reads the next frame into `frame` (1 or 2 samples). A looped buffer starts again at its
    /// end; a stream loops by itself.
    fn next(&mut self, frame: &mut [f32], looped: bool) -> Pop {
        match &self.samples {
            Samples::Buffer(samples) => {
                let width = frame.len();
                if looped && self.position + width > samples.len() {
                    self.position = 0;
                }
                match samples.get(self.position..self.position + width) {
                    Some(source) => {
                        frame.copy_from_slice(source);
                        self.position += width;
                        Pop::Frame
                    }
                    None => Pop::End,
                }
            }
            Samples::Stream(stream) => stream.pop(frame),
        }
    }

    /// Whether everything has played (a buffer at its end, a stream that ended and ran dry).
    fn finished(&self, width: usize) -> bool {
        match &self.samples {
            Samples::Buffer(samples) => self.position + width > samples.len(),
            Samples::Stream(stream) => stream.finished(),
        }
    }
}

/// A sound ready to mix: samples, mono or interleaved stereo, and a gain.
#[derive(Debug, Clone)]
pub struct Voice {
    head: Playhead,
    stereo: bool,
    gain: f32,
}

impl Voice {
    /// A voice for samples at the mixer's rate (`stereo`: interleaved left, right); `None` for
    /// gains that are not positive.
    pub fn new(samples: Arc<[f32]>, stereo: bool, gain: f32) -> Option<Self> {
        Self::of(Samples::Buffer(samples), stereo, gain)
    }

    /// A voice for a stream at the mixer's rate.
    pub fn streamed(stream: StreamHandle, stereo: bool, gain: f32) -> Option<Self> {
        Self::of(Samples::Stream(stream), stereo, gain)
    }

    fn of(samples: Samples, stereo: bool, gain: f32) -> Option<Self> {
        // Also skips NaN gains.
        if gain.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            return None;
        }
        Some(Self { head: Playhead::new(samples), stereo, gain: gain.min(1.0) })
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
    /// positive. (A request's sound asset is the host's to resample or stream: [`ClipCache`],
    /// [`crate::audio_stream`].)
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
    /// (ambience samples, gain). A cue's music asset is the host's to stream
    /// ([`crate::audio_stream`]).
    pub fn slots(&self, cue: &MusicCue) -> [LoopRequest; 2] {
        let slot = |name: Option<&str>, gain: f32| LoopRequest {
            samples: name.and_then(|name| self.get(name)).map(Samples::Buffer),
            stereo: false,
            gain,
        };
        [slot(cue.music, cue.music_gain), slot(cue.ambience, cue.ambience_gain)]
    }
}

/// What a loop slot should play: a loop (a buffer shared with a [`MusicBank`], or a looped
/// stream), mono or interleaved stereo, at a gain, or silence.
#[derive(Debug, Clone, Default)]
pub struct LoopRequest {
    pub samples: Option<Samples>,
    pub stereo: bool,
    pub gain: f32,
}

impl LoopRequest {
    /// Whether `self` and `other` play the same samples at the same gain.
    pub fn same(&self, other: &LoopRequest) -> bool {
        let samples = match (&self.samples, &other.samples) {
            (Some(a), Some(b)) => a.same(b),
            (None, None) => true,
            _ => false,
        };
        samples && self.stereo == other.stereo && self.gain.to_bits() == other.gain.to_bits()
    }
}

/// A loop being played, with its gain moving toward a target.
#[derive(Debug, Clone)]
struct LoopVoice {
    head: Playhead,
    stereo: bool,
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
    /// (a [`MusicBank`] always does; a stream's queue outlives its handles on the feeder's
    /// thread).
    pub fn set_loop(&mut self, slot: usize, request: LoopRequest) {
        let Some(slot) = self.loops.get_mut(slot) else { return };
        let gain = if request.gain.is_finite() { request.gain.clamp(0.0, 1.0) } else { 0.0 };
        let same = match (&slot.current, &request.samples) {
            (Some(current), Some(samples)) => current.head.samples.same(samples),
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
        slot.current = request.samples.filter(|samples| samples.playable(stereo)).map(|samples| {
            // A loop that was fading out comes back from where it is.
            match was_fading.filter(|fading| fading.head.samples.same(&samples) && fading.stereo == stereo) {
                Some(mut back) => {
                    back.target = gain;
                    back
                }
                None => LoopVoice { head: Playhead::new(samples), stereo, gain: 0.0, target: gain },
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

    /// Starts a prepared voice (from a [`SoundBank`], a [`ClipCache`] or a stream). Never
    /// allocates: the oldest voice makes room beyond [`MAX_VOICES`].
    pub fn start(&mut self, voice: Voice) {
        if self.voices.len() >= MAX_VOICES {
            self.voices.remove(0);
        }
        self.voices.push(voice);
    }

    /// Starts a voice for `request` (unknown cues are ignored), rendering its preset or
    /// resampling its decoded sound asset on first use. A sound asset that is not kept decoded
    /// needs a stream ([`crate::audio_stream`]) and is ignored here. Real-time callbacks use
    /// [`Mixer::start`] with a [`SoundBank`] and a [`ClipCache`] instead.
    pub fn play(&mut self, request: &SoundRequest) {
        // Also skips NaN gains.
        if request.gain.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            return;
        }
        if request.file.is_some() {
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
        self.start(Voice { head: Playhead::new(Samples::Buffer(samples)), stereo: false, gain: request.gain.min(1.0) });
    }

    /// Voices still playing.
    pub fn active(&self) -> usize {
        self.voices.len()
    }

    /// Fills `out` (interleaved, `channels` per frame) with the mix; silence when idle. A stream
    /// that falls behind is silent until it catches up.
    pub fn mix(&mut self, out: &mut [f32], channels: usize) {
        out.fill(0.0);
        let channels = channels.max(1);
        let step = 1.0 / (LOOP_FADE_SECONDS * self.sample_rate as f32);
        let mut source = [0.0f32; 2];
        for slot in &mut self.loops {
            for voice in [slot.current.as_mut(), slot.fading.as_mut()].into_iter().flatten() {
                let source = &mut source[..width(voice.stereo)];
                for frame in out.chunks_exact_mut(channels) {
                    voice.gain = if voice.gain < voice.target {
                        (voice.gain + step).min(voice.target)
                    } else {
                        (voice.gain - step).max(voice.target)
                    };
                    if voice.head.next(source, true) == Pop::Frame {
                        add_frame(frame, source, voice.gain);
                    }
                }
            }
            if slot.fading.as_ref().is_some_and(|fading| fading.gain <= 0.0) {
                slot.fading = None;
            }
        }
        for voice in &mut self.voices {
            let source = &mut source[..width(voice.stereo)];
            for frame in out.chunks_exact_mut(channels) {
                match voice.head.next(source, false) {
                    Pop::Frame => add_frame(frame, source, voice.gain),
                    Pop::Gap => {}
                    Pop::End => break,
                }
            }
        }
        self.voices.retain(|voice| !voice.head.finished(width(voice.stereo)));
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

/// Cubic (Catmull-Rom) interpolation between `y1` and `y2` at `t` (0..1), clipped to [-1, 1]:
/// the resampling of [`resample`] and of streams ([`crate::audio_stream::Stream`]).
pub(crate) fn cubic(y0: f32, y1: f32, y2: f32, y3: f32, t: f32) -> f32 {
    let c1 = 0.5 * (y2 - y0);
    let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
    (((c3 * t + c2) * t + c1) * t + y1).clamp(-1.0, 1.0)
}

/// `clip`'s samples at `rate`: the clip's own buffer when the rates match, else resampled with
/// cubic (Catmull-Rom) interpolation, channel by channel. A `looped` clip interpolates across
/// its end into its start, so the loop stays seamless. Takes a while for a long clip: do it
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
            out.push(cubic(
                at(whole - 1, channel),
                at(whole, channel),
                at(whole + 1, channel),
                at(whole + 2, channel),
                t,
            ));
        }
    }
    Arc::from(out)
}

/// The resampled clips [`ClipCache`] keeps, in bytes, before it drops the least recently used
/// ones that are not playing: about 45 seconds of stereo at 48 kHz.
pub const CLIP_CACHE_BYTES: usize = 16 << 20;

/// A game's short sounds resampled to one output rate, so playing one again costs nothing. It
/// holds at most [`CLIP_CACHE_BYTES`] of resampled copies (a budget of its own with
/// [`ClipCache::with_budget`]): past that it drops the clips played least recently, but never
/// one that is still playing or waiting to play (one a [`Voice`] holds), so a real-time
/// callback never frees the last reference to one. Files are told apart by identity
/// ([`AudioFile`]'s `PartialEq`), without keeping them alive.
#[derive(Debug, Clone)]
pub struct ClipCache {
    rate: u32,
    budget: usize,
    entries: Vec<CacheEntry>,
    /// Counts uses, to find the least recently used clip.
    clock: u64,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    file: Weak<dyn AsRef<[u8]> + Send + Sync>,
    samples: Arc<[f32]>,
    /// The bytes this copy takes: none when it is the file's own decoded buffer (the rates
    /// match), which the game's assets hold anyway.
    bytes: usize,
    used: u64,
}

impl CacheEntry {
    fn is(&self, file: &FileBytes) -> bool {
        std::ptr::addr_eq(self.file.as_ptr(), Arc::as_ptr(file))
    }

    /// Whether nothing but the cache holds the samples: not playing, not waiting to play, and
    /// (for the file's own buffer) not part of a game any more.
    fn unused(&self) -> bool {
        Arc::strong_count(&self.samples) == 1
    }
}

impl ClipCache {
    pub fn new(rate: u32) -> Self {
        Self::with_budget(rate, CLIP_CACHE_BYTES)
    }

    /// A cache holding at most `budget` bytes of resampled clips that are not playing.
    pub fn with_budget(rate: u32, budget: usize) -> Self {
        Self { rate: rate.max(1), budget, entries: Vec::new(), clock: 0 }
    }

    pub fn sample_rate(&self) -> u32 {
        self.rate
    }

    /// The bytes of resampled copies held now.
    pub fn bytes(&self) -> usize {
        self.entries.iter().map(|entry| entry.bytes).sum()
    }

    /// Clips held now.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// `file`'s samples at the cache's rate, if they are ready.
    pub fn get(&self, file: &AudioFile) -> Option<Arc<[f32]>> {
        self.entries.iter().find(|entry| entry.is(&file.bytes)).map(|entry| Arc::clone(&entry.samples))
    }

    /// `file`'s decoded samples at the cache's rate, resampled now on first use; `None` for a
    /// file that is not kept decoded (it streams). May drop other clips that are not playing.
    pub fn prepare(&mut self, file: &AudioFile) -> Option<Arc<[f32]>> {
        self.clock += 1;
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.is(&file.bytes)) {
            entry.used = self.clock;
            return Some(Arc::clone(&entry.samples));
        }
        let clip = file.decoded.as_ref()?;
        let samples = resample(clip, self.rate, false);
        let bytes = if Arc::ptr_eq(&samples, &clip.samples) { 0 } else { std::mem::size_of_val(&*samples) };
        self.entries.push(CacheEntry {
            file: Arc::downgrade(&file.bytes),
            samples: Arc::clone(&samples),
            bytes,
            used: self.clock,
        });
        // The new clip is held by `samples` here: it is never the one dropped.
        self.trim();
        Some(samples)
    }

    /// Drops the clips of games that are gone, then the least recently used clips nothing
    /// plays until the cache is within its budget. Cheap: call it every frame, so clips that
    /// finished playing are let go even when nothing new plays.
    pub fn trim(&mut self) {
        self.entries.retain(|entry| entry.file.strong_count() > 0 || !entry.unused());
        let mut total = self.bytes();
        while total > self.budget {
            let oldest = self
                .entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| entry.bytes > 0 && entry.unused())
                .min_by_key(|(_, entry)| entry.used)
                .map(|(index, _)| index);
            let Some(index) = oldest else { break };
            total -= self.entries.remove(index).bytes;
        }
    }

    /// The voice for a request's sound asset; `None` without one, for one that is not kept
    /// decoded, or for gains that are not positive.
    pub fn voice(&mut self, request: &SoundRequest) -> Option<Voice> {
        let file = request.file.as_ref()?;
        if request.gain.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            return None;
        }
        Voice::new(self.prepare(file)?, file.channels == 2, request.gain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_stream::Feed;

    #[test]
    fn mixes_presets_until_they_end() {
        let mut mixer = Mixer::new(8000);
        mixer.play(&SoundRequest { cue: "coin".into(), gain: 0.5, file: None });
        mixer.play(&SoundRequest { cue: "no-such-cue".into(), gain: 1.0, file: None });
        mixer.play(&SoundRequest { cue: "ui".into(), gain: 0.0, file: None });
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

    fn buffer(samples: &Arc<[f32]>) -> Option<Samples> {
        Some(Samples::Buffer(Arc::clone(samples)))
    }

    #[test]
    fn loops_fade_in_repeat_and_cross_fade() {
        let rate = 1000;
        let up: Arc<[f32]> = Arc::from(vec![0.5f32; 300]);
        let down: Arc<[f32]> = Arc::from(vec![-0.25f32; 200]);
        let mut mixer = Mixer::new(rate);
        mixer.set_loop(0, LoopRequest { samples: buffer(&up), stereo: false, gain: 1.0 });
        assert!(mixer.loop_playing(0) && !mixer.loop_playing(1));
        // Fades in over LOOP_FADE_SECONDS, then holds: past the loop's end it starts again.
        let mut out = vec![0.0f32; 2 * 2000];
        mixer.mix(&mut out, 2);
        assert!(out[0].abs() < 0.01, "starts silent: {}", out[0]);
        assert!((out[out.len() - 1] - 0.5).abs() < 1e-6, "full volume, looping: {}", out[out.len() - 1]);
        // Same loop, new volume: no restart, just a fade.
        mixer.set_loop(0, LoopRequest { samples: buffer(&up), stereo: false, gain: 0.5 });
        mixer.mix(&mut out, 2);
        assert!((out[out.len() - 1] - 0.25).abs() < 1e-6);
        // Another loop: the old fades out while the new fades in, then only the new plays.
        mixer.set_loop(0, LoopRequest { samples: buffer(&down), stereo: false, gain: 1.0 });
        mixer.mix(&mut out, 2);
        assert!((out[out.len() - 1] + 0.25).abs() < 1e-6, "{}", out[out.len() - 1]);
        // Silence fades the slot out and frees it.
        mixer.set_loop(0, LoopRequest::default());
        mixer.mix(&mut out, 2);
        mixer.mix(&mut out, 2);
        assert!(!mixer.loop_playing(0));
        assert!(out.iter().all(|s| *s == 0.0));
        // Unknown slots are ignored.
        mixer.set_loop(7, LoopRequest { samples: buffer(&up), stereo: false, gain: 1.0 });
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
        assert!(bank.voice(&SoundRequest { cue: "no-such-cue".into(), gain: 1.0, file: None }).is_none());
        assert!(bank.voice(&SoundRequest { cue: "coin".into(), gain: f32::NAN, file: None }).is_none());
        let request = SoundRequest { cue: "coin".into(), gain: 0.5, file: None };
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

    /// A sound asset kept decoded (its bytes are a stand-in: only their identity matters).
    fn decoded(rate: u32, channels: u16, samples: Vec<f32>) -> AudioFile {
        let clip = clip(rate, channels, samples);
        AudioFile {
            sample_rate: rate,
            channels,
            frames: clip.frames(),
            bytes: Arc::new(vec![0u8]),
            decoded: Some(clip),
        }
    }

    #[test]
    fn stereo_voices_keep_their_sides_and_mix_down_for_mono() {
        let request =
            SoundRequest { cue: "duet".into(), gain: 1.0, file: Some(decoded(8000, 2, vec![0.5, -0.25, 0.5, -0.25])) };
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
        mixer.set_loop(0, LoopRequest { samples: buffer(&samples), stereo: true, gain: 1.0 });
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
        let sound = decoded(22_050, 1, vec![0.25; 2205]);
        let mut cache = ClipCache::new(44_100);
        assert!(cache.get(&sound).is_none());
        let request = SoundRequest { cue: "chime".into(), gain: 0.5, file: Some(sound.clone()) };
        let voice = cache.voice(&request).unwrap();
        let Samples::Buffer(samples) = &voice.head.samples else { panic!("a buffer") };
        assert_eq!(samples.len(), 4410);
        let again = cache.get(&sound).unwrap();
        assert!(Arc::ptr_eq(&again, samples));
        assert_eq!(cache.bytes(), 4410 * 4);
        // Another file with the same samples is another clip.
        assert!(cache.get(&decoded(22_050, 1, vec![0.25; 2205])).is_none());
        assert!(cache.voice(&SoundRequest { gain: 0.0, ..request.clone() }).is_none());
        assert!(cache.voice(&SoundRequest { file: None, ..request.clone() }).is_none());
        // A file that streams is not the cache's.
        let streamed = AudioFile { decoded: None, ..decoded(22_050, 1, vec![0.25; 4]) };
        assert!(cache.voice(&SoundRequest { file: Some(streamed), ..request }).is_none());
        // At the cache's own rate the clip's buffer is shared: it costs nothing.
        let native = decoded(44_100, 1, vec![0.5; 100]);
        let shared = cache.prepare(&native).unwrap();
        assert!(Arc::ptr_eq(&shared, &native.decoded.as_ref().unwrap().samples));
        assert_eq!(cache.bytes(), 4410 * 4);
    }

    #[test]
    fn the_clip_cache_drops_clips_played_longest_ago_but_never_one_playing() {
        // Room for two resampled clips of 1000 samples (4000 bytes each).
        let mut cache = ClipCache::with_budget(16_000, 8000);
        let sounds: Vec<AudioFile> = (0..4).map(|i| decoded(8000, 1, vec![0.1 * i as f32; 500])).collect();
        let play = |cache: &mut ClipCache, sound: &AudioFile| {
            cache.voice(&SoundRequest { cue: "s".into(), gain: 1.0, file: Some(sound.clone()) }).unwrap()
        };
        drop(play(&mut cache, &sounds[0]));
        drop(play(&mut cache, &sounds[1]));
        assert_eq!((cache.len(), cache.bytes()), (2, 8000));
        // Sound 0 plays again, so sound 1 is the one played longest ago: a third clip pushes it
        // out.
        drop(play(&mut cache, &sounds[0]));
        drop(play(&mut cache, &sounds[2]));
        assert_eq!((cache.len(), cache.bytes()), (2, 8000));
        assert!(cache.get(&sounds[1]).is_none(), "the least recently used clip went");
        assert!(cache.get(&sounds[0]).is_some() && cache.get(&sounds[2]).is_some());
        // Clips that are playing stay, over budget if need be, until they finish.
        let mut mixer = Mixer::new(16_000);
        let playing: Vec<Voice> = sounds.iter().map(|sound| play(&mut cache, sound)).collect();
        assert_eq!(cache.len(), 4, "all four play");
        assert_eq!(cache.bytes(), 16_000);
        for voice in playing {
            mixer.start(voice);
        }
        cache.trim();
        assert_eq!(cache.len(), 4, "still playing");
        let mut out = vec![0.0f32; 2000];
        mixer.mix(&mut out, 1);
        assert_eq!(mixer.active(), 0, "done");
        cache.trim();
        assert_eq!(cache.bytes(), 8000, "back within budget once nothing plays");
        assert!(cache.get(&sounds[2]).is_some() && cache.get(&sounds[3]).is_some(), "the newest kept");
        // A game that is gone takes its clips with it.
        drop(sounds);
        cache.trim();
        assert!(cache.is_empty());
    }

    #[test]
    fn streams_play_in_voices_and_loops() {
        let samples: Vec<i16> = (0..3000).map(|i| ((i % 100) * 100) as i16).collect();
        let wav = crate::audio_assets::tests::wav(8000, 1, &samples);
        let file = crate::audio_assets::check(Arc::new(wav), false).unwrap();
        assert!(file.decoded.is_none());
        let expected: Vec<f32> = samples.iter().map(|s| f32::from(*s) / 32768.0).collect();
        // A voice: the stream's samples, then the voice ends.
        let (mut feed, handle) = Feed::new(&file, 8000, false, 0.5);
        let mut mixer = Mixer::new(8000);
        mixer.start(Voice::streamed(handle, false, 1.0).unwrap());
        let mut played = Vec::new();
        let mut out = vec![0.0f32; 256];
        for _ in 0..20 {
            feed.fill();
            mixer.mix(&mut out, 1);
            played.extend_from_slice(&out);
        }
        assert_eq!(&played[..3000], &expected[..]);
        assert!(played[3000..].iter().all(|s| *s == 0.0));
        assert_eq!(mixer.active(), 0);
        assert!(!feed.is_heard(), "the mixer let the stream go");
        // A loop: fades in and goes round; a gap (the feed behind) is silence, not a stop.
        let (mut feed, handle) = Feed::new(&file, 8000, true, 0.5);
        let request = LoopRequest { samples: Some(Samples::Stream(handle.clone())), stereo: false, gain: 1.0 };
        mixer.set_loop(0, request.clone());
        let mut out = vec![0.0f32; 1000];
        mixer.mix(&mut out, 1);
        assert!(out.iter().all(|s| *s == 0.0), "nothing decoded yet");
        let mut played = Vec::new();
        for _ in 0..30 {
            feed.fill();
            mixer.mix(&mut out, 1);
            played.extend_from_slice(&out);
        }
        // Past the fade-in (9600 frames) the loop is the file over and over, with no gap.
        let tail = &played[played.len() - 6000..];
        let offset = (0..3000).find(|&at| tail[at..at + 3000] == expected[..]).expect("a whole pass");
        assert_eq!(&tail[offset + 3000..], &expected[..3000 - offset]);
        // The same stream again is the same loop: only its gain changes.
        mixer.set_loop(0, LoopRequest { gain: 0.5, ..request });
        assert!(mixer.loop_playing(0));
        mixer.set_loop(0, LoopRequest::default());
        for _ in 0..20 {
            feed.fill();
            mixer.mix(&mut out, 1);
        }
        assert!(!mixer.loop_playing(0));
        drop(handle);
        assert!(!feed.is_heard());
    }
}
