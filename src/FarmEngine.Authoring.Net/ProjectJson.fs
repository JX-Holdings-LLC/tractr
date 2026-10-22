namespace FarmEngine.Authoring.Net

open System.Text.Json.Nodes
open FarmEngine.Authoring
open FarmEngine.Schemas

/// Project and exported-game loading for .NET callers that hold System.Text.Json nodes: the
/// `ProjectLoad` of the authoring core, with `JsonNode` in and out. Never throws.
module ProjectJson =
    /// The schema checks an exported game gets (`SchemaChecks.validateExportedGame`).
    let validateExportedGame (game: ExportedGame) : string list = SchemaChecks.validateExportedGame game

    /// A node `Json` cannot hold (a number beyond the double range, a duplicate key), as a
    /// failed result.
    let private unreadable (what: string) (message: string) : LoadResult<'T> =
        { Ok = false; Data = None; Errors = [ what + " is not valid JSON: " + message ] }

    /// Parse and validate a raw project (from storage or import; null for JSON null). The node
    /// is not modified.
    let loadProject (raw: JsonNode | null) : LoadResult<GameProject> =
        match JsonInterop.tryOfNode raw with
        | Ok json -> ProjectLoad.loadProject json
        | Error message -> unreadable "Project data" message

    /// `loadProject` over JSON text.
    let loadProjectText (json: string) : LoadResult<GameProject> = ProjectLoad.loadProjectText json

    /// Parse and validate an exported/shared game payload (null for JSON null).
    let loadExportedGame (raw: JsonNode | null) : LoadResult<ExportedGame> =
        match JsonInterop.tryOfNode raw with
        | Ok json -> ProjectLoad.loadExportedGame json
        | Error message -> unreadable "Game data" message

    /// `loadExportedGame` over JSON text.
    let loadExportedGameText (json: string) : LoadResult<ExportedGame> = ProjectLoad.loadExportedGameText json

    /// A project as a System.Text.Json node (members in schema order).
    let toNode (project: GameProject) : JsonNode =
        match JsonInterop.toNode (ProjectLoad.toJson project) with
        | null -> JsonObject() :> JsonNode
        | node -> node
