namespace FarmEngine.Authoring

open FarmEngine.Schemas

/// Checks of the tooling around a shipped game: test states that name content the game lacks
/// (#229) and save migration rules, including ids the last export had that the game lost without
/// a rule (#242). Nothing here runs for a project without test states, rules or an export record.
module internal ChecksUpkeep =
    let private testStates (content: GameContent) (project: GameProject) (sink: Sink) =
        let items = content.Items |> List.map (fun i -> i.Id) |> Set.ofList
        let quests = content.Quests |> List.map (fun q -> q.Id) |> Set.ofList
        let npcs = content.Npcs |> List.map (fun n -> n.Id) |> Set.ofList

        let seasons =
            content.Settings.Calendar.Seasons |> List.map (fun s -> s.Id) |> Set.ofList

        let seen = System.Collections.Generic.HashSet<string>()

        TestStates.all project
        |> List.iteri (fun index state ->
            let path = sprintf "testStates[%d]" index
            let label = if state.Name = "" then state.Id else state.Name

            let warn code (member': string) message =
                sink.Warning("testState." + code, path + member', sprintf "Test state '%s': %s" label message, Some NavigationTarget.Settings)

            if state.Id = "" then
                warn "id" ".id" "it needs an id."
            elif not (seen.Add state.Id) then
                warn "duplicate" ".id" (sprintf "another test state has the id '%s'." state.Id)

            match state.SceneId with
            | Some sceneId when sceneId <> "" ->
                match content.Scenes |> List.tryFind (fun s -> s.Id = sceneId) with
                | None -> warn "scene" ".sceneId" (sprintf "the game has no scene '%s'." sceneId)
                | Some scene ->
                    let outside (v: float option) (size: float) =
                        v |> Option.exists (fun v -> v < 0.0 || v >= size)

                    if outside state.X scene.Width || outside state.Y scene.Height then
                        warn
                            "tile"
                            ".x"
                            (sprintf
                                "tile (%g, %g) is outside scene '%s' (%g×%g)."
                                (defaultArg state.X 0.0)
                                (defaultArg state.Y 0.0)
                                sceneId
                                scene.Width
                                scene.Height)
            | _ -> ()

            match state.Season with
            | Some season when not (seasons.Contains season) -> warn "season" ".season" (sprintf "the calendar has no season '%s'." season)
            | _ -> ()

            match state.Facing with
            | Some facing when not (List.contains facing [ Directions.Up; Directions.Down; Directions.Left; Directions.Right ]) ->
                warn "facing" ".facing" (sprintf "facing '%s' is not up, down, left or right." facing)
            | _ -> ()

            for questId, status in state.Quests do
                if not (quests.Contains questId) then
                    warn "quest" (ProblemPath.memberPath ".quests" questId) (sprintf "the game has no quest '%s'." questId)
                elif
                    not (
                        List.contains
                            status
                            [
                                QuestStatuses.NotStarted
                                QuestStatuses.Active
                                QuestStatuses.Completed
                                QuestStatuses.Failed
                            ]
                    )
                then
                    warn
                        "questStatus"
                        (ProblemPath.memberPath ".quests" questId)
                        (sprintf "quest status '%s' is not not-started, active, completed or failed." status)

            for npcId, _ in state.Friendship do
                if not (npcs.Contains npcId) then
                    warn "npc" (ProblemPath.memberPath ".friendship" npcId) (sprintf "the game has no NPC '%s'." npcId)

            defaultArg state.Inventory []
            |> List.iteri (fun i line ->
                if not (items.Contains line.ItemId) then
                    warn "item" (sprintf ".inventory[%d].itemId" i) (sprintf "the game has no item '%s'." line.ItemId)))

    let private migrations (content: GameContent) (project: GameProject) (sink: Sink) =
        let items = content.Items |> List.map (fun i -> i.Id) |> Set.ofList
        let scenes = content.Scenes |> List.map (fun s -> s.Id) |> Set.ofList
        let seen = System.Collections.Generic.HashSet<string * string>()

        SaveMigrations.rules project
        |> List.iteri (fun index rule ->
            let path = sprintf "saveMigrations[%d]" index

            let add severity code (member': string) message =
                sink.Add(severity, "saveMigration." + code, path + member', message, Some NavigationTarget.Settings)

            let warn = add Severity.Warning

            let current =
                if rule.Kind = SaveMigrationKinds.Scene then
                    scenes
                else
                    items

            if not (List.contains rule.Kind SaveMigrationKinds.All) then
                warn "kind" ".kind" (sprintf "Save migration kind '%s' is not item or scene; the player ignores the rule." rule.Kind)
            elif rule.From = "" then
                warn "from" ".from" "A save migration needs the old id it renames."
            elif not (seen.Add(rule.Kind, rule.From)) then
                warn "duplicate" ".from" (sprintf "Another save migration already handles %s '%s'; only the first one changes saves." rule.Kind rule.From)
            else
                if current.Contains rule.From then
                    add
                        Severity.Info
                        "inactive"
                        ".from"
                        (sprintf "The game still has %s '%s', so this save migration does nothing until it is removed." rule.Kind rule.From)

                match rule.To with
                | Some target when not (current.Contains target) ->
                    warn
                        "to"
                        ".to"
                        (sprintf "Save migration of '%s' leads to %s '%s', which the game does not have; saves keep the old id." rule.From rule.Kind target)
                | Some target when target = rule.From -> warn "to" ".to" (sprintf "Save migration of '%s' leads to itself." rule.From)
                | _ -> ()

                if rule.Quantity |> Option.exists (fun q -> q < 1.0 || q <> floor q) then
                    warn "quantity" ".quantity" "A replacement quantity is a whole number from 1."

                if rule.Money |> Option.exists (fun m -> m < 0.0) then
                    warn "money" ".money" "The money paid for a removed item cannot be negative."

                if
                    rule.Kind = SaveMigrationKinds.Scene
                    && (rule.Quantity.IsSome || rule.Money.IsSome)
                then
                    warn "sceneFields" ".quantity" "A scene migration has no quantity or money.")

        match SaveMigrations.exported project with
        | None -> ()
        | Some ids ->
            let version =
                if ids.Version = "" then
                    "the last export"
                else
                    "version " + ids.Version

            for kind, id in SaveMigrations.orphans content project do
                let noun = if kind = SaveMigrationKinds.Scene then "Scene" else "Item"

                let consequence =
                    if kind = SaveMigrationKinds.Scene then
                        "saves made there keep a scene the game no longer has"
                    else
                        "saves holding it will set it aside"

                sink.Warning(
                    "saveMigration.orphan",
                    "saveMigrations",
                    sprintf "%s '%s' was in %s and is gone now: %s. Add a save migration that renames, replaces or removes it." noun id version consequence,
                    Some NavigationTarget.Settings
                )

    let run (project: GameProject) (sink: Sink) =
        let hasStates = not (List.isEmpty (TestStates.all project))
        let hasRules = not (List.isEmpty (SaveMigrations.rules project))

        if hasStates || hasRules || (SaveMigrations.exported project).IsSome then
            let content = ContentCompiler.compile project
            testStates content project sink
            migrations content project sink
