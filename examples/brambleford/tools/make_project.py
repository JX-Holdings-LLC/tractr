"""Builds Brambleford, a short Tractr farming game, as a single-file Tractr project.

    python3 make_project.py <base.json> <art/out> <brambleford.json>

<base.json> is the Quest RPG template (tools/template.fsx writes it); its tools, materials, nodes and
fish stay, and everything that makes Brambleford Brambleford (maps, people, crops, story) is written
here. The art comes from art/make_art.py (Vixl).
"""

import base64
import copy
import json
import pathlib
import sys

BASE, ART, OUT = (pathlib.Path(a) for a in sys.argv[1:4])
project = json.loads(BASE.read_text())
art_manifest = json.loads((ART / "manifest.json").read_text())

# ------------------------------------------------------------------ art assets

ASSETS = []


def data_url(name):
    return "data:image/png;base64," + base64.b64encode((ART / f"{name}.png").read_bytes()).decode()


def frame(x, y, w, h, ticks=8):
    return {"x": x, "y": y, "width": w, "height": h, "ticks": ticks}


def add_asset(name, kind, label, animations=None):
    from PIL import Image  # only to read the sheet size

    with Image.open(ART / f"{name}.png") as image:
        width, height = image.size
    asset = {"id": f"art-{name}", "name": label, "type": kind, "width": width, "height": height,
             "dataUrl": data_url(name)}
    if animations:
        asset["animations"] = animations
    ASSETS.append(asset)
    return asset["id"]


def npc_sheet(key, label):
    clips = []
    for row, direction in enumerate(["down", "left", "right", "up"]):
        # Tractr fits creator art into one tile, so the frames are cut to the figure (hat brim to boots).
        clips.append({"name": f"idle-{direction}", "loop": True, "frames": [frame(3, row * 48 + 4, 26, 44, 30)]})
        clips.append({"name": f"walk-{direction}", "loop": True,
                      "frames": [frame(c * 32 + 3, row * 48 + 4, 26, 44, 8) for c in range(4)]})
    return add_asset(f"npc-{key}", "npc", label, clips)


def strip(name, label, kind, clip, ticks=8, top=0):
    """A one-row sheet as a clip; `top` skips empty rows above the art (crops grow in the lower 32)."""
    spec = art_manifest[name]
    w, h = spec["frame"]
    frames = [frame(i * w, top, w, h - top, ticks) for i in range(spec["frames"])]
    return add_asset(name, kind, label, [{"name": clip, "loop": True, "frames": frames}])


def visual(asset_id, animation=None, fr=None):
    v = {"assetId": asset_id}
    if animation:
        v["animation"] = animation
    if fr:
        v["frame"] = fr
    return v


# ------------------------------------------------------------------ crops and items

REMOVED_CROPS = {"wheat", "corn", "tomato", "carrot", "potato", "strawberry", "pumpkin", "cauliflower", "blueberry"}
removed_items = {f"seed-{c}" for c in REMOVED_CROPS} | {f"crop-{c}" for c in REMOVED_CROPS}
removed_items |= {"food-veggie-soup", "food-fruit-tart", "food-preserves"}


def uses_removed(value):
    text = json.dumps(value)
    return any(f'"{item}"' in text for item in removed_items)


project["items"] = [i for i in project["items"] if i["id"] not in removed_items]
project["recipes"] = [r for r in project["recipes"] if not uses_removed(r)]
project["customCrops"] = []

ember_crop = strip("crop-emberroot", "Emberroot (growing)", "art", "growth", top=16)
moon_crop = strip("crop-moonmelon", "Moonmelon (growing)", "art", "growth", top=16)
ember_item = add_asset("item-emberroot", "item", "Emberroot")
moon_item = add_asset("item-moonmelon", "item", "Moonmelon")
ember_seed = add_asset("item-seed-emberroot", "item", "Emberroot seeds")
moon_seed = add_asset("item-seed-moonmelon", "item", "Moonmelon seeds")

project["customCrops"] = [
    {
        "id": "emberroot", "name": "Emberroot", "visual": visual(ember_crop),
        "seedCost": 15, "baseHarvestValue": 40, "growthDays": 3, "stages": 5,
        "seasons": ["spring", "summer"], "canRegrow": False, "mutationChance": 0.02,
        "yieldMin": 1, "yieldMax": 2, "harvestItemId": "crop-emberroot",
    },
    {
        "id": "moonmelon", "name": "Moonmelon", "visual": visual(moon_crop),
        "seedCost": 40, "baseHarvestValue": 120, "growthDays": 5, "stages": 5,
        "seasons": ["spring", "summer"], "regrowthDays": 3, "canRegrow": True, "mutationChance": 0.03,
        "yieldMin": 1, "yieldMax": 1, "harvestItemId": "crop-moonmelon",
        "light": {"radius": 2, "color": "#9fd8ff", "brightness": 55},
    },
]


def item(id, name, description, type, value, **extra):
    base = {"id": id, "name": name, "description": description, "type": type, "stackable": True,
            "maxStack": 99, "value": value}
    base.update(extra)
    return base


NEW_ITEMS = [
    item("seed-emberroot", "Emberroot Seeds", "Warm little seeds. Sprout in 3 days, spring and summer.", "seed", 15,
         cropType="emberroot", visual=visual(ember_seed)),
    item("crop-emberroot", "Emberroot", "A spicy red root that stays warm in your pocket.", "crop", 40,
         cropType="emberroot", visual=visual(ember_item)),
    item("seed-moonmelon", "Moonmelon Seeds", "Pale seeds that hum faintly. 5 days, and they grow again.", "seed", 40,
         cropType="moonmelon", visual=visual(moon_seed)),
    item("crop-moonmelon", "Moonmelon", "A cool, silvery melon that glows after dark.", "crop", 120,
         cropType="moonmelon", visual=visual(moon_item)),
    item("food-ember-stew", "Ember Stew", "Marigold's recipe. Warms you right down to the toes.", "food", 110,
         energy=45, maxStack=20),
    item("food-moon-jam", "Moonmelon Jam", "Spread on toast at midnight, as is tradition.", "food", 300,
         energy=60, maxStack=20),
]
project["items"].extend(NEW_ITEMS)
ITEMS = {i["id"]: i for i in project["items"]}

project["itemCategories"] = [
    {"id": "food", "name": "Food", "plantable": False, "fertilizer": False, "equippable": False, "edible": True,
     "energy": 30, "placeable": False, "giftable": True, "sellable": True},
]

project["recipes"].extend([
    {"id": "recipe-ember-stew", "name": "Ember Stew",
     "inputs": [{"itemId": "crop-emberroot", "quantity": 2}, {"itemId": "material-fiber", "quantity": 1}],
     "outputs": [{"itemId": "food-ember-stew", "quantity": 1}], "processingMinutes": 0, "category": "cooking"},
    {"id": "recipe-moon-jam", "name": "Moonmelon Jam",
     "inputs": [{"itemId": "crop-moonmelon", "quantity": 1}, {"itemId": "crop-emberroot", "quantity": 1}],
     "outputs": [{"itemId": "food-moon-jam", "quantity": 2}], "processingMinutes": 0, "category": "cooking"},
])

# ------------------------------------------------------------------ terrains (Vixl props)

lantern_art = strip("prop-lantern", "Street lantern", "tile", "lit")
fence_art = add_asset("prop-fence", "tile", "Fence")
flowers_art = strip("prop-flowers", "Wildflowers", "tile", "sway", ticks=40)
wheel_art = strip("prop-millwheel", "Mill wheel", "tile", "turn", ticks=14)
bridge_art = add_asset("prop-bridge", "tile", "Bridge planks")
well_art = add_asset("prop-well", "tile", "Village well")


def terrain(id, name, layer, color, vis, walkable, **extra):
    t = {"id": id, "name": name, "layer": layer, "color": color, "visual": vis, "walkable": walkable,
         "plantable": False, "waterSource": False, "fishable": False, "blocksLight": False,
         "hazardEnergy": 0, "indoor": False, "transforms": []}
    t.update(extra)
    return t


project["terrains"] = [
    terrain("lantern", "Lantern", "object", "#3a3640",
            visual(lantern_art, fr=frame(0, 0, 32, 64)), False, tall=1,
            light={"radius": 4, "color": "#ffcf7a", "brightness": 80}),
    terrain("fence", "Fence", "object", "#7a5c3a", visual(fence_art), False),
    terrain("flowers", "Wildflowers", "overlay", "#e8556a", visual(flowers_art, "sway"), True),
    terrain("millwheel", "Mill wheel", "object", "#6b4a2e", visual(wheel_art, "turn"), False, tall=1),
    terrain("bridge", "Bridge", "overlay", "#9a7a52", visual(bridge_art), True),
    terrain("well", "Well", "object", "#8a8a95", visual(well_art), False, tall=1),
]

# ------------------------------------------------------------------ maps

LEGEND = {
    ".": dict(type="grass", background="grass"),
    ",": dict(type="grass", background="grass", overlay="flowers"),
    "=": dict(type="path", background="grass", overlay="path"),
    "f": dict(type="floor", background="floor"),
    "#": dict(type="wall", background="grass", object="wall", collision=True),
    "D": dict(type="door", background="grass", object="door"),
    "s": dict(type="soil", background="soil"),
    "~": dict(type="water", background="water", collision=True),
    "b": dict(type="path", background="water", overlay="bridge"),
    "L": dict(type="grass", background="grass", object="lantern", collision=True),
    "F": dict(type="grass", background="grass", object="fence", collision=True),
    "W": dict(type="floor", background="floor", object="well", collision=True),
    "M": dict(type="water", background="water", object="millwheel", collision=True),
    "T": dict(type="grass", background="grass", node="node-tree"),
    "P": dict(type="grass", background="grass", node="node-tree-pine"),
    "R": dict(type="grass", background="grass", node="node-rock"),
    "w": dict(type="grass", background="grass", node="node-weeds"),
    "u": dict(type="grass", background="grass", node="node-stump"),
}
NODE_HEALTH = {n["id"]: n["health"] for n in project["nodeTypes"]}
if "node-tree-pine" not in NODE_HEALTH:
    LEGEND["P"] = LEGEND["T"]


def tiles(rows, crops=None):
    crops = crops or {}
    width = len(rows[0])
    grid = []
    for y, row in enumerate(rows):
        assert len(row) == width, (y, row, len(row))
        line = []
        for x, ch in enumerate(row):
            spec = LEGEND[ch]
            t = {"x": x, "y": y, "type": spec["type"], "background": spec["background"],
                 "overlay": spec.get("overlay"), "object": spec.get("object"),
                 "collision": spec.get("collision", False), "soilMoisture": 0, "soilFertility": 0}
            if "node" in spec:
                t["node"] = {"typeId": spec["node"], "remainingHealth": NODE_HEALTH[spec["node"]]}
                t["collision"] = False
            if (x, y) in crops:
                kind, stage = crops[(x, y)]
                t["crop"] = {"type": kind, "plantedOnDay": 1, "daysGrown": stage, "stage": stage,
                             "watered": False, "quality": "normal", "mutation": None, "harvestCount": 0,
                             "daysWithoutWater": 0}
                t["soilMoisture"] = 0
            line.append(t)
        grid.append(line)
    return grid


FARM = [
    "########################",
    "#TT.,,....#####...T.T.T#",
    "#T........#####.......T#",
    "#.........##D##....R...#",
    "#...........=.......w..#",
    "#.ssssssss..=..........#",
    "#.ssssssss..=......R...#",
    "#.ssssssss..=..........#",
    "#.ssssssss..==========.=",
    "#.ssssssss.....,,......#",
    "#..............FFF.FF..#",
    "#.w...L........F....F..#",
    "#..............F....F.T#",
    "#T.....,,......FFFFFF.T#",
    "#TT...........w......TT#",
    "########################",
]
SQUARE = [
    "########################",
    "#T..####...L..L...####T#",
    "#...####..........####.#",
    "#...##D#..........##D#.#",
    "#.L...=...........==..L#",
    "#.....=...ffffff..=....#",
    "#,,...=...ffWfff..=...,#",
    "#.....=...ffffff..=....#",
    "========================",
    "#.....,...ffffff...,,..#",
    "#.L.......ffffff.....L.#",
    "#...,,..........,,.....#",
    "#T....FFFF....FFFF....T#",
    "#T..........L.........T#",
    "#TT..,,..........,,..TT#",
    "########################",
]
RIVERSIDE = [
    "########################",
    "#T...######.~~~.....T.T#",
    "#....######.~~~......T.#",
    "#....###D##M~~~...R....#",
    "#.......=..L~~~........#",
    "#.L.....=...~~~.....,,.#",
    "#.......=...~~~..w.....#",
    "#..,,...=...~~~........#",
    "============bbb=======.#",
    "#...........~~~.....F..#",
    "#.T....,,...~~~..R..F..#",
    "#...........~~~.....F..#",
    "#.T..L......~~~..L..F..#",
    "#T..........~~~~.......#",
    "#TT.T.......~~~~~...TTT#",
    "########################",
]

farm_crops = {(x, y): ("emberroot", 2 if (x + y) % 3 else 3) for x in range(2, 6) for y in range(5, 7)}

project["scenes"] = [
    {"id": "scene-farm", "name": "Thistledown Farm", "width": 24, "height": 16, "tiles": tiles(FARM, farm_crops),
     "transitions": [{"fromX": 23, "fromY": 8, "toSceneId": "scene-square", "toX": 1, "toY": 8}],
     "npcs": [], "events": []},
    {"id": "scene-square", "name": "Brambleford Square", "width": 24, "height": 16, "tiles": tiles(SQUARE),
     "transitions": [{"fromX": 0, "fromY": 8, "toSceneId": "scene-farm", "toX": 22, "toY": 8},
                     {"fromX": 23, "fromY": 8, "toSceneId": "scene-riverside", "toX": 1, "toY": 8}],
     "npcs": [], "events": []},
    {"id": "scene-riverside", "name": "Millbrook Riverside", "width": 24, "height": 16,
     "tiles": tiles(RIVERSIDE),
     "transitions": [{"fromX": 0, "fromY": 8, "toSceneId": "scene-square", "toX": 22, "toY": 8}],
     "npcs": [], "events": []},
]
project["startSceneId"] = "scene-farm"
project["name"] = "Brambleford"
project["version"] = "1.0"

# ------------------------------------------------------------------ people

PORTRAIT = {e: frame(i * 48, 0, 48, 48) for i, e in enumerate(["neutral", "happy", "sad"])}


def portraits(key, label):
    asset = add_asset(f"portrait-{key}", "art", f"{label} portraits")
    return [{"expression": e, "visual": visual(asset, fr=fr)} for e, fr in PORTRAIT.items()]


def stop(minute, scene, x, y):
    return {"minute": minute, "sceneId": scene, "x": x, "y": y}


def line(text, *conditions, expression=None, weight=None):
    entry = {"text": text, "conditions": list(conditions)}
    if expression:
        entry["expression"] = expression
    if weight:
        entry["weight"] = weight
    return entry


def rain():
    return {"type": "weather", "weatherIds": ["rain", "storm"]}


def quest_is(quest, status):
    return {"type": "questStatus", "questId": quest, "status": status}


def flag(name, value=True):
    return {"type": "flag", "flag": name, "value": value}


def evening():
    return {"type": "timeOfDay", "minMinute": 1080, "maxMinute": 1560}


def morning():
    return {"type": "timeOfDay", "minMinute": 360, "maxMinute": 660}


def dialogue(id, npc, text, options, expression=None):
    d = {"id": id, "npcId": npc, "text": text, "options": options}
    if expression:
        d["expression"] = expression
    return d


NPCS = []
DIALOGUES = []


def person(key, name, x, y, scene, appearance, dialogues, lines, schedule=None, tastes=None, birthday=None,
           move="stationary", romanceable=None):
    npc_id = f"npc-{key}"
    npc = {"id": npc_id, "name": name, "visual": visual(npc_sheet(key, name)), "x": x, "y": y,
           "sceneId": scene, "dialogue": dialogues, "canMove": bool(schedule) or move != "stationary",
           "movePattern": move, "appearance": appearance, "lines": lines,
           "portraits": portraits(key, name)}
    if move == "wander":
        npc["wanderRadius"] = 3
    if schedule:
        npc["schedule"] = schedule
    if tastes:
        npc["giftTastes"] = {"loved": tastes[0], "liked": tastes[1], "disliked": tastes[2], "hated": tastes[3]}
    if birthday:
        npc["birthday"] = {"season": "spring", "day": birthday}
    if romanceable is not None:
        npc["romanceable"] = romanceable
    NPCS.append(npc)
    DIALOGUES.extend(dialogues)
    return npc_id


Q1, Q2, Q3 = "quest-roots", "quest-silent-mill", "quest-lantern-night"
Q_STEW, Q_FISH, Q_PIP = "quest-ember-stew", "quest-something-biting", "quest-pips-map"

person(
    "odile", "Mayor Odile", 9, 4, "scene-square", "elder",
    [dialogue("dlg-odile", "npc-odile",
              "Welcome to Brambleford, dear. Your Aunt Wren kept Thistledown Farm blooming for forty years. "
              "We'd all be so glad to see it green again.",
              [{"text": "I'll do my best.", "once": True, "giveItem": "seed-emberroot", "giveItemQuantity": 5},
               {"text": "What happened to the Lantern Night?", "nextDialogueId": "dlg-odile-lanterns"},
               {"text": "Just saying hello!"}], expression="happy"),
     dialogue("dlg-odile-lanterns", "npc-odile",
              "Every spring we lit the lanterns with moonmelons from Wren's field. Then the mill went quiet, "
              "the seeds ran out, and... well. Some lights take a while to come back.",
              [{"text": "Maybe I can help."}], expression="sad")],
    [line("Good morning! The square always looks brightest before the bakery smoke rolls in.", morning(),
          expression="happy"),
     line("Rain again. Wren used to say the emberroot drinks it and laughs.", rain()),
     line("Five emberroot, and the soil will remember what it's for. That's how it starts.",
          quest_is(Q1, "active")),
     line("Bram's been grumbling at that mill for a year. Perhaps he'd let you help.", quest_is(Q1, "completed"),
          quest_is(Q2, "not-started")),
     line("Moonmelons! Three, and I'll have the lanterns out of the attic before you can blink.",
          quest_is(Q3, "active"), expression="happy"),
     line("Look at it glow. Thank you, truly.", flag("lanterns-lit"), expression="happy", weight=3),
     line("Evenings are long without the lanterns. I keep a candle in my window, just in case.", evening(),
          expression="sad")],
    schedule=[stop(360, "scene-square", 9, 4), stop(720, "scene-square", 15, 10), stop(1020, "scene-riverside", 4, 6),
              stop(1200, "scene-square", 9, 4)],
    tastes=(["crop-moonmelon", "gift-flower"], ["crop-emberroot", "food-moon-jam"], ["material-stone"], ["junk-boot"]),
    birthday=9,
)

person(
    "bram", "Bram the Miller", 7, 4, "scene-riverside", "farmer",
    [dialogue("dlg-bram", "npc-bram",
              "Wheel turns, stones don't. The axle on my grindstone cracked clean through last spring. "
              "No grinding, no moonmelon seed meal, no Lantern Night.",
              [{"text": "What do you need?", "offerQuestId": Q2},
               {"text": "Sounds rough. Good luck."}], expression="sad")],
    [line("Mornin'. Flour on my boots, flour in my beard. Flour in my dreams.", morning()),
     line("Ten logs and five good stones. Trees by the farm, rocks by the river.", quest_is(Q2, "active")),
     line("Listen to her go! Best sound in the valley.", flag("mill-fixed"), expression="happy", weight=2),
     line("Rain keeps the river high. Good for the wheel, bad for my knees.", rain()),
     line("Marigold's stew is the only reason I survive the winters.", evening(), expression="happy")],
    schedule=[stop(360, "scene-riverside", 7, 4), stop(1080, "scene-square", 7, 5),
              stop(1260, "scene-riverside", 7, 4)],
    tastes=(["food-ember-stew"], ["material-stone", "material-wood"], ["gift-flower"], ["junk-boot"]),
)

person(
    "juniper", "Juniper Vale", 20, 4, "scene-square", "merchant",
    [dialogue("dlg-juniper", "npc-juniper",
              "Vale Seeds and Sundries! Emberroot's fresh in. Tools, fertilizer, and the odd mystery packet.",
              [{"text": "Let's see the shop.", "openShopId": "shop-vale"},
               {"text": "How do I grow emberroot?", "nextDialogueId": "dlg-juniper-tips"},
               {"text": "Bye, Juniper!"}], expression="happy"),
     dialogue("dlg-juniper-tips", "npc-juniper",
              "Till with T, plant, then water with Q every day. Emberroot's ready in three days. "
              "Moonmelon takes five, but it keeps on giving, and it glows!",
              [{"text": "Thanks!"}])],
    [line("Seeds don't care how long a field's been asleep. They just want a chance.", expression="happy"),
     line("I've got moonmelon seed again! Bram's mill is a miracle worker.", flag("mill-fixed"),
          expression="happy", weight=2),
     line("Rainy days are for sorting seed packets. Very soothing. Very boring.", rain()),
     line("Pip keeps trying to plant pebbles in my window boxes.", morning())],
    tastes=(["gift-flower"], ["crop-emberroot", "crop-moonmelon"], ["junk-boot"], ["fish-catfish"]),
    romanceable=True,
)

person(
    "pip", "Pip", 15, 11, "scene-square", "villager",
    [dialogue("dlg-pip", "npc-pip",
              "Psst. I found a MAP. Rook says it's a real treasure map but he won't tell me where it goes "
              "unless a grown-up asks. You're a grown-up, right?",
              [{"text": "I'll ask Rook.", "offerQuestId": Q_PIP},
               {"text": "Maybe later, Pip."}], expression="happy")],
    [line("Did you know frogs can see behind them? I'm practising.", expression="happy"),
     line("Puddles! Best weather ever!", rain(), expression="happy", weight=2),
     line("Sela is my big sister. She can catch a fish with her eyes closed. Probably.", morning()),
     line("Emberroot is SPICY. Do not lick it. Trust me.", quest_is(Q1, "active")),
     line("It's my birthday! Did you bring a frog? An egg is also okay.",
          {"type": "dayRange", "minDay": 5, "maxDay": 5}, expression="happy", weight=5)],
    schedule=[stop(360, "scene-square", 15, 11), stop(540, "scene-farm", 13, 9), stop(780, "scene-square", 16, 11),
              stop(960, "scene-riverside", 10, 10), stop(1140, "scene-square", 15, 11)],
    tastes=(["product-egg", "fish-perch"], ["gift-flower"], ["material-stone"], ["crop-emberroot"]),
    birthday=5,
)

person(
    "sela", "Sela Marsh", 16, 10, "scene-riverside", "villager",
    [dialogue("dlg-sela", "npc-sela",
              "Shh. They're biting today. If you've got a rod, the bend past the bridge is the sweet spot.",
              [{"text": "Teach me?", "offerQuestId": Q_FISH},
               {"text": "I'll leave you to it."}])],
    [line("Quiet water, loud thoughts.", morning()),
     line("The river runs faster in the rain. So do the catfish.", rain()),
     line("Dad built a little boat once. Sank in a week. Best week of my life.", evening(), expression="happy"),
     line("You've been feeding the village. Pip says you're a wizard. I said farmer. Same thing.",
          quest_is(Q1, "completed"), expression="happy")],
    schedule=[stop(360, "scene-riverside", 16, 10), stop(1140, "scene-square", 19, 9),
              stop(1320, "scene-riverside", 16, 10)],
    tastes=(["fish-catfish"], ["fish-carp", "fish-perch", "food-ember-stew"], ["gift-flower"], ["junk-boot"]),
    romanceable=True,
)

person(
    "marigold", "Marigold Fenn", 6, 4, "scene-square", "merchant",
    [dialogue("dlg-marigold", "npc-marigold",
              "Hello, sunshine! The Copper Kettle's been serving stew without emberroot for a year. "
              "It's soup. It's just soup. Bring me some and I'll teach you the real thing.",
              [{"text": "I'll bring you emberroot.", "offerQuestId": Q_STEW},
               {"text": "Just browsing the smells."}], expression="happy")],
    [line("Every pot I stir has a little bit of someone's farm in it.", expression="happy"),
     line("Rain means soup weather. Soup weather means everyone visits. Everyone visiting means more soup.",
          rain(), expression="happy"),
     line("Bram eats three bowls and then says he isn't hungry. Every day.", evening()),
     line("Ember Stew is back on the menu! You did that.", quest_is(Q_STEW, "completed"), expression="happy",
          weight=2)],
    schedule=[stop(360, "scene-square", 6, 4), stop(900, "scene-riverside", 9, 6), stop(1080, "scene-square", 6, 4)],
    tastes=(["crop-emberroot"], ["product-egg", "crop-moonmelon"], ["material-fiber"], ["junk-boot"]),
)

person(
    "rook", "Rook the Wanderer", 13, 11, "scene-square", "villager",
    [dialogue("dlg-rook", "npc-rook",
              "Ah, a new face. I collect stories, old boots and the occasional shiny rock. "
              "In trade, I tell people where things are buried.",
              [{"text": "Pip wants to know about the map.", "requiresFlag": "pip-asked",
                "nextDialogueId": "dlg-rook-map"},
               {"text": "Tell me a story.", "nextDialogueId": "dlg-rook-story"},
               {"text": "Safe travels."}]),
     dialogue("dlg-rook-map", "npc-rook",
              "The X is under the old well. Not the treasure, mind. The treasure is that he'll spend all "
              "afternoon looking. Tell him I said so.",
              [{"text": "Ha. I'll tell him.", "eventFlag": "rook-told"}], expression="happy"),
     dialogue("dlg-rook-story", "npc-rook",
              "Long ago a farmer planted a melon under a full moon, and it never stopped glowing. "
              "They say every moonmelon since is a cutting of that first light.",
              [{"text": "That's lovely."}])],
    [line("Roads are long, but this village is a good place to sit down.", expression="neutral"),
     line("Rain on my hat brim. Best drum in the world.", rain(), expression="happy"),
     line("I hear the lanterns are coming back. I may stay the season.", flag("mill-fixed"), expression="happy")],
    move="wander",
    tastes=(["gem-quartz", "junk-boot"], ["ore-copper"], ["crop-emberroot"], ["gift-flower"]),
)

project["npcs"] = NPCS
project["dialogues"] = DIALOGUES

# ------------------------------------------------------------------ shop

project["shops"] = [{
    "id": "shop-vale", "name": "Vale Seeds & Sundries",
    "stock": [{"itemId": "seed-emberroot"}, {"itemId": "fertilizer-basic"}, {"itemId": "fertilizer-quality",
              "dailyLimit": 5}, {"itemId": "tool-fishing-rod"}, {"itemId": "feed-hay"}, {"itemId": "gift-flower"},
              {"itemId": "tool-hoe-2"}, {"itemId": "tool-watering-can-2"}],
    "sellPriceMultiplier": 1, "buysItems": True, "repairsTools": True, "repairCostPerPoint": 0.5,
}]

# ------------------------------------------------------------------ quests


def objective(id, type, description, **targets):
    o = {"id": id, "type": type, "description": description, "completed": False, "progress": 0}
    o.update(targets)
    return o


project["quests"] = [
    {"id": Q1, "name": "Roots in the Ground", "giver": "npc-odile", "status": "not-started",
     "description": "Aunt Wren's field has been asleep for a year. Wake it up: harvest 5 emberroot. "
                    "(Till with T, plant seeds, water with Q.)",
     "objectives": [objective("obj-roots", "harvest", "Harvest 5 emberroot", targetCropType="emberroot",
                              targetCropQuantity=5)],
     "rewards": {"money": 150, "items": [{"itemId": "seed-emberroot", "quantity": 6}]},
     "autoStart": True, "repeatable": False},
    {"id": Q2, "name": "The Silent Mill", "giver": "npc-bram", "status": "not-started",
     "description": "Bram's grindstone axle snapped. Bring him 10 wood and 5 stone so the mill can grind "
                    "moonmelon seed again.",
     "objectives": [objective("obj-mill-wood", "collect", "Gather 10 wood", targetItemId="material-wood",
                              targetItemQuantity=10),
                    objective("obj-mill-stone", "collect", "Gather 5 stone", targetItemId="material-stone",
                              targetItemQuantity=5)],
     "rewards": {"money": 200, "items": [{"itemId": "seed-moonmelon", "quantity": 4}]},
     "prerequisites": [Q1], "autoStart": False, "repeatable": False},
    {"id": Q3, "name": "Lantern Night", "giver": "npc-odile", "status": "not-started",
     "description": "Grow 3 moonmelons so Brambleford can light its lanterns again.",
     "objectives": [objective("obj-moon", "harvest", "Harvest 3 moonmelons", targetCropType="moonmelon",
                              targetCropQuantity=3)],
     "rewards": {"money": 500},
     "prerequisites": [Q2], "autoStart": True, "repeatable": False},
    {"id": Q_STEW, "name": "Real Ember Stew", "giver": "npc-marigold", "status": "not-started",
     "description": "Bring Marigold 4 emberroot for the Copper Kettle.",
     "objectives": [objective("obj-stew", "collect", "Bring 4 emberroot", targetItemId="crop-emberroot",
                              targetItemQuantity=4)],
     "rewards": {"money": 120, "items": [{"itemId": "food-ember-stew", "quantity": 2}]},
     "autoStart": False, "repeatable": False},
    {"id": Q_FISH, "name": "Something's Biting", "giver": "npc-sela", "status": "not-started",
     "description": "Catch 2 carp in the river below the bridge.",
     "objectives": [objective("obj-fish", "catch", "Catch 2 carp", targetItemId="fish-carp", targetItemQuantity=2)],
     "rewards": {"money": 100, "items": [{"itemId": "fish-catfish", "quantity": 1}]},
     "autoStart": False, "repeatable": False},
    {"id": Q_PIP, "name": "Pip's Treasure Map", "giver": "npc-pip", "status": "not-started",
     "description": "Ask Rook about Pip's map.",
     "objectives": [objective("obj-pip-rook", "talk", "Talk to Rook", targetNPCId="npc-rook")],
     "rewards": {"money": 50, "items": [{"itemId": "gift-flower", "quantity": 2}]},
     "autoStart": False, "repeatable": False},
]

# ------------------------------------------------------------------ events, mail, cutscenes


def outcome(type, **fields):
    o = {"type": type}
    o.update(fields)
    return o


def event(id, name, trigger, outcomes, conditions=(), scene="", subject=None, repeatable=False, minute=None):
    e = {"id": id, "name": name, "sceneId": scene, "trigger": trigger, "conditions": list(conditions),
         "outcomes": outcomes, "active": True, "repeatable": repeatable}
    if subject:
        e["subject"] = subject
    if minute is not None:
        e["minute"] = minute
    return e


project["events"] = [
    event("evt-arrival", "Arrival", "enter", [
        outcome("message", message="Thistledown Farm. Aunt Wren's field is overgrown, but the soil still smells sweet."),
        outcome("setFlag", flagName="arrived"),
    ], scene="scene-farm", conditions=[{"type": "enterTile", "x": 0, "y": 0, "x2": 23, "y2": 15},
                                       flag("arrived", False)]),
    event("evt-pip-asked", "Pip's map quest", "quest", [outcome("setFlag", flagName="pip-asked")], subject=Q_PIP,
          conditions=[quest_is(Q_PIP, "active")]),
    event("evt-mill-fixed", "The mill grinds again", "quest", [
        outcome("setFlag", flagName="mill-fixed"),
        outcome("addShopStock", shopId="shop-vale", itemId="seed-moonmelon"),
        outcome("playSound", soundId="coin"),
        outcome("message", message="Down by the river, the grindstone rumbles back to life. Juniper now sells moonmelon seeds!"),
        outcome("sendMail", mailId="mail-bram-thanks"),
    ], subject=Q2, conditions=[quest_is(Q2, "completed")]),
    event("evt-lantern-night", "Lantern Night", "quest", [outcome("setFlag", flagName="lanterns-lit"),
                                                     outcome("startCutscene", cutsceneId="cut-lantern-night")],
          subject=Q3, conditions=[quest_is(Q3, "completed")]),
    event("evt-moon-glow", "First moonmelon glow", "harvest", [
        outcome("message", message="The moonmelon is cool in your hands, and faintly, unmistakably, glowing."),
    ], subject="crop-moonmelon"),
    event("evt-dusk", "Lanterns at dusk", "time", [
        outcome("message", message="The street lanterns flicker on, one by one."),
    ], scene="scene-square", minute=1140, repeatable=True, conditions=[flag("lanterns-lit")]),
]

project["mail"] = [
    {"id": "mail-wren", "subject": "If you're reading this", "from": "",
     "text": "My dear,\n\nIf this letter found you, then the farm is yours. Don't fuss over the weeds. "
             "Plant emberroot first, it's forgiving. And when the time is right, plant moonmelons: "
             "Brambleford has always lit its lanterns with them.\n\nTell Odile I said she still owes me a pie.\n\n"
             "All my love,\nAunt Wren",
     "attachments": [{"itemId": "seed-emberroot", "quantity": 4}], "money": 100, "outcomes": [],
     "conditions": [{"type": "dayRange", "minDay": 2, "maxDay": 2}], "repeat": False},
    {"id": "mail-bram-thanks", "subject": "The wheel and the stone", "from": "npc-bram",
     "text": "Farmer,\n\nShe grinds! First proper sack of seed meal in a year. Juniper's got the moonmelon seed. "
             "Come by for flour any time. Bring stones. I like stones.\n\nBram",
     "attachments": [{"itemId": "seed-moonmelon", "quantity": 2}], "money": 0, "outcomes": [], "conditions": [],
     "repeat": False},
    {"id": "mail-pip-birthday", "subject": "IT IS MY BIRTHDAY", "from": "npc-pip",
     "text": "Hi. It is my birthday today. That is all. Eggs are a good present. Frogs are a better present. - Pip",
     "attachments": [], "money": 0, "outcomes": [],
     "conditions": [{"type": "dayRange", "minDay": 5, "maxDay": 5}], "repeat": False},
]


def say(speaker, text, expression=None):
    s = {"type": "say", "speaker": speaker, "text": text, "options": [], "conditions": [], "outcomes": []}
    if expression:
        s["expression"] = expression
    return s


def step(type, **fields):
    s = {"type": type, "options": [], "conditions": [], "outcomes": []}
    s.update(fields)
    return s


GATHER = {"npc-odile": (12, 4), "npc-juniper": (14, 4), "npc-bram": (10, 5), "npc-marigold": (15, 5),
          "npc-pip": (11, 9), "npc-sela": (16, 9), "npc-rook": (17, 7)}

project["cutscenes"] = [
    {"id": "cut-lantern-night", "name": "Lantern Night", "skippable": True, "steps": [
        step("fade", mode="out", seconds=1.0),
        step("outcomes", outcomes=[outcome("warpPlayer", sceneId="scene-square", x=12, y=8)] +
             [outcome("moveNpc", npcId=npc, sceneId="scene-square", x=x, y=y) for npc, (x, y) in GATHER.items()]),
        step("fade", mode="in", seconds=1.5),
        say("npc-odile", "Everyone! Gather round. For the first time in a year...", "happy"),
        say("npc-odile", "...Brambleford lights its lanterns!", "happy"),
        step("emote", actor="npc-pip", emote="exclaim", seconds=0.8),
        say("npc-pip", "THEY'RE GLOWING. The melons are GLOWING. I'm never sleeping again.", "happy"),
        say("npc-bram", "Mill's grinding, lanterns are lit. Not bad for a farm that was all weeds a week ago."),
        say("npc-marigold", "Ember Stew for everyone! Bram, put that third bowl down.", "happy"),
        step("emote", actor="npc-sela", emote="music", seconds=0.8),
        say("npc-sela", "Dad would have liked this. He always said the river looks best with lights on it."),
        say("npc-juniper", "I'm going to need so many more moonmelon seeds.", "happy"),
        say("npc-rook", "And that, friends, is a story I'll be telling on every road from here to the sea.", "happy"),
        step("emote", actor="npc-odile", emote="heart", seconds=1.0),
        say("npc-odile", "Wren would be so proud of you. Welcome home, {player}.", "happy"),
        step("outcomes", outcomes=[outcome("setFlag", flagName="lanterns-lit"),
                                   outcome("modifyFriendship", npcId="npc-odile", amount=100)]),
        say(None, "The End... of the beginning. Thank you for playing Brambleford! "
                  "The farm is yours to keep growing."),
        step("end"),
    ]},
    {"id": "cut-sela-boat", "name": "Sela's boat", "skippable": True, "steps": [
        step("face", actor="npc-sela", direction="left"),
        say("npc-sela", "Can I tell you something? See that bend? Dad's little boat sank right there."),
        say("npc-sela", "Everyone laughed. He laughed loudest. Then he built another one.", "happy"),
        step("emote", actor="npc-sela", emote="heart", seconds=1.0),
        say("npc-sela", "You remind me of him a bit. You just keep planting.", "happy"),
        step("end"),
    ]},
]
for s in project["cutscenes"][0]["steps"] + project["cutscenes"][1]["steps"]:
    if s.get("speaker") is None:
        s.pop("speaker", None)

project["heartEvents"] = [
    {"id": "heart-sela-2", "name": "Sela's boat", "npcId": "npc-sela", "hearts": 2, "cutsceneId": "cut-sela-boat",
     "conditions": []},
]

project["gamePanels"] = [
    {"id": "panel-brambleford", "title": "Brambleford", "entries": [
        {"label": "Mill grinding", "kind": "flag", "value": "mill-fixed"},
        {"label": "Lanterns lit", "kind": "flag", "value": "lanterns-lit"},
        {"label": "Moonmelons", "kind": "item", "value": "crop-moonmelon"},
    ]},
]

# ------------------------------------------------------------------ calendar, animals, player

project["settings"]["calendar"]["festivals"] = [
    {"id": "festival-lantern-night", "name": "Lantern Night", "seasonId": "spring", "day": 14},
]
project["mine"]["enabled"] = False
project["animals"] = [
    {"id": f"hen-{name.lower()}", "speciesId": "animal-chicken", "name": name, "sceneId": "scene-farm", "x": x,
     "y": 12, "mood": 70, "fedToday": False, "pettedToday": False, "ageDays": 30, "daysSinceProduct": 0,
     "productReady": False}
    for name, x in (("Clementine", 17), ("Biscuit", 19))
]

START = [("seed-emberroot", 10), ("tool-hoe", 1), ("tool-watering-can", 1), ("tool-axe", 1), ("tool-pickaxe", 1),
         ("tool-scythe", 1), ("tool-fishing-rod", 1), ("feed-hay", 5)]
player = project["player"]
player.update({"x": 12, "y": 5, "direction": "down", "sceneId": "scene-farm", "money": 200,
               "inventory": [{"item": copy.deepcopy(ITEMS[i]), "quantity": q} for i, q in START],
               "activeQuests": [], "completedQuests": []})

project["export"] = {
    "title": "Brambleford", "executableName": "Brambleford", "version": "1.0.0",
    "gameId": "com.tractr-examples.brambleford", "author": "Tractr example", "icon": "art-icon-brambleford",
    "window": {"width": 1280, "height": 800}, "pixelScale": "integer",
    "credits": "A Tractr demo game. Art drawn with Vixl.", "targets": ["web", "linux-x64", "windows-x64"],
}

add_asset("icon-brambleford", "art", "Game icon")
project["customAssets"] = ASSETS
OUT.write_text(json.dumps(project, indent=1))
print(f"wrote {OUT} ({len(NPCS)} villagers, {len(project['scenes'])} scenes, {len(ASSETS)} art assets)")
