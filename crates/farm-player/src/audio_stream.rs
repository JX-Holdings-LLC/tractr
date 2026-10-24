//! Streamed playback: music and long sounds decoded and resampled while they play, so their
//! decoded samples are never all in memory (a minute of stereo at 44.1 kHz is about 21 MB).
//!
//! - [`Stream`] decodes a file a block at a time ([`crate::audio_assets::Decoder`]) and resamples
//!   it to the output rate as it goes, with the same cubic interpolation as
//!   [`crate::audio::resample`]: the same samples as resampling the whole file, from a few
//!   thousand frames of it. A looped stream starts the file again at its end and interpolates
//!   across the seam, so the loop has no click.
//! - A [`Feed`] and its [`StreamHandle`] share a fixed-size, lock-free queue of samples: the
//!   feed fills it from a stream, the audio callback plays the handle in a
//!   [`crate::audio::Mixer`]. Taking samples never allocates, frees or waits; decoding (which
//!   allocates) stays off the callback.
//! - [`Feeder`] is a thread that keeps every feed about [`AHEAD_SECONDS`] ahead of what plays,
//!   and drops a stream (on its own thread) once nothing plays it any more.
//!
//! Wall-clock time paces the feeder thread only; nothing here reaches the simulation.

use crate::audio::cubic;
use crate::audio_assets::{self, Decoder, FileReader};
use farm_runtime::audio::AudioFile;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

/// Seconds of output a feed keeps ready.
pub const AHEAD_SECONDS: f32 = 0.5;
/// How often the feeder thread tops its feeds up.
const FEED_INTERVAL: Duration = Duration::from_millis(20);
/// Output frames a feed makes at a time.
const FEED_BLOCK: usize = 1024;
/// Decoded source frames a stream keeps behind what it is resampling before dropping them.
const TRIM_FRAMES: usize = 4096;

/// A file decoded and resampled to an output rate while it plays.
pub struct Stream {
    file: AudioFile,
    rate: u32,
    looped: bool,
    /// Samples per frame: 1 or 2.
    width: usize,
    /// `file`'s rate over the output rate: source frames per output frame.
    step: f64,
    decoder: Option<Decoder<FileReader>>,
    /// Decoded source frames (interleaved), from frame `start` on. Frame numbers count on
    /// through a looped file's repeats.
    source: Vec<f32>,
    start: u64,
    /// Frames the current pass through the file has decoded (a pass that decodes nothing ends a
    /// looped stream rather than spinning).
    pass_frames: u64,
    /// The file will give no more frames: its end (not looped), or a decode error.
    exhausted: bool,
    /// The next output frame.
    next: u64,
    block: Vec<f32>,
}

impl std::fmt::Debug for Stream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stream")
            .field("file", &self.file)
            .field("rate", &self.rate)
            .field("looped", &self.looped)
            .field("next", &self.next)
            .finish_non_exhaustive()
    }
}

impl Stream {
    /// `file` at `rate`, from its start; `looped` plays it again and again. The decoder opens
    /// on the first [`Stream::read`].
    pub fn new(file: &AudioFile, rate: u32, looped: bool) -> Self {
        let rate = rate.max(1);
        Self {
            file: file.clone(),
            rate,
            looped,
            width: usize::from(file.channels.clamp(1, 2)),
            step: f64::from(file.sample_rate.max(1)) / f64::from(rate),
            decoder: None,
            source: Vec::new(),
            start: 0,
            pass_frames: 0,
            exhausted: false,
            next: 0,
            block: Vec::new(),
        }
    }

    /// Samples per frame: 1 (mono) or 2 (stereo, left then right).
    pub fn channels(&self) -> usize {
        self.width
    }

    /// Source frames decoded so far.
    fn decoded(&self) -> u64 {
        self.start + (self.source.len() / self.width) as u64
    }

    /// Decodes the next block of the file (the file again from its start when it is looped);
    /// false when there is no more.
    fn decode_more(&mut self) -> bool {
        while !self.exhausted {
            if self.decoder.is_none() {
                match audio_assets::open(&self.file) {
                    // The file checked out when the game loaded: the same layout again.
                    Ok(decoder) if usize::from(decoder.channels()) == self.width => self.decoder = Some(decoder),
                    _ => self.exhausted = true,
                }
                continue;
            }
            let Some(decoder) = self.decoder.as_mut() else { break };
            self.block.clear();
            match decoder.read(&mut self.block) {
                Ok(true) => {
                    let frames = self.block.len() / self.width;
                    if frames > 0 {
                        self.source.extend_from_slice(&self.block[..frames * self.width]);
                        self.pass_frames += frames as u64;
                        return true;
                    }
                }
                Ok(false) | Err(_) => {
                    self.decoder = None;
                    if !self.looped || self.pass_frames == 0 {
                        self.exhausted = true;
                    }
                    self.pass_frames = 0;
                }
            }
        }
        false
    }

    /// Fills `out` with the next frames at the output rate (interleaved, [`Stream::channels`]
    /// per frame) and returns the samples written: fewer than `out` holds only at the end.
    /// Decodes as it goes: call it off the audio callback.
    pub fn read(&mut self, out: &mut [f32]) -> usize {
        let width = self.width;
        let (source_rate, rate) = (u64::from(self.file.sample_rate.max(1)), u64::from(self.rate));
        let mut written = 0;
        for frame in out.chunks_exact_mut(width) {
            let position = self.next as f64 * self.step;
            let whole = position.floor();
            let t = (position - whole) as f32;
            let whole = whole as u64;
            while self.decoded() <= whole + 2 && self.decode_more() {}
            let total = self.decoded();
            // The end (as resampling the whole file would end): every output frame before the
            // source's last frame.
            if self.exhausted && (total == 0 || self.next * source_rate >= total * rate) {
                break;
            }
            let at = |offset: i64, channel: usize| -> f32 {
                let frame = (whole as i64 + offset).clamp(0, total as i64 - 1) as u64;
                self.source[(frame - self.start) as usize * width + channel]
            };
            for (channel, sample) in frame.iter_mut().enumerate() {
                *sample = cubic(at(-1, channel), at(0, channel), at(1, channel), at(2, channel), t);
            }
            self.next += 1;
            written += width;
        }
        // Frames before the next output frame's left neighbour are no longer needed.
        let needed = ((self.next as f64 * self.step).floor() as u64).saturating_sub(1);
        let held = (self.decoded() - self.start).saturating_sub(2);
        let behind = needed.saturating_sub(self.start).min(held) as usize;
        if behind > TRIM_FRAMES {
            self.source.drain(..behind * width);
            self.start += behind as u64;
        }
        written
    }
}

/// The queue between a [`Feed`] and its [`StreamHandle`]: a ring of samples written by one
/// thread and read by another, never resized.
#[derive(Debug)]
struct Ring {
    samples: Box<[AtomicU32]>,
    width: usize,
    /// Samples written and read so far (wrapping).
    written: AtomicUsize,
    read: AtomicUsize,
    /// The stream ended: what is in the ring is all there is.
    ended: AtomicBool,
}

impl Ring {
    fn new(frames: usize, width: usize) -> Self {
        let samples = (0..frames.max(1) * width).map(|_| AtomicU32::new(0)).collect();
        Self { samples, width, written: AtomicUsize::new(0), read: AtomicUsize::new(0), ended: AtomicBool::new(false) }
    }

    /// Whole frames there is room for.
    fn free_frames(&self) -> usize {
        let used = self.written.load(Ordering::Relaxed).wrapping_sub(self.read.load(Ordering::Acquire));
        (self.samples.len() - used) / self.width
    }

    /// Writes the whole frames of `samples` there is room for; returns the samples written.
    fn push(&self, samples: &[f32]) -> usize {
        let count = (samples.len() / self.width).min(self.free_frames()) * self.width;
        let written = self.written.load(Ordering::Relaxed);
        let capacity = self.samples.len();
        for (offset, sample) in samples[..count].iter().enumerate() {
            self.samples[written.wrapping_add(offset) % capacity].store(sample.to_bits(), Ordering::Relaxed);
        }
        self.written.store(written.wrapping_add(count), Ordering::Release);
        count
    }

    /// Takes one frame into `frame` (`width` samples): true when there was one.
    fn pop(&self, frame: &mut [f32]) -> bool {
        let read = self.read.load(Ordering::Relaxed);
        let available = self.written.load(Ordering::Acquire).wrapping_sub(read);
        if available < frame.len() {
            return false;
        }
        let capacity = self.samples.len();
        for (offset, sample) in frame.iter_mut().enumerate() {
            *sample = f32::from_bits(self.samples[read.wrapping_add(offset) % capacity].load(Ordering::Relaxed));
        }
        self.read.store(read.wrapping_add(frame.len()), Ordering::Release);
        true
    }
}

/// What a [`StreamHandle`] gave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pop {
    /// A frame.
    Frame,
    /// Nothing yet: the feed is behind (silence for this frame).
    Gap,
    /// The stream ended and everything in it has played.
    End,
}

/// The playing end of a stream: frames taken from its queue, without allocating, freeing or
/// waiting. Clones play the same stream (only one of them should take frames); handles compare
/// by identity.
#[derive(Debug, Clone)]
pub struct StreamHandle {
    ring: Arc<Ring>,
}

impl StreamHandle {
    /// Samples per frame: 1 (mono) or 2 (stereo).
    pub fn channels(&self) -> usize {
        self.ring.width
    }

    /// Takes the next frame into `frame` (exactly [`StreamHandle::channels`] samples).
    pub fn pop(&self, frame: &mut [f32]) -> Pop {
        // `ended` is read first: once it is set, everything the stream wrote is visible.
        let ended = self.ring.ended.load(Ordering::Acquire);
        if frame.len() == self.ring.width && self.ring.pop(frame) {
            Pop::Frame
        } else if ended {
            Pop::End
        } else {
            Pop::Gap
        }
    }

    /// Whether the stream ended and everything in it has played.
    pub fn finished(&self) -> bool {
        let ended = self.ring.ended.load(Ordering::Acquire);
        let available = self.ring.written.load(Ordering::Acquire).wrapping_sub(self.ring.read.load(Ordering::Relaxed));
        ended && available < self.ring.width
    }

    pub fn same(&self, other: &StreamHandle) -> bool {
        Arc::ptr_eq(&self.ring, &other.ring)
    }
}

/// The decoding end of a stream: [`Feed::fill`] tops its queue up. Fill it from one thread at
/// a time (a [`Feeder`]'s, or a test's).
#[derive(Debug)]
pub struct Feed {
    stream: Stream,
    ring: Arc<Ring>,
    block: Vec<f32>,
}

impl Feed {
    /// A stream of `file` at `rate` with a queue of `seconds` of output, and the handle that
    /// plays it.
    pub fn new(file: &AudioFile, rate: u32, looped: bool, seconds: f32) -> (Self, StreamHandle) {
        let stream = Stream::new(file, rate, looped);
        let width = stream.channels();
        let frames = (seconds.max(0.0) * rate.max(1) as f32) as usize;
        let ring = Arc::new(Ring::new(frames.max(FEED_BLOCK), width));
        let handle = StreamHandle { ring: Arc::clone(&ring) };
        (Self { stream, ring, block: vec![0.0; FEED_BLOCK * width] }, handle)
    }

    /// Decodes into the queue until it is full or the stream ends.
    pub fn fill(&mut self) {
        let width = self.ring.width;
        while !self.ring.ended.load(Ordering::Relaxed) {
            let frames = self.ring.free_frames().min(FEED_BLOCK);
            if frames == 0 {
                break;
            }
            let wanted = frames * width;
            let count = self.stream.read(&mut self.block[..wanted]);
            self.ring.push(&self.block[..count]);
            if count < wanted {
                self.ring.ended.store(true, Ordering::Release);
            }
        }
    }

    /// Whether a handle still plays the stream (or could).
    pub fn is_heard(&self) -> bool {
        Arc::strong_count(&self.ring) > 1
    }
}

/// A thread that keeps streams decoded a little ahead of the audio callback. Dropping it stops
/// the thread and drops its streams.
#[derive(Debug)]
pub struct Feeder {
    sender: Option<Sender<Feed>>,
    thread: Option<JoinHandle<()>>,
}

impl Feeder {
    /// Starts the thread; `None` when the platform has none.
    pub fn start() -> Option<Self> {
        let (sender, receiver) = mpsc::channel::<Feed>();
        let thread =
            std::thread::Builder::new().name("farm-audio-stream".to_owned()).spawn(move || feed(receiver)).ok()?;
        Some(Self { sender: Some(sender), thread: Some(thread) })
    }

    /// Starts streaming `file` at `rate` (`looped`: again and again); the handle plays it.
    /// The stream is decoded on the feeder's thread, beginning now; it stops once every handle
    /// is gone.
    pub fn play(&self, file: &AudioFile, rate: u32, looped: bool) -> Option<StreamHandle> {
        let (feed, handle) = Feed::new(file, rate, looped, AHEAD_SECONDS);
        self.sender.as_ref()?.send(feed).ok()?;
        Some(handle)
    }
}

impl Drop for Feeder {
    fn drop(&mut self) {
        // Closing the channel ends the thread's loop.
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The feeder thread: takes new feeds, tops every feed up, and drops the ones nothing plays.
/// A feed is only dropped here, after its last handle: the callback never frees a queue.
fn feed(receiver: Receiver<Feed>) {
    let mut feeds: Vec<Feed> = Vec::new();
    loop {
        match receiver.recv_timeout(FEED_INTERVAL) {
            Ok(feed) => feeds.push(feed),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        feeds.extend(receiver.try_iter());
        feeds.retain_mut(|feed| {
            if !feed.is_heard() {
                return false;
            }
            feed.fill();
            true
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::resample;
    use crate::audio_assets::{check, decode};

    const TONE_OGG: &[u8] = include_bytes!("../../../fixtures/audio/tone.ogg");

    fn tone() -> AudioFile {
        check(Arc::new(TONE_OGG.to_vec()), false).unwrap()
    }

    /// A mono 16-bit WAV file of a slow sine, `frames` long at `rate`.
    fn sine_wav(rate: u32, frames: usize) -> AudioFile {
        let samples: Vec<i16> = (0..frames).map(|i| ((i as f32 * 0.01).sin() * 12_000.0) as i16).collect();
        check(Arc::new(crate::audio_assets::tests::wav(rate, 1, &samples)), false).unwrap()
    }

    /// Everything a stream gives, read `chunk` samples at a time, up to `limit` samples.
    fn read_all(stream: &mut Stream, chunk: usize, limit: usize) -> Vec<f32> {
        let mut out = Vec::new();
        let mut buffer = vec![0.0f32; chunk];
        while out.len() < limit {
            let count = stream.read(&mut buffer);
            out.extend_from_slice(&buffer[..count]);
            if count < buffer.len() {
                break;
            }
        }
        out
    }

    #[test]
    fn streaming_gives_the_samples_of_resampling_the_whole_file() {
        let file = tone();
        let whole = decode(TONE_OGG).unwrap();
        for rate in [22_050, 44_100, 48_000, 8000] {
            let expected = resample(&whole, rate, false);
            // Where one read stops and the next starts does not change the samples.
            for chunk in [2, 1000, 4096] {
                let streamed = read_all(&mut Stream::new(&file, rate, false), chunk, usize::MAX);
                assert_eq!(streamed.len(), expected.len(), "{rate} Hz, chunks of {chunk}");
                assert!(streamed.iter().zip(expected.iter()).all(|(a, b)| a == b), "{rate} Hz, chunks of {chunk}");
            }
        }
        // A long file: dropping what was played keeps the same samples.
        let file = sine_wav(8000, 40_000);
        let expected = resample(&decode((*file.bytes).as_ref()).unwrap(), 44_100, false);
        let streamed = read_all(&mut Stream::new(&file, 44_100, false), 3000, usize::MAX);
        assert_eq!(streamed.len(), expected.len());
        assert!(streamed.iter().zip(expected.iter()).all(|(a, b)| a == b));
    }

    #[test]
    fn a_stream_keeps_only_a_little_of_the_file_decoded() {
        let file = sine_wav(8000, 100_000);
        let mut stream = Stream::new(&file, 48_000, false);
        let mut buffer = vec![0.0f32; 4096];
        let mut most = 0;
        while stream.read(&mut buffer) == buffer.len() {
            most = most.max(stream.source.len());
        }
        assert!(stream.next as usize > 500_000, "played it all");
        // A few blocks of source, not 100,000 frames.
        assert!(most < 3 * TRIM_FRAMES + 3 * 4096, "{most} samples held");
    }

    #[test]
    fn looped_streams_restart_cleanly() {
        let file = tone();
        let whole = decode(TONE_OGG).unwrap();
        // Twice the rate: each pass through the file is the same number of output frames.
        let looped = resample(&whole, 44_100, true);
        let pass = looped.len();
        let streamed = read_all(&mut Stream::new(&file, 44_100, true), 1234, 3 * pass + 10);
        assert!(streamed.len() >= 3 * pass, "a looped stream does not end");
        // The second and third passes are the whole file resampled as a loop (interpolated
        // across the seam), sample for sample.
        assert!(streamed[pass..2 * pass].iter().zip(looped.iter()).all(|(a, b)| a == b));
        assert_eq!(&streamed[pass..2 * pass], &streamed[2 * pass..3 * pass]);
        // The first pass only differs right at its start, where nothing comes before it.
        let differ = streamed[..pass].iter().zip(looped.iter()).filter(|(a, b)| a != b).count();
        assert!(differ <= 8, "{differ} samples differ");
        // At the file's own rate, every pass is the file itself.
        let length = whole.samples.len();
        let samples = read_all(&mut Stream::new(&file, 22_050, true), 998, 2 * length);
        assert_eq!(&samples[..length], &whole.samples[..]);
        assert_eq!(&samples[length..2 * length], &whole.samples[..]);
    }

    #[test]
    fn feeds_queue_samples_for_their_handle() {
        let file = sine_wav(8000, 3000);
        let expected = resample(&decode((*file.bytes).as_ref()).unwrap(), 8000, false);
        let (mut feed, handle) = Feed::new(&file, 8000, false, 0.1);
        let mut frame = [0.0f32; 1];
        assert_eq!(handle.pop(&mut frame), Pop::Gap, "nothing decoded yet");
        let mut played = Vec::new();
        loop {
            feed.fill();
            match handle.pop(&mut frame) {
                Pop::Frame => played.push(frame[0]),
                Pop::Gap => panic!("filled, yet empty"),
                Pop::End => break,
            }
            // Take a few hundred frames between fills, as a callback would.
            for _ in 0..300 {
                match handle.pop(&mut frame) {
                    Pop::Frame => played.push(frame[0]),
                    _ => break,
                }
            }
        }
        assert_eq!(played, expected.to_vec());
        assert!(handle.finished());
        // The feed is heard while a handle lives.
        assert!(feed.is_heard());
        let other = handle.clone();
        assert!(other.same(&handle));
        drop((handle, other));
        assert!(!feed.is_heard());
    }

    #[test]
    fn the_feeder_thread_plays_streams_and_drops_them_when_unheard() {
        let feeder = Feeder::start().unwrap();
        let file = sine_wav(8000, 2000);
        let handle = feeder.play(&file, 8000, false).unwrap();
        let mut frame = [0.0f32; 1];
        let (mut frames, mut waits) = (0, 0);
        // Wall-clock waiting is fine in a test: the thread decodes on its own schedule.
        while waits < 10_000 {
            match handle.pop(&mut frame) {
                Pop::Frame => frames += 1,
                Pop::Gap => {
                    waits += 1;
                    std::thread::sleep(Duration::from_millis(1));
                }
                Pop::End => break,
            }
        }
        assert_eq!(frames, 2000);
        assert!(handle.finished());
        let looped = feeder.play(&file, 8000, true).unwrap();
        drop(looped);
        drop(feeder);
    }
}
