# Architecture

How Tractr is put together, and where new code goes. Read this before you add
a part of the editor or the engine. [CONVENTIONS.md](CONVENTIONS.md) has the
code conventions, [NUMERICS.md](NUMERICS.md) the integer numerics, and
[EDITOR-ARCHITECTURE.md](EDITOR-ARCHITECTURE.md) the C# app's views.

![How Tractr is built: the C# desktop editor, the F# authoring core and the Rust engine and player, with the cartridge between F# and Rust and the C ABI between C# and Rust.](../media/dev/architecture.png)

Designs marked **Future** are collected in [Future](#future-not-built); the
rest describes what is built.

## The rule in one line

**Rust runs the game. F# understands the project. C# is the desktop app.**

| Language | Owns | Never owns |
|---|---|---|
| **Rust** | Everything that runs while a game is being played: simulation, saves, state hashing, RNG, fixed timestep, input bindings, minigame kinds, in-game UI, rendering (draw lists and the GPU backend), audio playback, the plugin sandbox, the standalone player, the WebAssembly build. | Project JSON, editor state, anything a creator edits directly. |
| **F#** | Everything about authored data: the project schema, loading, validation and the Problems panel, content packs (merge and namespacing), default content and templates, workshop patterns, edit operations and undo/redo, the condition and outcome vocabulary and the dialogue checks, and the compiler that turns a project into a cartridge. Plus the `farmc` CLI. | Gameplay rules, rendering, OS or UI code. |
| **C#** | The Avalonia app: windows, views and view models, dialogs and file pickers, the project store on disk, image import, the Update Center, and hosting the Rust player and preview. | Game rules, project rules. A view model calls F# or Rust; it doesn't decide anything itself. |

The contract between F# and Rust is the **cartridge**: a compiled, validated,
binary form of a project. F# writes it and Rust reads it. JSON is the project
file and the debug format; it is not what the game runs on.

### If you're unsure where something goes

1. **Does it change what happens in a playthrough?** (a rule, a number, a random
   roll, a timer) → **Rust**, in `farm-sim`.
2. **Does it read or change the project a creator is editing?** (a field, a
   default, a check, an import) → **F#**, in `FarmEngine.Authoring`.
3. **Is it pixels on screen during play, or sound?** → **Rust**
   (`farm-render`, `farm-ui`, `farm-player`).
4. **Is it an editor window, form, file dialog, menu or OS integration?** →
   **C#**.
5. **Is it a check that authored data is valid?** → F#. **Is it a check that a
   command is allowed right now in the game?** → Rust.

## Why this split

- **The overnight pass dominates.** Sleeping advances every tile, crop,
  animal, machine and schedule at once. Rust with flat per-layer arrays turns
  that into tight loops over contiguous memory.
- **Long saves need exact repeatability.** Hundreds of in-game days compound
  any numeric drift. Rust gives integer and fixed-point math with lints that
  enforce it, and compiles to native code and to WebAssembly from the same
  source, so the desktop player and the web demo run one engine.
- **The editor is mostly data transformation.** Loading, validation,
  pack merging and compiling content are compiler work. F# is built for that:
  discriminated unions, exhaustive pattern matching, and immutable
  collections with structural sharing (cheap undo).
- **The desktop app is C#.** Avalonia, Velopack, the project store and the
  UI tests; F# and C# share one .NET solution at no cost.

## The big picture

```text
                 ┌─────────────────────────── .NET (one solution) ───────────────────────────┐
 project folder ▶│ FarmEngine.Authoring (F#)          Tractr.App (C#, Avalonia)               │
                 │  schema · loading · validation      views · view models · dialogs          │
                 │  packs · edits + undo · Problems ◀── calls F# for every project change     │
                 │  compiler ─▶ cartridge bytes        hosts the Rust player and previews     │
                 │  Export Game, farmc                 draws tool overlays on Rust frames     │
                 │        │                         FarmEngine.Interop (C#): P/Invoke         │
                 └────────┼─────────────────────────────────────▲─────────────────────────────┘
                          │ game.cart               C ABI (farm-ffi): commands in, frames out
                 ┌────────▼─────────────────────────────────────┴─────────────────────────────┐
                 │ Rust workspace                                                              │
                 │  farm-cart (cartridge and save formats) ─▶ farm-sim (deterministic core)    │
                 │  farm-runtime (timestep, input, minigames, panels, audio)                   │
                 │  farm-plugins (QuickJS in WebAssembly, run by wasmi)                        │
                 │  farm-render (draw lists, CPU rasterizer) · farm-gpu (wgpu) · farm-ui       │
                 │  farm-player (the game; also embedded in Play Mode) · farm-host (protocol)  │
                 │  farm-ffi (C ABI for .NET) · farm-wasm (the web player)                     │
                 └─────────────────────────────────────────────────────────────────────────────┘
```

One player, three places: the editor's **Play Mode**, **exported Windows and
Linux games**, and the optional **web demo export** all run `farm-player`.
What a creator tests is exactly what ships. [Export internals](EXPORT-INTERNALS.md)
covers what an exported game contains.

## Contracts between languages

### Cartridge (F# → Rust)

- **Format:** [FlatBuffers](https://flatbuffers.dev), schemas in `schemas/cart.fbs`.
  It reads without copying (fast loads), generates code for Rust, C# (usable from
  F#) and TypeScript, and has clear rules for evolving a schema: fields are
  only added, never renumbered.
- **Game info:** a `GameInfo` table (title, version, `gameId`, author,
  window defaults, pixel scale) from the export settings, so the standalone
  player never reads project JSON. See [Export internals](EXPORT-INTERNALS.md).
- **Contents:** the compiled content, the new-game start state and
  the presentation data as JSON sections with string ids and numbers in
  authoring units, an asset table with every embedded file once, and the pack
  plugins with their granted hooks and minigame kinds (see "Format 3" below).
- **Version header:** `cart_format` (bumped on breaking change) plus the
  project schema version it was compiled from.
- **Format 3:** `cart.fbs` carries game info, the compiled content,
  the new-game start state (`StartState`) and presentation data
  (`Presentation`) as JSON in authoring units, which the engine converts to
  its fixed-unit integers as it reads them ([NUMERICS.md](NUMERICS.md)), plus
  an asset table that holds every embedded file once (JSON refers to it as
  `asset:<id>`). The player never reads project JSON. F# resolves the
  built-in catalog, pack namespacing, load order, overrides and locale
  strings. Content keeps its string ids: interning them into indexed tables
  would only help if the simulation used interned indices, which it doesn't.
  See [schemas/README.md](../../schemas/README.md).

### Saves (Rust only)

- The player's machine persists saves, so **Rust owns the save format and
  save loading**. **F# owns project loading**: the creator persists projects.
  Both read only their current versions (save v5; project schema versions 2
  and 3); others are refused.
- The format is FlatBuffers too (`schemas/save.fbs`): a version header, a
  slot preview the title screen reads without loading the game, and the state
  compressed with zstd (pure-Rust `ruzstd`, so it also builds for wasm). JSON
  saves with the same header remain readable.
- **Saves outlive the cartridge that wrote them.** A player's save from
  version 1.0 of a game has to load in 1.1. So saves refer to content by its
  stable string id, never by the cartridge's interned indices, and the header
  carries `gameId`, the game version and the cartridge hash. Ids that no
  longer exist go to quarantine instead of failing the load. See
  [Export internals](EXPORT-INTERNALS.md#compatibility-rules). Rust can also export a save as stable JSON for
  debugging.
- Saving is synchronous: the player serializes, compresses and writes the
  save on the simulation thread. Stable JSON is written straight from
  `Serialize` (`farm_sim::stable_json`), putting only the objects whose keys
  arrive out of order into key order; a `serde_json::Value` tree writer stays
  as the reference the tests compare it with. Serializing the mid-size
  benchmark state takes about 2.5 ms against a 1 ms budget, so CI holds it to a
  regression ceiling (`SAVE_CEILING` in `crates/farm-bench`). Unsorted
  `serde_json` output of the same state takes about 1 ms, so the budget needs
  more than a faster writer, and copying the state to serialize it elsewhere
  costs about as much as writing it. **Future:** compress and write on a
  worker thread.

### FFI: .NET → Rust (`farm-ffi`)

- A C ABI `cdylib`. The C# bindings in `FarmEngine.Interop` are hand-written
  `LibraryImport` declarations (`NativeMethods.cs`) wrapped in small owning
  classes (`RustPlayer`, `RustPreview`, `RustSession`). F# uses the same
  wrappers.
- **Coarse, handle-based, batched.** Never one call per tile or entity. Three
  handles:

  ```c
  // The editor's Play Mode: the whole game player (crates/farm-ffi/src/player.rs).
  fe_result fe_player_new(const uint8_t* game, size_t len, const uint8_t* options, size_t options_len,
                          fe_player** out, fe_bytes* error);         // project JSON or cartridge
  fe_result fe_player_frame(fe_player*, const uint8_t* request, size_t len, fe_bytes* out);
                          // {dt, events, width, height} → size, info JSON, RGBA pixels
  fe_result fe_player_frame_info(fe_player*, const uint8_t* request, size_t len, fe_bytes* out);
                          // the same without the pixels, which stay in the player until …
  fe_result fe_player_copy_pixels(fe_player*, uint8_t* dst, size_t len, size_t stride, fe_bytes* out);
                          // … the host copies them once, straight into its bitmap
  fe_result fe_player_debug(fe_player*, const uint8_t* action, size_t len, fe_bytes* out);
  fe_result fe_player_synced_project(fe_player*, fe_bytes* out);    // "keep changes"
  fe_result fe_player_query_json(fe_player*, const uint8_t* query, size_t len, fe_bytes* out);

  // Edit Mode's map and art previews (render.rs).
  fe_result fe_preview_new(const uint8_t* project, size_t len, fe_preview** out, fe_bytes* error);
  fe_result fe_preview_set_scenes(fe_preview*, const uint8_t* scenes, size_t len, fe_bytes* out);
                          // a paint stroke: the changed scenes only, not the project and its art
  fe_result fe_preview_render(fe_preview*, const uint8_t* request, size_t len, fe_bytes* out);

  // A headless game for tools and tests (session.rs).
  fe_result fe_session_new(const uint8_t* game, size_t len, const uint8_t* seed, size_t seed_len,
                           bool auto_start_quests, fe_session** out, fe_bytes* error);
  fe_result fe_session_apply(fe_session*, const uint8_t* commands, size_t len, fe_bytes* out);
  fe_result fe_session_tick(fe_session*, uint32_t ticks, fe_bytes* out);
  fe_result fe_session_save(fe_session*, fe_bytes* out);

  // Sound and music assets in the Art tab (audio.rs): no handle, one preview at a time.
  uint8_t   fe_audio_output(void);                                   // 1 with `audio-out`
  fe_result fe_audio_probe(const uint8_t* file, size_t len, fe_bytes* out);       // the player's decoder
  fe_result fe_audio_preview_play(const uint8_t* file, size_t len, fe_bytes* out); // plays it once
  void      fe_audio_preview_stop(void);
  uint8_t   fe_audio_preview_playing(void);

  void      fe_bytes_free(fe_bytes);
  uint32_t  fe_abi_version(void);
  ```

  Requests and answers are JSON (stable JSON for state); frames are raw
  premultiplied RGBA. Each handle has its `_free` and its `_last_error`.
- **Errors:** every call on a handle answers a result code, and on failure
  `out` holds the UTF-8 message (for `Poisoned`, the error that poisoned the
  handle); `fe_<handle>_last_error` returns it again.
- **Loading:** `FarmEngine.Interop` loads the library from the application's
  folder only (a `DllImportResolver`, and `DefaultDllImportSearchPaths` set to
  the assembly directory), then compares `fe_abi_version()` with the
  `FE_ABI_VERSION` it was written for and refuses another; bump both with any
  signature, layout or convention change. `FarmFfi.LoadError` says why the
  library didn't load. On Linux the default `audio-out` feature links ALSA
  (`libasound.so.2`) when the library loads, because cpal has no backend that
  opens it at run time. Linux editor builds also ship `libfarm_ffi_silent.so`:
  the resolver loads this application-local fallback if the sound-enabled engine
  cannot load. Editing and Play Mode still work without ALSA; sound is disabled.
  `-p:FarmFfiAudio=false` builds only the silent engine.
- **Play Mode.** The editor forwards raw input events (keys by the engine's
  names, pointer positions in frame pixels) and draws the returned pixels; the
  player owns the game, its UI and its sounds (`audio` option). Details in
  [Player internals](PLAYER-INTERNALS.md#in-the-editor).
- **Render requests.** `fe_render_json` returns Rust-decorated Edit Mode
  snapshots and PNG rasters of any snapshot. The `fe_preview_*` handle keeps a
  project and its image caches, and rasterizes one scene viewport (or one
  visual binding, for the art studio) per call.
- **Memory:** Rust allocates result buffers; .NET copies what it needs and
  frees them through `fe_bytes_free`. No pointer into Rust memory outlives the
  next call on that handle. The C# handles are `SafeHandle`s: a call keeps its
  handle alive, a disposed one throws `ObjectDisposedException` before reaching
  Rust, and a forgotten one is freed by its finalizer.
- **Threading:** a handle is used by one thread at a time; the C# wrappers
  serialize their calls with a lock. The editor runs Play Mode frames on a
  worker thread and copies the pixels into the bitmap on the UI thread.
- **Panics** are caught at the boundary (`catch_unwind`) and returned as
  error results, never unwound into .NET. The handle is then poisoned.
  Freeing a handle catches panics too, and leaks a poisoned one rather than
  run its drop code. Every `unsafe` operation in `farm-ffi` is in its own
  block with a `// SAFETY:` comment (`unsafe_op_in_unsafe_fn` and
  `clippy::undocumented_unsafe_blocks` are denied).
- **Plugin hooks.** Plugin mutations are queued and come back as
  commands (`PluginMutationQueue`), drained right before each tick
  (`PlaySession` advances one tick at a time, so they do not depend on the
  frame rate). The engine lets `onWeatherRoll` listeners return an override
  *during* the nightly step, but plugins never do. So `farm-plugins`' `PluginRuntime` dispatches
  `onWeatherRoll` after the step like any other hook, and installs no
  `WeatherRollListener`; the Problems panel warns about plugins that listen
  to it ([PLUGINS.md](../PLUGINS.md)).

### WebAssembly (`farm-wasm`)

- `wasm-bindgen` API over `farm-ffi`'s player: `Player`, `hashState` and the
  sound and music samples, over the same host protocol (`farm-host`, shared
  with `farm-ffi`). The web demo export target uses it; headless sessions and
  Edit Mode previews are `farm-ffi` only. Plugins run inside the module (QuickJS in wasmi, like the
  desktop), on the calling thread; a page that wants them off the main thread
  runs the whole player in a Worker. See `crates/farm-wasm/README.md`.

## Two project pipelines

Both languages can still turn a project into what a game runs on, and both are in use:

- **F#** (`ContentCompiler`, `PackMerge`, `Builtin`, `CartridgeCompiler`) writes every
  cartridge: Export Game, `farmc` and the editor's Play Mode.
- **Rust** (`farm_sim::packs`, `content_builtin`, `state::create_content_from_project`,
  `StartState::from_project`, `Presentation::from_project`, `apply_state_to_project_json`)
  serves the hosts that receive project JSON instead of a cartridge: `fe_session_new` and
  `fe_preview_*` (the editor's map preview and headless sessions), `Player::from_project`, and
  farm-wasm when it is given a project.

So the rule "Rust never owns project JSON" holds for cartridges and saves, not for those
hosts. Until they receive cartridges (or F#-compiled sections) and the Rust `from_project`
paths can go, the two pipelines are kept identical by tests: `ParityTests.fs` compiles the
Rust-recorded `fixtures/golden/content` projects with F#, and records `fixtures/parity` (a
kitchen-sink project and content with every F# record field set, plus the F# pipeline's
output for the templates, the fixture projects and the example packs), which
`crates/farm-sim/tests/fsharp_parity.rs` checks against serde and the Rust pipeline. The
schema itself has one source per language: on the F# side `Schema.fs`, from which
`SchemaJson.fs` is generated (`tools/codegen/schema-json.fsx`); on the Rust side the serde
types, kept by hand. Generating the serde types (or checking their defaults) from `Schema.fs`
is future work; these tests catch a field added on one side only.

## Determinism rules

The same seed and the same inputs must give the same game on every machine,
in every build and in the browser. These rules are enforced in `farm-sim`
with `clippy.toml` and crate attributes, and CI runs clippy with
`-D warnings`:

- `#![forbid(unsafe_code)]` in `farm-sim`, `farm-cart`, `farm-runtime`,
  `farm-plugins`.
- Plugins are deterministic too. Their budgets are fuel (counted wasm
  instructions), not wall-clock time; the guest's clock is constant and
  `Math.random` is seeded the same way on every run; payloads are serialized
  in engine order.
- `disallowed-types`: `std::collections::HashMap` and `HashSet` (use `Vec`,
  `IndexMap`, `BTreeMap`), `std::time::Instant` and `SystemTime` (use the game
  clock). The simulation has no source of randomness but its seeded `Rng`.
- `disallowed-methods`: `slice::sort_unstable*` (equal elements must keep
  their order).
- **Integers only:** `#![deny(clippy::float_arithmetic)]` in `farm-sim`.
  Every quantity has an explicit integer type and scale; the full table is in
  [NUMERICS.md](NUMERICS.md). The main ones:

  | Quantity | Type | Unit |
  |---|---|---|
  | Money | `i64` | whole gold |
  | Energy | `i32` | 1/1000 point |
  | Time of day | `u32` | 1/1000000 minute |
  | Clock | `u64` | ticks (20 per second) |
  | Player, NPC, animal and monster position | `i32` | 1/8192 tile |
  | Friendship | `i32` | whole points |
  | Probabilities | `u64` | threshold out of 2³² (compared with `Rng::next_u32`) |

  The state hash is xxh3-64 over the canonical binary state encoding.
  Rendering may use floats. It reads state and never writes it.

## Repository layout

```text
Cargo.toml                       # Rust workspace; rust-toolchain.toml pins the version
schemas/cart.fbs, save.fbs
crates/
  farm-cart/       # FlatBuffers readers, cartridge + save load
  farm-cart-schema/ # generated FlatBuffers accessors (flatc output, checked in)
  farm-sim/        # deterministic core
  farm-runtime/    # timestep, input, minigames, panel model, audio cues
  farm-plugins/    # PluginHost trait; QuickJS-in-wasm host run by wasmi
  farm-render/     # draw-list builder; CPU rasterizer (tiny-skia)
  farm-ui/         # in-game UI (HUD, dialogue, shop, inventory, crafting, quests, panels)
  farm-gpu/        # wgpu backend for draw lists (the desktop player's GPU renderer)
  farm-player/     # standalone player (winit + softbuffer or wgpu + cpal + gilrs) and game shell; embeddable
  farm-host/       # host protocol shared by farm-ffi and farm-wasm
  farm-ffi/        # C ABI for .NET
  farm-wasm/       # wasm-bindgen API for the web
  farm-bench/      # criterion benchmarks and the CI time budgets
  farm-fuzz/       # fuzz targets and state invariants (run on stable; fuzz/ wraps them for cargo-fuzz)
  farm-plugin-guest/ # the QuickJS guest compiled to the checked-in plugin sandbox module (MIT)
fixtures/golden/                 # goldens shared by Rust and F# tests
src/
  FarmEngine.Authoring/          # F# core: schema records, edits, checks, compiler (see below)
  FarmEngine.Authoring.Net/      # F#, .NET-only: System.Text.Json edge, RecordJson, RecordWith (C# builders)
  FarmEngine.Export/             # F#, .NET-only: Export Game (templates, PE resources, archives)
  FarmEngine.Cli/                # F#: the farmc CLI
  FarmEngine.Interop/            # C#: hand-written LibraryImport bindings + owning wrappers; builds farm-ffi
  Tractr.Updates/                # C#: the Update Center backend (Velopack)
  Tractr.App/                    # C#: the Avalonia desktop editor
tests/
  FarmEngine.Authoring.Tests/    # F# (xUnit + FsCheck)
  FarmEngine.Export.Tests/       # F#
  Tractr.App.Tests/              # C#: headless Avalonia tests
tools/wasm/                      # farm-wasm build + Node smoke test
tools/codegen/                   # generators for SchemaJson.fs, RecordWith.fs and RecordJson.fs tables
```

`FarmEngine.Interop` has an MSBuild target that runs `cargo build` and copies
`farm_ffi.dll` / `.so` / `.dylib` into the output, so `dotnet build`,
`dotnet test` and `dotnet run` keep working as the only commands a contributor
needs (with a Rust toolchain installed). `-p:CargoProfile=dev` builds the
debug library instead of the release one. Tests that need the Rust library
fail when it is missing; set `FARM_ALLOW_MISSING_NATIVE=1` to build and test
without a Rust toolchain, which reports them as skipped.

### F# conventions

- **A pure core.** `FarmEngine.Authoring` has no reflection, no
  `System.Text.Json`, no file I/O and no .NET-only APIs (such as
  `SortedDictionary` or `System.Security.Cryptography`). JSON goes through the
  explicit decoders and encoders in `SchemaJson.fs` (generated from
  `Schema.fs`) over the immutable `Json` type, and .NET-only code lives in
  `FarmEngine.Authoring.Net`. The split keeps the core easy to test.
- **Schema records.** `Schema.fs` holds every project, content and save shape
  as an immutable record: `option` for optional fields, `list` for lists,
  `(string * 'T) list` for ordered maps, `Json` for free-form values, an `Extra`
  field for undeclared keys on the records that keep them, and a union for
  conditions. Every record has a `Default`, which is also what a missing key
  decodes to. `SchemaJson.fs` is generated from the records
  (`tools/codegen/schema-json.fsx`): the JSON key is the field name in
  camelCase, and three attributes say what the types do not (`[<JsonKey>]` for
  another key, `[<JsonNull>]` for a `.nullable()` field written as `null`,
  `[<JsonDecode>]` for a decoder the type does not give;
  [tools/codegen](../../tools/codegen/README.md#schemajsonfs)). So a schema change
  is: edit `Schema.fs`, run `schema-json.fsx`, then regenerate the C# helpers
  (`tools/codegen/record-json.py`, `tools/codegen/record-with.fsx`).
- The compiler writes cartridges with a plain-F# FlatBuffers builder
  (`FlatBuffers.fs`) that produces the same bytes as the official C# builder,
  and `CartridgeReader` reads them back.
- **Numbers** are `float` in authoring units, as in the project JSON
  ([NUMERICS.md](NUMERICS.md)). **Future:** units of measure on authored
  quantities (`price: float<gold>`, `x: int<tile>`); they would be erased at
  compile time, so C# would still see plain numbers.
- **Every project change is an `Edit`** (a discriminated union) applied by
  `Document.apply`. Undo/redo is a stack of immutable documents (structural
  sharing makes that cheap). Workshop patterns are functions that return a
  list of edits, applied as one undo step.

  ```fsharp
  type Edit =   // Edits.fs (excerpt)
    | PaintTiles of sceneId: string * layer: TileLayer * cells: (int * int) list * tileType: string
    | FloodFill of sceneId: string * layer: TileLayer * x: int * y: int * tileType: string
    | UpsertCrop of crop: CustomCropDefinition
    | UpsertNpc of npc: Npc
    | Batch of label: string * edits: Edit list   // one undo step
  ```

- **C# friendliness at the boundary.** Types C# view models consume are
  records with `[<CLIMutable>]` only where binding needs it. Options are
  exposed as nullable through small helper modules; don't make C# match on
  F# unions.

### Rust conventions

- Modules are snake_case, one per engine system, in the folders of
  [CONVENTIONS.md](CONVENTIONS.md); a change to play comes with a golden or
  replay that pins it.
- `farm-sim` has no I/O, no threads and no logging side effects. It is a
  reducer: `(state, content, command | ticks) → (state, effects, hook events)`.
- State is **mutable inside a step, immutable across steps.** Rust updates
  state in place. Undo and replay use the command log.
  Snapshots for the debug drawer and playtest "keep changes" are explicit
  copies.
- **Every sentence the engine shows a player comes from the message catalog**
  (`farm_sim::messages`): a stable key (`msg.notEnoughMoney`) and an English
  template with positional arguments. `Effect::say(level, message)` records
  the English text in the effect (what replays and goldens hold) and carries
  the message beside it, outside serialization and equality. The player translates it with the
  `farm-ui` table of the player's language (`Lang::message`), falling back to
  English for a key the table lacks. Built-in minigames' default texts and the
  save-load notices (`LoadedSave::error_messages`) use the same catalog.
  `Effect::message` is only for text a creator wrote (event and plugin
  messages); a test fails when engine code passes it a literal or a `format!`.

## Tests

The test suites, by kind (CONTRIBUTING.md has the commands):

- **Goldens:** Rust integration tests in `farm-sim` and `farm-cart` replay
  `fixtures/golden` (replays, content, saves, RNG, hash; [NUMERICS.md](NUMERICS.md#goldens)),
  F# tests check the content and cartridge goldens, and `Tractr.App.Tests`
  drives the editor headless and compares editor screenshots.
- **Property tests:** proptest (Rust) for "replay ⇒ same hash" and "save →
  load ⇒ same state". FsCheck (F#) for "a varied project loads and passes
  the checks" and "compile never throws on a project with no validation
  errors".
- **Fuzzing:** `crates/farm-fuzz` has one entry point per untrusted-input
  boundary (cartridges, saves, save states, images, project JSON,
  render, preview and session requests, plugin mutations, random play) and
  the state invariants every reachable state keeps (money ≥ 0, energy in
  range, stacks and slots within their caps, the player on a scene's grid,
  grids of their scene's size). Its tests run every target on stable over
  its seed corpus, random bytes and edited seeds, and the invariants under
  generated content; `fuzz/` wraps the same targets for cargo-fuzz (nightly,
  the optional weekly `Fuzz` workflow). On the F# side, FsCheck properties
  (`FuzzPropertyTests.fs`) feed the JSON parser near-JSON and arbitrary text
  and the cartridge reader edited and arbitrary bytes. A proptest failure
  prints its shrunk input (and is kept in a `.proptest-regressions` file that
  reruns it first); `PROPTEST_RNG_SEED` replays a run and `PROPTEST_CASES`
  lengthens it. FsCheck prints a `Replay` seed.
- **Benchmarks** (`farm-bench`, criterion), with budgets checked in CI
  (medians, with a 3× margin for shared runners). Targets for a mid-range
  laptop:

  | Scenario | Target |
  |---|---|
  | Overnight pass, 64×64 farm, full crops | < 2 ms |
  | Overnight pass, 256×256 farm, full crops + 50 machines | < 30 ms |
  | Overnight pass, 512×512 farm (the largest scene), full crops + 100 machines | < 120 ms |
  | Save (serialize on sim thread) | < 1 ms |
  | Cartridge load, sample games | < 50 ms |
  | Frame (draw list + wgpu), 1080p, integrated GPU (not measured: CI has no GPU; the CPU rasterizer is held to one 60 Hz frame) | < 4 ms |
  | CPU frame, 1080p, on a 256×256 map, a 512×512 map, and at night under 64 lights (the light map) | < 16.7 ms |

  On a 4-core cloud machine the 512×512 overnight pass took about 72 ms, a 1080p CPU frame about 4 ms on the
  starter farm's own map, 6 ms on a 256×256 map, 6.5 ms on a 512×512 map
  and 8.5 ms at night under 64 lights: a frame costs what the camera shows,
  not the size of the scene.

## Checklist: adding an editor feature

Use this for every editor feature:

1. **Split the feature.** List what it *decides* (defaults, checks, derived
   values, what a button changes) and what it *shows*. Decisions go to F#,
   display goes to C#.
2. **Anything that changes play goes to Rust.** If the feature reaches into
   engine rules, the rule belongs in `farm-sim`, not in the editor.
3. **Add `Edit` cases** in F#. Every change to the project goes through
   `Document.apply`, so undo/redo and autosave come for free. Multi-step
   actions use `Edit.Batch`.
4. **Add validation** to the problems pipeline with a JSON path, so the
   Problems panel can jump to it.
5. **Keep view models thin.** A C# view model binds fields, calls F# and
   shows results. If it contains an `if` about game or project rules, move
   that `if` to F#.
6. **Previews come from Rust.** If the editor needs to show what will
   happen in play (sprite, schedule, growth), ask a Rust preview session
   instead of re-implementing the rule.
7. **Tests:** F# tests for edits and validation, a headless Avalonia test for
   the view, Rust tests plus a golden or replay if simulation changed.
8. **Docs and changelog:** update the [Creator guide](../CREATOR-GUIDE.md)
   (it is also the in-app help, so bold text must be labels the editor
   shows), add the change to the [CHANGELOG](../../CHANGELOG.md), and tick
   the item in [ROADMAP.md](../../ROADMAP.md) if it is there.

## Future (not built)

Designs this plan describes that do not exist yet. Each would need its own
issue before work starts.

- **Interned cartridge content.** Dense integer indices for ids, a string
  table per locale, compiled condition bytecode and dialogue graphs, flat
  tile-layer arrays, texture atlases packed at load. Today the sections are
  JSON with string ids.
- **A condition language and a dialogue language** in F# (parse, type-check,
  compile). Today conditions and dialogue are schema records checked by the
  Problems pipeline.
- **Units of measure** on the F# records (`<gold>`, `<day>`, `<tile>`).
- **Asynchronous saving** within a 1 ms budget on the simulation thread.
- **GPU effects** on the wgpu renderer (`farm-gpu` draws the CPU renderer's
  frames today), and a measured 4 ms 1080p frame target on an integrated GPU.
- **kira audio**; the player mixes its own audio (built-in music, ambience and
  the game's own sounds and music, with crossfades) and outputs it through
  cpal.
- **Generated FFI bindings with `SafeHandle`s**; `FarmEngine.Interop` is
  hand-written `LibraryImport` declarations with owning wrappers.
- **The balancing lab** (`FarmEngine.Lab`).

## Open questions and settled decisions

- **Player-installed mods for exported games.** Pack merging is a compile
  step in F#. For players to add mods to a shipped game, either `farm-player`
  bundles the compiler (through a .NET sidecar), or packs are
  precompiled into overlay cartridges that Rust layers with the same
  namespacing rules.
- **Edit Mode rendering.** Settled: Edit Mode rasterizes the visible map
  region with the Rust CPU rasterizer into an Avalonia bitmap and draws its
  tool overlays on top in Avalonia; one rasterizer everywhere.
- **In-game UI toolkit.** Settled: `farm-ui` is a small custom
  immediate-mode layer on the draw list rather than egui, which looks like a
  tool, not a cozy game. Panels are rounded, shadowed shapes in the theme's
  colours, and layout is rectangle cutting, so it needs no layout engine.
  Nine-slice pixel-art panels can come later as a theme option.
- **Plugin sandbox without a JIT.** Settled: `farm-plugins` runs plugins in
  wasmi, a pure-Rust interpreter, so no platform needs a JIT (consoles and
  iOS forbid them). The wasm engine sits behind a small internal trait, so
  wasmtime (JIT or its Pulley interpreter) can be added as a build flag if
  plugins ever need more speed.

## History

Tractr began as a port of a TypeScript web version of the engine. The port
went through a C# engine before the simulation, runtime, renderer and player
moved to Rust and the project logic to F#; the C# engine projects and the
web version's migrations are gone, and the web version is a separate project.
[CHANGELOG.md](../../CHANGELOG.md) records the steps.
