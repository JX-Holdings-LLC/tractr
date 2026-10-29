# Creator guide: build a farm game without code

This guide walks through Tractr on the desktop: start a project, build
the world in **Edit Mode**, playtest in **Play Mode**, and export a game that
anyone can play. Everything happens in the editor; you never need to write
code. The editor shows this guide under **Help → Creator Guide** (F1).

## 1. Start a project

**File → New Project** (Ctrl+N) asks for a name and a template:

- **Starter Farm**: the full farming loop, with crops, a shop, quests and crafting.
- **Cozy Garden**: a cottage garden with a pond, beds already growing (one
  ready to pick), a merchant's stall and two hens; no energy or collapse,
  and slow days.
- **Quest RPG**: a story quest chain, gated dialogue and an elder NPC to
  remix; the farm's south gate leads to a village square with a fountain,
  where the elder and the lantern keeper wait.
- **Blank**: an empty scene and the default item catalog.

**File → Open Project** (Ctrl+O) lists your projects. They are saved
automatically in the app's data folder (`%APPDATA%\Tractr` on
Windows, `~/.config/Tractr` on Linux,
`~/Library/Application Support/Tractr` on macOS), one *project folder* each under
`projects/`: `project.json`, a file per scene, NPC, dialogue and item
(`scenes/`, `npcs/`, `dialogue/`, `items/`), and your art and sounds as
ordinary image and sound files in `assets/`. A big game opens quickly, and a
team can keep the folder in git and review and merge each other's changes:
a scene's tiles are one line of the map per row. A save writes only the
files that changed and leaves anything else you keep in the folder (a
README, `.git`) alone. A project from before 0.10 moves into its folder the
next time it is saved.

**File → Export Project JSON** writes the whole project as one file you can
keep anywhere or send to someone, and **Import Project JSON** brings one back
as a new project; it also takes the `project.json` of a project folder (a
teammate's checkout, say) and imports the whole folder. A project folder
copied into `projects/` shows up in Open Project by itself.

## 2. Build the world in Edit Mode

Press **F6** (or the mode button in the header) for Edit Mode. The editor has
eight tabs: Map, Content, Problems, Settings, Mods, Art, Workshop and Interface.
Undo and redo work everywhere: **Ctrl+Z** and **Ctrl+Y**.

### Map

Pick a scene in the **Scene** list above the map; zoom with **−**, **+** and
**Fit**. Hover a tile to inspect it. The map works from the keyboard too: click
it or Tab to it, move the gold editing cursor with the **arrow keys**, and
press **Enter** or **Space** to use the current tool there (Rectangle and
Select take two presses, one per corner; **Esc** cancels). While the map has
focus, letter keys pick the tools: `V` Inspect, `B` Brush, `R` Rectangle,
`G` Fill area, `I` Pick, `M` Select, `E` Erase layer, `X` Block, `U`
Unblock, `D` Door and `P` Player start. Screen readers announce the tile
under the cursor. Picking a brush is not an undo step.

The **Sky** box above the map previews the scene as the game draws it: tick
it, pick a time of day (**Morning**, **Noon**, **Evening** or **Night**), a
season and a weather, and the map shows the day/night tint, the foliage tint
and the weather overlay for them (the scene's own look first, then the
season's and the game's; an indoor scene has no weather). The rain and snow
stand still. It starts off so the map stays easy to read, and it is only a
view: it isn't saved and isn't an undo step.

Markers on the map show doors (`D`), where doors from other scenes arrive
(`A`), event triggers (`!`, with their region outlined), the mine entrance
(`M`) and the player start (`S`); the **Markers** box above the map hides
them. With **Inspect**, click a marker to open it: the door form, the event in
Content, the door an arrival comes from, or the mine settings. Wherever a form
asks for a tile (a door's arrival, the Workshop's tile, an event's tile, an NPC
schedule stop or waypoint, the mine entrance), **Pick on map** lets you click
it instead: the map shows the right scene, and **Esc** cancels.

The side panel holds the tools (drag its edge to make it wider):

- **Tile brush**: the terrain to paint (grass, soil, water, path, wall, door,
  floor, and your own tiles from the Art tab).
- **Map tools**: **Brush** paints one tile at a time (click and drag),
  **Rectangle** fills a rectangle, **Fill area** floods connected tiles,
  **Pick** takes the terrain under the cursor as the brush, **Select** marks
  an area for **Copy selection** and **Paste on map**, and **Erase layer**
  clears the chosen layer (Background, Overlay or Object, or one of the
  scene's extra layers, see [Bigger maps, layers and
  lights](#bigger-maps-layers-and-lights)). Choose the brush first: a brush
  picks its terrain's own layer, and the **Layer** list then switches it.
- **Block** and **Unblock** set where the player can walk. **Player start**
  moves the player's starting tile.
- **Place**: put an **NPC**, a gathering **Node** (trees, rocks, weeds), an
  **Item**, a **Machine** or an **Animal** on the map, and **Remove** things
  placed there.
- **Scene**: add a scene, rename, resize, duplicate or delete it, **Set as
  start**, **Fill scene** with one tile, **Clear crops/items** and **Reset soil**.
  **Indoor (no weather)** marks a greenhouse or interior: rain doesn't water its
  soil, storms don't damage its crops, and no rain or snow falls on screen.
  Scenes can be up to 512×512 tiles.
  Under the size boxes, the calculator shows the tile count and aspect ratio,
  and warns when a resize would cut tiles off. Shrinking moves the player
  start, NPCs (with their schedules and waypoints), animals, door arrivals,
  event tiles and the mine entrance that stood on the cut tiles inside, and
  removes doors leaving from them; the note lists each one first.
- **Transitions**: choose **Door**, click the departure tile, then pick the
  destination scene and tile. **Return door** adds the way back. The list
  below shows every door of the scene; **Duplicate** starts a copy of a door
  on the next free tile, ready to adjust and save.

### Content

The Content tab edits everything the game is made of. Pick a category, select
an entry (or click **New** to start one), change its fields and click **Save
changes**. Lists show each entry's art, and forms preview the art they point
to:

- **NPCs** and **Dialogue**: characters, where they stand, their daily
  schedules (a row per stop: the time of day, the scene and the tile), patrol
  waypoints and gift tastes, and dialogue trees whose options can open shops,
  offer quests, or depend on friendship, items and flags. In the game the
  dialogue box shows the NPC's art.
- **Items** and **Crops**: the item catalog (tools and their tiers, seeds,
  materials, gifts) and crops with growth days, seasons, regrowth and yields,
  on **Basic**, **Growth** and **Asset** tabs. A crop shows its profit per
  harvest and a summary card. The built-in crops are listed too: **Customize**
  one to change it (deleting your version brings the built-in back).
- **Quests**: objectives (harvest, collect, talk, visit, craft, gift, ship,
  slay, catch, hearts, bundle and mineFloor), rewards, prerequisites and
  availability. `deadlineDays` counts playable days including acceptance;
  `deadlineDay` is an inclusive absolute date. When both are set, the earlier
  date wins. Expired quests become **failed**, run `onFail` once, and appear
  in the quest log. A quest-status condition can check failure. Only repeatable
  quests may restart after failure.
- **Request boards**: place a board at a scene tile, paint an object to identify
  it, and add weighted quest templates. `intervalDays` is 1 for daily requests
  or 7 for weekly requests. Interacting accepts the period's request once.
  Optional item lists, quantity ranges and money ranges vary item objectives
  and rewards deterministically from the game seed. Accepted requests keep their
  rolled values across rotation and saving. A board template should have
  auto-start disabled; its requests are repeatable.
- **Mines**: add independently progressing mines, each with a name, entrance,
  floor dimensions, depth bands, elevator interval and optional music.
  The original mine remains in Settings and has the reserved id `mine`.
  Named mine entrances appear on the map and use the same interaction as the
  original. `mineFloor` conditions and quest objectives accept a mine id;
  leaving it empty selects the original mine. Progress and elevator checkpoints
  are saved separately for each mine.
- **Events**: triggers, conditions and outcomes, for story beats, flags and
  variables (see [Event logic](#event-logic) below).
- **Shops**: stock (a card per item with its price override, daily limit and
  the seasons it's sold in), prices and tool repair.
- **Recipes**, **Node types** and **Machine types**: hand crafting, the
  resources trees and rocks drop, and machines that process ingredients. A
  recipe's category groups it in the game's crafting menu, and the editor
  shows its profit per craft and per hour of machine time. The built-in node
  types (trees, rocks, weeds, mine rocks and ore) are listed and can be
  customized like crops. Players can pick an idle machine back up from the
  crafting menu; doors, door arrivals and the mine entrance stay clear of
  machines.
- **Animal species** and **Fish tables**: what animals produce, and which fish
  bite in each scene and season.
- **Actions** and **Minigames**: an action is a named bundle of effects (give
  an item, set a flag, show a message, start a minigame) that an item's
  **Use** button, a dialogue option, an event or a hotkey can run. A minigame
  is a short challenge whose score picks the reward. Its **Kind** is one of
  the games built into the player: **Timing bar**, **Hold to catch**,
  **Simple battle**, **Rhythm tap**, **Moving target** and **Memory
  sequence**, or a kind an enabled content pack defines (listed as
  `pack:name (from Pack Name)`; its plugin runs and draws the game, see
  [docs/PLUGINS.md](PLUGINS.md#minigames)). Each built-in kind's settings
  show as fields with their defaults (clear one to play the default); a
  pack kind's settings are passed to its plugin as written. A kind the game
  doesn't have plays as a single "Go!" button that scores 0.5, which
  Problems warns about (also when the pack of a pack kind is missing, turned
  off or lacks the permission), as it does about settings a built-in kind
  doesn't read or numbers outside their range.

- **Terrains**, **Tools**, **Item kinds**, **Crop qualities** and **Crop
  mutations**: the vocabulary of your game's world (see
  [Your game's world](#your-games-world) below). The built-in ones are listed
  and can be customized like crops.
- **Cutscenes**: scenes that play out over time (see
  [Cutscenes](#cutscenes) below).
- **Heart events**: cutscenes that play once at a number of hearts with an
  NPC (see [People and relationships](#people-and-relationships) below).
- **Buildings**: barns, coops, sheds, greenhouses and house upgrades a
  builder puts up (see [Building the farm](#building-the-farm) below).
- **Collections**, **Bundles**, **Achievements** and **Mail**: goals that
  last a whole game, and letters (see [Long-term goals](#long-term-goals)
  below).
- **Monsters**: what the player fights when combat is on (see
  [Combat and adventure](#combat-and-adventure) below).

Every nested field also has an **Edit as JSON** box, for pasting or bulk
changes.

The filter box above the list narrows it by name or id, and **Ctrl+K** (or
`Ctrl+P`) finds any entry, scene or art in the whole project. Fields you
haven't saved stay as typed when you switch tabs or undo something elsewhere;
choosing another entry asks whether to save them first, and they are saved
before another project opens or the editor closes. A number outside its range
is reported by name instead of being changed for you.

### Your game's world

A game doesn't have to be a farm on Earth. The ground, the tools, the kinds of
item, the crop quality tiers and the sky are all content, so a game on an
alien planet can have regolith instead of grass, a plasma cutter instead of an
axe, and twin suns instead of one. A new project starts with the classic
vocabulary; change any of it in the Content tab.

- **Terrains** are what a tile can be. Each one is painted on a layer (ground,
  path or object) and has its rules: whether the player can **walk** on it,
  whether crops can be **planted** on it (soil), whether it counts as **water**
  for crops that need water nearby, whether the fishing rod works on it,
  whether it **blocks light**, the **energy** a player loses stepping onto it
  (lava, acid), and whether it keeps the weather off like a greenhouse floor.
  Its **transforms** say which tool action turns it into what: the built-in
  grass and floor become soil when tilled. A transform can name one tool, or
  apply to every tool with that action. The map's palette lists your
  terrains with their colour (or their art, when you give them some).
- **Tools** have an **action**: water, till, harvest, fish, break, transform
  (apply a terrain's transform: a plasma cutter turning rock walls into
  floor), place (put down a terrain: a terraformer laying water) or plant (a
  seed drone sowing the seeds you hold). Every tool still strikes the
  gathering node it faces. A tool has its **key** in play, its energy cost
  (15% less per tier above the first), the **area** it covers at each tier (a
  row across the player's facing, and rows ahead) and the skill its node
  breaks train. An item is a tool when its kind is equippable and its
  **tool type** names the tool.
- **Item kinds** (the item's **type**) say what items of the kind do: planted
  (seeds), fertilize, equipped as a tool, **eaten** for energy (the item's own
  energy, or the kind's), placed, given as gifts, sold to shops.
- **Crop qualities** are the tiers a harvest comes out at, ranked in list
  order: each has its price, yield and mutation-chance multipliers, its stars
  and colour. Fertilized soil plants at the second tier, and every three
  farming levels raise a harvest one tier. **Crop mutations** are rare
  harvests that yield more; each takes its **share** of a crop's mutation
  chance, checked in list order (shares above a total of 1 never happen).
- **Growth requirements** (a crop's **Requirements**): the terrains it can be
  planted on, the scene tags it needs (`greenhouse`, `cave`), fertilizers it
  must be planted with, light or dark, a temperature range, and neighbours (a
  number of crops of a type, of a terrain, or of water within a radius). A
  crop whose light, temperature or neighbours aren't right on a night doesn't
  grow that night. Scenes have **dark**, **temperature** and **tags**;
  seasons have a temperature too (a scene's own wins). A place with no
  temperature suits every crop.
- **The sky**: in **Settings**, the day/night tint is a list of keyframes
  (a minute of the day and a colour the scene is multiplied by), with a
  foliage tint, for the whole game and per season; a scene can have its own.
  **Minutes per day** sets the length of a day (1440 is 24 hours): the clock,
  time-of-day conditions, machines and the tint follow it. Weather types pick
  an **overlay**: the built-in rain and snow, or your own (rain streaks, snow
  flakes, drifting dust, or just a tint) with its colour, density, speed and
  screen tint. Acid rain is a green rain overlay with a tint.
- **Words**: in **Settings**, how money is written (`{0} credits`), what
  energy is called (`Oxygen`) and the skills' names, with translations per
  language. The game's HUD, shops, inventory and quest log use them.

Problems lists a tile, item, node type or event that names a terrain, tool,
kind or tier the game doesn't have, a tool whose key moves the player or
opens a menu, and requirements that name what doesn't exist.

### Bigger maps, layers and lights

- *Size*: a scene can be up to 512 tiles a side (256 before 0.10). The
  game only draws and builds what the camera shows, so a big map costs
  little more than a small one to play.
- *Extra layers*: besides background, overlay and object, a scene can have
  its own layers. List them in the scene panel's **Extra layers (one per line)** box, one
  per line: an id (`rugs`), or an id and `above` (`treetops above`) for a
  layer drawn over the player and everything standing on the map (roofs, the
  tops of trees, the railings of a bridge you walk under). **Save
  surroundings** adds them to the map tools' **Layer** list, where you paint
  them with any terrain like the fixed layers. They are scenery: they never
  block the player and never hold crops. Removing a layer from the list
  removes what was painted on it (one undo step).
- *Autotiling*: tick a terrain's **Autotile** and it joins up with its
  neighbours of the same terrain. Without art, the game draws its edges
  where it meets another terrain, so a pond or a path has a shore or a
  border. With art, give the terrain's asset an animation clip named
  `autotile` with 16 frames: the game draws frame `n`, where `n` adds 1 when
  the tile above is the same terrain, 2 for the one to the right, 4 below
  and 8 to the left (frame 0 stands alone, frame 15 is surrounded).
- *Tall objects*: an object-layer terrain's **Tall** (0 to 8) is how many
  tiles it rises above its own tile: a tree, a lamp post, a high fence. It is
  drawn standing, in front of what is behind it and behind what is in front,
  so the player walks behind a tree's crown and in front of its trunk. Its
  tile still blocks the way its terrain says.
- *Lights*: terrains, node types, machine types, crops and building types
  can have a **Light**: a radius in tiles (1 to 16), a colour (empty is a
  warm lamp) and a brightness (1 to 100). At night the light lights the
  darkened scene back up, in its colour, so the grass under a lamp shows as
  it does by day: lamps and braziers (machines), glowing mushrooms (a crop,
  once grown), glowing crystals (a node, until broken), lava (a terrain,
  even on an extra layer) and a building's lit windows (once it stands). The
  darker the day/night tint, the more a light stands out; at noon it adds
  nothing.
  The Sky box above the map shows them at **Night**.

Problems lists a tile painted on a layer its scene doesn't have, a light
outside its ranges and two extra layers with one id.

### Sound and music

Import sound effects and music as **OGG** or **WAV** files in the Art tab
(as **Sound** or **Music** assets); they ship inside the exported game. The
`playSound` outcome plays a sound asset (or a built-in sound by name). Music
can be set per **scene**, **season**, **weather** (outdoors) and **festival**,
and the `playMusic` outcome (or a cutscene's music step) plays a music asset
until the player changes scene, however they get there (a door, a warp, a
mine ladder, a debug teleport). What plays, first match wins: the music an
event or cutscene chose, today's festival, the weather, the scene, the
season. With none of those, the built-in music plays as before.

In the Art tab's sound list, **Play** plays the selected sound or music once
(press it again to stop), through the same decoder the game uses. **Rename**
changes the name the editor shows; the asset's id stays, so everything that
plays it still does. Problems lists a file the game can't play (cut short,
damaged, Ogg Opus instead of Ogg Vorbis, a compressed WAV, longer than 10
minutes) as an error: the game would leave it out and play the built-in sound
or music in its place.

### Cutscenes

A cutscene is a list of steps that run one after another, over time. Start
one from an event, an action or a dialogue option with the `startCutscene`
outcome. While it plays the player watches: movement and other commands wait
until it ends, and the clock follows the **pause in modals** setting.

- **say**: a line, by an NPC, the player or a narrator; the player advances it.
- **choice**: a question with answers, each going to a step **label**.
- **jump**: goes to a label, when its conditions hold (flags, items,
  friendship, the same conditions events use).
- **move**: walks the player or an NPC to a tile (waiting for them to arrive,
  unless told not to); **face** turns them; **emote** shows a bubble over them
  (`exclaim`, `question`, `heart`, `music`, `anger`, `sweat`, `sleep`,
  `idea`).
- **wait**: pauses for some seconds.
- **camera**: pans to a tile or follows a character; a camera step with
  neither returns to the player. **fade**: fades the screen out to a colour,
  or back in.
- **music** changes the music; **sound** plays a sound.
- **outcomes** runs outcomes (give items, set flags, start quests).
- **end** stops the cutscene.

A skippable cutscene skips ahead when the player presses Escape: the
outcomes steps it passes still run, music and face steps apply, and moved
characters arrive where they were going. Skipping stops at a choice, so the
player still picks the branch. In play, Interact, Enter or Space advances a
line, the number keys pick an answer, and a click works too.

### Event logic

An event runs its outcomes when its **trigger** happens and all its
conditions hold. An event with a scene only fires while the player is in that
scene.

The triggers: entering a tile, interacting with one, time passing (`tick`),
the start of a day, a time of day (`time`, at the event's **Minute**), and
harvesting, selling, crafting, catching a fish, picking up an item, a
friendship change, a quest changing, a building finished (`build`), a bundle
completed, an achievement unlocked and a monster slain (`slay`). All but the
first five can name a **subject** (the crop or item, the NPC, the quest, the
building kind, the bundle, the achievement or the monster); without one the
event fires for any.

**Conditions** check tiles, items, flags, the day, the season, the year, the
time, quests, friendship and the weather, and also:

- **variable**: compares a variable (`eq`, `ne`, `lt`, `le`, `gt`, `ge`)
  with a number, or with a text for `eq` and `ne`.
- **all**, **any** and **not**: groups of conditions, nested as deep as you
  like.
- **chance**: holds with a probability (0 to 1). The roll comes from the
  game's seed and the moment, so a replay rolls the same.
- **scene**, **hearts** (at least and at most), `dayOfSeason` and
  **relationship** (single, dating, engaged, married).
- **money** (at least), **skillLevel**, **mineFloor** (the deepest floor
  reached), **building** (standing buildings of a kind, its upgrades
  counting), **collection** (items it holds, or all of them), **bundle**
  (complete), **achievement** (unlocked), **shipped** (units shipped of an
  item, or of everything), **slain** (monsters of a kind, or of any) and
  **birthday** (today is an NPC's birthday, or anyone's).

Variables are flags that hold a number or a text as well as true or false:
**setVariable** sets one, **addVariable** adds to a number (a missing one
counts as 0), and `{var:NAME}` shows one in any text.

**Outcomes** give and take items and money, set flags, change friendship and
energy, start quests, dialogues, cutscenes, minigames and actions, play
sounds and music, change tiles and warp the player, and also: **moveNpc**
(to a scene and tile), **setWeather**, **giveExperience** (in a skill),
**addShopStock** and **removeShopStock** (a shop keeps the change for the
rest of the game), **placeNode** and **removeNode**, **placeMachine** and
**removeMachine**, **setRelationship**, **sendMail** (a letter that arrives
the next morning) and **modifyHealth** (with combat on). An outcome that
triggers events of its own (a harvest, a sale) runs them too, four levels
deep at most.

### People and relationships

#### Daily lines

An NPC's **Lines** are what it says when the player talks to it, without a
dialogue tree for each day. A line has the events' conditions (the day, the
season, the weather, the scene, hearts, flags; an empty NPC in **hearts** or
**relationship** means the NPC itself) and a **weight**. Each day the NPC
picks one of the lines whose conditions hold, by weight, and says it in place
of its first dialogue's text, with that dialogue's options. An NPC with lines
and no dialogue just says the line.

#### Text substitution

Any dialogue, cutscene line, event or action message, quest and game panel
can name things that change: `{player}`, `{farm}`, `{spouse}`, `{npc:ID}`,
`{item:ID}`, `{var:NAME}`, `{season}`, `{day}`, `{year}`, `{date}`, `{time}`
and `{money}`. Anything else in braces stays as written.

#### Portraits

An NPC's **Portraits** give art per expression (`happy`, `sad`, any name). A
line, a dialogue or a cutscene line picks one by its **Expression**; without
one the dialogue box shows the NPC's own art.

#### Heart events

A heart event plays its cutscene once, when the player has the hearts with
the NPC and its conditions hold: on entering the event's scene, or,
without a scene, when the player talks to the NPC.

#### Relationships

Relationships are off until you tick **Relationships** in **Settings →
People** and save their rules:

- Friendship decay: points an NPC loses on a day the player neither
  talked to it nor gave it a gift.
- **Dating**: a **romanceable** NPC at the dating hearts accepts the dating
  gift (or, without one, starts dating by itself the next morning).
- **Marriage**: a dated NPC at the proposal hearts accepts the proposal
  gift; the wedding is some days later, with its own cutscene if you pick
  one. One partner at a time.
- The spouse moves to the home tile you choose and, if it helps, does
  one chore each morning: waters the farm's crops, feeds the animals or
  brings a gift.
- **Children** arrive every so many days of marriage, up to a number, with
  the names you list.

The **setRelationship** outcome sets a stage directly, and the
**relationship** condition checks one.

#### The player character

With **New games open the character creator (name, farm name, colours)**
ticked (Settings → People), a new game opens a screen where the player picks
a name, a farm name and the colours of skin, hair, shirt, trousers and an
accent, or **Random**. Items with a **Wear** slot (hat, shirt, trousers) are
clothes: the inventory's **Wear** button puts one on, and the player is drawn
with its colour or, for a hat, its art.

### Building the farm

#### Placed objects

A **machine type** is anything the player places on a tile, picks back up
and walks around (or through, without **Blocks movement**). Beside machines
that process recipes it can be:

- **Storage**: a chest with that many slots. Interacting opens it; items go in
  and out, and it can't be picked up until it is empty.
- **Shipping**: a shipping bin. What it holds is sold overnight at the item's
  value for its quality (see [Long-term goals](#long-term-goals)); only
  items of a sellable kind go in.
- **Gate**: interacting opens and closes it; an open gate never blocks. A
  fence is just a machine type that blocks movement.
- `areaEffects`: what it does to the square of tiles **radius** around
  it. **water** (a sprinkler) waters the soil there each morning; **protect**
  (a scarecrow) keeps storms and pests off the crops there overnight. Pests
  are off until the pest chance in **Settings → Farm** is above 0: each night an
  outdoor crop no object protects is eaten with that chance.
- Furniture and decoration: a machine type with none of these.

Placeable objects appear in the game's crafting menu with **Place** (an item
the type names is used up, as for machines).

#### Buildings

A **building** has a footprint (**Width** × **Height** tiles, up to 16), a
`door` in it (the middle of the bottom row by default) and an interior
scene (`interiorSceneId`): each building of the kind gets its own copy of that scene, entered
through the door; the interior's `interiorDoor` (the middle of its bottom
row by default) leads back out. It costs **Cost** money and its
**Materials**, takes `buildDays` nights to stand, may only go in its
**Scenes** (empty: the farm scene of **Settings → Farm**, else the start
scene), and houses **Capacity** animals.

A shop whose **Buildings** list names it is a builder: its **Build** tab
places a new building on a free spot (no blocked tile, crop, node, object,
item, door, NPC, animal or player under it) and shows each upgrade of a
standing building. A building with `upgradeOf` set is never built new: it
is what that building becomes, with its own interior, cost and nights; what
stood inside stays where the new floor has room. The player's house
upgrades the same way: add it under **Content → Placed buildings** (its kind,
scene, top-left tile, with **Pick on map**, and its authored interior
scene), and its upgrade replaces that scene's floor.

Tick **All seasons (greenhouse)** in a scene's panel (a greenhouse's
interior) and crops are planted and grow there in any season.

#### Animals and their buildings

An animal species lists the buildings that house it (an upgrade of one
of them counts too). A shop whose **Animals** list names it sells it into a
standing building that houses it and has room; the animal lives inside.
Beside its product, a species can have more **Products**, each on its
own interval and needing a mood. `moodQualities` give a product's quality
by the animal's mood (the best one it reaches: 90 for gold, say), and
**Breeding** gives adults in a building with room a chance each night of
young.

#### Farm layouts

**Settings → Farm** names the farm scene and lists **Layouts**: farm maps a
new game picks from. With two or more, a new game asks the player to choose
(with the character creator, or on its own screen), and the chosen layout's
scene takes the farm scene's place; doors and events that lead to the farm
lead to the chosen map.

### Long-term goals

- Shipping log: the game counts every unit its shipping bins sold, and
  shows last night's shipment.
- **Collections**: lists of items to find (the first time the player
  holds one), to ship, or to **donate** (handed in from the Goals screen,
  in the collection's **Scene** when it names one: a museum). **Rewards**
  are given once each, when the collection holds their **count** of items: a
  message and outcomes.
- **Bundles**: lines of items to hand in, a **quantity** of each, of a
  **quality** or better, in the bundle's **Scene** when it names one. A
  bundle is complete when **Required** lines are full (all of them by
  default); then it shows its message and runs its outcomes (unlock a door,
  set a flag, place nodes: the world fixed). The game lists bundles by their
  **Group** (a room or a board).
- **Achievements** unlock the moment their conditions hold (the same
  conditions as events; an achievement without any never unlocks). A
  **hidden** one shows as ??? until then. `steamId` is kept for exports
  with Steamworks (see [Later](../ROADMAP.md#later)).
- **Mail**: a letter has a subject, a sender (an NPC), a text (with
  [text substitution](#text-substitution)), attachments, money and outcomes
  that run when it is first read. It arrives in the morning: after a
  **sendMail** outcome (from an event, an action, a dialogue option or a
  cutscene), after a quest whose rewards name it (**Mail**), or by itself on
  a morning its **conditions** hold (an NPC at some hearts, a
  **birthday**). Without **Repeat** a letter comes once; with it, every
  morning its conditions hold. The player reads letters on the Mail screen,
  taking what they hold.

### Combat and adventure

Combat is off until you tick **Combat** in **Settings → Combat**. Then:

- The player has **health** beside energy (`maxHealth`). Monsters that
  touch them and terrain with **Damage** take it (a hit leaves the player
  unharmed for `hurtTicks`); food with **Health** and a night's sleep
  restore it. At 0 the player faints and comes to at the faint scene and
  tile (the start scene's middle by default) with half their health, paying
  the faint penalty.
- **Monsters** have health, the damage they do, defense, speed, how they
  move (**chase** a player within **Sight**, **wander**, or stay **still**),
  drops and the combat experience they give. A scene's **Monsters** (in the
  scene panel, one spawn per line: `slime 3`, `bat 2 at 4,4`,
  `slime 3 in 2,2-10,8`) spawns them each morning; a mine depth band's
  **Monsters** and `monsterCount` (Settings → Mine, a form per band) put
  them on its floors.
- A weapon is a tool whose action is **attack** (Content → Tools): it
  hits the monsters on the tiles it covers for its item's `toolPower`,
  plus the player's combat level, less the monster's defense.
- Mine depth bands also scatter **Hazards** (a terrain with damage or
  hazard energy, on the overlay layer, by density) and leave **Treasure**
  (weighted items, `treasureCount` of them) on their floors.

### Problems

The Problems tab checks the whole project: missing references, unreachable
content and settings that would break the game. **Errors block export**;
warnings and tips help you improve the game. **Go to** jumps to the entry or
scene that needs a fix.

### Settings

Project settings: name, version and the game-text **locale**; gameplay
(energy, skills and the XP each skill level needs, player speed, collapse
penalties); the clock (day start and
end, game minutes per real second, and whether it **pauses** while a dialogue,
shop, minigame or menu is open, on by default); the calendar's **seasons**
(add, rename, reorder, change their length) and **festivals**. The day must
end at least an hour after it starts and by minute 4294; the clock can run at
most 1440 game minutes per real second. The **Export** section holds
what the exported game shows: its title, executable name, version, author,
company, game id (set it once: it keeps players' save folders stable), icon,
window size, pixel scale, fullscreen and credits.

The locale also picks the language of the game's own interface when the
player hasn't chosen one: English and Spanish are built in. Players can
change it in the game's **Settings → Accessibility → Language**. The engine's
own messages follow it too ("Not enough money!", the built-in minigames'
prompts and buttons, save-load notices); what you write (names, dialogue,
event messages, a minigame's own **prompt**) shows as you wrote it.

### Mods

Install **content packs** from their JSON file: the editor shows what a pack
adds, overrides and needs, which hooks its plugins hear and what they may do,
before **Install reviewed pack**. Installed packs can be turned on and off,
reordered, removed, or imported into the project as ordinary content. A pack's
scenes join the game world the first time a door or a warp leads there; pack
ids are namespaced (`my-pack:cave`) but scene references inside a pack are not,
so a pack names its own scenes in full.

Packs can also bring vocabulary and story: terrains, tools, item kinds, crop
qualities, crop mutations, cutscenes and heart events. Vocabulary keeps the
ids it was written with (a pack's `lava` is painted as `lava`), and a pack
replaces a built-in or an earlier pack's definition only when its manifest
lists that id in `overrides`; otherwise Problems warns and the earlier one
stays. Cutscenes and heart events are namespaced like other content, and so
are the farm-building, goal and combat content packs can add: building
types, collections, bundles, achievements, letters and monsters, with the
references between them (a building's upgrade and materials, a builder
shop's buildings, a collection's items, a letter's attachments, a monster's
drops, a condition's or outcome's building, bundle, achievement, letter or
monster, and the subject of an event's trigger).

A pack with the UI panels permission (`permissions.uiPanels`) adds its
own **game panels** (`content.gamePanels`, the same panels as the Interface
tab's) to the game screen, after your game's own. Without the permission
they are not shown, and Problems says so.

A pack's plugins only change your game in the ways its manifest declares
(`permissions.mutations`): by default they can show messages, play sounds
and touch the pack's own flags and items, nothing else. Plugin answers apply
right before the next game tick, and an `onWeatherRoll` answer comes too late
to change the night's watering and storm damage. [PLUGINS.md](PLUGINS.md)
has the permissions, timing and limits.

### Art

**Import images** takes PNG, JPEG, WebP, GIF, BMP and SVG files; the list shows
a thumbnail of each. Make **animation clips** by slicing a whole sheet, by
clicking cells of the sheet (each click adds that cell as the next frame), or
from separate images (**Add image frame** adds another picture as a frame, so
one animation can use several files). The preview plays the clip; **Pause
preview** holds a frame. Then **Assign artwork** to the player, a map brush
(with its tile behavior), an NPC, an item, a crop, a node, an animal or a
machine. Mark pixel art as **Crisp pixel art** so it stays sharp. Art for the
player or an NPC turns and walks when its clips are named by direction
(`idle-down`, `idle-left`, `walk-up`, `walk-right`…, with `idle` and `walk`
for any direction): walking, a schedule and a cutscene's **face** and
**move** steps pick the clip.

### Workshop

The Workshop creates working interactions in one step (and one undo):
a branching story, a romance milestone, a letter and mailbox, a building with
an interior, a watering spell, a combat encounter, a fishing challenge, a
catchable insect, forageables, trees, rocks, weeds and a crafting discipline.
Fill in the name and text, **Create pattern**, then **Open its editor** to
fine-tune what it made. **Build your game** links jump straight to each part
of the editor: art, scenes, crops, animals, items, recipes, characters,
quests, events, actions and the Problems check.

### Interface

Game panels appear on the game screen: live counters, story status or action
buttons. Give a panel a title, optionally a story flag that must be set before
it shows, and entries: a label, what it shows (text, money, energy, day, an
item count, a flag or an action button), and for items and actions a pick
from your game's list. Installed packs with the UI panels permission add
theirs after yours (see [Mods](#mods)).

### If something goes wrong

If the editor hits an unexpected error, it shows what happened instead of
closing: **Try Again** reopens the editor on the same project, and **Undo last
change and try again** helps when the error came right after an edit. Your
project is saved up to the error. If the project can't be saved (a full disk,
a folder you can't write to), a red banner above the editor says why and keeps
your changes open; **Retry save** tries again, and File → Export Project JSON…
keeps a copy anywhere.

## 3. Playtest in Play Mode

Press **F5** (or the mode button) to play the project with the same player an
exported game uses; it starts straight in the game, without the title screen.
Click the game to give it the keyboard. **Esc** opens the game's own pause menu and settings; **F6**
returns to the editor.

Playtests are safe: returning to Edit Mode restores the world to the moment
you started, so your farm isn't trampled by testing. The Play Mode toolbar has:

- **Restart**: start the playtest over from that moment.
- **Keep changes**: keep what happened in the playtest when you return: the
  world, the player, flags, the date, and what was open or under way (a
  dialogue or shop, a cutscene that was playing, today's purchases, walking
  NPCs, the mine floor), so the next playtest starts right there.
- **Debug**: add money, restore energy, skip a day or an hour, change the
  season, give seeds or materials, teleport to a scene and set flags.

## 4. Share your game

**File → Export Game** (Ctrl+Shift+E, or Cmd+Shift+E on macOS) builds real desktop games: a Windows x64
folder with the game's `.exe`, and a Linux x64 folder that also runs on the
Steam Deck, each with an optional zip or tar.gz archive. Players unzip and
play; no editor, browser or install is needed. The exported game has a title
screen, three save slots with autosave each morning, a pause menu, settings
(display, audio, rebindable controls, language, text size, a readable font and
reduced motion) and gamepad support.

The Problems tab warns about names and dialogue containing characters the game's
font cannot draw, with a link to the affected content. The bundled font covers
Latin, Greek and Cyrillic; unsupported characters appear as boxes. Right-to-left
shaping is not supported. Check these warnings before sharing a translated game.

The **Web (browser demo)** target makes a page for itch.io; on phones and
tablets it shows touch controls (a D-pad, Interact, Sleep, Inventory and
Menu).

Export stops when the Problems tab has errors. Bump the version in Settings
when you ship an update: saves from the older version keep loading.

## Player controls (default)

WASD or the arrow keys move (hold to walk, diagonals too) · E, Space or Enter
interact · Q watering can · T hoe · R axe · F pickaxe · C scythe · X crafting ·
Z sleep · I inventory · J quests · G goals · M mail · Esc closes a panel or opens the pause menu ·
1–9 pick a dialogue option. Gamepads work everywhere, menus included. Players
can rebind keys in the game's **Settings → Controls**; **Help → Keyboard
Shortcuts** lists the editor's shortcuts too.

## More

- [Exporting games](EXPORT.md): what an exported game contains and how to
  ship it on Steam or itch.io.
- [The game player](PLAYER.md): the player's screens, settings, saves and
  command-line options.

### Seasonal forage and deeper fishing

In **Node types**, set **Seasons** to hide a node outside those seasons. Leave
it empty for year-round gathering. Each drop may also have its own seasons;
only the current season's entries participate in the weighted drop roll.
An authored seasonal node keeps its remaining health while dormant.

In the map's surroundings, **Morning forage** edits a list of spawn tables.
Give each a unique id, a maximum number of new nodes per morning, weighted
node types and allowed background terrains. Optional scene tags, seasons and
rectangular regions restrict where it applies. **Preview eligible tiles** lists
the coordinates before you save. Paths, doors and their arrival tiles, blocked
tiles, crops, items, machines and existing nodes are excluded; the live player
and NPC positions are also excluded at runtime. The first game tick and each
morning use the game's seed, so identical games spawn identical forage. Set
**Lifetime days** to remove uncollected forage after that many days; all spawned
forage clears when the season changes. Workshop **Forage** creates a spawn
table and its resource node together. Problems warns when no tiles qualify.

In **Fish tables**, **Filter** restricts the minute range (inclusive), weather,
season and an event condition. Each entry's **Rules → Filter** can narrow it
further. By default the first eligible table wins, preserving existing games.
Set **Layered** on that first table to combine entries from every eligible
table into one weighted roll; the first table still determines the junk roll.
Entry **Rules** also chooses XP (default 8), enables quality, and sets an
optional integer size range. Quality uses the game's crop-quality tiers,
fishing skill and minigame score. The **Catch log** records the best size of
each species and survives saving and keeping playtest changes.

An item's **Fishing gear** selects `bait` or `tackle`. The first inventory
stack of each kind is used automatically: bait adds weight to its chosen fish
ids (empty means all), and tackle reduces escape chance by a percentage and
can improve quality. One unit of each is consumed only when a fish lands in
the inventory. No bait is consumed by junk, an escaped fish or a full bag.

A machine's **Fish production** makes a passive trap or pond. Select exactly
one **Table id** for a trap or **Species item id** for a pond, plus a production
interval and quantity. On matching absolute game days, an empty machine fills
with output that the player collects normally. Trap tables respect season,
time, weather, scene and entry filters; pending output is never overwritten.

### Player progression, skills and food buffs

In **Settings → Player progression**, enable limits for permanent backpack,
energy and health upgrades. The `setInventorySize`, `addInventorySize`,
`addMaxEnergy` and `addMaxHealth` event outcomes use `amount` in slots or whole
points. They clamp to the configured limits and never remove occupied slots.
The matching plugin capabilities use an integer `amount`. Health upgrades
require combat. An upgrade changes the maximum; it does not heal the player.

A shop stock line can include an **Upgrade** with a stable id, one of those
outcome kinds and its amount. The line's item supplies its name, artwork and
base price; purchasing it applies the upgrade once instead of giving that item.
Keep the upgrade id stable after shipping a game, since saves remember it.
A shop's **Tool upgrades** list connects a source item to the next-tier tool
of the same kind, with a gold cost, materials and a number of nights. Players
hand in the tool on the shop's Repair tab and collect it there after the
specified nights. A full backpack leaves the finished tool safely at the shop;
it returns to its old slot when possible. Closed shops refuse both operations.

The progression form also holds **Skills**. With no definitions the built-in
XP and bonuses remain unchanged. **Classic skills** creates editable definitions
with the current curve's farming yield/quality and combat bonuses, classic XP,
and node/monster training rows from the current project. Definitions replace automatic action training:
list `harvest`, `gather`, `catch`, `gift`, `talk`, `craft`, `slay` or `ship` and
its XP for each skill. An optional training **Subject** limits a row to one
crop/fish/gift/shipped item id, node type id, NPC id, monster type id, or crafted
recipe id. Machine-output collection uses its first output item id as the craft
subject. Omit the subject to train on every successful action of that kind.
Use the existing level curve to set thresholds. Level
rewards grant stat bonuses and may run event outcomes once on reaching that
level. Perks offer named choices at a level; the inventory's skills section
shows XP, the next reward and unlocked choices. Choosing a perk is permanent
and saved. The `perk` condition checks the skill, perk id and optional choice.

Supported reward stats are `yield`, `quality` (quality tiers), `attack`,
`defense`, `maxEnergy`, `maxHealth`, `moveSpeed`, `toolEnergyCost`, `fishingEscape`,
`craftingSpeed` and `sellPrice`. Speed, cost, escape, crafting speed and sale
bonuses are percentages; other amounts are whole points. Positive tool cost,
fishing escape and crafting-speed bonuses reduce cost, escape and duration.
`sellPrice:crop`, for example, limits a sale bonus to the `crop` item category.

Edible items can carry **Buffs**: an id, stat, amount and duration in game
minutes, plus an optional icon (an asset id, item id, or literal symbol). `applyBuff` outcomes carry the same nested
`buff` record; plugins need the `applyBuff` capability. A repeat of the same id
refreshes its duration. Different ids affecting the same stat take the
strongest magnitude unless marked additive. Buffs are saved, expire by the
game clock, appear on the HUD with minutes remaining, and clear on sleep
unless marked **Persists after sleep**. The `buff` condition checks an active
id. Temporary `skillLevel` bonuses affect skill checks and harvest quality;
`luck` changes weighted fishing, forage, node and monster rewards. Projects without these optional records
keep their previous replay behavior.

Skills can also be selected in **Mods → Export selection as pack**. Their
ids and event `skill` references gain the pack prefix on installation;
importing the pack adds its definitions to the progression settings.

### Weekdays, changing routines and shop calendars

New projects start with a seven-day week, Monday through Sunday. Settings →
Calendar also has a **Classic week** preset. Add, remove or reorder its
weekday rows to make your own week, and set a zero-based **Weekday offset** to
choose the day on which absolute day 1 falls. The week continues across seasons
and years. Missing or empty weekdays disable the week, preserving older projects. Each
row has an id, a name and a translated name for the project's selected language;
translations already entered for other languages are retained.

The **Weekday** condition accepts one or more ids and works everywhere event
conditions do. `{weekday}` in game text uses the current language's name, with
the main name as fallback. The HUD date, Sky preview and Playtest Debug show it.

An NPC's **Schedules** list contains named variants with conditions and stops.
The first matching variant wins; otherwise the original **Schedule** applies.
The choice is made each morning and saved. Changing weather reselects the route
at the next in-game minute; flags and hearts changed during the day apply the
next morning. An empty matching route means no scheduled destinations that day.
The stop rows include **Pick on map**, just like the original schedule.
Problems reports missing scenes, blocked or out-of-bounds stops, duplicate route
ids, unknown weekday references and variants hidden by an unconditional route.

Shops can set opening **Hours**, **Closed on** weekdays, seasons or festivals,
and a **Keeper NPC id**. The keeper must be in the player's scene. Hours include
the opening minute and exclude the closing minute; an opening time later than
closing represents an overnight shop. Equal times mean always closed. Opening
through dialogue and every purchase, sale and service recheck these rules.

**Price modifiers** multiply prices in list order when their seasons, weekdays,
item and conditions match. Buy and sell multipliers are independent. Optional
**Saturation** lowers the next transaction's sell price after each
**Quantity per step** sold, stopping at **Minimum multiplier**. Sales stop
counting after **Recovery days**. A transaction's entire quantity receives the
quoted price; the next transaction sees the new demand. Demand is saved and
also retained by **Keep changes**.

### Festival places and activities

The Workshop's **Town festival** pattern creates a festival map, an entrance
on the chosen tile and a once-per-day prize activity. Its calendar date and
**Festival program** are editable in Settings → Festivals. The program chooses
the grounds scene, entrance and arrival tiles, opening/closing minutes and any
entry conditions. Interact with the entrance on the festival date while it is
open. Interact with the arrival tile inside to leave; leaving advances the clock
to closing. **End day** sleeps after leaving. Closing time also ends a visit.

**Attendance** places named NPCs on the grounds and holds their schedules until
the visit ends. A guest can have a festival dialogue; otherwise its usual daily
lines and conversation apply. Original positions and routes return on leaving,
sleeping or an event that moves the player elsewhere. Active visits survive a
save and load.

A program can provide a **Shop id** and **Shop tile**. That shop is available
only during a visit. **Activity** supplies a tile and an action, cutscene or
minigame. Minigame result tiers and actions define their own rewards. An optional
**Contest** checks for a specified item at or above a minimum quality, chooses
the best qualifying item, optionally consumes one, then runs its rewards.
A successful activity can run once per festival day, including after leaving
and reloading. A failed item check leaves the activity available. **On miss**
outcomes run overnight when the player did not visit. Problems checks the
program's locations, hours, guests, dialogues, shop and activity references.
