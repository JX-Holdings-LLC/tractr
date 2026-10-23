namespace FarmEngine.Authoring

open System
open System.Collections.Generic
open FarmEngine.Schemas

/// Problems for the world vocabulary (M1: terrains, tools, item categories, crop qualities and
/// mutations, growth requirements, weather overlays) and for sound, music and cutscenes (M2).
/// The schema checks (`SchemaChecks`) refuse values of the wrong shape; these find references
/// to things the game does not have and definitions that cannot work.
module internal ChecksVocabulary =
    let private hasValue = Context.hasValue

    let private text (value: string option) = defaultArg value ""

    /// Play-mode keys a tool key must not take: movement, interact and the menus (the built-in
    /// tools' own keys are free for a game's tools that replace them).
    let private reservedKeys = set [ "w"; "a"; "s"; "d"; "e"; " "; "z"; "i"; "j"; "x" ]

    let private duplicates (context: Context) (sink: Sink) =
        let check (family: string) (path: string) (ids: seq<string>) (target: string -> NavigationTarget) =
            let seen = HashSet<string>()
            ids
            |> Seq.iteri (fun i id ->
                if id.Length > 0 && not (seen.Add id) then
                    sink.Error(sprintf "duplicate.%s" family, sprintf "%s[%d].id" path i, sprintf "Duplicate %s id \"%s\"" family id, Some(target id)))
        let p = context.Project
        let ids (list: 'T list option) (id: 'T -> string) = defaultArg list [] |> Seq.map id
        check "terrain" "terrains" (ids p.Terrains (fun t -> t.Id)) NavigationTarget.Terrain
        check "tool" "tools" (ids p.Tools (fun t -> t.Id)) NavigationTarget.Tool
        check "itemCategory" "itemCategories" (ids p.ItemCategories (fun c -> c.Id)) NavigationTarget.ItemCategory
        check "cropQuality" "cropQualities" (ids p.CropQualities (fun q -> q.Id)) NavigationTarget.CropQuality
        check "cropMutation" "cropMutations" (ids p.CropMutations (fun m -> m.Id)) NavigationTarget.CropMutation
        check "cutscene" "cutscenes" (ids p.Cutscenes (fun c -> c.Id)) NavigationTarget.Cutscene

    /// Tiles naming a terrain, crop quality or mutation the game lacks: one problem per scene
    /// and missing id, at its first tile.
    let private tiles (context: Context) (sink: Sink) =
        context.Project.Scenes
        |> List.iteri (fun s scene ->
            let reported = HashSet<string>()
            scene.Tiles
            |> List.iteri (fun y row ->
                row
                |> List.iteri (fun x tile ->
                    let report (code: string) (what: string) (id: string) =
                        if reported.Add(code + "\u0000" + id) then
                            sink.Error(
                                code,
                                sprintf "scenes[%d].tiles[%d][%d]" s y x,
                                sprintf "Scene \"%s\" has tiles of missing %s \"%s\" (first at %d,%d)" scene.Name what id x y,
                                Some(NavigationTarget.Scene(scene.Id, x, y)))
                    let terrain (id: string) = if id.Length > 0 && not (context.TerrainIds.Contains id) then report "scene.unknownTerrain" "terrain" id
                    terrain tile.Type
                    terrain tile.Background
                    tile.Overlay |> Option.iter terrain
                    tile.Object |> Option.iter terrain
                    match tile.Crop with
                    | Some crop ->
                        if crop.Quality.Length > 0 && not (context.QualityIds.Contains crop.Quality) then
                            report "scene.unknownQuality" "crop quality" crop.Quality
                        match crop.Mutation with
                        | Some mutation when mutation.Length > 0 && not (context.MutationIds.Contains mutation) ->
                            report "scene.unknownMutation" "crop mutation" mutation
                        | _ -> ()
                    | None -> ())))

    let private items (context: Context) (sink: Sink) =
        let p = context.Project
        p.Items
        |> List.iteri (fun i item ->
            let path = sprintf "items[%d]" i
            let target = Some(NavigationTarget.Item item.Id)
            if item.Type.Length > 0 && not (context.CategoryIds.Contains item.Type) then
                sink.Error("item.unknownCategory", path + ".type", sprintf "Item \"%s\" is of missing kind \"%s\"" item.Name item.Type, target)
            match item.ToolType with
            | Some tool when tool.Length > 0 && not (context.ToolIds.Contains tool) ->
                sink.Error("item.unknownTool", path + ".toolType", sprintf "Item \"%s\" is missing tool \"%s\"" item.Name tool, target)
            | _ -> ())
        p.NodeTypes
        |> List.iteri (fun i node ->
            if node.RequiredTool.Length > 0 && not (context.ToolIds.Contains node.RequiredTool) then
                sink.Error(
                    "nodeType.unknownTool",
                    sprintf "nodeTypes[%d].requiredTool" i,
                    sprintf "Node type \"%s\" needs missing tool \"%s\"" node.Name node.RequiredTool,
                    Some(NavigationTarget.NodeType node.Id)))

    let private terrains (context: Context) (sink: Sink) =
        defaultArg context.Project.Terrains []
        |> List.iteri (fun i terrain ->
            let path = sprintf "terrains[%d]" i
            let target = Some(NavigationTarget.Terrain terrain.Id)
            if String.IsNullOrWhiteSpace terrain.Name then
                sink.Warning("terrain.emptyName", path + ".name", sprintf "Terrain \"%s\" has no name" terrain.Id, target)
            terrain.Transforms
            |> List.iteri (fun k transform ->
                let tp = sprintf "%s.transforms[%d]" path k
                if transform.Into.Length > 0 && not (context.TerrainIds.Contains transform.Into) then
                    sink.Error("terrain.unknownInto", tp + ".into", sprintf "Terrain \"%s\" turns into missing terrain \"%s\"" terrain.Name transform.Into, target)
                if transform.Into.Length = 0 && terrain.Layer = TileLayers.Background then
                    sink.Error("terrain.removesBackground", tp + ".into", sprintf "Terrain \"%s\" is ground and can only change into another terrain, not be removed" terrain.Name, target)
                match transform.Tool with
                | Some tool when tool.Length > 0 && not (context.ToolIds.Contains tool) ->
                    sink.Error("terrain.unknownTool", tp + ".tool", sprintf "Terrain \"%s\" is changed by missing tool \"%s\"" terrain.Name tool, target)
                | _ -> ()))

    let private tools (context: Context) (sink: Sink) =
        let all = ContentCompiler.tools context.Project
        let keys = Dictionary<string, string>()
        for tool in all do
            match tool.Key with
            | Some key when key.Length > 0 ->
                let key = key.ToLowerInvariant()
                match keys.TryGetValue key with
                | true, other ->
                    sink.Warning("tool.sharedKey", "tools", sprintf "Tools \"%s\" and \"%s\" share the key \"%s\"; both are used when it is pressed" other tool.Name key, Some(NavigationTarget.Tool tool.Id))
                | _ -> keys.[key] <- tool.Name
            | _ -> ()
        defaultArg context.Project.Tools []
        |> List.iteri (fun i tool ->
            let path = sprintf "tools[%d]" i
            let target = Some(NavigationTarget.Tool tool.Id)
            if String.IsNullOrWhiteSpace tool.Name then
                sink.Warning("tool.emptyName", path + ".name", sprintf "Tool \"%s\" has no name" tool.Id, target)
            match tool.Key with
            | Some key when reservedKeys.Contains(key.ToLowerInvariant()) ->
                sink.Error("tool.reservedKey", path + ".key", sprintf "Tool \"%s\" uses the key \"%s\", which moves the player or opens a menu" tool.Name key, target)
            | _ -> ()
            if tool.Action = ToolActions.Place then
                match tool.PlaceTerrain with
                | Some terrain when context.TerrainIds.Contains terrain -> ()
                | Some terrain when terrain.Length > 0 ->
                    sink.Error("tool.unknownTerrain", path + ".placeTerrain", sprintf "Tool \"%s\" places missing terrain \"%s\"" tool.Name terrain, target)
                | _ -> sink.Error("tool.noTerrain", path + ".placeTerrain", sprintf "Tool \"%s\" places terrain but does not say which" tool.Name, target)
            match tool.Skill with
            | Some skill when skill.Length > 0 && not (List.contains skill SaveSchema.SkillNames) ->
                sink.Warning("tool.unknownSkill", path + ".skill", sprintf "Tool \"%s\" trains \"%s\", which is not a skill" tool.Name skill, target)
            | _ -> ()
            if tool.Area.IsEmpty && tool.Action <> ToolActions.Fish then
                sink.Info("tool.noArea", path + ".area", sprintf "Tool \"%s\" has no area; it works on the faced tile" tool.Name, target))

    let private qualities (context: Context) (sink: Sink) =
        let mutations = ContentCompiler.cropMutations context.Project
        let shares = mutations |> List.sumBy (fun m -> m.Share)
        if shares > 1.0 + 1e-9 then
            sink.Warning(
                "cropMutation.sharesAboveOne",
                "cropMutations",
                sprintf "The crop mutations' shares add up to %g; mutations past a total of 1 never happen" shares,
                mutations |> List.tryLast |> Option.map (fun m -> NavigationTarget.CropMutation m.Id))
        if ContentCompiler.cropQualities context.Project |> List.isEmpty then
            sink.Error("cropQuality.none", "cropQualities", "The game has no crop quality tiers", None)

    /// Growth requirements of the project's crops (M1).
    let private requirements (context: Context) (sink: Sink) =
        let p = context.Project
        let terrains = ContentCompiler.terrains p
        let sceneTags = HashSet<string>(p.Scenes |> Seq.collect (fun s -> defaultArg s.Tags []))
        defaultArg p.CustomCrops []
        |> List.iteri (fun i crop ->
            match crop.Requirements with
            | None -> ()
            | Some needs ->
                let path = sprintf "customCrops[%d].requirements" i
                let target = Some(NavigationTarget.Crop crop.Id)
                for terrain in needs.Terrains do
                    match terrains |> List.tryFind (fun t -> t.Id = terrain) with
                    | None -> sink.Error("crop.unknownTerrain", path + ".terrains", sprintf "Crop \"%s\" grows on missing terrain \"%s\"" crop.Name terrain, target)
                    | Some t when not t.Plantable ->
                        sink.Warning("crop.unplantableTerrain", path + ".terrains", sprintf "Crop \"%s\" grows on \"%s\", where nothing can be planted" crop.Name t.Name, target)
                    | _ -> ()
                for fertilizer in needs.Fertilizers do
                    if not (context.ItemIds.Contains fertilizer) then
                        sink.Error("crop.unknownFertilizer", path + ".fertilizers", sprintf "Crop \"%s\" needs missing fertilizer \"%s\"" crop.Name fertilizer, target)
                for tag in needs.SceneTags do
                    if not (sceneTags.Contains tag) then
                        sink.Warning("crop.unknownSceneTag", path + ".sceneTags", sprintf "Crop \"%s\" needs scene tag \"%s\", which no scene has" crop.Name tag, target)
                needs.Neighbors
                |> List.iteri (fun k need ->
                    let np = sprintf "%s.neighbors[%d].id" path k
                    match need.Kind, need.Id with
                    | kind, Some id when kind = NeighborKinds.Crop && not (context.CropIds.Contains id) ->
                        sink.Error("crop.unknownNeighbor", np, sprintf "Crop \"%s\" needs neighbours of missing crop \"%s\"" crop.Name id, target)
                    | kind, Some id when kind = NeighborKinds.Terrain && not (context.TerrainIds.Contains id) ->
                        sink.Error("crop.unknownNeighbor", np, sprintf "Crop \"%s\" needs neighbours of missing terrain \"%s\"" crop.Name id, target)
                    | kind, id when kind <> NeighborKinds.Water && not (hasValue id) ->
                        sink.Error("crop.neighborWithoutId", np, sprintf "Crop \"%s\" needs %s neighbours but does not say which" crop.Name kind, target)
                    | _ -> ()))

    let private overlays (context: Context) (sink: Sink) =
        let p = context.Project
        let known = HashSet<string>([ OverlayStyles.Rain; OverlayStyles.Snow ] @ (defaultArg p.Weather.Overlays [] |> List.map (fun o -> o.Id)))
        p.Weather.Types
        |> List.iteri (fun w weather ->
            match weather.Overlay with
            | Some overlay when overlay.Length > 0 && not (known.Contains overlay) ->
                sink.Warning("weather.unknownOverlay", sprintf "weather.types[%d].overlay" w, sprintf "Weather \"%s\" shows missing overlay \"%s\" (the game guesses one from the weather's id)" weather.Name overlay, Some NavigationTarget.Settings)
            | _ -> ())

    /// Music picked by scenes, seasons, festivals and weather (M2) must be music assets.
    let private music (context: Context) (sink: Sink) =
        let p = context.Project
        let check (path: string) (owner: string) (music: string option) (target: NavigationTarget option) =
            match music with
            | Some id when id.Length > 0 && not (context.MusicIds.Contains id) ->
                let what = if context.SoundIds.Contains id then "a sound effect, not music" else "missing"
                sink.Error("music.unknown", path, sprintf "%s plays music \"%s\", which is %s" owner id what, target)
            | _ -> ()
        p.Scenes |> List.iteri (fun s scene -> check (sprintf "scenes[%d].music" s) (sprintf "Scene \"%s\"" scene.Name) scene.Music (Some(NavigationTarget.Scene(scene.Id, 0, 0))))
        p.Settings.Calendar.Seasons
        |> List.iteri (fun s season -> check (sprintf "settings.calendar.seasons[%d].music" s) (sprintf "Season \"%s\"" season.Name) season.Music (Some NavigationTarget.Settings))
        p.Settings.Calendar.Festivals
        |> List.iteri (fun f festival -> check (sprintf "settings.calendar.festivals[%d].music" f) (sprintf "Festival \"%s\"" festival.Name) festival.Music (Some NavigationTarget.Settings))
        p.Weather.Types
        |> List.iteri (fun w weather -> check (sprintf "weather.types[%d].music" w) (sprintf "Weather \"%s\"" weather.Name) weather.Music (Some NavigationTarget.Settings))
        p.CustomAssets
        |> List.iteri (fun a asset ->
            if asset.Type = CustomAssetTypes.Sound || asset.Type = CustomAssetTypes.Music then
                let url = asset.DataUrl.ToLowerInvariant()
                let audio = [ "data:audio/ogg"; "data:audio/vorbis"; "data:audio/wav"; "data:audio/x-wav"; "data:audio/wave"; "data:audio/vnd.wave" ] |> List.exists url.StartsWith
                if not audio then
                    sink.Error("asset.notAudio", sprintf "customAssets[%d].dataUrl" a, sprintf "%s \"%s\" is not an OGG or WAV file" (if asset.Type = CustomAssetTypes.Music then "Music" else "Sound") asset.Name, Some(NavigationTarget.Asset asset.Id)))

    /// `playMusic` and `startCutscene` outcomes (M2), wherever outcomes are.
    let private outcomeChecks (context: Context) (sink: Sink) (path: string) (label: string) (target: NavigationTarget option) (outcomes: EventOutcome list) =
        outcomes
        |> List.iteri (fun o outcome ->
            let opath = sprintf "%s.outcomes[%d]" path o
            match outcome.Type with
            | EventOutcomeTypes.PlayMusic ->
                match outcome.SoundId with
                | Some id when id.Length > 0 && not (context.MusicIds.Contains id) ->
                    sink.Error("outcome.unknownMusic", opath + ".soundId", sprintf "%s plays missing music \"%s\"" label id, target)
                | _ -> ()
            | EventOutcomeTypes.StartCutscene ->
                match outcome.CutsceneId with
                | Some id when context.CutsceneIds.Contains id -> ()
                | Some id when id.Length > 0 ->
                    sink.Error("outcome.unknownCutscene", opath + ".cutsceneId", sprintf "%s plays missing cutscene \"%s\"" label id, target)
                | _ -> sink.Error("outcome.noCutscene", opath + ".cutsceneId", sprintf "%s has a startCutscene outcome with no cutscene" label, target)
            | EventOutcomeTypes.ChangeTile ->
                match outcome.NewTileType with
                | Some terrain when terrain.Length > 0 && not (context.TerrainIds.Contains terrain) ->
                    sink.Error("outcome.unknownTerrain", opath + ".newTileType", sprintf "%s changes a tile to missing terrain \"%s\"" label terrain, target)
                | _ -> ()
            | _ -> ())

    let private allOutcomes (context: Context) (sink: Sink) =
        let p = context.Project
        p.Events |> List.iteri (fun e event -> outcomeChecks context sink (sprintf "events[%d]" e) (sprintf "Event \"%s\"" event.Name) (Some(NavigationTarget.Event event.Id)) event.Outcomes)
        p.Actions |> List.iteri (fun a action -> outcomeChecks context sink (sprintf "actions[%d]" a) (sprintf "Action \"%s\"" action.Name) (Some(NavigationTarget.Action action.Id)) action.Outcomes)
        p.Minigames
        |> List.iteri (fun m minigame ->
            minigame.ResultTiers
            |> List.iteri (fun t tier ->
                outcomeChecks context sink (sprintf "minigames[%d].resultTiers[%d]" m t) (sprintf "Minigame \"%s\"" minigame.Name) (Some(NavigationTarget.Minigame minigame.Id)) tier.Outcomes))

    let private cutscenes (context: Context) (sink: Sink) =
        defaultArg context.Project.Cutscenes []
        |> List.iteri (fun c cutscene ->
            let path = sprintf "cutscenes[%d]" c
            let target = Some(NavigationTarget.Cutscene cutscene.Id)
            let label = sprintf "Cutscene \"%s\"" cutscene.Name
            if cutscene.Steps.IsEmpty then
                sink.Warning("cutscene.empty", path + ".steps", sprintf "%s has no steps" label, target)
            let labels = HashSet<string>()
            cutscene.Steps
            |> List.iteri (fun k step ->
                match step.Label with
                | Some l when l.Length > 0 && not (labels.Add l) ->
                    sink.Error("cutscene.duplicateLabel", sprintf "%s.steps[%d].label" path k, sprintf "%s has two steps labelled \"%s\"" label l, target)
                | _ -> ())
            let actor (sp: string) (field: string) (id: string option) =
                match id with
                | Some id when id.Length > 0 && id <> "player" && not (context.NpcIds.Contains id) ->
                    sink.Error("cutscene.unknownActor", sp + "." + field, sprintf "%s uses missing NPC \"%s\"" label id, target)
                | _ -> ()
            let goto (sp: string) (field: string) (id: string) =
                if id.Length > 0 && not (labels.Contains id) then
                    sink.Error("cutscene.unknownLabel", sp + "." + field, sprintf "%s goes to missing step label \"%s\"" label id, target)
            cutscene.Steps
            |> List.iteri (fun k step ->
                let sp = sprintf "%s.steps[%d]" path k
                let needs (field: string) (present: bool) =
                    if not present then
                        sink.Error("cutscene.missingField", sp + "." + field, sprintf "%s has a %s step with no %s" label step.Type field, target)
                actor sp "actor" step.Actor
                match step.Type with
                | CutsceneStepTypes.Say ->
                    needs "text" (hasValue step.Text)
                    actor sp "speaker" step.Speaker
                | CutsceneStepTypes.Choice ->
                    if step.Options.IsEmpty then needs "options" false
                    step.Options |> List.iteri (fun o option -> goto sp (sprintf "options[%d].goto" o) option.Goto)
                | CutsceneStepTypes.Jump ->
                    needs "goto" (hasValue step.Goto)
                    goto sp "goto" (text step.Goto)
                | CutsceneStepTypes.Move ->
                    needs "actor" (hasValue step.Actor)
                    needs "x" step.X.IsSome
                    needs "y" step.Y.IsSome
                | CutsceneStepTypes.Face ->
                    needs "actor" (hasValue step.Actor)
                    needs "direction" (hasValue step.Direction)
                | CutsceneStepTypes.Emote ->
                    needs "actor" (hasValue step.Actor)
                    needs "emote" (hasValue step.Emote)
                | CutsceneStepTypes.Fade -> needs "mode" (hasValue step.Mode)
                | CutsceneStepTypes.Music ->
                    match step.SoundId with
                    | Some id when id.Length > 0 && not (context.MusicIds.Contains id) ->
                        sink.Error("cutscene.unknownMusic", sp + ".soundId", sprintf "%s plays missing music \"%s\"" label id, target)
                    | _ -> ()
                | CutsceneStepTypes.Sound -> needs "soundId" (hasValue step.SoundId)
                | CutsceneStepTypes.Outcomes -> outcomeChecks context sink sp label target step.Outcomes
                | _ -> ()))

    let run (context: Context) (sink: Sink) =
        duplicates context sink
        tiles context sink
        items context sink
        terrains context sink
        tools context sink
        qualities context sink
        requirements context sink
        overlays context sink
        music context sink
        allOutcomes context sink
        cutscenes context sink
