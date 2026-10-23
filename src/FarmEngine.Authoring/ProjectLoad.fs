namespace FarmEngine.Authoring

open FarmEngine.Schemas

/// A loaded project or exported game: the parsed and validated value, or why there is none.
type LoadResult<'T> =
    { Ok: bool
      Data: 'T option
      Errors: string list }

/// Loading projects and exported games: the schema version check, the typed parse
/// (`SchemaJson`) and the schema checks (`SchemaChecks`, at most 20 errors, like zod's
/// `safeParse` shown in the editor). The current schema version loads, and version 1, whose
/// files are the same shape (`migrate`). Never throws.
module ProjectLoad =
    let private failed (errors: string list) : LoadResult<'T> = { Ok = false; Data = None; Errors = errors }

    /// Version 1 (0.5–0.6) to 2 (0.7, M1 and M2): the new fields are all optional or defaulted,
    /// so a version 1 file reads as it is under the new number.
    let private migrate (raw: Json) : Json =
        match raw, Json.get "schemaVersion" raw with
        | JObject members, JNumber 1.0 ->
            JObject(members |> List.map (fun (key, value) -> if key = "schemaVersion" then key, JNumber ProjectSchema.CurrentProjectSchemaVersion else key, value))
        | _ -> raw

    /// Why `raw` cannot be read as a `noun` of the current schema version, if it cannot.
    let private versionError (noun: string) (raw: Json) : string option =
        let current = ProjectSchema.CurrentProjectSchemaVersion
        match raw with
        | JObject _ ->
            match Json.get "schemaVersion" raw with
            | JNumber v when v = current || v = 1.0 -> None
            | JNumber v when v > current ->
                Some(
                    noun + " schema version " + JsNumber.format v + " is newer than this engine supports ("
                    + JsNumber.format current + "). Update the engine."
                )
            | JNumber v ->
                Some(noun + " schema version " + JsNumber.format v + " is not supported (this engine reads version " + JsNumber.format current + ").")
            | _ -> Some(noun + " data has no schemaVersion (this engine reads version " + JsNumber.format current + ").")
        | _ -> Some(noun + " data is not an object")

    /// The version check, the typed parse and the checks; `prepare` runs on the parsed value
    /// before the checks. An exception from any step is an error in the result.
    let private load (noun: string) (decode: Path -> Json -> 'T) (prepare: 'T -> 'T) (validate: 'T -> string list) (raw: Json) : LoadResult<'T> =
        match versionError noun raw with
        | Some error -> failed [ error ]
        | None ->
            try
                match Decode.run decode (migrate raw) with
                | Error issue -> failed [ issue ]
                | Ok value ->
                    let value = prepare value
                    match List.truncate 20 (validate value) with
                    | [] -> { Ok = true; Data = Some value; Errors = [] }
                    | errors -> failed errors
            with error ->
                failed [ noun + " data could not be read: " + error.Message ]

    let private parseThen (noun: string) (loadJson: Json -> LoadResult<'T>) (text: string) : LoadResult<'T> =
        match Json.parse text with
        | Ok raw -> loadJson raw
        | Error message -> failed [ noun + " data is not valid JSON: " + message ]

    /// Parse and validate raw project data.
    let loadProject (raw: Json) : LoadResult<GameProject> =
        load "Project" SchemaJson.decodeGameProject DialogueCopies.reconcile SchemaChecks.validateProject raw

    /// `loadProject` over JSON text.
    let loadProjectText (text: string) : LoadResult<GameProject> = parseThen "Project" loadProject text

    /// A project as JSON (members in schema order).
    let toJson (project: GameProject) : Json = SchemaJson.encodeGameProject project

    /// A project as indented JSON text, for project files.
    let toText (project: GameProject) : string = Json.stringifyIndented (toJson project)
