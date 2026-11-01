# Project files

A Tractr project is plain JSON, so you can keep it in version control, review
it, edit it in a text editor or generate parts of it with your own tools. This
page describes the two layouts a project can be saved in, the JSON Schema that
describes the format, and a worked example. The [Creator guide](CREATOR-GUIDE.md)
explains what each part does in the game.

## Two layouts

The same project can be a **folder** or a **single file**. The editor opens
both; `farmc convert` turns one into the other without losing anything
([Export from the command line](EXPORT.md#export-from-the-command-line)):

```sh
dotnet run --project src/FarmEngine.Cli -- convert path/to/WillowCreek --out WillowCreek.json   # folder → file
dotnet run --project src/FarmEngine.Cli -- convert WillowCreek.json --out path/to/WillowCreek   # file → folder
```

### The folder

A folder is what the editor saves: one file per scene, NPC, dialogue, item and
asset, so a change touches only the files it changes and a team can merge
them.

```text
WillowCreek/
  project.json          everything else, with the big lists as file paths
  scenes/scene-farm.json
  npcs/npc-hazel.json
  dialogue/dialogue-hazel-hello.json
  items/crop-carrot.json
  assets/asset-hazel-happy.png
```

- `project.json` holds the project, except that `scenes`, `npcs`,
  `dialogues` and `items` are lists of file paths
  (`"scenes/scene-farm.json"`) in the project's order, and each custom asset
  has a `dataFile` (`"assets/hero.png"`) and a `dataType` (`"image/png"`)
  instead of its data. An entry written out in full in those lists is read as
  it is.
- A scene file stores its tiles as a `tileGrid`: a `palette` of the distinct
  tiles (without their `x` and `y`) and one `rows` text per row of palette
  indices. `n*i` is a run of `n` tiles of index `i`:

  ```json
  "tileGrid": {
    "palette": [
      { "type": "grass", "background": "grass", "overlay": null, "object": null, "collision": false, "soilMoisture": 0, "soilFertility": 0 },
      { "type": "path", "background": "grass", "overlay": "path", "object": null, "collision": false, "soilMoisture": 0, "soilFertility": 0 }
    ],
    "rows": [
      "12*0",
      "4*0 6*1 2*0"
    ]
  }
  ```

  A tile that names another spot than where it stands (a hand-edited file)
  keeps its `x` and `y` in the palette.
- Files are named after the ids, with characters a file name can't hold
  replaced by `_`. Only `project.json` and the files in these five folders
  are read; anything else in the folder (a README, `.git`) is left alone.

### The single file

A single-file project is the same JSON with everything inline: the lists hold
the scenes, NPCs, dialogues and items themselves, each scene has its `tiles`
expanded (`tiles[y][x]`, every tile with its `x` and `y`), and each asset
carries its data as a `dataUrl` (`data:image/png;base64,…`). It is the format
for sharing a project in one file and the one the JSON Schema describes.

## The JSON Schema

[`schemas/project.schema.json`](../schemas/project.schema.json) is a
[JSON Schema](https://json-schema.org) (draft 2020-12) of a single-file
project, generated from the editor's own schema, with every field's
description. Point your editor at it to get completion and checking while you
write a project by hand. In VS Code, for example, in `.vscode/settings.json`:

```json
{
  "json.schemas": [
    { "fileMatch": ["*.tractr.json", "my-game.json"], "url": "./schemas/project.schema.json" }
  ]
}
```

What the schema says:

- Every key is optional except `schemaVersion`: a missing key takes its
  `default`, the value the editor uses.
- Keys are camelCase, with a few that keep their capitals: `selectedNPCId`,
  `targetNPCId`.
- Fixed vocabularies (event triggers, cutscene step types, quest statuses,
  directions, emotes) are an `enum`. Vocabularies a game can extend (tile
  types, tools, crop qualities) and outcome types (content packs add their
  own) list the built-in values and accept others.
- Objects that keep keys the editor doesn't know accept them; the others
  refuse unknown keys, so a typo shows.

The schema checks the shape of a project. Whether its references hold up
(an event that starts a missing cutscene, a quest given by an NPC that
doesn't exist) is what Problems checks: run `farmc check`.

## A worked example

[`docs/examples/willow-lane`](examples/willow-lane) is a small project folder
with one of everything an event-driven story uses, and no Problems:

- two scenes (a lane and a cottage) joined by a door;
- an NPC, Hazel, with dialogue (one option opens the shop), a daily
  schedule, two daily lines, a portrait, a birthday and gift tastes;
- an event for each trigger, among them a not-repeatable `enter` event that
  greets the player the first time they walk into the lane;
- a cutscene with every kind of step (fades, music, camera, move, face,
  emote, lines, a choice that jumps to a label, a sound, outcomes that set a
  flag, a conditional jump, the end);
- a heart event that plays the cutscene at two hearts, a letter Hazel sends
  on the second morning, and a quest;
- the built-in item catalog the blank template starts with.

Copy it, open it in the editor, or check and convert it:

```sh
dotnet run --project src/FarmEngine.Cli -- check docs/examples/willow-lane
dotnet run --project src/FarmEngine.Cli -- convert docs/examples/willow-lane --out willow-lane.json
```
