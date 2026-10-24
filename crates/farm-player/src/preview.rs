//! The editor's sound preview (feature `audio-out`): one decoded sound or music [`Clip`],
//! played once on the default output device until it ends or is stopped. The Art tab's play
//! button uses it through farm-ffi; Avalonia has no audio of its own.
//!
//! The stream lives on a thread of its own (cpal streams cannot move between threads), apart
//! from a game's [`crate::speaker`]: no sound bank or music loops are rendered for it, and
//! dropping the [`Preview`] closes the stream at once.
// The preview thread waits with a timeout; no wall clock reaches the simulation.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]

use crate::audio::resample;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream, StreamConfig};
use farm_runtime::audio::Clip;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

/// How often the preview thread looks whether the clip ended.
const POLL: Duration = Duration::from_millis(50);

/// A clip playing on the default output device. Dropping it stops the sound.
#[derive(Debug)]
pub struct Preview {
    stop: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
    playing: Arc<AtomicBool>,
}

impl Preview {
    /// Plays `clip` once at `gain` (0..=1). Fails when there is no output device or it refuses
    /// to open; the clip is resampled to the device's rate first, so long music takes a moment.
    pub fn start(clip: Clip, gain: f32) -> Result<Self, String> {
        let playing = Arc::new(AtomicBool::new(true));
        let flag = Arc::clone(&playing);
        let (opened_sender, opened) = mpsc::channel::<Result<(), String>>();
        let (stop, stopped) = mpsc::channel::<()>();
        let thread = std::thread::Builder::new()
            .name("farm-audio-preview".to_owned())
            .spawn(move || {
                let stream = match open(&clip, gain) {
                    Ok(stream) => stream,
                    Err(error) => {
                        flag.store(false, Ordering::Relaxed);
                        let _ = opened_sender.send(Err(error));
                        return;
                    }
                };
                let _ = opened_sender.send(Ok(()));
                wait(&stream.done, &stopped);
                drop(stream);
                flag.store(false, Ordering::Relaxed);
            })
            .map_err(|error| format!("could not start the preview: {error}"))?;
        let preview = Self { stop: Some(stop), thread: Some(thread), playing };
        match opened.recv() {
            Ok(Ok(())) => Ok(preview),
            Ok(Err(error)) => Err(error),
            Err(_) => Err("the preview stopped before it played".to_owned()),
        }
    }

    /// Whether the clip is still playing (false once it ended or the device went away).
    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        // Closing the channel ends the thread's wait, which drops the stream.
        self.stop.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Waits until the clip ended or the [`Preview`] went away.
fn wait(done: &AtomicBool, stopped: &Receiver<()>) {
    while !done.load(Ordering::Relaxed) {
        match stopped.recv_timeout(POLL) {
            Err(RecvTimeoutError::Timeout) => {}
            Ok(()) | Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// An open stream and the flag its callback sets when the clip ended (or the stream failed).
struct Playing {
    _stream: Stream,
    done: Arc<AtomicBool>,
}

fn open(clip: &Clip, gain: f32) -> Result<Playing, String> {
    let device =
        cpal::default_host().default_output_device().ok_or_else(|| "there is no sound output device".to_owned())?;
    let supported = device.default_output_config().map_err(|error| format!("the sound device refused: {error}"))?;
    let config = supported.config();
    let samples = resample(clip, config.sample_rate, false);
    let source = Source { samples, stereo: clip.channels == 2, gain: gain.clamp(0.0, 1.0) };
    let done = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&done);
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, config, source, flag),
        SampleFormat::F64 => build::<f64>(&device, config, source, flag),
        SampleFormat::I8 => build::<i8>(&device, config, source, flag),
        SampleFormat::I16 => build::<i16>(&device, config, source, flag),
        SampleFormat::I24 => build::<cpal::I24>(&device, config, source, flag),
        SampleFormat::I32 => build::<i32>(&device, config, source, flag),
        SampleFormat::I64 => build::<i64>(&device, config, source, flag),
        SampleFormat::U8 => build::<u8>(&device, config, source, flag),
        SampleFormat::U16 => build::<u16>(&device, config, source, flag),
        SampleFormat::U24 => build::<cpal::U24>(&device, config, source, flag),
        SampleFormat::U32 => build::<u32>(&device, config, source, flag),
        SampleFormat::U64 => build::<u64>(&device, config, source, flag),
        other => return Err(format!("the sound device wants {other} samples")),
    }
    .map_err(|error| format!("the sound device refused: {error}"))?;
    stream.play().map_err(|error| format!("the sound device refused: {error}"))?;
    Ok(Playing { _stream: stream, done })
}

/// The samples to play at the device's rate, mono or interleaved stereo.
struct Source {
    samples: Arc<[f32]>,
    stereo: bool,
    gain: f32,
}

impl Source {
    /// Writes the frames from `position` on into `out` (`channels` per frame), silence after the
    /// end; returns the new position.
    fn fill(&self, out: &mut [f32], channels: usize, position: usize) -> usize {
        let width = if self.stereo { 2 } else { 1 };
        let mut position = position;
        for frame in out.chunks_mut(channels.max(1)) {
            let (left, right) = match self.samples.get(position..position + width) {
                Some([left, right]) => (*left, *right),
                Some([mono]) => (*mono, *mono),
                _ => {
                    frame.fill(0.0);
                    continue;
                }
            };
            position += width;
            let (left, right) = (left * self.gain, right * self.gain);
            match frame {
                [only] => *only = 0.5 * (left + right),
                [first, second, rest @ ..] => {
                    *first = left;
                    *second = right;
                    rest.fill(0.5 * (left + right));
                }
                [] => {}
            }
        }
        position
    }

    fn ended(&self, position: usize) -> bool {
        position >= self.samples.len()
    }
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    source: Source,
    done: Arc<AtomicBool>,
) -> Result<Stream, cpal::Error>
where
    T: SizedSample + FromSample<f32> + Send + 'static,
{
    let channels = usize::from(config.channels);
    let mut position = 0;
    let mut scratch = vec![0.0f32; 16 * 1024];
    let failed = Arc::clone(&done);
    device.build_output_stream::<T, _, _>(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            if scratch.len() < data.len() {
                // Only when the device asks for a larger buffer than ever before.
                scratch.resize(data.len(), 0.0);
            }
            let mix = &mut scratch[..data.len()];
            position = source.fill(mix, channels, position);
            for (out, sample) in data.iter_mut().zip(mix.iter()) {
                *out = T::from_sample(*sample);
            }
            if source.ended(position) {
                done.store(true, Ordering::Relaxed);
            }
        },
        move |error: cpal::Error| {
            eprintln!("Sound preview stream error: {error}");
            if crate::speaker::is_fatal(error.kind()) {
                failed.store(true, Ordering::Relaxed);
            }
        },
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_fill_every_channel_and_then_silence() {
        let mono = Source { samples: Arc::from([0.5f32, -0.5]), stereo: false, gain: 1.0 };
        let mut out = [9.0f32; 6];
        let position = mono.fill(&mut out, 2, 0);
        assert_eq!(out, [0.5, 0.5, -0.5, -0.5, 0.0, 0.0]);
        assert!(mono.ended(position));
        let stereo = Source { samples: Arc::from([0.25f32, 0.5, 0.75, 1.0]), stereo: true, gain: 0.5 };
        let mut out = [9.0f32; 3];
        let position = stereo.fill(&mut out, 3, 0);
        assert_eq!(out, [0.125, 0.25, 0.1875]);
        assert!(!stereo.ended(position));
        let mut out = [9.0f32; 2];
        let position = stereo.fill(&mut out, 1, position);
        assert_eq!(out, [0.4375, 0.0]);
        assert!(stereo.ended(position));
    }

    #[test]
    fn without_a_device_the_preview_says_why() {
        // CI machines have no output device; a desktop plays a few silent frames.
        let clip = Clip { sample_rate: 8000, channels: 1, samples: Arc::from(vec![0.0f32; 80]) };
        match Preview::start(clip, 1.0) {
            Ok(preview) => drop(preview),
            Err(error) => assert!(!error.is_empty()),
        }
    }
}
