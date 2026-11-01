namespace FarmEngine.Authoring

open FarmEngine.Schemas

/// The whole-game checks over the content graph (#231): items nothing gives (and, as info, items
/// nothing uses), quest prerequisites that loop or wait on a quest nothing starts, recipes and
/// quests gated behind an item the game never gives, and season mismatches. Plugins can give
/// items and start quests in ways the graph cannot see, so with pack plugins installed every
/// graph finding is info.
module internal ChecksGraph =
    let private needingKinds =
        set [ "ingredient"; "objective"; "bundle"; "building"; "upgrade"; "collection" ]

    let private seasonList (seasons: string list) = String.concat ", " seasons

    let run (project: GameProject) (sink: Sink) =
        let content = ContentCompiler.compile project
        let graph = ContentGraph.build project content

        let pluginsCanGive =
            project.ContentPacks
            |> List.exists (fun install -> install.Enabled && not (List.isEmpty install.Pack.Plugins))

        let sure = if pluginsCanGive then Severity.Info else Severity.Warning
        let itemIndex = project.Items |> List.mapi (fun i item -> item.Id, i) |> Map.ofList
        let questIndex = project.Quests |> List.mapi (fun i q -> q.Id, i) |> Map.ofList
        let recipeIndex = project.Recipes |> List.mapi (fun i r -> r.Id, i) |> Map.ofList

        let itemPath id =
            match Map.tryFind id itemIndex with
            | Some i -> sprintf "items[%d]" i
            | None -> "items"

        let questPath id =
            match Map.tryFind id questIndex with
            | Some i -> sprintf "quests[%d]" i
            | None -> "quests"

        let entryOf kind id = ContentGraph.tryFind kind id graph
        let obtainable kind id = graph.Obtainable.Contains(kind, id)

        let hasSources kind id =
            entryOf kind id |> Option.exists (fun e -> not (List.isEmpty e.Sources))

        // Items.
        for item in content.Items do
            let entry = entryOf GraphKinds.Item item.Id
            let sources = entry |> Option.map (fun e -> e.Sources) |> Option.defaultValue []
            let uses = entry |> Option.map (fun e -> e.Uses) |> Option.defaultValue []
            let needed = uses |> List.exists (fun u -> needingKinds.Contains u.Via)
            let target = Some(NavigationTarget.Item item.Id)

            if List.isEmpty sources then
                let severity = if needed then sure else Severity.Info

                let why =
                    match uses |> List.tryFind (fun u -> needingKinds.Contains u.Via) with
                    | Some u -> sprintf " yet it is needed (%s)" (u.Label.ToLowerInvariant())
                    | None -> ""

                sink.Add(
                    severity,
                    "graph.itemNoSource",
                    itemPath item.Id,
                    sprintf "Nothing in the game gives \"%s\" (no shop, crop, recipe, drop, reward or event)%s." item.Name why,
                    target
                )
            elif not (obtainable GraphKinds.Item item.Id) then
                let blocked =
                    sources
                    |> List.collect (fun s -> s.Needs |> List.filter (graph.Obtainable.Contains >> not))
                    |> List.distinct
                    |> List.map (fun (kind, id) -> sprintf "%s \"%s\"" kind id)

                sink.Info(
                    "graph.itemUnobtainable",
                    itemPath item.Id,
                    sprintf "\"%s\" can never be had: every way to get it needs %s, which the game never gives." item.Name (String.concat " or " blocked),
                    target
                )

            if List.isEmpty uses && item.Value <= 0.0 && item.Type <> ItemTypes.Quest then
                sink.Info(
                    "graph.itemNoUse",
                    itemPath item.Id,
                    sprintf "Nothing uses \"%s\" and it sells for nothing: no recipe, quest, gift, collection or bundle asks for it." item.Name,
                    target
                )

        // Recipes gated behind an item nothing gives, and ingredients of another season.
        for recipe in content.Recipes do
            let path =
                match Map.tryFind recipe.Id recipeIndex with
                | Some i -> sprintf "recipes[%d]" i
                | None -> "recipes"

            let target = Some(NavigationTarget.Recipe recipe.Id)

            for input in recipe.Inputs do
                if not (hasSources GraphKinds.Item input.ItemId) then
                    // Reported once on the item (graph.itemNoSource); here as what it blocks.
                    sink.Add(
                        sure,
                        "graph.recipeGated",
                        path + ".inputs",
                        sprintf "Recipe \"%s\" can never be made: nothing gives its ingredient \"%s\"." recipe.Name input.ItemId,
                        target
                    )

            match recipe.Unlock |> Option.bind (fun u -> u.Seasons) with
            | Some allowed when not (List.isEmpty allowed) ->
                for input in recipe.Inputs do
                    match
                        entryOf GraphKinds.Item input.ItemId
                        |> Option.bind (ContentGraph.seasonsOf graph)
                    with
                    | Some seasons when seasons |> List.forall (fun s -> not (List.contains s allowed)) ->
                        sink.Info(
                            "graph.seasonal",
                            path + ".unlock.seasons",
                            sprintf
                                "Recipe \"%s\" is made in %s, but \"%s\" only comes in %s: players must keep it from an earlier season (the first year may not manage)."
                                recipe.Name
                                (seasonList allowed)
                                input.ItemId
                                (seasonList seasons),
                            target
                        )
                    | _ -> ()
            | _ -> ()

        // Quests.
        for cycle in ContentGraph.prerequisiteCycles content do
            match cycle with
            | first :: _ when cycle.Length > 1 ->
                sink.Warning(
                    "graph.questCycle",
                    questPath first + ".prerequisites",
                    sprintf "Quest prerequisites go round in a circle (%s), so none of these quests can start." (String.concat " → " (cycle @ [ first ])),
                    Some(NavigationTarget.Quest first)
                )
            | _ -> ()

        for quest in content.Quests do
            let target = Some(NavigationTarget.Quest quest.Id)

            if not (hasSources GraphKinds.Quest quest.Id) then
                sink.Info(
                    "graph.questNeverStarts",
                    questPath quest.Id,
                    sprintf "Nothing starts quest \"%s\": it does not start on its own and no conversation, event or board offers it." quest.Name,
                    target
                )

            for prerequisite in defaultArg quest.Prerequisites [] do
                if
                    prerequisite <> quest.Id
                    && not (hasSources GraphKinds.Quest prerequisite)
                    && content.Quests |> List.exists (fun q -> q.Id = prerequisite)
                then
                    sink.Add(
                        sure,
                        "graph.questDeadEnd",
                        questPath quest.Id + ".prerequisites",
                        sprintf "Quest \"%s\" waits for \"%s\", which nothing starts, so it can never start." quest.Name prerequisite,
                        target
                    )

            for objective in quest.Objectives do
                match objective.TargetItemId with
                | Some item when
                    content.Items |> List.exists (fun i -> i.Id = item)
                    && not (hasSources GraphKinds.Item item)
                    ->
                    sink.Add(
                        sure,
                        "graph.questGated",
                        questPath quest.Id + ".objectives",
                        sprintf "Quest \"%s\" asks for \"%s\", which nothing in the game gives." quest.Name item,
                        target
                    )
                | Some item ->
                    match quest.AvailableSeasons with
                    | Some allowed when not (List.isEmpty allowed) ->
                        match entryOf GraphKinds.Item item |> Option.bind (ContentGraph.seasonsOf graph) with
                        | Some seasons when seasons |> List.forall (fun s -> not (List.contains s allowed)) ->
                            sink.Info(
                                "graph.seasonal",
                                questPath quest.Id + ".availableSeasons",
                                sprintf
                                    "Quest \"%s\" runs in %s, but \"%s\" only comes in %s: players must keep it from an earlier season."
                                    quest.Name
                                    (seasonList allowed)
                                    item
                                    (seasonList seasons),
                                target
                            )
                        | _ -> ()
                    | _ -> ()
                | None -> ()
