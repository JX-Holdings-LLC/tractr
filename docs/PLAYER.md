# The game player

Every game made with Tractr runs in the same program: the Tractr game player.
It is inside every exported Windows, Linux and web game, and it is what the
editor's Play Mode runs, so what you test is exactly what your players get.
This page describes the player from your players' side: its screens,
controls, settings, saves and what to do when something goes wrong. Share
the parts that help with your players.

## What players get

- **A title screen** with New Game, Continue, Load, Settings, Credits and
  Quit, over your game's first scene.
- **Three save slots**, each with a preview (farm name, day, season, year,
  money, play time and a picture of the scene). The game saves itself every
  morning, after the player sleeps.
- **A pause menu** (Esc): Resume, Save, Load, Settings, Quit to title and
  Quit. Closing the window during a game asks first.
- **Settings** for display, audio, controls, accessibility and language.
- **Keyboard, mouse and gamepad** everywhere, menus included, and touch
  controls in the web demo on phones and tablets.
- **Screenshots** with F12.

The interface fits the Steam Deck's 1280×800 screen as well as 16:9 screens,
and scales with the window.

## Controls

| Keyboard | Gamepad | Action |
|---|---|---|
| WASD / arrows | Left stick / D-pad | Move (diagonals too); move between buttons in menus |
| E / Space / Enter | A | Interact; press the focused button |
| Q · T · R · F · C | X · Y · LB · RB · LT | Watering can · Hoe · Axe · Pickaxe · Scythe |
| X | RT | Crafting |
| Z | L3 | Sleep |
| I | View | Inventory |
| J | R3 | Quests |
| G | | Goals: collections, bundles, achievements, the shipping log (games that have them) |
| M | | Mail (games that have letters) |
| Esc | B / Menu | Close a panel, dialogue or shop; open the pause menu |
| 1–9 | | Pick a dialogue option |
| Tab / Shift+Tab | RB / LB | Next / previous tab in the shop and settings |
| F11, Alt+Enter | | Fullscreen |
| F12 | | Screenshot |

The mouse clicks every button, and the wheel (or the right stick) scrolls
lists. The toolbar at the top has Inventory, Quests, Craft, Sleep and Menu,
and Goals and Mail (with an unread count) in games that use them; with combat
on, the HUD shows health beside energy. The hint row at the bottom shows the
keys of the device used last: keyboard keys, or gamepad buttons once a pad is
used.

A few things worth telling players:

- **Planting**: **Hold** on a seed or a fertilizer in the inventory picks what
  interacting with open soil uses (press it again to put it away). Without a
  held seed, the first seed that grows this season is planted; fertilizer is
  only used when one is held.
- **Chests and shipping bins** open like a shop: take from their slots,
  store from the inventory.
- **Building**: a builder's Build tab shows the farm with the building's
  footprint, green where it can go and red (with the reason) where it can't.
  Move it with the arrows, the stick or the pointer, and choose **Build
  here**.
- **Messages** stay on screen long enough to read; pointing at one holds it,
  clicking it dismisses it.

Keyboard keys can be rebound in **Settings → Controls**: press a key's
button, then the new key (Esc, gamepad B or Start cancel). A key already used
elsewhere moves to the new action. The gamepad layout is shown there too.

## Settings

| Section | What players can change |
|---|---|
| Display | Fullscreen (borderless) or windowed; integer scaling (crisp pixels in whole steps) or fit; pause when the window loses focus (on by default); interface size, 75–150% |
| Audio | Master, music and sound-effect volume, and mute |
| Controls | Keyboard bindings; the gamepad layout |
| Accessibility | Text size (90–140%), reduced motion (no floating pops, fades or flashes), a more readable font (Atkinson Hyperlegible) and the language |

The first time a game runs, fullscreen and scaling come from the game's
export settings. The language starts as the system's language when the game
has it, else the game's own; English and Spanish are built in, and cover
the game's menus and messages. Text you wrote in the game shows as you wrote
it.

**Music and sound.** Every game has Tractr's own day and night music,
birdsong, crickets and rain, plus the sounds and music the game brings. Music
and the ambience share the Music volume. Sound follows the default output
device: plug in headphones and the game moves to them.

**Text.** The game's fonts cover Latin, Greek and Cyrillic. Other scripts
(Chinese, Japanese, Korean, Arabic…) show as boxes.

## Saves

| System | Saves | Settings |
|---|---|---|
| Windows | `%APPDATA%\<company>\<gameId>\saves\` | `%APPDATA%\<company>\<gameId>\settings.toml` |
| Linux | `$XDG_DATA_HOME/<gameId>/saves/` (usually `~/.local/share/<gameId>/saves/`) | `$XDG_CONFIG_HOME/<gameId>/settings.toml` (usually `~/.config/<gameId>/`) |
| Web demo | The browser's storage for the page | The browser's storage for the page |

`<company>` and `<gameId>` come from the game's export settings; without a
company the Windows folders skip that level. `FARM_PLAYER_USER_DIR` puts
saves, settings and screenshots under one folder instead (for a portable
copy on a USB stick, say).

- **New Game** starts in the first free slot (or asks for one), and the game
  saves to that slot every morning. The pause menu saves to any slot (another
  game's slot asks first) and loads any slot. **Continue** loads the most
  recent save.
- **Saves are safe from crashes.** A save is written to a temporary file and
  renamed into place, so a crash or power loss never leaves half a save. The
  previous save of a slot is kept as `slot<N>.bak`: if a slot's save is
  missing or broken, the slot list offers the backup and says so.
- **Saves survive updates.** A save from an older version of the game loads
  in a newer one; anything the newer version no longer has is kept aside,
  not lost. A save from a newer version loads with a warning. A save from
  another game is refused.

## Screenshots

F12 saves the frame on screen, interface included, as a PNG in a
`screenshots` folder beside the `saves` folder, named after the time
(`screenshot-2026-10-21_17-25-03.png`). A message says where it went.

## Crash logs

If the game stops with an error, the window says what happened and where the
crash log is, until it is closed or Esc or Enter is pressed. Logs are named
`crash-<time>-<pid>.log` and kept next to the saves (the newest ten). A log
holds the game and its version, where the player was and what they did last,
so a creator can replay the crash exactly: ask players to attach it, with
their last save, when they report a problem.

## Graphics

The desktop game draws with the graphics card (Vulkan or DirectX 12) when it
can, and with the processor otherwise; both show the same picture. If the
game shows nothing or the graphics card misbehaves, it switches to the
processor by itself. To choose, start the game with
`--renderer cpu` or `--renderer gpu` (or set `FARM_RENDERER`), or set
`renderer = "cpu"` under `[display]` in `settings.toml`.

## Command-line options

The player has options that help creators test, record and make store
pictures. The game program is `<Game>.exe` (Windows) or `<Game>` (Linux) in
an exported game.

| Option | What it does |
|---|---|
| `--cart <file>` | Play another cartridge (default: `game.cart` beside the program). |
| `--fullscreen`, `--windowed` | Start this way; the setting is saved. |
| `--renderer auto\|gpu\|cpu` | Choose how the game draws, for this run. |
| `--exit-after-frames <n>` | Close after `n` frames. |
| `--screenshot <out.png>` | Draw without a window and save the last frame. |
| `--size <W>x<H>` | Screenshot size (default 1280x800). |
| `--frames <n>` | Frames to run before the screenshot, 60 per second (default 60). |
| `--script <file>` | Scripted input for the screenshot run (below). |
| `--headless` | No window: load, replay, report (below). |
| `--replay <file>`, `--load <save>`, `--save <save>` | The replay to run, a save to start from, and where to save at the end. |

### Store pictures with `--screenshot`

A screenshot run always gives the same picture for the same arguments (it uses
temporary saves and settings and a fixed clock), so it is handy for store
pages at exact sizes. A script presses keys and buttons on given frames:

```json
{ "steps": [
  { "at": 2, "press": "enter" },
  { "at": 30, "events": [{ "type": "keyDown", "key": "d" }] },
  { "at": 60, "events": [{ "type": "keyUp", "key": "d" }] },
  { "at": 70, "button": "south" },
  { "at": 80, "click": [640, 400] }
] }
```

```sh
WillowCreek --screenshot title.png --size 1920x1080 --frames 90
WillowCreek --screenshot field.png --script walk.json --frames 120
```

`press`, `button` and `click` go down on their frame and up on the next.

### Checking a game with `--headless`

`--headless` plays a recorded list of inputs without a window and prints a
JSON report with the game id, version, content and state hashes. The same
game and the same inputs always end in the same state, on every machine, so
a build server can check that an exported game still plays the same:

```json
{
  "seed": "optional-seed",
  "autoStartQuests": false,
  "inputs": [
    { "kind": "tick", "ticks": 20 },
    { "kind": "command", "command": { "type": "sleep" } }
  ],
  "expectedHash": "optional-16-character-hash"
}
```

The player exits with an error for a broken cartridge, a save it can't load
or a hash that doesn't match `expectedHash`. `--save` writes a save at the
end (a path ending in `.json` writes it as JSON), and `--load` starts from
one.

How the player works inside (embedding, renderers, audio, performance) is in
[Player internals](dev/PLAYER-INTERNALS.md).
