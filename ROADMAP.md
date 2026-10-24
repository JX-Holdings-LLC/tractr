# Tractr roadmap

Tractr (called *Farming RPG Maker* until 0.4.0) began as a phased port of the
web version, `jxburros/farm-game-engine` (a private repository), which was the
reference implementation until both apps moved onto one engine.
The port is complete: phases 1–7 below are done, including phase 7, one
engine for the web and native versions (see [Phase 7](#phase-7-one-engine)).
Since 0.5.0 the two are separate projects, and Tractr keeps nothing for the
web version or for older files
([Separate projects](#separate-projects-050-060)). The port's checklists below
are a record of what was done; what they say about migrations, the Fable build
and matching the TypeScript engine was removed in 0.5.0 and 0.6.0.
Each phase shipped as a normal release through the Update Center.

What comes next is in two parts:

- [Toward full-size farm games](#toward-full-size-farm-games): what creators
  need to make games the size of Stardew Valley, Harvest Moon or Story of
  Seasons, in any setting, an alien planet included.
- [Later](#later): platform, polish and infrastructure work.

[Done since the port](#done-since-the-port) lists what 0.3.x and 0.4.0
added after the port was finished.

## Done — foundation (v0.1)

- [x] **Engine parity.** Schemas, migrations and the deterministic simulation
  core were ported (first to C#, since replaced by Rust and F#). The golden
  fixtures replay 23 recorded TypeScript sessions and must match every state
  hash and effect.
- [x] **Content and runtime.** Sample games, templates, fixed timestep, input
  bindings, minigames, creator game panels, the audio model, and the Jint
  plugin sandbox (first in C#, since replaced by `farm-runtime`,
  `farm-plugins` and F#).
- [x] **Desktop shell.** Avalonia app, Update Center (Velopack + GitHub
  Releases, Stable/Pre-release channels), and CI and release workflows.
- [x] **Play Mode.** Native Skia renderer, HUD, dialogue, shops, crafting,
  inventory, quests, minigames and playtest snapshots.
- [x] **Project management.** Projects stored as JSON files, JSON
  import/export, and templates.

## Done — editor port

Each part was ported following the checklist in
[docs/LANGUAGES.md](docs/LANGUAGES.md#checklist-adding-an-editor-feature):
project logic in F#, views in C#, anything that runs in play in Rust.

The web editor lives in `src/components/*` of the web repo (~13k lines of React).
Ported in rough order of how often creators use each part (all done; see
phase 5 below):

1. **Tile painter**: layers, brushes, rectangle/fill, copy/paste, universal
   undo/redo (`EditorPanel.tsx`, `GridTile.tsx`, `SceneManager.tsx`,
   `TransitionEditor.tsx`). The native map UI now covers these tools, collision,
   scene management and transitions; artwork bindings remain in the art phase.
2. **World content editors**:
   - NPCs and dialogue (`NPCEditor.tsx`)
   - items (`ItemEditor.tsx`)
   - crops (`CropEditor.tsx`)
   - quests (`QuestEditor.tsx`)
   - events (`EventsEditor.tsx`, `event-forms.tsx`)
   - shops (`ShopEditor.tsx`)
   - recipes (`RecipeEditor.tsx`)
   - node types (`NodeTypeEditor.tsx`)
   - wildlife (`WildlifeEditor.tsx`)
3. **Project settings and calendar** (`ProjectSettingsEditor.tsx`).
4. **Art pipeline**: asset import, the animation studio and art bindings
   (`AssetManager.tsx`, `ArtBindings.tsx`, `src/lib/import-art.ts`,
   `src/lib/validate-graphics.ts`).
5. **Creator workshop patterns and interface panels**
   (`CreatorWorkshop.tsx`, `InterfaceEditor.tsx`, `src/lib/creator-patterns.ts`).
6. **Mods and actions**: pack install/enable/order, plugin permission review
   (`ModsEditor.tsx`, `ActionsEditor.tsx`, `src/lib/mod-registry.ts`,
   `src/lib/validate-extensibility.ts`).
7. **Problems panel and debug drawer** (`ProblemsPanel.tsx`, `DebugDrawer.tsx`).

## Language migration (Rust, F#, C#)

Described in [docs/LANGUAGES.md](docs/LANGUAGES.md), which has the details
and exit criteria for each phase.

1. [x] Scaffolding: Cargo workspace, FFI, F# projects, Interop, CI and the
   FlatBuffers schemas (`schemas/cart.fbs`, `schemas/save.fbs`)
2. [x] Rust core in compatibility mode: all 23 replay, content, save, RNG
   and hash goldens pass in Rust, a differential test agrees with the C#
   engine, and `farm-runtime` has input, minigames, panels and the audio
   model.
3. [x] F# authoring: migrations, validation, packs, compiler, cartridges,
   undo. (The schema records moved from C# to F# in phase 7.)
4. [x] Switch the app to F# + Rust; the C# engine projects are deleted
5. [x] Editor port on the new stack (the list above)
6. [x] Rust player and plugin sandbox, embedded in Play Mode; Export Game for
   Windows and Linux, and (with `farm-wasm`) the web demo. The desktop player
   draws on the GPU too (`farm-gpu`, wgpu); GPU effects come later.
7. [x] One engine for web and native: F# schema records and the authoring core
   under Fable, `farm-wasm`, the web editor on both, native numerics (schema
   v9)

## Remaining work

Status in October 2026 (0.6.0): phases 1–7 are done
([Phase 7](#phase-7-one-engine) below), and so are the
[editor gaps found after phase 7](#editor-gaps-closed-after-phase-7). What's left is under
[Toward full-size farm games](#toward-full-size-farm-games) and
[Later](#later). The checklists in this section are the record of the port.

**Rust core (phase 2)**: done.

- [x] `SaveMigrations.cs` ported to `farm-cart::save`; all eight save
  migration goldens (`fixtures/golden/saves`) match the TypeScript result,
  stable JSON and hash.
- [x] `FarmEngine.Runtime` logic ported to `farm-runtime`: `input`,
  `minigames`, `panels` and the `audio` model, with every C# runtime test
  ported (81 tests). Minigames take their cosmetic randomness from the host
  (`with_random`) instead of the OS.
- [x] The four ignored `farm-sim` tests were test setup problems (the
  starter farm's own Kitchen; the C# 6x6 test field). All four pass now.
- [x] Save files (`farm-cart::save_file`) carry the header from
  [EXPORT.md](docs/EXPORT.md#what-earlier-phases-must-get-right): save
  format, `gameId`, game version and cartridge hash. A save from another
  game is refused, one from a newer game version loads with a warning, and
  when the content changed, inventory items are matched by string id:
  removed items go to quarantine and come back with their id. Exposed as
  `fe_session_save` / `fe_session_load_save` and `RustSession.Save()` /
  `LoadSave()`. Until the export settings exist (phase 3), `gameId` is the
  project id.

**F# authoring (phase 3)**:

- [x] F# authoring core merged: `Edit` cases for every editor, `Defaults`,
  the problems pipeline with JSON paths, workshop patterns, the C# `Api`,
  and `ProjectWorkspace` on the F# `Document` (62 F# tests, all app tests
  pass).
- [x] The F# test project hung `dotnet build` on SDK 10.0.112 (minutes per
  file; `DefaultsTests.fs` was excluded). The cause was nullness checking in
  the tests; it is now off for the test project only, the whole project
  compiles in about 15 seconds and `DefaultsTests.fs` is back (68 F# tests).
- [x] Project migrations in F#: `Migrations.fs` (v1→v8 and exported games)
  runs on a Fable-safe immutable JSON type (`Json.fs`) with JavaScript
  semantics. Every migration golden passes, and a differential test agreed
  with the C# `Migrations.MigrateProject` on 25 inputs, step by step, until
  the C# code was deleted in phase 7. `ProjectStore` (open and import) uses
  the F# migrations.
- [x] Validation in F#: `SchemaChecks.fs` (the schema checks of
  `SchemaValidation.cs`, project and exported game) and `ContentLints.fs`
  (`Validation.cs`, the web `validateProjectContent`). The Problems panel
  and the F# migrations use them. A parity test compared them with the C#
  on every golden and sample project and on 111 broken projects that
  together hit every check; since phase 7 those broken projects are
  expectation tests, and the content-lint findings are recorded.
- [x] F# pack namespacing, compatibility checks, load order, conflict-aware
  merge, localization and project import. The built-in authored catalog and
  project-to-content compiler now also run in F#. Cartridge exports, editor
  content previews and Problems use this path. Content goldens and differential
  tests preserve TypeScript behavior, including absent/null fields and ordering.
- [x] Remove `FarmEngine.Authoring`'s dependency on the C# simulation project:
  map construction and layer edits use F# authoring helpers, and the assembly
  dependency is guarded by a test.
- [x] Port the Farm Essentials starter pack, blank project, Cozy Garden and
  Quest RPG factories to F#. New Project, first launch and imported-game
  defaults use the F# catalog. Time is supplied by the desktop host. The app
  no longer references `FarmEngine.Content`.
- [x] Cartridge format 2: compiled content, the new-game start state and
  presentation data as separate sections, and an asset table that stores each
  embedded file once. The player no longer reads project JSON. Binary saves
  (`save.fbs`) carry a slot preview and a zstd-compressed state; JSON saves
  still load. Content keeps string ids; interned indexed tables only pay off
  once the simulation indexes by integer.
- [x] `farmc compile` writes a deterministic cartridge; Rust verifies and
  loads it, and cross-language tests compare its content, start state,
  presentation and play state with the project path.
- [x] Add optional export identity/window/target settings to the project,
  with a stable generated game id and F# Problems validation. The cartridge
  compiler and Export Game consume them.
- [x] Editor controls for the export settings (title, executable name,
  version, company, icon), in Project Settings.
- [x] Move the rest of `FarmEngine.Authoring` off the C# schema records so
  the project compiles under Fable. Done in phase 7 (below).

**Switch the app (phase 4)**: done; the C# engine projects are deleted.

- [x] Play Mode moved to the Rust engine, first behind a C# fallback and a
  differential test against the C# engine after every step, then (phase 6)
  to the embedded Rust player; the fallback, the C# overlays and the state
  mirror are gone.
- [x] `farm-render` ports all of `FarmEngine.Rendering`: typed snapshots,
  Edit Mode snapshots, art decoration, the built-in art pack, draw lists, a
  CPU rasterizer and embedded fonts, checked against the C# reference before
  it was deleted. Edit Mode's map and art previews draw with it (only the
  visible region; `RustPreview`).
- [x] The C# simulation, runtime, renderer and content projects are deleted
  along with their tests (the Rust and F# suites carry ports; the shared
  fixtures live in `fixtures/projects`). `FarmEngine.Interop` keeps a
  JSON-only headless session (`RustSession`) for tools and tests.

**Editor port (phase 5)**: map tools and a broad content workspace are now
available as native views.

- [x] Tile painter (layers, rectangle, fill, copy/paste), scene manager,
  transitions and collision; art bindings remain in the art pipeline milestone.
- [x] Content workspace for NPCs, dialogue, items, crops, quests, events,
  shops, recipes, node and machine types, animal species, fish tables,
  actions and minigames. Every save/removal uses the F# document and undo
  history.
- [x] Project settings and calendar editor; Problems panel with navigation
  to affected map scenes and content entries.
- [x] Mods panel: validate and review pack manifests, permissions and plugin
  source before install; enable/disable, reorder, remove and import packs.
- [x] Art import (PNG, JPEG, WebP, GIF, BMP and SVG normalized to PNG),
  animation clip slicing and frame edits, visual bindings for the player,
  map brush and content definitions. Art edits use F# undo/redo.
- [x] Creator Workshop exposes the 13 F# patterns with a one-step undo and
  links to their generated content. Interface editor creates and edits
  in-game panels and their entries.
- [x] Nested editors and cross-reference pickers for every content type.
  F# `References` declares what each schema field holds (a test fails when an
  id-like field is undeclared) and lists picker options from the content
  compiler; F# `Vocabulary` ports the condition/outcome forms. Forms show
  nested records, list rows with add/remove/move, reference chips and a
  collapsed Edit as JSON box per nested field. Missing ids stay visible.
- [x] Export settings controls in Project Settings, with the `ChecksExport`
  problems shown inline.
- [x] SVG import (Svg.Skia, self-contained files only, optional size), frame
  duplication and frame durations (one frame or every frame), as F# edits.
- [x] Map placement tools (items, nodes, machines, NPCs, animals, remove),
  the eyedropper, the transitions list with Clear all, a default tile for new
  scenes, and "Go to asset" in Problems.
- [x] Welcome tour on first launch, Help menu (Creator Guide on F1, keyboard
  shortcuts, language), the game interface in English and Spanish with a
  readable-font option (Atkinson Hyperlegible), and the editor's mode names,
  Play Mode toolbar and help in Spanish.

**Player and export (phase 6)**: done. `farm-plugins` (QuickJS in wasmi),
`farm-ui`, `farm-player`, game shell, Export Game, and the player embedded in
Play Mode.

- [x] Headless `farm-player` loads `game.cart` beside the executable or by
  `--cart`, replays commands, checks a hash, and loads/writes portable saves.
- [x] `farm-ui`: an immediate-mode UI on `farm-render` draw lists, with
  stable widget ids, layout, scrolling, focus navigation for keyboard,
  gamepad and mouse, and a theme with UI scale and text size. It ports every
  Play Mode overlay (HUD, dialogue, shop, crafting, inventory, quest log,
  creator panels, minigames, toasts, credit). Every action is an engine
  command, and the rules come from Rust queries (`farm_sim::overlay`, shared
  with `farm-ffi`).
- [x] Graphical `farm-player`: title screen, three save slots with previews
  and thumbnails, autosave each morning, pause menu, settings (display,
  audio, controls, accessibility) and credits. winit and softbuffer, gilrs
  gamepads, cpal audio, TOML settings in the user's folders, a crash log.
  `--screenshot` renders PNGs; screenshot goldens at 1280×800 and 1920×1080.
  The `Player` library runs standalone or embedded, from a cartridge or a
  project, is `Send`, and reports engine failures as errors.
- [x] Play Mode embeds `Player` through `farm-ffi` (`fe_player_*`,
  `RustPlayer`): the game and all of its UI are drawn by Rust exactly as in
  an exported game, frames run on a worker thread, sounds play through cpal,
  and the editor keeps Restart, Keep changes and the debug drawer. The C#
  play views are deleted.
- [x] `farm-plugins`: the plugin sandbox in Rust. QuickJS is compiled to
  WebAssembly (checked in; `tools/plugin-guest/build.sh` rebuilds it) and
  each plugin gets its own instance in wasmi, a pure-Rust interpreter with no
  JIT. Budgets are deterministic fuel, not wall-clock time: an infinite loop
  stops after about 40 ms in release builds. Strikes, errors, mutation
  validation and the command queue work like the Jint host, and every C# and
  web plugin test is ported. `PluginRuntime` feeds a step's hook events to
  plugins and hands mutations back as commands.
- [x] Play Mode runs pack plugins in `farm-plugins` (through the embedded
  player); the Jint host is deleted.
- [x] `farm-render` supplies world snapshots, draw lists and a CPU
  rasterizer that also builds for WebAssembly. The desktop player can draw
  the same draw lists with wgpu (`farm-gpu`); GPU effects are under [Later](#later).
- [x] Export Game packaging (`FarmEngine.Export`, File → Export Game…,
  `farmc export`): Problems gate, deterministic cartridge, renamed player
  template, Windows icon and version info patched from .NET, Linux `.png` and
  `.desktop`, license notices, reproducible `.zip`/`.tar.gz`. Releases build
  both templates (Linux in Steam Runtime sniper) and ship them in `players/`.
  Exported games run the graphical player.
- [x] Ship only used assets: the compiler leaves unused custom assets out of
  the cartridge, and Export lists them as warnings.

**Audit follow-ups**:

- [x] Run plugin hooks off the UI thread: Play Mode frames, plugins included,
  run on a worker thread.
- [x] `respawnDays` keeps absent and `null` apart in both engines, like the
  TypeScript `.nullable().optional()`, so hand-written packs hash like the
  web version (content golden `packs-nodes`).
- [x] Both `settings.json` stores share `JsonSettingsStore.JsonOptions`.
- [x] The editor renders on a supported Skia (#114): Avalonia 12.1, SkiaSharp
  3.119 and Svg.Skia 5.1, with the app tests on xUnit v3. The managed image
  header checks and the SVG renderer's lockdown stay; Windows, GPU and
  Wayland still need a check by hand
  ([docs/SKIA-MIGRATION.md](docs/SKIA-MIGRATION.md)).

## Phase 7: one engine

The web editor (`jxburros/farm-game-engine`, private)
and this app now run the same code: Rust plays the game, F# understands the
project. See [docs/LANGUAGES.md](docs/LANGUAGES.md#phases).

- [x] **Schema records in F#.** `src/FarmEngine.Authoring/Schema.fs` holds
  the project, content and save shapes as immutable F# records (options,
  lists, ordered maps and the `EventCondition` union) with JSON codecs
  (`SchemaJson.fs`) generated from the records (`tools/codegen/schema-json.fsx`); parity tests against the Rust
  serde types guard them (docs/LANGUAGES.md "Two project pipelines"). The C# `FarmEngine.Schemas` project and its
  migrations, validation and generated FlatBuffers readers are deleted. The
  editor builds and updates records through generated `WithField` extensions
  (`RecordWith`), and reads them through `FSharpInterop`.
- [x] **The authoring core compiles under Fable.** No reflection,
  System.Text.Json or .NET-only APIs in `FarmEngine.Authoring`: JSON, UTF-8,
  base64, SHA-256 and the FlatBuffers builder are plain F#. `WebApi.fs` is the
  JavaScript entry point. CI compiles it (`tools/fable/build.sh`) and checks
  in Node that it reproduces the .NET cartridges byte for byte and every
  migration golden (`tools/fable/smoke.mjs`).
- [x] **`farm-wasm`**: the player, headless sessions and previews for web
  pages, sharing the host protocol (`farm-host`) with `farm-ffi`. It replays
  every golden in Node, and CI builds and smoke-tests it.
- [x] **The web editor on the shared core**: its Play Mode runs `farm-wasm`,
  and opening, importing, the Problems panel and a cartridge export run the
  Fable build (vendored in the web repo's `packages/engine-native`).
- [x] **Play Mode runs cartridges.** The editor playtests the cartridge the
  F# compiler makes, like an exported game, and Keep changes writes the final
  state back in F# (`Playtest.applyState`), checked against Rust's own
  write-back.
- [x] **Benchmarks and property tests**: `farm-bench` checks the time budgets
  in CI (saving is about 2.5x over its 1 ms target since the stable-JSON writer
  streams from `Serialize`, and is held to a regression ceiling); proptest
  covers replay determinism and save round trips, FsCheck covers migrations
  and compiling.
- [x] **Native numerics (v9)**: every quantity in `farm-sim` is an integer in
  a fixed unit (float arithmetic is denied outside the JSON adapters), JSON
  stays in authoring units, the state hash is xxh3-64 over a canonical binary
  encoding, saves are version 5 and projects schema 9 (both migrate on load),
  and the goldens are re-recorded from Rust; the TypeScript goldens stay in
  `fixtures/golden/v8` as migration inputs. Before re-recording, every v8
  replay was checked to give the same player-visible outcome at every step.
  See [docs/NUMERICS.md](docs/NUMERICS.md).
- [x] **Web demo export**: Export Game's web target runs `farm-wasm`'s
  standalone player in a page for itch.io.

## Separate projects (0.5.0, 0.6.0)

Tractr and the web version are separate projects now, and neither has old
projects to carry forward, so the compatibility code is gone:

- [x] The Fable build of the authoring core (`tools/fable`, `WebApi.fs`, the
  Fable dotnet tool and its CI step).
- [x] Project migrations (`Migrations.fs`): `ProjectLoad` reads schema v9 only
  and refuses older versions with a message.
- [x] Save migrations (`farm-cart::save`): only save version 5 loads.
- [x] The TypeScript golden generator (`tools/golden`) and the v8 goldens and
  outcomes (`fixtures/golden/v8`, `v8_outcomes`); every golden is recorded
  from Rust. The RNG and stable-JSON vectors stay as fixed vectors
  (`fixtures/golden/vectors`).
- [x] `farm-wasm` is the web demo's player only (0.6.0): `Session`,
  `Preview`, `renderJson` and `hashText` are removed.
- [x] Project schema version 1 (0.6.0): the fields kept only for older files
  are gone (crop millisecond growth times and `plantedAt`, the `customImage`,
  `playerCustomImage` and crop `customAsset` image fields, the `unlockScene`
  outcome, the player's pixel and target fields, `currentTime`), and the
  version numbering starts over at 1.
- [x] Web interchange (0.6.0): Import Project JSON reads whole projects only
  (no web "exported game" files), JSON saves need their header (no bare
  `GameState`), the old text hash (`fe_hash_text`) and cartridge format 2's
  reserved project slot are gone (cartridge format 3), and the TypeScript
  reference fixture `packs-project.ts-reference.json` and its tests are
  deleted.

## Editor gaps closed after phase 7

After phase 7 a review listed what the editor still lacked. All of it is done:

- [x] Keyboard map editing (arrow-key cursor, Enter/Space applies the tool,
  announced tile position) and screen-reader names across the editor.
- [x] Touch controls in the web demo (D-pad, Interact, Sleep, Inventory, Menu)
  through the player's `action` input event.
- [x] Profit per craft and per hour for recipes; profit per harvest and a
  summary card for crops (F# `Readouts`).
- [x] An error screen with Try Again, and a warning with Retry when a save
  fails.
- [x] Art studio: click the sprite sheet to add frames, asset thumbnails,
  preview Pause/Play, animations from frames of several images.
- [x] NPC portraits in the game's dialogue box.
- [x] Custom layouts for NPC schedules, crop tabs and shop stock; the skill
  level curve as rows; built-in crops and node types in the content lists;
  thumbnails in lists and forms; item and action dropdowns in the interface
  editor.
- [x] Workshop "Build your game" links, duplicating a door, dialogue and asset
  counts in the project stats, the scene size calculator.

## Export Game

Export makes real desktop games, not browser games. It copies a prebuilt
`farm-player` for the target, renames it, and puts the compiled `game.cart`
next to it. Windows and Linux (including Steam Deck) came first; a web demo
for itch.io pages (the `farm-wasm` player in a page) is done too, and macOS
comes later.

Packaging works now: **File → Export Game…** and `farmc export` build the
Windows, Linux and web folders and archives from the prebuilt templates. The games
it makes run the graphical player, with a title screen, save slots, settings
and gamepad support ([docs/PLAYER.md](docs/PLAYER.md)).
[docs/EXPORT.md](docs/EXPORT.md) has the design, what is done and what is left.

## Content packs

- [x] **Minigames from content packs.** A pack plugin defines a minigame kind
  with `api.minigame(name, { start, update, draw })`, allowed by the
  manifest's `permissions.minigames`. Each session runs in its own sandbox
  instance with a fuel budget per frame; `update` gets the routed game input,
  `draw` answers a checked list of rectangles, text and item sprites that
  `farm-ui` draws in the minigame window, and the score resolves the def's
  tiers like a built-in kind (a plugin that fails or breaks a limit scores 0
  and reports the error). The editor offers the enabled packs' kinds, Problems
  knows them, and cartridges carry each plugin's granted kinds. No guest
  change: the API is a JavaScript prelude over `api.on`
  ([docs/PLUGINS.md](docs/PLUGINS.md#minigames)). Since 0.7.0 pack
  minigames get pointer input and can play sounds (M2).

## Done since the port

Shipped in 0.3.x and 0.4.0, after phases 1–7 (details in
[CHANGELOG.md](CHANGELOG.md)):

- [x] **Tractr.** The app, executable, installer, data folders, C# projects
  and "Made with" credit carry the new name, and the editor wears the Seed
  Catalog look ([docs/brand](docs/brand/README.md)).
- [x] **Music and ambience.** Day and night music, birdsong, crickets and
  rain, synthesized by the engine (`farm-runtime/src/music.rs`). They follow
  the clock, the weather and indoor scenes.
- [x] **More built-in minigames**: Rhythm tap, Moving target and Memory
  sequence join the timing bar, with per-kind settings in the editor and
  Problems checks for them.
- [x] **GPU renderer** for exported desktop games (`farm-gpu`, wgpu), with
  the CPU renderer as a fallback.
- [x] **Calendar and clock**: games can start in any season, the clock can
  pause in dialogue, shops, minigames and menus, and scenes can be marked
  indoor (no weather).
- [x] **Exported game quality of life**: pausing in the background, F12
  screenshots, `.bak` saves, an error screen in place of a silent crash
  (the "message box when the game crashes" item from [Later](#later)).
- [x] **Hardening**: `farm-fuzz` targets and invariant tests, hostile project
  files refused, deletions that keep gated content locked, F#/Rust parity
  tests.

## Toward full-size farm games

The engine already has most of a farm game's systems: crops with quality,
mutations, multi-tile growth and regrowth; gathering nodes; machines, recipes
and crafting stations; animals; fishing; a procedural mine; weather; skills;
NPC schedules, gifts and birthdays; quests; shops; events, actions,
minigames and creator panels; a creator-defined calendar; content packs with
sandboxed plugins. Many of these systems are shallow next to Stardew Valley,
Harvest Moon or Story of Seasons, though, and the basic vocabulary of the
world (terrain, tools, item kinds) is fixed in code. That vocabulary is what a
game on an alien planet would need to change.

The milestones below are roughly in priority order. M1 came first because
later milestones build on data-driven terrain, tools and items; M1 and M2 are
done (0.7.0), M3, M4 and M11 (0.8.0), and M5, M6 and M7 (0.9.0). Each change to
the content model goes through the usual path: F# records and checks, Rust
serde types and parity tests, a schema version bump, and goldens. (Tractr
has no project migrations since 0.5.0; add one when a schema change has
released projects to carry forward.)

### M1: A data-driven world vocabulary

Done in 0.7.0: the closed enums are content, so a game defines its own world
([Creator Guide](docs/CREATOR-GUIDE.md#your-games-world)). A new project starts
with the classic vocabulary as built-in definitions, which reproduce the old
rules exactly (every recorded replay keeps its states and effects).

- [x] **Terrain types.** `TerrainDefinition`: the layer it is painted on, its
  colour and art, walkable, plantable (soil), water source, fishable, blocks
  light, hazard energy, sheltered from the weather, and transforms (which tool
  action, or which tool, turns it into what). The seven tile types are the
  built-in terrains; Problems reports tiles of a terrain the game lacks.
- [x] **Tool types.** `ToolDefinition`: an action (water, till, harvest, fish,
  break, transform, place, plant), its play-mode key, energy cost, the area per
  tier (width across the facing, rows ahead), the skill its node breaks train
  and the terrain it places. The six tools are the built-in ones.
- [x] **Item categories.** `ItemCategoryDefinition`: plantable, fertilizer,
  equippable (a tool), edible (energy, or the item's own), placeable,
  giftable, sellable. The engine and the game's screens ask the category, not
  the type name.
- [x] **Crop qualities and mutations.** `CropQualityDefinition` (price, yield
  and mutation-chance multipliers, stars, colour; ranked in list order) and
  `CropMutationDefinition` (yield multiplier, share of the mutation chance,
  colour).
- [x] **Growth requirements.** A crop's `requirements`: terrains, scene tags
  and fertilizers checked when it is planted; light or dark, a temperature
  range (scene, else season) and neighbouring crops, terrain or water checked
  each night, holding growth back when not met.
- [x] **Atmosphere.** The day/night tint keyframes and the foliage tint per
  game, season and scene; weather overlays as definitions (rain, snow, dust or
  a tint, with colour, density, speed and tint); the length of a day
  (`time.dayMinutes`).
- [x] **Words of the game**: how money is written, what energy and the skills
  are called, in every language (`settings.words`).

### M2: Sound and cutscenes

Done in 0.7.0.

- [x] **Custom audio.** Sound and music assets (OGG, WAV) in the project and
  the cartridge. `playSound` plays a sound asset (or a built-in sound by
  name); music is chosen by the `playMusic` outcome or a cutscene, then
  today's festival, the weather (outdoors), the scene, the season, else the
  built-in synthesized music.
- [x] **Cutscenes.** Steps that run over time: say, choice (branching to step
  labels), jump (with conditions), move, face, emote, wait, camera, fade,
  music, sound, outcomes, end; started by the `startCutscene` outcome,
  skippable by the player.
- [x] **Sound and pointer input for pack minigames**: `update` gets
  `input.pointer` (mouse or touch over the minigame, in canvas units) and may
  answer `sounds` to play (docs/PLUGINS.md#minigames).

### M3: Event logic for creators who don't write code

Done in 0.8.0 ([Creator Guide](docs/CREATOR-GUIDE.md#event-logic)).

- [x] **Variables.** Flags hold numbers and texts as well as true or false:
  the `variable` condition compares one (`eq`, `ne`, `lt`, `le`, `gt`,
  `ge`), `setVariable` and `addVariable` change one, and `{var:NAME}` shows
  one in text.
- [x] **Logic.** `all`, `any` and `not` groups of conditions, nested; a
  `chance` condition rolled from the game's seed and the moment, without
  touching the game's random stream. Also `scene`, `hearts`, `dayOfSeason`
  and `relationship` conditions.
- [x] **More triggers.** Day start, a time of day, harvesting, selling,
  crafting, catching a fish, picking up an item, a friendship change and a
  quest changing, the last seven optionally about one subject (a crop or
  item, an NPC, a quest). Events an outcome triggers run too, four levels
  deep at most.
- [x] **More outcomes**: `moveNpc`, `startCutscene` (M2), `setWeather`,
  `giveExperience`, `addShopStock` and `removeShopStock` (kept in the save),
  `placeNode`, `removeNode`, `placeMachine`, `removeMachine` and
  `setRelationship`.

### M4: Dialogue and relationships

Done in 0.8.0 ([Creator Guide](docs/CREATOR-GUIDE.md#people-and-relationships)).

- [x] **Daily lines.** An NPC's `lines` have the events' conditions (day,
  season, weather, scene, hearts, flags) and a weight; each day the NPC picks
  one that holds, in place of its first dialogue's text.
- [x] **Text substitution**: `{player}`, `{farm}`, `{spouse}`, `{npc:ID}`,
  `{item:ID}`, `{var:NAME}`, the date, the time and money, in dialogue,
  cutscene lines, event and action messages, quests and game panels.
- [x] **Portrait expressions** chosen per line, dialogue and cutscene line
  from an NPC's `portraits`.
- [x] **Relationships**: friendship decay, heart events as a content type
  (at given hearts, with a cutscene, on entering a scene or talking), dating,
  engagement and marriage by gifts at given hearts, a spouse who moves to the
  home tile and does a morning chore, children. All off until a game turns
  them on (`settings.relationships`, Settings → People).
- [x] **Player character**: a character creator (name, farm name, skin, hair,
  shirt, trousers and accent colours, Random) when `settings.characterCreator`
  is on; hats, shirts and trousers as items worn from the inventory (`wear`),
  drawn on the player.

### M5: Building the farm

Done in 0.9.0 ([Creator Guide](docs/CREATOR-GUIDE.md#building-the-farm)).

- [x] **Placeable objects**: machine types gain **storage** (chests, opened
  as a modal; picked up only when empty), **shipping** (bins, M6), **gates**
  that open and close (an open gate never blocks) and **area effects**:
  sprinklers water their square each morning, scarecrows keep storms and
  pests (`settings.farm.pestChance`) off theirs. Furniture and decoration are
  machine types without these; all are placed and picked up in play.
- [x] **Buildings**: a content type with a footprint, a door, an interior
  template, cost, materials, nights of building, the scenes it may go in and
  an animal capacity. A shop that lists buildings is a builder: the player
  places one on a free spot of the farm, it stands after its nights with its
  own copy of the interior, and **upgrades** (`upgradeOf`) rebuild a standing
  building into the next kind, keeping what stood inside. The house is a
  building on the map from the start (`buildings`) and upgrades the same
  way. A scene with **every season** on grows any season's crops (a
  greenhouse).
- [x] **Animals with housing**: species name the buildings that house them;
  shops sell them into a building with room; more products per species,
  each on its own interval and mood; product quality by mood; young born
  overnight in buildings with room.
- [x] **Farm layouts**: `settings.farm.layouts` lists farm maps; with two or
  more a new game asks which, and the chosen map takes the farm scene's
  place.

### M6: Long-term goals

Done in 0.9.0 ([Creator Guide](docs/CREATOR-GUIDE.md#long-term-goals)).

- [x] **Shipping bin**: what a bin holds is sold overnight at the item's
  value for its quality, with a log of the units shipped per item and of
  last night's shipment.
- [x] **Collections**: lists of items found, shipped or donated (in the
  collection's scene: a museum), with rewards at counts.
- [x] **Bundles**: lines of items (a quantity, a lowest quality) to hand in,
  grouped by room; complete, a bundle runs its outcomes, which unlock or fix
  the world.
- [x] **Achievements**: unlocked the moment their conditions hold, shown in
  the game (hidden ones as ???), with an `achievementUnlocked` effect and a
  Steam id kept for Steamworks (see [Later](#later)).
- [x] **Mail**: letters arrive in the morning from the `sendMail` outcome
  (events, actions, dialogue, cutscenes), quest rewards and their own
  conditions (an NPC's hearts, the new `birthday` condition), with items
  and money attached and outcomes run on reading.
- [x] New conditions (`money`, `skillLevel`, `mineFloor`, `building`,
  `collection`, `bundle`, `achievement`, `shipped`, `slain`, `birthday`)
  and triggers (`build`, `bundle`, `achievement`, `slay`) for goals of
  every kind.

### M7: Combat and adventure (optional per game)

Done in 0.9.0 ([Creator Guide](docs/CREATOR-GUIDE.md#combat-and-adventure));
off until `settings.combat.enabled`.

- [x] **Player health**, separate from energy: damage from monsters and
  terrain with `damage`, a moment of safety after a hit, healing from food
  (`health`), the `modifyHealth` outcome and sleep; fainting carries the
  player to the faint spot at half health for a money penalty.
- [x] **Monsters**: a content type with health, damage, defense, speed,
  movement (chase within sight, wander, still), drops and experience,
  spawned each morning by a scene's spawns and on mine floors by their depth
  band. Their moves roll from the game's seed, never its random stream.
- [x] **Weapons** and the combat skill: a tool whose action is `attack` hits
  the monsters ahead for its power plus the combat level, less their
  defense; slaying one gives its drops and combat experience.
- [x] **Mine floors with hazards and treasure**: depth bands scatter hazard
  terrain and leave weighted treasure, besides rocks and monsters.

### M8: Projects at full size

- [ ] **Assets as files.** Images live as base64 `dataUrl` strings inside
  the project JSON (`CustomAsset.DataUrl`), so a game with hundreds of
  sprites makes a huge file that is slow to open and impossible to review in
  git. Move to a project folder: project JSON plus one file per asset,
  with single-file JSON import and export kept for sharing a project.
- [ ] **Split project files** (scenes, NPCs, dialogue, items) so teams can
  work on a game together through version control.
- [ ] **Bigger and richer maps**: scenes above 256×256 tiles, more than the
  three fixed layers, autotiling, and tall objects drawn in front of or
  behind the player.
- [ ] **Light sources** at night (lamps, glowing crops, windows), with the
  GPU lighting pass from [Later](#later).

### M9: More power for content packs

- [ ] **Game panels from packs** (the `uiPanels` permission is reserved and
  grants nothing yet).
- [ ] **More hooks and mutations**: hooks for harvest, sale, craft, catch,
  scene entry and time of day; mutations for the M3 outcomes; plugins that
  add their own systems with saved state.
- [ ] **Packs add M5–M7 content**: buildings, collections, bundles,
  achievements, letters and monsters in content packs, namespaced like the
  rest (projects have them since 0.9.0; packs don't yet).

### M10: Playing together

- [ ] **Local and online co-op.** The deterministic engine and its command
  log are a good base for lockstep multiplayer: shared farm, separate
  inventories and energy, and a split-screen or networked player.

### M11: Finishing M1 and M2

What 0.7.0 left out of the world vocabulary, sound and cutscenes. Done in
0.8.0.

- [x] **Packs add vocabulary.** Content packs add terrains, tools, item
  kinds, crop qualities and mutations under their own ids (replacing a
  built-in or earlier one only when listed in `overrides`), and cutscenes and
  heart events namespaced like other content; importing a pack writes them
  into the project.
- [x] **Audio memory.** Only sounds up to 5 seconds stay decoded. Music and
  longer sounds stream from the cartridge's compressed bytes, decoded and
  resampled about half a second ahead of the output. The player's cache of
  resampled sounds holds at most 16 MB and lets go of sounds no longer
  playing.
- [x] **Audio in the editor**: a play button previews a sound or music
  asset through the player's decoder (`farm_player::preview`, via
  farm-ffi), sound and music assets can be renamed (the id stays), and
  Problems reports files the player can't decode (`asset.audioUnreadable`,
  an error).
- [x] **Music across every scene change.** Music an event or cutscene chose
  ends whenever the player changes scene: doors, warps, the mine's ladders
  and exit, and debug teleports. Keep changes carries it into the next
  playtest.
- [x] **Cutscene polish**: smooth camera pans between ticks, NPC and player
  pickers for a step's actor and speaker, creator-art NPC sprites that turn,
  carrying a running cutscene through Keep changes, and cutscene commands in
  the `farm-fuzz` generator.
- [x] **Edit Mode previews the sky**: a Sky box above the map draws the
  scene at a time of day, season and weather with play's day/night tint,
  foliage tint and weather overlay (none indoors).
- [x] **Editor details**: a new scene filled with a game's own terrain
  places it on the terrain's layer; a new weather overlay shows in the
  weather types' pickers before the weather is saved; removing a season
  removes its sky look and Problems reports one left behind; the Edit Mode
  strings M1 and M2 added go through `EditorStrings` (the rest of Edit Mode
  stays under [Later](#later)).
- [x] **Advisories for the audio crates**: cargo-deny's advisory check runs
  on every pull request (a required check) and weekly, and fails on
  unmaintained notices too. lewton, ogg, hound and cpal have none;
  `deny.toml` records that lewton (no release since 2021, no unsafe code) is
  replaced by symphonia if a notice appears.

## Later

- **Localization** of the rest of the editor UI (tabs, forms and Problems;
  the menu bar, mode names, Play Mode toolbar and help are translated) and of
  the Creator Guide. The game's interface and the engine's own messages are
  translated already.
- **Code signing** for the Windows installer (see `docs/RELEASING.md`).
- **Player polish.** GPU effects: day/night lighting, season palette swaps
  and weather particles as passes of the wgpu renderer (`farm-gpu` draws
  the CPU renderer's frames today and falls back to it; docs/PLAYER.md
  "Renderers"), each with a CPU equivalent or a clear "GPU only" rule;
  drawing the remaining shapes (circles, strokes, blurs) in the shader
  instead of as CPU-rasterized sprites; the GPU path for the editor's Play
  Mode (it hands CPU pixels to the C# app). Gamepad rebinding in the controls
  menu (keys are rebindable today).
- **macOS/Linux builds** of the editor. Avalonia and Velopack already support
  them; this needs packaging and CI jobs. (Exported *games* get Linux builds
  in phase 6, whatever the editor runs on.)
- **More export targets**: macOS, and Steamworks integration (achievements,
  overlay, Steam Input). See the open questions in
  [docs/EXPORT.md](docs/EXPORT.md#open-questions).
