namespace FarmEngine.Schemas

open FarmEngine.Authoring

// Generated once from the C# records of FarmEngine.Schemas (a port of packages/engine-schemas), then
// kept by hand. Numbers are `float` in authoring units (tiles, points, minutes; docs/NUMERICS.md):
// project JSON keeps them, and the Rust engine converts them to fixed-unit integers when it loads
// a cartridge. `option` is an optional or nullable field; `Extra` keeps the keys the schema does
// not declare, in order.
//
// JSON: SchemaJson.fs is generated from this file (tools/codegen/schema-json.fsx; run it after
// changing a record or union here). It follows from the types, the `Default` members (a missing
// key decodes to the field's value in `Default`) and the `[<Json…>]` attributes below, which say
// what the types do not.
//
// A record without `Extra` drops keys it does not declare when it loads. A change to what a
// project stores bumps the schema version (`ProjectSchema.CurrentProjectSchemaVersion`).
//
// The Rust engine has its own serde types for the same JSON (crates/farm-sim/src/schema). The
// parity tests keep the two in step: ParityTests.fs records an F# encoding of every field of every
// record (fixtures/parity), which crates/farm-sim/tests/fsharp_parity.rs reads back through serde.

/// The JSON key of a record field, when it is not the field name in camelCase (`selectedNPCId`).
/// Read by tools/codegen/schema-json.fsx; nothing reads it at run time.
[<System.AttributeUsage(System.AttributeTargets.Property)>]
type JsonKeyAttribute(key: string) =
    inherit System.Attribute()
    /// The JSON key.
    member _.Key = key

/// An `option` field zod declares `.nullable()` (a required key): `None` is written as `null`
/// instead of being left out. Read by tools/codegen/schema-json.fsx.
[<System.AttributeUsage(System.AttributeTargets.Property)>]
type JsonNullAttribute() =
    inherit System.Attribute()

/// The decoder of a record field, as F# source, when it is not the one the field's type gives:
/// an expression of the field's type over `key`, `path` (the record's path) and `value` (the
/// member's JSON). Read by tools/codegen/schema-json.fsx; the encoder still follows the type.
[<System.AttributeUsage(System.AttributeTargets.Property)>]
type JsonDecodeAttribute(expression: string) =
    inherit System.Attribute()
    /// The decoder expression.
    member _.Expression = expression

type ActionDef =
    {
        Id: string
        Name: string
        Description: string
        /// All must hold for the action to run (same vocabulary as events).
        Conditions: EventCondition list
        /// Shown when a condition fails; silent when empty.
        FailMessage: string
        /// Applied in order when the action runs (same vocabulary as events).
        Outcomes: EventOutcome list
        /// Energy spent on a successful run (respects the energy toggle). nonnegative.
        EnergyCost: float
        /// Optional single-character play-mode hotkey (max length 1). Reserved gameplay keys (movement/tools/panels) are ignored by hosts.
        Hotkey: string option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `ActionDef` with every field at its schema default.
    static member Default : ActionDef =
        { Id = ""; Name = ""; Description = ""; Conditions = []; FailMessage = ""; Outcomes = []; EnergyCost = 0.0; Hotkey = None; Extra = [] }

// ---- Building the farm (M5, Rust schema/farm.rs) ----

/// What a placed object does to the square of tiles `Radius` around it, in its scene (M5).
and AreaEffect =
    {
        /// One of `AreaEffects`: `water` (a sprinkler) or `protect` (a scarecrow).
        Effect: string
        /// Tiles in each direction (1: the 3×3 square around the object). int, 0..32.
        Radius: float
    }

    /// A `AreaEffect` with every field at its schema default.
    static member Default : AreaEffect =
        { Effect = "water"; Radius = 1.0 }

/// A building a builder puts up (M5): a footprint on a farm scene with a door into an interior
/// scene made from a template.
and BuildingDefinition =
    {
        Id: string
        Name: string
        Description: string
        Visual: VisualRef option
        /// `#rrggbb`: how the game draws it without art.
        Color: string
        /// Footprint in tiles. int, 1..16.
        Width: float
        /// int, 1..16.
        Height: float
        /// The door's tile within the footprint (from its top-left corner); absent: the middle of
        /// the bottom row.
        Door: GridPoint option
        /// The scene each building of this kind gets a copy of as its interior; absent: no
        /// interior (a silo, a well).
        InteriorSceneId: string option
        /// The interior's door: the tile that leads back out; the player arrives on the tile
        /// above it. Absent: the middle of the interior's bottom row.
        InteriorDoor: GridPoint option
        /// What building it costs. nonnegative.
        Cost: float
        Materials: RecipeIngredient list
        /// Nights of building before it stands; 0: done the next morning all the same. int.
        BuildDays: float
        /// Scenes it may be built in; empty: the farm scene (`FarmSettings.SceneId`), else the
        /// start scene.
        Scenes: string list
        /// Animals it houses. int.
        Capacity: float
        /// The building this one is an upgrade of: it is never built new, only by upgrading one of
        /// those (the house upgrades the same way).
        UpgradeOf: string option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `BuildingDefinition` with every field at its schema default.
    static member Default : BuildingDefinition =
        { Id = ""; Name = ""; Description = ""; Visual = None; Color = "#b5651d"; Width = 3.0; Height = 2.0; Door = None; InteriorSceneId = None; InteriorDoor = None; Cost = 0.0; Materials = []; BuildDays = 2.0; Scenes = []; Capacity = 0.0; UpgradeOf = None; Extra = [] }

/// A building on the map (M5), from the project's start (`GameProject.Buildings`) or built in play.
and BuildingState =
    {
        /// Unique among the game's buildings; built ones are `building-N`.
        Id: string
        /// Its `BuildingDefinition`.
        BuildingId: string
        SceneId: string
        /// The footprint's top-left tile. int.
        X: float
        /// int.
        Y: float
        /// The scene behind its door (empty: none yet, or no interior).
        InteriorSceneId: string
        /// Nights of building left; 0: it stands. int.
        DaysLeft: float
        /// The building it becomes when the work under way (`DaysLeft`) is done (an upgrade).
        UpgradeTo: string option
    }

    /// A `BuildingState` with every field at its schema default.
    static member Default : BuildingState =
        { Id = ""; BuildingId = ""; SceneId = ""; X = 0.0; Y = 0.0; InteriorSceneId = ""; DaysLeft = 0.0; UpgradeTo = None }

/// One more product of an animal species (M5) beside its `ProductItemId`.
and AnimalProduct =
    {
        ItemId: string
        /// Days between products. int, positive.
        IntervalDays: float
        /// The mood (0..100) the animal needs for it. int.
        MinMood: float
    }

    /// A `AnimalProduct` with every field at its schema default.
    static member Default : AnimalProduct =
        { ItemId = ""; IntervalDays = 1.0; MinMood = 30.0 }

/// A product quality an animal's mood earns (M5): the best tier whose `MinMood` it has.
and MoodQuality =
    {
        /// int.
        MinMood: float
        /// A crop quality id.
        Quality: string
    }

    /// A `MoodQuality` with every field at its schema default.
    static member Default : MoodQuality =
        { MinMood = 0.0; Quality = "" }

/// How animals of a species have young (M5): each night an adult in a building with room may.
and AnimalBreeding =
    {
        /// Chance per night, 0..1.
        Chance: float
        /// int.
        MinMood: float
    }

    /// A `AnimalBreeding` with every field at its schema default.
    static member Default : AnimalBreeding =
        { Chance = 0.05; MinMood = 70.0 }

/// Progress toward one of `AnimalSpeciesDefinition.Products` (M5).
and ProductProgress =
    {
        /// int.
        Days: float
        Ready: bool
    }

    /// A `ProductProgress` with every field at its schema default.
    static member Default : ProductProgress =
        { Days = 0.0; Ready = false }

/// A farm map a new game can start with (M5).
and FarmLayout =
    {
        Id: string
        Name: string
        Description: string
        /// The scene that becomes the farm.
        SceneId: string
    }

    /// A `FarmLayout` with every field at its schema default.
    static member Default : FarmLayout =
        { Id = ""; Name = ""; Description = ""; SceneId = "" }

/// The farm (M5): its scene, the layouts a new game picks from, and pests.
and FarmSettings =
    {
        /// The farm scene: buildings go here by default, and a layout replaces it.
        SceneId: string
        /// Farm maps a new game picks from (two or more make the character creator ask).
        Layouts: FarmLayout list
        /// Chance per night that pests eat an outdoor crop no object protects, 0..1.
        PestChance: float
    }

    /// A `FarmSettings` with every field at its schema default.
    static member Default : FarmSettings =
        { SceneId = ""; Layouts = []; PestChance = 0.0 }

// ---- Long-term goals (M6, Rust schema/goals.rs) ----

/// A reward a collection gives once it holds `Count` items (M6).
and CollectionReward =
    {
        /// int.
        Count: float
        /// Shown when it is given; empty: none.
        Message: string
        Outcomes: EventOutcome list
    }

    /// A `CollectionReward` with every field at its schema default.
    static member Default : CollectionReward =
        { Count = 0.0; Message = ""; Outcomes = [] }

/// A museum, catalogue or other list of items to find (M6).
and CollectionDefinition =
    {
        Id: string
        Name: string
        Description: string
        Items: string list
        /// One of `CollectionKinds`: how an item joins it.
        Kind: string
        /// Donations are taken only in this scene (the museum); absent: anywhere.
        SceneId: string option
        /// In order of `Count`; each is given once.
        Rewards: CollectionReward list
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `CollectionDefinition` with every field at its schema default.
    static member Default : CollectionDefinition =
        { Id = ""; Name = ""; Description = ""; Items = []; Kind = "found"; SceneId = None; Rewards = []; Extra = [] }

/// A line of a bundle (M6): `Quantity` of an item, of `Quality` or better.
and BundleItem =
    {
        ItemId: string
        /// int.
        Quantity: float
        /// The lowest crop quality it takes; absent: any.
        Quality: string option
    }

    /// A `BundleItem` with every field at its schema default.
    static member Default : BundleItem =
        { ItemId = ""; Quantity = 1.0; Quality = None }

/// A Community Center-style list of items to hand in (M6); done, it runs its outcomes (rewards,
/// flags, an unlocked door: the world fixed).
and BundleDefinition =
    {
        Id: string
        Name: string
        Description: string
        /// The room or board it belongs to (the game lists bundles by group).
        Group: string
        /// Items are handed in only in this scene; absent: anywhere.
        SceneId: string option
        Items: BundleItem list
        /// Lines that complete it; absent: all of them. int.
        Required: float option
        /// Shown when it is complete; empty: a built-in message.
        Message: string
        Outcomes: EventOutcome list
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `BundleDefinition` with every field at its schema default.
    static member Default : BundleDefinition =
        { Id = ""; Name = ""; Description = ""; Group = ""; SceneId = None; Items = []; Required = None; Message = ""; Outcomes = []; Extra = [] }

/// Something to achieve (M6): unlocked the moment its conditions hold.
and AchievementDefinition =
    {
        Id: string
        Name: string
        Description: string
        Conditions: EventCondition list
        /// Shown as "???" until unlocked.
        Hidden: bool
        Visual: VisualRef option
        /// Its id on Steam, for an export with Steamworks (later).
        SteamId: string option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `AchievementDefinition` with every field at its schema default.
    static member Default : AchievementDefinition =
        { Id = ""; Name = ""; Description = ""; Conditions = []; Hidden = false; Visual = None; SteamId = None; Extra = [] }

/// A letter (M6). It arrives in the morning: after a `sendMail` outcome or a quest that sends
/// it, or by itself on a morning its `Conditions` hold (an NPC at some hearts, a birthday).
and MailDefinition =
    {
        Id: string
        Subject: string
        /// The NPC who writes it; empty: no sender (or the text says).
        From: string
        Text: string
        /// Items in the envelope, taken when it is read.
        Attachments: RecipeIngredient list
        Money: float
        /// Run when it is first read.
        Outcomes: EventOutcome list
        /// It arrives by itself on a morning these hold; empty: only when sent.
        Conditions: EventCondition list
        /// It may come again (each morning its conditions hold; a birthday letter every year).
        Repeat: bool
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `MailDefinition` with every field at its schema default.
    static member Default : MailDefinition =
        { Id = ""; Subject = ""; From = ""; Text = ""; Attachments = []; Money = 0.0; Outcomes = []; Conditions = []; Repeat = false; Extra = [] }

// ---- Combat and adventure (M7, Rust schema/combat.rs) ----

/// Player health and fainting (M7). Absent or not `Enabled`: no health, and monsters are not
/// spawned.
and CombatSettings =
    {
        Enabled: bool
        /// Whole points. int, positive.
        MaxHealth: float
        /// Money lost on fainting. nonnegative.
        FaintMoneyPenalty: float
        /// Where the player wakes after fainting; absent: the start scene.
        FaintSceneId: string option
        /// int.
        FaintX: float option
        /// int.
        FaintY: float option
        /// Ticks after a hit during which the player takes no more damage. int.
        HurtTicks: float
    }

    /// A `CombatSettings` with every field at its schema default.
    static member Default : CombatSettings =
        { Enabled = false; MaxHealth = 100.0; FaintMoneyPenalty = 0.0; FaintSceneId = None; FaintX = None; FaintY = None; HurtTicks = 30.0 }

/// A monster (M7).
and MonsterDefinition =
    {
        Id: string
        Name: string
        Visual: VisualRef option
        /// `#rrggbb`: how the game draws it without art.
        Color: string
        /// Hit points. int, positive.
        Health: float
        /// Health the player loses when it touches them. int, nonnegative.
        Damage: float
        /// Taken off every hit it takes (a hit always does at least 1). int, nonnegative.
        Defense: float
        /// Tiles per second.
        Speed: float
        /// One of `MonsterMovements`.
        Movement: string
        /// How near (tiles) the player must come for a chaser to notice them. int.
        Sight: float
        /// Rolled when it is slain, like a node's drops.
        Drops: NodeDrop list
        /// Combat experience for slaying it. int.
        Experience: float
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `MonsterDefinition` with every field at its schema default.
    static member Default : MonsterDefinition =
        { Id = ""; Name = ""; Visual = None; Color = "#6a8f3a"; Health = 10.0; Damage = 5.0; Defense = 0.0; Speed = 2.0; Movement = "chase"; Sight = 5.0; Drops = []; Experience = 5.0; Extra = [] }

/// Monsters a scene has each morning (M7): `Count` of a monster on free tiles of the area
/// (`X..=X2`, `Y..=Y2`; absent ends: the whole scene).
and MonsterSpawn =
    {
        MonsterId: string
        /// int.
        Count: float
        /// int.
        X: float option
        /// int.
        Y: float option
        /// int.
        X2: float option
        /// int.
        Y2: float option
    }

    /// A `MonsterSpawn` with every field at its schema default.
    static member Default : MonsterSpawn =
        { MonsterId = ""; Count = 1.0; X = None; Y = None; X2 = None; Y2 = None }

/// A weighted monster of a mine depth band (M7).
and MineMonsterWeight =
    {
        MonsterId: string
        /// int.
        Weight: float
    }

    /// A `MineMonsterWeight` with every field at its schema default.
    static member Default : MineMonsterWeight =
        { MonsterId = ""; Weight = 0.0 }

/// A hazard terrain a mine depth band scatters over its floors (M7), on the overlay layer.
and MineHazard =
    {
        TerrainId: string
        /// Fraction of free floor tiles, 0..1.
        Density: float
    }

    /// A `MineHazard` with every field at its schema default.
    static member Default : MineHazard =
        { TerrainId = ""; Density = 0.05 }

/// A weighted treasure of a mine depth band (M7): an item left on a floor tile.
and MineTreasure =
    {
        ItemId: string
        /// int.
        Weight: float
    }

    /// A `MineTreasure` with every field at its schema default.
    static member Default : MineTreasure =
        { ItemId = ""; Weight = 0.0 }

// ---- The conditions M5–M7 add ----

/// The player has at least `Min` money (M6).
and MoneyCondition =
    {
        Min: float
    }

    static member Default : MoneyCondition =
        { Min = 0.0 }

/// The player's level in a skill is at least `Min` (M6).
and SkillLevelCondition =
    {
        Skill: string
        /// int.
        Min: float
    }

    static member Default : SkillLevelCondition =
        { Skill = ""; Min = 1.0 }

/// The deepest mine floor reached is at least `Min` (M6).
and MineFloorCondition =
    {
        /// int.
        Min: float
    }

    static member Default : MineFloorCondition =
        { Min = 1.0 }

/// A collection holds at least `Min` items; absent `Min`: all of its items (M6).
and CollectionCondition =
    {
        CollectionId: string
        /// int.
        Min: float option
    }

    static member Default : CollectionCondition =
        { CollectionId = ""; Min = None }

/// A bundle is complete (M6).
and BundleCondition =
    {
        BundleId: string
    }

    static member Default : BundleCondition =
        { BundleId = "" }

/// An achievement is unlocked (M6).
and AchievementCondition =
    {
        AchievementId: string
    }

    static member Default : AchievementCondition =
        { AchievementId = "" }

/// At least `Min` units shipped of an item; an empty `ItemId`: of everything (M6).
and ShippedCondition =
    {
        ItemId: string
        /// int.
        Min: float
    }

    static member Default : ShippedCondition =
        { ItemId = ""; Min = 1.0 }

/// At least `Min` standing buildings of a kind, its upgrades counting (M5).
and BuildingCondition =
    {
        BuildingId: string
        /// int.
        Min: float
    }

    static member Default : BuildingCondition =
        { BuildingId = ""; Min = 1.0 }

/// At least `Min` monsters slain of a kind; an empty `MonsterId`: of any (M7).
and SlainCondition =
    {
        MonsterId: string
        /// int.
        Min: float
    }

    static member Default : SlainCondition =
        { MonsterId = ""; Min = 1.0 }

/// Today is an NPC's birthday; an empty `NpcId`: anyone's (M6).
and BirthdayCondition =
    {
        NpcId: string
    }

    static member Default : BirthdayCondition =
        { NpcId = "" }

and AnimalSpeciesDefinition =
    {
        Id: string
        Name: string
        Visual: VisualRef option
        /// nonnegative.
        PurchaseCost: float
        /// Item consumed daily; absent = grazes for free.
        FeedItemId: string option
        ProductItemId: string
        /// Days between products (when fed and adult). int, positive.
        ProductIntervalDays: float
        /// int, nonnegative.
        DaysToAdult: float
        /// Renderer hint.
        Color: string
        /// Buildings its animals live in (M5, `BuildingDefinition` ids); a building that upgrades
        /// one of them counts too.
        Housing: (string list) option
        /// Products beside `ProductItemId` (M5), each on its own interval.
        Products: (AnimalProduct list) option
        /// The product quality each mood earns (M5); empty: normal quality.
        MoodQualities: (MoodQuality list) option
        /// Young born in buildings with room (M5); absent: none.
        Breeding: AnimalBreeding option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `AnimalSpeciesDefinition` with every field at its schema default.
    static member Default : AnimalSpeciesDefinition =
        { Id = ""; Name = ""; Visual = None; PurchaseCost = 0.0; FeedItemId = None; ProductItemId = ""; ProductIntervalDays = 1.0; DaysToAdult = 3.0; Color = "#e8d8c3"; Housing = None; Products = None; MoodQualities = None; Breeding = None; Extra = [] }

/// A live animal (persisted per project/save).
and AnimalState =
    {
        Id: string
        SpeciesId: string
        Name: string
        SceneId: string
        /// int.
        X: float
        /// int.
        Y: float
        /// 0..100; fed & petted raise it, neglect lowers it.
        Mood: float
        FedToday: bool
        PettedToday: bool
        /// int.
        AgeDays: float
        /// int.
        DaysSinceProduct: float
        /// Product waiting to be collected.
        ProductReady: bool
        /// The building it lives in (M5: a `BuildingState` id).
        BuildingId: string option
        /// Progress toward the species' `Products` (M5), by index.
        MoreProducts: (ProductProgress list) option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `AnimalState` with every field at its schema default.
    static member Default : AnimalState =
        { Id = ""; SpeciesId = ""; Name = ""; SceneId = ""; X = 0.0; Y = 0.0; Mood = 70.0; FedToday = false; PettedToday = false; AgeDays = 0.0; DaysSinceProduct = 0.0; ProductReady = false; BuildingId = None; MoreProducts = None; Extra = [] }

and AnimationClip =
    {
        /// min length 1.
        Name: string
        Loop: bool
        /// 1..1024 frames.
        Frames: ArtFrame list
    }

    /// A `AnimationClip` with every field at its schema default.
    static member Default : AnimationClip =
        { Name = ""; Loop = true; Frames = [] }

/// Rectangles are in source pixels; display size remains independent of art resolution.
and ArtFrame =
    {
        AssetId: string option
        /// int, nonnegative.
        X: float
        /// int, nonnegative.
        Y: float
        /// int, positive.
        Width: float
        /// int, positive.
        Height: float
        /// int, positive.
        Ticks: float
    }

    /// A `ArtFrame` with every field at its schema default.
    static member Default : ArtFrame =
        { AssetId = None; X = 0.0; Y = 0.0; Width = 0.0; Height = 0.0; Ticks = 6.0 }

/// The look of a game's sky (M1): the day/night tint and the foliage tint. Empty or absent parts
/// keep the look below them: a scene's look, then its season's, then the game's, then the built-in.
and AtmosphereLook =
    {
        /// Day/night multiply-tint keyframes, linear between them, by minute of the day; empty
        /// keeps the look below.
        Daylight: DaylightKey list
        /// Multiply tint of trees and weeds (`#rrggbb`); absent keeps the look below.
        FoliageTint: string option
    }

    /// A `AtmosphereLook` with every field at its schema default.
    static member Default : AtmosphereLook =
        { Daylight = []; FoliageTint = None }

/// A game's atmosphere (M1): its look, and a look per season.
and AtmosphereSettings =
    {
        /// The look of every season without its own.
        AllSeasons: AtmosphereLook
        /// Looks by season id.
        Seasons: (string * AtmosphereLook) list
    }

    /// A `AtmosphereSettings` with every field at its schema default.
    static member Default : AtmosphereSettings =
        { AllSeasons = AtmosphereLook.Default; Seasons = [] }

and CalendarConfig =
    {
        /// Ordered list of seasons; the calendar year is the sum of their lengths.
        Seasons: CalendarSeason list
        /// Named festival days, each pinned to a season + day-of-season.
        Festivals: CalendarFestival list
    }

    /// A `CalendarConfig` with every field at its schema default.
    static member Default : CalendarConfig =
        { Seasons = [ ({ Id = "spring"; Name = "Spring"; Days = 28.0; Temperature = None; Music = None } : CalendarSeason); ({ Id = "summer"; Name = "Summer"; Days = 28.0; Temperature = None; Music = None } : CalendarSeason); ({ Id = "fall"; Name = "Fall"; Days = 28.0; Temperature = None; Music = None } : CalendarSeason); ({ Id = "winter"; Name = "Winter"; Days = 28.0; Temperature = None; Music = None } : CalendarSeason) ]; Festivals = [] }

and CalendarFestival =
    {
        Id: string
        Name: string
        /// Season this festival falls in (matches a CalendarSeason id).
        SeasonId: string
        /// 1-based day within that season. int, positive.
        Day: float
        /// A music asset that plays all festival day (M2).
        Music: string option
    }

    /// A `CalendarFestival` with every field at its schema default.
    static member Default : CalendarFestival =
        { Id = ""; Name = ""; SeasonId = ""; Day = 0.0; Music = None }

and CalendarSeason =
    {
        Id: string
        Name: string
        /// In-game days this season lasts. int, positive.
        Days: float
        /// How warm the season is, for crops with a temperature range (M1).
        Temperature: float option
        /// A music asset that plays in the season instead of the game's own music (M2).
        Music: string option
    }

    /// A `CalendarSeason` with every field at its schema default.
    static member Default : CalendarSeason =
        { Id = ""; Name = ""; Days = 0.0; Temperature = None; Music = None }

and ContentPack =
    {
        Manifest: PackManifest
        Content: PackContent
        Plugins: PackPlugin list
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `ContentPack` with every field at its schema default.
    static member Default : ContentPack =
        { Manifest = PackManifest.Default; Content = PackContent.Default; Plugins = []; Extra = [] }

/// A dropped/growing crop instance on a tile (game state, not content).
and Crop =
    {
        Type: string
        /// Absolute in-game day the crop was planted. int.
        PlantedOnDay: float option
        /// Watered in-game days accumulated toward growthDays.
        DaysGrown: float option
        /// Killed by season change; renders dead and can be cleared.
        Withered: bool option
        Stage: float
        Watered: bool
        LastWateredDay: float option
        /// One of `CropQualities`.
        Quality: string
        /// One of `CropMutations` or `null` (required key).
        [<JsonNull>]
        Mutation: string option
        IsMultiTileRoot: bool option
        MultiTileId: string option
        HarvestCount: float
        DaysWithoutWater: float
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `Crop` with every field at its schema default.
    static member Default : Crop =
        { Type = ""; PlantedOnDay = None; DaysGrown = None; Withered = None; Stage = 0.0; Watered = false; LastWateredDay = None; Quality = ""; Mutation = None; IsMultiTileRoot = None; MultiTileId = None; HarvestCount = 0.0; DaysWithoutWater = 0.0; Extra = [] }

and CropDefinition =
    {
        Id: string
        Name: string
        Visual: VisualRef option
        SeedCost: float
        BaseHarvestValue: float
        /// In-game days from planting to maturity. positive; validation requires it (absent or 0
        /// grows in one day).
        GrowthDays: float option
        /// int, positive.
        Stages: float
        Seasons: string list
        /// In-game days between repeat harvests for regrowing crops. positive.
        RegrowthDays: float option
        CanRegrow: bool
        MultiTile: CropMultiTile option
        MutationChance: float option
        YieldMin: float
        YieldMax: float
        /// The item a harvest gives. Absent: the item named `crop-{id}` (the editor's convention).
        HarvestItemId: string option
        /// What it needs beyond water and its seasons (M1).
        Requirements: GrowthRequirements option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `CropDefinition` with every field at its schema default.
    static member Default : CropDefinition =
        { Id = ""; Name = ""; Visual = None; SeedCost = 0.0; BaseHarvestValue = 0.0; GrowthDays = None; Stages = 0.0; Seasons = []; RegrowthDays = None; CanRegrow = false; MultiTile = None; MutationChance = None; YieldMin = 0.0; YieldMax = 0.0; HarvestItemId = None; Requirements = None; Extra = [] }

/// TS `CropDefinitionSchema.multiTile` (inline object).
and CropMultiTile =
    {
        /// int, positive.
        Width: float
        /// int, positive.
        Height: float
    }

    /// A `CropMultiTile` with every field at its schema default.
    static member Default : CropMultiTile =
        { Width = 0.0; Height = 0.0 }

/// A crop mutation (M1): a rare harvest that yields more. Mutations are checked in list order.
and CropMutationDefinition =
    {
        Id: string
        Name: string
        /// How much more a mutated harvest yields (2 = twice). positive.
        YieldMultiplier: float
        /// The part of a crop's mutation chance that lands on this mutation, checked in list order
        /// (the shares add up to at most 1). 0..1.
        Share: float
        /// `#rrggbb`: how the editor and the game mark it.
        Color: string
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `CropMutationDefinition` with every field at its schema default.
    static member Default : CropMutationDefinition =
        { Id = ""; Name = ""; YieldMultiplier = 1.0; Share = 0.0; Color = ""; Extra = [] }

/// A crop quality tier (M1). Tiers are ranked in list order, the first being the plain one.
and CropQualityDefinition =
    {
        Id: string
        Name: string
        /// Sell price multiplier (1 = the item's value). positive.
        PriceMultiplier: float
        /// Harvest yield multiplier. positive.
        YieldMultiplier: float
        /// Multiplies a crop's mutation chance at harvest. nonnegative.
        MutationMultiplier: float
        /// Stars shown after an item's name. int, nonnegative.
        Stars: float
        /// `#rrggbb`: how the editor marks it.
        Color: string
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `CropQualityDefinition` with every field at its schema default.
    static member Default : CropQualityDefinition =
        { Id = ""; Name = ""; PriceMultiplier = 1.0; YieldMultiplier = 1.0; MutationMultiplier = 1.0; Stars = 0.0; Color = ""; Extra = [] }

and CustomAsset =
    {
        Id: string
        Name: string
        /// One of `CustomAssetTypes`.
        Type: string
        /// int, positive.
        Width: float option
        /// int, positive.
        Height: float option
        Animations: (AnimationClip list) option
        DataUrl: string
        TileType: string option
        /// Present when the image is an animated sprite sheet (M7).
        Sheet: SpriteSheet option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `CustomAsset` with every field at its schema default.
    static member Default : CustomAsset =
        { Id = ""; Name = ""; Type = ""; Width = None; Height = None; Animations = None; DataUrl = ""; TileType = None; Sheet = None; Extra = [] }

/// A project's own crop (`customCrops`): the fields of `CropDefinition`.
and CustomCropDefinition =
    {
        Id: string
        Name: string
        Visual: VisualRef option
        SeedCost: float
        BaseHarvestValue: float
        /// In-game days from planting to maturity. positive; validation requires it (absent or 0
        /// grows in one day).
        GrowthDays: float option
        /// int, positive.
        Stages: float
        Seasons: string list
        /// In-game days between repeat harvests for regrowing crops. positive.
        RegrowthDays: float option
        CanRegrow: bool
        MultiTile: CropMultiTile option
        MutationChance: float option
        YieldMin: float
        YieldMax: float
        /// The item a harvest gives. Absent: the item named `crop-{id}` (the editor's convention).
        HarvestItemId: string option
        /// What it needs beyond water and its seasons (M1).
        Requirements: GrowthRequirements option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `CustomCropDefinition` with every field at its schema default.
    static member Default : CustomCropDefinition =
        { Id = ""; Name = ""; Visual = None; SeedCost = 0.0; BaseHarvestValue = 0.0; GrowthDays = None; Stages = 0.0; Seasons = []; RegrowthDays = None; CanRegrow = false; MultiTile = None; MutationChance = None; YieldMin = 0.0; YieldMax = 0.0; HarvestItemId = None; Requirements = None; Extra = [] }

/// A day/night tint keyframe: at `Minute` of the day the scene is multiplied by `Color`.
and DaylightKey =
    {
        /// Minute of the day. nonnegative.
        Minute: float
        /// `#rrggbb` multiply colour (`#ffffff` leaves the scene as drawn).
        Color: string
    }

    /// A `DaylightKey` with every field at its schema default.
    static member Default : DaylightKey =
        { Minute = 0.0; Color = "" }

/// A cutscene (M2): steps that run over time — characters move, face and emote, the camera pans,
/// the screen fades, music changes, lines are said and choices branch. Events start one with the
/// `startCutscene` outcome.
and Cutscene =
    {
        Id: string
        Name: string
        Steps: CutsceneStep list
        /// The player may skip it (Escape); the steps it skips do nothing, except `outcomes` steps,
        /// which still run.
        Skippable: bool
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `Cutscene` with every field at its schema default.
    static member Default : Cutscene =
        { Id = ""; Name = ""; Steps = []; Skippable = true; Extra = [] }

/// One answer of a cutscene `choice` step: its text and the step label it jumps to.
and CutsceneChoice =
    {
        Text: string
        /// The label of the step to go on from; empty: the next step.
        Goto: string
    }

    /// A `CutsceneChoice` with every field at its schema default.
    static member Default : CutsceneChoice =
        { Text = ""; Goto = "" }

/// One step of a cutscene. Like an outcome, one flat object whose `type` (one of
/// `CutsceneStepTypes`) says which fields apply.
and CutsceneStep =
    {
        /// One of `CutsceneStepTypes`.
        Type: string
        /// A name `jump` steps and choices go to.
        Label: string option
        /// Who acts: `player` or an NPC id (move, face, emote; camera follows them).
        Actor: string option
        /// Tile column (move, camera).
        X: float option
        /// Tile row (move, camera).
        Y: float option
        /// One of `Directions` (face).
        Direction: string option
        /// One of `Emotes` (emote).
        Emote: string option
        /// How long it takes, in seconds (wait, emote, camera, fade). nonnegative.
        Seconds: float option
        /// The line (say) or the question (choice).
        Text: string option
        /// Who says it: `player`, an NPC id, or absent for a narrator.
        Speaker: string option
        /// The speaker's portrait for the line (one of the NPC's `portraits`, M4).
        Expression: string option
        /// The answers (choice).
        Options: CutsceneChoice list
        /// The label to go to (jump).
        Goto: string option
        /// A jump is taken only when all of these hold (absent: always).
        Conditions: EventCondition list
        /// `out` or `in` (fade).
        Mode: string option
        /// `#rrggbb` (fade; black by default).
        Color: string option
        /// A sound asset (sound) or music asset (music; empty: back to the game's music).
        SoundId: string option
        /// Run in order (outcomes), from the outcome vocabulary.
        Outcomes: EventOutcome list
        /// A move waits for the actor to arrive before the next step (absent: it waits).
        Wait: bool option
    }

    /// A `CutsceneStep` with every field at its schema default.
    static member Default : CutsceneStep =
        { Type = ""; Label = None; Actor = None; X = None; Y = None; Direction = None; Emote = None; Seconds = None; Text = None; Speaker = None; Expression = None; Options = []; Goto = None; Conditions = []; Mode = None; Color = None; SoundId = None; Outcomes = []; Wait = None }

and Dialogue =
    {
        Id: string
        NpcId: string
        Text: string
        Options: DialogueOption list
        /// The portrait the dialogue box shows (one of the NPC's `portraits`, M4).
        Expression: string option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `Dialogue` with every field at its schema default.
    static member Default : Dialogue =
        { Id = ""; NpcId = ""; Text = ""; Options = []; Expression = None; Extra = [] }

and DialogueOption =
    {
        Text: string
        NextDialogueId: string option
        GiveItem: string option
        GiveItemQuantity: float option
        TakeMoney: float option
        GiveMoney: float option
        /// Choosing this option sets this flag (true).
        EventFlag: string option
        /// Only once: the option is hidden after it has been chosen (it remembers through its
        /// `EventFlag`, or a flag of its own when it has none).
        Once: bool option
        /// The option is hidden while this flag is set.
        HiddenIfFlag: string option
        RequiresItem: string option
        RequiresFlag: string option
        /// Choosing this option closes the dialogue and opens the given shop (M2).
        OpenShopId: string option
        /// Choosing this option offers/starts the given quest (M3 quest-giver binding).
        OfferQuestId: string option
        /// Option only shown at/above this friendship (M4 heart-gated dialogue).
        RequiresFriendship: float option
        /// Choosing this option performs a creator-defined action.
        ActionId: string option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `DialogueOption` with every field at its schema default.
    static member Default : DialogueOption =
        { Text = ""; NextDialogueId = None; GiveItem = None; GiveItemQuantity = None; TakeMoney = None; GiveMoney = None; EventFlag = None; Once = None; HiddenIfFlag = None; RequiresItem = None; RequiresFlag = None; OpenShopId = None; OfferQuestId = None; RequiresFriendship = None; ActionId = None; Extra = [] }

/// A single event/action outcome. Not a discriminated union: one flat object whose `type` selects which optional fields apply.
and EventOutcome =
    {
        /// One of `EventOutcomeTypes`.
        Type: string
        Message: string option
        ItemId: string option
        ItemQuantity: float option
        /// finite.
        Amount: float option
        /// int, 0..10.
        Radius: float option
        FlagName: string option
        QuestId: string option
        NpcId: string option
        DialogueId: string option
        TileX: float option
        TileY: float option
        /// One of `TileTypes`.
        NewTileType: string option
        SceneId: string option
        X: float option
        Y: float option
        SoundId: string option
        ActionId: string option
        MinigameId: string option
        /// The cutscene `startCutscene` plays (M2).
        CutsceneId: string option
        /// The variable `setVariable` / `addVariable` change (M3).
        Variable: string option
        /// The text `setVariable` stores (M3).
        Text: string option
        /// The weather `setWeather` makes today's (M3).
        WeatherId: string option
        /// The skill `giveExperience` trains (M3).
        Skill: string option
        /// The shop `addShopStock` / `removeShopStock` change (M3).
        ShopId: string option
        /// The node type `placeNode` puts down (M3).
        NodeTypeId: string option
        /// The machine type `placeMachine` puts down (M3).
        MachineTypeId: string option
        /// The relationship `setRelationship` sets (M4), one of `RelationshipStatuses`.
        Status: string option
        /// The letter `sendMail` sends (M6).
        MailId: string option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `EventOutcome` with every field at its schema default.
    static member Default : EventOutcome =
        { Type = ""; Message = None; ItemId = None; ItemQuantity = None; Amount = None; Radius = None; FlagName = None; QuestId = None; NpcId = None; DialogueId = None; TileX = None; TileY = None; NewTileType = None; SceneId = None; X = None; Y = None; SoundId = None; ActionId = None; MinigameId = None; CutsceneId = None; Variable = None; Text = None; WeatherId = None; Skill = None; ShopId = None; NodeTypeId = None; MachineTypeId = None; Status = None; MailId = None; Extra = [] }

/// The project's export settings (Export Game).
and ExportSettings =
    {
        Title: string option
        ExecutableName: string option
        Version: string option
        /// Stable save-folder identity; never derive it from a later project rename.
        GameId: string
        Author: string option
        Company: string option
        [<JsonKey "icon">]
        IconAssetId: string option
        Window: ExportWindow
        /// `integer` or `fit`.
        PixelScale: string
        Credits: string option
        Targets: string list
    }

    /// A `ExportSettings` with every field at its schema default.
    static member Default : ExportSettings =
        { Title = None; ExecutableName = None; Version = None; GameId = ""; Author = None; Company = None; IconAssetId = None; Window = ExportWindow.Default; PixelScale = "integer"; Credits = None; Targets = [ "windows-x64"; "linux-x64" ] }

/// Default window for a standalone exported game.
and ExportWindow =
    {
        Width: int
        Height: int
        Fullscreen: bool
    }

    /// A `ExportWindow` with every field at its schema default.
    static member Default : ExportWindow =
        { Width = 1280; Height = 800; Fullscreen = false }

and FishTable =
    {
        Id: string
        Name: string
        /// Restrict to seasons (absent = all).
        Seasons: (string list) option
        /// Restrict to scenes (absent = any water).
        SceneIds: (string list) option
        Entries: FishTableEntry list
        /// Chance (0..1) a cast catches junk instead of rolling the table.
        JunkChance: float
        JunkItemId: string option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `FishTable` with every field at its schema default.
    static member Default : FishTable =
        { Id = ""; Name = ""; Seasons = None; SceneIds = None; Entries = []; JunkChance = 0.15; JunkItemId = None; Extra = [] }

and FishTableEntry =
    {
        ItemId: string
        /// positive.
        Weight: float
        /// 0..1 — harder fish escape low-tier rods more often.
        Difficulty: float
    }

    /// A `FishTableEntry` with every field at its schema default.
    static member Default : FishTableEntry =
        { ItemId = ""; Weight = 0.0; Difficulty = 0.3 }

and GameContent =
    {
        /// int.
        ContentVersion: float
        /// Crop definitions by id (built-in merged with project custom crops).
        Crops: (string * CropDefinition) list
        Items: Item list
        Npcs: Npc list
        Dialogues: Dialogue list
        Quests: Quest list
        Events: GameEvent list
        Shops: ShopDefinition list
        NodeTypes: NodeTypeDefinition list
        Settings: ProjectSettings
        Recipes: RecipeDefinition list
        MachineTypes: MachineTypeDefinition list
        Weather: WeatherConfig
        AnimalSpecies: AnimalSpeciesDefinition list
        FishTables: FishTable list
        Mine: MineConfig
        /// Creator-defined actions (extensibility layer).
        Actions: ActionDef list
        /// Declared minigames (extensibility layer).
        Minigames: MinigameDef list
        /// Authored initial scenes — the template the world state is created from.
        Scenes: Scene list
        StartSceneId: string
        /// Terrains (built-in merged with the project's).
        Terrains: TerrainDefinition list
        /// Tools (built-in merged with the project's).
        Tools: ToolDefinition list
        /// Item categories (built-in merged with the project's).
        ItemCategories: ItemCategoryDefinition list
        /// Crop quality tiers, ranked (built-in merged with the project's).
        CropQualities: CropQualityDefinition list
        /// Crop mutations, in roll order (built-in merged with the project's).
        CropMutations: CropMutationDefinition list
        /// Cutscenes (M2).
        Cutscenes: Cutscene list
        /// Heart events (M4).
        HeartEvents: HeartEvent list
        /// Buildings a builder puts up (M5); absent when there are none (Rust skips an empty list).
        BuildingTypes: (BuildingDefinition list) option
        /// Collections (M6).
        Collections: (CollectionDefinition list) option
        /// Bundles (M6).
        Bundles: (BundleDefinition list) option
        /// Achievements (M6).
        Achievements: (AchievementDefinition list) option
        /// Letters (M6).
        Mail: (MailDefinition list) option
        /// Monsters (M7).
        Monsters: (MonsterDefinition list) option
    }

    /// A `GameContent` with every field at its schema default.
    static member Default : GameContent =
        { ContentVersion = 0.0; Crops = []; Items = []; Npcs = []; Dialogues = []; Quests = []; Events = []; Shops = []; NodeTypes = []; Settings = ProjectSettings.Default; Recipes = []; MachineTypes = []; Weather = WeatherConfig.Default; AnimalSpecies = []; FishTables = []; Mine = MineConfig.Default; Actions = []; Minigames = []; Scenes = []; StartSceneId = ""; Terrains = []; Tools = []; ItemCategories = []; CropQualities = []; CropMutations = []; Cutscenes = []; HeartEvents = []; BuildingTypes = None; Collections = None; Bundles = None; Achievements = None; Mail = None; Monsters = None }

and GameEvent =
    {
        Id: string
        Name: string
        /// Scene the event lives in; empty string = evaluated in every scene.
        SceneId: string
        /// One of `EventTriggers`.
        Trigger: string
        Conditions: EventCondition list
        Outcomes: EventOutcome list
        Active: bool
        Repeatable: bool
        /// What the trigger is about (M3): an item (harvest, sell, craft, catch, pickUp), an NPC
        /// (friendship) or a quest (quest). Absent or empty: anything.
        Subject: string option
        /// The time of day a `time` trigger fires at, in minutes (M3).
        Minute: float option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `GameEvent` with every field at its schema default.
    static member Default : GameEvent =
        { Id = ""; Name = ""; SceneId = ""; Trigger = ""; Conditions = []; Outcomes = []; Active = false; Repeatable = false; Subject = None; Minute = None; Extra = [] }

and GamePanel =
    {
        Id: string
        Title: string
        VisibleFlag: string option
        /// At most 40 entries.
        Entries: GamePanelEntry list
    }

    /// A `GamePanel` with every field at its schema default.
    static member Default : GamePanel =
        { Id = ""; Title = ""; VisibleFlag = None; Entries = [] }

/// TS `GamePanelSchema.entries` item (inline object).
and GamePanelEntry =
    {
        Label: string
        /// One of `GamePanelEntryKinds`.
        Kind: string
        Value: string
    }

    /// A `GamePanelEntry` with every field at its schema default.
    static member Default : GamePanelEntry =
        { Label = ""; Kind = ""; Value = "" }

and GameProject =
    {
        /// int.
        SchemaVersion: float
        Id: string
        Name: string
        Version: string
        Scenes: Scene list
        Npcs: Npc list
        Items: Item list
        Events: GameEvent list
        Dialogues: Dialogue list
        Quests: Quest list
        Player: Player
        /// Flags a new game starts with: `boolean | number | string` (plugins store numbers and
        /// strings, and Keep changes writes them back as they are), so each value is checked to be
        /// one of those.
        [<JsonDecode "(Decode.dict Decode.flagValue) (key :: path) value">]
        EventFlags: (string * Json) list
        StartSceneId: string
        /// One of `EditorModes`.
        Mode: string
        /// One of `TileTypes`.
        SelectedTileType: string
        SelectedTileVisual: VisualRef option
        [<JsonKey "selectedNPCId"; JsonNull>]
        SelectedNpcId: string option
        [<JsonNull>]
        SelectedItemId: string option
        CustomAssets: CustomAsset list
        CustomCrops: (CustomCropDefinition list) option
        PlayerVisual: VisualRef option
        Graphics: GraphicsSettings option
        GamePanels: (GamePanel list) option
        CurrentSeason: string
        CurrentDay: float
        /// 1-based day within `CurrentSeason` (mirrors GameState.clock.dayOfSeason). Absent when
        /// `CurrentDay` already lands on it in a calendar that starts on day 1 of the first
        /// season. int, positive.
        CurrentDayOfSeason: float option
        /// Minute-of-day of the game clock (v4+).
        CurrentTimeMinutes: float
        /// int.
        CurrentYear: float
        GameStartTime: float
        Shops: ShopDefinition list
        NodeTypes: NodeTypeDefinition list
        Settings: ProjectSettings
        Recipes: RecipeDefinition list
        MachineTypes: MachineTypeDefinition list
        Weather: WeatherConfig
        AnimalSpecies: AnimalSpeciesDefinition list
        Animals: AnimalState list
        FishTables: FishTable list
        Mine: MineConfig
        Actions: ActionDef list
        Minigames: MinigameDef list
        /// Installed content packs (M5). Array order = load order.
        ContentPacks: PackInstallation list
        /// The project's terrains (M1), beside and over the built-in ones.
        Terrains: (TerrainDefinition list) option
        /// The project's tools (M1), beside and over the built-in ones.
        Tools: (ToolDefinition list) option
        /// The project's item categories (M1), beside and over the built-in ones.
        ItemCategories: (ItemCategoryDefinition list) option
        /// The project's crop qualities (M1), beside and over the built-in tiers.
        CropQualities: (CropQualityDefinition list) option
        /// The project's crop mutations (M1), beside and over the built-in ones.
        CropMutations: (CropMutationDefinition list) option
        /// Cutscenes (M2).
        Cutscenes: (Cutscene list) option
        /// Heart events (M4).
        HeartEvents: (HeartEvent list) option
        /// Buildings a builder puts up (M5).
        BuildingTypes: (BuildingDefinition list) option
        /// Buildings on the map when a game starts (M5: the house; Keep changes adds built ones).
        Buildings: (BuildingState list) option
        /// Collections (M6).
        Collections: (CollectionDefinition list) option
        /// Bundles (M6).
        Bundles: (BundleDefinition list) option
        /// Achievements (M6).
        Achievements: (AchievementDefinition list) option
        /// Letters (M6).
        Mail: (MailDefinition list) option
        /// Monsters (M7).
        Monsters: (MonsterDefinition list) option
        /// Optional native export identity, window and target settings (additive to schema v8).
        Export: ExportSettings option
        /// Serialized PRNG state so play sessions resume deterministically (additive, optional).
        RngState: RngState option
        /// Today's weather id (mirrors GameState.clock.weatherId).
        CurrentWeatherId: string option
        /// Friendship & gifting state per NPC (mirrors GameState.social).
        SocialState: ((string * NpcSocialState) list) option
        /// Deepest mine floor reached (mirrors GameState.mine.deepestFloor). int, nonnegative.
        MineDeepestFloor: float option
        /// Items whose owning pack is missing/disabled (mirrors GameState.quarantinedItems).
        QuarantinedItems: (InventorySlot list) option
        /// The rest of a kept playtest's state (Rust `KeptState`: tick, open dialogue, shop,
        /// minigame or cutscene, chosen music, today's shop purchases, NPC walks, mine floor,
        /// children, changed shop stock, and the M5–M7 `goals`: open storage, the farm layout,
        /// the shipping log, collections, bundles, achievements, mail, monsters and health).
        /// Only the engine reads it.
        KeptState: Json option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `GameProject` with every field at its schema default.
    static member Default : GameProject =
        { SchemaVersion = 0.0; Id = ""; Name = ""; Version = ""; Scenes = []; Npcs = []; Items = []; Events = []; Dialogues = []; Quests = []; Player = Player.Default; EventFlags = []; StartSceneId = ""; Mode = ""; SelectedTileType = ""; SelectedTileVisual = None; SelectedNpcId = None; SelectedItemId = None; CustomAssets = []; CustomCrops = None; PlayerVisual = None; Graphics = None; GamePanels = None; CurrentSeason = ""; CurrentDay = 0.0; CurrentDayOfSeason = None; CurrentTimeMinutes = 0.0; CurrentYear = 0.0; GameStartTime = 0.0; Shops = []; NodeTypes = []; Settings = ProjectSettings.Default; Recipes = []; MachineTypes = []; Weather = WeatherConfig.Default; AnimalSpecies = []; Animals = []; FishTables = []; Mine = MineConfig.Default; Actions = []; Minigames = []; ContentPacks = []; Terrains = None; Tools = None; ItemCategories = None; CropQualities = None; CropMutations = None; Cutscenes = None; HeartEvents = None; BuildingTypes = None; Buildings = None; Collections = None; Bundles = None; Achievements = None; Mail = None; Monsters = None; Export = None; RngState = None; CurrentWeatherId = None; SocialState = None; MineDeepestFloor = None; QuarantinedItems = None; KeptState = None; Extra = [] }

/// The words of a game (M1): what it calls money, energy and the skills, in every language.
and GameWords =
    {
        /// How an amount of money is written, `{0}` standing for the number (`${0}`, `{0} credits`).
        Money: string option
        /// What energy is called (`Energy`, `Oxygen`).
        Energy: string option
        /// Skill names by skill id.
        Skills: (string * string) list
        /// The same words in other languages, by locale code (their own `Locales` are not read).
        Locales: (string * GameWords) list
    }

    /// A `GameWords` with every field at its schema default.
    static member Default : GameWords =
        { Money = None; Energy = None; Skills = []; Locales = [] }

and GiftTastes =
    {
        Loved: string list
        Liked: string list
        Disliked: string list
        Hated: string list
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `GiftTastes` with every field at its schema default.
    static member Default : GiftTastes =
        { Loved = []; Liked = []; Disliked = []; Hated = []; Extra = [] }

/// What a crop needs to be planted and to grow, beyond water and its seasons (M1).
and GrowthRequirements =
    {
        /// Terrains it can be planted on; empty: any plantable terrain.
        Terrains: string list
        /// One of `LightNeeds`: `light` (no terrain on its tile blocks light, and the scene is not
        /// dark) or `dark`; absent: either.
        Light: string option
        /// The coolest it grows at (`Scene.Temperature`, else the season's).
        MinTemperature: float option
        /// The warmest it grows at.
        MaxTemperature: float option
        /// Scene tags it needs, all of them (`Scene.Tags`).
        SceneTags: string list
        /// Fertilizer items it must be planted with (one of them); empty: none needed.
        Fertilizers: string list
        /// Neighbours it needs around its tile.
        Neighbors: NeighborRequirement list
    }

    /// A `GrowthRequirements` with every field at its schema default.
    static member Default : GrowthRequirements =
        { Terrains = []; Light = None; MinTemperature = None; MaxTemperature = None; SceneTags = []; Fertilizers = []; Neighbors = [] }

and GraphicsSettings =
    {
        PixelArt: bool
    }

    /// A `GraphicsSettings` with every field at its schema default.
    static member Default : GraphicsSettings =
        { PixelArt = true }

/// An integer tile coordinate: TS inline `{ x: int, y: int }` objects in `NPCSchema.patrolPoints` and `NpcStateSchema.path`.
and GridPoint =
    {
        /// int.
        X: float
        /// int.
        Y: float
    }

    /// A `GridPoint` with every field at its schema default.
    static member Default : GridPoint =
        { X = 0.0; Y = 0.0 }

and InventorySlot =
    {
        Item: Item
        Quantity: float
        /// The crop quality of the units in this slot (one of `CropQualities` other than normal);
        /// absent for normal quality.
        Quality: string option
    }

    /// A `InventorySlot` with every field at its schema default.
    static member Default : InventorySlot =
        { Item = Item.Default; Quantity = 0.0; Quality = None }

/// A kind of item (M1) and what items of it do. The built-in kinds are seed, tool, crop, gift,
/// quest, fertilizer, material and fish.
and ItemCategoryDefinition =
    {
        Id: string
        Name: string
        /// Planted on soil (the item's `CropType` says what grows).
        Plantable: bool
        /// Planted with a seed to fertilize its soil.
        Fertilizer: bool
        /// Held and used as a tool (the item's `ToolType`): it wears down and shops repair it.
        Equippable: bool
        /// Eaten from the inventory for energy.
        Edible: bool
        /// Energy an item of it restores when eaten (the item's `Energy` replaces it). nonnegative.
        Energy: float
        /// Put down in the world (a machine type names the item).
        Placeable: bool
        /// Can be given to NPCs.
        Giftable: bool
        /// Shops buy it.
        Sellable: bool
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `ItemCategoryDefinition` with every field at its schema default.
    static member Default : ItemCategoryDefinition =
        { Id = ""; Name = ""; Plantable = false; Fertilizer = false; Equippable = false; Edible = false; Energy = 0.0; Placeable = false; Giftable = true; Sellable = true; Extra = [] }

and Item =
    {
        Id: string
        Name: string
        Visual: VisualRef option
        Description: string
        /// An item category id (`ItemCategoryDefinition`; the built-in ones are `ItemTypes`).
        Type: string
        Stackable: bool
        MaxStack: float
        Value: float
        CropType: string option
        /// A tool id (`ToolDefinition`; the built-in ones are `ToolTypes`).
        ToolType: string option
        ToolPower: float option
        /// Tool tier: 1 = basic. Higher tiers hit harder, cost less energy, gain AoE. int, positive.
        ToolTier: float option
        Durability: float option
        MaxDurability: float option
        /// Action performed when the item is used from the inventory.
        UseActionId: string option
        /// Consume one of the item on a successful use.
        ConsumeOnUse: bool option
        /// Energy it restores when eaten, for an edible kind (M1); absent: the kind's.
        Energy: float option
        /// The item is clothing the player can wear (M4).
        Wear: WearDefinition option
        /// Health it restores when eaten, with combat on (M7). int.
        Health: float option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `Item` with every field at its schema default.
    static member Default : Item =
        { Id = ""; Name = ""; Visual = None; Description = ""; Type = ""; Stackable = false; MaxStack = 0.0; Value = 0.0; CropType = None; ToolType = None; ToolPower = None; ToolTier = None; Durability = None; MaxDurability = None; UseActionId = None; ConsumeOnUse = None; Energy = None; Wear = None; Health = None; Extra = [] }

/// TS `TileMachineSchema.processing` (inline object): an in-flight machine job.
and MachineProcessing =
    {
        RecipeId: string
        CompletesAtMinute: float
    }

    /// A `MachineProcessing` with every field at its schema default.
    static member Default : MachineProcessing =
        { RecipeId = ""; CompletesAtMinute = 0.0 }

and MachineTypeDefinition =
    {
        Id: string
        Name: string
        Visual: VisualRef option
        Description: string
        /// Renderer hint.
        Color: string
        /// Item consumed to place this machine (crafted or bought).
        ItemId: string option
        BlocksMovement: bool
        /// Station categories a placed machine of this type provides to nearby hand-crafting (e.g. a Kitchen provides 'cooking').
        StationCategories: string list
        /// Slots of storage: a chest (M5). Absent: none (a shipping bin has
        /// `FarmSchema.DefaultShippingSlots`). int, nonnegative.
        Storage: float option
        /// A shipping bin (M6): what it holds is sold overnight. Absent: not one.
        Shipping: bool option
        /// A gate (M5): interacting opens and closes it, and an open gate never blocks.
        Gate: bool option
        /// What it does to the tiles around it (M5): sprinklers, scarecrows.
        AreaEffects: (AreaEffect list) option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `MachineTypeDefinition` with every field at its schema default.
    static member Default : MachineTypeDefinition =
        { Id = ""; Name = ""; Visual = None; Description = ""; Color = "#9a7b4f"; ItemId = None; BlocksMovement = true; StationCategories = []; Storage = None; Shipping = None; Gate = None; AreaEffects = None; Extra = [] }

and MineBand =
    {
        /// int, positive.
        FromFloor: float
        /// int, positive.
        ToFloor: float
        /// Weighted node types spawned in this depth band.
        Rocks: MineRockWeight list
        /// Rock density (fraction of floor tiles occupied). 0..1.
        Density: float
        /// Weighted monsters on its floors (M7).
        Monsters: (MineMonsterWeight list) option
        /// Monsters per floor (M7), with combat on; absent: none. int.
        MonsterCount: float option
        /// Hazard terrains scattered over its floors (M7).
        Hazards: (MineHazard list) option
        /// Weighted treasure left on its floors (M7).
        Treasure: (MineTreasure list) option
        /// Treasures per floor (M7); absent: none. int.
        TreasureCount: float option
    }

    /// A `MineBand` with every field at its schema default.
    static member Default : MineBand =
        { FromFloor = 0.0; ToFloor = 0.0; Rocks = []; Density = 0.35; Monsters = None; MonsterCount = None; Hazards = None; Treasure = None; TreasureCount = None }

and MineConfig =
    {
        Enabled: bool
        /// Scene holding the mine entrance (descend via the entrance tile).
        EntranceSceneId: string option
        /// int.
        EntranceX: float option
        /// int.
        EntranceY: float option
        /// int, positive.
        Floors: float
        /// int, positive.
        FloorWidth: float
        /// int, positive.
        FloorHeight: float
        Bands: MineBand list
        /// Chance a broken rock reveals the ladder down. 0..1.
        LadderChance: float
        /// Elevator checkpoint every N floors. int, positive.
        ElevatorEvery: float
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `MineConfig` with every field at its schema default.
    static member Default : MineConfig =
        { Enabled = false; EntranceSceneId = None; EntranceX = None; EntranceY = None; Floors = 20.0; FloorWidth = 14.0; FloorHeight = 12.0; Bands = []; LadderChance = 0.18; ElevatorEvery = 5.0; Extra = [] }

/// TS `MineBandSchema.rocks` item (inline object): a weighted node type.
and MineRockWeight =
    {
        NodeTypeId: string
        /// positive.
        Weight: float
    }

    /// A `MineRockWeight` with every field at its schema default.
    static member Default : MineRockWeight =
        { NodeTypeId = ""; Weight = 0.0 }

and MinigameDef =
    {
        Id: string
        Name: string
        /// Implementation key looked up in the host's minigame registry ('timing-bar' ships with the engine; game code registers custom kinds). Unknown kinds fall back to a neutral confirm that scores 0.5.
        Kind: string
        /// Kind-specific tuning, passed verbatim to the implementation (`z.unknown()` values).
        Config: (string * Json) list
        /// Declarative consequences by score tier (highest matching minScore).
        ResultTiers: MinigameResultTier list
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `MinigameDef` with every field at its schema default.
    static member Default : MinigameDef =
        { Id = ""; Name = ""; Kind = "timing-bar"; Config = []; ResultTiers = []; Extra = [] }

and MinigameResultTier =
    {
        /// Tier applies when score ≥ minScore; the highest matching tier wins. 0..1.
        MinScore: float
        Outcomes: EventOutcome list
    }

    /// A `MinigameResultTier` with every field at its schema default.
    static member Default : MinigameResultTier =
        { MinScore = 0.0; Outcomes = [] }

and MovementConfig =
    {
        /// Player walk speed in tiles per second (free movement). positive.
        PlayerSpeed: float
    }

    /// A `MovementConfig` with every field at its schema default.
    static member Default : MovementConfig =
        { PlayerSpeed = 4.5 }

/// Weighted drop-table entry for a gathering node.
/// Neighbours a crop needs around its tile: crops of a type, a terrain, or water.
and NeighborRequirement =
    {
        /// One of `NeighborKinds`: `crop`, `terrain` or `water` (a terrain that is a water source).
        Kind: string
        /// The crop type or terrain id; unused for water.
        Id: string option
        /// How far to look, in tiles (1 = the eight tiles around). int, positive.
        Radius: float
        /// How many matching tiles are needed. int, positive.
        Min: float
    }

    /// A `NeighborRequirement` with every field at its schema default.
    static member Default : NeighborRequirement =
        { Kind = ""; Id = None; Radius = 1.0; Min = 1.0 }

and NodeDrop =
    {
        ItemId: string
        /// int, nonnegative.
        Min: float
        /// int, nonnegative.
        Max: float
        /// positive.
        Weight: float
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `NodeDrop` with every field at its schema default.
    static member Default : NodeDrop =
        { ItemId = ""; Min = 0.0; Max = 0.0; Weight = 0.0; Extra = [] }

/// A gathering node type (tree, rock, weeds, …) — content-defined so mods and projects can add their own.
and NodeTypeDefinition =
    {
        Id: string
        Name: string
        Visual: VisualRef option
        /// Number of tool hits required to break the node. int, positive.
        Health: float
        /// One of `ToolTypes`.
        RequiredTool: string
        /// Minimum tool tier required (tools default to tier 1). int, positive.
        RequiredToolTier: float
        /// Weighted drop table; each hit that depletes the node rolls once per entry range.
        Drops: NodeDrop list
        RespawnDays: float option option
        /// Renderer hint (hex color).
        Color: string
        /// Whether the node blocks movement while present.
        BlocksMovement: bool
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `NodeTypeDefinition` with every field at its schema default.
    static member Default : NodeTypeDefinition =
        { Id = ""; Name = ""; Visual = None; Health = 0.0; RequiredTool = ""; RequiredToolTier = 1.0; Drops = []; RespawnDays = None; Color = "#7a5a3a"; BlocksMovement = true; Extra = [] }

/// TS `NPC` (`NPCSchema`).
and Npc =
    {
        Id: string
        Name: string
        Visual: VisualRef option
        X: float
        Y: float
        SceneId: string
        Dialogue: Dialogue list
        CanMove: bool
        /// One of `NpcMovePatterns`.
        MovePattern: string option
        /// Max tiles from home for the wander pattern (default 3). int, positive.
        WanderRadius: float option
        /// Waypoints for the patrol pattern (visited in order, looping).
        PatrolPoints: (GridPoint list) option
        /// Time-based schedule (M3): sorted by minute; latest passed entry wins.
        Schedule: (NpcScheduleEntry list) option
        /// Gift preferences (M4 social); unlisted items are neutral.
        GiftTastes: GiftTastes option
        /// Birthday: day-of-season (1-28) within birthSeason, doubles gift effects.
        Birthday: NpcBirthday option
        Appearance: string
        /// What the NPC says when the player talks to it, by day (M4): see `NpcLine`.
        Lines: (NpcLine list) option
        /// Portraits by expression for the dialogue box (M4).
        Portraits: (NpcPortrait list) option
        /// The NPC can be dated and married, when the game allows it (M4).
        Romanceable: bool option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `Npc` with every field at its schema default.
    static member Default : Npc =
        { Id = ""; Name = ""; Visual = None; X = 0.0; Y = 0.0; SceneId = ""; Dialogue = []; CanMove = false; MovePattern = None; WanderRadius = None; PatrolPoints = None; Schedule = None; GiftTastes = None; Birthday = None; Appearance = ""; Lines = None; Portraits = None; Romanceable = None; Extra = [] }

/// TS `NPCSchema.birthday` (inline object).
and NpcBirthday =
    {
        /// One of the classic seasons ('spring' | 'summer' | 'fall' | 'winter').
        Season: string
        /// int.
        Day: float
    }

    /// A `NpcBirthday` with every field at its schema default.
    static member Default : NpcBirthday =
        { Season = ""; Day = 0.0 }

/// A scheduled destination: at `minute` (of day), head to (sceneId, x, y).
and NpcScheduleEntry =
    {
        Minute: float
        SceneId: string
        /// int.
        X: float
        /// int.
        Y: float
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `NpcScheduleEntry` with every field at its schema default.
    static member Default : NpcScheduleEntry =
        { Minute = 0.0; SceneId = ""; X = 0.0; Y = 0.0; Extra = [] }

/// Per-NPC social state (friendship points etc.).
and NpcSocialState =
    {
        Friendship: float
        /// int.
        GiftsToday: float
        /// int.
        LastGiftDay: float option
        /// The last day the player talked to the NPC (M4: friendship decay). int.
        TalkedDay: float option
        /// Dating, engaged or married (M4, one of `RelationshipStatuses`); absent: single.
        Status: string option
        /// The day `Status` began; for an engagement, the wedding day. int.
        StatusDay: float option
    }

    /// A `NpcSocialState` with every field at its schema default.
    static member Default : NpcSocialState =
        { Friendship = 0.0; GiftsToday = 0.0; LastGiftDay = None; TalkedDay = None; Status = None; StatusDay = None }

and PackContent =
    {
        Crops: CropDefinition list
        Items: Item list
        Recipes: RecipeDefinition list
        MachineTypes: MachineTypeDefinition list
        NodeTypes: NodeTypeDefinition list
        AnimalSpecies: AnimalSpeciesDefinition list
        FishTables: FishTable list
        WeatherTypes: WeatherTypeDefinition list
        Npcs: Npc list
        Dialogues: Dialogue list
        Scenes: Scene list
        Events: GameEvent list
        Quests: Quest list
        Shops: ShopDefinition list
        Actions: ActionDef list
        Minigames: MinigameDef list
        PlayerStart: PackPlayerStart option
        /// Terrains the pack adds or replaces (M11).
        Terrains: (TerrainDefinition list) option
        /// Tools the pack adds or replaces (M11).
        Tools: (ToolDefinition list) option
        /// Item categories the pack adds or replaces (M11).
        ItemCategories: (ItemCategoryDefinition list) option
        /// Crop quality tiers the pack adds or replaces (M11).
        CropQualities: (CropQualityDefinition list) option
        /// Crop mutations the pack adds or replaces (M11).
        CropMutations: (CropMutationDefinition list) option
        /// Cutscenes the pack adds or replaces (M11).
        Cutscenes: (Cutscene list) option
        /// Heart events the pack adds or replaces (M4).
        HeartEvents: (HeartEvent list) option
        /// Building types the pack adds or replaces (M9: the M5 content).
        BuildingTypes: (BuildingDefinition list) option
        /// Collections the pack adds or replaces (M9: the M6 content).
        Collections: (CollectionDefinition list) option
        /// Bundles the pack adds or replaces (M9).
        Bundles: (BundleDefinition list) option
        /// Achievements the pack adds or replaces (M9).
        Achievements: (AchievementDefinition list) option
        /// Letters the pack adds or replaces (M9).
        Mail: (MailDefinition list) option
        /// Monsters the pack adds or replaces (M9: the M7 content).
        Monsters: (MonsterDefinition list) option
        /// Game panels the pack adds (M9), shown only with `permissions.uiPanels`.
        GamePanels: (GamePanel list) option
        /// Per-locale string tables for game text (M7 i18n). Keys address content fields: `item:{id}:name`, `item:{id}:description`, `dialogue:{id}:text`, `quest:{id}:name`, `quest:{id}:description`. The authored text is the fallback locale.
        Strings: (string * (string * string) list) list
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `PackContent` with every field at its schema default.
    static member Default : PackContent =
        { Crops = []; Items = []; Recipes = []; MachineTypes = []; NodeTypes = []; AnimalSpecies = []; FishTables = []; WeatherTypes = []; Npcs = []; Dialogues = []; Scenes = []; Events = []; Quests = []; Shops = []; Actions = []; Minigames = []; PlayerStart = None; Terrains = None; Tools = None; ItemCategories = None; CropQualities = None; CropMutations = None; Cutscenes = None; HeartEvents = None; BuildingTypes = None; Collections = None; Bundles = None; Achievements = None; Mail = None; Monsters = None; GamePanels = None; Strings = []; Extra = [] }

and PackDependency =
    {
        PackId: string
        Version: string option
    }

    /// A `PackDependency` with every field at its schema default.
    static member Default : PackDependency =
        { PackId = ""; Version = None }

/// How a project stores an installed pack. Array order = load order.
and PackInstallation =
    {
        Pack: ContentPack
        Enabled: bool
    }

    /// A `PackInstallation` with every field at its schema default.
    static member Default : PackInstallation =
        { Pack = ContentPack.Default; Enabled = true }

and PackManifest =
    {
        /// Must match `^[a-z0-9][a-z0-9-]*$` (see `PacksSchema.PackIdPattern`).
        Id: string
        Name: string
        Version: string
        Description: string option
        Author: string option
        /// Semver range: '*', exact '1.2.3', '^1.2.3' or '>=1.2.3'.
        EngineCompatibility: string
        /// Base packs (content-default) keep their plain IDs; all other packs get their definitions namespaced to `packId:localId` at load time.
        Base: bool
        Dependencies: PackDependency list
        /// Fully-qualified IDs this pack intentionally replaces. Redefining an existing ID without declaring it here is a conflict surfaced in the Problems panel (the earlier definition wins).
        Overrides: string list
        Permissions: PackPermissions
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `PackManifest` with every field at its schema default.
    static member Default : PackManifest =
        { Id = ""; Name = ""; Version = ""; Description = None; Author = None; EngineCompatibility = "*"; Base = false; Dependencies = []; Overrides = []; Permissions = PackPermissions.Default; Extra = [] }

and PackPermissions =
    {
        /// Hook names the pack's plugins may subscribe to (user-approved at install).
        Hooks: string list
        /// May contribute content definitions, a player start and string tables (the normal case).
        /// When off, none of the pack's content loads; its plugins still run.
        ContentInject: bool
        /// May add game panels (`content.gamePanels`, M9): shown after the game's own, whatever `contentInject` says.
        UiPanels: bool
        /// Mutation capabilities the pack's plugins may use (`message`, `giveItem`, `setFlag:any`,
        /// `*`, … — see docs/PLUGINS.md). None: the defaults, and onEffect/onCommand answers are dropped.
        Mutations: (string list) option
        /// Minigame kinds the pack's plugins may define (user-approved at install), by local name
        /// (`lantern-catch`); minigame defs play one as `packId:name` (docs/PLUGINS.md). None and
        /// empty both allow none.
        Minigames: (string list) option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `PackPermissions` with every field at its schema default.
    static member Default : PackPermissions =
        { Hooks = []; ContentInject = true; UiPanels = false; Mutations = None; Minigames = None; Extra = [] }

/// Optional player-start block so a base pack can express the whole starter game.
and PackPlayerStart =
    {
        SceneId: string option
        /// int.
        X: float option
        /// int.
        Y: float option
        Money: float option
        Inventory: PackStartItem list
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `PackPlayerStart` with every field at its schema default.
    static member Default : PackPlayerStart =
        { SceneId = None; X = None; Y = None; Money = None; Inventory = []; Extra = [] }

/// A sandboxed plugin: `source` is the body of `function (api) { ... }` and registers handlers with `api.on(hookName, fn)`. Handlers return an array of mutations (validated as `PluginMutation`) — never raw state access.
and PackPlugin =
    {
        Id: string
        Name: string option
        /// Hooks this plugin wants; effective set = intersection with manifest permissions.hooks.
        Hooks: string list
        /// Minigame kinds this plugin defines (`api.minigame(name, …)`), by local name; effective
        /// set = intersection with manifest permissions.minigames.
        Minigames: (string list) option
        Source: string
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `PackPlugin` with every field at its schema default.
    static member Default : PackPlugin =
        { Id = ""; Name = None; Hooks = []; Minigames = None; Source = ""; Extra = [] }

/// TS `PackPlayerStartSchema.inventory` item (inline object).
and PackStartItem =
    {
        ItemId: string
        /// int, min 1.
        Quantity: float
    }

    /// A `PackStartItem` with every field at its schema default.
    static member Default : PackStartItem =
        { ItemId = ""; Quantity = 0.0 }

/// The project's player start/record (TS `PlayerSchema`); runtime uses `PlayerState`.
and Player =
    {
        X: float
        Y: float
        /// One of `Directions`.
        Direction: string
        SceneId: string
        Inventory: InventorySlot list
        MaxInventorySize: float
        Money: float
        Energy: float option
        MaxEnergy: float option
        /// Per-category skill XP/levels (M4g); mirrored from GameState on save.
        Skills: ((string * SkillState) list) option
        ActiveQuests: string list
        CompletedQuests: string list
        EquippedTool: string option
        /// The player's name (M4; a new game may ask for it).
        Name: string option
        /// The farm's name (M4).
        FarmName: string option
        /// How the player looks (M4); absent: the built-in look.
        Look: PlayerLook option
        /// The clothes the player wears (M4), one per slot.
        Worn: (InventorySlot list) option
        /// Health (M7, with combat on); absent: full. int.
        Health: float option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `Player` with every field at its schema default.
    static member Default : Player =
        { X = 0.0; Y = 0.0; Direction = ""; SceneId = ""; Inventory = []; MaxInventorySize = 0.0; Money = 0.0; Energy = None; MaxEnergy = None; Skills = None; ActiveQuests = []; CompletedQuests = []; EquippedTool = None; Name = None; FarmName = None; Look = None; Worn = None; Health = None; Extra = [] }

/// The player character's colours (M4): `#rrggbb` each; absent ones keep the built-in art's.
and PlayerLook =
    {
        Skin: string option
        Hair: string option
        Shirt: string option
        Pants: string option
        /// The headband (or other accent).
        Accent: string option
    }

    /// A `PlayerLook` with every field at its schema default.
    static member Default : PlayerLook =
        { Skin = None; Hair = None; Shirt = None; Pants = None; Accent = None }

and ProjectSettings =
    {
        Movement: MovementConfig
        EnergyEnabled: bool
        /// positive.
        MaxEnergy: float
        /// Fraction of energy restored after a collapse (vs full sleep). 0..1.
        CollapseEnergyFraction: float
        /// Money penalty charged on collapse. nonnegative.
        CollapseMoneyPenalty: float
        Time: TimeConfig
        /// Creator-configurable calendar (M9): seasons, their lengths, and festival days.
        Calendar: CalendarConfig
        /// Player skills (M4g): XP per action category, levels unlock recipes.
        SkillsEnabled: bool
        /// XP thresholds per level (index = level).
        SkillLevelCurve: float list
        /// Game-text locale (M7 i18n): packs may carry per-locale string tables.
        Locale: string
        /// Optional, creator-controlled credit shown in exported games.
        ShowMadeWithCredit: bool
        /// What the game calls money, energy and the skills (M1); absent: the built-in words.
        Words: GameWords option
        /// The day/night and foliage tints (M1); absent: the built-in look.
        Atmosphere: AtmosphereSettings option
        /// Friendship decay, dating, marriage and children (M4); absent: none of them.
        Relationships: RelationshipSettings option
        /// A new game asks for the player's name, the farm's name and the player's colours (M4).
        CharacterCreator: bool option
        /// The farm scene, its layouts and pests (M5); absent: no layouts, no pests.
        Farm: FarmSettings option
        /// Player health, monsters and fainting (M7); absent: no combat.
        Combat: CombatSettings option
    }

    /// A `ProjectSettings` with every field at its schema default.
    static member Default : ProjectSettings =
        { Movement = MovementConfig.Default; EnergyEnabled = true; MaxEnergy = 100.0; CollapseEnergyFraction = 0.5; CollapseMoneyPenalty = 50.0; Time = TimeConfig.Default; Calendar = CalendarConfig.Default; SkillsEnabled = true; SkillLevelCurve = [ 0.0; 50.0; 150.0; 300.0; 500.0; 750.0; 1050.0; 1400.0; 1800.0; 2250.0 ]; Locale = "en"; ShowMadeWithCredit = false; Words = None; Atmosphere = None; Relationships = None; CharacterCreator = None; Farm = None; Combat = None }

and Quest =
    {
        Id: string
        Name: string
        Description: string
        Giver: string option
        /// One of `QuestStatuses`.
        Status: string
        Objectives: QuestObjective list
        Rewards: QuestRewards
        Prerequisites: (string list) option
        AutoStart: bool option
        Repeatable: bool option
        /// Seasonal availability (M3): quest only offered/auto-started in these seasons.
        AvailableSeasons: (string list) option
        /// Absolute-day window (M3): offered from/until these days (inclusive).
        AvailableFromDay: float option
        AvailableToDay: float option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `Quest` with every field at its schema default.
    static member Default : Quest =
        { Id = ""; Name = ""; Description = ""; Giver = None; Status = ""; Objectives = []; Rewards = QuestRewards.Default; Prerequisites = None; AutoStart = None; Repeatable = None; AvailableSeasons = None; AvailableFromDay = None; AvailableToDay = None; Extra = [] }

and QuestObjective =
    {
        Id: string
        /// One of `QuestObjectiveTypes`.
        Type: string
        Description: string
        TargetItemId: string option
        TargetItemQuantity: float option
        TargetCropType: string option
        TargetCropQuantity: float option
        [<JsonKey "targetNPCId">]
        TargetNpcId: string option
        TargetSceneId: string option
        Completed: bool
        Progress: float
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `QuestObjective` with every field at its schema default.
    static member Default : QuestObjective =
        { Id = ""; Type = ""; Description = ""; TargetItemId = None; TargetItemQuantity = None; TargetCropType = None; TargetCropQuantity = None; TargetNpcId = None; TargetSceneId = None; Completed = false; Progress = 0.0; Extra = [] }

/// TS `QuestRewardsSchema.items` item (inline object).
and QuestRewardItem =
    {
        ItemId: string
        Quantity: float
    }

    /// A `QuestRewardItem` with every field at its schema default.
    static member Default : QuestRewardItem =
        { ItemId = ""; Quantity = 0.0 }

and QuestRewards =
    {
        Money: float option
        Items: (QuestRewardItem list) option
        Experience: float option
        /// The skill `Experience` goes to (farming, mining, foraging, fishing, social, …). Absent: farming.
        Skill: string option
        /// A letter sent on completion (M6), arriving the next morning.
        MailId: string option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `QuestRewards` with every field at its schema default.
    static member Default : QuestRewards =
        { Money = None; Items = None; Experience = None; Skill = None; MailId = None; Extra = [] }

and RecipeDefinition =
    {
        Id: string
        Name: string
        Inputs: RecipeIngredient list
        Outputs: RecipeIngredient list
        /// In-game minutes a machine needs; 0 = instant hand-craft. nonnegative.
        ProcessingMinutes: float
        /// Machine type required; absent = craftable by hand.
        MachineTypeId: string option
        /// Freeform crafting discipline for UI grouping ('cooking', 'magic', 'carpentry', …).
        Category: string
        /// Hand-craftable only while near a machine providing this station category.
        RequiresStationCategory: string option
        Unlock: RecipeUnlock option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `RecipeDefinition` with every field at its schema default.
    static member Default : RecipeDefinition =
        { Id = ""; Name = ""; Inputs = []; Outputs = []; ProcessingMinutes = 0.0; MachineTypeId = None; Category = "crafting"; RequiresStationCategory = None; Unlock = None; Extra = [] }

and RecipeIngredient =
    {
        ItemId: string
        /// int, positive.
        Quantity: float
    }

    /// A `RecipeIngredient` with every field at its schema default.
    static member Default : RecipeIngredient =
        { ItemId = ""; Quantity = 0.0 }

/// TS `RecipeUnlockSchema.skill` (inline object).
and RecipeSkillRequirement =
    {
        Skill: string
        /// int.
        Level: float
    }

    /// A `RecipeSkillRequirement` with every field at its schema default.
    static member Default : RecipeSkillRequirement =
        { Skill = ""; Level = 0.0 }

and RecipeUnlock =
    {
        /// Requires a skill at a minimum level.
        Skill: RecipeSkillRequirement option
        /// Requires a completed quest.
        QuestId: string option
        /// Only craftable in these seasons.
        Seasons: (string list) option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `RecipeUnlock` with every field at its schema default.
    static member Default : RecipeUnlock =
        { Skill = None; QuestId = None; Seasons = None; Extra = [] }

/// Serialized PRNG state — deterministic resume is a core guarantee.
and RngState =
    {
        /// Always `"xoshiro128ss"` (zod literal). Any value decodes (a non-string to `""`): the
        /// checks report the literal ("Invalid literal value", SchemaChecks.fs), as zod does.
        [<JsonDecode "(match value with JString s -> s | _ -> \"\")">]
        Algorithm: string
        S: uint32 list
    }

    /// A `RngState` with every field at its schema default.
    static member Default : RngState =
        { Algorithm = "xoshiro128ss"; S = [ 0u; 0u; 0u; 0u ] }

and Scene =
    {
        Id: string
        Name: string
        /// int, positive.
        Width: float
        /// int, positive.
        Height: float
        /// Row-major: `Tiles[y][x]`.
        Tiles: (Tile list) list
        Transitions: SceneTransition list
        Npcs: string list
        Events: string list
        /// Indoor scenes (greenhouses, interiors, mine floors) keep the weather out: rain does not
        /// water their soil and storms do not damage their crops. Absent means outdoor.
        Indoor: bool option
        /// No daylight reaches its crops (caves), for crops that need light or dark (M1).
        Dark: bool option
        /// How warm it is, for crops with a temperature range (M1); absent: the season's.
        Temperature: float option
        /// Free-form tags crops can require (`GrowthRequirements.SceneTags`).
        Tags: (string list) option
        /// Its own day/night and foliage tints (M1).
        Atmosphere: AtmosphereLook option
        /// A music asset that plays in the scene (M2).
        Music: string option
        /// Crops are planted and grow here in every season (a greenhouse, M5).
        AllSeasons: bool option
        /// The building whose interior this is (M5: a `BuildingState` id; the engine sets it).
        Building: string option
        /// Monsters it has each morning (M7).
        Monsters: (MonsterSpawn list) option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `Scene` with every field at its schema default.
    static member Default : Scene =
        { Id = ""; Name = ""; Width = 0.0; Height = 0.0; Tiles = []; Transitions = []; Npcs = []; Events = []; Indoor = None; Dark = None; Temperature = None; Tags = None; Atmosphere = None; Music = None; AllSeasons = None; Building = None; Monsters = None; Extra = [] }

and SceneTransition =
    {
        FromX: float
        FromY: float
        ToSceneId: string
        ToX: float
        ToY: float
        /// Locked transitions don't fire; events can lock/unlock them (M3).
        Locked: bool option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `SceneTransition` with every field at its schema default.
    static member Default : SceneTransition =
        { FromX = 0.0; FromY = 0.0; ToSceneId = ""; ToX = 0.0; ToY = 0.0; Locked = None; Extra = [] }

and ShopDefinition =
    {
        Id: string
        Name: string
        Stock: ShopStockEntry list
        /// Multiplier applied to item base value when the player sells here.
        SellPriceMultiplier: float
        /// Whether this shop buys player items at all.
        BuysItems: bool
        /// Whether this shop repairs broken tools (for durability * repairCostPerPoint).
        RepairsTools: bool
        RepairCostPerPoint: float
        /// Buildings it builds and upgrades (M5): a builder.
        Buildings: (string list) option
        /// Animal species it sells (M5).
        Animals: (string list) option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `ShopDefinition` with every field at its schema default.
    static member Default : ShopDefinition =
        { Id = ""; Name = ""; Stock = []; SellPriceMultiplier = 1.0; BuysItems = true; RepairsTools = false; RepairCostPerPoint = 0.5; Buildings = None; Animals = None; Extra = [] }

/// A single line in a shop's stock list.
and ShopStockEntry =
    {
        ItemId: string
        /// Override price; defaults to the item's base value.
        Price: float option
        /// Restrict availability to these seasons (empty/absent = always).
        Seasons: (string list) option
        /// Max units purchasable per in-game day (absent = unlimited). int, positive.
        DailyLimit: float option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `ShopStockEntry` with every field at its schema default.
    static member Default : ShopStockEntry =
        { ItemId = ""; Price = None; Seasons = None; DailyLimit = None; Extra = [] }

and SkillState =
    {
        Xp: float
        /// int.
        Level: float
    }

    /// A `SkillState` with every field at its schema default.
    static member Default : SkillState =
        { Xp = 0.0; Level = 0.0 }

/// Sprite-sheet slicing metadata (M7). A sheet is a grid: columns = walk frames, rows = directions (down, left, right, up) when `directional`. Assets without this stay static images; games with zero art still render via the colored-rectangle fallback.
and SpriteSheet =
    {
        /// int, positive.
        FrameWidth: float
        /// int, positive.
        FrameHeight: float
        /// int, positive.
        Frames: float
        /// int, positive.
        TicksPerFrame: float
        Directional: bool
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `SpriteSheet` with every field at its schema default.
    static member Default : SpriteSheet =
        { FrameWidth = 0.0; FrameHeight = 0.0; Frames = 0.0; TicksPerFrame = 6.0; Directional = true; Extra = [] }

and Tile =
    {
        X: float
        Y: float
        /// One of `TileTypes`.
        Type: string
        /// Background layer: grass, soil, water, floor
        Background: string
        /// Overlay layer: path, rug, … renders on top of background
        [<JsonNull>]
        Overlay: string option
        /// Object layer: wall, door, … renders on top of overlay
        [<JsonNull>]
        Object: string option
        Crop: Crop option
        Item: Item option
        /// Gathering node instance (tree/rock/…) occupying this tile.
        Node: TileNode option
        /// Placed machine instance (M4 crafting).
        Machine: TileMachine option
        /// Mine ladder going down (M4 mining, generated floors).
        LadderDown: bool option
        Collision: bool
        Visuals: TileVisuals option
        /// One of `SoilStates`.
        SoilState: string option
        SoilMoisture: float
        SoilFertility: float
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `Tile` with every field at its schema default.
    static member Default : Tile =
        { X = 0.0; Y = 0.0; Type = ""; Background = ""; Overlay = None; Object = None; Crop = None; Item = None; Node = None; Machine = None; LadderDown = None; Collision = false; Visuals = None; SoilState = None; SoilMoisture = 0.0; SoilFertility = 0.0; Extra = [] }

/// Live machine instance on a tile.
/// A kind of ground, path or object a tile can be (M1), and its rules. The built-in terrains are
/// grass, soil, water, path, wall, door and floor.
and TerrainDefinition =
    {
        Id: string
        Name: string
        /// One of `TileLayers`: the layer a tile of it is painted on.
        Layer: string
        /// `#rrggbb`: the editor's swatch, and how the game draws it without art.
        Color: string
        Visual: VisualRef option
        /// The player and NPCs can walk on it.
        Walkable: bool
        /// Crops are planted on it (soil).
        Plantable: bool
        /// Counts as water for crops that need water nearby.
        WaterSource: bool
        /// The fishing action works on it.
        Fishable: bool
        /// A crop on a tile of it gets no light.
        BlocksLight: bool
        /// Energy the player loses on stepping onto it. nonnegative.
        HazardEnergy: float
        /// Keeps the weather off its tile, as an indoor scene does (greenhouse floors).
        Indoor: bool
        /// Health the player loses on stepping onto it, with combat on (M7); absent: none. int,
        /// nonnegative.
        Damage: float option
        /// What tools turn it into.
        Transforms: TerrainTransform list
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `TerrainDefinition` with every field at its schema default.
    static member Default : TerrainDefinition =
        { Id = ""; Name = ""; Layer = "background"; Color = ""; Visual = None; Walkable = true; Plantable = false; WaterSource = false; Fishable = false; BlocksLight = false; HazardEnergy = 0.0; Indoor = false; Damage = None; Transforms = []; Extra = [] }

/// A terrain change a tool makes.
and TerrainTransform =
    {
        /// One of `ToolActions`: the action that makes the change (`till`, `transform` or `break`).
        Action: string
        /// Only this tool makes it; absent: every tool with the action.
        Tool: string option
        /// The terrain it becomes; empty removes an overlay or object.
        Into: string
    }

    /// A `TerrainTransform` with every field at its schema default.
    static member Default : TerrainTransform =
        { Action = ""; Tool = None; Into = "" }

and TileMachine =
    {
        TypeId: string
        /// In-flight job: recipe + absolute game-minute it completes.
        Processing: MachineProcessing option
        /// Finished output awaiting collection.
        Output: (RecipeIngredient list) option
        /// What a chest or shipping bin holds (M5).
        Contents: (InventorySlot list) option
        /// A gate stands open (M5).
        Open: bool option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `TileMachine` with every field at its schema default.
    static member Default : TileMachine =
        { TypeId = ""; Processing = None; Output = None; Contents = None; Open = None; Extra = [] }

/// Live node instance state stored on a tile.
and TileNode =
    {
        TypeId: string
        /// int.
        RemainingHealth: float
        /// Set when depleted; used for respawn scheduling. int.
        DepletedOnDay: float option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `TileNode` with every field at its schema default.
    static member Default : TileNode =
        { TypeId = ""; RemainingHealth = 0.0; DepletedOnDay = None; Extra = [] }

/// TS `TileSchema.visuals` (inline object): per-layer art overrides.
and TileVisuals =
    {
        Background: VisualRef option
        Overlay: VisualRef option
        Object: VisualRef option
    }

    /// A `TileVisuals` with every field at its schema default.
    static member Default : TileVisuals =
        { Background = None; Overlay = None; Object = None }

and TimeConfig =
    {
        /// Minute-of-day the player wakes up (6:00). int.
        DayStartMinute: float
        /// Minute-of-day the player collapses if still awake (26:00 = 2am). int.
        DayEndMinute: float
        /// In-game minutes that pass per real-time second. positive, at most 1440.
        MinutesPerRealSecond: float
        /// Whether the clock stops while a dialogue, shop, minigame or menu is open. Absent
        /// means on; v8 ran the clock on.
        PauseInModals: bool option
        /// Minutes in a day (M1); absent: 1440. int, at least 60.
        DayMinutes: float option
    }

    /// A `TimeConfig` with every field at its schema default.
    static member Default : TimeConfig =
        { DayStartMinute = 360.0; DayEndMinute = 1560.0; MinutesPerRealSecond = 1.0; PauseInModals = None; DayMinutes = None }

/// The area a tool covers at one tier: `Width` tiles across the player's facing, `Length` tiles ahead.
and ToolArea =
    {
        /// Odd widths centre on the faced tile. int, positive.
        Width: float
        /// int, positive.
        Length: float
    }

    /// A `ToolArea` with every field at its schema default.
    static member Default : ToolArea =
        { Width = 1.0; Length = 1.0 }

/// A tool (M1): what it does, what it costs and its key. The built-in tools are the watering can,
/// hoe, axe, pickaxe, scythe and fishing rod.
and ToolDefinition =
    {
        Id: string
        Name: string
        Description: string
        /// One of `ToolActions`: water, till, harvest, fish, break, transform, place, plant or
        /// attack (a weapon, M7: the item's `ToolPower` is the damage). Any tool strikes the
        /// gathering node it faces.
        Action: string
        /// The play-mode key that uses it (one character); absent: none.
        Key: string option
        /// Energy a tier 1 use costs; each tier above costs 15% less. nonnegative.
        EnergyCost: float
        /// The area it covers by tier (the first entry is tier 1, the last covers the tiers above).
        Area: ToolArea list
        /// The skill that gains experience when it breaks a node; absent: foraging.
        Skill: string option
        /// The terrain the `place` action puts down.
        PlaceTerrain: string option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `ToolDefinition` with every field at its schema default.
    static member Default : ToolDefinition =
        { Id = ""; Name = ""; Description = ""; Action = ""; Key = None; EnergyCost = 0.0; Area = []; Skill = None; PlaceTerrain = None; Extra = [] }

and VisualRef =
    {
        AssetId: string
        Animation: string option
        Frame: ArtFrame option
    }

    /// A `VisualRef` with every field at its schema default.
    static member Default : VisualRef =
        { AssetId = ""; Animation = None; Frame = None }

and WeatherConfig =
    {
        Types: WeatherTypeDefinition list
        /// Per-season weighted roll tables (rolled at each day start).
        Table: (string * WeatherTableEntry list) list
        /// The game's own overlays (M1), beside and over the built-in rain and snow.
        Overlays: (WeatherOverlayDefinition list) option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `WeatherConfig` with every field at its schema default.
    static member Default : WeatherConfig =
        { Types = []; Table = []; Overlays = None; Extra = [] }

/// How a weather looks on screen (M1): particles and a tint. The built-in overlays are rain and snow.
and WeatherOverlayDefinition =
    {
        Id: string
        /// One of `OverlayStyles`: `rain` (streaks), `snow` (flakes), `dust` (drifting specks) or
        /// `none` (the tint only).
        Style: string
        /// `#rrggbb` of the particles.
        Color: string
        /// Particles relative to the built-in amount. nonnegative.
        Density: float
        /// Speed relative to the built-in style. nonnegative.
        Speed: float
        /// `#rrggbb` multiplied over the scene while it falls; absent: none.
        Tint: string option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `WeatherOverlayDefinition` with every field at its schema default.
    static member Default : WeatherOverlayDefinition =
        { Id = ""; Style = "rain"; Color = ""; Density = 1.0; Speed = 1.0; Tint = None; Extra = [] }

and WeatherTableEntry =
    {
        WeatherId: string
        /// positive.
        Weight: float
    }

    /// A `WeatherTableEntry` with every field at its schema default.
    static member Default : WeatherTableEntry =
        { WeatherId = ""; Weight = 0.0 }

and WeatherTypeDefinition =
    {
        Id: string
        Name: string
        /// Rain-like: outdoor soil is watered automatically at day start.
        WatersOutdoorSoil: bool
        /// Chance (0..1) each outdoor crop is destroyed overnight (storms).
        CropDamageChance: float
        /// NPCs with schedules stay home (skip schedule walking).
        NpcsStayInside: bool
        /// The overlay it shows (a `WeatherOverlayDefinition` id: `rain`, `snow` or the game's own) or null.
        [<JsonNull>]
        Overlay: string option
        /// A music asset that plays in this weather, outdoors (M2).
        Music: string option
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `WeatherTypeDefinition` with every field at its schema default.
    static member Default : WeatherTypeDefinition =
        { Id = ""; Name = ""; WatersOutdoorSoil = false; CropDamageChance = 0.0; NpcsStayInside = false; Overlay = None; Music = None; Extra = [] }

/// A line an NPC may say when the player talks to it (M4): in place of its first dialogue's
/// text, on days when its conditions hold. Lines whose conditions hold share the day by weight.
and NpcLine =
    {
        Text: string
        /// The events' conditions; an empty `npcId` in `hearts` and `relationship` is this NPC.
        Conditions: EventCondition list
        /// The portrait the dialogue box shows with the line (one of the NPC's `portraits`).
        Expression: string option
        /// How often the line comes up next to the others that hold (absent: 1). int.
        Weight: float option
    }

    /// A `NpcLine` with every field at its schema default.
    static member Default : NpcLine =
        { Text = ""; Conditions = []; Expression = None; Weight = None }

/// A portrait an NPC shows in the dialogue box for an expression (M4).
and NpcPortrait =
    {
        /// `happy`, `sad`… any name; lines, dialogues and cutscene lines pick it by this name.
        Expression: string
        Visual: VisualRef
    }

    /// A `NpcPortrait` with every field at its schema default.
    static member Default : NpcPortrait =
        { Expression = ""; Visual = VisualRef.Default }

/// A cutscene that plays once when the player has enough hearts with an NPC (M4).
and HeartEvent =
    {
        Id: string
        Name: string
        NpcId: string
        /// Whole hearts needed. int.
        Hearts: float
        CutsceneId: string
        /// Where it plays: on entering this scene. Absent or empty: when the player talks to the NPC.
        SceneId: string option
        /// More conditions (the events'); an empty `npcId` is the event's NPC.
        Conditions: EventCondition list
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `HeartEvent` with every field at its schema default.
    static member Default : HeartEvent =
        { Id = ""; Name = ""; NpcId = ""; Hearts = 0.0; CutsceneId = ""; SceneId = None; Conditions = []; Extra = [] }

/// A game's relationship rules (M4). Every part is off until the creator turns it on.
and RelationshipSettings =
    {
        /// Friendship points an NPC loses on a day the player neither talked to it nor gave it a gift (0: no decay). int.
        DecayPerDay: float
        /// Romanceable NPCs can be dated.
        Dating: bool
        /// Hearts a romanceable NPC needs before it accepts the dating gift. int.
        DatingHearts: float
        /// The gift that asks to date (absent: dating starts at the hearts by itself).
        DatingItemId: string option
        /// Dated NPCs can be married.
        Marriage: bool
        /// Hearts a dated NPC needs before it accepts the proposal. int.
        ProposalHearts: float
        /// The gift that proposes (needed for marriage).
        ProposalItemId: string option
        /// Days from the engagement to the wedding. int.
        WeddingDays: float
        /// Plays on the wedding morning.
        WeddingCutsceneId: string option
        /// Where a spouse lives (absent: the spouse keeps its own place).
        HomeSceneId: string option
        HomeX: float option
        HomeY: float option
        /// A spouse helps each morning: waters the crops, feeds the animals or brings a gift.
        SpouseHelps: bool
        /// What a spouse may bring in the morning.
        SpouseGiftItemIds: string list
        /// Married couples have children.
        Children: bool
        /// Days married before a child arrives, and between children. int.
        ChildDays: float
        /// int.
        MaxChildren: float
        /// Names the children get, in order.
        ChildNames: string list
        /// Undeclared keys, in order.
        Extra: (string * Json) list
    }

    /// A `RelationshipSettings` with every field at its schema default.
    static member Default : RelationshipSettings =
        { DecayPerDay = 0.0; Dating = false; DatingHearts = 8.0; DatingItemId = None; Marriage = false; ProposalHearts = 10.0; ProposalItemId = None; WeddingDays = 3.0; WeddingCutsceneId = None; HomeSceneId = None; HomeX = None; HomeY = None; SpouseHelps = true; SpouseGiftItemIds = []; Children = false; ChildDays = 14.0; MaxChildren = 2.0; ChildNames = []; Extra = [] }

/// Where and how a piece of clothing is worn (M4).
and WearDefinition =
    {
        /// One of `WearSlots`.
        Slot: string
        /// The colour it gives the shirt, the trousers or the hat (`#rrggbb`).
        Color: string option
        /// Art drawn over the player (a hat), facing like the player.
        Visual: VisualRef option
    }

    /// A `WearDefinition` with every field at its schema default.
    static member Default : WearDefinition =
        { Slot = ""; Color = None; Visual = None }

/// Player entered a tile (or region when x2/y2 present).
and EnterTileCondition =
    {
        X: float
        Y: float
        X2: float option
        Y2: float option
    }

    static member Default : EnterTileCondition =
        { X = 0.0; Y = 0.0; X2 = None; Y2 = None }

/// Player interacted while facing a tile (or region).
and InteractTileCondition =
    {
        X: float
        Y: float
        X2: float option
        Y2: float option
    }

    static member Default : InteractTileCondition =
        { X = 0.0; Y = 0.0; X2 = None; Y2 = None }

and HasItemCondition =
    {
        ItemId: string
        Quantity: float
    }

    static member Default : HasItemCondition =
        { ItemId = ""; Quantity = 1.0 }

and InventorySpaceCondition =
    {
        ItemId: string
        /// int, positive.
        Quantity: float
    }

    static member Default : InventorySpaceCondition =
        { ItemId = ""; Quantity = 1.0 }

and FlagCondition =
    {
        Flag: string
        Value: bool
    }

    static member Default : FlagCondition =
        { Flag = ""; Value = true }

and DayRangeCondition =
    {
        MinDay: float option
        MaxDay: float option
    }

    static member Default : DayRangeCondition =
        { MinDay = None; MaxDay = None }

and SeasonCondition =
    {
        Seasons: string list
    }

    static member Default : SeasonCondition =
        { Seasons = [] }

and YearRangeCondition =
    {
        MinYear: float option
        MaxYear: float option
    }

    static member Default : YearRangeCondition =
        { MinYear = None; MaxYear = None }

and TimeOfDayCondition =
    {
        MinMinute: float
        MaxMinute: float
    }

    static member Default : TimeOfDayCondition =
        { MinMinute = 0.0; MaxMinute = 0.0 }

and QuestStatusCondition =
    {
        QuestId: string
        /// One of `QuestStatuses`.
        Status: string
    }

    static member Default : QuestStatusCondition =
        { QuestId = ""; Status = "" }

/// Friendship threshold with an NPC (M4 heart events).
and FriendshipCondition =
    {
        NpcId: string
        Min: float
    }

    static member Default : FriendshipCondition =
        { NpcId = ""; Min = 0.0 }

/// Current weather (M4).
and WeatherCondition =
    {
        WeatherIds: string list
    }

    static member Default : WeatherCondition =
        { WeatherIds = [] }

/// Today is the named festival (M9 calendar).
and FestivalIdCondition =
    {
        FestivalId: string
    }

    static member Default : FestivalIdCondition =
        { FestivalId = "" }

/// A variable (a flag holding a number or text, M3) compared with a number (`Value`) or, when
/// `Text` is set, with a text.
and VariableCondition =
    {
        Name: string
        /// One of `VariableComparisons`.
        Compare: string
        Value: float option
        Text: string option
    }

    static member Default : VariableCondition =
        { Name = ""; Compare = "eq"; Value = None; Text = None }

/// Every condition of the group holds (M3).
and AllCondition =
    {
        Conditions: EventCondition list
    }

    static member Default : AllCondition =
        { Conditions = [] }

/// At least one condition of the group holds (M3).
and AnyCondition =
    {
        Conditions: EventCondition list
    }

    static member Default : AnyCondition =
        { Conditions = [] }

/// None of the conditions of the group holds (M3).
and NotCondition =
    {
        Conditions: EventCondition list
    }

    static member Default : NotCondition =
        { Conditions = [] }

/// A random chance (M3). 0..1.
and ChanceCondition =
    {
        Chance: float
    }

    static member Default : ChanceCondition =
        { Chance = 0.0 }

/// The player is in one of these scenes (M3).
and SceneCondition =
    {
        SceneIds: string list
    }

    static member Default : SceneCondition =
        { SceneIds = [] }

/// Whole hearts with an NPC within a range (M4); an empty `NpcId` is the NPC the player is
/// talking to.
and HeartsCondition =
    {
        NpcId: string
        Min: float option
        Max: float option
    }

    static member Default : HeartsCondition =
        { NpcId = ""; Min = None; Max = None }

/// The day of the season within a range (M4).
and DayOfSeasonCondition =
    {
        MinDay: float option
        MaxDay: float option
    }

    static member Default : DayOfSeasonCondition =
        { MinDay = None; MaxDay = None }

/// The player's relationship with an NPC (M4), one of `RelationshipStatuses`; an empty `NpcId`
/// is the NPC the player is talking to, else anyone.
and RelationshipCondition =
    {
        NpcId: string
        Status: string
    }

    static member Default : RelationshipCondition =
        { NpcId = ""; Status = "" }

/// TS `EventConditionSchema`: a union on `type`.
and [<RequireQualifiedAccess>] EventCondition =
    | EnterTile of EnterTileCondition
    | InteractTile of InteractTileCondition
    | HasItem of HasItemCondition
    | InventorySpace of InventorySpaceCondition
    | Flag of FlagCondition
    | DayRange of DayRangeCondition
    | Season of SeasonCondition
    | YearRange of YearRangeCondition
    | TimeOfDay of TimeOfDayCondition
    | QuestStatus of QuestStatusCondition
    | Friendship of FriendshipCondition
    | Weather of WeatherCondition
    | FestivalId of FestivalIdCondition
    | Variable of VariableCondition
    | All of AllCondition
    | Any of AnyCondition
    | Not of NotCondition
    | Chance of ChanceCondition
    | Scene of SceneCondition
    | Hearts of HeartsCondition
    | DayOfSeason of DayOfSeasonCondition
    | Relationship of RelationshipCondition
    | Money of MoneyCondition
    | SkillLevel of SkillLevelCondition
    | MineFloor of MineFloorCondition
    | Collection of CollectionCondition
    | Bundle of BundleCondition
    | Achievement of AchievementCondition
    | Shipped of ShippedCondition
    | Building of BuildingCondition
    | Slain of SlainCondition
    | Birthday of BirthdayCondition

    /// The `type` discriminator.
    member this.Type =
        match this with
        | EnterTile _ -> "enterTile"
        | InteractTile _ -> "interactTile"
        | HasItem _ -> "hasItem"
        | InventorySpace _ -> "inventorySpace"
        | Flag _ -> "flag"
        | DayRange _ -> "dayRange"
        | Season _ -> "season"
        | YearRange _ -> "yearRange"
        | TimeOfDay _ -> "timeOfDay"
        | QuestStatus _ -> "questStatus"
        | Friendship _ -> "friendship"
        | Weather _ -> "weather"
        | FestivalId _ -> "festivalId"
        | Variable _ -> "variable"
        | All _ -> "all"
        | Any _ -> "any"
        | Not _ -> "not"
        | Chance _ -> "chance"
        | Scene _ -> "scene"
        | Hearts _ -> "hearts"
        | DayOfSeason _ -> "dayOfSeason"
        | Relationship _ -> "relationship"
        | Money _ -> "money"
        | SkillLevel _ -> "skillLevel"
        | MineFloor _ -> "mineFloor"
        | Collection _ -> "collection"
        | Bundle _ -> "bundle"
        | Achievement _ -> "achievement"
        | Shipped _ -> "shipped"
        | Building _ -> "building"
        | Slain _ -> "slain"
        | Birthday _ -> "birthday"
