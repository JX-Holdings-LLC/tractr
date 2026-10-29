# Tractr roadmap

What Tractr does today, and what comes next. Finished work is recorded in
[CHANGELOG.md](CHANGELOG.md); this page looks ahead.

## Where Tractr is

Tractr makes complete 2D farming games without code. The editor covers
maps (up to 512×512 tiles, with layers, autotiling, tall objects and
lights), a world vocabulary creators define themselves (terrains, tools,
item kinds, crop qualities, the sky), crops, animals, machines and
buildings, fishing, forage and mines, people with routines, relationships
and heart events, quests, request boards, festivals and cutscenes, events
with conditions and outcomes, skills, perks and buffs, long-term goals,
optional combat, custom art and sound, and content packs with sandboxed
plugins. Projects are folders that teams can keep in git. Export Game builds
Windows and Linux games (Steam Deck ready) and a web demo, all running the
same deterministic Rust player as the editor's Play Mode. The
[Creator guide](docs/CREATOR-GUIDE.md) describes all of it.

## Next

### Playing together

- [ ] **Local and online co-op.** The deterministic engine and its command
  log are a good base for lockstep multiplayer: a shared farm, separate
  inventories and energy, and a split-screen or networked player.

### Shipping games

- [ ] **macOS games.** An `.app` export target, with signing and
  notarization by the creator's own Apple Developer account
  ([Export internals](docs/dev/EXPORT-INTERNALS.md#open-questions)).
- [ ] **Steamworks.** Achievements (projects already store a `steamId`), the
  overlay and Steam Input, as an optional player feature with its own
  template.
- [ ] **A clean start state.** An explicit "a new game starts here" state, so
  playtest changes kept with Keep changes don't ship as a game's opening.
- [ ] **Player-installed mods** for exported games.
- [ ] **Web demo options**, such as ending the demo after a number of days.

### The editor

- [ ] **Localization** of the rest of the editor UI (tabs, forms and
  Problems) and of the Creator guide. The game's interface and the engine's
  own messages are translated already.
- [ ] **Farm-building polish**: Edit Mode draws the buildings a game starts
  with (they show in play); the game's toolbar keeps one row at 1280×800 when
  a game has Goals and Mail.
- [ ] **Code signing** of the Windows installer and notarization of the Mac
  app ([Releasing](docs/dev/RELEASING.md#unsigned-builds-and-smartscreen)).
- [ ] **A balancing lab**: run thousands of seeds through the engine and
  chart gold per day per crop or strategy, flagging dominant crops.
- [ ] **Text beyond Latin, Greek and Cyrillic** in the game's fonts, and
  right-to-left text.

### The player

- [ ] **GPU effects**: season palette swaps and weather particles as passes
  of the wgpu renderer, each with a CPU equivalent or a clear "GPU only"
  rule; drawing circles, strokes and blurs in the shader instead of as
  CPU-rasterized sprites; and the GPU path for the editor's Play Mode.
- [ ] **Gamepad rebinding** in the controls menu (keys are rebindable today).
- [ ] **Faster large frames**: capping the UI's internal resolution,
  skipping unchanged menu frames and upscaling straight into the window's
  buffer.

### The engine

- [ ] **Saving off the simulation thread**, to meet the 1 ms save budget.
- [ ] The **Future** designs in [Architecture](docs/dev/ARCHITECTURE.md#future-not-built):
  interned cartridge content, a condition and dialogue language, units of
  measure on authored quantities, and generated FFI bindings.

## Proposing something

Open an [issue](https://github.com/JX-Holdings-LLC/tractr/issues) that says
what a creator or player would be able to do. For engine work, the
[architecture checklist](docs/dev/ARCHITECTURE.md#checklist-adding-an-editor-feature)
says where each part of a feature goes.
