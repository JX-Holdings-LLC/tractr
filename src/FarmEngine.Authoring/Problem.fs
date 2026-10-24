namespace FarmEngine.Authoring

open FarmEngine.Schemas

/// How bad a problem is. Errors block export (`Problems.blocksExport`).
[<RequireQualifiedAccess>]
type Severity =
    | Error
    | Warning
    | Info

/// The editor to open for a problem (ProblemsPanel.tsx "go to"). `Scene` carries the tile to
/// scroll to. Qualified access keeps the case names from shadowing the schema record types.
[<RequireQualifiedAccess>]
type NavigationTarget =
    | Scene of sceneId: string * x: int * y: int
    | Npc of id: string
    | Item of id: string
    | Crop of id: string
    | Quest of id: string
    | Event of id: string
    | Shop of id: string
    | Recipe of id: string
    | NodeType of id: string
    | MachineType of id: string
    | AnimalSpecies of id: string
    | FishTable of id: string
    | Action of id: string
    | Minigame of id: string
    | Settings
    | Interface
    | Pack of id: string
    | Asset of id: string
    | Terrain of id: string
    | Tool of id: string
    | ItemCategory of id: string
    | CropQuality of id: string
    | CropMutation of id: string
    | Cutscene of id: string
    /// A heart event (M4).
    | HeartEvent of id: string
    /// A building type (M5) and a building on the start map (M5).
    | Building of id: string
    | PlacedBuilding of id: string
    /// Monsters (M7), collections, bundles, achievements and letters (M6).
    | Monster of id: string
    | Collection of id: string
    | Bundle of id: string
    | Achievement of id: string
    | Mail of id: string

/// One line of the Problems panel: the same shape for schema errors, content lints and editor
/// checks. `Path` is a JSON path into the project (`npcs[3].dialogue[0].options[1].nextDialogueId`)
/// and `Code` a stable dotted id (`dialogue.unreachable`) the UI can filter on. The extra
/// members exist for C#, which reads them instead of matching on the unions.
type Problem =
    { Severity: Severity
      Code: string
      Message: string
      Path: string
      Target: NavigationTarget option }

    member this.IsError = (this.Severity = Severity.Error)
    member this.IsWarning = (this.Severity = Severity.Warning)

    /// "error" | "warning" | "info" (the web `ProblemSeverity` strings).
    member this.SeverityName =
        match this.Severity with
        | Severity.Error -> "error"
        | Severity.Warning -> "warning"
        | Severity.Info -> "info"

    member this.HasTarget = this.Target.IsSome

    /// The editor tab to open: "scene", "npc", "item", "crop", "quest", "event", "shop",
    /// "recipe", "nodeType", "machineType", "animalSpecies", "fishTable", "action", "minigame",
    /// "settings", "interface", "pack", "asset", "terrain", "tool", "itemCategory", "cropQuality",
    /// "cropMutation", "cutscene", "heartEvent", "building", "placedBuilding", "monster",
    /// "collection", "bundle", "achievement", "mail" or "" when there is nowhere to go.
    member this.TargetKind =
        match this.Target with
        | None -> ""
        | Some target ->
            match target with
            | NavigationTarget.Scene _ -> "scene"
            | NavigationTarget.Npc _ -> "npc"
            | NavigationTarget.Item _ -> "item"
            | NavigationTarget.Crop _ -> "crop"
            | NavigationTarget.Quest _ -> "quest"
            | NavigationTarget.Event _ -> "event"
            | NavigationTarget.Shop _ -> "shop"
            | NavigationTarget.Recipe _ -> "recipe"
            | NavigationTarget.NodeType _ -> "nodeType"
            | NavigationTarget.MachineType _ -> "machineType"
            | NavigationTarget.AnimalSpecies _ -> "animalSpecies"
            | NavigationTarget.FishTable _ -> "fishTable"
            | NavigationTarget.Action _ -> "action"
            | NavigationTarget.Minigame _ -> "minigame"
            | NavigationTarget.Settings -> "settings"
            | NavigationTarget.Interface -> "interface"
            | NavigationTarget.Pack _ -> "pack"
            | NavigationTarget.Asset _ -> "asset"
            | NavigationTarget.Terrain _ -> "terrain"
            | NavigationTarget.Tool _ -> "tool"
            | NavigationTarget.ItemCategory _ -> "itemCategory"
            | NavigationTarget.CropQuality _ -> "cropQuality"
            | NavigationTarget.CropMutation _ -> "cropMutation"
            | NavigationTarget.Cutscene _ -> "cutscene"
            | NavigationTarget.HeartEvent _ -> "heartEvent"
            | NavigationTarget.Building _ -> "building"
            | NavigationTarget.PlacedBuilding _ -> "placedBuilding"
            | NavigationTarget.Monster _ -> "monster"
            | NavigationTarget.Collection _ -> "collection"
            | NavigationTarget.Bundle _ -> "bundle"
            | NavigationTarget.Achievement _ -> "achievement"
            | NavigationTarget.Mail _ -> "mail"

    /// The id of the thing to open (scene id for tiles), or null for settings/interface/none.
    member this.TargetId: string | null =
        match this.Target with
        | None -> null
        | Some target ->
            match target with
            | NavigationTarget.Scene(id, _, _) -> id
            | NavigationTarget.Npc id | NavigationTarget.Item id | NavigationTarget.Crop id | NavigationTarget.Quest id
            | NavigationTarget.Event id | NavigationTarget.Shop id | NavigationTarget.Recipe id | NavigationTarget.NodeType id
            | NavigationTarget.MachineType id | NavigationTarget.AnimalSpecies id | NavigationTarget.FishTable id
            | NavigationTarget.Action id | NavigationTarget.Minigame id | NavigationTarget.Pack id | NavigationTarget.Asset id
            | NavigationTarget.Terrain id | NavigationTarget.Tool id | NavigationTarget.ItemCategory id
            | NavigationTarget.CropQuality id | NavigationTarget.CropMutation id | NavigationTarget.Cutscene id
            | NavigationTarget.HeartEvent id | NavigationTarget.Building id | NavigationTarget.PlacedBuilding id
            | NavigationTarget.Monster id | NavigationTarget.Collection id | NavigationTarget.Bundle id
            | NavigationTarget.Achievement id | NavigationTarget.Mail id -> id
            | NavigationTarget.Settings | NavigationTarget.Interface -> null

    /// Tile to scroll to for a scene target (0 otherwise).
    member this.TargetX =
        match this.Target with
        | Some(NavigationTarget.Scene(_, x, _)) -> x
        | _ -> 0

    member this.TargetY =
        match this.Target with
        | Some(NavigationTarget.Scene(_, _, y)) -> y
        | _ -> 0

/// JSON paths for Problems (`npcs[3].dialogue[0]`).
module ProblemPath =
    let private isIdentifier (key: string) =
        key.Length > 0
        && not (System.Char.IsDigit key.[0])
        && key |> Seq.forall (fun c -> System.Char.IsLetterOrDigit c || c = '_' || c = '$' || c = '-')

    /// The member `key` of the value at `parent`: `parent.key`, or `parent["key"]` for a key
    /// that a dotted path would misread (a number, which reads as an index, or one with dots).
    let memberPath (parent: string) (key: string) : string =
        if isIdentifier key then parent + "." + key else parent + "[" + Json.quote key + "]"

/// Collects problems while the checks run.
type internal Sink() =
    let items = ResizeArray<Problem>()

    member _.Add(severity: Severity, code: string, path: string, message: string, target: NavigationTarget option) =
        items.Add { Severity = severity; Code = code; Message = message; Path = path; Target = target }

    member this.Error(code, path, message, target) = this.Add(Severity.Error, code, path, message, target)
    member this.Warning(code, path, message, target) = this.Add(Severity.Warning, code, path, message, target)
    member this.Info(code, path, message, target) = this.Add(Severity.Info, code, path, message, target)

    member _.ToList() : Problem list = List.ofSeq items

/// Id sets every check looks things up in, built once per `Problems.collect`.
type internal Context =
    { Project: GameProject
      Scenes: System.Collections.Generic.IDictionary<string, Scene>
      ItemIds: System.Collections.Generic.HashSet<string>
      NpcIds: System.Collections.Generic.HashSet<string>
      QuestIds: System.Collections.Generic.HashSet<string>
      ShopIds: System.Collections.Generic.HashSet<string>
      ActionIds: System.Collections.Generic.HashSet<string>
      MinigameIds: System.Collections.Generic.HashSet<string>
      MachineTypeIds: System.Collections.Generic.HashSet<string>
      NodeTypeIds: System.Collections.Generic.HashSet<string>
      CropIds: System.Collections.Generic.HashSet<string>
      SpeciesIds: System.Collections.Generic.HashSet<string>
      AssetIds: System.Collections.Generic.HashSet<string>
      SeasonIds: System.Collections.Generic.HashSet<string>
      FestivalIds: System.Collections.Generic.HashSet<string>
      WeatherIds: System.Collections.Generic.HashSet<string>
      /// The game's terrains, tools, item categories, crop qualities and mutations (M1).
      TerrainIds: System.Collections.Generic.HashSet<string>
      ToolIds: System.Collections.Generic.HashSet<string>
      CategoryIds: System.Collections.Generic.HashSet<string>
      QualityIds: System.Collections.Generic.HashSet<string>
      MutationIds: System.Collections.Generic.HashSet<string>
      /// Cutscenes, sound assets and music assets (M2).
      CutsceneIds: System.Collections.Generic.HashSet<string>
      SoundIds: System.Collections.Generic.HashSet<string>
      MusicIds: System.Collections.Generic.HashSet<string>
      /// Building types and the buildings on the start map (M5), monsters (M7), collections,
      /// bundles, achievements and letters (M6).
      BuildingIds: System.Collections.Generic.HashSet<string>
      PlacedBuildingIds: System.Collections.Generic.HashSet<string>
      MonsterIds: System.Collections.Generic.HashSet<string>
      CollectionIds: System.Collections.Generic.HashSet<string>
      BundleIds: System.Collections.Generic.HashSet<string>
      AchievementIds: System.Collections.Generic.HashSet<string>
      MailIds: System.Collections.Generic.HashSet<string>
      /// Flags something can set: `setFlag`/`clearFlag` and the variable outcomes, dialogue `eventFlag`s, the
      /// project's flag table and every event's auto-managed fired flag.
      KnownFlags: System.Collections.Generic.HashSet<string> }

module internal Context =
    open System.Collections.Generic

    let private ids (xs: seq<string>) = HashSet<string>(xs)

    let private flagsIn (outcomes: seq<EventOutcome>) =
        outcomes
        |> Seq.choose (fun o ->
            match o.Type with
            | EventOutcomeTypes.SetFlag | EventOutcomeTypes.ClearFlag -> o.FlagName
            // Variables are flags that hold a number or a text (M3).
            | EventOutcomeTypes.SetVariable | EventOutcomeTypes.AddVariable -> o.Variable
            | _ -> None)

    let create (project: GameProject) : Context =
        let scenes = Dictionary<string, Scene>()
        for scene in project.Scenes do
            if not (scenes.ContainsKey scene.Id) then scenes.[scene.Id] <- scene
        let crops = ids (ContentCompiler.mergeCrops project.CustomCrops |> List.map fst)
        let flags = HashSet<string>()
        let add (xs: seq<string>) = for x in xs do flags.Add x |> ignore
        add (project.EventFlags |> List.map fst)
        add (project.Events |> Seq.map (fun e -> EventsSchema.EventFiredFlag e.Id))
        add (project.Events |> Seq.collect (fun e -> flagsIn e.Outcomes))
        add (project.Actions |> Seq.collect (fun a -> flagsIn a.Outcomes))
        add (project.Minigames |> Seq.collect (fun m -> m.ResultTiers |> Seq.collect (fun t -> flagsIn t.Outcomes)))
        // Rewards of collections, bundles and letters (M6) set flags too.
        add (defaultArg project.Collections [] |> Seq.collect (fun c -> c.Rewards |> Seq.collect (fun r -> flagsIn r.Outcomes)))
        add (defaultArg project.Bundles [] |> Seq.collect (fun b -> flagsIn b.Outcomes))
        add (defaultArg project.Mail [] |> Seq.collect (fun m -> flagsIn m.Outcomes))
        let dialogueFlags (d: Dialogue) = d.Options |> Seq.choose (fun o -> o.EventFlag)
        add (project.Dialogues |> Seq.collect dialogueFlags)
        add (project.Npcs |> Seq.collect (fun n -> n.Dialogue |> Seq.collect dialogueFlags))
        { Project = project
          Scenes = scenes
          ItemIds = ids (project.Items |> Seq.map (fun i -> i.Id))
          NpcIds = ids (project.Npcs |> Seq.map (fun n -> n.Id))
          QuestIds = ids (project.Quests |> Seq.map (fun q -> q.Id))
          ShopIds = ids (project.Shops |> Seq.map (fun s -> s.Id))
          ActionIds = ids (project.Actions |> Seq.map (fun a -> a.Id))
          MinigameIds = ids (project.Minigames |> Seq.map (fun m -> m.Id))
          MachineTypeIds = ids (project.MachineTypes |> Seq.map (fun m -> m.Id))
          NodeTypeIds = ids (EditScenes.placeableNodeTypes project |> Seq.map (fun n -> n.Id))
          CropIds = crops
          SpeciesIds = ids (project.AnimalSpecies |> Seq.map (fun s -> s.Id))
          AssetIds = ids (project.CustomAssets |> Seq.map (fun a -> a.Id))
          SeasonIds = ids (project.Settings.Calendar.Seasons |> Seq.map (fun s -> s.Id))
          FestivalIds = ids (project.Settings.Calendar.Festivals |> Seq.map (fun f -> f.Id))
          WeatherIds = ids (project.Weather.Types |> Seq.map (fun w -> w.Id))
          TerrainIds = ids (ContentCompiler.terrains project |> Seq.map (fun t -> t.Id))
          ToolIds = ids (ContentCompiler.tools project |> Seq.map (fun t -> t.Id))
          CategoryIds = ids (ContentCompiler.itemCategories project |> Seq.map (fun c -> c.Id))
          QualityIds = ids (ContentCompiler.cropQualities project |> Seq.map (fun q -> q.Id))
          MutationIds = ids (ContentCompiler.cropMutations project |> Seq.map (fun m -> m.Id))
          CutsceneIds = ids (defaultArg project.Cutscenes [] |> Seq.map (fun c -> c.Id))
          SoundIds = ids (project.CustomAssets |> Seq.filter (fun a -> a.Type = CustomAssetTypes.Sound) |> Seq.map (fun a -> a.Id))
          MusicIds = ids (project.CustomAssets |> Seq.filter (fun a -> a.Type = CustomAssetTypes.Music) |> Seq.map (fun a -> a.Id))
          BuildingIds = ids (defaultArg project.BuildingTypes [] |> Seq.map (fun b -> b.Id))
          PlacedBuildingIds = ids (defaultArg project.Buildings [] |> Seq.map (fun b -> b.Id))
          MonsterIds = ids (defaultArg project.Monsters [] |> Seq.map (fun m -> m.Id))
          CollectionIds = ids (defaultArg project.Collections [] |> Seq.map (fun c -> c.Id))
          BundleIds = ids (defaultArg project.Bundles [] |> Seq.map (fun b -> b.Id))
          AchievementIds = ids (defaultArg project.Achievements [] |> Seq.map (fun a -> a.Id))
          MailIds = ids (defaultArg project.Mail [] |> Seq.map (fun m -> m.Id))
          KnownFlags = flags }

    /// A scene tile is inside the scene grid.
    let tileInScene (context: Context) (sceneId: string) (x: float) (y: float) =
        match context.Scenes.TryGetValue sceneId with
        | true, scene -> x >= 0.0 && x < scene.Width && y >= 0.0 && y < scene.Height
        | _ -> false

    let hasValue (value: string option) =
        match value with
        | None -> false
        | Some s -> s.Length > 0
