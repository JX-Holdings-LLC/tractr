namespace FarmEngine.Authoring

open System
open System.Collections.Generic
open FarmEngine.Schemas

/// What an id in a project names. Pickers list the ids of one kind (`References.options`).
[<RequireQualifiedAccess>]
type ReferenceKind =
    | Item
    | Npc
    | Scene
    | Quest
    | Dialogue
    | Shop
    | Recipe
    | Crop
    | NodeType
    | MachineType
    | AnimalSpecies
    | Animal
    | FishTable
    | Mine
    | Action
    | Minigame
    | Event
    | Weather
    | Season
    | Festival
    | Weekday
    | Skill
    | Asset
    | Pack
    /// A crafting station category that machine types provide (`stationCategories`).
    | StationCategory
    /// The world vocabulary (M1).
    | Terrain
    | Tool
    | ItemCategory
    | CropQuality
    | CropMutation
    /// A weather overlay (M1): the built-in rain and snow or the game's own.
    | Overlay
    /// Sound and music assets and cutscenes (M2).
    | Sound
    | Music
    | Cutscene
    /// Who acts or speaks in a cutscene step (M2): the player or an NPC.
    | Actor
    /// A building type (M5, `BuildingTypes`).
    | Building
    /// A building on the start map (M5, `GameProject.Buildings`).
    | PlacedBuilding
    /// Monsters (M7), collections, bundles, achievements and letters (M6).
    | Monster
    | Collection
    | Bundle
    | Achievement
    | Mail

module ReferenceKind =
    let all =
        [
            ReferenceKind.Item
            ReferenceKind.Npc
            ReferenceKind.Scene
            ReferenceKind.Quest
            ReferenceKind.Dialogue
            ReferenceKind.Shop
            ReferenceKind.Recipe
            ReferenceKind.Crop
            ReferenceKind.NodeType
            ReferenceKind.MachineType
            ReferenceKind.AnimalSpecies
            ReferenceKind.Animal
            ReferenceKind.FishTable
            ReferenceKind.Mine
            ReferenceKind.Action
            ReferenceKind.Minigame
            ReferenceKind.Event
            ReferenceKind.Weather
            ReferenceKind.Season
            ReferenceKind.Festival
            ReferenceKind.Weekday
            ReferenceKind.Skill
            ReferenceKind.Asset
            ReferenceKind.Pack
            ReferenceKind.StationCategory
            ReferenceKind.Terrain
            ReferenceKind.Tool
            ReferenceKind.ItemCategory
            ReferenceKind.CropQuality
            ReferenceKind.CropMutation
            ReferenceKind.Overlay
            ReferenceKind.Sound
            ReferenceKind.Music
            ReferenceKind.Cutscene
            ReferenceKind.Actor
            ReferenceKind.Building
            ReferenceKind.PlacedBuilding
            ReferenceKind.Monster
            ReferenceKind.Collection
            ReferenceKind.Bundle
            ReferenceKind.Achievement
            ReferenceKind.Mail
        ]

    /// The name C# uses ("item", "npc", "nodeType", …).
    let name (kind: ReferenceKind) =
        match kind with
        | ReferenceKind.Item -> "item"
        | ReferenceKind.Npc -> "npc"
        | ReferenceKind.Scene -> "scene"
        | ReferenceKind.Quest -> "quest"
        | ReferenceKind.Dialogue -> "dialogue"
        | ReferenceKind.Shop -> "shop"
        | ReferenceKind.Recipe -> "recipe"
        | ReferenceKind.Crop -> "crop"
        | ReferenceKind.NodeType -> "nodeType"
        | ReferenceKind.MachineType -> "machineType"
        | ReferenceKind.AnimalSpecies -> "animalSpecies"
        | ReferenceKind.Animal -> "animal"
        | ReferenceKind.FishTable -> "fishTable"
        | ReferenceKind.Mine -> "mine"
        | ReferenceKind.Action -> "action"
        | ReferenceKind.Minigame -> "minigame"
        | ReferenceKind.Event -> "event"
        | ReferenceKind.Weather -> "weather"
        | ReferenceKind.Season -> "season"
        | ReferenceKind.Festival -> "festival"
        | ReferenceKind.Weekday -> "weekday"
        | ReferenceKind.Skill -> "skill"
        | ReferenceKind.Asset -> "asset"
        | ReferenceKind.Pack -> "pack"
        | ReferenceKind.StationCategory -> "stationCategory"
        | ReferenceKind.Terrain -> "terrain"
        | ReferenceKind.Tool -> "tool"
        | ReferenceKind.ItemCategory -> "itemCategory"
        | ReferenceKind.CropQuality -> "cropQuality"
        | ReferenceKind.CropMutation -> "cropMutation"
        | ReferenceKind.Overlay -> "overlay"
        | ReferenceKind.Sound -> "sound"
        | ReferenceKind.Music -> "music"
        | ReferenceKind.Cutscene -> "cutscene"
        | ReferenceKind.Actor -> "actor"
        | ReferenceKind.Building -> "building"
        | ReferenceKind.PlacedBuilding -> "placedBuilding"
        | ReferenceKind.Monster -> "monster"
        | ReferenceKind.Collection -> "collection"
        | ReferenceKind.Bundle -> "bundle"
        | ReferenceKind.Achievement -> "achievement"
        | ReferenceKind.Mail -> "mail"

    let tryParse (text: string) =
        all |> List.tryFind (fun kind -> name kind = text)

/// One entry of a picker: the id to store and the text to show. `Missing` marks the entry that
/// keeps a stored id the project no longer has, so the creator sees it instead of losing it.
type PickerOption =
    {
        Id: string
        Label: string
        Missing: bool
    }

/// What a schema property holds (docs/dev/ARCHITECTURE.md: the editor asks F# what a field means).
[<RequireQualifiedAccess>]
type FieldRole =
    /// The id of one thing. `empty` labels the "nothing chosen" entry when empty is allowed.
    | Reference of kind: ReferenceKind * empty: string option
    /// A list of ids.
    | ReferenceList of kind: ReferenceKind
    /// A dictionary keyed by ids (weather table rows by season, social state by NPC).
    | ReferenceKeys of kind: ReferenceKind
    /// One of fixed values, as (value, label) pairs.
    | OneOf of choices: (string * string) list * empty: string option
    /// Named like a reference but deliberately not one: the reason says what it holds.
    | NotReference of reason: string

/// A field of a schema record (`Owner` is the record type name, `Property` the field name) and
/// what it holds.
type FieldDeclaration =
    {
        Owner: string
        Property: string
        Role: FieldRole
    }

/// Every schema property that holds an id of another thing, and the enum-like strings the editor
/// shows as choices. The `Id` property of a record is its own id and is never declared. A test
/// walks the records reachable from `GameProject` and fails when an id-like property has no
/// entry here, so a new reference field cannot slip into the schema without a picker.
module References =
    let private none = Some "(none)"

    let private reference owner property kind =
        {
            Owner = owner
            Property = property
            Role = FieldRole.Reference(kind, None)
        }

    let private optional owner property kind =
        {
            Owner = owner
            Property = property
            Role = FieldRole.Reference(kind, none)
        }

    let private optionalAs owner property kind empty =
        {
            Owner = owner
            Property = property
            Role = FieldRole.Reference(kind, Some empty)
        }

    let private list owner property kind =
        {
            Owner = owner
            Property = property
            Role = FieldRole.ReferenceList kind
        }

    let private keys owner property kind =
        {
            Owner = owner
            Property = property
            Role = FieldRole.ReferenceKeys kind
        }

    let private plain owner property reason =
        {
            Owner = owner
            Property = property
            Role = FieldRole.NotReference reason
        }

    let private same (values: seq<string>) =
        values |> Seq.map (fun v -> v, v) |> List.ofSeq

    let private oneOf owner property (choices: (string * string) list) =
        {
            Owner = owner
            Property = property
            Role = FieldRole.OneOf(choices, None)
        }

    let private optionalOneOf owner property (choices: (string * string) list) =
        {
            Owner = owner
            Property = property
            Role = FieldRole.OneOf(choices, none)
        }

    /// NPCEditor appearance select.
    let appearances =
        [
            "villager", "Villager"
            "farmer", "Farmer"
            "merchant", "Merchant"
            "elder", "Elder"
            "child", "Child"
            "guard", "Guard"
        ]

    /// QuestEditor objective type select (plus `gift`, which the engine also tracks).
    let objectiveTypes =
        [
            QuestObjectiveTypes.Collect, "Collect Item"
            QuestObjectiveTypes.Harvest, "Harvest Crop"
            QuestObjectiveTypes.Talk, "Talk to NPC"
            QuestObjectiveTypes.Visit, "Visit Scene"
            QuestObjectiveTypes.Craft, "Craft Item"
            QuestObjectiveTypes.Gift, "Give Gift"
            QuestObjectiveTypes.Ship, "Ship Item"
            QuestObjectiveTypes.Catch, "Catch Fish"
            QuestObjectiveTypes.Slay, "Slay Monster"
            QuestObjectiveTypes.Hearts, "Reach Hearts"
            QuestObjectiveTypes.Bundle, "Complete Bundle"
            QuestObjectiveTypes.MineFloor, "Reach Mine Floor"
        ]

    let private flag =
        "a flag name: flags are free text that events, actions and dialogue set and check"

    let private variable =
        "a variable name: variables are flags that hold a number or a text (M3)"

    let private expression = "a portrait expression of the NPC (its `Portraits`)"

    /// The event triggers, with the editor's labels (M3).
    let triggers =
        [
            EventTriggers.Enter, "Enter a tile"
            EventTriggers.Interact, "Interact with a tile"
            EventTriggers.Tick, "Every minute"
            EventTriggers.DayStart, "Each morning"
            EventTriggers.Time, "At a time of day"
            EventTriggers.Harvest, "Harvesting"
            EventTriggers.Sell, "Selling"
            EventTriggers.Craft, "Crafting"
            EventTriggers.Catch, "Catching a fish"
            EventTriggers.PickUp, "Picking up an item"
            EventTriggers.Friendship, "Friendship changes"
            EventTriggers.Quest, "A quest starts or ends"
            EventTriggers.Build, "A building is finished"
            EventTriggers.Bundle, "A bundle is completed"
            EventTriggers.Achievement, "An achievement is unlocked"
            EventTriggers.Slay, "Slaying a monster"
        ]

    /// How items join a collection (M6), with the editor's labels.
    let collectionKinds =
        [
            CollectionKinds.Found, "Found (held once)"
            CollectionKinds.Shipped, "Shipped (sold in a bin)"
            CollectionKinds.Donated, "Donated (handed in)"
        ]

    /// How monsters move (M7), with the editor's labels.
    let monsterMovements =
        [
            MonsterMovements.Chase, "Chases the player"
            MonsterMovements.Wander, "Wanders"
            MonsterMovements.Still, "Stays still"
        ]

    /// What placed objects do around them (M5), with the editor's labels.
    let areaEffects =
        [
            AreaEffects.Water, "Waters the soil (sprinkler)"
            AreaEffects.Protect, "Protects crops (scarecrow)"
        ]

    /// The relationship stages, with the editor's labels (M4).
    let relationshipStatuses =
        [
            RelationshipStatuses.Single, "Single"
            RelationshipStatuses.Dating, "Dating"
            RelationshipStatuses.Engaged, "Engaged"
            RelationshipStatuses.Married, "Married"
        ]

    let declarations: FieldDeclaration list =
        [ // Project
            reference "GameProject" "StartSceneId" ReferenceKind.Scene
            oneOf "GameProject" "Mode" (same EditorModes.All)
            reference "GameProject" "SelectedTileType" ReferenceKind.Terrain
            optional "GameProject" "SelectedNpcId" ReferenceKind.Npc
            optional "GameProject" "SelectedItemId" ReferenceKind.Item
            reference "GameProject" "CurrentSeason" ReferenceKind.Season
            optional "GameProject" "CurrentWeatherId" ReferenceKind.Weather
            plain "GameProject" "EventFlags" flag
            keys "GameProject" "SocialState" ReferenceKind.Npc
            // Player start
            oneOf "Player" "Direction" (same Directions.All)
            reference "Player" "SceneId" ReferenceKind.Scene
            list "Player" "ActiveQuests" ReferenceKind.Quest
            list "Player" "CompletedQuests" ReferenceKind.Quest
            optional "Player" "EquippedTool" ReferenceKind.Tool
            keys "Player" "Skills" ReferenceKind.Skill
            // World
            list "Scene" "Npcs" ReferenceKind.Npc
            list "Scene" "Events" ReferenceKind.Event
            reference "SceneTransition" "ToSceneId" ReferenceKind.Scene
            reference "Tile" "Type" ReferenceKind.Terrain
            reference "Tile" "Background" ReferenceKind.Terrain
            optional "Tile" "Overlay" ReferenceKind.Terrain
            optional "Tile" "Object" ReferenceKind.Terrain
            optionalOneOf "Tile" "SoilState" (same SoilStates.All)
            plain "Tile" "Decor" "terrain ids by extra layer id (M8); checked by `scene.unknownTerrain` and `scene.unknownLayer`"
            reference "Crop" "Type" ReferenceKind.Crop
            reference "Crop" "Quality" ReferenceKind.CropQuality
            optional "Crop" "Mutation" ReferenceKind.CropMutation
            plain "Crop" "MultiTileId" "groups the tiles of one planted multi-tile crop"
            reference "TileNode" "TypeId" ReferenceKind.NodeType
            reference "TileMachine" "TypeId" ReferenceKind.MachineType
            reference "MachineProcessing" "RecipeId" ReferenceKind.Recipe
            // Art
            reference "VisualRef" "AssetId" ReferenceKind.Asset
            plain "VisualRef" "Animation" "a clip name on the chosen asset"
            optionalAs "ArtFrame" "AssetId" ReferenceKind.Asset "(same image)"
            oneOf "CustomAsset" "Type" (same CustomAssetTypes.All)
            optional "CustomAsset" "TileType" ReferenceKind.Terrain
            // Items and crops
            reference "Item" "Type" ReferenceKind.ItemCategory
            optional "Item" "CropType" ReferenceKind.Crop
            optional "Item" "ToolType" ReferenceKind.Tool
            optional "Item" "UseActionId" ReferenceKind.Action
            list "CropDefinition" "Seasons" ReferenceKind.Season
            optionalAs "CropDefinition" "HarvestItemId" ReferenceKind.Item "(crop-<id>)"
            list "CustomCropDefinition" "Seasons" ReferenceKind.Season
            optionalAs "CustomCropDefinition" "HarvestItemId" ReferenceKind.Item "(crop-<id>)"
            optional "InventorySlot" "Quality" ReferenceKind.CropQuality
            // Characters and dialogue
            reference "Npc" "SceneId" ReferenceKind.Scene
            optionalOneOf "Npc" "MovePattern" (same NpcMovePatterns.All)
            oneOf "Npc" "Appearance" appearances
            reference "NpcBirthday" "Season" ReferenceKind.Season
            reference "NpcScheduleEntry" "SceneId" ReferenceKind.Scene
            list "GiftTastes" "Loved" ReferenceKind.Item
            list "GiftTastes" "Liked" ReferenceKind.Item
            list "GiftTastes" "Disliked" ReferenceKind.Item
            list "GiftTastes" "Hated" ReferenceKind.Item
            reference "Dialogue" "NpcId" ReferenceKind.Npc
            optionalAs "DialogueOption" "NextDialogueId" ReferenceKind.Dialogue "(end conversation)"
            optional "DialogueOption" "GiveItem" ReferenceKind.Item
            plain "DialogueOption" "EventFlag" flag
            plain "DialogueOption" "HiddenIfFlag" flag
            optional "DialogueOption" "RequiresItem" ReferenceKind.Item
            plain "DialogueOption" "RequiresFlag" flag
            optional "DialogueOption" "OpenShopId" ReferenceKind.Shop
            optional "DialogueOption" "OfferQuestId" ReferenceKind.Quest
            optional "DialogueOption" "ActionId" ReferenceKind.Action
            // Quests
            optional "Quest" "Giver" ReferenceKind.Npc
            oneOf "Quest" "Status" (same QuestStatuses.All)
            list "Quest" "Prerequisites" ReferenceKind.Quest
            list "Quest" "AvailableSeasons" ReferenceKind.Season
            optional "QuestObjective" "TargetMonsterId" ReferenceKind.Monster
            optional "QuestObjective" "TargetBundleId" ReferenceKind.Bundle
            optionalAs "QuestObjective" "TargetMineId" ReferenceKind.Mine "(original mine)"
            oneOf "QuestObjective" "Type" objectiveTypes
            optional "QuestObjective" "TargetItemId" ReferenceKind.Item
            optional "QuestObjective" "TargetCropType" ReferenceKind.Crop
            optional "QuestObjective" "TargetNpcId" ReferenceKind.Npc
            optional "QuestObjective" "TargetSceneId" ReferenceKind.Scene
            reference "QuestRewardItem" "ItemId" ReferenceKind.Item
            optionalAs "QuestRewards" "Skill" ReferenceKind.Skill "(farming)"
            // Events and the condition/outcome vocabulary
            optionalAs "GameEvent" "SceneId" ReferenceKind.Scene "(every scene)"
            oneOf "GameEvent" "Trigger" triggers
            plain "GameEvent" "Subject" "an item, an NPC or a quest, by the event's trigger"
            reference "HasItemCondition" "ItemId" ReferenceKind.Item
            reference "InventorySpaceCondition" "ItemId" ReferenceKind.Item
            plain "FlagCondition" "Flag" flag
            list "SeasonCondition" "Seasons" ReferenceKind.Season
            reference "QuestStatusCondition" "QuestId" ReferenceKind.Quest
            oneOf "QuestStatusCondition" "Status" (same QuestStatuses.All)
            reference "FriendshipCondition" "NpcId" ReferenceKind.Npc
            list "WeatherCondition" "WeatherIds" ReferenceKind.Weather
            list "WeekdayCondition" "WeekdayIds" ReferenceKind.Weekday
            reference "FestivalProgram" "SceneId" ReferenceKind.Scene
            reference "FestivalProgram" "EntranceSceneId" ReferenceKind.Scene
            optional "FestivalProgram" "ShopId" ReferenceKind.Shop
            reference "FestivalAttendance" "NpcId" ReferenceKind.Npc
            optional "FestivalAttendance" "DialogueId" ReferenceKind.Dialogue
            optional "FestivalActivity" "MinigameId" ReferenceKind.Minigame
            optional "FestivalActivity" "CutsceneId" ReferenceKind.Cutscene
            optional "FestivalActivity" "ActionId" ReferenceKind.Action
            reference "FestivalContest" "ItemId" ReferenceKind.Item
            optional "FestivalContest" "MinimumQuality" ReferenceKind.CropQuality
            reference "FestivalIdCondition" "FestivalId" ReferenceKind.Festival
            oneOf "EventOutcome" "Type" (same EventOutcomeTypes.All)
            optional "EventOutcome" "ItemId" ReferenceKind.Item
            plain "EventOutcome" "FlagName" flag
            optional "EventOutcome" "QuestId" ReferenceKind.Quest
            optional "EventOutcome" "NpcId" ReferenceKind.Npc
            optional "EventOutcome" "DialogueId" ReferenceKind.Dialogue
            optional "EventOutcome" "NewTileType" ReferenceKind.Terrain
            optional "EventOutcome" "SceneId" ReferenceKind.Scene
            plain "EventOutcome" "SoundId" "a sound or music asset, or a built-in sound name the player plays"
            optional "EventOutcome" "CutsceneId" ReferenceKind.Cutscene
            optional "EventOutcome" "ActionId" ReferenceKind.Action
            optional "EventOutcome" "MinigameId" ReferenceKind.Minigame
            // M3 and M4 conditions and outcomes
            plain "VariableCondition" "Name" variable
            oneOf "VariableCondition" "Compare" (same VariableComparisons.All)
            list "SceneCondition" "SceneIds" ReferenceKind.Scene
            optionalAs "HeartsCondition" "NpcId" ReferenceKind.Npc "(the NPC speaking)"
            optionalAs "RelationshipCondition" "NpcId" ReferenceKind.Npc "(the NPC speaking, or anyone)"
            oneOf "RelationshipCondition" "Status" relationshipStatuses
            plain "EventOutcome" "Variable" variable
            optional "EventOutcome" "WeatherId" ReferenceKind.Weather
            optional "EventOutcome" "Skill" ReferenceKind.Skill
            optional "EventOutcome" "ShopId" ReferenceKind.Shop
            optional "EventOutcome" "NodeTypeId" ReferenceKind.NodeType
            optional "EventOutcome" "MachineTypeId" ReferenceKind.MachineType
            optionalOneOf "EventOutcome" "Status" relationshipStatuses
            // Dialogue and relationships (M4)
            plain "Dialogue" "Expression" expression
            plain "NpcLine" "Expression" expression
            plain "NpcPortrait" "Expression" "the expression's name, which lines and dialogues pick"
            plain "CutsceneStep" "Expression" expression
            reference "HeartEvent" "NpcId" ReferenceKind.Npc
            reference "HeartEvent" "CutsceneId" ReferenceKind.Cutscene
            optionalAs "HeartEvent" "SceneId" ReferenceKind.Scene "(when talking to the NPC)"
            optionalAs "RelationshipSettings" "DatingItemId" ReferenceKind.Item "(none: dating starts at the hearts)"
            optional "RelationshipSettings" "ProposalItemId" ReferenceKind.Item
            optional "RelationshipSettings" "WeddingCutsceneId" ReferenceKind.Cutscene
            optionalAs "RelationshipSettings" "HomeSceneId" ReferenceKind.Scene "(the spouse stays where they live)"
            list "RelationshipSettings" "SpouseGiftItemIds" ReferenceKind.Item
            plain "RelationshipSettings" "ChildNames" "names the children get, in order"
            oneOf "WearDefinition" "Slot" (same WearSlots.All)
            plain "Player" "Name" "the player's name (a new game may ask for it)"
            plain "Player" "FarmName" "the farm's name"
            oneOf "NpcSocialState" "Status" relationshipStatuses
            // Building the farm (M5)
            optionalAs "BuildingDefinition" "InteriorSceneId" ReferenceKind.Scene "(no interior)"
            list "BuildingDefinition" "Scenes" ReferenceKind.Scene
            optionalAs "BuildingDefinition" "UpgradeOf" ReferenceKind.Building "(built new)"
            reference "BuildingState" "BuildingId" ReferenceKind.Building
            reference "BuildingState" "SceneId" ReferenceKind.Scene
            optionalAs "BuildingState" "InteriorSceneId" ReferenceKind.Scene "(made when it is built)"
            optionalAs "BuildingState" "UpgradeTo" ReferenceKind.Building "(no upgrade under way)"
            list "ShopDefinition" "Buildings" ReferenceKind.Building
            list "ShopDefinition" "Animals" ReferenceKind.AnimalSpecies
            list "AnimalSpeciesDefinition" "Housing" ReferenceKind.Building
            reference "AnimalProduct" "ItemId" ReferenceKind.Item
            reference "MoodQuality" "Quality" ReferenceKind.CropQuality
            optionalAs "AnimalState" "BuildingId" ReferenceKind.PlacedBuilding "(no building)"
            optionalAs "Scene" "Building" ReferenceKind.PlacedBuilding "(not a building's interior)"
            oneOf "AreaEffect" "Effect" areaEffects
            optionalAs "FarmSettings" "SceneId" ReferenceKind.Scene "(the start scene)"
            reference "FarmLayout" "SceneId" ReferenceKind.Scene
            // Seasonal forage and deeper fishing.
            list "NodeTypeDefinition" "Seasons" ReferenceKind.Season
            list "NodeDrop" "Seasons" ReferenceKind.Season
            list "ForageSpawnTable" "Seasons" ReferenceKind.Season
            list "ForageSpawnTable" "Terrains" ReferenceKind.Terrain
            plain "ForageSpawnTable" "SceneTags" "all scene tags required for this spawn table"
            list "FishingFilter" "Seasons" ReferenceKind.Season
            list "FishingFilter" "WeatherIds" ReferenceKind.Weather
            list "FishingGear" "FishIds" ReferenceKind.Item
            oneOf "FishingGear" "Kind" [ "bait", "Bait"; "tackle", "Tackle" ]
            optionalAs "FishProduction" "TableId" ReferenceKind.FishTable "(a pond uses a species instead)"
            optionalAs "FishProduction" "SpeciesItemId" ReferenceKind.Item "(a trap uses a fish table instead)"
            // Long-term goals (M6)
            list "CollectionDefinition" "Items" ReferenceKind.Item
            oneOf "CollectionDefinition" "Kind" collectionKinds
            optionalAs "CollectionDefinition" "SceneId" ReferenceKind.Scene "(anywhere)"
            reference "BundleItem" "ItemId" ReferenceKind.Item
            optionalAs "BundleItem" "Quality" ReferenceKind.CropQuality "(any quality)"
            optionalAs "BundleDefinition" "SceneId" ReferenceKind.Scene "(anywhere)"
            plain "AchievementDefinition" "SteamId" "the achievement's id on Steam, for a Steamworks export"
            optionalAs "MailDefinition" "From" ReferenceKind.Npc "(no sender)"
            optional "EventOutcome" "MailId" ReferenceKind.Mail
            optional "QuestRewards" "MailId" ReferenceKind.Mail
            reference "SkillLevelCondition" "Skill" ReferenceKind.Skill
            plain "SkillTraining" "Subject" "an optional item, node type, NPC, recipe or monster id selected by the training action"
            reference "PerkCondition" "Skill" ReferenceKind.Skill
            plain "PerkCondition" "PerkId" "a perk within the selected skill"
            plain "PerkCondition" "ChoiceId" "a choice within the selected perk"
            reference "ToolUpgrade" "FromItemId" ReferenceKind.Item
            reference "ToolUpgrade" "ToItemId" ReferenceKind.Item
            reference "CollectionCondition" "CollectionId" ReferenceKind.Collection
            reference "BundleCondition" "BundleId" ReferenceKind.Bundle
            reference "AchievementCondition" "AchievementId" ReferenceKind.Achievement
            optionalAs "ShippedCondition" "ItemId" ReferenceKind.Item "(everything)"
            reference "BuildingCondition" "BuildingId" ReferenceKind.Building
            optionalAs "SlainCondition" "MonsterId" ReferenceKind.Monster "(any monster)"
            optionalAs "BirthdayCondition" "NpcId" ReferenceKind.Npc "(anyone's)"

            // Combat and adventure (M7)
            optionalAs "CombatSettings" "FaintSceneId" ReferenceKind.Scene "(the start scene)"
            oneOf "MonsterDefinition" "Movement" monsterMovements
            reference "MonsterSpawn" "MonsterId" ReferenceKind.Monster
            reference "MineMonsterWeight" "MonsterId" ReferenceKind.Monster
            reference "MineHazard" "TerrainId" ReferenceKind.Terrain
            reference "MineTreasure" "ItemId" ReferenceKind.Item

            // Economy, crafting, gathering
            optional "ShopDefinition" "KeeperNpcId" ReferenceKind.Npc
            list "ShopClosedOn" "Weekdays" ReferenceKind.Weekday
            list "ShopClosedOn" "Seasons" ReferenceKind.Season
            list "ShopClosedOn" "Festivals" ReferenceKind.Festival
            list "ShopPriceModifier" "Weekdays" ReferenceKind.Weekday
            list "ShopPriceModifier" "Seasons" ReferenceKind.Season
            optional "ShopPriceModifier" "ItemId" ReferenceKind.Item
            reference "ShopStockEntry" "ItemId" ReferenceKind.Item
            list "ShopStockEntry" "Seasons" ReferenceKind.Season
            reference "RecipeIngredient" "ItemId" ReferenceKind.Item
            optionalAs "RecipeDefinition" "MachineTypeId" ReferenceKind.MachineType "(by hand)"
            optionalAs "RecipeDefinition" "RequiresStationCategory" ReferenceKind.StationCategory "(anywhere)"
            reference "RecipeSkillRequirement" "Skill" ReferenceKind.Skill
            optional "RecipeUnlock" "QuestId" ReferenceKind.Quest
            list "RecipeUnlock" "Seasons" ReferenceKind.Season
            optional "MachineTypeDefinition" "ItemId" ReferenceKind.Item
            plain "MachineTypeDefinition" "StationCategories" "the station categories this machine provides (it defines them)"
            reference "NodeTypeDefinition" "RequiredTool" ReferenceKind.Tool
            reference "NodeDrop" "ItemId" ReferenceKind.Item

            // Wildlife
            optionalAs "AnimalSpeciesDefinition" "FeedItemId" ReferenceKind.Item "(grazes for free)"
            reference "AnimalSpeciesDefinition" "ProductItemId" ReferenceKind.Item
            reference "AnimalState" "SpeciesId" ReferenceKind.AnimalSpecies
            reference "AnimalState" "SceneId" ReferenceKind.Scene
            list "FishTable" "Seasons" ReferenceKind.Season
            list "FishTable" "SceneIds" ReferenceKind.Scene
            reference "FishTableEntry" "ItemId" ReferenceKind.Item
            optional "FishTable" "JunkItemId" ReferenceKind.Item

            // Extensibility
            // The built-in kinds; `roleIn` adds the kinds a project's packs define.
            oneOf "MinigameDef" "Kind" MinigameKinds.choices
            plain "MinigameDef" "Config" "settings passed to the minigame kind, keyed by setting name"

            // Settings, weather, mine, interface
            plain "ProjectSettings" "Locale" "a locale code for pack string tables"
            reference "CalendarFestival" "SeasonId" ReferenceKind.Season
            plain "CalendarWeekday" "Words" "weekday names by locale code"
            optional "WeatherTypeDefinition" "Overlay" ReferenceKind.Overlay
            optional "WeatherTypeDefinition" "Music" ReferenceKind.Music
            oneOf "WeatherOverlayDefinition" "Style" (same OverlayStyles.All)
            optional "Scene" "Music" ReferenceKind.Music
            plain "Scene" "Tags" "free-form tags crops can require"
            optional "CalendarSeason" "Music" ReferenceKind.Music
            optional "CalendarFestival" "Music" ReferenceKind.Music
            keys "AtmosphereSettings" "Seasons" ReferenceKind.Season

            // World vocabulary (M1)
            oneOf "TerrainDefinition" "Layer" (same TileLayers.All)
            oneOf "TerrainTransform" "Action" (same ToolActions.Transforming)
            optionalAs "TerrainTransform" "Tool" ReferenceKind.Tool "(any tool)"
            optionalAs "TerrainTransform" "Into" ReferenceKind.Terrain "(remove)"
            oneOf "ToolDefinition" "Action" (same ToolActions.All)
            optionalAs "ToolDefinition" "Skill" ReferenceKind.Skill "(foraging)"
            optional "ToolDefinition" "PlaceTerrain" ReferenceKind.Terrain
            list "GrowthRequirements" "Terrains" ReferenceKind.Terrain
            optionalOneOf "GrowthRequirements" "Light" (same LightNeeds.All)
            plain "GrowthRequirements" "SceneTags" "scene tags (`Scene.Tags`)"
            list "GrowthRequirements" "Fertilizers" ReferenceKind.Item
            oneOf "NeighborRequirement" "Kind" (same NeighborKinds.All)
            plain "NeighborRequirement" "Id" "a crop type or terrain id, by kind"
            keys "GameWords" "Skills" ReferenceKind.Skill
            plain "GameWords" "Locales" "words by locale code"

            // Cutscenes (M2)
            oneOf "CutsceneStep" "Type" (same CutsceneStepTypes.All)
            plain "CutsceneStep" "Label" "a step label jumps and choices go to"
            optional "CutsceneStep" "Actor" ReferenceKind.Actor
            optionalAs "CutsceneStep" "Speaker" ReferenceKind.Actor "(narrator)"
            optionalOneOf "CutsceneStep" "Direction" (same Directions.All)
            optionalOneOf "CutsceneStep" "Emote" (same Emotes.All)
            plain "CutsceneStep" "Goto" "a step label of the cutscene"
            optionalOneOf "CutsceneStep" "Mode" [ "out", "Fade out"; "in", "Fade in" ]
            plain "CutsceneStep" "SoundId" "a sound asset (sound) or a music asset (music)"
            plain "CutsceneChoice" "Goto" "a step label of the cutscene"
            keys "WeatherConfig" "Table" ReferenceKind.Season
            reference "WeatherTableEntry" "WeatherId" ReferenceKind.Weather
            optional "MineConfig" "EntranceSceneId" ReferenceKind.Scene
            optional "MineDefinition" "Music" ReferenceKind.Music
            optionalAs "MineFloorCondition" "MineId" ReferenceKind.Mine "(original mine)"
            reference "RequestBoard" "SceneId" ReferenceKind.Scene
            reference "RequestBoardEntry" "QuestId" ReferenceKind.Quest
            list "RequestBoardEntry" "ItemIds" ReferenceKind.Item
            reference "MineRockWeight" "NodeTypeId" ReferenceKind.NodeType
            plain "GamePanel" "VisibleFlag" flag
            oneOf "GamePanelEntry" "Kind" (same GamePanelEntryKinds.All)
            plain "GamePanelEntry" "Value" "text, an item id, an action id or a flag, depending on the entry kind"

            // Content packs
            reference "PackDependency" "PackId" ReferenceKind.Pack
            plain "PackManifest" "Overrides" "ids from other packs this pack replaces (checked when packs merge)"
            plain "PackPermissions" "Hooks" "plugin hook names the pack may use"
            plain "PackPermissions" "Mutations" "plugin mutation capabilities (docs/PLUGINS.md)"
            plain "PackPermissions" "Minigames" "minigame kinds the pack's plugins may define (docs/PLUGINS.md)"
            plain "PackPlugin" "Hooks" "plugin hook names the plugin handles"
            plain "PackPlugin" "Minigames" "minigame kinds the plugin defines with api.minigame"
            plain "PackContent" "Strings" "locale string tables keyed by locale code"
            optional "PackPlayerStart" "SceneId" ReferenceKind.Scene
            reference "PackStartItem" "ItemId" ReferenceKind.Item

            // Export
            plain "ExportSettings" "GameId" "the stable save-folder identity of the exported game"
            optional "ExportSettings" "IconAssetId" ReferenceKind.Asset
            oneOf "ExportSettings" "PixelScale" [ "integer", "Whole-number scaling (crisp)"; "fit", "Fit the window" ]
            plain "ExportSettings" "Targets" "export target names (windows-x64, linux-x64, web)"
        ]

    let private byProperty =
        let table = Dictionary<string, FieldDeclaration>()

        for declaration in declarations do
            table.[declaration.Owner + "." + declaration.Property] <- declaration

        table

    /// What `owner.property` holds, when it is declared.
    let roleOf (owner: string) (property: string) : FieldRole option =
        match byProperty.TryGetValue(owner + "." + property) with
        | true, declaration -> Some declaration.Role
        | _ -> None

    /// What `owner.property` holds in `project`: `roleOf`, except that a minigame's kind also
    /// offers the kinds the project's enabled packs define (`MinigameKinds.choicesFor`).
    let roleIn (project: GameProject) (owner: string) (property: string) : FieldRole option =
        match owner, property with
        | "MinigameDef", "Kind" -> Some(FieldRole.OneOf(MinigameKinds.choicesFor project, None))
        | _ -> roleOf owner property

    let private label (name: string option) (id: string) =
        match name with
        | None -> id
        | Some name when String.IsNullOrWhiteSpace name || name = id -> id
        | Some name when name.ToLowerInvariant() = id.ToLowerInvariant() -> name
        | Some name -> sprintf "%s (%s)" name id

    let private entries (idOf: 'T -> string) (nameOf: 'T -> string option) (items: 'T list) : PickerOption list =
        let seen = HashSet<string>()

        [
            for item in items do
                let id = idOf item

                if seen.Add id then
                    {
                        Id = id
                        Label = label (nameOf item) id
                        Missing = false
                    }
        ]

    let private named (name: string) = Some name

    let private shorten (text: string) =
        if text.Length > 40 then
            text.Substring(0, 39) + "…"
        else
            text

    /// The ids a picker of `kind` offers, in project order. Items, crops, node types, weather and
    /// the calendar come from the content compiler, so the list holds exactly what the game
    /// resolves (the built-in catalog included where the compiler merges it). Pack content is not
    /// offered: packs stay separate layers until they are imported.
    let options (kind: ReferenceKind) (project: GameProject) : PickerOption list =
        match kind with
        | ReferenceKind.Item -> ContentCompiler.items project |> entries (fun i -> i.Id) (fun i -> named i.Name)
        | ReferenceKind.Npc -> project.Npcs |> entries (fun n -> n.Id) (fun n -> named n.Name)
        | ReferenceKind.Scene -> project.Scenes |> entries (fun s -> s.Id) (fun s -> named s.Name)
        | ReferenceKind.Quest -> project.Quests |> entries (fun q -> q.Id) (fun q -> named q.Name)
        | ReferenceKind.Dialogue ->
            let names = Dictionary<string, string>()

            for npc in project.Npcs do
                names.[npc.Id] <- npc.Name

            let owner (d: Dialogue) =
                match names.TryGetValue d.NpcId with
                | true, name -> name
                | _ -> d.NpcId

            (project.Npcs |> List.collect (fun n -> n.Dialogue)) @ project.Dialogues
            |> entries (fun d -> d.Id) (fun d -> named (sprintf "%s: %s" (owner d) (shorten d.Text)))
        | ReferenceKind.Shop -> project.Shops |> entries (fun s -> s.Id) (fun s -> named s.Name)
        | ReferenceKind.Recipe -> project.Recipes |> entries (fun r -> r.Id) (fun r -> named r.Name)
        | ReferenceKind.Crop ->
            ContentCompiler.mergeCrops project.CustomCrops
            |> List.map snd
            |> entries (fun c -> c.Id) (fun c -> named c.Name)
        | ReferenceKind.NodeType ->
            ContentCompiler.nodeTypes project
            |> entries (fun n -> n.Id) (fun n -> named n.Name)
        | ReferenceKind.MachineType -> project.MachineTypes |> entries (fun m -> m.Id) (fun m -> named m.Name)
        | ReferenceKind.AnimalSpecies -> project.AnimalSpecies |> entries (fun s -> s.Id) (fun s -> named s.Name)
        | ReferenceKind.Animal -> project.Animals |> entries (fun a -> a.Id) (fun a -> named a.Name)
        | ReferenceKind.Mine ->
            {
                Id = "mine"
                Label = "Original mine"
                Missing = false
            }
            :: (defaultArg project.Mines []
                |> entries (fun mine -> mine.Id) (fun mine -> named mine.Name))
        | ReferenceKind.FishTable -> project.FishTables |> entries (fun f -> f.Id) (fun f -> named f.Name)
        | ReferenceKind.Action -> project.Actions |> entries (fun a -> a.Id) (fun a -> named a.Name)
        | ReferenceKind.Minigame -> project.Minigames |> entries (fun m -> m.Id) (fun m -> named m.Name)
        | ReferenceKind.Event -> project.Events |> entries (fun e -> e.Id) (fun e -> named e.Name)
        | ReferenceKind.Weather ->
            (ContentCompiler.weather project).Types
            |> entries (fun w -> w.Id) (fun w -> named w.Name)
        | ReferenceKind.Season ->
            (ContentCompiler.settings project).Calendar.Seasons
            |> entries (fun s -> s.Id) (fun s -> named s.Name)
        | ReferenceKind.Weekday ->
            project.Settings.Calendar.Weekdays
            |> Option.defaultValue []
            |> entries (fun d -> d.Id) (fun d -> named d.Name)
        | ReferenceKind.Festival ->
            (ContentCompiler.settings project).Calendar.Festivals
            |> entries (fun f -> f.Id) (fun f -> named f.Name)
        | ReferenceKind.Skill ->
            (SaveSchema.SkillNames
             @ (defaultArg project.Settings.SkillDefinitions []
                |> List.map (fun skill -> skill.Id)))
            |> List.distinct
            |> entries id (fun _ -> None)
        | ReferenceKind.Asset -> project.CustomAssets |> entries (fun a -> a.Id) (fun a -> named a.Name)
        | ReferenceKind.Pack ->
            project.ContentPacks
            |> entries (fun p -> p.Pack.Manifest.Id) (fun p -> named p.Pack.Manifest.Name)
        | ReferenceKind.StationCategory ->
            project.MachineTypes
            |> List.collect (fun m -> m.StationCategories)
            |> entries id (fun _ -> None)
        | ReferenceKind.Terrain ->
            ContentCompiler.terrains project
            |> entries (fun t -> t.Id) (fun t -> named t.Name)
        | ReferenceKind.Tool -> ContentCompiler.tools project |> entries (fun t -> t.Id) (fun t -> named t.Name)
        | ReferenceKind.ItemCategory ->
            ContentCompiler.itemCategories project
            |> entries (fun c -> c.Id) (fun c -> named c.Name)
        | ReferenceKind.CropQuality ->
            ContentCompiler.cropQualities project
            |> entries (fun q -> q.Id) (fun q -> named q.Name)
        | ReferenceKind.CropMutation ->
            ContentCompiler.cropMutations project
            |> entries (fun m -> m.Id) (fun m -> named m.Name)
        | ReferenceKind.Overlay ->
            [ OverlayStyles.Rain; OverlayStyles.Snow ]
            @ (defaultArg project.Weather.Overlays [] |> List.map (fun o -> o.Id))
            |> entries id (fun _ -> None)
        | ReferenceKind.Sound ->
            project.CustomAssets
            |> List.filter (fun a -> a.Type = CustomAssetTypes.Sound)
            |> entries (fun a -> a.Id) (fun a -> named a.Name)
        | ReferenceKind.Music ->
            project.CustomAssets
            |> List.filter (fun a -> a.Type = CustomAssetTypes.Music)
            |> entries (fun a -> a.Id) (fun a -> named a.Name)
        | ReferenceKind.Cutscene ->
            defaultArg project.Cutscenes []
            |> entries (fun c -> c.Id) (fun c -> named c.Name)
        | ReferenceKind.Actor ->
            ("player", "Player") :: (project.Npcs |> List.map (fun n -> n.Id, n.Name))
            |> entries fst (snd >> named)
        | ReferenceKind.Building ->
            defaultArg project.BuildingTypes []
            |> entries (fun b -> b.Id) (fun b -> named b.Name)
        | ReferenceKind.PlacedBuilding ->
            let names =
                defaultArg project.BuildingTypes [] |> List.map (fun b -> b.Id, b.Name) |> dict

            let kind (b: BuildingState) =
                match names.TryGetValue b.BuildingId with
                | true, name -> name
                | _ -> b.BuildingId

            defaultArg project.Buildings []
            |> entries (fun b -> b.Id) (fun b -> named (sprintf "%s at %s (%g,%g)" (kind b) b.SceneId b.X b.Y))
        | ReferenceKind.Monster ->
            defaultArg project.Monsters []
            |> entries (fun m -> m.Id) (fun m -> named m.Name)
        | ReferenceKind.Collection ->
            defaultArg project.Collections []
            |> entries (fun c -> c.Id) (fun c -> named c.Name)
        | ReferenceKind.Bundle -> defaultArg project.Bundles [] |> entries (fun b -> b.Id) (fun b -> named b.Name)
        | ReferenceKind.Achievement ->
            defaultArg project.Achievements []
            |> entries (fun a -> a.Id) (fun a -> named a.Name)
        | ReferenceKind.Mail ->
            defaultArg project.Mail []
            |> entries (fun m -> m.Id) (fun m -> named (shorten m.Subject))

    /// A picker's entries: the "nothing chosen" entry when `empty` allows it, a "(missing: id)"
    /// entry when `current` names something that is not among `available`, then `available`.
    let pickerEntries (available: PickerOption list) (empty: string option) (current: string option) : PickerOption list =
        let currentId = defaultArg current ""

        let known =
            currentId.Length = 0 || available |> List.exists (fun o -> o.Id = currentId)

        [
            match empty with
            | Some text ->
                {
                    Id = ""
                    Label = text
                    Missing = false
                }
            | None -> ()
            if not known then
                {
                    Id = currentId
                    Label = sprintf "(missing: %s)" currentId
                    Missing = true
                }
            yield! available
        ]

    /// Picker entries for a reference of `kind` holding `current`.
    let picker (kind: ReferenceKind) (empty: string option) (project: GameProject) (current: string option) =
        pickerEntries (options kind project) empty current

    /// Choice entries as picker options (value, label).
    let choiceOptions (choices: (string * string) list) : PickerOption list =
        choices
        |> List.map (fun (value, text) ->
            {
                Id = value
                Label = text
                Missing = false
            })
