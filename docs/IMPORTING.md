# Importing art and maps

Tractr reads sprite sheets made in [Aseprite](https://www.aseprite.org) and
maps made in [Tiled](https://www.mapeditor.org) or [LDtk](https://ldtk.io).
The file you drew stays the source: import it again after a change and the
asset or scene it made is updated in place, so everything that uses it keeps
working. The [Creator guide](CREATOR-GUIDE.md) shows where the buttons are;
this page lists what each file becomes.

## Aseprite sprite sheets

**Import Aseprite** in the Art tab takes either:

- a `.aseprite` (or `.ase`) file: its visible layers are drawn onto one sheet,
  frames left to right (a new row after 8192 pixels), with normal blending and
  the layer and cel opacity. RGBA, grayscale and indexed sprites are read;
  tilemap layers are left out (flatten them first) and other blend modes draw
  as Normal, with a note;
- the JSON of **File ▸ Export Sprite Sheet** (array or hash frames, with
  *Tags* and *Slices* ticked in its output options) and the PNG it names.
  Export without *Trim*, so every frame keeps its place.

What it becomes:

| In Aseprite | In Tractr |
|---|---|
| a tag | an animation clip of the tag's frames, in its direction (forward, reverse, ping-pong, ping-pong reverse); it loops unless the tag repeats a set number of times |
| a frame's duration | the frame's ticks: 20 a second, so 100 ms is 2 ticks (at least 1) |
| a slice | a clip of one frame: the slice's rectangle on the first frame (icons, portraits, a single sprite of a sheet) |
| no tags or slices | one clip, `idle`, of every frame |

Tag names become Tractr names: lowercase words joined by `-`, so
`walk_down`, `Walk Down` and `walkDown` are all `walk-down`, the name the
player and NPCs turn and walk with. The preview lists every tag and slice with
its clip; rename a clip there and the next import keeps the name.

Importing the same file again (the editor watches it and does this on its own
when the file changes) updates the same asset: its image, and the frames of
every clip it imported, in place. Clips you made by hand stay, a clip keeps
its **Loop** setting, a new tag adds a clip and a tag that is gone removes its
clip. The asset's id never changes, so the player, NPCs, items, crops,
terrains and everything else assigned to it keep pointing at it.

Images over 8192 pixels a side or 16 megapixels are refused, as for any art.

## Tiled and LDtk maps

**Import map** in Edit Mode's scene panel takes a Tiled map (`.tmx` or
`.tmj`, with its tilesets inline or in `.tsx`/`.tsj` files beside it) or an
LDtk project (`.ldtk`; choose the level when it has several). Maps must be
orthogonal and finite, at most 512 tiles a side. The map becomes a scene;
**Re-import map** reads it again over that scene.

### Tile layers

A tile layer is painted on the layer its `tractr` (or `layer`) property names:
`background`, `overlay`, `object`, `ignore`, or the id of one of the scene's
extra layers. Without the property, its name decides: Background, Ground,
Floor, Terrain or Base are the background; Overlay, Path, Paths or Roads the
overlay; Object, Objects, Walls or Buildings the object layer; any other
name is an extra layer, drawn above the player when its `above` property is
true or its name says Above or Roof. Without a background layer, the bottom
layer is the background. Hidden layers and groups are left out. LDtk IntGrid
layers paint the background.

### Tiles and terrains

Every tile becomes a terrain:

- a tile with a `terrain` property (LDtk: an enum tag, or `terrain=soil` in
  the tile's custom data) is that terrain: a built-in one (`soil`, `water`,
  `wall`…) or one of your game's, else a new one of that name;
- any other tile gets a terrain of its own (`tileset-name-7`), drawn with the
  tile's art from the tileset image;
- an LDtk IntGrid value is the terrain named like the value.

These tile properties set a terrain the import makes: `walkable` (or
`collision`), `plantable`, `water` (counts as water and can be fished),
`fishable`, `name` and `color`. Tiles on the object layer block the way
unless they say `walkable`. Flipped tiles are imported unflipped.

### Objects

An object (an LDtk entity) becomes a marker by its type (Tiled's *Class*, or
its name):

| Type | Properties | Becomes |
|---|---|---|
| `door`, `transition`, `warp`, `exit` | `to` (scene id), `toX`, `toY`, `locked` | a door on every tile it covers |
| `player`, `start`, `spawn` | — | where a new game starts |
| `npc` | `npc` (or the object's name) | the character's place, in this scene |
| `schedule`, `stop` | `npc`, `time` (`8:30`, `8:30pm` or minutes), `variant` | a schedule stop |
| `node` | `node` (a node type) | a tree, rock or other node |
| `machine` | `machine` (a machine type) | a machine |
| `light`, `lamp` | `radius` (1–16), `color`, `brightness` (1–100) | a light on the scene's `lights` layer |
| `event`, `trigger` | `event` (an event id), `trigger` (`enter` or `interact`) | the event's tile area (a new event when none has the id) |

### The report

Problems-style notes list what didn't map, with the file, the layer and the
tile: tiles without a terrain or art, objects of an unknown type, characters,
node or machine types the game doesn't have, doors without a destination.

### Re-import

The file stays authoritative: re-import replaces the scene's tiles, extra
layers, doors, nodes, machines, lights and the imported markers, and keeps
what play changed: crops, soil (tilled, watered, fertilized), dropped items,
a node's damage and a machine's contents when the same node or machine is
still there. The scene keeps its name, music, weather, monsters and the other
settings made in Tractr.
