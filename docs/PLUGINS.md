# Pack plugins

A content pack can ship **plugins**: small JavaScript programs that react to
what happens in the game. A plugin registers handlers with
`api.on(hook, fn)`; a handler gets the hook's payload as plain JSON and
answers with a list of **mutations** such as
`{ type: 'giveMoney', amount: 10 }`. Plugins never touch the game state
themselves. Each answer is checked and then runs as a `pluginMutation`
command, so it enters the command log like anything the player does, and a
replay of that log needs no plugins. A plugin can also define its own
minigame kinds, which it runs and draws frame by frame
([Minigames](#minigames)).

Plugins run in `farm-plugins`: QuickJS compiled to WebAssembly, one isolated
instance per plugin, run by the wasmi interpreter with deterministic budgets.
The same sandbox runs in the editor's Play Mode, in exported games and in the
web player (`farm-wasm`).

```json
{
  "manifest": {
    "id": "glow-farm", "name": "Glow Farm", "version": "1.0.0",
    "permissions": {
      "hooks": ["onDayStart"],
      "mutations": ["message", "giveItem"]
    }
  },
  "plugins": [{
    "id": "morning-hum",
    "hooks": ["onDayStart"],
    "source": "api.on('onDayStart', p => [{ type: 'message', text: 'Day ' + p.day }, { type: 'giveItem', itemId: 'seed-glowshroom', quantity: 1 }])"
  }]
}
```

## Permissions

The manifest's `permissions` are what the player approves when installing
the pack. The Mods tab's install review shows them.

| Permission | Default | What it allows |
|---|---|---|
| `hooks` | none | The hooks the pack's plugins may hear. A plugin hears the hooks in its own `hooks` list that the manifest also lists. |
| `mutations` | see below | What the plugins may answer with. |
| `contentInject` | `true` | Whether the pack's content (definitions, `playerStart`, string tables) loads. When it is `false`, nothing of it loads, the Problems panel warns if the pack has content, and its plugins still run. |
| `uiPanels` | `false` | Whether the pack's game panels (`content.gamePanels`) show on the game screen, after the game's own ([Game panels](#game-panels)). Without it they are not shown, and the Problems panel warns if the pack has some. It does not depend on `contentInject`. |
| `minigames` | none | The minigame kinds the pack's plugins may define ([Minigames](#minigames)). A plugin defines the kinds in its own `minigames` list that the manifest also lists. |

### Mutation capabilities

Each entry of `permissions.mutations` allows one mutation type:

| Entry | Allows |
|---|---|
| `"message"`, `"playSound"` | Messages and sounds. |
| `"giveMoney"`, `"takeMoney"`, `"modifyEnergy"`, `"grantXp"`, `"setWeather"` | That change, with any value the schema allows. |
| `"setFlag"`, `"giveItem"`, `"takeItem"`, `"modifyFriendship"`, `"startQuest"`, `"warpPlayer"`, `"startDialogue"`, `"performAction"`, `"startMinigame"` | That change, for the pack's **own** content only (see below). `"giveItem:own"` means the same. |
| `"moveNpc"`, `"startCutscene"`, `"setVariable"`, `"addVariable"`, `"addShopStock"`, `"removeShopStock"`, `"placeNode"`, `"removeNode"`, `"placeMachine"`, `"removeMachine"`, `"setRelationship"`, `"sendMail"` | The event outcome of that name ([Outcome mutations](#outcome-mutations)), for the pack's own content only, like the row above. |
| `"modifyHealth"` | The `modifyHealth` outcome, with any value the schema allows. |
| `"giveItem:any"` (any of the types above with `:any`) | That change for any id: your game's content, base content and other packs. |
| `"*"` | Everything, with any ids. |

**Own content.** For the types that name content, a plain id means the
plugin's own pack: `setFlag` with `flag: 'met'` from pack `glow-farm` sets
the flag `glow-farm:met`, and `giveItem` with `itemId: 'seed'` gives
`glow-farm:seed`. Ids already written as `glow-farm:…` are kept. Ids of
other packs (`other-pack:seed`) are refused unless the pack has `type:any`;
so is your game's own content, because a plain id always means the pack's. With `:any`, ids are used exactly as
written. A base pack's own definitions keep plain ids, so its plugins need
`:any` to name them.

**Without `permissions.mutations`** (packs written before it existed) the
plugins get `message`, `playSound`, `setFlag`, `giveItem` and `takeItem` for
their own content, and their answers to `onEffect` and `onCommand` are
dropped: those hooks are for watching unless the pack declares what its
plugins do.

**Never allowed:** flags (and variables, which are flags) starting with
`event:`. The engine keeps one-shot events there (`event:<id>:fired`), so a
plugin could re-arm or switch off the game's events.

**Always allowed:** `saveData` ([Saved data](#saved-data)): it keeps only the
plugin's own data.

### Outcome mutations

These run the event outcome of the same name (Creator Guide, [Event
logic](CREATOR-GUIDE.md#event-logic)), with its meaning: an unknown id does
nothing, a node or machine goes only on a free tile, and so on. Coordinates
are tiles of the largest scene (0 to 511); a `sceneId` left out means the
scene the NPC is in (`moveNpc`) or the player's (the tile mutations).

| Mutation | Fields |
|---|---|
| `moveNpc` | `npcId`, `sceneId` (optional), `x`, `y` |
| `startCutscene` | `cutsceneId` |
| `setVariable` | `variable`, `value`: a number (at most 1e9 either way) or a string (at most 4096 characters). `true`/`false` is `setFlag`. |
| `addVariable` | `variable`, `amount`: an integer, -1,000,000 to 1,000,000 |
| `addShopStock` | `shopId`, `itemId`, `price` (optional, whole gold, 0 to 1,000,000; absent: the item's price), `dailyLimit` (optional, 1 to 999) |
| `removeShopStock` | `shopId`, `itemId` |
| `placeNode` / `removeNode` | `sceneId` (optional), `x`, `y`; `nodeTypeId` for `placeNode` |
| `placeMachine` / `removeMachine` | `sceneId` (optional), `x`, `y`; `machineTypeId` for `placeMachine` |
| `setRelationship` | `npcId`, `status`: `single`, `dating`, `engaged` or `married` |
| `sendMail` | `mailId`: the letter arrives the next morning |
| `modifyHealth` | `delta`: an integer, -1000 to 1000 (with combat on) |

Variables count toward the plugin's 256 different flags per session.

## Hooks

A plugin hears the hooks in its `hooks` list that the manifest's
`permissions.hooks` also lists. Payloads are plain JSON objects, keys in the
order below.

| Hook | When | Payload |
|---|---|---|
| `onDayStart`, `onDayEnd` | Each morning; each night before it | `{ day, season, year }` |
| `onSeasonChange` | A new season | `{ season, previousSeason, year }` |
| `onYearStart` | A new year | `{ year }` |
| `onTimeOfDay` | The clock reaches a full hour (the hours past midnight count on: 24, 25, …) | `{ hour, day }` |
| `onSceneEnter` | The player is in another scene than before the step: a door, a warp, the mine, fainting, sleeping, an outcome | `{ sceneId, fromSceneId }` |
| `onCropHarvest` | A crop was harvested | `{ cropType, quantity, quality }` |
| `onItemSell` | Items sold at a shop, or from a shipping bin overnight (one per item and quality) | `{ itemId, quantity, money, via, shopId }`: `via` is `shop` (with `shopId`) or `shipping` |
| `onRecipeCraft` | A recipe was crafted | `{ recipeId }` |
| `onFishCatch` | A fish was caught | `{ itemId, fishTableId }` |
| `onResourceGather` | A node broke and dropped items | `{ nodeTypeId, drops: [{ itemId, quantity }] }` |
| `onNPCInteract`, `onGiftGiven`, `onRelationshipChange` | Talking to an NPC, a gift, a friendship change | `{ npcId }`; `{ npcId, itemId, reaction }`; `{ npcId, friendship }` |
| `onBuild` | A building was finished or upgraded | `{ buildingId, sceneId }` |
| `onBundleComplete` | A bundle was completed | `{ bundleId }` |
| `onAchievementUnlock` | An achievement was unlocked | `{ achievementId }` |
| `onMonsterSlay` | A monster was slain | `{ monsterId, sceneId }` |
| `onAction`, `onMinigameResolve` | A creator action ran; a minigame ended | `{ actionId }`; `{ minigameId, score }` |
| `onWeatherRoll` | The night rolled the weather ([Timing](#timing)) | `{ weatherId, day }` |
| `onCommand`, `onEffect` | Every player command; every effect of a step | `{ commandType }`; `{ effectType }` |
| `onLoad` | The game started or was loaded with the plugin attached ([Saved data](#saved-data)) | `{ data }` |

## Saved data

A plugin can run a system of its own across saves: its JavaScript
variables start over whenever the game loads, but what it answers with
`saveData` is kept in the save.

```js
var visits = 0;
api.on('onLoad', p => { visits = p.data ? p.data.visits : 0; return []; });
api.on('onSceneEnter', p => {
  visits++;
  return [{ type: 'saveData', data: { visits: visits } }, { type: 'message', text: 'Visit ' + visits }];
});
```

- `{ type: 'saveData', data }` keeps `data` (any JSON, at most 16 KiB as
  compact text) as the plugin's saved data, replacing what it saved before.
  It needs no capability: it touches nothing but the plugin's own data.
- `onLoad` is heard by each plugin alone, with `{ data }`: what it saved in
  this game, or `null`. It comes when a new game starts and when a save
  loads (and when Play Mode starts a playtest, whose **Keep changes**
  carries the data into the next one), before any other hook.
  Answers to it apply before the first tick like any other. A plugin rebuilt
  after a strike (its JavaScript state starts over) hears it again before
  the next tick, with what it has saved up to then.
- The saved data is part of the game state, so it is in the command log and
  replays the same; a plugin's data stays when its pack is turned off and
  comes back with it.

## Game panels

A pack with `permissions.uiPanels` can add game panels, the same panels the
editor's Interface tab makes: a title, an optional flag that must be set
before the panel shows, and entries (`text`, `money`, `energy`, `day`, an
`item` count, a `flag` or an `action` button).

```json
"content": {
  "gamePanels": [{
    "id": "fair-board", "title": "County Fair",
    "entries": [
      { "label": "Ribbons", "kind": "item", "value": "ribbon" },
      { "label": "Barn raised", "kind": "flag", "value": "county-fair:barn-raised" }
    ]
  }]
}
```

Panel ids are namespaced (`county-fair:fair-board`), and so are the items and
actions entries name when the pack defines them. Flags are named as written:
a plugin's own flags are `packId:flag`, so a panel can show what the pack's
plugins keep there. The game shows the panels of its enabled packs, in load
order, after its own. [fixtures/projects/county-fair-mod.json](../fixtures/projects/county-fair-mod.json)
is an example pack with a panel and the building, goal and combat content packs
can add.

A refused or malformed mutation is dropped; the others in the same answer
still run. The plugin errors list (the editor's Play Mode drawer, the game's
error toasts) says why. Unknown entries in `permissions.mutations` are
ignored and the Problems panel warns about them.

## Timing

- **Answers apply before the next tick.** A handler runs right after the step
  that fired its hook (a tick, a player command, another plugin's mutation).
  Its mutations wait in a queue and run right before the next tick, in the
  order they arrived. The game advances one tick at a time, so this does not
  depend on the frame rate: the same input gives the same game at 30 and at
  144 frames per second.
- **Chains stop.** A step caused by a plugin mutation does not fire
  `onCommand` or `onEffect`, and mutations three reactions deep are dropped
  (a plugin answering a mutation's hook, another answering that, and so on).
  A plugin cannot make the game loop on itself.
- **`onWeatherRoll` is a notification.** The engine lets the weather roll be
  overridden while the night runs, but plugins answer only after the step.
  A `setWeather` answer changes the weather for the day, yet the night's rain
  watering and storm damage already used the rolled weather. The Problems
  panel warns about plugins that listen to `onWeatherRoll`.

## Minigames

A pack can bring its own minigame kinds. A plugin defines one with
`api.minigame(name, { start, update, draw })`; the game runs it while a
minigame of that kind is open, one `update` and one `draw` per frame, and
draws what `draw` answers inside the minigame window. `update` reads the
game keys and the mouse or touch over the window, and can ask for sounds.
When `update` says the game is done, its score resolves the minigame exactly
like a built-in kind's: the def's result tiers give the rewards, and only the
score enters the command log, so replays need no plugins.

```json
{
  "manifest": {
    "id": "lantern-night", "name": "Lantern Night", "version": "1.0.0",
    "permissions": { "minigames": ["lantern-catch"] }
  },
  "content": {
    "minigames": [{
      "id": "catch", "name": "Lantern Catch", "kind": "lantern-night:lantern-catch",
      "config": { "lanterns": 6 },
      "resultTiers": [
        { "minScore": 0, "outcomes": [{ "type": "message", "message": "The lanterns drift away." }] },
        { "minScore": 1, "outcomes": [{ "type": "giveItem", "itemId": "lantern", "itemQuantity": 3 }] }
      ]
    }]
  },
  "plugins": [{
    "id": "lantern-catch",
    "minigames": ["lantern-catch"],
    "source": "api.minigame('lantern-catch', { start: (config, info) => ({ x: info.width / 2, caught: 0, t: 0 }), update: (s, input, dt) => { if (input.left) s.x -= 200 * dt; if (input.right) s.x += 200 * dt; s.t += dt; return s.t > 10 ? { state: s, done: { score: s.caught / 6 } } : { state: s }; }, draw: s => [{ type: 'rect', x: 0, y: 0, w: 400, h: 240, color: '#1b2140' }, { type: 'rect', x: s.x - 40, y: 204, w: 80, h: 16, radius: 5, color: 'primary' }, { type: 'text', x: 12, y: 26, text: 'Caught ' + s.caught, color: '#ffe9a8' }] });"
  }]
}
```

The complete example, with falling lanterns and item art, is
[fixtures/projects/lantern-night-mod.json](../fixtures/projects/lantern-night-mod.json).

**Kinds.** A kind is `packId:name`, and its name follows the pack id rule
(lowercase letters, digits and dashes). The plugin's `minigames` and the
manifest's `permissions.minigames` list the name (`packId:name` works too);
only kinds in both are defined, and only while the pack is enabled. Any
minigame def, the game's own or a pack's, can use the kind. The editor's
kind list offers the kinds of the enabled packs, and Problems warns when a
def uses a kind whose pack is not installed, turned off, lacks the
permission or has no plugin defining it (the def then plays as the neutral
"Go!" button, like any unknown kind).

**The functions.** The game keeps the state; the functions get it and answer
a new one:

| Function | Gets | Answers |
|---|---|---|
| `start(config, info)` | The def's `config`; `info` is `{ width: 400, height: 240, seed }`. | The first state (any JSON). Optional: without it the state starts as `null`. |
| `update(state, input, dt)` | The state, the player's input this frame, the frame time in seconds (at most 0.25). | `{ state, done, sounds }`: the new state (the old one stays when `state` is left out); `done: { score }` (0 to 1) or `done: { success }` (`true` scores 1, `false` 0) to finish; `sounds`, a list of sound ids to play this frame. |
| `draw(state)` | The state `update` just answered. | The frame: a list of draw commands. |

`input` is `{ up, down, left, right, action, actionPressed, actionReleased,
pointer }`. The first seven are `true` or `false`, from the game's input
bindings after rebinding: the move keys, arrows, left stick and D-pad, and
Interact (E, Space, Enter, gamepad A, the touch controls' A) as the action.
Escape and the window's Give up button cancel the minigame as usual.

`pointer` is the mouse or a finger over the minigame's canvas:
`{ x, y, down, pressed, released }`, with `x` and `y` in the same
coordinates `draw` uses (0–400, 0–240), `down` while the button or finger is
down, and `pressed` / `released` when it went down or up this frame. A tap
within one frame is both. It is `null` while the pointer is elsewhere (and
on a gamepad or keyboard). A press outside the canvas never reaches the
minigame. One that went down on it is reported until it is let go, wherever
it moves: off the canvas, it stays on the canvas's edge. The game reports the pointer once per frame, so
`update` sees it one frame after the window drew under it.

```js
api.minigame('pop-the-lantern', {
  start: () => ({ x: 200, y: 120, popped: 0 }),
  update: (s, input) => {
    var p = input.pointer;
    if (p && p.pressed && Math.abs(p.x - s.x) < 20 && Math.abs(p.y - s.y) < 20) {
      s.popped++;
      return s.popped >= 5 ? { state: s, done: { success: true }, sounds: ['quest'] } : { state: s, sounds: ['coin'] };
    }
    return { state: s };
  },
  draw: s => [{ type: 'rect', x: s.x - 20, y: s.y - 20, w: 40, h: 40, radius: 20, color: 'accent' }],
});
```

**Sounds** in `sounds` play exactly like a `playSound` outcome's sound id
(the built-in cues `ui`, `coin`, `harvest`, `quest`, `success`, `error`, …,
and any other id `playSound` can play), at the player's sound-effect
volume. They are cosmetic, like the frames: they need no mutation permission
and never enter the command log. An id the game does not know plays nothing.

**Draw commands** use minigame coordinates: the canvas is 400 × 240, origin
top left, `y` down, scaled into the window and clipped to it.

| Command | Fields |
|---|---|
| `{ type: 'rect', x, y, w, h, color, radius, outline }` | A filled rectangle with rounded corners of `radius`; `outline` draws a stroke of that width instead. |
| `{ type: 'text', x, y, text, size, color, align, bold }` | One line of text; `y` is its baseline, `align` is `left`, `center` or `right` of `x`, `size` 6–64 (default 14). |
| `{ type: 'sprite', x, y, w, h, item }` | An item's art fitted into the rectangle. A plain id means the pack's own item (`packId:item`) when there is one, else the id as written. |

Colors are `#rgb`, `#rrggbb`, `#rrggbbaa` or a theme color (`text`,
`muted`, `primary`, `accent`, `track`, `panel`, `background`, `error`), so a
minigame can match the game's interface. Unknown types and keys, a wrong
value or a list over the limits is an error, not ignored.

**Failing.** A minigame whose plugin throws, runs out of fuel or memory,
answers something malformed or breaks a limit ends at once as failed: it
resolves with score 0 (the lowest tier applies), and the error shows in the
plugin errors list and as an error toast, for example
`lantern-night:lantern-catch: [threw] minigame lantern-night:lantern-catch failed: threw in update: s.lantern is undefined`.
The game itself never stops or slows past one frame's budget.

**Isolation.** Each minigame session gets its own sandbox instance of the
plugin, started from its source and granted only that minigame. Its
JavaScript variables start over for every session and never mix with the
plugin's hook handlers, which run in their own instance; the hook budgets,
strikes and windows do not count minigame frames. The plugin's top-level code
runs in both instances, so keep it to registering handlers.

## Limits

Budgets are counted in **fuel**: one unit per WebAssembly instruction the
plugin's JavaScript engine runs (about 0.5–1.3 billion a second in a release
build). They never depend on the machine.

| Limit | Default | When it is hit |
|---|---|---|
| Fuel per handler call | 50 million (40–110 ms) | The call stops; a strike. |
| Fuel per plugin per second of game time | 200 million | The plugin is skipped until the next second; a strike. |
| JavaScript heap / linear memory per plugin | 16 MB / 40 MiB | The call stops; a strike. |
| Recursion | a few thousand JavaScript calls deep | The call stops; a strike. |
| Strikes | 3 within 100 calls of the plugin | The plugin is off for the session. |
| Restarts after a strike | 10 per session | The plugin is off for the session. |
| Startup fuel per plugin / all plugins | 500 million / 2 billion | The plugin does not start. |
| Plugins | 32 | Later plugins do not start. |
| Memory of all plugins at startup | 512 MiB | Later plugins do not start. |
| Mutations per answer | 64 | The rest are dropped. |
| Ids, flag names and skills | 256 characters | The mutation is dropped. |
| String flag values / messages | 4096 / 500 characters | The mutation is dropped. |
| Different flags / skills per plugin per session | 256 / 32 | Further new ones are dropped. |
| An answer's size | 1 MiB | The answer is dropped (like a throw). |
| An error message | 1 KB | The rest is cut off. |
| Minigame start (compile, `start`, first `draw`) | 200 million fuel | The minigame fails. |
| Minigame frame (`update` and `draw`) | 10 million fuel (8–20 ms) | The minigame fails. |
| Minigame state | 16 KiB as JSON | The minigame fails. |
| Draw commands per frame | 256 | The minigame fails. |
| Text per draw command / per frame | 200 / 4000 characters | The minigame fails. |
| Minigame answer (`update` and `draw` together) | 1 MiB | The minigame fails. |
| Sounds per `update` | 8, each id at most 256 characters | The minigame fails. |

A handler that throws gives no mutations and is not a strike, but the fuel it
burned counts toward its per-second budget. After a strike the plugin is
rebuilt from its source (its JavaScript variables start over) with at most
twice the fuel its first start used.

## Determinism

- Budgets and timeouts are counted in instructions, so whether a handler
  finishes is the same on every machine and in every build.
- `Date.now()` is always 0, and `Math.random()` gives the same sequence in
  every plugin and every run.
- A minigame session depends only on its start and its input: the same
  config, seed and input (keys and pointer) give the same states, frames,
  sounds and score on every machine (its `Math.random()` starts the same way in every session; use
  `info.seed` for variety). The seed comes from the game's cosmetic
  randomness, and only the score enters the command log.
- Passing the state into the sandbox and back costs about 260 fuel per byte
  each frame, so a 16 KiB state uses about 4 of a frame's 10 million. Keep
  states small.
- Plugin JavaScript state (variables kept between calls) is not saved. It
  starts over when the game loads, when Play Mode restarts, and when the
  plugin is rebuilt after a strike, so a plugin's answers can depend on the
  session's history; what must last goes in `saveData` and comes back in
  `onLoad` ([Saved data](#saved-data)), also after a rebuild: a plugin
  rebuilt after a strike hears `onLoad` again before the next tick, with
  the data it has saved so far. Replays are not affected: the command log, with the plugins'
  answers in it, is the replay.
- In the web player (`farm-wasm`) plugins run on the calling thread (a page
  that wants them off the main thread runs the whole player in a Worker), and
  the budgets above were measured natively: wasmi inside WebAssembly is
  slower, so the same fuel takes longer there.

## Sandbox hardening

Notes for engine developers (the 2026-09-30 audit's defence-in-depth items).

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
- **The minigame API needs no guest change.** `api.minigame` is a line of
  JavaScript (`farm_plugins::PRELUDE`) that runs before every plugin's
  source and registers the minigame as the handler of the hook
  `minigame:<name>` through the guest's own `api.on`. Only a minigame
  session's instance is granted that hook, so the hook handlers' instance
  never registers one, and one guest call per frame runs `update` and `draw`.
  Everything the plugin answers is checked on the host side
  (`farm_runtime::canvas::parse_canvas`, the state and score rules), so the
  prelude is a convenience, not a boundary: a plugin that rewrites it only
  breaks its own minigame.
- **Output sizes.** The host never copies more than 1 MiB of an answer or
  1 KB of an error text out of the guest, however large the plugin makes
  them. The guest itself still copies a thrown message in full into its own
  memory (within its 40 MiB); cutting it there too needs a guest rebuild
  (`tools/plugin-guest/build.sh`), which is left for the next guest change.

### Player progression mutations

Optional capabilities `setInventorySize`, `addInventorySize`, `addMaxEnergy`
and `addMaxHealth` take an integer `amount` between -1000 and 1000 and apply the
same configured limits as event outcomes. An occupied backpack slot is never
lost. `applyBuff` takes a `buff` record (`id`, `stat`, integer `amount`, positive
`durationMinutes`, optional `icon`, `persistsAfterSleep` and `additive`). The
payload is bounded to 2048 bytes, duration to 525600 minutes, amount to ±1000,
and at most 64 active buffs are retained. Reusing a buff id refreshes it.
