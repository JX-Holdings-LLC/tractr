namespace FarmEngine.Authoring

open FarmEngine.Schemas

/// Keep changes: a playtest's final game state written back into the project (TS
/// `applyStateToProject`, which the Rust player also implements for project-started players).
/// The editor's Play Mode runs a compiled cartridge, so the state comes back as the engine's
/// `GameState` JSON and F# applies it (docs/dev/ARCHITECTURE.md, `State.cs ApplyStateToProject`).
module Playtest =
    let private objectKey (key: string) (value: Json) : Json option =
        match Json.tryGet key value with
        | Some(JObject _ as found) -> Some found
        | _ -> None

    /// Sets each `(key, value)` in order (a missing value removes the key).
    let private setAll (pairs: (string * Json option) list) (target: Json) : Json =
        pairs
        |> List.fold
            (fun acc (key, value) ->
                match value with
                | Some v -> Json.set key v acc
                | None -> Json.remove key acc)
            target

    let private npc (npcs: Json) (entry: Json) : Json =
        match Json.asString (Json.get "id" entry) |> Option.bind (fun id -> objectKey id npcs) with
        | Some state ->
            entry
            |> setAll
                [
                    "x", Some(Json.get "x" state)
                    "y", Some(Json.get "y" state)
                    "sceneId", Some(Json.get "sceneId" state)
                ]
        | None -> entry

    let private quest (quests: Json) (entry: Json) : Json =
        match
            Json.asString (Json.get "id" entry)
            |> Option.bind (fun id -> objectKey id quests)
        with
        | None -> entry
        | Some progress ->
            let withStatus = Json.set "status" (Json.get "status" progress) entry

            match objectKey "objectives" progress, Json.get "objectives" withStatus with
            | Some objectives, JArray items ->
                let objective (item: Json) =
                    match
                        Json.asString (Json.get "id" item)
                        |> Option.bind (fun id -> objectKey id objectives)
                    with
                    | Some state ->
                        item
                        |> setAll
                            [
                                "progress", Some(Json.get "progress" state)
                                "completed", Some(Json.get "completed" state)
                            ]
                    | None -> item

                Json.set "objectives" (JArray(List.map objective items)) withStatus
            | _ -> withStatus

    let private mapArray (key: string) (f: Json -> Json) (value: Json) : Json =
        match Json.get key value with
        | JArray items -> Json.set key (JArray(List.map f items)) value
        | _ -> value

    /// The calendar the game ran (Rust `resolve_settings(&project.settings).calendar`).
    let private calendar (project: Json) : CalendarConfig =
        match Decode.run SchemaJson.decodeProjectSettings (Json.get "settings" project) with
        | Ok settings ->
            (ContentCompiler.settings
                { GameProject.Default with
                    Settings = settings
                })
                .Calendar
        | Error _ -> SettingsSchema.DefaultProjectSettings.Calendar

    /// `currentDayOfSeason` for the clock (Rust `kept_day_of_season`): absent when `currentDay`
    /// alone lands on the clock's season and day of season.
    let private dayOfSeason (calendar: CalendarConfig) (clock: Json) : Json option =
        let number =
            function
            | JNumber n -> Some n
            | _ -> None

        let day = number (Json.get "day" clock) |> Option.defaultValue 0.0
        let season = Json.asString (Json.get "season" clock) |> Option.defaultValue ""

        let kept =
            SettingsSchema.clockDayOfSeason calendar season day (number (Json.get "dayOfSeason" clock))

        let natural, naturalDay = SettingsSchema.naturalDate calendar day

        if natural.Id <> season || naturalDay <> kept then
            Some(JNumber kept)
        else
            None

    /// What the project's own fields do not hold (Rust `KeptState::of_state`): the tick, today's
    /// shop purchases, an open dialogue, shop or minigame, a cutscene that is playing, NPC walks,
    /// the mine floor, children, changed shop stock and the M5–M7 goals; absent when there is
    /// nothing to carry.
    let private keptState (state: Json) : Json option =
        let present (key: string) (value: Json) =
            if Json.isNullish value then [] else [ key, value ]

        let npcs =
            match Json.get "npcs" state with
            | JObject members ->
                members
                |> List.choose (fun (id, npc) ->
                    match
                        present "path" (Json.get "path" npc)
                        @ present "patrolIndex" (Json.get "patrolIndex" npc)
                        @ present "scheduleSelection" (Json.get "scheduleSelection" npc)
                    with
                    | [] -> None
                    | walk -> Some(id, JObject walk))
            | _ -> []

        let purchases =
            match Json.get "shopPurchasesToday" state with
            | JObject _ as purchases -> purchases
            | _ -> JObject []

        let tick = Json.get "tick" (Json.get "clock" state) |> Json.orElse (JNumber 0.0)

        let floor =
            Json.get "currentFloor" (Json.get "mine" state) |> Json.orElse (JNumber 0.0)

        let sessions =
            [ "dialogue"; "shop"; "minigame"; "cutscene"; "music"; "festival" ]
            |> List.collect (fun key -> present key (Json.get key state))
        // The player's children and the shops' changed stock (M3, M4), when there are any.
        let family =
            [
                for key in [ "shopSales"; "festivalVisits"; "pluginData" ] do
                    match Json.get key state with
                    | JObject(_ :: _) as value -> key, value
                    | _ -> ()
                match Json.get "fishCatches" state with
                | JArray(_ :: _) as catches -> "fishCatches", catches
                | _ -> ()
                match Json.get "children" state with
                | JArray(_ :: _) as children -> "children", children
                | _ -> ()
                match Json.get "shopChanges" state with
                | JObject(_ :: _) as changes -> "shopChanges", changes
                | _ -> ()
            ]

        let questFields field =
            match Json.get "quests" state with
            | JObject quests ->
                [
                    for id, quest in quests do
                        match Json.tryGet field quest with
                        | Some value when not (Json.isNullish value) -> yield id, value
                        | _ -> ()
                ]
            | _ -> []

        let requestsAndMines =
            [
                for key in [ "mines"; "requestBoards" ] do
                    match Json.get key state with
                    | JObject(_ :: _) as value -> yield key, value
                    | _ -> ()
                yield! present "currentMineId" (Json.get "currentMineId" state)
                match questFields "deadlineDay" with
                | [] -> ()
                | values -> yield "questDeadlines", JObject values
                match questFields "request" with
                | [] -> ()
                | values -> yield "requests", JObject values
            ]
        // The goals and the world beyond the project's fields (M5–M7, Rust `KeptGoals::of_state`):
        // open storage, the farm layout, the shipping log, collections, bundles, achievements,
        // mail, monsters and the player's health, each when it holds something.
        let goals =
            [
                for key in
                    [
                        "storage"
                        "farmLayout"
                        "shipped"
                        "lastShipment"
                        "collections"
                        "bundles"
                        "achievements"
                        "mailbox"
                        "mailQueue"
                        "monsters"
                        "nextMonster"
                        "slain"
                    ] do
                    match Json.get key state with
                    | JObject(_ :: _)
                    | JArray(_ :: _)
                    | JString _ as value -> key, value
                    | JNumber n as value when n <> 0.0 -> key, value
                    | _ -> ()
                for key in [ "health"; "maxHealth"; "buffs"; "perks"; "purchasedUpgrades"; "toolUpgrades" ] do
                    match Json.get key (Json.get "player" state) with
                    | JNumber _ as value -> key, value
                    | JObject(_ :: _) as value -> key, value
                    | JArray(_ :: _) as value -> key, value
                    | _ -> ()
            ]

        let goals = if goals.IsEmpty then [] else [ "goals", JObject goals ]

        if
            tick = JNumber 0.0
            && purchases = JObject []
            && sessions.IsEmpty
            && npcs.IsEmpty
            && floor = JNumber 0.0
            && family.IsEmpty
            && requestsAndMines.IsEmpty
            && goals.IsEmpty
        then
            None
        else
            Some(
                JObject(
                    [ "tick", tick; "shopPurchasesToday", purchases ]
                    @ sessions
                    @ [ "npcs", JObject npcs; "mineCurrentFloor", floor ]
                    @ family
                    @ requestsAndMines
                    @ goals
                )
            )

    /// The project JSON with the state written back. Flag values go back as they are (plugins
    /// store numbers and strings), and keys keep their places.
    let applyStateJson (project: Json) (state: Json) : Json =
        let player = Json.get "player" state
        let clock = Json.get "clock" state

        let flags =
            match Json.get "flags" state with
            | JObject _ as flags -> flags
            | _ -> JObject []

        let projectPlayer =
            Json.get "player" project
            |> setAll (
                [
                    for key in
                        [
                            "x"
                            "y"
                            "direction"
                            "sceneId"
                            "inventory"
                            "maxInventorySize"
                            "money"
                            "energy"
                            "maxEnergy"
                            "skills"
                            "activeQuests"
                            "completedQuests"
                        ] -> key, Some(Json.get key player)
                ]
                @ [
                    "equippedTool", Json.tryGet "equippedTool" player |> Option.filter (Json.isNullish >> not)
                    // The player character (M4): an empty name and no clothes are absent.
                    "name",
                    Json.tryGet "name" player
                    |> Option.filter (fun name -> name <> JString "" && not (Json.isNullish name))
                    "farmName",
                    Json.tryGet "farmName" player
                    |> Option.filter (fun name -> name <> JString "" && not (Json.isNullish name))
                    "look", Json.tryGet "look" player |> Option.filter (Json.isNullish >> not)
                    "worn",
                    Json.tryGet "worn" player
                    |> Option.filter (fun worn -> worn <> JArray [] && not (Json.isNullish worn))
                    // Health (M7): absent without combat.
                    "health", Json.tryGet "health" player |> Option.filter (Json.isNullish >> not)
                ]
            )

        project
        |> Json.set "scenes" (Json.get "scenes" (Json.get "world" state))
        // The buildings on the map (M5), built ones included; none is absent.
        |> setAll
            [
                "buildings",
                (match Json.get "buildings" state with
                 | JArray(_ :: _) as buildings -> Some buildings
                 | _ -> None)
            ]
        |> mapArray "npcs" (npc (Json.get "npcs" state))
        |> mapArray "quests" (quest (Json.get "quests" state))
        |> Json.set "player" projectPlayer
        |> setAll
            [
                "eventFlags", Some flags
                "animals", Some(Json.get "animals" state)
                "socialState", Some(Json.get "social" state)
                "quarantinedItems", Some(Json.get "quarantinedItems" state)
                "currentWeatherId", Some(Json.get "weatherId" clock)
                "mineDeepestFloor", Some(Json.get "deepestFloor" (Json.get "mine" state))
                "currentDay", Some(Json.get "day" clock)
                "currentSeason", Some(Json.get "season" clock)
                "currentDayOfSeason", dayOfSeason (calendar project) clock
                "currentTimeMinutes", Some(Json.get "timeMinutes" clock)
                "currentYear", Some(Json.get "year" clock)
                "rngState", Some(Json.get "rng" state)
                "keptState", keptState state
            ]

    /// The project with a playtest's final state (the engine's `GameState` JSON) written back,
    /// or why the result is not a valid project.
    let applyState (project: GameProject) (state: Json) : Result<GameProject, string> =
        match state with
        | JObject _ -> Decode.run SchemaJson.decodeGameProject (applyStateJson (SchemaJson.encodeGameProject project) state)
        | _ -> Error "The game state is not an object."

    // ── The kept playtest: beside the authored opening, never in it ──────────────────────────
    //
    // Keep changes used to write the played state over the project's own fields, and those
    // fields are what New Game starts from: a kept late-game playtest shipped as the opening of
    // the exported game. A kept playtest now lives in `keptPlaytest`. The editor's next playtest
    // resumes from it; the cartridge compiler, `farmc` and Export Game never read it. Making it
    // the opening is `promote`, which the editor offers with a preview (`promotionPreview`).

    /// FNV-1a (64 bit) over a scene's stable JSON: tells a scene edited since a playtest was kept.
    let private fingerprint (scene: Json) : string =
        let mutable hash = 14695981039346656037UL

        for c in Json.stableStringify scene do
            hash <- (hash ^^^ uint64 c) * 1099511628211UL

        hash.ToString "x16"

    let private idOf (value: Json) : string =
        Json.asString (Json.get "id" value) |> Option.defaultValue ""

    let private arrayOf (key: string) (value: Json) : Json list =
        match Json.get key value with
        | JArray items -> items
        | _ -> []

    /// The project without a kept playtest: what New Game and an export start from.
    let opening (project: GameProject) : GameProject =
        match project.KeptPlaytest with
        | None -> project
        | Some _ -> { project with KeptPlaytest = None }

    /// Keep changes: the playtest's final state (the engine's `GameState` JSON) kept beside the
    /// project, whose own fields (the opening) stay as authored. Fails when the state is not one
    /// the project can take.
    let keep (project: GameProject) (state: Json) : Result<GameProject, string> =
        let authored = opening project

        applyState authored state
        |> Result.map (fun _ ->
            let scenes =
                arrayOf "scenes" (SchemaJson.encodeGameProject authored)
                |> List.map (fun scene -> idOf scene, JString(fingerprint scene))

            { authored with
                KeptPlaytest = Some(JObject [ "scenes", JObject scenes; "state", state ])
            })

    /// The kept playtest's state on today's project: a scene the creator edited since the
    /// playtest was kept, or added, is the authored one (the edit shows in the next playtest, and
    /// what was played on that map is gone); an authored scene that is unchanged keeps what the
    /// playtest did to it; generated scenes (mine floors) stay; a deleted scene is dropped.
    let private keptState' (authored: GameProject) (kept: Json) : Json option =
        match Json.get "state" kept with
        | JObject _ as state ->
            let prints = Json.get "scenes" kept
            let played = arrayOf "scenes" (Json.get "world" state)
            let authoredScenes = arrayOf "scenes" (SchemaJson.encodeGameProject authored)

            let merged =
                [
                    for scene in authoredScenes do
                        let id = idOf scene
                        let unchanged = Json.get id prints = JString(fingerprint scene)

                        match played |> List.tryFind (fun p -> idOf p = id) with
                        | Some playedScene when unchanged -> yield playedScene
                        | _ -> yield scene
                    for scene in played do
                        if
                            Json.get "generated" scene = JBool true
                            && not (authoredScenes |> List.exists (fun a -> idOf a = idOf scene))
                        then
                            yield scene
                ]

            Some(Json.set "world" (Json.set "scenes" (JArray merged) (Json.get "world" state)) state)
        | _ -> None

    /// The project a playtest in the editor starts from: the kept playtest on today's project,
    /// or the opening when none is kept (or the kept one no longer fits the project).
    let resume (project: GameProject) : GameProject =
        let authored = opening project

        match project.KeptPlaytest |> Option.bind (keptState' authored) with
        | None -> authored
        | Some state ->
            match applyState authored state with
            | Ok resumed -> resumed
            | Error _ -> authored

    /// The scenes whose map changed since the playtest was kept (their kept tiles are dropped
    /// when the playtest resumes), by name.
    let editedSinceKept (project: GameProject) : string list =
        match project.KeptPlaytest with
        | None -> []
        | Some kept ->
            let prints = Json.get "scenes" kept

            arrayOf "scenes" (SchemaJson.encodeGameProject (opening project))
            |> List.filter (fun scene ->
                Json.has (idOf scene) prints
                && Json.get (idOf scene) prints <> JString(fingerprint scene))
            |> List.map (fun scene -> Json.asString (Json.get "name" scene) |> Option.defaultValue (idOf scene))

    /// Makes the kept playtest the opening: New Game and exports then start where it left off.
    /// The explicit step Keep changes no longer takes by itself.
    let promote (project: GameProject) : GameProject = resume project

    /// Forgets the kept playtest; the opening is untouched.
    let discard (project: GameProject) : GameProject = opening project

    // ── What a start is, in words ────────────────────────────────────────────────────────────

    /// One fact of a start: `Label` ("Date"), `Text` ("Day 1 of Spring, year 1, 6:00 AM").
    type StartFact = { Label: string; Text: string }

    let private plural (count: int) (one: string) (many: string) =
        if count = 1 then
            sprintf "1 %s" one
        else
            sprintf "%d %s" count many

    /// What a new game of `project` (its own fields, not a kept playtest) starts with.
    let describe (project: GameProject) : StartFact list =
        let calendar = (ContentCompiler.settings project).Calendar

        let dayOfSeason =
            SettingsSchema.clockDayOfSeason calendar project.CurrentSeason project.CurrentDay project.CurrentDayOfSeason

        let season =
            calendar.Seasons
            |> List.tryFind (fun s -> s.Id = project.CurrentSeason)
            |> Option.map (fun s -> s.Name)
            |> Option.defaultValue project.CurrentSeason

        let scene =
            project.Scenes
            |> List.tryFind (fun s -> s.Id = project.Player.SceneId)
            |> Option.map (fun s -> s.Name)
            |> Option.defaultValue project.Player.SceneId

        let items = project.Player.Inventory |> List.sumBy (fun slot -> int slot.Quantity)

        let questsWith (statuses: string list) =
            project.Quests
            |> List.filter (fun q -> List.contains q.Status statuses)
            |> List.length

        let tiles (pick: Tile -> bool) =
            project.Scenes
            |> List.sumBy (fun scene -> scene.Tiles |> List.sumBy (fun row -> row |> List.filter pick |> List.length))

        [
            {
                Label = "Date"
                Text =
                    sprintf "Day %d of %s, year %d, %s" (int dayOfSeason) season (int project.CurrentYear) (Readouts.clock project.CurrentTimeMinutes)
                    + (if project.CurrentDay > 1.0 then
                           sprintf " (day %d of the game)" (int project.CurrentDay)
                       else
                           "")
            }
            {
                Label = "Player"
                Text = sprintf "%s at (%d, %d)" scene (int (floor project.Player.X)) (int (floor project.Player.Y))
            }
            {
                Label = "Money"
                Text = sprintf "%d" (int64 project.Player.Money |> int)
            }
            {
                Label = "Inventory"
                Text =
                    if items = 0 then
                        "empty"
                    else
                        sprintf "%s in %s" (plural items "item" "items") (plural project.Player.Inventory.Length "slot" "slots")
            }
            {
                Label = "Quests"
                Text = sprintf "%d under way, %d completed" (questsWith [ QuestStatuses.Active ]) (questsWith [ QuestStatuses.Completed ])
            }
            {
                Label = "Flags and variables"
                Text = sprintf "%d" project.EventFlags.Length
            }
            {
                Label = "World"
                Text =
                    sprintf
                        "%s, %s, %s, %s"
                        (plural (tiles (fun t -> t.Crop.IsSome)) "crop" "crops")
                        (plural project.Animals.Length "animal" "animals")
                        (plural (defaultArg project.Buildings []).Length "building" "buildings")
                        (plural (tiles (fun t -> t.Machine.IsSome)) "machine" "machines")
            }
        ]

    /// What making the kept playtest the opening would change: each fact that differs, as
    /// (label, the opening now, the opening after). Empty without a kept playtest, or when the
    /// kept playtest starts exactly as the opening does.
    let promotionPreview (project: GameProject) : (string * string * string) list =
        match project.KeptPlaytest with
        | None -> []
        | Some _ ->
            let before = describe (opening project)
            let promoted = promote project
            let after = describe promoted

            let changedTiles =
                List.zip (opening project).Scenes (promoted.Scenes |> List.truncate (opening project).Scenes.Length)
                |> List.sumBy (fun (a, b) ->
                    if a.Id <> b.Id || a.Tiles.Length <> b.Tiles.Length then
                        0
                    else
                        List.zip a.Tiles b.Tiles
                        |> List.sumBy (fun (rowA, rowB) ->
                            if rowA.Length <> rowB.Length then
                                0
                            else
                                List.zip rowA rowB |> List.filter (fun (x, y) -> x <> y) |> List.length))

            [
                for a, b in List.zip before after do
                    if a.Text <> b.Text then
                        yield a.Label, a.Text, b.Text
                if changedTiles > 0 then
                    yield "Map", "as authored", plural changedTiles "tile changed by play" "tiles changed by play"
            ]
