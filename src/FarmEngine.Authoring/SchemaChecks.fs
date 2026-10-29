namespace FarmEngine.Authoring

open System
open FarmEngine.Schemas

/// One failed schema check: its dotted path (`scenes.0.tiles.1.2.background`) and message.
type SchemaIssue =
    {
        Path: string
        Message: string
    }

    /// `path: message`, the string form loading reports.
    override this.ToString() = this.Path + ": " + this.Message

/// The schema checks of a project.
///
/// Decoding (SchemaJson.fs) checks value kinds; it does not check value constraints (int,
/// positive, min/max, enum membership, …). `projectIssues` checks those: a project that fails
/// them does not load. `projectLints` adds structural checks (empty ids, duplicate scene ids, tile grid vs.
/// width/height, a dangling start scene, inverted regions and ranges): problems for the Problems
/// panel, never reasons to refuse a file. Not exhaustive.
///
/// Saves (`ValidateGameState`) are Rust-owned and not here.
[<CompilationRepresentation(CompilationRepresentationFlags.ModuleSuffix)>]
module SchemaChecks =
    /// Parse-level (refuses the file) or lint-level (a problem the editor shows).
    [<RequireQualifiedAccess>]
    type private Level =
        | Parse
        | Lint

    type private Finding = Level * SchemaIssue

    let private parse (path: string) (message: string) : Finding list =
        [ Level.Parse, { Path = path; Message = message } ]

    let private lint (path: string) (message: string) : Finding list =
        [ Level.Lint, { Path = path; Message = message } ]

    /// The checks for every element of a list, in order.
    let private each (list: 'T list) (check: int -> 'T -> Finding list) : Finding list = list |> List.mapi check |> List.concat

    let private whenSome (value: float option) (check: float -> Finding list) : Finding list =
        match value with
        | Some v -> check v
        | None -> []

    /// `Number.isInteger`.
    let isInteger (value: float) =
        not (Double.IsNaN value || Double.IsInfinity value) && Math.Floor value = value

    let private num (value: float) = JsNumber.format value

    // ── zod refinements ──────────────────────────────────────────────────────

    let private integer path (value: float) =
        if isInteger value then
            []
        else
            parse path $"Expected integer, received {num value}"

    let private positiveInt path (value: float) =
        if not (isInteger value) then
            parse path $"Expected integer, received {num value}"
        elif value <= 0.0 then
            parse path "Number must be greater than 0"
        else
            []

    // `not (value > 0)` rather than `value <= 0`: NaN fails, like zod.
    let private positive path (value: float) =
        if not (value > 0.0) then
            parse path "Number must be greater than 0"
        else
            []

    let private nonNegative path (value: float) =
        if not (value >= 0.0) then
            parse path "Number must be greater than or equal to 0"
        else
            []

    let private range path (value: float) (min: float) (max: float) =
        if not (value >= min && value <= max) then
            parse path $"Number must be between {num min} and {num max}"
        else
            []

    /// zod `z.string()` accepts "": an empty id is a lint problem, not a parse error.
    let private nonEmpty path (value: string) =
        if String.IsNullOrEmpty value then
            lint path "Required id must be a non-empty string"
        else
            []

    let private enumValue path (value: string) (allowed: string list) =
        if List.contains value allowed then
            []
        else
            let expected = allowed |> List.map (fun a -> $"'{a}'") |> String.concat " | "
            parse path $"Invalid enum value. Expected {expected}, received '{value}'"

    /// A required `z.enum`.
    let private oneOf path (value: string) (allowed: string list) = enumValue path value allowed

    /// An `.optional()` `z.enum`.
    let private optionalOneOf path (value: string option) (allowed: string list) =
        match value with
        | None -> []
        | Some value -> enumValue path value allowed

    // ── Events, actions and minigames ────────────────────────────────────────

    let private region path (x: float) (y: float) (x2: float option) (y2: float option) =
        [
            match x2 with
            | Some x2 when x2 < x -> yield! lint (path + ".x2") "x2 must be >= x"
            | _ -> ()
            match y2 with
            | Some y2 when y2 < y -> yield! lint (path + ".y2") "y2 must be >= y"
            | _ -> ()
        ]

    let rec private conditions (basePath: string) (list: EventCondition list) =
        each list (fun c condition ->
            let cp = $"{basePath}.conditions.{c}"

            match condition with
            | EventCondition.EnterTile t -> region cp t.X t.Y t.X2 t.Y2
            | EventCondition.InteractTile t -> region cp t.X t.Y t.X2 t.Y2
            | EventCondition.HasItem h -> nonEmpty (cp + ".itemId") h.ItemId
            | EventCondition.InventorySpace s -> nonEmpty (cp + ".itemId") s.ItemId @ positiveInt (cp + ".quantity") s.Quantity
            | EventCondition.Flag f ->
                if String.IsNullOrEmpty f.Flag then
                    lint (cp + ".flag") "Flag name must be a non-empty string"
                else
                    []
            | EventCondition.DayRange d ->
                match d.MinDay, d.MaxDay with
                | Some minDay, Some maxDay when minDay > maxDay -> lint cp "minDay is greater than maxDay"
                | _ -> []
            | EventCondition.YearRange y ->
                match y.MinYear, y.MaxYear with
                | Some minYear, Some maxYear when minYear > maxYear -> lint cp "minYear is greater than maxYear"
                | _ -> []
            // A start after the end wraps past midnight (22:00–2:00), as the engine reads it.
            | EventCondition.TimeOfDay _ -> []
            | EventCondition.QuestStatus q ->
                nonEmpty (cp + ".questId") q.QuestId
                @ oneOf (cp + ".status") q.Status QuestStatuses.All
            | EventCondition.Friendship f -> nonEmpty (cp + ".npcId") f.NpcId
            | EventCondition.Weekday w -> each w.WeekdayIds (fun i id -> nonEmpty ($"{cp}.weekdayIds.{i}") id)
            | EventCondition.FestivalId f -> nonEmpty (cp + ".festivalId") f.FestivalId
            | EventCondition.Variable v ->
                nonEmpty (cp + ".name") v.Name
                @ oneOf (cp + ".compare") v.Compare VariableComparisons.All
            | EventCondition.All g -> conditions cp g.Conditions
            | EventCondition.Any g -> conditions cp g.Conditions
            | EventCondition.Not g -> conditions cp g.Conditions
            | EventCondition.Chance c -> range (cp + ".chance") c.Chance 0.0 1.0
            | EventCondition.Hearts h ->
                match h.Min, h.Max with
                | Some min, Some max when min > max -> lint cp "min is greater than max"
                | _ -> []
            | EventCondition.DayOfSeason d ->
                match d.MinDay, d.MaxDay with
                | Some minDay, Some maxDay when minDay > maxDay -> lint cp "minDay is greater than maxDay"
                | _ -> []
            | EventCondition.Relationship r -> oneOf (cp + ".status") r.Status RelationshipStatuses.All
            // M5–M7: ids may be empty (anything); counts saturate as the engine reads them.
            | EventCondition.Money _
            | EventCondition.Perk _
            | EventCondition.Buff _
            | EventCondition.SkillLevel _
            | EventCondition.MineFloor _
            | EventCondition.Collection _
            | EventCondition.Bundle _
            | EventCondition.Achievement _
            | EventCondition.Shipped _
            | EventCondition.Building _
            | EventCondition.Slain _
            | EventCondition.Birthday _
            | EventCondition.Scene _
            | EventCondition.Season _
            | EventCondition.Weather _ -> [])

    let private progressionStats =
        [
            "maxHealth"
            "craftingSpeed"
            "sellPrice"
            "moveSpeed"
            "toolEnergyCost"
            "luck"
            "skillLevel"
            "maxEnergy"
            "attack"
            "defense"
            "fishingEscape"
            "quality"
            "yield"
        ]

    let private buffDefinition path (b: BuffDefinition) =
        nonEmpty (path + ".id") b.Id
        @ oneOf (path + ".stat") b.Stat progressionStats
        @ integer (path + ".amount") b.Amount
        @ range (path + ".amount") b.Amount -1000.0 1000.0
        @ positiveInt (path + ".durationMinutes") b.DurationMinutes
        @ range (path + ".durationMinutes") b.DurationMinutes 1.0 525600.0

    let private outcomes (basePath: string) (list: EventOutcome list) =
        each list (fun o outcome ->
            let op = $"{basePath}.outcomes.{o}"

            [
                yield! oneOf (op + ".type") outcome.Type EventOutcomeTypes.All
                match outcome.Buff with
                | Some buff -> yield! buffDefinition (op + ".buff") buff
                | None -> ()
                match outcome.Amount with
                | Some amount when Double.IsNaN amount || Double.IsInfinity amount -> yield! parse (op + ".amount") "Number must be finite"
                | _ -> ()
                yield! whenSome outcome.Radius (fun radius -> integer (op + ".radius") radius @ range (op + ".radius") radius 0.0 10.0)
            ])

    // ── Project sections, in the C# order ───────────────────────────────────

    let private soilStates = System.Collections.Generic.HashSet<string>(SoilStates.All)

    /// A tile's checks; `path` builds its path only when one fails. Its terrains and its crop's
    /// quality and mutation are the game's own vocabulary since M1: an unknown one is a problem
    /// (`ChecksVocabulary`), not a reason to refuse the file.
    let private tile (path: unit -> string) (tile: Tile) =
        match tile.SoilState with
        | Some state when not (soilStates.Contains state) -> optionalOneOf (path () + ".soilState") tile.SoilState SoilStates.All
        | _ -> []

    let private scene (scenes: Scene list) (s: int) (scene: Scene) =
        let sp = $"scenes.{s}"

        let duplicate =
            not (String.IsNullOrEmpty scene.Id)
            && scenes |> List.take s |> List.exists (fun earlier -> earlier.Id = scene.Id)

        let validWidth = isInteger scene.Width && scene.Width > 0.0
        let validHeight = isInteger scene.Height && scene.Height > 0.0

        [
            yield! nonEmpty (sp + ".id") scene.Id
            if duplicate then
                yield! lint (sp + ".id") $"Duplicate scene id '{scene.Id}'"
            yield! positiveInt (sp + ".width") scene.Width
            yield! positiveInt (sp + ".height") scene.Height
            if validHeight && float scene.Tiles.Length <> scene.Height then
                yield! lint (sp + ".tiles") $"Expected {num scene.Height} rows (scene height), found {scene.Tiles.Length}"
            yield!
                each scene.Tiles (fun y row ->
                    let rp = $"{sp}.tiles.{y}"

                    [
                        if validWidth && float row.Length <> scene.Width then
                            yield! lint rp $"Expected {num scene.Width} tiles (scene width), found {row.Length}"
                        yield! each row (fun x t -> tile (fun () -> $"{rp}.{x}") t)
                    ])
            yield! each scene.Transitions (fun t transition -> nonEmpty $"{sp}.transitions.{t}.toSceneId" transition.ToSceneId)
        ]

    let private scenes (p: GameProject) =
        [
            yield! each p.Scenes (scene p.Scenes)
            let known =
                p.Scenes
                |> List.exists (fun s -> not (String.IsNullOrEmpty s.Id) && s.Id = p.StartSceneId)

            if not p.Scenes.IsEmpty && not known then
                yield! lint "startSceneId" $"No scene with id '{p.StartSceneId}'"
        ]

    let private player (p: GameProject) =
        [
            yield! oneOf "player.direction" p.Player.Direction Directions.All
            yield! nonEmpty "player.sceneId" p.Player.SceneId
            yield! each p.Player.Inventory (fun i slot -> nonEmpty $"player.inventory.{i}.item.id" slot.Item.Id)
        ]

    let private items (p: GameProject) =
        each p.Items (fun i item ->
            let ip = $"items.{i}"

            [
                yield! nonEmpty (ip + ".id") item.Id
                yield! whenSome item.ToolTier (positiveInt (ip + ".toolTier"))
                yield! whenSome item.Energy (nonNegative (ip + ".energy"))
            ])

    let private npcs (p: GameProject) =
        each p.Npcs (fun i npc ->
            let np = $"npcs.{i}"

            [
                yield! nonEmpty (np + ".id") npc.Id
                yield! optionalOneOf (np + ".movePattern") npc.MovePattern NpcMovePatterns.All
                yield! whenSome npc.WanderRadius (positiveInt (np + ".wanderRadius"))
                match npc.PatrolPoints with
                | None -> ()
                | Some points ->
                    yield!
                        each points (fun k point ->
                            integer $"{np}.patrolPoints.{k}.x" point.X
                            @ integer $"{np}.patrolPoints.{k}.y" point.Y)
                match npc.Birthday with
                | None -> ()
                | Some birthday ->
                    // The season is checked against the project's calendar in ChecksContent (a
                    // warning: calendars are customizable).
                    yield! integer (np + ".birthday.day") birthday.Day
                yield! each npc.Dialogue (fun d dialogue -> nonEmpty $"{np}.dialogue.{d}.id" dialogue.Id)
            ])

    let private quests (p: GameProject) =
        each p.Quests (fun q quest ->
            let qp = $"quests.{q}"

            [
                yield! nonEmpty (qp + ".id") quest.Id
                yield! oneOf (qp + ".status") quest.Status QuestStatuses.All
                yield! whenSome quest.DeadlineDays (fun days -> integer (qp + ".deadlineDays") days @ positive (qp + ".deadlineDays") days)
                yield! whenSome quest.DeadlineDay (fun day -> integer (qp + ".deadlineDay") day @ positive (qp + ".deadlineDay") day)
                yield! outcomes qp (defaultArg quest.OnFail [])
                yield!
                    each quest.Objectives (fun o objective ->
                        nonEmpty $"{qp}.objectives.{o}.id" objective.Id
                        @ oneOf $"{qp}.objectives.{o}.type" objective.Type QuestObjectiveTypes.All)
            ])

    let private events (p: GameProject) =
        each p.Events (fun e event ->
            let ep = $"events.{e}"

            [
                yield! nonEmpty (ep + ".id") event.Id
                yield! oneOf (ep + ".trigger") event.Trigger EventTriggers.All
                yield! conditions ep event.Conditions
                yield! outcomes ep event.Outcomes
            ])

    let private actionsAndMinigames (p: GameProject) =
        [
            yield!
                each p.Actions (fun a action ->
                    let ap = $"actions.{a}"

                    [
                        yield! nonEmpty (ap + ".id") action.Id
                        yield! nonNegative (ap + ".energyCost") action.EnergyCost
                        match action.Hotkey with
                        | Some hotkey when hotkey.Length > 1 -> yield! parse (ap + ".hotkey") "String must contain at most 1 character(s)"
                        | _ -> ()
                        yield! conditions ap action.Conditions
                        yield! outcomes ap action.Outcomes
                    ])
            yield!
                each p.Minigames (fun m minigame ->
                    [
                        yield! nonEmpty $"minigames.{m}.id" minigame.Id
                        yield!
                            each minigame.ResultTiers (fun t tier ->
                                let tp = $"minigames.{m}.resultTiers.{t}"
                                range (tp + ".minScore") tier.MinScore 0.0 1.0 @ outcomes tp tier.Outcomes)
                    ])
        ]

    let private color path (value: string) =
        let hex = value.Length = 7 || value.Length = 9

        if value.StartsWith "#" && hex && value |> Seq.skip 1 |> Seq.forall Uri.IsHexDigit then
            []
        else
            parse path $"Expected a colour like #rrggbb, received '{value}'"

    let private optionalColor path (value: string option) =
        match value with
        | None -> []
        | Some value -> color path value

    /// The world vocabulary (M1): terrains, tools, item categories, crop qualities and mutations.
    let private vocabulary (p: GameProject) =
        let each' (list: 'T list option) check = each (defaultArg list []) check

        [
            yield!
                each' p.Terrains (fun t terrain ->
                    let tp = $"terrains.{t}"

                    [
                        yield! nonEmpty (tp + ".id") terrain.Id
                        yield! oneOf (tp + ".layer") terrain.Layer TileLayers.All
                        yield! color (tp + ".color") terrain.Color
                        yield! nonNegative (tp + ".hazardEnergy") terrain.HazardEnergy
                        yield! each terrain.Transforms (fun k transform -> oneOf $"{tp}.transforms.{k}.action" transform.Action ToolActions.Transforming)
                    ])
            yield!
                each' p.Tools (fun t tool ->
                    let tp = $"tools.{t}"

                    [
                        yield! nonEmpty (tp + ".id") tool.Id
                        yield! oneOf (tp + ".action") tool.Action ToolActions.All
                        yield! nonNegative (tp + ".energyCost") tool.EnergyCost
                        match tool.Key with
                        | Some key when key.Length > 1 -> yield! parse (tp + ".key") "String must contain at most 1 character(s)"
                        | _ -> ()
                        yield!
                            each tool.Area (fun k area ->
                                positiveInt $"{tp}.area.{k}.width" area.Width
                                @ positiveInt $"{tp}.area.{k}.length" area.Length)
                    ])
            yield!
                each' p.ItemCategories (fun c category ->
                    nonEmpty $"itemCategories.{c}.id" category.Id
                    @ nonNegative $"itemCategories.{c}.energy" category.Energy)
            yield!
                each' p.CropQualities (fun q quality ->
                    let qp = $"cropQualities.{q}"

                    [
                        yield! nonEmpty (qp + ".id") quality.Id
                        yield! positive (qp + ".priceMultiplier") quality.PriceMultiplier
                        yield! positive (qp + ".yieldMultiplier") quality.YieldMultiplier
                        yield! nonNegative (qp + ".mutationMultiplier") quality.MutationMultiplier
                        yield! integer (qp + ".stars") quality.Stars
                        yield! nonNegative (qp + ".stars") quality.Stars
                    ])
            yield!
                each' p.CropMutations (fun m mutation ->
                    let mp = $"cropMutations.{m}"

                    nonEmpty (mp + ".id") mutation.Id
                    @ positive (mp + ".yieldMultiplier") mutation.YieldMultiplier
                    @ range (mp + ".share") mutation.Share 0.0 1.0)
        ]

    /// What crops need to grow (M1).
    let private requirements (path: string) (needs: GrowthRequirements option) =
        match needs with
        | None -> []
        | Some needs ->
            [
                yield! optionalOneOf (path + ".light") needs.Light LightNeeds.All
                yield! whenSome needs.MinTemperature (integer (path + ".minTemperature"))
                yield! whenSome needs.MaxTemperature (integer (path + ".maxTemperature"))
                match needs.MinTemperature, needs.MaxTemperature with
                | Some low, Some high when low > high -> yield! lint (path + ".minTemperature") "minTemperature is greater than maxTemperature"
                | _ -> ()
                yield!
                    each needs.Neighbors (fun k need ->
                        let np = $"{path}.neighbors.{k}"

                        oneOf (np + ".kind") need.Kind NeighborKinds.All
                        @ positiveInt (np + ".radius") need.Radius
                        @ positiveInt (np + ".min") need.Min)
            ]

    /// The look of the sky (M1).
    let private look (path: string) (look: AtmosphereLook) =
        [
            yield!
                each look.Daylight (fun k key ->
                    nonNegative $"{path}.daylight.{k}.minute" key.Minute
                    @ color $"{path}.daylight.{k}.color" key.Color)
            yield! optionalColor (path + ".foliageTint") look.FoliageTint
        ]

    /// Cutscenes (M2).
    let private cutscenes (p: GameProject) =
        each (defaultArg p.Cutscenes []) (fun c cutscene ->
            let cp = $"cutscenes.{c}"

            [
                yield! nonEmpty (cp + ".id") cutscene.Id
                yield!
                    each cutscene.Steps (fun k step ->
                        let sp = $"{cp}.steps.{k}"

                        [
                            yield! oneOf (sp + ".type") step.Type CutsceneStepTypes.All
                            yield! optionalOneOf (sp + ".direction") step.Direction Directions.All
                            yield! optionalOneOf (sp + ".emote") step.Emote Emotes.All
                            yield! optionalOneOf (sp + ".mode") step.Mode [ "out"; "in" ]
                            yield! optionalColor (sp + ".color") step.Color
                            yield! whenSome step.Seconds (nonNegative (sp + ".seconds"))
                            yield! whenSome step.X (integer (sp + ".x"))
                            yield! whenSome step.Y (integer (sp + ".y"))
                            yield! conditions sp step.Conditions
                            yield! outcomes sp step.Outcomes
                        ])
            ])

    let private definitions (p: GameProject) =
        let idLists =
            [
                "shops", p.Shops |> List.map (fun x -> x.Id)
                "recipes", p.Recipes |> List.map (fun x -> x.Id)
                "machineTypes", p.MachineTypes |> List.map (fun x -> x.Id)
                "animalSpecies", p.AnimalSpecies |> List.map (fun x -> x.Id)
                "fishTables", p.FishTables |> List.map (fun x -> x.Id)
                "animals", p.Animals |> List.map (fun x -> x.Id)
            ]

        [
            for name, ids in idLists do
                for index, id in List.indexed ids do
                    yield! nonEmpty $"{name}.{index}.id" id
            yield!
                each p.NodeTypes (fun n node ->
                    let np = $"nodeTypes.{n}"

                    [
                        yield! nonEmpty (np + ".id") node.Id
                        yield! positiveInt (np + ".health") node.Health
                        yield! positiveInt (np + ".requiredToolTier") node.RequiredToolTier
                        yield! whenSome (Option.flatten node.RespawnDays) (positiveInt (np + ".respawnDays"))
                    ])
            yield!
                each p.Recipes (fun r recipe ->
                    [
                        yield! nonNegative $"recipes.{r}.processingMinutes" recipe.ProcessingMinutes
                        yield! each recipe.Inputs (fun k input -> positiveInt $"recipes.{r}.inputs.{k}.quantity" input.Quantity)
                        yield! each recipe.Outputs (fun k output -> positiveInt $"recipes.{r}.outputs.{k}.quantity" output.Quantity)
                    ])
            yield!
                each p.Weather.Types (fun w weather ->
                    nonEmpty $"weather.types.{w}.id" weather.Id
                    @ range $"weather.types.{w}.cropDamageChance" weather.CropDamageChance 0.0 1.0)
            yield!
                each (defaultArg p.Weather.Overlays []) (fun o overlay ->
                    let op = $"weather.overlays.{o}"

                    [
                        yield! nonEmpty (op + ".id") overlay.Id
                        yield! oneOf (op + ".style") overlay.Style OverlayStyles.All
                        if overlay.Style <> OverlayStyles.None then
                            yield! color (op + ".color") overlay.Color
                        yield! nonNegative (op + ".density") overlay.Density
                        yield! nonNegative (op + ".speed") overlay.Speed
                        yield! optionalColor (op + ".tint") overlay.Tint
                    ])
            yield! each (defaultArg p.CustomCrops []) (fun c crop -> requirements $"customCrops.{c}.requirements" crop.Requirements)
            for season, entries in p.Weather.Table do
                yield! each entries (fun k entry -> positive $"weather.table.{season}.{k}.weight" entry.Weight)
        ]

    let private settings (p: GameProject) =
        let s = p.Settings

        [
            yield! positive "settings.maxEnergy" s.MaxEnergy
            yield! range "settings.collapseEnergyFraction" s.CollapseEnergyFraction 0.0 1.0
            yield! nonNegative "settings.collapseMoneyPenalty" s.CollapseMoneyPenalty
            yield! positive "settings.movement.playerSpeed" s.Movement.PlayerSpeed
            yield! integer "settings.time.dayStartMinute" s.Time.DayStartMinute
            yield! nonNegative "settings.time.dayStartMinute" s.Time.DayStartMinute
            yield! integer "settings.time.dayEndMinute" s.Time.DayEndMinute
            // An end at or before the start collapsed the player on every tick (#24).
            if s.Time.DayEndMinute - s.Time.DayStartMinute < SettingsSchema.MinDayWindowMinutes then
                yield!
                    parse
                        "settings.time.dayEndMinute"
                        $"The day must end at least {num SettingsSchema.MinDayWindowMinutes} minutes after it starts ({num s.Time.DayStartMinute})"
            // The clock counts micro-minutes in 32 bits: a later end never comes.
            if s.Time.DayEndMinute > SettingsSchema.MaxDayEndMinute then
                yield! parse "settings.time.dayEndMinute" $"Number must be less than or equal to {num SettingsSchema.MaxDayEndMinute}"
            yield! positive "settings.time.minutesPerRealSecond" s.Time.MinutesPerRealSecond
            if s.Time.MinutesPerRealSecond > SettingsSchema.MaxMinutesPerRealSecond then
                yield! parse "settings.time.minutesPerRealSecond" $"Number must be less than or equal to {num SettingsSchema.MaxMinutesPerRealSecond}"
            yield!
                each s.Calendar.Seasons (fun c season ->
                    nonEmpty $"settings.calendar.seasons.{c}.id" season.Id
                    @ positiveInt $"settings.calendar.seasons.{c}.days" season.Days)
            yield!
                each s.Calendar.Festivals (fun f festival ->
                    nonEmpty $"settings.calendar.festivals.{f}.id" festival.Id
                    @ positiveInt $"settings.calendar.festivals.{f}.day" festival.Day)
            yield!
                whenSome s.Calendar.WeekdayOffset (fun offset ->
                    integer "settings.calendar.weekdayOffset" offset
                    @ nonNegative "settings.calendar.weekdayOffset" offset)
            yield! each (s.Calendar.Weekdays |> Option.defaultValue []) (fun i day -> nonEmpty $"settings.calendar.weekdays.{i}.id" day.Id)
            yield! each s.Calendar.Seasons (fun c season -> whenSome season.Temperature (integer $"settings.calendar.seasons.{c}.temperature"))
            match s.Time.DayMinutes with
            | Some minutes when not (isInteger minutes) || minutes < 60.0 ->
                yield! parse "settings.time.dayMinutes" "Number must be a whole number of at least 60"
            | _ -> ()
            match s.Atmosphere with
            | None -> ()
            | Some atmosphere ->
                yield! look "settings.atmosphere.allSeasons" atmosphere.AllSeasons

                for season, l in atmosphere.Seasons do
                    yield! look $"settings.atmosphere.seasons.{season}" l
        ]

    let private mineConfig (prefix: string) (m: MineConfig) =
        [
            yield! positiveInt (prefix + ".floors") m.Floors
            yield! positiveInt (prefix + ".floorWidth") m.FloorWidth
            yield! positiveInt (prefix + ".floorHeight") m.FloorHeight
            yield! positiveInt (prefix + ".elevatorEvery") m.ElevatorEvery
            yield! range (prefix + ".ladderChance") m.LadderChance 0.0 1.0
            yield!
                each m.Bands (fun b band ->
                    [
                        yield! positiveInt $"{prefix}.bands.{b}.fromFloor" band.FromFloor
                        yield! positiveInt $"{prefix}.bands.{b}.toFloor" band.ToFloor
                        yield! range $"{prefix}.bands.{b}.density" band.Density 0.0 1.0
                    ])
        ]

    let private mine (p: GameProject) =
        [
            yield! mineConfig "mine" p.Mine
            yield! each (defaultArg p.Mines []) (fun i mine -> nonEmpty $"mines.{i}.id" mine.Id @ mineConfig $"mines.{i}.config" mine.Config)
            yield! whenSome p.MineDeepestFloor (fun deepest -> integer "mineDeepestFloor" deepest @ nonNegative "mineDeepestFloor" deepest)
        ]

    let private requestBoards (p: GameProject) =
        each (defaultArg p.RequestBoards []) (fun i board ->
            let path = $"requestBoards.{i}"

            [
                yield! nonEmpty (path + ".id") board.Id
                yield! nonEmpty (path + ".sceneId") board.SceneId
                yield! integer (path + ".x") board.X
                yield! integer (path + ".y") board.Y
                yield! positiveInt (path + ".intervalDays") board.IntervalDays
                yield!
                    each board.Entries (fun j entry ->
                        let ep = $"{path}.entries.{j}"

                        [
                            yield! nonEmpty (ep + ".questId") entry.QuestId
                            yield! positiveInt (ep + ".weight") entry.Weight
                            yield! whenSome entry.MinQuantity (positiveInt (ep + ".minQuantity"))
                            yield! whenSome entry.MaxQuantity (positiveInt (ep + ".maxQuantity"))
                            yield! whenSome entry.MinReward (fun value -> integer (ep + ".minReward") value @ nonNegative (ep + ".minReward") value)
                            yield! whenSome entry.MaxReward (fun value -> integer (ep + ".maxReward") value @ nonNegative (ep + ".maxReward") value)
                            if entry.MinQuantity > entry.MaxQuantity && entry.MaxQuantity.IsSome then
                                yield! parse ep "Minimum quantity exceeds maximum"
                            if entry.MinReward > entry.MaxReward && entry.MaxReward.IsSome then
                                yield! parse ep "Minimum reward exceeds maximum"
                        ])
            ])

    /// The `rngState` literal and tuple, from its algorithm and state words.
    let private rngState (algorithm: string option) (words: uint32 list option) =
        [
            if algorithm <> Some "xoshiro128ss" then
                yield! parse "rngState.algorithm" "Invalid literal value, expected \"xoshiro128ss\""
            let isTuple =
                match words with
                | None -> false
                | Some words -> words.Length = 4

            if not isTuple then
                yield! parse "rngState.s" "Expected a tuple of 4 integers"
        ]

    let private packsAndRng (p: GameProject) =
        [
            yield!
                each p.ContentPacks (fun i install ->
                    if PackRules.isValidPackId install.Pack.Manifest.Id then
                        []
                    else
                        parse $"contentPacks.{i}.pack.manifest.id" "pack ids must be lowercase letters, digits and dashes")
            match p.RngState with
            | None -> ()
            | Some rng -> yield! rngState (Some rng.Algorithm) (Some rng.S)
        ]

    /// A whole number in `min`..`max`, as a lint (Problems), never a load error.
    let private lintWhole path (value: float) (min: float) (max: float) =
        if not (isInteger value && value >= min && value <= max) then
            lint path $"Expected a whole number from {num min} to {num max}, received {num value}"
        else
            []

    /// A light (M8): radius 1–16 tiles, brightness 1–100 percent, a colour or none.
    let private light path (value: LightSource option) =
        match value with
        | None -> []
        | Some light ->
            [
                yield! lintWhole (path + ".radius") light.Radius 1.0 16.0
                yield! lintWhole (path + ".brightness") light.Brightness 1.0 100.0
                if light.Color.Length > 0 then
                    yield! color (path + ".color") light.Color
            ]

    /// Lights, tall terrains and extra scene layers (M8).
    let private richerMaps (p: GameProject) =
        [
            yield!
                each (defaultArg p.Terrains []) (fun t terrain ->
                    [
                        yield! light $"terrains.{t}.light" terrain.Light
                        match terrain.Tall with
                        | Some tall -> yield! lintWhole $"terrains.{t}.tall" tall 0.0 8.0
                        | None -> ()
                    ])
            yield! each p.NodeTypes (fun n node -> light $"nodeTypes.{n}.light" node.Light)
            yield! each p.MachineTypes (fun m machine -> light $"machineTypes.{m}.light" machine.Light)
            yield! each (defaultArg p.CustomCrops []) (fun c crop -> light $"customCrops.{c}.light" crop.Light)
            yield! each (defaultArg p.BuildingTypes []) (fun b building -> light $"buildingTypes.{b}.light" building.Light)
            yield!
                each p.Scenes (fun s scene ->
                    let layers = defaultArg scene.Layers []

                    each layers (fun l layer ->
                        [
                            yield! nonEmpty $"scenes.{s}.layers.{l}.id" layer.Id
                            if layers |> List.take l |> List.exists (fun earlier -> earlier.Id = layer.Id) then
                                yield! lint $"scenes.{s}.layers.{l}.id" $"Duplicate layer id '{layer.Id}'"
                        ]))
        ]

    let private fishingAndForage (p: GameProject) =
        let seasonIds = p.Settings.Calendar.Seasons |> List.map (fun s -> s.Id)

        let seasons path values =
            each (defaultArg values []) (fun i season -> oneOf $"{path}.{i}" season seasonIds)

        let fishFilter path (filter: FishingFilter option) =
            match filter with
            | None -> []
            | Some f ->
                [
                    yield! seasons (path + ".seasons") f.Seasons
                    yield!
                        whenSome f.StartMinute (fun n ->
                            integer (path + ".startMinute") n
                            @ range (path + ".startMinute") n 0.0 SettingsSchema.MaxDayEndMinute)
                    yield!
                        whenSome f.EndMinute (fun n ->
                            integer (path + ".endMinute") n
                            @ range (path + ".endMinute") n 0.0 SettingsSchema.MaxDayEndMinute)
                    if defaultArg f.StartMinute 0.0 > defaultArg f.EndMinute SettingsSchema.MaxDayEndMinute then
                        yield! lint path "Start minute must not exceed end minute"
                    yield! each (defaultArg f.WeatherIds []) (fun i id -> oneOf $"{path}.weatherIds.{i}" id (p.Weather.Types |> List.map (fun w -> w.Id)))
                    yield! conditions path (f.Condition |> Option.toList)
                ]

        [
            for n, node in List.indexed p.NodeTypes do
                yield! seasons $"nodeTypes.{n}.seasons" node.Seasons

                for d, drop in List.indexed node.Drops do
                    yield! seasons $"nodeTypes.{n}.drops.{d}.seasons" drop.Seasons
            for f, table in List.indexed p.FishTables do
                let path = $"fishTables.{f}"
                yield! fishFilter (path + ".filter") table.Filter

                for e, entry in List.indexed table.Entries do
                    let ep = $"{path}.entries.{e}"
                    yield! positiveInt (ep + ".weight") entry.Weight
                    yield! range (ep + ".difficulty") entry.Difficulty 0.0 1.0

                    match entry.Rules with
                    | None -> ()
                    | Some rules ->
                        yield! fishFilter (ep + ".rules.filter") rules.Filter
                        yield! whenSome rules.Xp (fun n -> integer (ep + ".rules.xp") n @ nonNegative (ep + ".rules.xp") n)
                        yield! whenSome rules.MinSize (positiveInt (ep + ".rules.minSize"))
                        yield! whenSome rules.MaxSize (positiveInt (ep + ".rules.maxSize"))

                        if rules.MaxSize.IsSome && rules.MinSize.IsNone then
                            yield! lint ep "A size range needs its minimum"

                        if defaultArg rules.MinSize 0.0 > defaultArg rules.MaxSize System.Double.MaxValue then
                            yield! lint ep "Minimum size must not exceed maximum"
            for i, item in List.indexed p.Items do
                match item.FishingGear with
                | None -> ()
                | Some gear ->
                    let path = $"items.{i}.fishingGear"
                    yield! oneOf (path + ".kind") gear.Kind [ "bait"; "tackle" ]
                    yield! range (path + ".escapeReduction") gear.EscapeReduction 0.0 100.0
                    yield! integer (path + ".escapeReduction") gear.EscapeReduction
                    yield! integer (path + ".weightBonus") gear.WeightBonus
                    yield! nonNegative (path + ".weightBonus") gear.WeightBonus
                    yield! integer (path + ".qualityBonus") gear.QualityBonus
                    yield! nonNegative (path + ".qualityBonus") gear.QualityBonus
            for m, machine in List.indexed p.MachineTypes do
                match machine.FishProduction with
                | None -> ()
                | Some production ->
                    let path = $"machineTypes.{m}.fishProduction"
                    yield! positiveInt (path + ".everyDays") production.EveryDays
                    yield! positiveInt (path + ".quantity") production.Quantity

                    if production.TableId.IsSome = production.SpeciesItemId.IsSome then
                        yield! lint path "Choose one trap table or one pond species"
            for s, scene in List.indexed p.Scenes do
                for t, table in List.indexed (defaultArg scene.ForageSpawns []) do
                    let path = $"scenes.{s}.forageSpawns.{t}"
                    yield! nonEmpty (path + ".id") table.Id
                    yield! positiveInt (path + ".maximum") table.Maximum
                    yield! range (path + ".maximum") table.Maximum 1.0 1024.0
                    yield! whenSome table.LifetimeDays (positiveInt (path + ".lifetimeDays"))
                    yield! seasons (path + ".seasons") table.Seasons

                    if table.Terrains.IsEmpty then
                        yield! lint path "Choose at least one terrain for forage"

                    if table.Entries.IsEmpty then
                        yield! lint path "Choose at least one node type for forage"

                    for e, entry in List.indexed table.Entries do
                        yield! positiveInt $"{path}.entries.{e}.weight" entry.Weight

                    match table.Region with
                    | None -> ()
                    | Some region ->
                        yield! integer (path + ".region.x") region.X
                        yield! integer (path + ".region.y") region.Y
                        yield! positiveInt (path + ".region.width") region.Width
                        yield! positiveInt (path + ".region.height") region.Height
        ]

    /// Every finding, parse- and lint-level interleaved in the C# order.
    let private progression (p: GameProject) =
        let upgrades =
            [ "setInventorySize"; "addInventorySize"; "addMaxEnergy"; "addMaxHealth" ]

        let item id =
            p.Items |> List.tryFind (fun item -> item.Id = id)

        let stat path (value: string) =
            if value.StartsWith("sellPrice:", StringComparison.Ordinal) then
                nonEmpty path (value.Substring("sellPrice:".Length))
            else
                oneOf path value ("" :: progressionStats)

        let unique path ids =
            ids
            |> List.countBy id
            |> List.collect (fun (id, count) -> if count > 1 then lint path $"Duplicate id: {id}" else [])

        [
            yield! unique "settings.skillDefinitions" (defaultArg p.Settings.SkillDefinitions [] |> List.map _.Id)
            match p.Settings.Progression with
            | Some limits ->
                yield! positiveInt "settings.progression.maxInventorySize" limits.MaxInventorySize
                yield! range "settings.progression.maxInventorySize" limits.MaxInventorySize 1.0 1024.0
                yield! positiveInt "settings.progression.maxEnergy" limits.MaxEnergy
                yield! positiveInt "settings.progression.maxHealth" limits.MaxHealth
            | None -> ()
            for i, skill in List.indexed (defaultArg p.Settings.SkillDefinitions []) do
                let path = $"settings.skillDefinitions.{i}"
                yield! nonEmpty (path + ".id") skill.Id

                yield!
                    each skill.Training (fun j t ->
                        oneOf $"{path}.training.{j}.action" t.Action [ "harvest"; "gather"; "catch"; "gift"; "talk"; "craft"; "slay"; "ship" ]
                        @ integer $"{path}.training.{j}.experience" t.Experience
                        @ nonNegative $"{path}.training.{j}.experience" t.Experience)

                yield!
                    each skill.Rewards (fun j reward ->
                        nonNegative $"{path}.rewards.{j}.level" reward.Level
                        @ integer $"{path}.rewards.{j}.level" reward.Level
                        @ stat $"{path}.rewards.{j}.stat" reward.Stat
                        @ integer $"{path}.rewards.{j}.amount" reward.Amount
                        @ outcomes $"{path}.rewards.{j}" (defaultArg reward.Outcomes []))

                let perks = defaultArg skill.Perks []
                yield! unique (path + ".perks") (perks |> List.map _.Id)

                for j, perk in List.indexed perks do
                    let pp = $"{path}.perks.{j}"
                    yield! nonEmpty (pp + ".id") perk.Id
                    yield! nonNegative (pp + ".level") perk.Level
                    yield! integer (pp + ".level") perk.Level
                    yield! unique (pp + ".choices") (perk.Choices |> List.map _.Id)

                    if perk.Choices.IsEmpty then
                        yield! lint (pp + ".choices") "Choose at least one perk reward"

                    for k, choice in List.indexed perk.Choices do
                        let cp = $"{pp}.choices.{k}"
                        yield! nonEmpty (cp + ".id") choice.Id
                        yield! nonEmpty (cp + ".name") choice.Name
                        yield! stat (cp + ".stat") choice.Stat
                        yield! integer (cp + ".amount") choice.Amount
                        yield! outcomes cp (defaultArg choice.Outcomes [])
            for i, definition in List.indexed p.Items do
                yield! each (defaultArg definition.Buffs []) (fun j b -> buffDefinition $"items.{i}.buffs.{j}" b)
            for i, shop in List.indexed p.Shops do
                for j, stock in List.indexed shop.Stock do
                    match stock.Upgrade with
                    | None -> ()
                    | Some upgrade ->
                        let path = $"shops.{i}.stock.{j}.upgrade"
                        yield! nonEmpty (path + ".id") upgrade.Id
                        yield! oneOf (path + ".kind") upgrade.Kind upgrades
                        yield! positiveInt (path + ".amount") upgrade.Amount

                for j, rule in List.indexed (defaultArg shop.ToolUpgrades []) do
                    let path = $"shops.{i}.toolUpgrades.{j}"
                    yield! nonNegative (path + ".cost") rule.Cost
                    yield! integer (path + ".cost") rule.Cost
                    yield! positiveInt (path + ".nights") rule.Nights

                    match item rule.FromItemId, item rule.ToItemId with
                    | Some source, Some target when
                        source.ToolType.IsSome
                        && source.ToolType = target.ToolType
                        && defaultArg target.ToolTier 1.0 = defaultArg source.ToolTier 1.0 + 1.0
                        ->
                        ()
                    | _ -> yield! lint path "A tool upgrade must connect existing tools of the same kind at consecutive tiers"

                    for k, ingredient in List.indexed rule.Materials do
                        yield! positiveInt $"{path}.materials.{k}.quantity" ingredient.Quantity

                        if item ingredient.ItemId |> Option.isNone then
                            yield! lint $"{path}.materials.{k}.itemId" "Material item does not exist"

                        if ingredient.ItemId = rule.FromItemId then
                            yield! lint path "The handed-in tool cannot also be a material"
        ]

    let private calendarDepth (p: GameProject) : Finding list =
        let whole field value =
            integer field value @ nonNegative field value

        [
            for i, npc in List.indexed p.Npcs do
                for j, variant in List.indexed (defaultArg npc.Schedules []) do
                    let path = $"npcs.{i}.schedules.{j}"
                    yield! nonEmpty (path + ".id") variant.Id
                    yield! conditions path variant.Conditions

                    for k, stop in List.indexed variant.Stops do
                        yield! whole $"{path}.stops.{k}.minute" stop.Minute
                        yield! integer $"{path}.stops.{k}.x" stop.X
                        yield! integer $"{path}.stops.{k}.y" stop.Y

                for j, stop in List.indexed (defaultArg npc.Schedule []) do
                    yield! whole $"npcs.{i}.schedule.{j}.minute" stop.Minute
            for i, shop in List.indexed p.Shops do
                let path = $"shops.{i}"

                match shop.Hours with
                | Some hours ->
                    yield! whole (path + ".hours.open") hours.Open
                    yield! whole (path + ".hours.close") hours.Close
                | None -> ()

                match shop.Saturation with
                | Some rule ->
                    yield! positiveInt (path + ".saturation.quantityPerStep") rule.QuantityPerStep
                    yield! positiveInt (path + ".saturation.recoveryDays") rule.RecoveryDays
                    yield! range (path + ".saturation.reductionPerStep") rule.ReductionPerStep 0.0 1.0
                    yield! range (path + ".saturation.minimumMultiplier") rule.MinimumMultiplier 0.0 1.0
                | None -> ()

                for j, rule in List.indexed (defaultArg shop.PriceModifiers []) do
                    let rulePath = $"{path}.priceModifiers.{j}"
                    yield! conditions rulePath rule.Conditions
                    yield! whenSome rule.BuyMultiplier (fun value -> range (rulePath + ".buyMultiplier") value 0.0 1000.0)
                    yield! whenSome rule.SellMultiplier (fun value -> range (rulePath + ".sellMultiplier") value 0.0 1000.0)
            for i, festival in List.indexed p.Settings.Calendar.Festivals do
                match festival.Program with
                | None -> ()
                | Some program ->
                    let path = $"settings.calendar.festivals.{i}.program"
                    yield! conditions path program.Conditions
                    yield! whole (path + ".openMinute") program.OpenMinute
                    yield! whole (path + ".closeMinute") program.CloseMinute

                    for field, value in
                        [
                            "entranceX", program.EntranceX
                            "entranceY", program.EntranceY
                            "arrivalX", program.ArrivalX
                            "arrivalY", program.ArrivalY
                        ] do
                        yield! integer (path + "." + field) value

                    for j, guest in List.indexed program.Attendance do
                        yield! integer $"{path}.attendance.{j}.x" guest.X
                        yield! integer $"{path}.attendance.{j}.y" guest.Y

                    match program.Activity with
                    | Some activity ->
                        yield! integer (path + ".activity.x") activity.X
                        yield! integer (path + ".activity.y") activity.Y
                    | None -> ()
        ]

    let private findings (p: GameProject) : Finding list =
        [
            yield! integer "schemaVersion" p.SchemaVersion
            yield! nonEmpty "id" p.Id
            yield! oneOf "mode" p.Mode EditorModes.All
            yield! integer "currentYear" p.CurrentYear
            yield! scenes p
            yield!
                each p.Scenes (fun s scene ->
                    [
                        yield! whenSome scene.Temperature (integer $"scenes.{s}.temperature")
                        match scene.Atmosphere with
                        | Some l -> yield! look $"scenes.{s}.atmosphere" l
                        | None -> ()
                    ])
            yield! vocabulary p
            yield! richerMaps p
            yield! fishingAndForage p
            yield! cutscenes p
            yield! player p
            yield! items p
            yield! npcs p
            yield! each p.Dialogues (fun d dialogue -> nonEmpty $"dialogues.{d}.id" dialogue.Id)
            yield! quests p
            yield! requestBoards p
            yield! events p
            yield! actionsAndMinigames p
            yield! definitions p
            yield! settings p
            yield! progression p
            yield! calendarDepth p
            yield! mine p
            yield! packsAndRng p
        ]

    let private at (level: Level) (all: Finding list) : SchemaIssue list =
        all |> List.choose (fun (l, issue) -> if l = level then Some issue else None)

    /// Parse-level checks only (what zod rejects; C# `SchemaValidation.ValidateProject`).
    /// Import and migrations use this.
    let projectIssues (project: GameProject) : SchemaIssue list = findings project |> at Level.Parse

    /// Structural lint zod cannot express (C# `SchemaValidation.LintProject`). The web editor
    /// lets creators save all of these, so they are reported, not rejected.
    let projectLints (project: GameProject) : SchemaIssue list = findings project |> at Level.Lint

    /// `projectIssues` and `projectLints` from one pass over the project (the Problems panel
    /// shows both).
    let projectChecks (project: GameProject) : SchemaIssue list * SchemaIssue list =
        let all = findings project
        at Level.Parse all, at Level.Lint all

    let private strings (issues: SchemaIssue list) = issues |> List.map string

    /// `projectIssues` as `path: message` strings .
    let validateProject (project: GameProject) : string list = projectIssues project |> strings

    /// `projectLints` as `path: message` strings (C# `SchemaValidation.LintProject`).
    let lintProject (project: GameProject) : string list = projectLints project |> strings
