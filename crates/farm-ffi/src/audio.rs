//! Sound and music assets in the editor: decoding a file the way the player does
//! ([`fe_audio_probe`]) and the Art tab's preview ([`fe_audio_preview_play`],
//! [`fe_audio_preview_stop`]), which plays one file at a time on the default output device
//! (`farm_player::preview`, feature `audio-out`).
//!
//! Without the feature (an editor built without ALSA) [`fe_audio_output`] answers 0, the
//! editor disables its play button, and [`fe_audio_preview_play`] refuses with a message.
//! The preview is one per process: the editor plays one file at a time, so it is a global
//! rather than a handle.

use crate::{bytes_arg, write, write_empty, FeBytes, FeResult};
use farm_player::audio_assets;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Preview volume: a little under full, like the game's default sound-effects volume.
#[cfg(feature = "audio-out")]
const PREVIEW_GAIN: f32 = 0.8;

#[cfg(feature = "audio-out")]
static PREVIEW: std::sync::Mutex<Option<farm_player::preview::Preview>> = std::sync::Mutex::new(None);

/// The preview slot; a panic while it was held leaves it usable (it holds no invariant).
#[cfg(feature = "audio-out")]
fn preview() -> std::sync::MutexGuard<'static, Option<farm_player::preview::Preview>> {
    PREVIEW.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// 1 when this library plays sound (built with `audio-out`), else 0. Whether a device is
/// there shows only when something plays.
#[no_mangle]
pub extern "C" fn fe_audio_output() -> u8 {
    u8::from(cfg!(feature = "audio-out"))
}

/// Writes `message` to `out` and returns `InvalidArgument`.
///
/// # Safety
/// A non-null `out` must be valid for writes.
unsafe fn refuse(out: *mut FeBytes, message: String) -> FeResult {
    // SAFETY: forwarded from the caller.
    unsafe { write(out, message.into_bytes()) };
    FeResult::InvalidArgument
}

/// Decodes the Ogg Vorbis or WAV file at `bytes` with the player's decoder. On success `out`
/// holds `{"sampleRate","channels","seconds"}` (JSON); otherwise the reason it won't play.
///
/// # Safety
/// `bytes` must point to `len` readable bytes (or be null with `len` 0); a non-null `out` must
/// be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn fe_audio_probe(bytes: *const u8, len: usize, out: *mut FeBytes) -> FeResult {
    // SAFETY: the caller guarantees `bytes`/`len`.
    let Some(file) = (unsafe { bytes_arg(bytes, len) }) else {
        // SAFETY: the caller guarantees `out`.
        return unsafe { refuse(out, "null bytes".to_owned()) };
    };
    match catch_unwind(AssertUnwindSafe(|| audio_assets::decode(file))) {
        Ok(Ok(clip)) => {
            let info = format!(
                r#"{{"sampleRate":{},"channels":{},"seconds":{}}}"#,
                clip.sample_rate,
                clip.channels,
                clip.seconds()
            );
            // SAFETY: the caller guarantees `out`.
            unsafe { write(out, info.into_bytes()) };
            FeResult::Ok
        }
        // SAFETY: the caller guarantees `out`.
        Ok(Err(error)) => unsafe { refuse(out, error) },
        Err(_) => {
            // SAFETY: the caller guarantees `out`.
            unsafe { write(out, b"the decoder failed".to_vec()) };
            FeResult::Panic
        }
    }
}

/// Plays the Ogg Vorbis or WAV file at `bytes` once on the default output device, stopping
/// whatever the preview played before. Fails (with the reason in `out`) when the file does not
/// decode, there is no device, or the library was built without sound; `out` is empty on success.
/// The file is decoded and resampled before this returns, so call it off the UI thread.
///
/// # Safety
/// As [`fe_audio_probe`].
#[no_mangle]
pub unsafe extern "C" fn fe_audio_preview_play(bytes: *const u8, len: usize, out: *mut FeBytes) -> FeResult {
    // SAFETY: the caller guarantees `bytes`/`len`.
    let Some(file) = (unsafe { bytes_arg(bytes, len) }) else {
        // SAFETY: the caller guarantees `out`.
        return unsafe { refuse(out, "null bytes".to_owned()) };
    };
    match catch_unwind(AssertUnwindSafe(|| play(file))) {
        Ok(Ok(())) => {
            // SAFETY: the caller guarantees `out`.
            unsafe { write_empty(out) };
            FeResult::Ok
        }
        // SAFETY: the caller guarantees `out`.
        Ok(Err(error)) => unsafe { refuse(out, error) },
        Err(_) => {
            // SAFETY: the caller guarantees `out`.
            unsafe { write(out, b"the preview failed".to_vec()) };
            FeResult::Panic
        }
    }
}

#[cfg(feature = "audio-out")]
fn play(file: &[u8]) -> Result<(), String> {
    let clip = audio_assets::decode(file)?;
    // The old preview stops before the new one opens the device.
    preview().take();
    let started = farm_player::preview::Preview::start(clip, PREVIEW_GAIN)?;
    *preview() = Some(started);
    Ok(())
}

#[cfg(not(feature = "audio-out"))]
fn play(file: &[u8]) -> Result<(), String> {
    audio_assets::decode(file)?;
    Err("this editor was built without sound output".to_owned())
}

/// Stops the preview (nothing happens when none plays).
#[no_mangle]
pub extern "C" fn fe_audio_preview_stop() {
    #[cfg(feature = "audio-out")]
    {
        // Joining the preview thread must not unwind into .NET.
        let _ = catch_unwind(|| drop(preview().take()));
    }
}

/// 1 while the preview plays, 0 once it ended, was stopped, or never started.
#[no_mangle]
pub extern "C" fn fe_audio_preview_playing() -> u8 {
    #[cfg(feature = "audio-out")]
    {
        catch_unwind(|| u8::from(preview().as_ref().is_some_and(farm_player::preview::Preview::is_playing)))
            .unwrap_or(0)
    }
    #[cfg(not(feature = "audio-out"))]
    {
        0
    }
}

#[cfg(test)]
#[allow(clippy::undocumented_unsafe_blocks)]
mod tests {
    use super::*;
    use crate::fe_bytes_free;

    const TONE_OGG: &[u8] = include_bytes!("../../../fixtures/audio/tone.ogg");

    fn text(bytes: FeBytes) -> String {
        let text = if bytes.ptr.is_null() {
            String::new()
        } else {
            String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(bytes.ptr, bytes.len) }).into_owned()
        };
        unsafe { fe_bytes_free(bytes) };
        text
    }

    fn probe(file: &[u8]) -> (FeResult, String) {
        let mut out = FeBytes::empty();
        let result = unsafe { fe_audio_probe(file.as_ptr(), file.len(), &mut out) };
        (result, text(out))
    }

    #[test]
    fn probing_decodes_like_the_player() {
        let (result, info) = probe(TONE_OGG);
        assert_eq!(result, FeResult::Ok, "{info}");
        let info: serde_json::Value = serde_json::from_str(&info).unwrap();
        assert_eq!((info["sampleRate"].as_u64(), info["channels"].as_u64()), (Some(22050), Some(2)));
        assert!((info["seconds"].as_f64().unwrap() - 0.25).abs() < 0.01);
        let (result, error) = probe(b"\x89PNG");
        assert_eq!(result, FeResult::InvalidArgument);
        assert!(error.contains("not an Ogg Vorbis or WAV"), "{error}");
        let mut out = FeBytes::empty();
        assert_eq!(unsafe { fe_audio_probe(std::ptr::null(), 4, &mut out) }, FeResult::InvalidArgument);
        drop(text(out));
    }

    #[test]
    fn the_preview_refuses_broken_files_and_stops_cleanly() {
        let mut out = FeBytes::empty();
        let broken = &TONE_OGG[..100];
        assert_eq!(
            unsafe { fe_audio_preview_play(broken.as_ptr(), broken.len(), &mut out) },
            FeResult::InvalidArgument
        );
        assert!(text(out).starts_with("Ogg Vorbis"));
        // CI has no output device: the call says so; a desktop plays a quarter second.
        let mut out = FeBytes::empty();
        let result = unsafe { fe_audio_preview_play(TONE_OGG.as_ptr(), TONE_OGG.len(), &mut out) };
        let message = text(out);
        assert!(result == FeResult::Ok || !message.is_empty(), "{result:?}");
        if !cfg!(feature = "audio-out") {
            assert_eq!(message, "this editor was built without sound output");
        }
        fe_audio_preview_stop();
        assert_eq!(fe_audio_preview_playing(), 0);
        fe_audio_preview_stop();
        assert_eq!(fe_audio_output(), u8::from(cfg!(feature = "audio-out")));
    }
}
