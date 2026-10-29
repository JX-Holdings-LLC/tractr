namespace FarmEngine.Authoring

open FarmEngine.Schemas

/// What a form field holds, so the editor can pick a control.
[<RequireQualifiedAccess>]
type FieldKind =
    | Text
    | Integer
    | Number
    /// A switch; the labels say what on and off mean ("is set" / "is NOT set").
    | Bool of onLabel: string * offLabel: string
    | OneOf of choices: (string * string) list
    | Reference of kind: ReferenceKind
    | ReferenceList of kind: ReferenceKind
    /// A nested list of conditions (the `all`, `any` and `not` groups, M3).
    | Conditions

/// One field of a condition or outcome form (event-forms.tsx). `Key` is the JSON key. Clearing
/// an `Optional` field removes it; clearing a required number stores `WhenEmpty` (the web
/// `n ?? 0` / `n ?? 1`). `Min`/`Max` clamp numbers like the web does. `Empty` labels the
/// "nothing chosen" entry of an optional reference picker.
type VocabularyField =
    {
        Key: string
        Label: string
        Kind: FieldKind
        Optional: bool
        Placeholder: string
        WhenEmpty: float
        Min: float option
        Max: float option
        Empty: string option
    }

/// Port of `event-vocabulary.ts` and the per-type fields of `event-forms.tsx`: the condition and
/// outcome types events, actions and minigame tiers share, which fields each type shows, and the
/// defaults a new or re-typed row starts from.
module Vocabulary =
    let private field key label kind =
        {
            Key = key
            Label = label
            Kind = kind
            Optional = false
            Placeholder = ""
            WhenEmpty = 0.0
            Min = None
            Max = None
            Empty = None
        }

    let private integer key label = field key label FieldKind.Integer

    let private optionalInteger key label =
        { integer key label with
            Optional = true
        }

    let private text key label placeholder =
        { field key label FieldKind.Text with
            Placeholder = placeholder
        }

    let private reference key label kind =
        field key label (FieldKind.Reference kind)

    /// `CONDITION_TYPES`, in the web order.
    let conditionTypes =
        [
            "enterTile", "Enter tile/region"
            "interactTile", "Interact with tile"
            "friendship", "Friendship threshold"
            "weather", "Weather"
            "inventorySpace", "Inventory can receive item"
            "hasItem", "Has item ≥ n"
            "flag", "Flag is set/unset"
            "dayRange", "Day range"
            "season", "Season"
            "yearRange", "Year range"
            "timeOfDay", "Time of day"
            "questStatus", "Quest status"
            "festivalId", "Festival"
            "weekday", "Weekday"
            "variable", "Variable compares"
            "all", "All of these"
            "any", "Any of these"
            "not", "None of these"
            "chance", "Random chance"
            "scene", "Player is in scene"
            "hearts", "Hearts with NPC"
            "dayOfSeason", "Day of the season"
            "relationship", "Relationship"
            "money", "Has money ≥ n"
            "perk", "Perk is chosen"
            "buff", "Buff is active"
            "skillLevel", "Skill level ≥ n"
            "mineFloor", "Reached mine floor"
            "collection", "Collection holds items"
            "bundle", "Bundle is complete"
            "achievement", "Achievement unlocked"
            "shipped", "Shipped ≥ n"
            "building", "Has building"
            "slain", "Monsters slain ≥ n"
            "birthday", "Today is a birthday"
        ]

    /// How a `variable` condition compares, with the editor's labels.
    let comparisons =
        [
            VariableComparisons.Eq, "="
            VariableComparisons.Ne, "≠"
            VariableComparisons.Lt, "<"
            VariableComparisons.Le, "≤"
            VariableComparisons.Gt, ">"
            VariableComparisons.Ge, "≥"
        ]

    /// The relationship stages, with the editor's labels.
    let relationshipStatuses = References.relationshipStatuses

    /// The event triggers, with the editor's labels (M3).
    let triggers = References.triggers

    /// The outcome types the editor offers, in its order.
    let outcomeTypes =
        [
            EventOutcomeTypes.ModifyFriendship, "Change friendship"
            EventOutcomeTypes.ModifyEnergy, "Restore / spend energy"
            EventOutcomeTypes.WaterArea, "Water nearby soil (magic)"
            EventOutcomeTypes.Message, "Show message"
            EventOutcomeTypes.GiveItem, "Give item"
            EventOutcomeTypes.TakeItem, "Take item"
            EventOutcomeTypes.GiveMoney, "Give money"
            EventOutcomeTypes.TakeMoney, "Take money"
            EventOutcomeTypes.SetFlag, "Set flag"
            EventOutcomeTypes.ClearFlag, "Clear flag"
            EventOutcomeTypes.StartQuest, "Start quest"
            EventOutcomeTypes.CompleteQuest, "Complete quest"
            EventOutcomeTypes.SpawnNpc, "Spawn NPC"
            EventOutcomeTypes.RemoveNpc, "Remove NPC"
            EventOutcomeTypes.ChangeTile, "Change tile"
            EventOutcomeTypes.WarpPlayer, "Warp player"
            EventOutcomeTypes.StartDialogue, "Start dialogue"
            EventOutcomeTypes.LockTransition, "Lock transition"
            EventOutcomeTypes.UnlockTransition, "Unlock transition"
            EventOutcomeTypes.PlaySound, "Play sound"
            EventOutcomeTypes.PerformAction, "Perform action"
            EventOutcomeTypes.StartMinigame, "Start minigame"
            EventOutcomeTypes.PlayMusic, "Play music"
            EventOutcomeTypes.StartCutscene, "Start cutscene"
            EventOutcomeTypes.SetVariable, "Set variable"
            EventOutcomeTypes.AddVariable, "Add to variable"
            EventOutcomeTypes.MoveNpc, "Move NPC"
            EventOutcomeTypes.SetWeather, "Change the weather"
            EventOutcomeTypes.GiveExperience, "Give experience"
            EventOutcomeTypes.AddShopStock, "Add to a shop"
            EventOutcomeTypes.RemoveShopStock, "Remove from a shop"
            EventOutcomeTypes.PlaceNode, "Place node"
            EventOutcomeTypes.RemoveNode, "Remove node"
            EventOutcomeTypes.PlaceMachine, "Place machine"
            EventOutcomeTypes.RemoveMachine, "Remove machine"
            EventOutcomeTypes.SetRelationship, "Set relationship"
            EventOutcomeTypes.SendMail, "Send a letter"
            EventOutcomeTypes.SetInventorySize, "Set backpack size"
            EventOutcomeTypes.AddInventorySize, "Increase backpack size"
            EventOutcomeTypes.AddMaxEnergy, "Increase maximum energy"
            EventOutcomeTypes.AddMaxHealth, "Increase maximum health"
            EventOutcomeTypes.ApplyBuff, "Apply timed buff"
            EventOutcomeTypes.ModifyHealth, "Restore / take health"
        ]

    /// The label of a condition type (the type itself when unknown).
    let conditionLabel (kind: string) =
        conditionTypes
        |> List.tryFind (fun (value, _) -> value = kind)
        |> Option.map snd
        |> Option.defaultValue kind

    /// The label of an outcome type (the type itself when unknown, like the web).
    let outcomeLabel (kind: string) =
        outcomeTypes
        |> List.tryFind (fun (value, _) -> value = kind)
        |> Option.map snd
        |> Option.defaultValue kind

    /// `ConditionRow`: the fields each condition type shows.
    let conditionFields (kind: string) : VocabularyField list =
        match kind with
        | "enterTile"
        | "interactTile" ->
            [
                integer "x" "X"
                integer "y" "Y"
                optionalInteger "x2" "X2 (opt)"
                optionalInteger "y2" "Y2 (opt)"
            ]
        | "friendship" ->
            [
                reference "npcId" "Character" ReferenceKind.Npc
                integer "min" "Minimum friendship"
            ]
        | "weather" ->
            [
                field "weatherIds" "Weather IDs" (FieldKind.ReferenceList ReferenceKind.Weather)
            ]
        | "hasItem"
        | "inventorySpace" ->
            [
                reference "itemId" "Item" ReferenceKind.Item
                { integer "quantity" "Quantity ≥" with
                    WhenEmpty = 1.0
                }
            ]
        | "flag" ->
            [
                text "flag" "Flag name" ""
                field "value" "Flag state" (FieldKind.Bool("is set", "is NOT set"))
            ]
        | "dayRange" -> [ optionalInteger "minDay" "From day"; optionalInteger "maxDay" "To day" ]
        | "yearRange" -> [ optionalInteger "minYear" "From year"; optionalInteger "maxYear" "To year" ]
        | "timeOfDay" -> [ integer "minMinute" "From minute"; integer "maxMinute" "To minute" ]
        | "season" -> [ field "seasons" "Seasons" (FieldKind.ReferenceList ReferenceKind.Season) ]
        | "weekday" ->
            [
                field "weekdayIds" "Weekdays" (FieldKind.ReferenceList ReferenceKind.Weekday)
            ]
        | "festivalId" -> [ reference "festivalId" "Festival" ReferenceKind.Festival ]
        | "questStatus" ->
            [
                reference "questId" "Quest" ReferenceKind.Quest
                field "status" "Status" (FieldKind.OneOf(QuestStatuses.All |> Seq.map (fun s -> s, s) |> List.ofSeq))
            ]
        | "variable" ->
            [
                text "name" "Variable" "coins"
                field "compare" "Compare" (FieldKind.OneOf comparisons)
                { field "value" "Number" FieldKind.Number with
                    Optional = true
                }
                { text "text" "or text" "" with
                    Optional = true
                    Placeholder = "compare with a text instead"
                }
            ]
        | "all"
        | "any"
        | "not" -> [ field "conditions" "Conditions" FieldKind.Conditions ]
        | "chance" ->
            [
                { field "chance" "Chance (0–1)" FieldKind.Number with
                    Min = Some 0.0
                    Max = Some 1.0
                }
            ]
        | "scene" -> [ field "sceneIds" "Scenes" (FieldKind.ReferenceList ReferenceKind.Scene) ]
        | "hearts" ->
            [
                { reference "npcId" "Character" ReferenceKind.Npc with
                    Optional = true
                    Empty = Some "(the NPC speaking)"
                }
                optionalInteger "min" "At least (hearts)"
                optionalInteger "max" "At most (hearts)"
            ]
        | "dayOfSeason" -> [ optionalInteger "minDay" "From day"; optionalInteger "maxDay" "To day" ]
        | "relationship" ->
            [
                { reference "npcId" "Character" ReferenceKind.Npc with
                    Optional = true
                    Empty = Some "(the NPC speaking, or anyone)"
                }
                field "status" "Status" (FieldKind.OneOf relationshipStatuses)
            ]
        // M5–M7
        | "money" -> [ integer "min" "At least (money)" ]
        | "perk" ->
            [
                reference "skill" "Skill" ReferenceKind.Skill
                text "perkId" "Perk" ""
                text "choiceId" "Choice (empty for any)" ""
            ]
        | "buff" -> [ text "id" "Buff id" "" ]
        | "skillLevel" ->
            [
                reference "skill" "Skill" ReferenceKind.Skill
                { integer "min" "Level ≥" with
                    WhenEmpty = 1.0
                }
            ]
        | "mineFloor" ->
            [
                reference "mineId" "Mine" ReferenceKind.Mine
                { integer "min" "Floor ≥" with
                    WhenEmpty = 1.0
                }
            ]
        | "collection" ->
            [
                reference "collectionId" "Collection" ReferenceKind.Collection
                { optionalInteger "min" "At least (items)" with
                    Placeholder = "all of them"
                }
            ]
        | "bundle" -> [ reference "bundleId" "Bundle" ReferenceKind.Bundle ]
        | "achievement" -> [ reference "achievementId" "Achievement" ReferenceKind.Achievement ]
        | "shipped" ->
            [
                { reference "itemId" "Item" ReferenceKind.Item with
                    Optional = true
                    Empty = Some "(everything)"
                }
                { integer "min" "At least (units)" with
                    WhenEmpty = 1.0
                }
            ]
        | "building" ->
            [
                reference "buildingId" "Building" ReferenceKind.Building
                { integer "min" "At least" with
                    WhenEmpty = 1.0
                }
            ]
        | "slain" ->
            [
                { reference "monsterId" "Monster" ReferenceKind.Monster with
                    Optional = true
                    Empty = Some "(any monster)"
                }
                { integer "min" "At least" with
                    WhenEmpty = 1.0
                }
            ]
        | "birthday" ->
            [
                { reference "npcId" "Character" ReferenceKind.Npc with
                    Optional = true
                    Empty = Some "(anyone's)"
                }
            ]
        | _ -> []

    /// `OutcomeFields`: the fields each outcome type shows.
    let outcomeFields (kind: string) : VocabularyField list =
        let item = reference "itemId" "Item" ReferenceKind.Item
        let npc = reference "npcId" "NPC" ReferenceKind.Npc
        let scene = reference "sceneId" "Scene" ReferenceKind.Scene

        let hereScene =
            { scene with
                Optional = true
                Empty = Some "(the player's scene)"
            }

        match kind with
        | EventOutcomeTypes.Message -> [ text "message" "Message" "Message text…" ]
        | EventOutcomeTypes.GiveItem
        | EventOutcomeTypes.TakeItem ->
            [
                item
                { integer "itemQuantity" "Quantity" with
                    WhenEmpty = 1.0
                }
            ]
        | EventOutcomeTypes.GiveMoney
        | EventOutcomeTypes.TakeMoney -> [ integer "amount" "Amount" ]
        | EventOutcomeTypes.ModifyEnergy -> [ integer "amount" "Energy change" ]
        | EventOutcomeTypes.WaterArea ->
            [
                { integer "radius" "Radius (0-10 tiles)" with
                    WhenEmpty = 1.0
                    Min = Some 0.0
                    Max = Some 10.0
                }
            ]
        | EventOutcomeTypes.ModifyFriendship -> [ npc; integer "amount" "Friendship change" ]
        | EventOutcomeTypes.SetFlag
        | EventOutcomeTypes.ClearFlag -> [ text "flagName" "Flag name" "flag-name" ]
        | EventOutcomeTypes.StartQuest
        | EventOutcomeTypes.CompleteQuest -> [ reference "questId" "Quest" ReferenceKind.Quest ]
        | EventOutcomeTypes.SpawnNpc -> [ npc; optionalInteger "x" "X (opt)"; optionalInteger "y" "Y (opt)" ]
        | EventOutcomeTypes.RemoveNpc
        | EventOutcomeTypes.StartDialogue -> [ npc ]
        | EventOutcomeTypes.ChangeTile ->
            [
                integer "tileX" "X"
                integer "tileY" "Y"
                reference "newTileType" "New terrain" ReferenceKind.Terrain
            ]
        | EventOutcomeTypes.WarpPlayer -> [ scene; integer "x" "X"; integer "y" "Y" ]
        | EventOutcomeTypes.LockTransition
        | EventOutcomeTypes.UnlockTransition -> [ scene; integer "x" "From X"; integer "y" "From Y" ]
        | EventOutcomeTypes.PlaySound -> [ text "soundId" "Sound" "a sound asset id, or coin, quest, success…" ]
        | EventOutcomeTypes.PerformAction -> [ reference "actionId" "Action" ReferenceKind.Action ]
        | EventOutcomeTypes.StartMinigame -> [ reference "minigameId" "Minigame" ReferenceKind.Minigame ]
        | EventOutcomeTypes.PlayMusic ->
            [
                { reference "soundId" "Music" ReferenceKind.Music with
                    Optional = true
                    Empty = Some "(the game's own music)"
                }
            ]
        | EventOutcomeTypes.StartCutscene -> [ reference "cutsceneId" "Cutscene" ReferenceKind.Cutscene ]
        | EventOutcomeTypes.SetVariable ->
            [
                text "variable" "Variable" "coins"
                { field "amount" "Number" FieldKind.Number with
                    Optional = true
                }
                { text "text" "or text" "" with
                    Optional = true
                    Placeholder = "store a text instead"
                }
            ]
        | EventOutcomeTypes.AddVariable -> [ text "variable" "Variable" "coins"; field "amount" "Add" FieldKind.Number ]
        | EventOutcomeTypes.MoveNpc -> [ npc; hereScene; integer "x" "X"; integer "y" "Y" ]
        | EventOutcomeTypes.SetWeather -> [ reference "weatherId" "Weather" ReferenceKind.Weather ]
        | EventOutcomeTypes.GiveExperience -> [ reference "skill" "Skill" ReferenceKind.Skill; integer "amount" "Experience" ]
        | EventOutcomeTypes.AddShopStock ->
            [
                reference "shopId" "Shop" ReferenceKind.Shop
                item
                { optionalInteger "amount" "Price (opt)" with
                    Placeholder = "the item's"
                }
                { optionalInteger "itemQuantity" "Daily limit (opt)" with
                    Placeholder = "unlimited"
                }
            ]
        | EventOutcomeTypes.RemoveShopStock -> [ reference "shopId" "Shop" ReferenceKind.Shop; item ]
        | EventOutcomeTypes.PlaceNode ->
            [
                hereScene
                integer "tileX" "X"
                integer "tileY" "Y"
                reference "nodeTypeId" "Node" ReferenceKind.NodeType
            ]
        | EventOutcomeTypes.PlaceMachine ->
            [
                hereScene
                integer "tileX" "X"
                integer "tileY" "Y"
                reference "machineTypeId" "Machine" ReferenceKind.MachineType
            ]
        | EventOutcomeTypes.RemoveNode
        | EventOutcomeTypes.RemoveMachine -> [ hereScene; integer "tileX" "X"; integer "tileY" "Y" ]
        | EventOutcomeTypes.SetRelationship -> [ npc; field "status" "Status" (FieldKind.OneOf relationshipStatuses) ]
        | EventOutcomeTypes.SendMail -> [ reference "mailId" "Letter" ReferenceKind.Mail ]
        | EventOutcomeTypes.SetInventorySize -> [ integer "amount" "Amount" ]
        | EventOutcomeTypes.AddInventorySize -> [ integer "amount" "Amount" ]
        | EventOutcomeTypes.AddMaxEnergy -> [ integer "amount" "Amount" ]
        | EventOutcomeTypes.AddMaxHealth -> [ integer "amount" "Amount" ]
        | EventOutcomeTypes.ModifyHealth -> [ integer "amount" "Health change" ]
        | _ -> []

    /// `defaultCondition`: a condition of the type with the web's starting values.
    let defaultCondition (kind: string) (project: GameProject) : EventCondition = Defaults.defaultCondition kind project

    /// The web adds `{ type }`: an outcome of the type with nothing filled in yet.
    let defaultOutcome (kind: string) : EventOutcome = Defaults.defaultOutcome kind

    /// QuestEditor: the target fields each objective type shows. Other target fields stay hidden.
    /// A craft objective names the item made: the engine counts every output of a hand craft and
    /// of goods collected from a machine, by quantity (a recipe id still counts one per hand
    /// craft, for older content).
    let objectiveTargets (kind: string) : string list =
        match kind with
        | QuestObjectiveTypes.Collect
        | QuestObjectiveTypes.Ship
        | QuestObjectiveTypes.Catch
        | QuestObjectiveTypes.Craft
        | QuestObjectiveTypes.Gift -> [ "TargetItemId"; "TargetItemQuantity" ]
        | QuestObjectiveTypes.Harvest -> [ "TargetCropType"; "TargetCropQuantity" ]
        | QuestObjectiveTypes.Hearts -> [ "TargetNpcId"; "TargetItemQuantity" ]
        | QuestObjectiveTypes.Slay -> [ "TargetMonsterId"; "TargetItemQuantity" ]
        | QuestObjectiveTypes.Bundle -> [ "TargetBundleId" ]
        | QuestObjectiveTypes.MineFloor -> [ "TargetMineId"; "TargetItemQuantity" ]
        | QuestObjectiveTypes.Talk -> [ "TargetNpcId" ]
        | QuestObjectiveTypes.Visit -> [ "TargetSceneId" ]
        | _ -> []

    let private objectiveTargetProperties =
        [
            "TargetItemId"
            "TargetItemQuantity"
            "TargetCropType"
            "TargetCropQuantity"
            "TargetNpcId"
            "TargetSceneId"
        ]

    /// The fields a cutscene step of each type uses besides its type and label (M2).
    let stepFields (kind: string) : string list =
        match kind with
        | CutsceneStepTypes.Say -> [ "Text"; "Speaker"; "Expression" ]
        | CutsceneStepTypes.Choice -> [ "Text"; "Options" ]
        | CutsceneStepTypes.Jump -> [ "Goto"; "Conditions" ]
        | CutsceneStepTypes.Move -> [ "Actor"; "X"; "Y"; "Wait" ]
        | CutsceneStepTypes.Face -> [ "Actor"; "Direction" ]
        | CutsceneStepTypes.Emote -> [ "Actor"; "Emote"; "Seconds" ]
        | CutsceneStepTypes.Wait -> [ "Seconds" ]
        | CutsceneStepTypes.Camera -> [ "Actor"; "X"; "Y"; "Seconds" ]
        | CutsceneStepTypes.Fade -> [ "Mode"; "Color"; "Seconds" ]
        | CutsceneStepTypes.Music
        | CutsceneStepTypes.Sound -> [ "SoundId" ]
        | CutsceneStepTypes.Outcomes -> [ "Outcomes" ]
        | _ -> []

    let private stepProperties =
        [
            "Actor"
            "X"
            "Y"
            "Direction"
            "Emote"
            "Seconds"
            "Text"
            "Speaker"
            "Options"
            "Goto"
            "Conditions"
            "Mode"
            "Color"
            "SoundId"
            "Outcomes"
            "Wait"
            "Expression"
        ]

    /// Properties of `owner` a form hides while its `type` discriminator is `kind`.
    let hiddenProperties (owner: string) (kind: string) : string list =
        match owner with
        | "QuestObjective" ->
            let shown = objectiveTargets kind
            objectiveTargetProperties |> List.filter (fun p -> not (List.contains p shown))
        | "CutsceneStep" ->
            let shown = stepFields kind
            stepProperties |> List.filter (fun p -> not (List.contains p shown))
        // An event's subject and minute (M3) apply to some triggers only.
        | "GameEvent" ->
            [
                if not (List.contains kind EventTriggers.WithSubject) then
                    "Subject"
                if kind <> EventTriggers.Time then
                    "Minute"
            ]
        | _ -> []

    /// What `owner.property` holds while the record's `type` is `kind`, where that differs from
    /// its declaration: a cutscene step's `SoundId` is a music asset on a music step (empty: back
    /// to the game's music) and a sound asset on a sound step; a camera step without an actor
    /// looks at its tile, or back at the player.
    let roleFor (owner: string) (property: string) (kind: string) : FieldRole option =
        match owner, property, kind with
        | "CutsceneStep", "Actor", CutsceneStepTypes.Camera -> Some(FieldRole.Reference(ReferenceKind.Actor, Some "(a tile, or back to the player)"))
        | "CutsceneStep", "SoundId", CutsceneStepTypes.Music -> Some(FieldRole.Reference(ReferenceKind.Music, Some "(the game's own music)"))
        | "CutsceneStep", "SoundId", CutsceneStepTypes.Sound -> Some(FieldRole.Reference(ReferenceKind.Sound, None))
        // What an event's subject names depends on its trigger (M3).
        | "GameEvent", "Subject", (EventTriggers.Harvest | EventTriggers.Sell | EventTriggers.Craft | EventTriggers.Catch | EventTriggers.PickUp) ->
            Some(FieldRole.Reference(ReferenceKind.Item, Some "(any item)"))
        | "GameEvent", "Subject", EventTriggers.Friendship -> Some(FieldRole.Reference(ReferenceKind.Npc, Some "(anyone)"))
        | "GameEvent", "Subject", EventTriggers.Quest -> Some(FieldRole.Reference(ReferenceKind.Quest, Some "(any quest)"))
        | "GameEvent", "Subject", EventTriggers.Build -> Some(FieldRole.Reference(ReferenceKind.Building, Some "(any building)"))
        | "GameEvent", "Subject", EventTriggers.Bundle -> Some(FieldRole.Reference(ReferenceKind.Bundle, Some "(any bundle)"))
        | "GameEvent", "Subject", EventTriggers.Achievement -> Some(FieldRole.Reference(ReferenceKind.Achievement, Some "(any achievement)"))
        | "GameEvent", "Subject", EventTriggers.Slay -> Some(FieldRole.Reference(ReferenceKind.Monster, Some "(any monster)"))
        | _ -> None

    let private some (value: 'T) : obj option = Some(box value |> nonNull)

    let private firstId (options: PickerOption list) =
        match options with
        | first :: _ -> first.Id
        | [] -> ""

    /// A new element for a list in a content form (the web "Add" buttons). `entity` is the entry
    /// being edited and `siblingIds` the ids already in the list (objectives need unique ids).
    /// `None` when the type has no special default; the form then starts from the record's own
    /// defaults.
    let newElement (elementType: string) (project: GameProject) (entity: obj) (siblingIds: string list) : obj option =
        let firstItem () =
            firstId (References.options ReferenceKind.Item project)

        match elementType with
        | "ShopStockEntry" ->
            some
                { ShopStockEntry.Default with
                    ItemId = firstItem ()
                }
        | "RecipeIngredient" ->
            some (
                {
                    ItemId = firstItem ()
                    Quantity = 1.0
                }
                : RecipeIngredient
            )
        | "NodeDrop" ->
            some (
                { NodeDrop.Default with
                    ItemId = firstItem ()
                    Min = 1.0
                    Max = 1.0
                    Weight = 1.0
                    Extra = []
                }
                : NodeDrop
            )
        | "FishTableEntry" ->
            some (
                { FishTableEntry.Default with
                    ItemId = firstItem ()
                    Weight = 1.0
                    Difficulty = 0.3
                }
                : FishTableEntry
            )
        | "QuestRewardItem" ->
            some (
                {
                    ItemId = firstItem ()
                    Quantity = 1.0
                }
                : QuestRewardItem
            )
        | "MineRockWeight" ->
            some (
                {
                    NodeTypeId = firstId (References.options ReferenceKind.NodeType project)
                    Weight = 1.0
                }
                : MineRockWeight
            )
        | "WeatherTableEntry" ->
            some (
                {
                    WeatherId = firstId (References.options ReferenceKind.Weather project)
                    Weight = 1.0
                }
                : WeatherTableEntry
            )
        | "QuestObjective" ->
            some
                { QuestObjective.Default with
                    Id = Defaults.nextId "obj" siblingIds
                    Type = QuestObjectiveTypes.Collect
                    Description = "New objective"
                    Completed = false
                    Progress = 0.0
                }
        | "DialogueOption" -> some (Defaults.newDialogueOption ())
        | "MinigameResultTier" -> some (Defaults.newResultTier ())
        | "NpcScheduleEntry" ->
            match entity with
            | :? Npc as npc -> some (Defaults.newScheduleEntry npc)
            | _ -> None
        | "GridPoint" ->
            match entity with
            | :? Npc as npc -> some ({ X = npc.X; Y = npc.Y }: GridPoint)
            | _ -> None
        | "Dialogue" ->
            match entity with
            | :? Npc as npc ->
                let taken =
                    Seq.append (Defaults.allIds project) (npc.Dialogue |> Seq.map (fun d -> d.Id))
                    |> Seq.append siblingIds

                let dialogue = Defaults.newDialogue project npc.Id

                some
                    { dialogue with
                        Id = Defaults.nextId "dialogue" taken
                    }
            | _ -> None
        | "EventCondition" -> some (defaultCondition "enterTile" project)
        | "EventOutcome" -> some (defaultOutcome EventOutcomeTypes.Message)
        | "CutsceneStep" ->
            some
                { CutsceneStep.Default with
                    Type = CutsceneStepTypes.Say
                    Text = Some "..."
                }
        | "CutsceneChoice" ->
            some
                { CutsceneChoice.Default with
                    Text = "OK"
                }
        | "NpcLine" ->
            some
                { NpcLine.Default with
                    Text = "Nice day, isn't it?"
                }
        | "NpcPortrait" ->
            some
                { NpcPortrait.Default with
                    Expression = "happy"
                }
        | "TerrainTransform" ->
            some (
                {
                    Action = ToolActions.Till
                    Tool = None
                    Into = TileTypes.Soil
                }
                : TerrainTransform
            )
        | "NeighborRequirement" ->
            some
                { NeighborRequirement.Default with
                    Kind = NeighborKinds.Water
                }
        // M5–M7 lists: the first of what they name.
        | "AreaEffect" -> some AreaEffect.Default
        | "AnimalProduct" ->
            some
                { AnimalProduct.Default with
                    ItemId = firstItem ()
                }
        | "MoodQuality" ->
            let qualities = References.options ReferenceKind.CropQuality project

            let quality =
                if qualities |> List.exists (fun q -> q.Id = CropQualities.Silver) then
                    CropQualities.Silver
                else
                    firstId qualities

            some
                { MoodQuality.Default with
                    MinMood = 70.0
                    Quality = quality
                }
        | "FarmLayout" ->
            some
                { FarmLayout.Default with
                    Id = Defaults.nextId "layout" siblingIds
                    Name = "New layout"
                    SceneId = project.StartSceneId
                }
        | "CollectionReward" ->
            some
                { CollectionReward.Default with
                    Count = 1.0
                }
        | "BundleItem" ->
            some
                { BundleItem.Default with
                    ItemId = firstItem ()
                }
        | "MonsterSpawn" ->
            some
                { MonsterSpawn.Default with
                    MonsterId = firstId (References.options ReferenceKind.Monster project)
                }
        | "MineMonsterWeight" ->
            some
                { MineMonsterWeight.Default with
                    MonsterId = firstId (References.options ReferenceKind.Monster project)
                    Weight = 1.0
                }
        | "MineHazard" ->
            some
                { MineHazard.Default with
                    TerrainId = firstId (References.options ReferenceKind.Terrain project)
                }
        | "MineTreasure" ->
            some
                { MineTreasure.Default with
                    ItemId = firstItem ()
                    Weight = 1.0
                }
        | _ -> None
