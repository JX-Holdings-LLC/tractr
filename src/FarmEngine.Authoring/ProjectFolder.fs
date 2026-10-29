namespace FarmEngine.Authoring

open System
open System.Collections.Generic
open System.Text
open FarmEngine.Schemas

/// A project as a folder (M8): `project.json` with everything but the big lists, one file per
/// scene, NPC, dialogue and item, and one file per art or sound asset, so a large game opens
/// quickly and a team can review and merge it in version control. Single-file project JSON (the
/// same project with everything inline) stays the format for sharing and for older projects.
///
/// No I/O here: `toFiles` gives the files' paths (relative, `/`-separated) and bytes, and
/// `load` reads a folder that was read into a map of the same shape. Paths in `project.json`
/// are looked up in that map only, so a project file cannot reach outside its folder.
///
/// The layout:
///
/// - `project.json`: the project, with `scenes`, `npcs`, `dialogues` and `items` as lists of
///   file paths (`"scenes/scene-farm.json"`) in the project's order, and each custom asset's
///   `dataUrl` replaced by `dataFile` (`"assets/hero.png"`) and `dataType` (`"image/png"`).
///   An inline object in those lists, or an inline `dataUrl`, is read as it is.
/// - `scenes/<id>.json`: a scene, its tiles as a `tileGrid`: a `palette` of the distinct tiles
///   (without their `x` and `y`) and one `rows` string per row of palette indices, `n*i` for a
///   run of `n` tiles of index `i` (`"3*0 1 2*0"`).
/// - `npcs/<id>.json`, `dialogue/<id>.json`, `items/<id>.json`: one definition each.
/// - `assets/<id>.<extension>`: the asset's bytes.
module ProjectFolder =
    /// The project's own file in the folder.
    [<Literal>]
    let MainFile = "project.json"

    /// The lists that get a file per entry: their key in the project and their folder.
    let private splitLists =
        [
            "scenes", "scenes"
            "npcs", "npcs"
            "dialogues", "dialogue"
            "items", "items"
        ]

    /// The folders a project file may name.
    let private folders = Set.ofList [ "scenes"; "npcs"; "dialogue"; "items"; "assets" ]

    /// Device names Windows refuses as a file name, with or without an extension.
    let private reservedNames =
        Set.ofList
            [
                yield! [ "con"; "prn"; "aux"; "nul" ]
                for n in 0..9 do
                    yield sprintf "com%d" n
                    yield sprintf "lpt%d" n
            ]

    /// A file name for `id`, unique (ignoring case, for Windows and macOS) among `taken`, which
    /// it joins: letters, digits, `-`, `_` and `.` are kept, anything else becomes `_`, and a
    /// name already taken gets `~2`, `~3`, … (the project file records which file is whose).
    let private fileStem (taken: HashSet<string>) (id: string) : string =
        let cleaned =
            String(
                id.ToCharArray()
                |> Array.map (fun c ->
                    if
                        (c >= 'a' && c <= 'z')
                        || (c >= 'A' && c <= 'Z')
                        || (c >= '0' && c <= '9')
                        || c = '-'
                        || c = '_'
                        || c = '.'
                    then
                        c
                    else
                        '_')
            )

        let cleaned = cleaned.TrimStart('.').TrimEnd('.', ' ')

        let cleaned =
            if cleaned.Length > 80 then
                cleaned.Substring(0, 80)
            else
                cleaned

        let cleaned = if cleaned.Length = 0 then "_" else cleaned

        let cleaned =
            if reservedNames.Contains(cleaned.Split('.').[0].ToLowerInvariant()) then
                cleaned + "_"
            else
                cleaned

        let mutable stem = cleaned
        let mutable n = 1

        while not (taken.Add(stem.ToLowerInvariant())) do
            n <- n + 1
            stem <- sprintf "%s~%d" cleaned n

        stem

    /// The file extension for an asset's media type.
    let private extension (mediaType: string) : string =
        match mediaType.ToLowerInvariant() with
        | "image/png" -> "png"
        | "image/jpeg"
        | "image/jpg" -> "jpg"
        | "image/gif" -> "gif"
        | "image/webp" -> "webp"
        | "image/svg+xml" -> "svg"
        | "audio/ogg"
        | "audio/vorbis" -> "ogg"
        | "audio/wav"
        | "audio/wave"
        | "audio/x-wav" -> "wav"
        | "audio/mpeg" -> "mp3"
        | _ -> "bin"

    /// A base64 `data:` URL's media type (`image/png`) and bytes; `None` for anything else.
    let private decodeDataUrl (url: string) : (string * byte[]) option =
        if not (url.StartsWith("data:", StringComparison.OrdinalIgnoreCase)) then
            None
        else
            let comma = url.IndexOf ','

            if comma < 0 then
                None
            else
                let header = url.Substring(5, comma - 5)

                if not (header.EndsWith(";base64", StringComparison.OrdinalIgnoreCase)) then
                    None
                else
                    try
                        Some(header.Substring(0, header.Length - 7), Convert.FromBase64String(url.Substring(comma + 1)))
                    with :? FormatException ->
                        None

    // ── Tile grids ─────────────────────────────────────────────────────────

    /// A scene's `tiles` as a `tileGrid` (see the module notes). Tiles that are not objects, or a
    /// grid that is not a list of lists, stay as they are.
    let private compactTiles (scene: Json) : Json =
        match Json.get "tiles" scene with
        | JArray rows when
            rows
            |> List.forall (function
                | JArray cells ->
                    cells
                    |> List.forall (function
                        | JObject _ -> true
                        | _ -> false)
                | _ -> false)
            ->
            let palette = ResizeArray<Json>()
            let index = Dictionary<string, int>()

            let indexOf (tile: Json) =
                let key = Json.stringify tile

                match index.TryGetValue key with
                | true, i -> i
                | _ ->
                    palette.Add tile
                    index.[key] <- palette.Count - 1
                    palette.Count - 1
            // A tile that says where it is drops its `x` and `y`; one that names another spot
            // (a hand-edited file) keeps them.
            let at (y: int) (x: int) (cell: Json) =
                match Json.get "x" cell, Json.get "y" cell with
                | JNumber cx, JNumber cy when cx = float x && cy = float y -> Json.remove "y" (Json.remove "x" cell)
                | _ -> cell

            let rowText (y: int) (cells: Json list) =
                let indices = cells |> List.mapi (fun x cell -> indexOf (at y x cell))
                let parts = ResizeArray<string>()

                let rec runs (rest: int list) =
                    match rest with
                    | [] -> ()
                    | i :: _ ->
                        let length = rest |> List.takeWhile ((=) i) |> List.length

                        if length >= 3 then
                            parts.Add(sprintf "%d*%d" length i)
                        else
                            for _ in 1..length do
                                parts.Add(string i)

                        runs (List.skip length rest)

                runs indices
                String.Join(" ", parts)

            let rowTexts =
                rows
                |> List.mapi (fun y row ->
                    match row with
                    | JArray cells -> JString(rowText y cells)
                    | other -> other)

            let grid =
                JObject [ "palette", JArray(List.ofSeq palette); "rows", JArray rowTexts ]

            match scene with
            | JObject members ->
                JObject(
                    members
                    |> List.map (fun (key, value) -> if key = "tiles" then "tileGrid", grid else key, value)
                )
            | other -> other
        | _ -> scene

    /// A `tileGrid` back as `tiles` (row-major, each tile with its `x` and `y`).
    let private expandTiles (file: string) (scene: Json) : Result<Json, string> =
        match Json.tryGet "tileGrid" scene with
        | None -> Ok scene
        | Some grid ->
            match Json.get "palette" grid, Json.get "rows" grid with
            | JArray palette, JArray rows ->
                let palette = Array.ofList palette

                let parseRow (y: int) (row: Json) : Result<Json, string> =
                    match row with
                    | JString text ->
                        let cells = ResizeArray<Json>()
                        let mutable error = None

                        for part in text.Split(' ', StringSplitOptions.RemoveEmptyEntries) do
                            if error.IsNone then
                                let count, indexText =
                                    match part.IndexOf '*' with
                                    | -1 -> "1", part
                                    | at -> part.Substring(0, at), part.Substring(at + 1)

                                match Int32.TryParse count, Int32.TryParse indexText with
                                | (true, count), (true, i) when count >= 1 && count <= 100000 && i >= 0 && i < palette.Length ->
                                    for _ in 1..count do
                                        let x = cells.Count

                                        cells.Add(
                                            match palette.[i] with
                                            | JObject members when not (List.exists (fun (k, _) -> k = "x" || k = "y") members) ->
                                                JObject(("x", JNumber(float x)) :: ("y", JNumber(float y)) :: members)
                                            | other -> other
                                        )
                                | _ -> error <- Some(sprintf "%s: tileGrid row %d: \"%s\" is not a tile of the palette (%d tiles)" file y part palette.Length)

                        match error with
                        | Some error -> Error error
                        | None -> Ok(JArray(List.ofSeq cells))
                    | _ -> Error(sprintf "%s: tileGrid row %d is not text" file y)

                let parsed = rows |> List.mapi parseRow

                match
                    parsed
                    |> List.tryPick (function
                        | Error e -> Some e
                        | Ok _ -> None)
                with
                | Some error -> Error error
                | None ->
                    let tiles =
                        JArray(
                            parsed
                            |> List.choose (function
                                | Ok row -> Some row
                                | Error _ -> None)
                        )

                    match scene with
                    | JObject members ->
                        Ok(
                            JObject(
                                members
                                |> List.map (fun (key, value) -> if key = "tileGrid" then "tiles", tiles else key, value)
                            )
                        )
                    | other -> Ok other
            | _ -> Error(sprintf "%s: tileGrid needs a palette and rows" file)

    // ── Writing ────────────────────────────────────────────────────────────

    let private text (json: Json) : byte[] =
        Encoding.UTF8.GetBytes(Json.stringifyIndented json + "\n")

    /// The folder's files for `project`, `project.json` first: paths relative to the folder,
    /// `/`-separated, and their bytes. The same project always gives the same files.
    let toFiles (project: GameProject) : (string * byte[]) list =
        let json = ProjectLoad.toJson project
        let files = ResizeArray<string * byte[]>()

        let members =
            Json.spread json
            |> List.map (fun (key, value) ->
                match List.tryFind (fun (list, _) -> list = key) splitLists, value with
                | Some(_, folder), JArray entries ->
                    let taken = HashSet<string>()

                    let paths =
                        entries
                        |> List.map (fun entry ->
                            let id = Json.get "id" entry |> Json.asString |> Option.defaultValue ""
                            let path = sprintf "%s/%s.json" folder (fileStem taken id)
                            let entry = if key = "scenes" then compactTiles entry else entry
                            files.Add(path, text entry)
                            JString path)

                    key, JArray paths
                | _ when key = "customAssets" ->
                    match value with
                    | JArray assets ->
                        let taken = HashSet<string>()

                        let assets =
                            assets
                            |> List.map (fun asset ->
                                match decodeDataUrl (Json.get "dataUrl" asset |> Json.asString |> Option.defaultValue "") with
                                | Some(mediaType, bytes) ->
                                    let id = Json.get "id" asset |> Json.asString |> Option.defaultValue ""
                                    let path = sprintf "assets/%s.%s" (fileStem taken id) (extension mediaType)
                                    files.Add(path, bytes)

                                    match asset with
                                    | JObject members ->
                                        JObject(
                                            members
                                            |> List.collect (fun (k, v) ->
                                                if k = "dataUrl" then
                                                    [ "dataFile", JString path; "dataType", JString mediaType ]
                                                else
                                                    [ k, v ])
                                        )
                                    | other -> other
                                | None -> asset)

                        key, JArray assets
                    | other -> key, other
                | _ -> key, value)

        (MainFile, text (JObject members)) :: List.ofSeq files

    // ── Reading ────────────────────────────────────────────────────────────

    /// Whether `path` names a file a project folder may hold (`scenes/x.json`, `assets/x.png`).
    let private isFolderPath (path: string) =
        match path.Split '/' with
        | [| folder; name |] ->
            folders.Contains folder
            && name.Length > 0
            && name <> "."
            && name <> ".."
            && not (name.Contains '\\')
        | _ -> false

    /// The single-file project JSON of a folder read into `files` (relative `/`-separated
    /// paths to bytes), or what is wrong with it.
    let join (files: IReadOnlyDictionary<string, byte[]>) : Result<Json, string list> =
        let read (path: string) : Result<byte[], string> =
            if not (isFolderPath path) && path <> MainFile then
                Error(sprintf "%s is not a file of a project folder" path)
            else
                match files.TryGetValue path with
                | true, bytes -> Ok bytes
                | _ -> Error(sprintf "%s is missing from the project folder" path)

        let readJson (path: string) : Result<Json, string> =
            read path
            |> Result.bind (fun bytes ->
                match Json.parse (Encoding.UTF8.GetString bytes) with
                | Ok json -> Ok json
                | Error message -> Error(sprintf "%s is not valid JSON: %s" path message))

        match readJson MainFile with
        | Error error -> Error [ error ]
        | Ok(JObject members) ->
            let errors = ResizeArray<string>()

            let resolved =
                members
                |> List.map (fun (key, value) ->
                    match List.tryFind (fun (list, _) -> list = key) splitLists, value with
                    | Some _, JArray entries ->
                        let entries =
                            entries
                            |> List.choose (fun entry ->
                                match entry with
                                | JString path ->
                                    match
                                        readJson path
                                        |> Result.bind (fun json -> if key = "scenes" then expandTiles path json else Ok json)
                                    with
                                    | Ok json -> Some json
                                    | Error error ->
                                        errors.Add error
                                        None
                                | written -> Some written)

                        key, JArray entries
                    | _ when key = "customAssets" ->
                        match value with
                        | JArray assets ->
                            let assets =
                                assets
                                |> List.map (fun asset ->
                                    match Json.get "dataFile" asset, asset with
                                    | JString path, JObject assetMembers ->
                                        let mediaType =
                                            Json.get "dataType" asset
                                            |> Json.asString
                                            |> Option.defaultValue "application/octet-stream"

                                        match read path with
                                        | Ok bytes ->
                                            let url = "data:" + mediaType + ";base64," + Convert.ToBase64String bytes

                                            JObject(
                                                assetMembers
                                                |> List.choose (fun (k, v) ->
                                                    if k = "dataFile" then Some("dataUrl", JString url)
                                                    elif k = "dataType" then None
                                                    else Some(k, v))
                                            )
                                        | Error error ->
                                            errors.Add error
                                            asset
                                    | _ -> asset)

                            key, JArray assets
                        | other -> key, other
                    | _ -> key, value)

            if errors.Count > 0 then
                Error(List.ofSeq errors |> List.truncate 20)
            else
                Ok(JObject resolved)
        | Ok _ -> Error [ MainFile + " does not contain a project (expected a JSON object)" ]

    /// Load a project folder read into `files` (see `join`), validated like a project file.
    let load (files: IReadOnlyDictionary<string, byte[]>) : LoadResult<GameProject> =
        match join files with
        | Ok json -> ProjectLoad.loadProject json
        | Error errors ->
            {
                Ok = false
                Data = None
                Errors = errors
            }

    /// Whether `files` holds a project folder (it has a `project.json`).
    let isFolder (files: IReadOnlyDictionary<string, byte[]>) = files.ContainsKey MainFile

    /// The folder as single-file project JSON text (indented), or "" when it cannot be joined: the
    /// editor keeps the version a save replaces this way.
    let singleFileText (files: IReadOnlyDictionary<string, byte[]>) : string =
        match join files with
        | Ok json -> Json.stringifyIndented json
        | Error _ -> ""

    /// Whether `path` (relative, `/`-separated) is a file the folder format writes: `project.json`
    /// or one in its folders. A save replaces these and leaves anything else in the folder alone.
    let isProjectFile (path: string) = path = MainFile || isFolderPath path

    /// `join` as indented single-file project JSON text.
    let joinText (files: IReadOnlyDictionary<string, byte[]>) : Result<string, string list> =
        join files |> Result.map Json.stringifyIndented
