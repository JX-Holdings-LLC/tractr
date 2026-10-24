//! A game's own sounds and music (M2): its `sound` and `music` custom assets, Ogg Vorbis or WAV,
//! as [`AudioFile`]s. When the game loads, every file is decoded once to check it and measure it.
//! A sound up to [`STREAM_SECONDS`] long keeps its decoded samples, at the file's own rate
//! ([`AudioFile::decoded`]); music and longer sounds keep only their file (the bytes the game's
//! asset table holds anyway), and hosts decode them while they play ([`Decoder`],
//! [`crate::audio_stream`]). Hosts resample to their output rate ([`crate::audio::resample`]).
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

use farm_cart::cartridge::ASSET_URL_PREFIX;
use farm_cart::AssetTable;
use farm_render::images::data_url_bytes;
use farm_runtime::audio::{AudioFile, Clip, FileBytes};
use farm_runtime::music::MusicAsset;
use farm_sim::schema::{custom_asset_types, CustomAsset};
use std::collections::BTreeMap;
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

/// The longest sound or music asset a game may have, in seconds (every file is decoded once
/// when the game loads, to check it).
pub const MAX_SECONDS: u32 = 600;
/// Sounds up to this long, in seconds, stay decoded in memory (decoded audio takes about 21 MB
/// a minute in stereo at 44.1 kHz); longer sounds, and all music, are decoded while they play.
pub const STREAM_SECONDS: u32 = 5;
/// Sample rates a file may have.
const RATES: std::ops::RangeInclusive<u32> = 1000..=384_000;
/// Frames a WAV file is read in at a time.
const WAV_BLOCK: usize = 4096;

/// The sound and music assets of a game, by asset id.
#[derive(Debug, Clone, Default)]
pub struct AudioAssets {
    sounds: BTreeMap<String, AudioFile>,
    music: BTreeMap<String, MusicAsset>,
    /// Assets that did not decode: (id, why).
    failed: Vec<(String, String)>,
}

/// A file in a cartridge's asset table: its bytes stay there, shared.
struct TableFile {
    table: Arc<AssetTable>,
    id: String,
}

impl AsRef<[u8]> for TableFile {
    fn as_ref(&self) -> &[u8] {
        self.table.get(&self.id).map_or(&[], |asset| asset.data.as_slice())
    }
}

impl AudioAssets {
    /// Checks the `sound` and `music` assets among `assets`. Their files are `asset:<id>`
    /// references into `table` (a cartridge; the bytes stay in the table) or `data:` URLs (an
    /// editor project). The first asset with an id wins.
    pub fn load(assets: &[CustomAsset], table: &Arc<AssetTable>) -> Self {
        let mut loaded = Self::default();
        for asset in assets {
            let is_music = asset.r#type == custom_asset_types::MUSIC;
            if !is_music && asset.r#type != custom_asset_types::SOUND {
                continue;
            }
            if asset.id.is_empty() || loaded.sounds.contains_key(&asset.id) || loaded.music.contains_key(&asset.id) {
                continue;
            }
            let bytes: Option<FileBytes> = match table.resolve(&asset.data_url) {
                Some(_) => asset
                    .data_url
                    .strip_prefix(ASSET_URL_PREFIX)
                    .map(|id| Arc::new(TableFile { table: Arc::clone(table), id: id.to_owned() }) as FileBytes),
                None => data_url_bytes(&asset.data_url).map(|bytes| Arc::new(bytes) as FileBytes),
            };
            let file = match bytes {
                Some(bytes) => check(bytes, !is_music),
                None => Err("the file is not embedded in the game".to_owned()),
            };
            match file {
                Ok(file) if is_music => {
                    let id = Arc::from(asset.id.as_str());
                    loaded.music.insert(asset.id.clone(), MusicAsset { id, file });
                }
                Ok(file) => {
                    loaded.sounds.insert(asset.id.clone(), file);
                }
                Err(error) => loaded.failed.push((asset.id.clone(), error)),
            }
        }
        loaded
    }

    /// The sound asset `id`.
    pub fn sound(&self, id: &str) -> Option<&AudioFile> {
        self.sounds.get(id)
    }

    /// The music asset `id`.
    pub fn music(&self, id: &str) -> Option<&MusicAsset> {
        self.music.get(id)
    }

    /// The sound or music asset `id`.
    pub fn file(&self, id: &str) -> Option<&AudioFile> {
        self.sounds.get(id).or_else(|| self.music.get(id).map(|music| &music.file))
    }

    /// The sound or music asset `id` decoded whole, at the file's own rate (hosts that play
    /// decoded buffers, like the web demo). Music and long sounds are decoded on every call:
    /// keep what it returns as long as it is needed, and no longer.
    pub fn decoded(&self, id: &str) -> Option<Clip> {
        let file = self.file(id)?;
        file.decoded.clone().or_else(|| decode((*file.bytes).as_ref()).ok())
    }

    /// Assets that did not decode, with the reason, in content order.
    pub fn failed(&self) -> &[(String, String)] {
        &self.failed
    }

    pub fn is_empty(&self) -> bool {
        self.sounds.is_empty() && self.music.is_empty()
    }
}

/// Decodes a whole Ogg Vorbis or WAV file to PCM at its own rate: mono and stereo stay as they
/// are, more channels are mixed down to mono. Samples outside [-1, 1] are clipped.
pub fn decode(bytes: &[u8]) -> Result<Clip, String> {
    let mut decoder = Decoder::new(Cursor::new(bytes))?;
    let (rate, channels) = (decoder.sample_rate(), decoder.channels());
    let limit = sample_limit(rate, channels, MAX_SECONDS);
    let mut samples = Vec::new();
    while decoder.read(&mut samples)? {
        if samples.len() > limit {
            return Err(too_long());
        }
    }
    if samples.is_empty() {
        return Err("the file has no sound".to_owned());
    }
    Ok(Clip { sample_rate: rate, channels, samples: Arc::from(samples) })
}

/// Checks that `bytes` decode, decoding the whole file once, and measures them. With
/// `keep_short`, a file up to [`STREAM_SECONDS`] long keeps its decoded samples (sounds).
pub fn check(bytes: FileBytes, keep_short: bool) -> Result<AudioFile, String> {
    let mut decoder = Decoder::new(Cursor::new((*bytes).as_ref()))?;
    let (rate, channels) = (decoder.sample_rate(), decoder.channels());
    let limit = sample_limit(rate, channels, MAX_SECONDS);
    let short = sample_limit(rate, channels, STREAM_SECONDS);
    let (mut block, mut kept, mut total, mut keeping) = (Vec::new(), Vec::new(), 0usize, keep_short);
    while decoder.read(&mut block)? {
        total += block.len();
        if total > limit {
            return Err(too_long());
        }
        if keeping && total > short {
            keeping = false;
            kept = Vec::new();
        }
        if keeping {
            kept.extend_from_slice(&block);
        }
        block.clear();
    }
    if total == 0 {
        return Err("the file has no sound".to_owned());
    }
    let decoded = keeping.then(|| Clip { sample_rate: rate, channels, samples: Arc::from(kept) });
    let frames = total / usize::from(channels);
    Ok(AudioFile { sample_rate: rate, channels, frames, bytes, decoded })
}

/// A decoder reading `file` from its start.
pub fn open(file: &AudioFile) -> Result<Decoder<FileReader>, String> {
    Decoder::new(FileReader { bytes: Arc::clone(&file.bytes), position: 0 })
}

/// Reads a file's shared bytes.
pub struct FileReader {
    bytes: FileBytes,
    position: u64,
}

impl std::fmt::Debug for FileReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileReader").field("position", &self.position).finish_non_exhaustive()
    }
}

impl Read for FileReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let bytes = (*self.bytes).as_ref();
        let start = usize::try_from(self.position).unwrap_or(usize::MAX).min(bytes.len());
        let count = buf.len().min(bytes.len() - start);
        buf[..count].copy_from_slice(&bytes[start..start + count]);
        self.position += count as u64;
        Ok(count)
    }
}

impl Seek for FileReader {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let len = (*self.bytes).as_ref().len() as i128;
        let position = match from {
            SeekFrom::Start(offset) => i128::from(offset),
            SeekFrom::End(offset) => len + i128::from(offset),
            SeekFrom::Current(offset) => i128::from(self.position) + i128::from(offset),
        };
        self.position = u64::try_from(position)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "seek before the start"))?;
        Ok(self.position)
    }
}

/// Decodes an Ogg Vorbis or WAV file a block at a time (a Vorbis packet, or a few thousand WAV
/// frames), so a stream never holds more of it decoded than it needs. A decoder bug never
/// stops the game: a panic inside the decoder comes back as an error.
pub struct Decoder<R: Read + Seek> {
    format: Format<R>,
    sample_rate: u32,
    /// Channels in the file (more than two are mixed down to mono).
    file_channels: usize,
    /// A packet or block before it is cleaned up.
    scratch: Vec<f32>,
    /// The decoder panicked: it reads nothing more.
    broken: bool,
}

enum Format<R: Read + Seek> {
    Wav { reader: hound::WavReader<R>, float: bool, scale: f32 },
    Vorbis(Box<lewton::inside_ogg::OggStreamReader<R>>),
}

impl<R: Read + Seek> std::fmt::Debug for Decoder<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let format = match self.format {
            Format::Wav { .. } => "WAV",
            Format::Vorbis(_) => "Ogg Vorbis",
        };
        f.debug_struct("Decoder")
            .field("format", &format)
            .field("sample_rate", &self.sample_rate)
            .field("file_channels", &self.file_channels)
            .finish_non_exhaustive()
    }
}

impl<R: Read + Seek> Decoder<R> {
    /// Reads the file's header; fails for a file that is neither Ogg Vorbis nor WAV, or whose
    /// format is not supported.
    pub fn new(mut reader: R) -> Result<Self, String> {
        let mut head = [0u8; 12];
        let mut filled = 0;
        while filled < head.len() {
            match reader.read(&mut head[filled..]) {
                Ok(0) => break,
                Ok(count) => filled += count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        reader.seek(SeekFrom::Start(0)).map_err(|error| error.to_string())?;
        let head = &head[..filled];
        let opened = if head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WAVE" {
            catch_unwind(AssertUnwindSafe(|| open_wav(reader)))
        } else if head.starts_with(b"OggS") {
            catch_unwind(AssertUnwindSafe(|| open_vorbis(reader)))
        } else {
            return Err("not an Ogg Vorbis or WAV file".to_owned());
        };
        let (format, sample_rate, file_channels) = opened.map_err(|_| "the decoder failed".to_owned())??;
        Ok(Self { format, sample_rate, file_channels, scratch: Vec::new(), broken: false })
    }

    /// The file's sample rate.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Channels [`Decoder::read`] produces: 1 or 2.
    pub fn channels(&self) -> u16 {
        if self.file_channels == 2 {
            2
        } else {
            1
        }
    }

    /// Appends the next block of samples to `out`: interleaved, [`Decoder::channels`] per frame,
    /// each finite and in [-1, 1]. `Ok(false)` at the end of the file, with nothing appended.
    pub fn read(&mut self, out: &mut Vec<f32>) -> Result<bool, String> {
        if self.broken {
            return Err("the decoder failed".to_owned());
        }
        match catch_unwind(AssertUnwindSafe(|| self.read_block(out))) {
            Ok(result) => result,
            Err(_) => {
                self.broken = true;
                Err("the decoder failed".to_owned())
            }
        }
    }

    fn read_block(&mut self, out: &mut Vec<f32>) -> Result<bool, String> {
        let channels = self.file_channels;
        self.scratch.clear();
        match &mut self.format {
            Format::Wav { reader, float, scale } => {
                let wanted = WAV_BLOCK * channels;
                if *float {
                    for sample in reader.samples::<f32>().take(wanted) {
                        self.scratch.push(sample.map_err(|e| format!("WAV: {e}"))?);
                    }
                } else {
                    for sample in reader.samples::<i32>().take(wanted) {
                        self.scratch.push(sample.map_err(|e| format!("WAV: {e}"))? as f32 * *scale);
                    }
                }
                if self.scratch.len() < channels {
                    return Ok(false);
                }
            }
            Format::Vorbis(reader) => {
                use lewton::samples::InterleavedSamples;
                let packet = reader
                    .read_dec_packet_generic::<InterleavedSamples<f32>>()
                    .map_err(|e| format!("Ogg Vorbis: {e}"))?;
                match packet {
                    // A chained stream with another layout ends the file: keep what came before.
                    Some(packet) if packet.channel_count == channels => self.scratch = packet.samples,
                    _ => return Ok(false),
                }
            }
        }
        push_frames(&self.scratch, channels, out);
        Ok(true)
    }
}

fn open_wav<R: Read + Seek>(reader: R) -> Result<(Format<R>, u32, usize), String> {
    let reader = hound::WavReader::new(reader).map_err(|e| format!("WAV: {e}"))?;
    let spec = reader.spec();
    let channels = usize::from(spec.channels);
    if !RATES.contains(&spec.sample_rate) || channels == 0 {
        return Err(format!("WAV: unsupported format ({} Hz, {channels} channels)", spec.sample_rate));
    }
    if reader.duration() / spec.sample_rate > MAX_SECONDS {
        return Err(too_long());
    }
    let float = spec.sample_format == hound::SampleFormat::Float;
    let scale = 1.0 / (1u64 << spec.bits_per_sample.clamp(1, 32).saturating_sub(1)) as f32;
    Ok((Format::Wav { reader, float, scale }, spec.sample_rate, channels))
}

fn open_vorbis<R: Read + Seek>(reader: R) -> Result<(Format<R>, u32, usize), String> {
    let reader = lewton::inside_ogg::OggStreamReader::new(reader).map_err(|e| format!("Ogg Vorbis: {e}"))?;
    let rate = reader.ident_hdr.audio_sample_rate;
    let channels = usize::from(reader.ident_hdr.audio_channels);
    if !RATES.contains(&rate) || channels == 0 {
        return Err(format!("Ogg Vorbis: unsupported format ({rate} Hz, {channels} channels)"));
    }
    Ok((Format::Vorbis(Box::new(reader)), rate, channels))
}

/// Appends the whole frames of `samples` (`channels` per frame) to `out`: mono and stereo as
/// they are, more channels mixed down to mono, every sample finite and in [-1, 1].
fn push_frames(samples: &[f32], channels: usize, out: &mut Vec<f32>) {
    let clean = |sample: f32| if sample.is_finite() { sample.clamp(-1.0, 1.0) } else { 0.0 };
    match channels {
        1 | 2 => {
            let whole = samples.len() - samples.len() % channels;
            out.extend(samples[..whole].iter().map(|sample| clean(*sample)));
        }
        _ => out.extend(
            samples
                .chunks_exact(channels)
                .map(|frame| clean(frame.iter().map(|sample| clean(*sample)).sum::<f32>() / channels as f32)),
        ),
    }
}

/// Samples allowed for `channels` at `rate` in `seconds`.
fn sample_limit(rate: u32, channels: u16, seconds: u32) -> usize {
    (rate as usize).saturating_mul(usize::from(channels)).saturating_mul(seconds as usize)
}

fn too_long() -> String {
    format!("longer than {} minutes", MAX_SECONDS / 60)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A quarter second of a 440 Hz tone, stereo at 22050 Hz (`ffmpeg -f lavfi -i
    /// "sine=frequency=440:sample_rate=22050:duration=0.25" -ac 2 -c:a libvorbis -q:a 0`).
    const TONE_OGG: &[u8] = include_bytes!("../../../fixtures/audio/tone.ogg");

    /// A 16-bit PCM WAV file of `frames` (one value per channel each).
    pub(crate) fn wav(rate: u32, channels: u16, frames: &[i16]) -> Vec<u8> {
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
        // Longer than a block: read a block at a time, the same samples.
        let long: Vec<i16> = (0..3 * WAV_BLOCK as i32 + 7).map(|i| (i % 2000 - 1000) as i16).collect();
        let clip = decode(&wav(8000, 1, &long)).unwrap();
        assert_eq!(clip.frames(), long.len());
        assert!(clip.samples.iter().zip(&long).all(|(a, b)| *a == f32::from(*b) / 32768.0));
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
        assert!(check(Arc::new(wav(8000, 1, &[])), true).unwrap_err().contains("no sound"));
        assert!(check(Arc::new(b"OggS\0\x02 not really".to_vec()), false).is_err());
    }

    #[test]
    fn short_sounds_stay_decoded_and_the_rest_streams() {
        let rate = 8000;
        let frames = |seconds: u32| vec![1000i16; (rate * seconds) as usize];
        let short = check(Arc::new(wav(rate, 1, &frames(STREAM_SECONDS))), true).unwrap();
        assert_eq!(short.frames, (rate * STREAM_SECONDS) as usize);
        assert_eq!(short.decoded.as_ref().map(Clip::frames), Some(short.frames));
        // One frame longer, or music: only the file is kept.
        let mut long = frames(STREAM_SECONDS);
        long.push(0);
        let long = check(Arc::new(wav(rate, 1, &long)), true).unwrap();
        assert_eq!((long.frames, long.decoded.is_none()), ((rate * STREAM_SECONDS) as usize + 1, true));
        let music = check(Arc::new(TONE_OGG.to_vec()), false).unwrap();
        assert!(music.decoded.is_none());
        assert_eq!((music.sample_rate, music.channels, music.frames), (22050, 2, decode(TONE_OGG).unwrap().frames()));
    }

    #[test]
    fn a_decoder_reads_a_file_a_block_at_a_time() {
        let file = check(Arc::new(TONE_OGG.to_vec()), false).unwrap();
        let mut decoder = open(&file).unwrap();
        assert_eq!((decoder.sample_rate(), decoder.channels()), (22050, 2));
        let (mut samples, mut blocks) = (Vec::new(), 0);
        while decoder.read(&mut samples).unwrap() {
            blocks += 1;
        }
        assert!(blocks > 4, "{blocks} blocks");
        assert_eq!(samples, decode(TONE_OGG).unwrap().samples.to_vec());
        assert!(!decoder.read(&mut samples).unwrap(), "stays at the end");
    }

    #[test]
    fn assets_load_from_the_table_and_from_data_urls() {
        let file = wav(8000, 1, &[100, 200]);
        let mut table = AssetTable::default();
        table.insert("hash", "audio/wav", file.clone());
        table.insert("song", "audio/ogg", TONE_OGG.to_vec());
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
        let table = Arc::new(table);
        let loaded = AudioAssets::load(
            &[
                asset("chime", custom_asset_types::SOUND, "asset:hash".to_owned()),
                asset("theme", custom_asset_types::MUSIC, inline),
                asset("tone", custom_asset_types::MUSIC, "asset:song".to_owned()),
                asset("tree", custom_asset_types::ART, "asset:hash".to_owned()),
                asset("missing", custom_asset_types::SOUND, "asset:nope".to_owned()),
                asset("broken", custom_asset_types::MUSIC, "data:audio/ogg;base64,AAAA".to_owned()),
                asset("chime", custom_asset_types::MUSIC, "asset:hash".to_owned()),
            ],
            &table,
        );
        let chime = loaded.sound("chime").unwrap();
        assert_eq!((chime.frames, chime.decoded.as_ref().map(Clip::frames)), (2, Some(2)));
        assert!(loaded.music("chime").is_none(), "the first asset with an id wins");
        assert_eq!(loaded.music("theme").map(|music| &*music.id), Some("theme"));
        assert!(loaded.sound("theme").is_none() && loaded.file("theme").is_some());
        assert!(loaded.file("tree").is_none());
        let failed: Vec<&str> = loaded.failed().iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(failed, ["missing", "broken"]);
        // Music keeps only its file: the table's bytes, not a copy. Decoding it on request gives
        // the whole file.
        let tone = &loaded.music("tone").unwrap().file;
        assert!(tone.decoded.is_none());
        assert_eq!((*tone.bytes).as_ref().as_ptr(), table.get("song").unwrap().data.as_ptr());
        assert_eq!(loaded.decoded("tone").unwrap().samples.to_vec(), decode(TONE_OGG).unwrap().samples.to_vec());
        assert!(Arc::ptr_eq(&loaded.decoded("chime").unwrap().samples, &chime.decoded.as_ref().unwrap().samples));
        assert!(loaded.decoded("tree").is_none());
    }
}
