namespace FarmEngine.Authoring

open System
open FarmEngine.Schemas

/// What a save migration renames (#242).
module SaveMigrationKinds =
    [<Literal>]
    let Item = "item"

    [<Literal>]
    let Scene = "scene"

    let All = [ Item; Scene ]

/// Save migration rules and the id set of the last export (#242). The rules compile into the
/// cartridge (`GameContent.SaveMigrations`) and the Rust player applies them when a save written
/// by an earlier version loads (`farm_cart::migrations`); Problems warns about ids the last export
/// had that the game lacks now and no rule covers (`ChecksUpkeep`).
module SaveMigrations =
    let rules (project: GameProject) : SaveMigration list = defaultArg project.SaveMigrations []

    let private sameRule (kind: string) (from: string) (rule: SaveMigration) = rule.Kind = kind && rule.From = from

    /// The project's rule for an old id, if any.
    let tryFind (kind: string) (from: string) (project: GameProject) : SaveMigration option =
        rules project |> List.tryFind (sameRule kind from)

    /// The rules with `rule` in place of the one for its old id (appended when there is none).
    let upsert (rule: SaveMigration) (project: GameProject) : SaveMigration list =
        let current = rules project

        if current |> List.exists (sameRule rule.Kind rule.From) then
            current
            |> List.map (fun r -> if sameRule rule.Kind rule.From r then rule else r)
        else
            current @ [ rule ]

    /// The rules without the one for an old id.
    let remove (kind: string) (from: string) (project: GameProject) : SaveMigration list =
        rules project |> List.filter (sameRule kind from >> not)

    /// `from` is now called `toId`.
    let rename (kind: string) (from: string) (toId: string) : SaveMigration =
        { SaveMigration.Default with
            Kind = kind
            From = from
            To = Some toId
        }

    /// Each unit of item `from` becomes `quantity` of `toId`.
    let replaceItem (from: string) (toId: string) (quantity: int) : SaveMigration =
        { rename SaveMigrationKinds.Item from toId with
            Quantity = (if quantity > 1 then Some(float quantity) else None)
        }

    /// Item `from` leaves the game; players get `money` a unit for it (nothing when 0).
    let removeItem (from: string) (name: string option) (money: float) : SaveMigration =
        { SaveMigration.Default with
            Kind = SaveMigrationKinds.Item
            From = from
            Money = (if money > 0.0 then Some money else None)
            Name = name
        }

    /// The ids of compiled content a save can hold, for the version being exported.
    let idsOf (content: GameContent) (version: string) : ExportedIds =
        {
            Version = version
            Items = content.Items |> List.map (fun item -> item.Id) |> List.distinct
            Scenes = content.Scenes |> List.map (fun scene -> scene.Id) |> List.distinct
        }

    /// The id set of the last export, if one was recorded.
    let exported (project: GameProject) : ExportedIds option =
        project.Export |> Option.bind (fun settings -> settings.ExportedIds)

    /// The export settings with this export's ids remembered; `None` when they are already.
    let remember (content: GameContent) (project: GameProject) : ExportSettings option =
        let settings =
            project.Export
            |> Option.defaultWith (fun () -> Defaults.newExportSettings project)

        let version =
            settings.Version
            |> Option.filter (String.IsNullOrWhiteSpace >> not)
            |> Option.defaultValue project.Version

        let ids = idsOf content version

        if project.Export.IsSome && settings.ExportedIds = Some ids then
            None
        else
            Some { settings with ExportedIds = Some ids }

    /// The ids the last export had that `content` lacks and no rule covers, as (kind, id):
    /// saves of that version would set them aside.
    let orphans (content: GameContent) (project: GameProject) : (string * string) list =
        match exported project with
        | None -> []
        | Some ids ->
            let covered kind id =
                rules project |> List.exists (sameRule kind id)

            let missing kind (old: string list) (current: string list) =
                let now = Set.ofList current

                old
                |> List.filter (fun id -> not (now.Contains id) && not (covered kind id))
                |> List.map (fun id -> kind, id)

            missing SaveMigrationKinds.Item ids.Items (content.Items |> List.map (fun i -> i.Id))
            @ missing SaveMigrationKinds.Scene ids.Scenes (content.Scenes |> List.map (fun s -> s.Id))

    /// Removing `id` would leave saves of the last export holding it without a rule: the editor
    /// asks what becomes of it first.
    let removalNeedsRule (kind: string) (id: string) (project: GameProject) : bool =
        match exported project with
        | None -> false
        | Some ids ->
            let shipped =
                if kind = SaveMigrationKinds.Scene then
                    ids.Scenes
                else
                    ids.Items

            List.contains id shipped && (tryFind kind id project).IsNone

/// Test states (#229): named game states a playtest, a screenshot run or a headless replay starts
/// from. They are part of the project file but never of a cartridge; Rust applies them after New
/// Game (`farm_sim::test_states`), the editor through the `applyTestState` debug action and
/// `farm-player` with `--test-state`.
module TestStates =
    let all (project: GameProject) : TestState list = defaultArg project.TestStates []

    let tryFind (id: string) (project: GameProject) : TestState option =
        all project |> List.tryFind (fun state -> state.Id = id)

    /// The states with `state` in place of the one with its id (appended when new).
    let upsert (state: TestState) (project: GameProject) : TestState list =
        let current = all project

        if current |> List.exists (fun s -> s.Id = state.Id) then
            current |> List.map (fun s -> if s.Id = state.Id then state else s)
        else
            current @ [ state ]

    let remove (id: string) (project: GameProject) : TestState list =
        all project |> List.filter (fun state -> state.Id <> id)

    /// A free id for a state named `name`.
    let newId (project: GameProject) (name: string) : string =
        Defaults.slugId name (all project |> List.map (fun state -> state.Id)) "state"

    let toJson (state: TestState) : Json = SchemaJson.encodeTestState state

    /// A test state as JSON text (a `--test-state` file).
    let toText (state: TestState) : string = Json.stringifyIndented (toJson state)

    /// The Play Mode debug action that puts the running game in `state`.
    let applyAction (state: TestState) : string =
        Json.stringify (JObject [ "type", JString "applyTestState"; "state", toJson state ])

    /// A test state from JSON text (a `--test-state` file, or what the player captured).
    let ofText (text: string) : Result<TestState, string> =
        match Json.parse text with
        | Error message -> Error message
        | Ok json ->
            try
                Ok(SchemaJson.decodeTestState [] json)
            with DecodeError(path, message) ->
                Error(if path = "" then message else path + ": " + message)
