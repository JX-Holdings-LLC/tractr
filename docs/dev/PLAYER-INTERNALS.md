# Player internals

How `farm-player` works inside. What players see is in
[The game player](../PLAYER.md); how Export Game packages the player is in
[Export internals](EXPORT-INTERNALS.md).

`farm-player` runs a compiled `game.cart`. It is the program inside every
exported game, and the same player runs the editor's Play Mode through
`farm-ffi` ([In the editor](#in-the-editor)) and the web demo through
`farm-wasm`. It has three faces:

- **The game.** A window with a title screen, save slots, a pause menu,
  settings and credits around the game (the *game shell*, drawn by
  `farm-ui`). Keyboard, mouse and gamepads work everywhere.
- **Headless.** `--headless` replays commands without a window and prints a
  JSON report; CI and the export checks use it.
- **Screenshots.** `--screenshot` renders frames without a window and writes
  a PNG; the screenshot goldens and store pictures use it.

```sh
cargo run -p farm-player -- --cart game.cart
cargo run -p farm-player -- --headless --cart game.cart --replay replay.json --save slot1.sav
cargo run -p farm-player -- --screenshot title.png --cart game.cart --size 1280x800 --frames 60
```

The flags, the replay format and the screenshot script format are in
[The game player](../PLAYER.md#command-line-options).

## Embedding the player

The player is a library first. `Player` owns a `PlaySession`, the in-game UI
(`farm-ui`), the shell's screens, a save store, a settings store and the
renderer. It needs no window, clock or OS access, and it is `Send`, so a host
can step it on a background thread and hand the pixels to its UI thread.

```rust
let mut player = Player::from_cartridge_bytes(&cart, PlayerOptions::standalone())?;
loop {
    let out = player.frame(dt_seconds, &events, width, height)?;
    // out.pixels: premultiplied RGBA; out.sounds; out.requests (Quit, SetFullscreen, SetTitle)
}
```

- `PlayerMode::Standalone` shows the title screen first. `Embedded` (the
  editor's Play Mode) starts in the game and has no title screen, save slots,
  autosave or quitting; its pause menu has Resume and Settings.
- `Player::from_cartridge`, `from_cartridge_bytes` and `from_project` (an
  editor project, through `create_content_from_project`,
  `StartState::from_project` and `Presentation::from_project`).
- For the editor: `state()`, `session()`, `debug(DebugAction)`,
  `replace_state`, `run_command`, `synced_project()` ("keep changes" when
  started from a project), `plugin_errors()`, `settings`/`set_settings`.
- `step` is `frame` without pixels. Engine panics come back as
  `PlayerError::Engine`; the player then refuses further frames.
- `InputEvent`: key down/up (farm-runtime key names), text, pointer
  move/down/up/leave, wheel, gamepad buttons and axes, focus lost.
- Stores: `FsSaveStore`/`FsSettingsStore` (files) and
  `MemorySaveStore`/`MemorySettingsStore` (tests, the editor).

A host with a renderer of its own calls `frame_composition` instead of
`frame`: the same frame, returned as the `farm_render::Composition` the CPU
path would rasterize (the world's draw list, native size, upscale and tints;
the UI's draw list; their image stores). `compose_pixels` rasterizes that
frame on the CPU after all.

The crate's `desktop` feature (on by default) adds the window: winit and
softbuffer, gilrs gamepads, cpal audio and the user folders; its `gpu`
feature (on by default too, so in the Export Game templates) adds the GPU
renderer ([Renderers](#renderers)). farm-ffi and the WebAssembly build use the
library without them; farm-ffi turns on `audio-out` alone (cpal through
`speaker::SpeakerThread`, which keeps the stream on its own thread so the
player can move between threads).

## In the editor

Play Mode embeds an `Embedded` player through `fe_player_*`
(`crates/farm-ffi/src/player.rs`, `RustPlayer` in `FarmEngine.Interop`):

- `fe_player_new` takes a cartridge (or project JSON) and
  `{seed, reducedMotion, uiScale, audio, locale}`. Play Mode passes the
  cartridge the F# compiler makes for the playtest
  (`CartridgeCompiler.CompileForPlaytest`: the export bytes, even while
  Problems still lists errors), so a playtest runs what Export Game ships;
  `locale` is the editor's language, which the game interface follows until
  the player picks one in Settings.
- `fe_player_frame` takes `{dt, events, width, height, render}` and returns
  the frame size, a small JSON block (sounds, requests, screen, whether a
  panel or modal is open) and the premultiplied RGBA pixels.
- `fe_player_debug` runs the debug drawer's actions (`DebugAction`),
  `fe_player_state_json` feeds "keep changes" (F# `Playtest.applyState`
  writes the state back into the project; `fe_player_synced_project` does the
  same in Rust for a player started from project JSON), and `fe_player_query_json`
  answers the drawer's summary, widget rectangles, toasts and plugin errors
  (the last three for tests).
- An engine failure or panic poisons the handle: every later call fails, and
  the editor ends the playtest without keeping changes.

`PlayModeView` renders at the surface's device pixels (at most 1920×1200;
larger surfaces are scaled up), maps Avalonia keys to the engine's key names
and pointer positions to frame pixels, and runs each frame on a worker thread
under one lock, so plugin hooks never block the editor. Editor shortcuts
(Ctrl/Alt combinations, F5/F6) stay with the editor.

## Input

Hosts pass raw events (`InputEvent`): key down and up by `farm-runtime` key
names, text, pointer move, down, up and leave, the wheel, gamepad buttons and
axes, and focus lost. The player routes them: keys bound to game actions
become the engine's keys, and the UI gets navigation, pointer and raw keys.

Rebinding happens in the player, not the engine. The engine still sees its
default keys: the player translates a pressed key to the key its action has
by default, and swallows default game keys that were rebound away. Creator
hotkeys and digits pass through unchanged. Interact presses on open soil go
to the engine as `interactWith` commands carrying the held seed or
fertilizer.

The pointer reaches a plugin minigame once per frame, in canvas units, so
`update` sees it one frame after the window drew under it
([Pack plugins](../PLUGINS.md#minigames)).

## Settings and user folders

Settings are TOML, stored apart from the saves through the
[directories](https://crates.io/crates/directories) crate
([Saves](../PLAYER.md#saves) has the paths). Every field has a default, so a
partial or older file loads, and an unreadable file is ignored.
`FARM_PLAYER_USER_DIR` puts saves, settings and screenshots under one folder
(tests, portable installs). The file also keeps the renderer
(`renderer = "auto"`, `"gpu"` or `"cpu"` under `[display]`), which has no row
in the settings screen. The first run takes fullscreen and integer scaling
from the cartridge's `GameInfo` (`window.fullscreen`, `pixelScale`).

Until the player picks a language, the game follows the system's, then the
game's `settings.locale`, then English; in the editor's Play Mode the
editor's language comes first. Engine messages come from the message catalog
(`farm_sim::messages`) and are translated by `farm-ui`'s table for the
language (`Lang::message`), falling back to English.

## Saves

The game has three save slots. **New Game** starts in the first free slot (or
asks for one), and the game **autosaves each morning** to its slot, as farming
games do. The pause menu saves to any slot (another game's slot asks first) and
loads any slot. **Continue** on the title screen loads the most recent save.

A save is the binary `FGSV` format (`schemas/save.fbs`): a header (game id,
game version, cartridge hash), a preview the slot list reads without loading
the game (farm name, day, season, year, money, play time, when it was saved and
a thumbnail of the scene) and the zstd-compressed state. A save from another
game is refused; one from a newer version loads with a warning; items the game
no longer has go to quarantine.

Files are written through a temporary file of the running copy of the game,
flushed to the disk before a rename puts it in place (and, on Linux and macOS,
the folder flushed after it), so a crash or power loss never leaves half a
save. Before a slot is replaced, its previous save becomes `slot<N>.bak`: when
a slot's save is missing or does not load, the slot list shows the backup and
loading it says so. Temporary files that a crash left behind are removed at
start-up.

Saving is synchronous, on the simulation thread
([Architecture](ARCHITECTURE.md#saves-rust-only)).

## Crash logs

A panic writes `crash-<time>-<pid>.log` next to the saves (in the temporary
folder when there is no user folder) and prints its path. The log has the
player version, the game and its version, the cartridge hash, the slot, tick,
day and scene, the state hash, the last 64 commands with their ticks and
recent plugin errors. The simulation is deterministic, so the last save plus
that command log replays the crash.

Logs are created as new files (never through an existing file or link), two
crashes in the same second get separate logs, and only the newest ten are
kept. The game report in a log is refreshed every second; a panic outside the
frame (the audio thread, the window system) says how many frames old it is.

When the game stops with an error, or its cartridge does not load, the window
shows what happened and where the crash log is (an exported Windows game has
no console), until it is closed or Esc or Enter is pressed. Closing the window
while a game runs asks first, like Quit in the pause menu; closing again while
that question is up closes at once.

## Sound and music

The built-in sound effects are `farm-runtime`'s synthesized presets, rendered
once when the device opens. Music and ambience are built-in loops composed
and synthesized by the engine's own code (`farm_runtime::music`: a fixed
score, small instruments and seeded noise), so there are no recorded or
third-party audio files to license or ship. Day music and birdsong play from
6:00 to 19:00, night music and crickets after that, rain under rainy
weather, no ambience in indoor scenes, and day music on the title screens;
the Music volume sets both, and the game fades between loops. The loops
render on a thread of their own when the device opens; the web demo renders
them with `musicSamples` and loops them with WebAudio. Sound follows the
default output device: when it goes away or another one becomes the
default, the player reopens the stream on the current default device.

A game can bring its own `sound` and `music` assets (Ogg Vorbis or WAV),
shipped in the cartridge's asset table like its images:

- **Decoding.** The player decodes each sound and music asset once when the
  game loads, to check it and measure it (`farm_player::audio_assets`, with
  the pure-Rust lewton and hound decoders), at the file's own sample rate.
  WAV may be 8- to 32-bit PCM or 32-bit float; mono and stereo stay as they
  are, more channels are mixed down to mono. A file is at most 10 minutes
  long. The format comes from the file's first bytes, not its MIME type.
- **Memory.** Decoded audio takes about 21 MB a minute of stereo at
  44.1 kHz, so only sounds up to 5 seconds long stay decoded. Music and
  longer sounds keep just their file (the bytes the cartridge's asset table
  holds anyway, not a copy) and are decoded and resampled while they play,
  about half a second ahead (`farm_player::audio_stream`): a stream holds a
  few thousand frames of its file, never all of it.
- **Sounds.** `playSound(id)` plays the sound asset `id` once, mixed with
  everything else at the sound-effects volume. Without such an asset the
  built-in preset of that name plays. An asset whose id is a
  built-in cue's name (`coin`, `harvest`, `ui`…) replaces that cue wherever
  the game plays it.
- **Music.** The music is chosen every frame
  (`farm_runtime::music::chosen_music`); the first that applies wins:
  1. the music an event or cutscene chose (`playMusic`), until the player
     is in another scene, however they got there: a transition, a warp, the
     mine's ladders and exit, or the debug drawer's teleport (an empty id
     goes back to the place's music);
  2. today's festival's music;
  3. the weather's music, outdoors only (not in an indoor scene);
  4. the scene's music;
  5. the season's music;
  6. none of them: the built-in day and night music.

  The chosen asset loops at the Music volume, and the game crossfades
  (1.2 seconds) when the choice changes. The built-in music is silent while
  an asset plays; the ambience (birds, crickets, rain) plays either way. The
  title screens keep the built-in day music.
- **Fallbacks.** An asset that is missing, has the other type (a `sound`
  named as music, or the reverse), or does not decode (not Ogg Vorbis or WAV,
  broken, too long) is left out when the game loads
  (`Player::audio_assets().failed()` lists it with the reason), and the
  built-in sound or music plays in its place. Audio never stops the game.
  The editor's Problems check (`AudioFiles` in F#) reads each file's
  structure the way lewton and hound do (Ogg pages and their CRCs, the
  Vorbis headers; WAV chunks and formats) and reports what would be left out
  as `asset.audioUnreadable`, an error that blocks Export.
- **Preview.** The Art tab's play button plays one asset through
  `farm_player::preview` (feature `audio-out`, via `fe_audio_preview_play`):
  the same decoder, resampled to the device's rate, on a stream of its own
  thread, without the game's sound bank or music loops.
- **Playback.** A `SoundRequest` carries its sound asset's `AudioFile` (with
  its decoded `Clip` when it is short), and a `MusicCue` its music asset
  (`asset`, with the built-in `music` as the fallback), so a host needs
  nothing else. The desktop game and the editor's Play Mode (`speaker`)
  resample a short sound to the device's rate the first time it plays (cubic
  interpolation) and keep it in a cache of 16 MB; past that the sounds played
  longest ago are dropped, never one that is still playing. Music and long
  sounds stream: a thread of their own (`Feeder`) decodes and resamples them
  with the same interpolation and hands the audio callback ready samples
  through a fixed queue, so the callback never decodes, allocates or waits.
  Looped music goes from its end straight back into its start, interpolated
  across the seam. The web demo plays them too: a frame names them
  (`info.sounds[].asset`, `info.music.asset`), the page fetches the decoded
  samples with `Player.audioClip(id)` (music and long sounds are decoded for
  that call; the page keeps the `AudioBuffer`) and WebAudio resamples them
  ([crates/farm-wasm/README.md](../../crates/farm-wasm/README.md#sound)).

Like the built-in sounds, none of this reaches the simulation: the state,
its hash and replays are the same with or without sound.

## How a frame is drawn

1. Input is routed: bound keys become the engine's keys, and the UI gets
   navigation, pointer and raw keys.
2. The session steps (fixed timestep) unless a menu pauses the game. The
   camera centres on the player as drawn between the last two ticks, or on
   where a cutscene's camera looks, which pans between ticks the same way.
3. `farm-render` builds the world snapshot of the tiles the camera shows (a
   tile window, one tile of margin and the row below for tall objects), so a
   256×256 farm costs about what a small one does; image URLs are shared
   `Arc`s that stay the same from frame to frame. `farm-ui` draws the HUD,
   panels and shell screens into a draw list.
4. The world is rasterized at its native pixel-art size (the camera viewport,
   32-pixel tiles, one pixel larger for smooth scrolling) and scaled up with
   nearest neighbour: whole steps with integer scaling, else fitted;
   letterboxed when the scene is smaller than the window.
5. The UI is rasterized at full resolution on top, so text stays crisp.

Steps 4 and 5 are the same with the GPU renderer, on the GPU
([Renderers](#renderers)); the times below are the CPU renderer's.

The UI scales with the window (1× at 1280×800, 1.35× at 1920×1080) times the
interface size setting. 16:10 (Steam Deck) and 16:9 both lay out without
overlap; the HUD drops its labels and then wraps when space runs out. A host
can describe its surface with `Player::set_host_view` (the web player does,
per frame): the pixel density, so the fit is computed in CSS pixels and the
UI keeps its size on dense phone screens, and the strip its on-screen touch
controls cover at the bottom, which the hint row, creator panels, "Made with"
credit, title menu and dialogue box stay above. Desktop windows keep the
defaults (density 1, no controls).

Frame times in a release build (starter farm, one thread, a 2.1 GHz Xeon
cloud core), for the whole frame: input, UI, world and rasterizing:

| Screen | 1280×800 | 1920×1080 |
|---|---|---|
| Title screen | 3.6 ms | 4.0 ms |
| Gameplay, walking | 3.1 ms | 4.3 ms |
| Shop open | 5.5 ms | 8.8 ms |

The UI keeps these low by drawing text from cached glyph coverage (one mask
per glyph, size and quarter-pixel position, coloured while blending, so
fading text reuses it; the least recently used masks make room), filling
rectangles with direct pixel loops, dropping commands outside the screen or
clip, building a clip's full-frame mask only for general path fills (and
then only rewriting the rows of the old and new clip), and dimming the world
at its native size instead of the full frame. The largest remaining cost is a
modal's soft shadow (about 2 ms at 1080p).

At high resolutions the whole frame is still recomposed every frame at full
physical resolution (fill, upscale, UI, then a copy into the window), also
on menus where nothing moves. `farm-bench` measures gameplay frames at
1280×800 and 1920×1080 against the 60 Hz budget, the same on a 256×256 farm,
and reports 2560×1600 and 3840×2160 without a budget (one run on a shared cloud machine:
5.4, 8.2 and 11.1 ms, then 13.7 and 23.8 ms). Capping the UI's internal resolution, skipping
unchanged menu frames and upscaling straight into the window's buffer are
the next steps.

## Renderers

The desktop game draws its frames with one of two renderers:

- **GPU** (`farm-gpu`, [wgpu] on Vulkan, DirectX 12 or Metal; no OpenGL). It
  executes the same `Composition` as the CPU path, in passes: the world's
  draw list into an offscreen texture at the native pixel-art size with the
  backdrop tints, then the frame at the window's size (the letterbox colour,
  the world scaled up with nearest neighbour through the CPU path's own
  column and row maps, the UI at full resolution), then a copy into the
  window's surface. Later palette-swap or particle passes go between
  those. Rectangles, rounded rectangles, nearest and bilinear images (with
  tints and opacity), glyphs, the day/night modulate and the light map (the
  tint plus up to 64 lights, summed per pixel in the CPU rasterizer's order
  and multiplied into the world) are drawn by the shader with the CPU
  rasterizer's arithmetic. Glyphs come from
  farm-render's own coverage masks, in a glyph atlas. Everything else
  (strokes, circles, ovals, lines, blurred shadows, outlined or very large
  text) is rasterized once by the CPU rasterizer into a sprite atlas, keyed
  by the command and the fractional part of its position, so a shape that
  moves by whole pixels is not rasterized again. Clips are scissor
  rectangles. A full atlas is cleared, then grown up to the device's limit;
  a frame that still does not fit is drawn on the CPU and shown through the
  GPU.
- **CPU** (`farm-render`'s rasterizer, presented through softbuffer). It is
  the reference: the screenshot goldens, `--screenshot`, headless runs, the
  editor's Play Mode (pixels handed to the C# app) and the web player all
  use it, and the GPU renderer is tested against it.

`--renderer`, else the `FARM_RENDERER` environment variable, else the
`renderer` setting picks one:

| Value | Renderer |
|---|---|
| `auto` (default) | The GPU when a hardware adapter works; otherwise the CPU. A software adapter (llvmpipe, WARP) rasterizes on the CPU too, so `auto` leaves it to the CPU renderer. |
| `gpu` | The GPU, software adapters included; the CPU when there is none. |
| `cpu` | The CPU. |

The game never stops or shows a black window over the renderer. No adapter,
no surface, a device that cannot be created, a device that is lost or
reports an error, or a panic in the GPU path switches the window to the CPU
renderer, which shows the same frame. The renderer in use, and why the
window fell back, is printed on stderr:

```text
Renderer: GPU, <adapter name> (discrete GPU, Vulkan)
Renderer: CPU (the GPU renderer is not available: no GPU adapter: only a software adapter: llvmpipe (…) (software, Vulkan))
Renderer: CPU (the GPU renderer stopped: the GPU device failed: …)
```

The two agree within a small tolerance: on the screenshot goldens' screens
at 1280×800 and 1920×1080, a walk with the camera between pixels, fitted
scaling and Spanish with the readable font, at most 25 pixels of a frame
differ by more than 6 in a channel (anti-aliased edges, which the GPU
computes from areas where tiny-skia scans), and most frames not at all
(lavapipe, `crates/farm-player/tests/gpu_parity.rs`). The GPU renderer adds
about 4.4 MB to the Linux template (13.4 → 17.8 MB stripped; 5.5 → 7.3 MB gzipped).

[wgpu]: https://wgpu.rs

## Tests

- `crates/farm-ui`: layout, focus navigation (keyboard and gamepad), clicks
  running the right commands, disabled states, wrapping, scaling.
- `crates/farm-player/tests/player.rs`: title → new game → walk → sleep →
  autosave → quit to title → continue (same state hash), slot previews,
  settings round trip, pause, embedded mode, plugins, a minigame, a gamepad,
  the game's own sounds and music and their fallbacks.
- `crates/farm-player/src/audio.rs`, `audio_assets.rs`: decoding (Ogg Vorbis
  from `fixtures/audio/tone.ogg`, WAV), refused files, resampling, stereo
  mixing and the clip cache; `farm_runtime::music`'s tests cover the music's
  order of precedence.
- `crates/farm-player/tests/screenshots.rs`: goldens of the title screen,
  settings, gameplay HUD, dialogue and shop at 1280×800 and 1920×1080, stored
  at half size in `fixtures/player/`. Bless with `FARM_PLAYER_BLESS=1`; write
  full-size frames with `FARM_PLAYER_SCREENSHOTS=<dir>` (the ignored
  `review_screens` test writes every other screen too).
- `crates/farm-player/tests/binary.rs`: `--screenshot`, and the real window
  under `xvfb-run` for a few frames with each renderer (skipped without
  Xvfb; `--renderer gpu` falls back to the CPU without a Vulkan driver).
- `crates/farm-player/tests/gpu_parity.rs` and `crates/farm-gpu/tests/parity.rs`:
  the GPU renderer offscreen against the CPU one, on the screenshot screens
  and on draw lists of every command, plus full atlases, oversized frames
  and a lost device (skipped without an adapter; on Linux,
  `mesa-vulkan-drivers` provides lavapipe). `FARM_GPU_SCREENSHOTS=<dir>`
  writes both frames.
- `crates/farm-player/tests/bench.rs` (ignored): frame times, run with
  `cargo test --release -p farm-player --test bench -- --ignored --nocapture`.
