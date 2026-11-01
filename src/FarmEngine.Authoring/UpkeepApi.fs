namespace FarmEngine.Authoring

open System.Collections.Generic
open FarmEngine.Schemas

/// Test states for C# (#229): the Play Mode toolbar's "Play from state" and "Save state".
[<AbstractClass; Sealed>]
type TestStatesApi =
    static member All(project: GameProject) : IReadOnlyList<TestState> =
        TestStates.all project |> Array.ofList :> IReadOnlyList<TestState>

    static member Find(project: GameProject, id: string) : TestState | null =
        match TestStates.tryFind id project with
        | Some state -> state
        | None -> null

    /// The debug action JSON that puts a running playtest in `state`.
    static member ApplyAction(state: TestState) : string = TestStates.applyAction state

    /// What the player captured (`captureTestState`), named `name` under a free id.
    static member FromCapture(project: GameProject, json: string, name: string) : TestState =
        match TestStates.ofText json with
        | Ok state ->
            let name =
                if System.String.IsNullOrWhiteSpace name then
                    "Test state"
                else
                    name.Trim()

            { state with
                Id = TestStates.newId project name
                Name = name
            }
        | Error message -> raise (System.FormatException message)

    /// The edit that adds `state` (or replaces the one with its id).
    static member Save(project: GameProject, state: TestState) : Edit =
        SetTestStates(TestStates.upsert state project)

    static member Remove(project: GameProject, id: string) : Edit =
        SetTestStates(TestStates.remove id project)

    /// `project` with `state` saved (a playtest's pre-play snapshot, which Keep changes writes back).
    static member WithState(project: GameProject, state: TestState) : GameProject =
        Document.run project (SetTestStates(TestStates.upsert state project))

    /// A state as a file for `farm-player --test-state`.
    static member ToText(state: TestState) : string = TestStates.toText state

/// Save migrations for C# (#242).
[<AbstractClass; Sealed>]
type SaveMigrationsApi =
    static member All(project: GameProject) : IReadOnlyList<SaveMigration> =
        SaveMigrations.rules project |> Array.ofList :> IReadOnlyList<SaveMigration>

    /// Removing item `itemId` would orphan saves of the last export: ask first.
    static member ItemRemovalNeedsRule(project: GameProject, itemId: string) : bool =
        SaveMigrations.removalNeedsRule SaveMigrationKinds.Item itemId project

    static member SceneRemovalNeedsRule(project: GameProject, sceneId: string) : bool =
        SaveMigrations.removalNeedsRule SaveMigrationKinds.Scene sceneId project

    /// The edit that records "`from` is now `toId`".
    static member Rename(project: GameProject, kind: string, from: string, toId: string) : Edit =
        SetSaveMigrations(SaveMigrations.upsert (SaveMigrations.rename kind from toId) project)

    /// The edit that records "each `from` becomes `quantity` of `toId`".
    static member ReplaceItem(project: GameProject, from: string, toId: string, quantity: int) : Edit =
        SetSaveMigrations(SaveMigrations.upsert (SaveMigrations.replaceItem from toId quantity) project)

    /// The edit that records "`from` leaves the game, paid `money` a unit" (the item's value by
    /// default: `RemoveItemAtValue`).
    static member RemoveItem(project: GameProject, from: string, money: float) : Edit =
        let name =
            project.Items
            |> List.tryFind (fun i -> i.Id = from)
            |> Option.map (fun i -> i.Name)

        SetSaveMigrations(SaveMigrations.upsert (SaveMigrations.removeItem from name money) project)

    /// The item removed and saves paid its value: one undo step for the editor's delete.
    static member RemoveItemAtValue(project: GameProject, itemId: string) : Edit =
        let value =
            project.Items
            |> List.tryFind (fun i -> i.Id = itemId)
            |> Option.map (fun i -> i.Value)
            |> Option.defaultValue 0.0

        Batch("Remove item", [ SaveMigrationsApi.RemoveItem(project, itemId, value); RemoveItem itemId ])

    static member Remove(project: GameProject, kind: string, from: string) : Edit =
        SetSaveMigrations(SaveMigrations.remove kind from project)

    /// The export settings edit that remembers this export's ids; false when nothing changes.
    static member TryRemember(project: GameProject, [<System.Runtime.InteropServices.Out>] edit: byref<Edit>) : bool =
        match SaveMigrations.remember (ContentCompiler.compile project) project with
        | Some settings ->
            edit <- SetExportSettings(Some settings)
            true
        | None -> false

/// The content graph for C# (#231): where an entry comes from and what it leads to.
[<AbstractClass; Sealed>]
type ContentGraphApi =
    static member Build(project: GameProject) : ContentGraph = ContentGraph.ofProject project

    static member Entries(graph: ContentGraph) : IReadOnlyList<GraphEntry> =
        graph.Entries |> Array.ofList :> IReadOnlyList<GraphEntry>

    static member Find(graph: ContentGraph, kind: string, id: string) : GraphEntry | null =
        match ContentGraph.tryFind kind id graph with
        | Some entry -> entry
        | None -> null

    static member Obtainable(graph: ContentGraph, kind: string, id: string) : bool = graph.Obtainable.Contains(kind, id)

    static member Sources(entry: GraphEntry) : IReadOnlyList<GraphLink> =
        entry.Sources |> Array.ofList :> IReadOnlyList<GraphLink>

    static member Uses(entry: GraphEntry) : IReadOnlyList<GraphLink> =
        entry.Uses |> Array.ofList :> IReadOnlyList<GraphLink>

    /// "Needs: item Moon Dust, quest q-roots" (empty when the link needs nothing).
    static member Needs(graph: ContentGraph, link: GraphLink) : string =
        link.Needs
        |> List.map (fun (kind, id) ->
            let name =
                ContentGraph.tryFind kind id graph
                |> Option.map (fun e -> e.Name)
                |> Option.defaultValue id

            kind + " " + name)
        |> String.concat ", "
