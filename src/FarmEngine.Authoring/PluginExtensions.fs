namespace FarmEngine.Authoring

open System
open FarmEngine.Schemas

/// The kinds of a plugin argument (`PluginArgument.Type`, #239).
module PluginArgumentTypes =
    [<Literal>]
    let Text = "text"

    [<Literal>]
    let Number = "number"

    [<Literal>]
    let Integer = "integer"

    [<Literal>]
    let Bool = "bool"

    [<Literal>]
    let Id = "id"

    let All = [ Text; Number; Integer; Bool; Id ]

/// One condition or outcome a plugin defines, as the editor offers it (#239).
type PluginFunctionChoice =
    {
        /// `packId:name`, what a `plugin` condition's `condition` or outcome's `pluginOutcome` holds.
        FullName: string
        /// The pack's name, for the picker's group.
        PackName: string
        PackId: string
        /// Whether the pack is enabled.
        Enabled: bool
        Function: PluginFunction
    }

/// Plugin systems on the authoring side (docs/PLUGINS.md): the hooks they grant (Rust
/// `farm_plugins::granted_extension_hooks`), the conditions and outcomes plugins define, and the
/// checks of their uses (#237–#241).
module PluginExtensions =
    /// What `permissions.read` understands (Rust `farm_plugins::READ_SCOPES`).
    let ReadScopes = [ "player"; "world"; "people"; "content"; "any" ]

    /// The plugin id prefix of project scripts (`script/<id>`, Rust `farm_plugins::PROJECT_SCRIPT_PREFIX`).
    [<Literal>]
    let ProjectScriptPrefix = "script/"

    /// The outcome type of plugin outcomes (`pluginOutcome` and `args` live in the outcome's extra keys).
    [<Literal>]
    let PluginOutcomeType = "plugin"

    let private local (packId: string) (names: string list option) =
        names
        |> Option.defaultValue []
        |> List.choose (PackRules.minigameLocalName packId)

    /// `wanted` names that `permitted` also lists, as `packId:name`, in order, without duplicates.
    let private granted (packId: string) (wanted: string list option) (permitted: string list option) =
        let permitted = local packId permitted

        local packId wanted
        |> List.filter (fun name -> List.contains name permitted)
        |> List.map (fun name -> packId + ":" + name)
        |> List.distinct

    let private names (functions: PluginFunction list option) =
        functions |> Option.defaultValue [] |> List.map (fun f -> f.Name) |> Some

    /// Whether the manifest lets the pack's plugins answer `startTimer` (Rust `MutationGrants::allows`).
    let private allowsTimers (permissions: PackPermissions) =
        match permissions.Mutations with
        | None -> false
        | Some entries ->
            entries
            |> List.exists (fun entry ->
                let entry = entry.Trim()

                entry = "*"
                || entry = "startTimer"
                || entry = "startTimer:own"
                || entry = "startTimer:any")

    /// The hooks of a plugin's own systems (Rust `farm_plugins::granted_extension_hooks`):
    /// `condition:`/`outcome:` for what it declares, `entity:`/`use:`/`screen:` for what the
    /// manifest permits. (`timer` is every plugin's: it hears only the timers it started.)
    let grantedExtensionHooks (pack: ContentPack) (plugin: PackPlugin) : string list =
        let packId = pack.Manifest.Id
        let p = pack.Manifest.Permissions

        [
            for name in local packId (names plugin.Conditions) do
                yield "condition:" + packId + ":" + name
            for name in local packId (names plugin.Outcomes) do
                yield "outcome:" + packId + ":" + name
            for full in granted packId plugin.Entities p.Entities do
                yield "entity:" + full
            for full in granted packId plugin.ItemUses p.ItemUses do
                yield "use:" + full
            let screens = plugin.Screens |> Option.map (List.map (fun s -> s.Name))

            for full in granted packId screens p.Screens do
                yield "screen:" + full
        ]
        |> List.distinct

    /// Every hook a plugin may hear (Rust `farm_plugins::plugin_spec`): its `hooks` the manifest
    /// grants, then its systems' hooks.
    let grantedHooks (pack: ContentPack) (plugin: PackPlugin) : string list =
        let permitted = Set.ofList pack.Manifest.Permissions.Hooks

        (plugin.Hooks |> List.filter permitted.Contains)
        @ grantedExtensionHooks pack plugin
        |> List.distinct

    /// What the pack's plugins may read, in one line for the install review (`None`: nothing).
    let describeReads (permissions: PackPermissions) : string option =
        let what =
            function
            | "player" -> Some "the player (position, money, energy, inventory, skills, buffs)"
            | "world" -> Some "the world (date, weather, tiles, crops, animals, buildings)"
            | "people" -> Some "people and story (characters, friendship, quests, the pack's own flags)"
            | "content" -> Some "content definitions (items, crops, characters)"
            | "any" -> Some "your game's and other packs' flags and variables"
            | _ -> None

        match
            permissions.Read
            |> Option.defaultValue []
            |> List.choose (fun e -> what (e.Trim()))
        with
        | [] -> None
        | parts -> Some("reads " + String.Join("; ", List.distinct parts))

    /// What the pack's plugins add to the world and the interface, in one line for the install
    /// review (`None`: nothing).
    let describeSystems (pack: ContentPack) : string option =
        let p = pack.Manifest.Permissions

        let list (label: string) (entries: string list option) =
            match local pack.Manifest.Id entries |> List.distinct with
            | [] -> None
            | names -> Some(label + " " + String.Join(", ", names))

        let functions label (pick: PackPlugin -> PluginFunction list option) =
            match
                pack.Plugins
                |> List.collect (fun plugin -> pick plugin |> Option.defaultValue [])
                |> List.map (fun f -> f.Name)
                |> List.distinct
            with
            | [] -> None
            | names -> Some(label + " " + String.Join(", ", names))

        match
            [
                list "entity types" p.Entities
                list "item uses" p.ItemUses
                list "screens" p.Screens
                functions "conditions" (fun plugin -> plugin.Conditions)
                functions "outcomes" (fun plugin -> plugin.Outcomes)
                (if allowsTimers p then Some "timers" else None)
            ]
            |> List.choose id
        with
        | [] -> None
        | parts -> Some(String.Join("; ", parts))

    let private choices (project: GameProject) (pick: PackPlugin -> PluginFunction list option) =
        [
            for install in project.ContentPacks do
                let pack = install.Pack

                for plugin in pack.Plugins do
                    for f in pick plugin |> Option.defaultValue [] do
                        match PackRules.minigameLocalName pack.Manifest.Id f.Name with
                        | Some name ->
                            yield
                                {
                                    FullName = pack.Manifest.Id + ":" + name
                                    PackName = pack.Manifest.Name
                                    PackId = pack.Manifest.Id
                                    Enabled = install.Enabled
                                    Function = f
                                }
                        | None -> ()
        ]

    /// The conditions the installed packs' plugins define, in load order (the editor's condition
    /// picker lists the enabled ones under their pack's name).
    let conditionChoices (project: GameProject) =
        choices project (fun plugin -> plugin.Conditions)

    /// The outcomes the installed packs' plugins define, in load order.
    let outcomeChoices (project: GameProject) =
        choices project (fun plugin -> plugin.Outcomes)

    /// Whether the project has enabled scripts (whose conditions and outcomes have plain names).
    let hasScripts (project: GameProject) =
        project.Scripts
        |> Option.defaultValue []
        |> List.exists (fun s -> not s.Disabled)

    /// One problem of a plugin condition or outcome use: severity (`true` = error), the path below
    /// the use, and the message.
    type UseProblem =
        {
            IsError: bool
            Path: string
            Code: string
            Message: string
        }

    /// Check one use of a plugin condition or outcome (`what`: "condition" or "outcome") named
    /// `name` with `args`: it must exist, its pack should be enabled, and its arguments must fit
    /// the declared form. `exists kind id` says whether content of `kind` has `id` (`None`: the
    /// kind is not checked).
    let checkUse (project: GameProject) (what: string) (name: string) (args: Json option) (exists: string -> string -> bool option) : UseProblem list =
        let found =
            (if what = "condition" then
                 conditionChoices project
             else
                 outcomeChoices project)
            |> List.tryFind (fun c -> c.FullName = name)

        let problem isError code path message =
            {
                IsError = isError
                Path = path
                Code = code
                Message = message
            }

        if String.IsNullOrWhiteSpace name then
            [
                problem true (what + "PluginName") "" (sprintf "The plugin %s has no name" what)
            ]
        else
            match found with
            | None when not (name.Contains ':') && hasScripts project -> []
            | None ->
                [
                    problem
                        true
                        (what + "PluginUnknown")
                        ""
                        (sprintf "No installed pack defines the plugin %s \"%s\": it is always %s" what name (if what = "condition" then "false" else "skipped"))
                ]
            | Some choice ->
                [
                    if not choice.Enabled then
                        yield
                            problem
                                false
                                (what + "PluginPackOff")
                                ""
                                (sprintf
                                    "The plugin %s \"%s\" comes from pack \"%s\", which is turned off: it is %s"
                                    what
                                    name
                                    choice.PackName
                                    (if what = "condition" then "always false" else "skipped"))
                    let values =
                        match args with
                        | Some(JObject members) -> members
                        | _ -> []

                    for arg in choice.Function.Args do
                        let path = "args." + arg.Name
                        let label = defaultArg arg.Label arg.Name

                        match values |> List.tryFind (fun (key, _) -> key = arg.Name) |> Option.map snd with
                        | None
                        | Some JNull ->
                            if not arg.Optional then
                                yield
                                    problem
                                        true
                                        (what + "PluginArgMissing")
                                        path
                                        (sprintf "%s \"%s\" needs %s" (if what = "condition" then "Condition" else "Outcome") name label)
                        | Some value ->
                            let wrong expected =
                                problem true (what + "PluginArgType") path (sprintf "%s of \"%s\" must be %s" label name expected)

                            match arg.Type, value with
                            | PluginArgumentTypes.Number, JNumber _ -> ()
                            | PluginArgumentTypes.Integer, JNumber n when Math.Floor n = n -> ()
                            | PluginArgumentTypes.Bool, JBool _ -> ()
                            | PluginArgumentTypes.Text, JString _ -> ()
                            | PluginArgumentTypes.Id, JString id ->
                                match arg.Kind with
                                | Some kind ->
                                    match exists kind id with
                                    | Some false ->
                                        yield problem true (what + "PluginArgUnknown") path (sprintf "%s of \"%s\" names missing %s \"%s\"" label name kind id)
                                    | _ -> ()
                                | None -> ()
                            | PluginArgumentTypes.Number, _ -> yield wrong "a number"
                            | PluginArgumentTypes.Integer, _ -> yield wrong "a whole number"
                            | PluginArgumentTypes.Bool, _ -> yield wrong "on or off"
                            | PluginArgumentTypes.Text, _ -> yield wrong "a text"
                            | PluginArgumentTypes.Id, _ -> yield wrong "an id"
                            | _ -> ()
                ]
