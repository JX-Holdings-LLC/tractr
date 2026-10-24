namespace FarmEngine.Authoring

open System
open FarmEngine.Schemas

/// One failed schema check: its dotted path (`scenes.0.tiles.1.2.background`) and message.
type SchemaIssue =
    { Path: string
      Message: string }

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

    let private parse (path: string) (message: string) : Finding list = [ Level.Parse, { Path = path; Message = message } ]
    let private lint (path: string) (message: string) : Finding list = [ Level.Lint, { Path = path; Message = message } ]

    /// The checks for every element of a list, in order.
    let private each (list: 'T list) (check: int -> 'T -> Finding list) : Finding list =
        list |> List.mapi check |> List.concat

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
        if isInteger value then [] else parse path $"Expected integer, received {num value}"

    let private positiveInt path (value: float) =
        if not (isInteger value) then parse path $"Expected integer, received {num value}"
        elif value <= 0.0 then parse path "Number must be greater than 0"
        else []

    // `not (value > 0)` rather than `value <= 0`: NaN fails, like zod.
    let private positive path (value: float) =
        if not (value > 0.0) then parse path "Number must be greater than 0" else []

    let private nonNegative path (value: float) =
        if not (value >= 0.0) then parse path "Number must be greater than or equal to 0" else []

    let private range path (value: float) (min: float) (max: float) =
        if not (value >= min && value <= max) then parse path $"Number must be between {num min} and {num max}" else []

    /// zod `z.string()` accepts "": an empty id is a lint problem, not a parse error.
    let private nonEmpty path (value: string) =
        if String.IsNullOrEmpty value then lint path "Required id must be a non-empty string" else []

    let private enumValue path (value: string) (allowed: string list) =
        if List.contains value allowed then []
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
        [ match x2 with
          | Some x2 when x2 < x -> yield! lint (path + ".x2") "x2 must be >= x"
          | _ -> ()
          match y2 with
          | Some y2 when y2 < y -> yield! lint (path + ".y2") "y2 must be >= y"
          | _ -> () ]

    let rec private conditions (basePath: string) (list: EventCondition list) =
        each list (fun c condition ->
            let cp = $"{basePath}.conditions.{c}"
            match condition with
            | EventCondition.EnterTile t -> region cp t.X t.Y t.X2 t.Y2
            | EventCondition.InteractTile t -> region cp t.X t.Y t.X2 t.Y2
            | EventCondition.HasItem h -> nonEmpty (cp + ".itemId") h.ItemId
            | EventCondition.InventorySpace s -> nonEmpty (cp + ".itemId") s.ItemId @ positiveInt (cp + ".quantity") s.Quantity
            | EventCondition.Flag f ->
                if String.IsNullOrEmpty f.Flag then lint (cp + ".flag") "Flag name must be a non-empty string" else []
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
            | EventCondition.QuestStatus q -> nonEmpty (cp + ".questId") q.QuestId @ oneOf (cp + ".status") q.Status QuestStatuses.All
            | EventCondition.Friendship f -> nonEmpty (cp + ".npcId") f.NpcId
            | EventCondition.FestivalId f -> nonEmpty (cp + ".festivalId") f.FestivalId
            | EventCondition.Variable v ->
                nonEmpty (cp + ".name") v.Name @ oneOf (cp + ".compare") v.Compare VariableComparisons.All
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

    let private outcomes (basePath: string) (list: EventOutcome list) =
        each list (fun o outcome ->
            let op = $"{basePath}.outcomes.{o}"
            [ yield! oneOf (op + ".type") outcome.Type EventOutcomeTypes.All
              match outcome.Amount with
              | Some amount when Double.IsNaN amount || Double.IsInfinity amount -> yield! parse (op + ".amount") "Number must be finite"
              | _ -> ()
              yield! whenSome outcome.Radius (fun radius -> integer (op + ".radius") radius @ range (op + ".radius") radius 0.0 10.0) ])

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
            not (String.IsNullOrEmpty scene.Id) && scenes |> List.take s |> List.exists (fun earlier -> earlier.Id = scene.Id)
        let validWidth = isInteger scene.Width && scene.Width > 0.0
        let validHeight = isInteger scene.Height && scene.Height > 0.0
        [ yield! nonEmpty (sp + ".id") scene.Id
          if duplicate then yield! lint (sp + ".id") $"Duplicate scene id '{scene.Id}'"
          yield! positiveInt (sp + ".width") scene.Width
          yield! positiveInt (sp + ".height") scene.Height
          if validHeight && float scene.Tiles.Length <> scene.Height then
              yield! lint (sp + ".tiles") $"Expected {num scene.Height} rows (scene height), found {scene.Tiles.Length}"
          yield!
              each scene.Tiles (fun y row ->
                  let rp = $"{sp}.tiles.{y}"
                  [ if validWidth && float row.Length <> scene.Width then
                        yield! lint rp $"Expected {num scene.Width} tiles (scene width), found {row.Length}"
                    yield! each row (fun x t -> tile (fun () -> $"{rp}.{x}") t) ])
          yield! each scene.Transitions (fun t transition -> nonEmpty $"{sp}.transitions.{t}.toSceneId" transition.ToSceneId) ]

    let private scenes (p: GameProject) =
        [ yield! each p.Scenes (scene p.Scenes)
          let known = p.Scenes |> List.exists (fun s -> not (String.IsNullOrEmpty s.Id) && s.Id = p.StartSceneId)
          if not p.Scenes.IsEmpty && not known then yield! lint "startSceneId" $"No scene with id '{p.StartSceneId}'" ]

    let private player (p: GameProject) =
        [ yield! oneOf "player.direction" p.Player.Direction Directions.All
          yield! nonEmpty "player.sceneId" p.Player.SceneId
          yield! each p.Player.Inventory (fun i slot -> nonEmpty $"player.inventory.{i}.item.id" slot.Item.Id) ]

    let private items (p: GameProject) =
        each p.Items (fun i item ->
            let ip = $"items.{i}"
            [ yield! nonEmpty (ip + ".id") item.Id
              yield! whenSome item.ToolTier (positiveInt (ip + ".toolTier"))
              yield! whenSome item.Energy (nonNegative (ip + ".energy")) ])

    let private npcs (p: GameProject) =
        each p.Npcs (fun i npc ->
            let np = $"npcs.{i}"
            [ yield! nonEmpty (np + ".id") npc.Id
              yield! optionalOneOf (np + ".movePattern") npc.MovePattern NpcMovePatterns.All
              yield! whenSome npc.WanderRadius (positiveInt (np + ".wanderRadius"))
              match npc.PatrolPoints with
              | None -> ()
              | Some points ->
                  yield!
                      each points (fun k point ->
                          integer $"{np}.patrolPoints.{k}.x" point.X @ integer $"{np}.patrolPoints.{k}.y" point.Y)
              match npc.Birthday with
              | None -> ()
              | Some birthday ->
                  // The season is checked against the project's calendar in ChecksContent (a
                  // warning: calendars are customizable).
                  yield! integer (np + ".birthday.day") birthday.Day
              yield! each npc.Dialogue (fun d dialogue -> nonEmpty $"{np}.dialogue.{d}.id" dialogue.Id) ])

    let private quests (p: GameProject) =
        each p.Quests (fun q quest ->
            let qp = $"quests.{q}"
            [ yield! nonEmpty (qp + ".id") quest.Id
              yield! oneOf (qp + ".status") quest.Status QuestStatuses.All
              yield!
                  each quest.Objectives (fun o objective ->
                      nonEmpty $"{qp}.objectives.{o}.id" objective.Id
                      @ oneOf $"{qp}.objectives.{o}.type" objective.Type QuestObjectiveTypes.All) ])

    let private events (p: GameProject) =
        each p.Events (fun e event ->
            let ep = $"events.{e}"
            [ yield! nonEmpty (ep + ".id") event.Id
              yield! oneOf (ep + ".trigger") event.Trigger EventTriggers.All
              yield! conditions ep event.Conditions
              yield! outcomes ep event.Outcomes ])

    let private actionsAndMinigames (p: GameProject) =
        [ yield!
              each p.Actions (fun a action ->
                  let ap = $"actions.{a}"
                  [ yield! nonEmpty (ap + ".id") action.Id
                    yield! nonNegative (ap + ".energyCost") action.EnergyCost
                    match action.Hotkey with
                    | Some hotkey when hotkey.Length > 1 -> yield! parse (ap + ".hotkey") "String must contain at most 1 character(s)"
                    | _ -> ()
                    yield! conditions ap action.Conditions
                    yield! outcomes ap action.Outcomes ])
          yield!
              each p.Minigames (fun m minigame ->
                  [ yield! nonEmpty $"minigames.{m}.id" minigame.Id
                    yield!
                        each minigame.ResultTiers (fun t tier ->
                            let tp = $"minigames.{m}.resultTiers.{t}"
                            range (tp + ".minScore") tier.MinScore 0.0 1.0 @ outcomes tp tier.Outcomes) ]) ]

    let private color path (value: string) =
        let hex = value.Length = 7 || value.Length = 9
        if value.StartsWith "#" && hex && value |> Seq.skip 1 |> Seq.forall Uri.IsHexDigit then []
        else parse path $"Expected a colour like #rrggbb, received '{value}'"

    let private optionalColor path (value: string option) =
        match value with
        | None -> []
        | Some value -> color path value

    /// The world vocabulary (M1): terrains, tools, item categories, crop qualities and mutations.
    let private vocabulary (p: GameProject) =
        let each' (list: 'T list option) check = each (defaultArg list []) check
        [ yield!
              each' p.Terrains (fun t terrain ->
                  let tp = $"terrains.{t}"
                  [ yield! nonEmpty (tp + ".id") terrain.Id
                    yield! oneOf (tp + ".layer") terrain.Layer TileLayers.All
                    yield! color (tp + ".color") terrain.Color
                    yield! nonNegative (tp + ".hazardEnergy") terrain.HazardEnergy
                    yield!
                        each terrain.Transforms (fun k transform ->
                            oneOf $"{tp}.transforms.{k}.action" transform.Action ToolActions.Transforming) ])
          yield!
              each' p.Tools (fun t tool ->
                  let tp = $"tools.{t}"
                  [ yield! nonEmpty (tp + ".id") tool.Id
                    yield! oneOf (tp + ".action") tool.Action ToolActions.All
                    yield! nonNegative (tp + ".energyCost") tool.EnergyCost
                    match tool.Key with
                    | Some key when key.Length > 1 -> yield! parse (tp + ".key") "String must contain at most 1 character(s)"
                    | _ -> ()
                    yield!
                        each tool.Area (fun k area ->
                            positiveInt $"{tp}.area.{k}.width" area.Width @ positiveInt $"{tp}.area.{k}.length" area.Length) ])
          yield!
              each' p.ItemCategories (fun c category ->
                  nonEmpty $"itemCategories.{c}.id" category.Id @ nonNegative $"itemCategories.{c}.energy" category.Energy)
          yield!
              each' p.CropQualities (fun q quality ->
                  let qp = $"cropQualities.{q}"
                  [ yield! nonEmpty (qp + ".id") quality.Id
                    yield! positive (qp + ".priceMultiplier") quality.PriceMultiplier
                    yield! positive (qp + ".yieldMultiplier") quality.YieldMultiplier
                    yield! nonNegative (qp + ".mutationMultiplier") quality.MutationMultiplier
                    yield! integer (qp + ".stars") quality.Stars
                    yield! nonNegative (qp + ".stars") quality.Stars ])
          yield!
              each' p.CropMutations (fun m mutation ->
                  let mp = $"cropMutations.{m}"
                  nonEmpty (mp + ".id") mutation.Id
                  @ positive (mp + ".yieldMultiplier") mutation.YieldMultiplier
                  @ range (mp + ".share") mutation.Share 0.0 1.0) ]

    /// What crops need to grow (M1).
    let private requirements (path: string) (needs: GrowthRequirements option) =
        match needs with
        | None -> []
        | Some needs ->
            [ yield! optionalOneOf (path + ".light") needs.Light LightNeeds.All
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
                      @ positiveInt (np + ".min") need.Min) ]

    /// The look of the sky (M1).
    let private look (path: string) (look: AtmosphereLook) =
        [ yield!
              each look.Daylight (fun k key ->
                  nonNegative $"{path}.daylight.{k}.minute" key.Minute @ color $"{path}.daylight.{k}.color" key.Color)
          yield! optionalColor (path + ".foliageTint") look.FoliageTint ]

    /// Cutscenes (M2).
    let private cutscenes (p: GameProject) =
        each (defaultArg p.Cutscenes []) (fun c cutscene ->
            let cp = $"cutscenes.{c}"
            [ yield! nonEmpty (cp + ".id") cutscene.Id
              yield!
                  each cutscene.Steps (fun k step ->
                      let sp = $"{cp}.steps.{k}"
                      [ yield! oneOf (sp + ".type") step.Type CutsceneStepTypes.All
                        yield! optionalOneOf (sp + ".direction") step.Direction Directions.All
                        yield! optionalOneOf (sp + ".emote") step.Emote Emotes.All
                        yield! optionalOneOf (sp + ".mode") step.Mode [ "out"; "in" ]
                        yield! optionalColor (sp + ".color") step.Color
                        yield! whenSome step.Seconds (nonNegative (sp + ".seconds"))
                        yield! whenSome step.X (integer (sp + ".x"))
                        yield! whenSome step.Y (integer (sp + ".y"))
                        yield! conditions sp step.Conditions
                        yield! outcomes sp step.Outcomes ]) ])

    let private definitions (p: GameProject) =
        let idLists =
            [ "shops", p.Shops |> List.map (fun x -> x.Id)
              "recipes", p.Recipes |> List.map (fun x -> x.Id)
              "machineTypes", p.MachineTypes |> List.map (fun x -> x.Id)
              "animalSpecies", p.AnimalSpecies |> List.map (fun x -> x.Id)
              "fishTables", p.FishTables |> List.map (fun x -> x.Id)
              "animals", p.Animals |> List.map (fun x -> x.Id) ]
        [ for name, ids in idLists do
              for index, id in List.indexed ids do
                  yield! nonEmpty $"{name}.{index}.id" id
          yield!
              each p.NodeTypes (fun n node ->
                  let np = $"nodeTypes.{n}"
                  [ yield! nonEmpty (np + ".id") node.Id
                    yield! positiveInt (np + ".health") node.Health
                    yield! positiveInt (np + ".requiredToolTier") node.RequiredToolTier
                    yield! whenSome (Option.flatten node.RespawnDays) (positiveInt (np + ".respawnDays")) ])
          yield!
              each p.Recipes (fun r recipe ->
                  [ yield! nonNegative $"recipes.{r}.processingMinutes" recipe.ProcessingMinutes
                    yield! each recipe.Inputs (fun k input -> positiveInt $"recipes.{r}.inputs.{k}.quantity" input.Quantity)
                    yield! each recipe.Outputs (fun k output -> positiveInt $"recipes.{r}.outputs.{k}.quantity" output.Quantity) ])
          yield!
              each p.Weather.Types (fun w weather ->
                  nonEmpty $"weather.types.{w}.id" weather.Id
                  @ range $"weather.types.{w}.cropDamageChance" weather.CropDamageChance 0.0 1.0)
          yield!
              each (defaultArg p.Weather.Overlays []) (fun o overlay ->
                  let op = $"weather.overlays.{o}"
                  [ yield! nonEmpty (op + ".id") overlay.Id
                    yield! oneOf (op + ".style") overlay.Style OverlayStyles.All
                    if overlay.Style <> OverlayStyles.None then yield! color (op + ".color") overlay.Color
                    yield! nonNegative (op + ".density") overlay.Density
                    yield! nonNegative (op + ".speed") overlay.Speed
                    yield! optionalColor (op + ".tint") overlay.Tint ])
          yield!
              each (defaultArg p.CustomCrops []) (fun c crop -> requirements $"customCrops.{c}.requirements" crop.Requirements)
          for season, entries in p.Weather.Table do
              yield! each entries (fun k entry -> positive $"weather.table.{season}.{k}.weight" entry.Weight) ]

    let private settings (p: GameProject) =
        let s = p.Settings
        [ yield! positive "settings.maxEnergy" s.MaxEnergy
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
              yield!
                  parse "settings.time.minutesPerRealSecond" $"Number must be less than or equal to {num SettingsSchema.MaxMinutesPerRealSecond}"
          yield!
              each s.Calendar.Seasons (fun c season ->
                  nonEmpty $"settings.calendar.seasons.{c}.id" season.Id
                  @ positiveInt $"settings.calendar.seasons.{c}.days" season.Days)
          yield!
              each s.Calendar.Festivals (fun f festival ->
                  nonEmpty $"settings.calendar.festivals.{f}.id" festival.Id
                  @ positiveInt $"settings.calendar.festivals.{f}.day" festival.Day)
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
                  yield! look $"settings.atmosphere.seasons.{season}" l ]

    let private mine (p: GameProject) =
        let m = p.Mine
        [ yield! positiveInt "mine.floors" m.Floors
          yield! positiveInt "mine.floorWidth" m.FloorWidth
          yield! positiveInt "mine.floorHeight" m.FloorHeight
          yield! positiveInt "mine.elevatorEvery" m.ElevatorEvery
          yield! range "mine.ladderChance" m.LadderChance 0.0 1.0
          yield!
              each m.Bands (fun b band ->
                  [ yield! positiveInt $"mine.bands.{b}.fromFloor" band.FromFloor
                    yield! positiveInt $"mine.bands.{b}.toFloor" band.ToFloor
                    yield! range $"mine.bands.{b}.density" band.Density 0.0 1.0 ])
          yield! whenSome p.MineDeepestFloor (fun deepest -> integer "mineDeepestFloor" deepest @ nonNegative "mineDeepestFloor" deepest) ]

    /// The `rngState` literal and tuple, from its algorithm and state words.
    let private rngState (algorithm: string option) (words: uint32 list option) =
        [ if algorithm <> Some "xoshiro128ss" then yield! parse "rngState.algorithm" "Invalid literal value, expected \"xoshiro128ss\""
          let isTuple =
              match words with
              | None -> false
              | Some words -> words.Length = 4
          if not isTuple then yield! parse "rngState.s" "Expected a tuple of 4 integers" ]

    let private packsAndRng (p: GameProject) =
        [ yield!
              each p.ContentPacks (fun i install ->
                  if PackRules.isValidPackId install.Pack.Manifest.Id then []
                  else parse $"contentPacks.{i}.pack.manifest.id" "pack ids must be lowercase letters, digits and dashes")
          match p.RngState with
          | None -> ()
          | Some rng -> yield! rngState (Some rng.Algorithm) (Some rng.S) ]

    /// Every finding, parse- and lint-level interleaved in the C# order.
    let private findings (p: GameProject) : Finding list =
        [ yield! integer "schemaVersion" p.SchemaVersion
          yield! nonEmpty "id" p.Id
          yield! oneOf "mode" p.Mode EditorModes.All
          yield! integer "currentYear" p.CurrentYear
          yield! scenes p
          yield!
              each p.Scenes (fun s scene ->
                  [ yield! whenSome scene.Temperature (integer $"scenes.{s}.temperature")
                    match scene.Atmosphere with
                    | Some l -> yield! look $"scenes.{s}.atmosphere" l
                    | None -> () ])
          yield! vocabulary p
          yield! cutscenes p
          yield! player p
          yield! items p
          yield! npcs p
          yield! each p.Dialogues (fun d dialogue -> nonEmpty $"dialogues.{d}.id" dialogue.Id)
          yield! quests p
          yield! events p
          yield! actionsAndMinigames p
          yield! definitions p
          yield! settings p
          yield! mine p
          yield! packsAndRng p ]

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

