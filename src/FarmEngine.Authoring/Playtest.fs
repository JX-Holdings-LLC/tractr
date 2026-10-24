namespace FarmEngine.Authoring

open FarmEngine.Schemas

/// Keep changes: a playtest's final game state written back into the project (TS
/// `applyStateToProject`, which the Rust player also implements for project-started players).
/// The editor's Play Mode runs a compiled cartridge, so the state comes back as the engine's
/// `GameState` JSON and F# applies it (docs/LANGUAGES.md, `State.cs ApplyStateToProject`).
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
            |> setAll [ "x", Some(Json.get "x" state); "y", Some(Json.get "y" state); "sceneId", Some(Json.get "sceneId" state) ]
        | None -> entry

    let private quest (quests: Json) (entry: Json) : Json =
        match Json.asString (Json.get "id" entry) |> Option.bind (fun id -> objectKey id quests) with
        | None -> entry
        | Some progress ->
            let withStatus = Json.set "status" (Json.get "status" progress) entry
            match objectKey "objectives" progress, Json.get "objectives" withStatus with
            | Some objectives, JArray items ->
                let objective (item: Json) =
                    match Json.asString (Json.get "id" item) |> Option.bind (fun id -> objectKey id objectives) with
                    | Some state ->
                        item |> setAll [ "progress", Some(Json.get "progress" state); "completed", Some(Json.get "completed" state) ]
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
        | Ok settings -> (ContentCompiler.settings { GameProject.Default with Settings = settings }).Calendar
        | Error _ -> SettingsSchema.DefaultProjectSettings.Calendar

    /// `currentDayOfSeason` for the clock (Rust `kept_day_of_season`): absent when `currentDay`
    /// alone lands on the clock's season and day of season.
    let private dayOfSeason (calendar: CalendarConfig) (clock: Json) : Json option =
        let number = function
            | JNumber n -> Some n
            | _ -> None
        let day = number (Json.get "day" clock) |> Option.defaultValue 0.0
        let season = Json.asString (Json.get "season" clock) |> Option.defaultValue ""
        let kept = SettingsSchema.clockDayOfSeason calendar season day (number (Json.get "dayOfSeason" clock))
        let natural, naturalDay = SettingsSchema.naturalDate calendar day
        if natural.Id <> season || naturalDay <> kept then Some(JNumber kept) else None

    /// What the project's own fields do not hold (Rust `KeptState::of_state`): the tick, today's
    /// shop purchases, an open dialogue, shop or minigame, a cutscene that is playing, NPC walks
    /// and the mine floor; absent when there is nothing to carry.
    let private keptState (state: Json) : Json option =
        let present (key: string) (value: Json) = if Json.isNullish value then [] else [ key, value ]
        let npcs =
            match Json.get "npcs" state with
            | JObject members ->
                members
                |> List.choose (fun (id, npc) ->
                    match present "path" (Json.get "path" npc) @ present "patrolIndex" (Json.get "patrolIndex" npc) with
                    | [] -> None
                    | walk -> Some(id, JObject walk))
            | _ -> []
        let purchases =
            match Json.get "shopPurchasesToday" state with
            | JObject _ as purchases -> purchases
            | _ -> JObject []
        let tick = Json.get "tick" (Json.get "clock" state) |> Json.orElse (JNumber 0.0)
        let floor = Json.get "currentFloor" (Json.get "mine" state) |> Json.orElse (JNumber 0.0)
        let sessions =
            [ "dialogue"; "shop"; "minigame"; "cutscene"; "music" ] |> List.collect (fun key -> present key (Json.get key state))
        // The player's children and the shops' changed stock (M3, M4), when there are any.
        let family =
            [ match Json.get "children" state with
              | JArray (_ :: _) as children -> "children", children
              | _ -> ()
              match Json.get "shopChanges" state with
              | JObject (_ :: _) as changes -> "shopChanges", changes
              | _ -> () ]
        if tick = JNumber 0.0 && purchases = JObject [] && sessions.IsEmpty && npcs.IsEmpty && floor = JNumber 0.0 && family.IsEmpty then
            None
        else
            Some(JObject([ "tick", tick; "shopPurchasesToday", purchases ] @ sessions @ [ "npcs", JObject npcs; "mineCurrentFloor", floor ] @ family))

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
            |> setAll
                ([ for key in [ "x"; "y"; "direction"; "sceneId"; "inventory"; "maxInventorySize"; "money"; "energy"; "maxEnergy"; "skills"; "activeQuests"; "completedQuests" ] ->
                       key, Some(Json.get key player) ]
                 @ [ "equippedTool", Json.tryGet "equippedTool" player |> Option.filter (Json.isNullish >> not)
                     // The player character (M4): an empty name and no clothes are absent.
                     "name", Json.tryGet "name" player |> Option.filter (fun name -> name <> JString "" && not (Json.isNullish name))
                     "farmName", Json.tryGet "farmName" player |> Option.filter (fun name -> name <> JString "" && not (Json.isNullish name))
                     "look", Json.tryGet "look" player |> Option.filter (Json.isNullish >> not)
                     "worn", Json.tryGet "worn" player |> Option.filter (fun worn -> worn <> JArray [] && not (Json.isNullish worn)) ])
        project
        |> Json.set "scenes" (Json.get "scenes" (Json.get "world" state))
        |> mapArray "npcs" (npc (Json.get "npcs" state))
        |> mapArray "quests" (quest (Json.get "quests" state))
        |> Json.set "player" projectPlayer
        |> setAll
            [ "eventFlags", Some flags
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
              "keptState", keptState state ]

    /// The project with a playtest's final state (the engine's `GameState` JSON) written back,
    /// or why the result is not a valid project.
    let applyState (project: GameProject) (state: Json) : Result<GameProject, string> =
        match state with
        | JObject _ -> Decode.run SchemaJson.decodeGameProject (applyStateJson (SchemaJson.encodeGameProject project) state)
        | _ -> Error "The game state is not an object."
