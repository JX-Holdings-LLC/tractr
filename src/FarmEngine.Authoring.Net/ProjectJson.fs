namespace FarmEngine.Authoring.Net

open System.Text.Json.Nodes
open FarmEngine.Authoring
open FarmEngine.Schemas

/// Project loading for .NET callers that hold System.Text.Json nodes: the
/// `ProjectLoad` of the authoring core, with `JsonNode` in and out. Never throws.
module ProjectJson =
    /// A node `Json` cannot hold (a number beyond the double range, a duplicate key), as a
    /// failed result.
    let private unreadable (what: string) (message: string) : LoadResult<'T> =
        {
            Ok = false
            Data = None
            Errors = [ what + " is not valid JSON: " + message ]
        }

    /// Parse and validate a raw project (from storage or import; null for JSON null). The node
    /// is not modified.
    let loadProject (raw: JsonNode | null) : LoadResult<GameProject> =
        match JsonInterop.tryOfNode raw with
        | Ok json -> ProjectLoad.loadProject json
        | Error message -> unreadable "Project data" message

    /// `loadProject` over JSON text.
    let loadProjectText (json: string) : LoadResult<GameProject> = ProjectLoad.loadProjectText json

    /// A project as a System.Text.Json node (members in schema order).
    let toNode (project: GameProject) : JsonNode =
        match JsonInterop.toNode (ProjectLoad.toJson project) with
        | null -> JsonObject() :> JsonNode
        | node -> node
