namespace FarmEngine.Schemas

open System
open FarmEngine.Authoring

// The schema's enums (string constants, as zod string enums stay open strings in JSON) and the
// constants and small helpers of packages/engine-schemas that are not records.

/// TS `TileTypeSchema`.
[<RequireQualifiedAccess>]
module TileTypes =
    [<Literal>]
    let Grass = "grass"

    [<Literal>]
    let Soil = "soil"

    [<Literal>]
    let Water = "water"

    [<Literal>]
    let Path = "path"

    [<Literal>]
    let Wall = "wall"

    [<Literal>]
    let Door = "door"

    [<Literal>]
    let Floor = "floor"

    let All : string list = [ Grass; Soil; Water; Path; Wall; Door; Floor ]

/// The tile layers a terrain is painted on (`TerrainDefinition.Layer`).
[<RequireQualifiedAccess>]
module TileLayers =
    [<Literal>]
    let Background = "background"

    [<Literal>]
    let Overlay = "overlay"

    [<Literal>]
    let Object = "object"

    let All : string list = [ Background; Overlay; Object ]

/// What a tool does (`ToolDefinition.Action`).
[<RequireQualifiedAccess>]
module ToolActions =
    [<Literal>]
    let Water = "water"

    [<Literal>]
    let Till = "till"

    [<Literal>]
    let Harvest = "harvest"

    [<Literal>]
    let Fish = "fish"

    [<Literal>]
    let Break = "break"

    [<Literal>]
    let Transform = "transform"

    [<Literal>]
    let Place = "place"

    [<Literal>]
    let Plant = "plant"

    /// Hits the monsters ahead (a weapon, M7): the item's `ToolPower` is the damage.
    [<Literal>]
    let Attack = "attack"

    let All : string list = [ Water; Till; Harvest; Fish; Break; Transform; Place; Plant; Attack ]

    /// The actions a terrain transform can name.
    let Transforming : string list = [ Till; Transform; Break ]

/// The light a crop needs (`GrowthRequirements.Light`).
[<RequireQualifiedAccess>]
module LightNeeds =
    [<Literal>]
    let Light = "light"

    [<Literal>]
    let Dark = "dark"

    let All : string list = [ Light; Dark ]

/// What a neighbour requirement counts (`NeighborRequirement.Kind`).
[<RequireQualifiedAccess>]
module NeighborKinds =
    [<Literal>]
    let Crop = "crop"

    [<Literal>]
    let Terrain = "terrain"

    [<Literal>]
    let Water = "water"

    let All : string list = [ Crop; Terrain; Water ]

/// How a weather overlay draws (`WeatherOverlayDefinition.Style`).
[<RequireQualifiedAccess>]
module OverlayStyles =
    [<Literal>]
    let Rain = "rain"

    [<Literal>]
    let Snow = "snow"

    [<Literal>]
    let Dust = "dust"

    [<Literal>]
    let None = "none"

    let All : string list = [ Rain; Snow; Dust; None ]

/// What a cutscene step does (`CutsceneStep.Type`, M2).
[<RequireQualifiedAccess>]
module CutsceneStepTypes =
    [<Literal>]
    let Say = "say"

    [<Literal>]
    let Choice = "choice"

    [<Literal>]
    let Jump = "jump"

    [<Literal>]
    let Move = "move"

    [<Literal>]
    let Face = "face"

    [<Literal>]
    let Emote = "emote"

    [<Literal>]
    let Wait = "wait"

    [<Literal>]
    let Camera = "camera"

    [<Literal>]
    let Fade = "fade"

    [<Literal>]
    let Music = "music"

    [<Literal>]
    let Sound = "sound"

    [<Literal>]
    let Outcomes = "outcomes"

    [<Literal>]
    let End = "end"

    let All : string list = [ Say; Choice; Jump; Move; Face; Emote; Wait; Camera; Fade; Music; Sound; Outcomes; End ]

/// The emotes a cutscene can show over a character (`CutsceneStep.Emote`).
[<RequireQualifiedAccess>]
module Emotes =
    let All : string list = [ "exclaim"; "question"; "heart"; "music"; "anger"; "sweat"; "sleep"; "idea" ]

/// TS `DirectionSchema`.
[<RequireQualifiedAccess>]
module Directions =
    [<Literal>]
    let Up = "up"

    [<Literal>]
    let Down = "down"

    [<Literal>]
    let Left = "left"

    [<Literal>]
    let Right = "right"

    let All : string list = [ Up; Down; Left; Right ]

/// TS `ItemTypeSchema`.
[<RequireQualifiedAccess>]
module ItemTypes =
    [<Literal>]
    let Seed = "seed"

    [<Literal>]
    let Tool = "tool"

    [<Literal>]
    let Crop = "crop"

    [<Literal>]
    let Gift = "gift"

    [<Literal>]
    let Quest = "quest"

    [<Literal>]
    let Fertilizer = "fertilizer"

    [<Literal>]
    let Material = "material"

    [<Literal>]
    let Fish = "fish"

    let All : string list = [ Seed; Tool; Crop; Gift; Quest; Fertilizer; Material; Fish ]

/// TS `ToolTypeSchema`.
[<RequireQualifiedAccess>]
module ToolTypes =
    [<Literal>]
    let WateringCan = "watering-can"

    [<Literal>]
    let Hoe = "hoe"

    [<Literal>]
    let Axe = "axe"

    [<Literal>]
    let Pickaxe = "pickaxe"

    [<Literal>]
    let Scythe = "scythe"

    [<Literal>]
    let FishingRod = "fishing-rod"

    let All : string list = [ WateringCan; Hoe; Axe; Pickaxe; Scythe; FishingRod ]

/// TS `EditorModeSchema`.
[<RequireQualifiedAccess>]
module EditorModes =
    [<Literal>]
    let Tiles = "tiles"

    [<Literal>]
    let Npcs = "npcs"

    [<Literal>]
    let Items = "items"

    [<Literal>]
    let Events = "events"

    [<Literal>]
    let Play = "play"

    [<Literal>]
    let Quests = "quests"

    let All : string list = [ Tiles; Npcs; Items; Events; Play; Quests ]

/// TS `QuestStatusSchema`.
[<RequireQualifiedAccess>]
module QuestStatuses =
    [<Literal>]
    let NotStarted = "not-started"

    [<Literal>]
    let Active = "active"

    [<Literal>]
    let Completed = "completed"

    [<Literal>]
    let Failed = "failed"

    let All : string list = [ NotStarted; Active; Completed; Failed ]

/// TS `QuestObjectiveTypeSchema`.
[<RequireQualifiedAccess>]
module QuestObjectiveTypes =
    [<Literal>]
    let Collect = "collect"

    [<Literal>]
    let Harvest = "harvest"

    [<Literal>]
    let Talk = "talk"

    [<Literal>]
    let Visit = "visit"

    [<Literal>]
    let Craft = "craft"

    [<Literal>]
    let Gift = "gift"

    let All : string list = [ Collect; Harvest; Talk; Visit; Craft; Gift ]

/// TS `SoilStateSchema`.
[<RequireQualifiedAccess>]
module SoilStates =
    [<Literal>]
    let Dry = "dry"

    [<Literal>]
    let Watered = "watered"

    [<Literal>]
    let Fertilized = "fertilized"

    [<Literal>]
    let Tilled = "tilled"

    let All : string list = [ Dry; Watered; Fertilized; Tilled ]

/// TS `CropQualitySchema`.
[<RequireQualifiedAccess>]
module CropQualities =
    [<Literal>]
    let Normal = "normal"

    [<Literal>]
    let Silver = "silver"

    [<Literal>]
    let Gold = "gold"

    [<Literal>]
    let Iridium = "iridium"

    let All : string list = [ Normal; Silver; Gold; Iridium ]

/// TS `CropMutationSchema`: one of these or `null` (so C# fields holding it are `string?` written as `null`).
[<RequireQualifiedAccess>]
module CropMutations =
    [<Literal>]
    let Giant = "giant"

    [<Literal>]
    let Golden = "golden"

    [<Literal>]
    let Ancient = "ancient"

    let All : string list = [ Giant; Golden; Ancient ]

/// TS `NPCSchema.movePattern` enum.
[<RequireQualifiedAccess>]
module NpcMovePatterns =
    [<Literal>]
    let Stationary = "stationary"

    [<Literal>]
    let Wander = "wander"

    [<Literal>]
    let Patrol = "patrol"

    let All : string list = [ Stationary; Wander; Patrol ]

/// TS `CustomAssetSchema.type` enum.
[<RequireQualifiedAccess>]
module CustomAssetTypes =
    [<Literal>]
    let Tile = "tile"

    [<Literal>]
    let Npc = "npc"

    [<Literal>]
    let Item = "item"

    [<Literal>]
    let Player = "player"

    [<Literal>]
    let Art = "art"

    /// A sound effect (OGG or WAV) that `playSound` plays (M2).
    [<Literal>]
    let Sound = "sound"

    /// Music (OGG or WAV) that scenes, seasons, weather, festivals and events pick (M2).
    [<Literal>]
    let Music = "music"

    let All : string list = [ Tile; Npc; Item; Player; Art; Sound; Music ]

/// TS `EventOutcomeSchema.type` enum.
[<RequireQualifiedAccess>]
module EventOutcomeTypes =
    [<Literal>]
    let Message = "message"

    [<Literal>]
    let ModifyFriendship = "modifyFriendship"

    [<Literal>]
    let ModifyEnergy = "modifyEnergy"

    [<Literal>]
    let WaterArea = "waterArea"

    [<Literal>]
    let GiveItem = "giveItem"

    [<Literal>]
    let TakeItem = "takeItem"

    [<Literal>]
    let GiveMoney = "giveMoney"

    [<Literal>]
    let TakeMoney = "takeMoney"

    [<Literal>]
    let SetFlag = "setFlag"

    [<Literal>]
    let ClearFlag = "clearFlag"

    [<Literal>]
    let StartQuest = "startQuest"

    [<Literal>]
    let CompleteQuest = "completeQuest"

    [<Literal>]
    let SpawnNpc = "spawnNPC"

    [<Literal>]
    let RemoveNpc = "removeNPC"

    [<Literal>]
    let ChangeTile = "changeTile"

    [<Literal>]
    let WarpPlayer = "warpPlayer"

    [<Literal>]
    let StartDialogue = "startDialogue"

    [<Literal>]
    let LockTransition = "lockTransition"

    [<Literal>]
    let UnlockTransition = "unlockTransition"

    [<Literal>]
    let PlaySound = "playSound"

    /// Run a creator-defined action (extensibility layer).
    [<Literal>]
    let PerformAction = "performAction"

    /// Open a declared minigame; its score resolves via the command log.
    [<Literal>]
    let StartMinigame = "startMinigame"

    /// Play a music asset until the scene changes (`soundId`; empty: back to the game's music).
    [<Literal>]
    let PlayMusic = "playMusic"

    /// Play a cutscene (`cutsceneId`, M2).
    [<Literal>]
    let StartCutscene = "startCutscene"

    /// Set a variable to a number (`amount`) or a text (`text`, M3).
    [<Literal>]
    let SetVariable = "setVariable"

    /// Add `amount` to a number variable (M3).
    [<Literal>]
    let AddVariable = "addVariable"

    /// Put an NPC on a tile (M3).
    [<Literal>]
    let MoveNpc = "moveNpc"

    /// Change today's weather (M3).
    [<Literal>]
    let SetWeather = "setWeather"

    /// Give `amount` experience in a skill (M3).
    [<Literal>]
    let GiveExperience = "giveExperience"

    /// Add an item to a shop (M3): `amount` is its price (absent: the item's), `itemQuantity` a daily limit.
    [<Literal>]
    let AddShopStock = "addShopStock"

    /// Stop a shop selling an item (M3).
    [<Literal>]
    let RemoveShopStock = "removeShopStock"

    /// Put a gathering node on a tile (M3).
    [<Literal>]
    let PlaceNode = "placeNode"

    /// Clear the node on a tile (M3).
    [<Literal>]
    let RemoveNode = "removeNode"

    /// Put a machine on a tile (M3).
    [<Literal>]
    let PlaceMachine = "placeMachine"

    /// Clear the machine on a tile (M3).
    [<Literal>]
    let RemoveMachine = "removeMachine"

    /// Change the player's relationship with an NPC (`status`, M4).
    [<Literal>]
    let SetRelationship = "setRelationship"

    /// Send a letter (`mailId`, M6): it arrives the next morning.
    [<Literal>]
    let SendMail = "sendMail"

    /// Restore (or, negative, take) `amount` health (M7).
    [<Literal>]
    let ModifyHealth = "modifyHealth"

    let All : string list = [ Message; ModifyFriendship; ModifyEnergy; WaterArea; GiveItem; TakeItem; GiveMoney; TakeMoney; SetFlag; ClearFlag; StartQuest; CompleteQuest; SpawnNpc; RemoveNpc; ChangeTile; WarpPlayer; StartDialogue; LockTransition; UnlockTransition; PlaySound; PerformAction; StartMinigame; PlayMusic; StartCutscene; SetVariable; AddVariable; MoveNpc; SetWeather; GiveExperience; AddShopStock; RemoveShopStock; PlaceNode; RemoveNode; PlaceMachine; RemoveMachine; SetRelationship; SendMail; ModifyHealth ]

/// TS `EventTriggerSchema`.
[<RequireQualifiedAccess>]
module EventTriggers =
    [<Literal>]
    let Enter = "enter"

    [<Literal>]
    let Interact = "interact"

    [<Literal>]
    let Tick = "tick"

    /// Each morning, after the night (M3).
    [<Literal>]
    let DayStart = "dayStart"

    /// When the clock reaches the event's `Minute` (M3).
    [<Literal>]
    let Time = "time"

    /// A crop was harvested (subject: the crop type or the item, M3).
    [<Literal>]
    let Harvest = "harvest"

    /// An item was sold (subject: the item, M3).
    [<Literal>]
    let Sell = "sell"

    /// A recipe was crafted (subject: the recipe or an item it made, M3).
    [<Literal>]
    let Craft = "craft"

    /// A fish was caught (subject: the item, M3).
    [<Literal>]
    let Catch = "catch"

    /// An item was picked up from the ground (subject: the item, M3).
    [<Literal>]
    let PickUp = "pickUp"

    /// Friendship with an NPC changed (subject: the NPC, M3).
    [<Literal>]
    let Friendship = "friendship"

    /// A quest started or was completed (subject: the quest, M3).
    [<Literal>]
    let Quest = "quest"

    /// A building was finished or upgraded (subject: the building kind, M5).
    [<Literal>]
    let Build = "build"

    /// A bundle was completed (subject: the bundle, M6).
    [<Literal>]
    let Bundle = "bundle"

    /// An achievement was unlocked (subject: the achievement, M6).
    [<Literal>]
    let Achievement = "achievement"

    /// A monster was slain (subject: the monster kind, M7).
    [<Literal>]
    let Slay = "slay"

    let All : string list = [ Enter; Interact; Tick; DayStart; Time; Harvest; Sell; Craft; Catch; PickUp; Friendship; Quest; Build; Bundle; Achievement; Slay ]

    /// The triggers whose events may name a `Subject` (M3, M5–M7).
    let WithSubject : string list = [ Harvest; Sell; Craft; Catch; PickUp; Friendship; Quest; Build; Bundle; Achievement; Slay ]

/// How a `variable` condition compares (M3).
[<RequireQualifiedAccess>]
module VariableComparisons =
    [<Literal>]
    let Eq = "eq"

    [<Literal>]
    let Ne = "ne"

    [<Literal>]
    let Lt = "lt"

    [<Literal>]
    let Le = "le"

    [<Literal>]
    let Gt = "gt"

    [<Literal>]
    let Ge = "ge"

    let All : string list = [ Eq; Ne; Lt; Le; Gt; Ge ]

/// A relationship's stage (M4).
[<RequireQualifiedAccess>]
module RelationshipStatuses =
    [<Literal>]
    let Single = "single"

    [<Literal>]
    let Dating = "dating"

    [<Literal>]
    let Engaged = "engaged"

    [<Literal>]
    let Married = "married"

    let All : string list = [ Single; Dating; Engaged; Married ]

/// What a placed object does to the tiles around it (M5).
[<RequireQualifiedAccess>]
module AreaEffects =
    /// Each morning the soil in the area is watered (a sprinkler).
    [<Literal>]
    let Water = "water"

    /// Overnight, crops in the area escape storm damage and pests (a scarecrow).
    [<Literal>]
    let Protect = "protect"

    let All : string list = [ Water; Protect ]

/// How an item joins a collection (M6).
[<RequireQualifiedAccess>]
module CollectionKinds =
    /// The first time the player holds it.
    [<Literal>]
    let Found = "found"

    /// The first time a shipping bin sells it.
    [<Literal>]
    let Shipped = "shipped"

    /// When the player hands one in (`donateItem`; it is taken).
    [<Literal>]
    let Donated = "donated"

    let All : string list = [ Found; Shipped; Donated ]

/// How a monster moves (M7).
[<RequireQualifiedAccess>]
module MonsterMovements =
    /// Ambles about.
    [<Literal>]
    let Wander = "wander"

    /// Wanders until the player comes within `sight`, then comes at them.
    [<Literal>]
    let Chase = "chase"

    /// Stays where it is (a turret, a slime that waits).
    [<Literal>]
    let Still = "still"

    let All : string list = [ Wander; Chase; Still ]

/// The places clothing is worn (M4).
[<RequireQualifiedAccess>]
module WearSlots =
    [<Literal>]
    let Hat = "hat"

    [<Literal>]
    let Shirt = "shirt"

    [<Literal>]
    let Pants = "pants"

    let All : string list = [ Hat; Shirt; Pants ]

/// TS `GiftReaction` (`keyof typeof GIFT_FRIENDSHIP_DELTAS`).
[<RequireQualifiedAccess>]
module GiftReactions =
    [<Literal>]
    let Loved = "loved"

    [<Literal>]
    let Liked = "liked"

    [<Literal>]
    let Neutral = "neutral"

    [<Literal>]
    let Disliked = "disliked"

    [<Literal>]
    let Hated = "hated"

    let All : string list = [ Loved; Liked; Neutral; Disliked; Hated ]

/// TS `GamePanelSchema.entries[].kind` enum.
[<RequireQualifiedAccess>]
module GamePanelEntryKinds =
    [<Literal>]
    let Text = "text"

    [<Literal>]
    let Money = "money"

    [<Literal>]
    let Energy = "energy"

    [<Literal>]
    let Day = "day"

    [<Literal>]
    let Item = "item"

    [<Literal>]
    let Flag = "flag"

    [<Literal>]
    let Action = "action"

    let All : string list = [ Text; Money; Energy; Day; Item; Flag; Action ]


/// `primitives.ts` lists.
[<RequireQualifiedAccess>]
module PrimitivesSchema =
    /// Seasons are open strings (creator-defined calendars, M9) so custom season ids validate the
    /// same way custom crops do. `CLASSIC_SEASONS` enumerates the four seasons the built-in
    /// calendar ships with, for defaults/editors.
    let ClassicSeasons : string list = [ "spring"; "summer"; "fall"; "winter" ]

    /// Crop identifiers are open strings so that custom/modded crops share the same pipeline as
    /// built-in ones. `BUILTIN_CROP_IDS` enumerates the crops shipped with the default content pack.
    let BuiltinCropIds : string list =
        [ "wheat"; "corn"; "tomato"; "carrot"; "potato"; "strawberry"; "pumpkin"; "cauliflower"; "blueberry" ]

[<RequireQualifiedAccess>]
module SettingsSchema =
    let ClassicCalendarSeasons () : CalendarSeason list =
        PrimitivesSchema.ClassicSeasons
        |> List.map (fun id -> { Id = id; Name = id.Substring(0, 1).ToUpperInvariant() + id.Substring 1; Days = 28.0; Temperature = None; Music = None })

    let DefaultProjectSettings : ProjectSettings = ProjectSettings.Default

    /// The shortest day a project may configure (`dayEndMinute - dayStartMinute`).
    let MinDayWindowMinutes = 60.0
    /// The latest `dayEndMinute` the engine's clock can reach (71:34; Rust `MAX_DAY_END_MINUTE`).
    let MaxDayEndMinute = 4294.0
    /// The fastest clock a project may configure (a whole day per real second).
    let MaxMinutesPerRealSecond = 1440.0

    let private isWhole (value: float) = not (Double.IsNaN value || Double.IsInfinity value) && Math.Truncate value = value

    /// Rust `game_time::is_valid_time_config`: a day the clock can run through. A start at or
    /// after the end collapsed the player on every tick; an end past 71:34 never came.
    let validTime (time: TimeConfig) : bool =
        isWhole time.DayStartMinute && isWhole time.DayEndMinute && time.DayStartMinute >= 0.0
        && time.DayEndMinute - time.DayStartMinute >= MinDayWindowMinutes
        && time.DayEndMinute <= MaxDayEndMinute
        && time.MinutesPerRealSecond > 0.0 && time.MinutesPerRealSecond <= MaxMinutesPerRealSecond

    /// Rust `game_time::calendar_seasons`: the positive-length seasons, a repeated id once (at
    /// its first place), or the classic calendar when none is left.
    let calendarSeasons (calendar: CalendarConfig) : CalendarSeason list =
        match calendar.Seasons |> List.filter (fun season -> season.Days > 0.0) |> List.distinctBy (fun season -> season.Id) with
        | [] -> ClassicCalendarSeasons ()
        | seasons -> seasons

    /// Rust `game_time::natural_date`: the season and day of season an absolute (1-based) day
    /// falls on in a game that started on day 1 of the first season.
    let naturalDate (calendar: CalendarConfig) (absoluteDay: float) : CalendarSeason * float =
        let seasons = calendarSeasons calendar
        let yearLength = seasons |> List.sumBy (fun season -> season.Days)
        let dayInYear = ((absoluteDay - 1.0) % yearLength + yearLength) % yearLength
        let rec find (remaining: float) (rest: CalendarSeason list) =
            match rest with
            | [ last ] when remaining >= last.Days -> last, last.Days
            | season :: tail when remaining >= season.Days -> find (remaining - season.Days) tail
            | season :: _ -> season, remaining + 1.0
            | [] -> List.last seasons, (List.last seasons).Days
        find dayInYear seasons

    /// Rust `game_time::clock_date`'s day: `dayOfSeason` (or, when absent, where the absolute day
    /// falls in its season), clamped to the length of `season` (an unknown season counts as the
    /// first).
    let clockDayOfSeason (calendar: CalendarConfig) (season: string) (absoluteDay: float) (dayOfSeason: float option) : float =
        let seasons = calendarSeasons calendar
        let current = seasons |> List.tryFind (fun s -> s.Id = season) |> Option.defaultValue seasons.Head
        let day =
            match dayOfSeason with
            | Some day when day <> 0.0 -> day
            | _ -> snd (naturalDate calendar absoluteDay)
        max 1.0 (min current.Days day)

/// Limits of M5's placed objects and buildings (Rust `schema::farm`).
[<RequireQualifiedAccess>]
module FarmSchema =
    /// The largest building side the engine places, in tiles (`MAX_BUILDING_SIZE`).
    [<Literal>]
    let MaxBuildingSize = 16.0

    /// The largest area effect radius the engine applies (`MAX_AREA_RADIUS`).
    [<Literal>]
    let MaxAreaRadius = 32.0

    /// The slots a shipping bin holds when its type sets no `storage` (`DEFAULT_SHIPPING_SLOTS`).
    [<Literal>]
    let DefaultShippingSlots = 24.0

    /// The largest storage an object has (`MAX_STORAGE_SLOTS`).
    [<Literal>]
    let MaxStorageSlots = 256.0

[<RequireQualifiedAccess>]
module ProjectSchema =
    /// The project schema version. Only this version loads (there are no migrations): bump it
    /// when the persisted shape changes, so files of another shape are refused with a message.
    [<Literal>]
    let CurrentProjectSchemaVersion = 2.0

[<RequireQualifiedAccess>]
module GameContentSchema =
    [<Literal>]
    let CurrentContentVersion = 1.0

[<RequireQualifiedAccess>]
module SaveSchema =
    /// TS `SKILL_NAMES`.
    let SkillNames : string list = [ "farming"; "foraging"; "fishing"; "mining"; "social" ]

    /// Tile index → tile center; already-fractional coordinates pass through.
    let CenterCoordinate (value: float) : float = if value = System.Math.Floor value then value + 0.5 else value

[<RequireQualifiedAccess>]
module SocialSchema =
    [<Literal>]
    let FriendshipPerHeart = 125.0

    [<Literal>]
    let MaxFriendship = 1250.0

    let GiftFriendshipDeltas : (string * float) list =
        [ GiftReactions.Loved, 80.0
          GiftReactions.Liked, 45.0
          GiftReactions.Neutral, 20.0
          GiftReactions.Disliked, -20.0
          GiftReactions.Hated, -40.0 ]

[<RequireQualifiedAccess>]
module EventsSchema =
    /// Flag used to record that a non-repeatable event has fired.
    let EventFiredFlag (eventId: string) : string = "event:" + eventId + ":fired"

[<RequireQualifiedAccess>]
module ExtensibilitySchema =
    [<Literal>]
    let FishingMinigameId = "fishing"

[<RequireQualifiedAccess>]
module MigrationsSchema =
    let private entry (weatherId: string) (weight: float) : WeatherTableEntry = { WeatherId = weatherId; Weight = weight }

    let private weather (id: string) (name: string) (waters: bool) (damage: float) (inside: bool) (overlay: string option) : WeatherTypeDefinition =
        { Id = id; Name = name; WatersOutdoorSoil = waters; CropDamageChance = damage; NpcsStayInside = inside; Overlay = overlay; Music = None; Extra = [] }

    /// Default weather set every pre-v6 project receives (tunable afterwards).
    let DefaultWeatherConfig () : WeatherConfig =
        { Types =
            [ weather "sun" "Sunny" false 0.0 false None
              weather "rain" "Rain" true 0.0 false (Some "rain")
              weather "storm" "Storm" true 0.03 true (Some "rain")
              weather "snow" "Snow" false 0.0 false (Some "snow") ]
          Table =
            [ "spring", [ entry "sun" 6.0; entry "rain" 3.0; entry "storm" 1.0 ]
              "summer", [ entry "sun" 7.0; entry "rain" 1.0; entry "storm" 2.0 ]
              "fall", [ entry "sun" 6.0; entry "rain" 3.0; entry "storm" 1.0 ]
              "winter", [ entry "sun" 5.0; entry "snow" 5.0 ] ]
          Overlays = None
          Extra = [] }
