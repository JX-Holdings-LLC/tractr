# Plugin sandbox internals

How `farm-plugins` runs pack plugins. What plugin authors need is in
[Pack plugins](../PLUGINS.md): hooks, mutations, permissions, timing,
minigames, limits and determinism, which this page doesn't repeat.

## The pieces

| Piece | Where | What it does |
|---|---|---|
| The guest | `crates/farm-plugin-guest`, built to `crates/farm-plugins/guest/farm_plugin_guest.wasm` | QuickJS (quickjs-ng through `rquickjs-sys`) compiled to `wasm32-wasip1`, with a fuel counter compiled in by `tools/plugin-guest/meter`. Checked in, so building the engine never needs a C toolchain; [its README](../../crates/farm-plugins/guest/README.md) has the recorded hashes. MIT-licensed. |
| The sandbox | `farm_plugins::sandbox` | The guest ABI (`fp_prepare`, `fp_init`, `fp_dispatch`, and since ABI 2 the host calls `farm.host_call` / `farm.host_answer`), written against two small traits (`GuestEngine`, `GuestInstance`) and implemented with wasmi. Instances start from a prepared memory image. |
| The prelude | `crates/farm-plugin-guest/src/prelude.js` (`farm_plugins::PRELUDE`) | `extend(api, pack)`: `api.state`, `api.minigame`, `api.condition`, `api.outcome`, `api.entity`, `api.itemUse`, `api.timer`, `api.screen` on top of the guest's `api.on` and `api.host`. Part of the bootstrap, so it is compiled once into the prepared image and a plugin's start never pays for it. |
| Reads | `farm_plugins::reads` | The read API (#237): answers `{ read: … }` host calls from the `GameView` (content and state) of the step being dispatched, checked against `permissions.read`, each charged to the call's fuel. |
| Conditions | `farm_plugins::conditions` | `PluginConditionHost`, the engine's `PluginConditions`: extra instances that only run `condition:` handlers, asked synchronously while `farm-sim` evaluates conditions. |
| Scripts | `farm_plugins::scripts` | Project scripts (#241): trusted specs (`script/<id>`), their budgets from `settings.scripts`, never struck off. |
| The host | `farm_plugins::host` | One isolated instance per plugin: dispatching hooks, budgets, strikes and restarts. |
| Specs | `farm_plugins::spec` | Which plugins a game runs, with their granted hooks, mutations and minigame kinds, from the cartridge or project. |
| Payloads | `farm_plugins::payload` | Hook payload JSON in a fixed key order, numbers and strings written as JavaScript's `JSON.stringify` writes them, so every host gives a plugin the same text. |
| Mutations | `farm_plugins::mutations`, `capabilities` | Parsing and validating answers, namespacing ids, and checking them against the granted capabilities. |
| The queue | `farm_plugins::queue` | Orders accepted mutations; they are drained right before the next tick as `Command::PluginMutation`. |
| The runtime | `farm_plugins::runtime` | Wires a host to a `PlaySession`: hooks from the session's effects, mutations back as commands, plugin errors out. |
| Minigames | `farm_plugins::minigame` | Pack minigame sessions (`PackMinigame`), each in a fresh instance granted only that minigame. |

Fuel is the guest's own: the meter compiled a counter into the guest (the
exported `fp_fuel` global, one unit per executed wasm instruction), so
budgets mean the same on every wasm engine. wasmi's built-in fuel is off; it
charges a loop's whole body up front, which overcharges QuickJS's interpreter
loop by a factor of a few hundred.

The wasm engine sits behind the two traits, so a wasmtime backend (JIT or its
Pulley interpreter) could be added behind a feature without touching the
rest. wasmi is a pure-Rust interpreter with no JIT, so the sandbox runs where
JITs are forbidden (consoles, iOS) and inside the web player.

## Hardening

- **wasmi's tail-call dispatch.** wasmi runs each WebAssembly instruction
  with a tail call that LLVM turns into a jump at opt-level 2 and above. If a
  build broke that (wasmi below opt-level 2, debug assertions in wasmi,
  sanitizers), every instruction would grow the native stack and a plugin
  using its whole budget would crash the process, editor included. The root
  `Cargo.toml` turns off wasmi's debug assertions in every profile, and the
  `farm-plugins` test
  `a_plugin_running_its_whole_budget_fits_in_a_small_native_stack` runs a
  full 50-million-instruction budget on a 512 KiB thread, so a broken build
  fails CI instead of crashing players. Builds that cannot keep those
  settings turn on `farm-plugins`' `portable-dispatch` feature (wasmi's loop
  dispatch: slower, never stack-bound).
- **WASI stubs.** The guest imports only inert `wasi_snapshot_preview1`
  stubs. `random_get` checks the range against linear memory before it
  allocates anything (the current guest does not import it).
- **Host calls are synchronous and borrow nothing.** `farm.host_call` reads the request
  out of linear memory and returns a wasmi host error; the call is resumable
  (`call_resumable`), so the protocol loop in `sandbox::run_resumable` answers the request
  with whatever the caller holds (the game state of the step a hook fired at, borrowed for
  the length of the dispatch) and resumes the guest with the answer's length;
  `farm.host_answer` copies the answer in. The store never owns game state. Each answer's
  cost is taken from the guest's own fuel counter, so a read that makes the counter go
  negative stops the call at its next fuel check like any overrun. Requests are at most
  64 KiB (the guest refuses longer ones before calling, the host checks again). A
  wasmtime backend would implement `GuestInstance::call_with` with its own suspension
  (or by holding the borrowed view for the call).
- **Trusted scripts register any hook.** A granted-hooks list containing `"*"` lets a
  project script's `api.on` register any hook (ABI 2); pack plugins never get it.
- **The API is a convenience, not a boundary.** `api.minigame` (and the other
  functions of `prelude.js`) register handlers through the guest's own
  `api.on`: a minigame is the handler of the hook `minigame:<name>`. Only a minigame
  session's instance is granted that hook, so the hook handlers' instance
  never registers one, and one guest call per frame runs `update` and `draw`.
  Everything the plugin answers is checked on the host side
  (`farm_runtime::canvas::parse_canvas`, the state and score rules), so the
  prelude is a convenience, not a boundary: a plugin that misuses it only
  breaks its own minigame. The prelude captures its helpers when the
  sandbox is prepared, before any plugin code runs.
- **Output sizes.** The host never copies more than 1 MiB of an answer or
  1 KB of an error text out of the guest, however large the plugin makes
  them. The guest bounds a thrown message before it crosses the guest
  boundary, too.

## Rebuilding the guest

The guest changes rarely. Rebuild it only when `crates/farm-plugin-guest`,
`tools/plugin-guest/meter` or `tools/plugin-guest/build.sh` changes:

```sh
tools/plugin-guest/build.sh          # rebuild, then record the new hashes in crates/farm-plugins/guest/README.md
tools/plugin-guest/build.sh --check  # what CI runs: rebuild and compare byte for byte
```

Two builds from the same inputs give the same bytes; the `farm-plugins` tests
fail until the recorded hashes match the file and its inputs.

## Tests

`crates/farm-plugins/tests` cover hooks and payloads, mutation validation and
capabilities, budgets, strikes and restarts, saved data, minigame sessions
(frames, input, sounds, failures), the stack test above, the plugin systems
(`systems.rs`: reads, conditions, outcomes, entities, timers, item use, project
scripts) and the TypeScript typings (`typings.rs`); `farm-fuzz`
fuzzes plugin mutations; `crates/farm-player/tests/player.rs` runs plugins and
a pack minigame in a real player; and the example packs in
`fixtures/projects` (`demo-mod.json`, `county-fair-mod.json`,
`lantern-night-mod.json`) run in the F# and Rust parity tests.
