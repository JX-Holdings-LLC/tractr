//! `farm-player`: the Tractr game player.
//!
//! - [`Player`] is the embeddable graphical player: a [`session::PlaySession`], the in-game UI
//!   (`farm-ui`), the game shell (title, save slots, pause menu, settings, credits), saves and
//!   settings stores, and rendering, driven by one [`Player::frame`] call per frame. It is
//!   platform independent (no window, clock or OS access) and `Send`; the editor's Play Mode
//!   embeds it through farm-ffi.
//! - [`desktop`] (feature `desktop`, on by default) is the exported game: a window, gamepads,
//!   audio, user folders and crash logs around a `Player`.
//! - `speaker` (feature `audio-out`, part of `desktop`) plays a frame's sounds on the default
//!   output device; the editor enables it without the rest of the desktop game.
//! - [`run_cartridge`] is the headless path: load a cartridge, start from a test state, replay
//!   commands and debug shortcuts, check a hash and expectations, write a save (`--headless`).
#![forbid(unsafe_code)]

pub mod actions;
pub mod audio;
pub mod audio_assets;
pub mod audio_stream;
#[cfg(feature = "desktop")]
pub mod desktop;
pub mod headless;
pub mod input;
pub mod player;
pub mod plugins;
#[cfg(feature = "audio-out")]
pub mod preview;
pub mod render;
pub mod saves;
pub mod script;
pub mod session;
#[cfg(feature = "audio-out")]
pub mod speaker;

pub use audio::{ClipCache, LoopRequest, Mixer, MusicBank, Samples, SoundBank, SoundRequest, Voice};
pub use audio_assets::AudioAssets;
pub use farm_runtime::audio::{AudioFile, Clip};
pub use farm_runtime::music::{MusicAsset, MusicCue};
pub use farm_ui::{GamepadButton, Settings};
pub use input::{FrameInput, GamepadAxis, InputEvent, InputRouter, PointerButton};
pub use player::{
    FrameOutput, HostView, Player, PlayerError, PlayerMode, PlayerOptions, PlayerRequest, ScreenKind, StepOutput,
};
pub use saves::{
    FsSaveStore, FsSettingsStore, MemorySaveStore, MemorySettingsStore, SaveStore, SettingsStore, SLOT_COUNT,
};
pub use session::{DebugAction, PlaySession};

pub use headless::{run_cartridge, Expectation, HeadlessInput, HeadlessReport, HeadlessRun, ReplayFile};
