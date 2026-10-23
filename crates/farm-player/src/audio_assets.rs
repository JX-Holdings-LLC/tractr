//! A game's own sounds and music (M2): its `sound` and `music` custom assets, Ogg Vorbis or WAV,
//! decoded once when the game loads into [`Clip`]s at the file's own sample rate. Hosts resample
//! them to their output rate ([`crate::audio::resample`]).
//!
//! - `playSound(id)` plays the sound asset `id` when there is one, else the built-in cue of
//!   that name ([`AudioAssets::sound`]).
//! - The music `farm_runtime::music::chosen_music` names plays when it is a music asset that
//!   decoded ([`AudioAssets::music`]); otherwise the built-in music plays.
//!
//! The format comes from the file's first bytes (`RIFF…WAVE`, `OggS`), not from its MIME type.
//! A file that does not decode, or that is longer than [`MAX_SECONDS`], is left out and listed
//! in [`AudioAssets::failed`]; the game then plays the built-in sound or music in its place.
//! Nothing here reaches the simulation.

use farm_cart::AssetTable;
use farm_render::images::data_url_bytes;
use farm_runtime::audio::Clip;
use farm_runtime::music::MusicAsset;
use farm_sim::schema::{custom_asset_types, CustomAsset};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::io::Cursor;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

/// The longest sound or music asset decoded, in seconds (decoded audio takes about 21 MB a
/// minute in stereo at 44.1 kHz).
pub const MAX_SECONDS: u32 = 600;
/// Sample rates a file may have.
const RATES: std::ops::RangeInclusive<u32> = 1000..=384_000;

/// The decoded sound and music assets of a game, by asset id.
#[derive(Debug, Clone, Default)]
pub struct AudioAssets {
    sounds: BTreeMap<String, Clip>,
    music: BTreeMap<String, MusicAsset>,
    /// Assets that did not decode: (id, why).
    failed: Vec<(String, String)>,
}

impl AudioAssets {
    /// Decodes the `sound` and `music` assets among `assets`. Their files are `asset:<id>`
    /// references into `table` (a cartridge) or `data:` URLs (an editor project). The first
    /// asset with an id wins.
    pub fn load(assets: &[CustomAsset], table: &AssetTable) -> Self {
        let mut loaded = Self::default();
        for asset in assets {
            let is_music = asset.r#type == custom_asset_types::MUSIC;
            if !is_music && asset.r#type != custom_asset_types::SOUND {
                continue;
            }
            if asset.id.is_empty() || loaded.sounds.contains_key(&asset.id) || loaded.music.contains_key(&asset.id) {
                continue;
            }
            let bytes = match table.resolve(&asset.data_url) {
                Some(file) => Some(Cow::Borrowed(file.data.as_slice())),
                None => data_url_bytes(&asset.data_url).map(Cow::Owned),
            };
            let clip = match bytes {
                Some(bytes) => decode(&bytes),
                None => Err("the file is not embedded in the game".to_owned()),
            };
            match clip {
                Ok(clip) if is_music => {
                    let id = Arc::from(asset.id.as_str());
                    loaded.music.insert(asset.id.clone(), MusicAsset { id, clip });
                }
                Ok(clip) => {
                    loaded.sounds.insert(asset.id.clone(), clip);
                }
                Err(error) => loaded.failed.push((asset.id.clone(), error)),
            }
        }
        loaded
    }

    /// The sound asset `id`.
    pub fn sound(&self, id: &str) -> Option<&Clip> {
        self.sounds.get(id)
    }

    /// The music asset `id`.
    pub fn music(&self, id: &str) -> Option<&MusicAsset> {
        self.music.get(id)
    }

    /// The sound or music asset `id` (hosts that fetch samples by id).
    pub fn clip(&self, id: &str) -> Option<&Clip> {
        self.sounds.get(id).or_else(|| self.music.get(id).map(|music| &music.clip))
    }

    /// Assets that did not decode, with the reason, in content order.
    pub fn failed(&self) -> &[(String, String)] {
        &self.failed
    }

    pub fn is_empty(&self) -> bool {
        self.sounds.is_empty() && self.music.is_empty()
    }
}

/// Decodes an Ogg Vorbis or WAV file to PCM at its own rate: mono and stereo stay as they are,
/// more channels are mixed down to mono. Samples outside [-1, 1] are clipped.
pub fn decode(bytes: &[u8]) -> Result<Clip, String> {
    let decoded = if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        // A decoder bug must not stop the game: the asset is left out instead.
        catch_unwind(AssertUnwindSafe(|| decode_wav(bytes)))
    } else if bytes.starts_with(b"OggS") {
        catch_unwind(AssertUnwindSafe(|| decode_vorbis(bytes)))
    } else {
        return Err("not an Ogg Vorbis or WAV file".to_owned());
    };
    let (rate, channels, samples) = decoded.map_err(|_| "the decoder failed".to_owned())??;
    clip(rate, channels, samples)
}

/// Samples allowed for `channels` at `rate`: [`MAX_SECONDS`] of them.
fn sample_limit(rate: u32, channels: usize) -> usize {
    (rate as usize).saturating_mul(channels).saturating_mul(MAX_SECONDS as usize)
}

fn too_long() -> String {
    format!("longer than {} minutes", MAX_SECONDS / 60)
}

fn decode_wav(bytes: &[u8]) -> Result<(u32, usize, Vec<f32>), String> {
    let reader = hound::WavReader::new(Cursor::new(bytes)).map_err(|e| format!("WAV: {e}"))?;
    let spec = reader.spec();
    let channels = usize::from(spec.channels);
    if !RATES.contains(&spec.sample_rate) || channels == 0 {
        return Err(format!("WAV: unsupported format ({} Hz, {channels} channels)", spec.sample_rate));
    }
    if reader.duration() / spec.sample_rate > MAX_SECONDS {
        return Err(too_long());
    }
    let samples = match spec.sample_format {
        hound::SampleFormat::Float => reader.into_samples::<f32>().collect::<Result<Vec<_>, _>>(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << spec.bits_per_sample.clamp(1, 32).saturating_sub(1)) as f32;
            reader.into_samples::<i32>().map(|sample| sample.map(|value| value as f32 * scale)).collect()
        }
    }
    .map_err(|e| format!("WAV: {e}"))?;
    Ok((spec.sample_rate, channels, samples))
}

fn decode_vorbis(bytes: &[u8]) -> Result<(u32, usize, Vec<f32>), String> {
    use lewton::inside_ogg::OggStreamReader;
    use lewton::samples::InterleavedSamples;
    let mut reader = OggStreamReader::new(Cursor::new(bytes)).map_err(|e| format!("Ogg Vorbis: {e}"))?;
    let rate = reader.ident_hdr.audio_sample_rate;
    let channels = usize::from(reader.ident_hdr.audio_channels);
    if !RATES.contains(&rate) || channels == 0 {
        return Err(format!("Ogg Vorbis: unsupported format ({rate} Hz, {channels} channels)"));
    }
    let limit = sample_limit(rate, channels);
    let mut samples = Vec::new();
    while let Some(packet) =
        reader.read_dec_packet_generic::<InterleavedSamples<f32>>().map_err(|e| format!("Ogg Vorbis: {e}"))?
    {
        if packet.channel_count != channels {
            // A chained stream with another layout: keep what came before it.
            break;
        }
        samples.extend_from_slice(&packet.samples);
        if samples.len() > limit {
            return Err(too_long());
        }
    }
    Ok((rate, channels, samples))
}

/// The clip of decoded samples: more than two channels mixed down to mono, every sample finite
/// and in [-1, 1].
fn clip(rate: u32, channels: usize, samples: Vec<f32>) -> Result<Clip, String> {
    let clean = |sample: f32| if sample.is_finite() { sample.clamp(-1.0, 1.0) } else { 0.0 };
    let (channels, samples): (u16, Vec<f32>) = match channels {
        1 | 2 => {
            let whole = samples.len() - samples.len() % channels;
            (channels as u16, samples[..whole].iter().map(|sample| clean(*sample)).collect())
        }
        _ => (
            1,
            samples
                .chunks_exact(channels)
                .map(|frame| clean(frame.iter().map(|sample| clean(*sample)).sum::<f32>() / channels as f32))
                .collect(),
        ),
    };
    if samples.len() > sample_limit(rate, usize::from(channels)) {
        return Err(too_long());
    }
    if samples.is_empty() {
        return Err("the file has no sound".to_owned());
    }
    Ok(Clip { sample_rate: rate, channels, samples: Arc::from(samples) })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A quarter second of a 440 Hz tone, stereo at 22050 Hz (`ffmpeg -f lavfi -i
    /// "sine=frequency=440:sample_rate=22050:duration=0.25" -ac 2 -c:a libvorbis -q:a 0`).
    const TONE_OGG: &[u8] = include_bytes!("../../../fixtures/audio/tone.ogg");

    /// A 16-bit PCM WAV file of `frames` (one value per channel each).
    fn wav(rate: u32, channels: u16, frames: &[i16]) -> Vec<u8> {
        let spec = hound::WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut bytes = Cursor::new(Vec::new());
        let mut writer = hound::WavWriter::new(&mut bytes, spec).unwrap();
        for sample in frames {
            writer.write_sample(*sample).unwrap();
        }
        writer.finalize().unwrap();
        bytes.into_inner()
    }

    #[test]
    fn wav_files_decode_at_their_own_rate() {
        let clip = decode(&wav(22050, 2, &[16384, -16384, 0, 32767])).unwrap();
        assert_eq!((clip.sample_rate, clip.channels, clip.frames()), (22050, 2, 2));
        assert_eq!(&clip.samples[..], &[0.5, -0.5, 0.0, 32767.0 / 32768.0]);
        // More than two channels mix down to mono.
        let clip = decode(&wav(8000, 3, &[3000, 6000, 9000])).unwrap();
        assert_eq!((clip.channels, clip.samples.len()), (1, 1));
        assert!((clip.samples[0] - 6000.0 / 32768.0).abs() < 1e-6);
    }

    #[test]
    fn ogg_vorbis_files_decode() {
        let clip = decode(TONE_OGG).unwrap();
        assert_eq!((clip.sample_rate, clip.channels), (22050, 2));
        assert!((clip.seconds() - 0.25).abs() < 0.01, "{} s", clip.seconds());
        let loud = clip.samples.iter().fold(0.0f32, |max, sample| max.max(sample.abs()));
        // The tone peaks at 1/8 (ffmpeg's default).
        assert!((0.05..=0.2).contains(&loud), "{loud}");
        // Cut short: what decodes, or an error; never a panic.
        assert!(decode(&TONE_OGG[..100]).is_err());
        for end in [TONE_OGG.len() / 2, TONE_OGG.len() - 1] {
            if let Ok(cut) = decode(&TONE_OGG[..end]) {
                assert!(cut.frames() <= clip.frames());
            }
        }
    }

    #[test]
    fn broken_and_unknown_files_are_refused() {
        assert!(decode(b"").is_err());
        assert!(decode(b"\x89PNG\r\n\x1a\n").unwrap_err().contains("not an Ogg Vorbis or WAV"));
        let mut cut = wav(8000, 1, &[1, 2, 3, 4]);
        cut.truncate(20);
        assert!(decode(&cut).is_err());
        assert!(decode(b"OggS\0\x02 not really").unwrap_err().starts_with("Ogg Vorbis"));
        assert!(decode(&wav(8000, 1, &[])).unwrap_err().contains("no sound"));
    }

    #[test]
    fn assets_load_from_the_table_and_from_data_urls() {
        let file = wav(8000, 1, &[100, 200]);
        let mut table = AssetTable::default();
        table.insert("hash", "audio/wav", file.clone());
        let asset = |id: &str, kind: &str, url: String| CustomAsset {
            id: id.to_owned(),
            r#type: kind.to_owned(),
            data_url: url,
            ..CustomAsset::default()
        };
        let mut inline = serde_json::Value::from("asset:hash");
        farm_cart::cartridge::inline_assets(&mut inline, &table);
        let inline = inline.as_str().unwrap().to_owned();
        assert!(inline.starts_with("data:audio/wav;base64,"));
        let loaded = AudioAssets::load(
            &[
                asset("chime", custom_asset_types::SOUND, "asset:hash".to_owned()),
                asset("theme", custom_asset_types::MUSIC, inline),
                asset("tree", custom_asset_types::ART, "asset:hash".to_owned()),
                asset("missing", custom_asset_types::SOUND, "asset:nope".to_owned()),
                asset("broken", custom_asset_types::MUSIC, "data:audio/ogg;base64,AAAA".to_owned()),
                asset("chime", custom_asset_types::MUSIC, "asset:hash".to_owned()),
            ],
            &table,
        );
        assert_eq!(loaded.sound("chime").map(Clip::frames), Some(2));
        assert!(loaded.music("chime").is_none(), "the first asset with an id wins");
        assert_eq!(loaded.music("theme").map(|music| &*music.id), Some("theme"));
        assert!(loaded.sound("theme").is_none() && loaded.clip("theme").is_some());
        assert!(loaded.clip("tree").is_none());
        let failed: Vec<&str> = loaded.failed().iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(failed, ["missing", "broken"]);
    }
}
