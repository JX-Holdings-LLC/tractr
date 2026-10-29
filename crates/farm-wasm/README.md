# farm-wasm

The game for web pages: a `wasm-bindgen` API over the same player the desktop editor reaches
through `farm-ffi`. Both crates are thin bindings over `farm-host`, which holds the JSON
protocol, so a page sends the same requests as .NET and gets the same pixels, sounds and state
hashes. Export Game's web demo target uses it. Headless sessions and Edit Mode previews are
`farm-ffi` only: the editor is the desktop app.

## Build

```sh
tools/wasm/build.sh          # → tools/wasm/dist/ (gitignored)
node tools/wasm/smoke.mjs    # smoke test of that output in Node 22
```

The script builds `cargo build -p farm-wasm --target wasm32-unknown-unknown --release`, runs
`wasm-bindgen --target web` (it installs `wasm-bindgen-cli` of the exact version in
`Cargo.lock` when needed) and, when `wasm-opt` is on the `PATH`, `wasm-opt -O3`. The output is an
ES module (`farm_wasm.js`), the module (`farm_wasm_bg.wasm`) and TypeScript declarations
(`farm_wasm.d.ts`, which use TypeScript 5.7's typed-array generics such as
`Uint8ClampedArray<ArrayBuffer>`), which a Vite app imports directly.

Sizes (0.2.0): 9.9 MB after wasm-bindgen, 8.5 MB after `wasm-opt -O3`, 3.0 MB gzipped. About
6.7 MB is code; the data holds the UI fonts, the plugin guest (QuickJS) and the built-in art.

## Loading

```ts
import init, { Player, hashState, sfxSamples } from "./farm_wasm.js";

await init();                                  // fetches farm_wasm_bg.wasm next to the module
// or: await init({ module_or_path: url });   Node: initSync({ module: bytes })
```

Every call is synchronous and runs on the calling thread. A frame of the starter farm at
1280×800 takes about 6 ms in V8; run the player in a Web Worker to keep the page responsive
(post the pixels back as a transferable, or draw into an `OffscreenCanvas`).

Arguments documented as JSON take either a JavaScript value or its JSON text. JSON results
(`stateJson`, `syncedProject`, `queryJson`) are strings, exactly the bytes farm-ffi returns, so
hashes can be checked (`hashState(player.stateJson()) === player.hash()`); `JSON.parse` them as
needed. A value `JSON.stringify` cannot turn into JSON (a function, a symbol) throws a
`FarmError` of kind `"invalid"` like malformed JSON does. Games are project JSON (text, object or UTF-8 bytes, at the current project
schema version) or compiled cartridge bytes (`Uint8Array`/`ArrayBuffer`).

## `Player`

The graphical player: the game, its HUD, dialogue, shops, panels, minigames, toasts and shell
screens, drawn in Rust.

```ts
const player = new Player(game, { seed?, mode?, reducedMotion?, uiScale?, locale?, storage? });
const ctx = canvas.getContext("2d")!;
let pixels: Uint8ClampedArray | undefined;

function tick(dt: number, events: InputEvent[]) {
  const density = canvas.width / canvas.getBoundingClientRect().width;   // devicePixelRatio
  const frame = player.frame({ dt, events, width: canvas.width, height: canvas.height, density }, pixels);
  if (frame.pixels) {
    pixels = frame.pixels;                     // reused next frame while the size holds
    ctx.putImageData(new ImageData(frame.pixels, frame.width, frame.height), 0, 0);
  }
  for (const { cue, gain } of frame.info.sounds) play(cue, gain);
  if (frame.storageChanged) persist(player.exportStorage());
}
```

| Member | Mirrors | |
|---|---|---|
| `new Player(game, options?)` | `fe_player_new` | `mode: "embedded"` (default; Play Mode: straight into the game, no title screen or save slots) or `"standalone"` (web demo: title screen, save slots, autosave each morning). `reducedMotion`/`uiScale` override the stored settings. `storage`: see below. |
| `frame(request, reuse?)` | `fe_player_frame` | `request` is `{dt, events, width, height, render?, density?, touchControls?, insetBottom?}`: `density` is frame pixels per CSS pixel (the interface is laid out in CSS pixels, so pass `devicePixelRatio` times any downscale), `touchControls` says the page shows on-screen controls (prompts then name no keys and the key hint row is left out), `insetBottom` is how many frame pixels at the bottom they cover (the HUD's bottom row, panels and the dialogue box stay above). Returns `{width, height, pixels, info, storageChanged}`: `pixels` is a `width × height × 4` `Uint8ClampedArray` for `ImageData` (frames are opaque, so their premultiplied RGBA is also straight RGBA), `null` with `render: false`; `info` is `{sounds: [{cue, gain, asset?}], requests, screen, modal, music}` ([Sound](#sound)). Pass the previous `pixels` as `reuse` to draw into it instead of allocating. |
| `debug(action)` | `fe_player_debug` | Debug-drawer actions: `{type: "addMoney", amount}`, `fullEnergy`, `addMinutes`, `setSeason`, `giveFirst`, `teleport`, `setFlag`, `skipDay`. |
| `commands(commands)` | `fe_player_commands` | Engine commands, as if the player did them. |
| `stateJson()`, `hash()` | `fe_player_state_json`, `fe_player_hash` | The live state (stable JSON) and its hash. |
| `syncedProject()` | `fe_player_synced_project` | "Keep changes": the project with the live state written back (throws for a cartridge). |
| `queryJson(query)` | `fe_player_query_json` | `{type: "summary"}`, `{type: "widgetRect", path}`, `{type: "toasts"}`, `{type: "pluginErrors"}`. |
| `gameInfo()` | | `{gameId, title, version, author, company, credits}`. |
| `audioClip(id)` | | The game's own sound or music asset, decoded: `{sampleRate, channels, samples}` ([Sound](#sound)); `undefined` when there is none. |
| `exportStorage()`, `importStorage(doc)` | | Saves and settings, below. |
| `free()` | `fe_player_free` | Releases the player (or rely on `FinalizationRegistry`, or `using`). |

**Input events** are `{type: "keyDown", key, repeat?}`, `keyUp`, `text`, `pointerMove`,
`pointerDown`/`pointerUp` (`{x, y, button?}` in frame pixels), `pointerLeft`, `wheel`
(`{dx?, dy}` in lines), `gamepadButton` (`{button, pressed}`, Xbox names such as `"south"`,
`"dpadUp"`), `gamepadAxis` (`{axis: "leftX", value}`) and `focusLost`. Keys are
`KeyboardEvent.key.toLowerCase()` (`"w"`, `"arrowup"`, `" "`, `"enter"`, `"escape"`).

**Requests** in `info.requests`: `"quit"` (never in the browser: web players have no Quit
button), `"fullscreen:on"`/`"fullscreen:off"` (standalone; call `requestFullscreen()`) and
`"title:<text>"` (set `document.title`).

### Saves and settings in the browser

There is no file system, so a player keeps its three save slots (binary `FGSV` saves, the
format of the desktop game) and its settings (TOML) in memory. The page persists them:

```ts
const key = `farm:${gameId}`;                        // saves belong to one game
const player = new Player(cart, { mode: "standalone", storage: localStorage.getItem(key) ?? undefined });
// …after a frame with storageChanged:
localStorage.setItem(key, player.exportStorage());
```

The document is `{"version": 1, "settings": "<toml>" | null, "slots": {"1": "<base64>", …}}`.
`storageChanged` turns true on the frame after a save, an autosave, a deleted slot or a settings
change, and `exportStorage()` acknowledges it. `importStorage(doc)` replaces the slots and
settings of a running player (the title screen's slot list updates at once); a malformed
document, or one whose settings TOML does not parse, throws and changes nothing (import it again
with `settings: null` to keep its saves). IndexedDB works the same way (store the string). Saves
of another game are refused on load, as on the desktop.

`localStorage` writes can fail (the origin's quota is shared, on itch.io by every game on the
same host): catch them and tell the player, because the game's own "Saved" toast cannot know.
The web demo page (`tools/wasm/web-template/game.js`) shows a notice until a later write works,
and keeps a stored document it cannot read under `<key>:unreadable` before anything overwrites
it.

### Sound

`info.sounds` lists the cues to start this frame with their gain (the master and effects
volumes already applied). Every cue is a synthesized preset, so the page renders it once per
sample rate and plays it with WebAudio:

```ts
const audio = new AudioContext();
const buffers = new Map<string, AudioBuffer>();
function play(cue: string, gain: number) {
  let buffer = buffers.get(cue);
  if (!buffer) {
    const samples = sfxSamples(cue, audio.sampleRate);
    if (!samples) return;
    buffer = audio.createBuffer(1, samples.length, audio.sampleRate);
    buffer.copyToChannel(samples, 0);
    buffers.set(cue, buffer);
  }
  const source = new AudioBufferSourceNode(audio, { buffer });
  const volume = new GainNode(audio, { gain });
  source.connect(volume).connect(audio.destination);
  source.start();
}
```

`sfxCues()` lists the cue names (to warm the cache).

A game may bring its own sounds and music (Ogg Vorbis or WAV assets). A sound with
`asset: true` is one: its samples are `player.audioClip(cue)`, at the file's own rate and
interleaved when stereo. Short sounds were decoded when the game loaded; music and sounds
longer than five seconds are decoded on each call (the engine keeps no decoded copy of them,
the way the desktop player streams them). Make the buffer at that rate (WebAudio resamples
it) and cache it per id:

```ts
const clip = player.audioClip(cue);
const frames = clip.samples.length / clip.channels;
const buffer = audio.createBuffer(clip.channels, frames, clip.sampleRate);
for (let channel = 0; channel < clip.channels; channel++) {
  buffer.copyToChannel(clip.samples.filter((_, i) => i % clip.channels === channel), channel);
}
```

`info.music` names the loops to play: `music` and `ambience` (built-in,
`musicSamples(name, sampleRate)`) with their gains. When `asset` is set, loop
`player.audioClip(asset)` at `musicGain` instead of `music`; `music` is the built-in fallback
for a page that cannot play it.

### Plugins

Pack plugins run inside the module, exactly as on the desktop: `farm-plugins` runs the QuickJS
guest in `wasmi`, a WebAssembly interpreter written in Rust, with the same fuel budgets, so
they are deterministic and synchronous with the frame. Their errors are in
`queryJson({type: "pluginErrors"})`.

## Functions

- `hashState(state)`: the state hash (16 hex digits) of a `GameState` given as JSON text or an
  object, so `hashState(player.stateJson()) === player.hash()`. Since v9 the
  state hash is xxh3-64 over the state's canonical binary encoding (docs/dev/NUMERICS.md), not a hash
  of its JSON text.
- `sfxCues()`, `sfxSamples(cue, sampleRate)`, `musicSamples(name, sampleRate)` (the built-in
  music and ambience loops a frame's `info.music` names; a game's own audio is
  `Player.audioClip`), `version()`, `lastPanic()`.
  `start()` installs the panic hook; `init` calls it.

## Errors and panics

Failures throw a JavaScript `Error` named `FarmError` with a `kind`:

- `"invalid"`: bad input or a refused request (a frame without a size, malformed JSON, a save
  of another game, a frame over 16 Mpx). The object
  stays usable.
- `"poisoned"`: the engine failed during a frame ("The game stopped: …"), or an earlier call
  did. The object refuses every later call; drop it (the editor ends the playtest).
- `"panic"`: reserved for hosts that can catch panics (never thrown on the web, see below).

`wasm32-unknown-unknown` aborts on panic: Rust cannot unwind or catch it, unlike farm-ffi's
`catch_unwind`. A panic traps, and the call throws a `WebAssembly.RuntimeError`
("unreachable"). Before trapping, the module's panic hook (installed when the module
initializes; a few lines, no extra dependency) logs the message with `console.error` and
records it. From then on **every** call on **any** object throws a `FarmError` of kind
`"poisoned"` carrying that message (`lastPanic()` returns it), because the module's memory may be
half-updated; calls on the object that panicked may instead throw wasm-bindgen's "recursive use
of an object" error. Load a fresh module instance to continue.

Running out of memory aborts the same way but without running the panic hook. Each engine call
marks itself as running and clears the mark when it returns, so the next call after such a trap
finds the mark and throws `"poisoned"` too (`lastPanic()` stays `undefined`). Treat any
`WebAssembly.RuntimeError` as the end of the module instance. Frames are capped at 16 Mpx on the web (64 Mpx natively) to keep allocations well inside a module's memory.
