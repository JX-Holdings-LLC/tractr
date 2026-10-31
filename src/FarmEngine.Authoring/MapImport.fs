namespace FarmEngine.Authoring

open System
open System.IO
open System.IO.Compression
open System.Text
open System.Xml
open FarmEngine.Schemas

/// A tileset of an imported map: its image (a path beside the map) cut into a grid, and the
/// properties of its tiles by local id. `FirstGid` is the number the map's layers use for its
/// first tile.
type MapTileset =
    {
        Name: string
        FirstGid: int
        Image: string option
        TileWidth: int
        TileHeight: int
        Columns: int
        Spacing: int
        Margin: int
        TileCount: int
        Tiles: (int * (string * string) list) list
        /// Where it came from (the `.tsx` of an external tileset, else the map).
        Source: string
    }

/// A tile layer: one gid per cell, row-major (0: empty).
type MapTileLayer =
    {
        Name: string
        Properties: (string * string) list
        Width: int
        Height: int
        Data: int64[]
    }

/// An object of an object layer (Tiled) or an entity (LDtk), in map pixels.
type MapObject =
    {
        Id: string
        Name: string
        Type: string
        X: float
        Y: float
        Width: float
        Height: float
        /// Tiled tile objects are anchored at their bottom-left corner.
        BottomAnchored: bool
        Properties: (string * string) list
        Layer: string
    }

/// A map read from Tiled or LDtk, before it becomes a scene.
type MapSource =
    {
        /// The file, for the import report.
        File: string
        Width: int
        Height: int
        TileWidth: int
        TileHeight: int
        Tilesets: MapTileset list
        Layers: MapTileLayer list
        Objects: MapObject list
        Warnings: Problem list
    }

/// What an imported map changed (besides the scene): the report lists everything that did not map.
type MapImportResult =
    {
        Project: GameProject
        SceneId: string
        Created: bool
        Terrains: string list
        Report: Problem list
    }

/// Maps from Tiled (`.tmx`, `.tmj`, with inline or external `.tsx`/`.tsj` tilesets) and LDtk
/// levels (#233), imported as a scene and re-imported over it. The source file is authoritative:
/// re-import replaces the tiles and the markers (doors, the player start, NPC positions and
/// schedule stops, nodes, machines, lights, event tiles) and keeps what play changed (crops,
/// soil, dropped items, a node's damage, a machine's contents).
[<CompilationRepresentation(CompilationRepresentationFlags.ModuleSuffix)>]
module MapImport =
    /// The largest scene side (docs/CREATOR-GUIDE.md "Bigger maps").
    [<Literal>]
    let MaxSide = 512

    /// The bits Tiled keeps in a gid for flipped and rotated tiles.
    [<Literal>]
    let FlipMask = 0xF0000000L

    let private problem (severity: Severity) (code: string) (message: string) (path: string) (target: NavigationTarget option) =
        {
            Severity = severity
            Code = code
            Message = message
            Path = path
            Target = target
        }

    let private warn code message path =
        problem Severity.Warning code message path None

    exception private Unreadable of string

    /// `text`, or empty for null (the .NET path and XML APIs).
    let private nn (text: string | null) : string =
        match text with
        | null -> ""
        | t -> t

    /// The file name of `path`.
    let fileName (path: string) = nn (Path.GetFileName path)

    /// A Tractr id from a name in the source (`Tall Grass` → `tall-grass`).
    let private clipless (value: string) = AsepriteImport.clipName value

    let private fail message = raise (Unreadable message)

    let private intOf (text: string) (fallback: int) =
        match Double.TryParse(text, Globalization.NumberStyles.Float, Globalization.CultureInfo.InvariantCulture) with
        | true, v when not (Double.IsNaN v) && abs v < 1e9 -> int (Math.Truncate v)
        | _ -> fallback

    let private floatOf (text: string) =
        match Double.TryParse(text, Globalization.NumberStyles.Float, Globalization.CultureInfo.InvariantCulture) with
        | true, v when not (Double.IsNaN v) && not (Double.IsInfinity v) -> v
        | _ -> 0.0

    /// `rel` beside `file` (the path the editor reads it from).
    let resolve (file: string) (rel: string) : string =
        let directory = nn (Path.GetDirectoryName file)

        Path.GetFullPath(Path.Combine((if String.IsNullOrEmpty directory then "." else directory), rel.Replace('\\', '/')))

    // ── Tile data ──────────────────────────────────────────────────────

    let private decodeData (encoding: string) (compression: string) (text: string) (count: int) : int64[] =
        match encoding with
        | "csv" ->
            text.Split([| ','; '\n'; '\r'; ' '; '\t' |], StringSplitOptions.RemoveEmptyEntries)
            |> Array.map (fun s ->
                match Int64.TryParse(s.Trim()) with
                | true, v -> v
                | _ -> fail "A tile layer's CSV data is damaged.")
        | "base64" ->
            let raw =
                try
                    Convert.FromBase64String(text.Trim())
                with :? FormatException ->
                    fail "A tile layer's base64 data is damaged."

            let expected = count * 4

            let bytes =
                match compression with
                | ""
                | "none" -> raw
                | "zlib"
                | "gzip" ->
                    use input = new MemoryStream(raw)

                    use z: Stream =
                        if compression = "zlib" then
                            new ZLibStream(input, CompressionMode.Decompress)
                        else
                            new GZipStream(input, CompressionMode.Decompress)

                    let output = Array.zeroCreate<byte> expected
                    let mutable read = 0
                    let mutable finished = false

                    while not finished && read < expected do
                        let n =
                            try
                                z.Read(output, read, expected - read)
                            with :? InvalidDataException ->
                                fail "A tile layer's compressed data is damaged."

                        if n = 0 then finished <- true else read <- read + n

                    Array.sub output 0 read
                | other -> fail (sprintf "Tile layers compressed with %s are not supported: use zlib, gzip or CSV." other)

            Array.init (bytes.Length / 4) (fun i -> int64 (BitConverter.ToUInt32(bytes, i * 4)))
        | other -> fail (sprintf "Tile layer encoding '%s' is not supported." other)

    let private fitted (count: int) (data: int64[]) =
        if data.Length = count then
            data
        elif data.Length > count then
            Array.sub data 0 count
        else
            Array.append data (Array.zeroCreate (count - data.Length))

    // ── Tiled XML (.tmx, .tsx) ─────────────────────────────────────────

    let private safeXml (text: string) : XmlDocument =
        let settings =
            XmlReaderSettings(DtdProcessing = DtdProcessing.Prohibit, XmlResolver = null)

        let document = XmlDocument(XmlResolver = null)

        try
            use reader = XmlReader.Create(new StringReader(text), settings)
            document.Load reader
            document
        with :? XmlException as e ->
            fail ("The XML could not be read: " + e.Message)

    let private attr (name: string) (node: XmlNode) : string =
        match node.Attributes with
        | null -> ""
        | attributes ->
            match attributes.[name] with
            | null -> ""
            | a -> a.Value

    /// The element children of `node`.
    let private elements (node: XmlNode) : XmlNode list =
        [
            for child in node.ChildNodes do
                match child with
                | Null -> ()
                | NonNull element ->
                    if element.NodeType = XmlNodeType.Element then
                        yield element
        ]

    let private children (name: string) (node: XmlNode) : XmlNode list =
        elements node |> List.filter (fun child -> child.Name = name)

    let private root (document: XmlDocument) : XmlNode =
        match document.DocumentElement with
        | null -> fail "The XML has no root element."
        | element -> element

    let private xmlProperties (node: XmlNode) : (string * string) list =
        [
            for properties in children "properties" node do
                for property in children "property" properties do
                    let value = attr "value" property

                    yield attr "name" property, (if value = "" then nn property.InnerText else value)
        ]

    let private tilesetFromXml (node: XmlNode) (firstGid: int) (source: string) : MapTileset =
        let image = children "image" node |> List.tryHead

        {
            Name = attr "name" node
            FirstGid = firstGid
            Image = image |> Option.map (attr "source") |> Option.filter (fun s -> s <> "")
            TileWidth = intOf (attr "tilewidth" node) 0
            TileHeight = intOf (attr "tileheight" node) 0
            Columns = intOf (attr "columns" node) 0
            Spacing = intOf (attr "spacing" node) 0
            Margin = intOf (attr "margin" node) 0
            TileCount = intOf (attr "tilecount" node) 0
            Tiles =
                [
                    for tile in children "tile" node do
                        let own = children "image" tile

                        if not own.IsEmpty then
                            // An image collection tileset: each tile its own picture.
                            yield intOf (attr "id" tile) -1, ("@image", attr "source" own.Head) :: xmlProperties tile
                        else
                            yield intOf (attr "id" tile) -1, xmlProperties tile
                ]
            Source = source
        }

    // ── Tiled JSON (.tmj, .tsj) ────────────────────────────────────────

    let private jText (key: string) (json: Json) =
        match Json.tryGet key json with
        | Some(JString s) -> s
        | Some(JNumber n) -> JsNumber.format n
        | Some(JBool b) -> (if b then "true" else "false")
        | _ -> ""

    let private jInt (key: string) (json: Json) (fallback: int) =
        match Json.tryGet key json with
        | Some(JNumber n) when abs n < 1e9 -> int (Math.Truncate n)
        | _ -> fallback

    let private jFloat (key: string) (json: Json) =
        match Json.tryGet key json with
        | Some(JNumber n) when not (Double.IsNaN n) && not (Double.IsInfinity n) -> n
        | _ -> 0.0

    let private jList (key: string) (json: Json) =
        match Json.tryGet key json with
        | Some(JArray items) -> items
        | _ -> []

    let private jsonProperties (json: Json) : (string * string) list =
        jList "properties" json |> List.map (fun p -> jText "name" p, jText "value" p)

    let private tilesetFromJson (json: Json) (firstGid: int) (source: string) : MapTileset =
        {
            Name = jText "name" json
            FirstGid = firstGid
            Image = Some(jText "image" json) |> Option.filter (fun s -> s <> "")
            TileWidth = jInt "tilewidth" json 0
            TileHeight = jInt "tileheight" json 0
            Columns = jInt "columns" json 0
            Spacing = jInt "spacing" json 0
            Margin = jInt "margin" json 0
            TileCount = jInt "tilecount" json 0
            Tiles =
                [
                    for tile in jList "tiles" json do
                        let image = jText "image" tile

                        yield jInt "id" tile -1, (if image <> "" then [ "@image", image ] else []) @ jsonProperties tile
                ]
            Source = source
        }

    let private parseJsonText (what: string) (text: string) : Json =
        match Json.parse text with
        | Ok json -> json
        | Error message -> fail (sprintf "The %s could not be read: %s" what message)

    /// Reads a Tiled map. `read` loads a file by its full path (external tilesets); `file` is
    /// the map's own path, for `.tmx` (XML) or `.tmj`/`.json`.
    let parseTiled (file: string) (text: string) (read: string -> string option) : Result<MapSource, string> =
        try
            let warnings = ResizeArray<Problem>()
            let fileName = fileName file

            let external (source: string) (firstGid: int) =
                let path = resolve file source

                match read path with
                | None -> fail (sprintf "The tileset %s could not be read." source)
                | Some tilesetText ->
                    if tilesetText.TrimStart().StartsWith "<" then
                        let document = safeXml tilesetText
                        tilesetFromXml (root document) firstGid path
                    else
                        tilesetFromJson (parseJsonText "tileset" tilesetText) firstGid path

            if text.TrimStart().StartsWith "<" then
                let document = safeXml text
                let map = root document

                if map.Name <> "map" then
                    fail "This is not a Tiled map."

                if attr "orientation" map <> "" && attr "orientation" map <> "orthogonal" then
                    fail "Only orthogonal Tiled maps can be imported."

                if attr "infinite" map = "1" then
                    fail "Infinite Tiled maps can't be imported: turn off Infinite in the map's properties."

                let width = intOf (attr "width" map) 0
                let height = intOf (attr "height" map) 0

                let tilesets =
                    [
                        for node in children "tileset" map do
                            let firstGid = intOf (attr "firstgid" node) 1

                            match attr "source" node with
                            | "" -> yield tilesetFromXml node firstGid file
                            | source -> yield external source firstGid
                    ]

                let layers = ResizeArray<MapTileLayer>()
                let objects = ResizeArray<MapObject>()

                let rec walk (node: XmlNode) (hidden: bool) =
                    for child in elements node do
                        do
                            let hidden = hidden || attr "visible" child = "0"

                            match child.Name with
                            | "group" -> walk child hidden
                            | "layer" when not hidden ->
                                let w = intOf (attr "width" child) width
                                let h = intOf (attr "height" child) height

                                let data =
                                    match children "data" child with
                                    | data :: _ ->
                                        if not (children "chunk" data).IsEmpty then
                                            fail "Infinite Tiled maps can't be imported."

                                        match attr "encoding" data with
                                        | "" ->
                                            children "tile" data
                                            |> List.map (fun t -> int64 (intOf (attr "gid" t) 0))
                                            |> Array.ofList
                                        | encoding -> decodeData encoding (attr "compression" data) (nn data.InnerText) (w * h)
                                    | [] -> [||]

                                layers.Add
                                    {
                                        Name = attr "name" child
                                        Properties = xmlProperties child
                                        Width = w
                                        Height = h
                                        Data = fitted (w * h) data
                                    }
                            | "objectgroup" when not hidden ->
                                for o in children "object" child do
                                    let kind =
                                        match attr "type" o with
                                        | "" -> attr "class" o
                                        | t -> t

                                    objects.Add
                                        {
                                            Id = attr "id" o
                                            Name = attr "name" o
                                            Type = kind
                                            X = floatOf (attr "x" o)
                                            Y = floatOf (attr "y" o)
                                            Width = floatOf (attr "width" o)
                                            Height = floatOf (attr "height" o)
                                            BottomAnchored = attr "gid" o <> ""
                                            Properties = xmlProperties o
                                            Layer = attr "name" child
                                        }
                            | "imagelayer" ->
                                warnings.Add(
                                    warn
                                        "import.imageLayer"
                                        (sprintf "Image layer '%s' was left out: paint it as tiles to import it." (attr "name" child))
                                        fileName
                                )
                            | _ -> ()

                walk map false

                Ok
                    {
                        File = fileName
                        Width = width
                        Height = height
                        TileWidth = intOf (attr "tilewidth" map) 0
                        TileHeight = intOf (attr "tileheight" map) 0
                        Tilesets = tilesets
                        Layers = List.ofSeq layers
                        Objects = List.ofSeq objects
                        Warnings = List.ofSeq warnings
                    }
            else
                let map = parseJsonText "map" text

                if jText "type" map <> "" && jText "type" map <> "map" then
                    fail "This is not a Tiled map."

                if jText "orientation" map <> "" && jText "orientation" map <> "orthogonal" then
                    fail "Only orthogonal Tiled maps can be imported."

                if Json.tryGet "infinite" map = Some(JBool true) then
                    fail "Infinite Tiled maps can't be imported: turn off Infinite in the map's properties."

                let width = jInt "width" map 0
                let height = jInt "height" map 0

                let tilesets =
                    [
                        for tileset in jList "tilesets" map do
                            let firstGid = jInt "firstgid" tileset 1

                            match jText "source" tileset with
                            | "" -> yield tilesetFromJson tileset firstGid file
                            | source -> yield external source firstGid
                    ]

                let layers = ResizeArray<MapTileLayer>()
                let objects = ResizeArray<MapObject>()

                let rec walk (items: Json list) (hidden: bool) =
                    for layer in items do
                        let hidden = hidden || Json.tryGet "visible" layer = Some(JBool false)

                        match jText "type" layer with
                        | "group" -> walk (jList "layers" layer) hidden
                        | "tilelayer" when not hidden ->
                            let w = jInt "width" layer width
                            let h = jInt "height" layer height

                            if not (jList "chunks" layer).IsEmpty then
                                fail "Infinite Tiled maps can't be imported."

                            let data =
                                match Json.tryGet "data" layer with
                                | Some(JArray cells) ->
                                    cells
                                    |> List.map (fun c ->
                                        match c with
                                        | JNumber n when n >= 0.0 && n < 4294967296.0 -> int64 n
                                        | _ -> 0L)
                                    |> Array.ofList
                                | Some(JString encoded) -> decodeData "base64" (jText "compression" layer) encoded (w * h)
                                | _ -> [||]

                            layers.Add
                                {
                                    Name = jText "name" layer
                                    Properties = jsonProperties layer
                                    Width = w
                                    Height = h
                                    Data = fitted (w * h) data
                                }
                        | "objectgroup" when not hidden ->
                            for o in jList "objects" layer do
                                let kind =
                                    match jText "type" o with
                                    | "" -> jText "class" o
                                    | t -> t

                                objects.Add
                                    {
                                        Id = jText "id" o
                                        Name = jText "name" o
                                        Type = kind
                                        X = jFloat "x" o
                                        Y = jFloat "y" o
                                        Width = jFloat "width" o
                                        Height = jFloat "height" o
                                        BottomAnchored = Json.has "gid" o
                                        Properties = jsonProperties o
                                        Layer = jText "name" layer
                                    }
                        | "imagelayer" ->
                            warnings.Add(
                                warn
                                    "import.imageLayer"
                                    (sprintf "Image layer '%s' was left out: paint it as tiles to import it." (jText "name" layer))
                                    fileName
                            )
                        | _ -> ()

                walk (jList "layers" map) false

                Ok
                    {
                        File = fileName
                        Width = width
                        Height = height
                        TileWidth = jInt "tilewidth" map 0
                        TileHeight = jInt "tileheight" map 0
                        Tilesets = tilesets
                        Layers = List.ofSeq layers
                        Objects = List.ofSeq objects
                        Warnings = List.ofSeq warnings
                    }
        with Unreadable message ->
            Error message

    // ── LDtk ───────────────────────────────────────────────────────────

    /// The level identifiers of an LDtk project, in order.
    let ldtkLevels (text: string) : string list =
        match Json.parse text with
        | Ok json -> jList "levels" json |> List.map (jText "identifier")
        | Error _ -> []

    /// LDtk custom data (`terrain=soil`, one per line, or JSON) as properties.
    let private customData (data: string) : (string * string) list =
        match Json.parse data with
        | Ok(JObject members) -> members |> List.map (fun (k, v) -> k, jText k (JObject [ k, v ]))
        | _ ->
            data.Split([| '\n'; ';' |], StringSplitOptions.RemoveEmptyEntries)
            |> Array.choose (fun line ->
                match line.IndexOf '=' with
                | i when i > 0 -> Some(line.Substring(0, i).Trim(), line.Substring(i + 1).Trim())
                | _ -> None)
            |> List.ofArray

    /// Reads one level of an LDtk project (`level`: its identifier; absent: the first). Levels
    /// saved in separate files (`externalRelPath`) are read with `read`.
    let parseLdtk (file: string) (text: string) (level: string option) (read: string -> string option) : Result<MapSource, string> =
        try
            let fileName = fileName file
            let project = parseJsonText "LDtk project" text
            let defs = defaultArg (Json.tryGet "defs" project) (JObject [])
            let levels = jList "levels" project

            let chosen =
                match level with
                | Some id -> levels |> List.tryFind (fun l -> jText "identifier" l = id)
                | None -> List.tryHead levels

            let chosen =
                match chosen with
                | None -> fail "The LDtk project has no such level."
                | Some l ->
                    match jText "externalRelPath" l with
                    | "" -> l
                    | rel ->
                        match read (resolve file rel) with
                        | Some levelText -> parseJsonText "LDtk level" levelText
                        | None -> fail (sprintf "The level file %s could not be read." rel)

            let layerDefs = jList "layers" defs
            let tilesetDefs = jList "tilesets" defs
            let warnings = ResizeArray<Problem>()

            // Every tileset (and every IntGrid layer, whose values are tiles of their own) gets a
            // range of gids, as in Tiled.
            let mutable nextGid = 1

            let tilesets =
                [
                    for tileset in tilesetDefs do
                        let w = jInt "pxWid" tileset 0
                        let h = jInt "pxHei" tileset 0
                        let grid = jInt "tileGridSize" tileset 16
                        let spacing = jInt "spacing" tileset 0
                        let padding = jInt "padding" tileset 0
                        let columns = max 1 ((w - 2 * padding + spacing) / max 1 (grid + spacing))
                        let rows = max 1 ((h - 2 * padding + spacing) / max 1 (grid + spacing))

                        let tags =
                            jList "enumTags" tileset
                            |> List.collect (fun tag ->
                                let value = jText "enumValueId" tag

                                jList "tileIds" tag
                                |> List.choose (fun id ->
                                    match id with
                                    | JNumber n -> Some(int n, ("terrain", clipless value))
                                    | _ -> None))

                        let data =
                            jList "customData" tileset
                            |> List.map (fun entry -> jInt "tileId" entry -1, customData (jText "data" entry))

                        let ids = (List.map fst tags @ List.map fst data) |> List.distinct

                        let first = nextGid
                        nextGid <- nextGid + columns * rows

                        yield
                            jInt "uid" tileset -1,
                            {
                                Name = jText "identifier" tileset
                                FirstGid = first
                                Image = Some(jText "relPath" tileset) |> Option.filter (fun s -> s <> "")
                                TileWidth = grid
                                TileHeight = grid
                                Columns = columns
                                Spacing = spacing
                                Margin = padding
                                TileCount = columns * rows
                                Tiles =
                                    [
                                        for id in ids ->
                                            id,
                                            (data
                                             |> List.tryFind (fun (i, _) -> i = id)
                                             |> Option.map snd
                                             |> Option.defaultValue [])
                                            @ (tags |> List.filter (fun (i, _) -> i = id) |> List.map snd)
                                    ]
                                Source = file
                            }
                ]

            let intGrids =
                [
                    for layer in layerDefs do
                        if jText "type" layer = "IntGrid" then
                            let values = jList "intGridValues" layer
                            let first = nextGid

                            nextGid <-
                                nextGid
                                + 1
                                + (values |> List.map (fun v -> jInt "value" v 0) |> List.fold max 0)

                            yield
                                jInt "uid" layer -1,
                                {
                                    Name = jText "identifier" layer
                                    FirstGid = first
                                    Image = None
                                    TileWidth = jInt "gridSize" layer 16
                                    TileHeight = jInt "gridSize" layer 16
                                    Columns = 0
                                    Spacing = 0
                                    Margin = 0
                                    TileCount = 0
                                    Tiles =
                                        [
                                            for v in values ->
                                                let identifier = jText "identifier" v

                                                jInt "value" v 0,
                                                [
                                                    "terrain",
                                                    (if identifier = "" then
                                                         sprintf "%s-%d" (jText "identifier" layer) (jInt "value" v 0)
                                                     else
                                                         identifier)
                                                ]
                                        ]
                                    Source = file
                                }
                ]

            let instances = jList "layerInstances" chosen

            let grid =
                instances
                |> List.tryHead
                |> Option.map (fun l -> jInt "__gridSize" l 16)
                |> Option.defaultValue 16

            let width =
                instances
                |> List.tryHead
                |> Option.map (fun l -> jInt "__cWid" l 0)
                |> Option.defaultValue 0

            let height =
                instances
                |> List.tryHead
                |> Option.map (fun l -> jInt "__cHei" l 0)
                |> Option.defaultValue 0

            let layers = ResizeArray<MapTileLayer>()
            let objects = ResizeArray<MapObject>()

            // LDtk lists layers top first; Tractr paints bottom first.
            for layer in List.rev instances do
                let name = jText "__identifier" layer
                let cw = jInt "__cWid" layer width
                let ch = jInt "__cHei" layer height
                let size = jInt "__gridSize" layer grid
                let visible = Json.tryGet "visible" layer <> Some(JBool false)
                let data = Array.zeroCreate<int64>(cw * ch)

                let paintTiles (tiles: Json list) (tileset: MapTileset) =
                    for tile in tiles do
                        match Json.tryGet "px" tile with
                        | Some(JArray [ JNumber x; JNumber y ]) ->
                            let cx = int x / max 1 size
                            let cy = int y / max 1 size

                            if cx >= 0 && cy >= 0 && cx < cw && cy < ch then
                                data.[cy * cw + cx] <- int64 (tileset.FirstGid + jInt "t" tile 0)

                            if jInt "f" tile 0 <> 0 then
                                warnings.Add(
                                    warn
                                        "import.flippedTile"
                                        (sprintf "A flipped tile was imported unflipped (layer '%s')." name)
                                        (sprintf "%s layer '%s' (%d, %d)" fileName name cx cy)
                                )
                        | _ -> ()

                let tilesetOf () =
                    let uid =
                        match Json.tryGet "__tilesetDefUid" layer with
                        | Some(JNumber n) -> int n
                        | _ -> -1

                    tilesets |> List.tryFind (fun (u, _) -> u = uid) |> Option.map snd

                if visible then
                    match jText "__type" layer with
                    | "Tiles" ->
                        tilesetOf () |> Option.iter (paintTiles (jList "gridTiles" layer))

                        layers.Add
                            {
                                Name = name
                                Properties = []
                                Width = cw
                                Height = ch
                                Data = data
                            }
                    | "AutoLayer" ->
                        tilesetOf () |> Option.iter (paintTiles (jList "autoLayerTiles" layer))

                        layers.Add
                            {
                                Name = name
                                Properties = []
                                Width = cw
                                Height = ch
                                Data = data
                            }
                    | "IntGrid" ->
                        let auto = jList "autoLayerTiles" layer

                        match tilesetOf () with
                        | Some tileset when not auto.IsEmpty -> paintTiles auto tileset
                        | _ ->
                            let uid = jInt "layerDefUid" layer -1

                            match intGrids |> List.tryFind (fun (u, _) -> u = uid) with
                            | Some(_, values) ->
                                jList "intGridCsv" layer
                                |> List.iteri (fun i v ->
                                    match v with
                                    | JNumber n when n > 0.0 && i < data.Length -> data.[i] <- int64 values.FirstGid + int64 n
                                    | _ -> ())
                            | None -> ()

                        // IntGrid values are ground (walls, water): painted on the background.
                        layers.Add
                            {
                                Name = name
                                Properties = [ "layer", TileLayers.Background ]
                                Width = cw
                                Height = ch
                                Data = data
                            }
                    | "Entities" ->
                        for entity in jList "entityInstances" layer do
                            let gx, gy =
                                match Json.tryGet "__grid" entity with
                                | Some(JArray [ JNumber x; JNumber y ]) -> int x, int y
                                | _ -> 0, 0

                            let fields =
                                jList "fieldInstances" entity
                                |> List.map (fun f ->
                                    let value =
                                        match Json.tryGet "__value" f with
                                        | Some(JObject _ as point) when Json.has "cx" point -> sprintf "%d,%d" (jInt "cx" point 0) (jInt "cy" point 0)
                                        | Some v -> jText "v" (JObject [ "v", v ])
                                        | None -> ""

                                    jText "__identifier" f, value)

                            objects.Add
                                {
                                    Id = jText "iid" entity
                                    Name = jText "__identifier" entity
                                    Type = jText "__identifier" entity
                                    X = float (gx * size)
                                    Y = float (gy * size)
                                    Width = max (float size) (jFloat "width" entity)
                                    Height = max (float size) (jFloat "height" entity)
                                    BottomAnchored = false
                                    Properties = fields
                                    Layer = name
                                }
                    | _ -> ()

            Ok
                {
                    File = fileName
                    Width = width
                    Height = height
                    TileWidth = grid
                    TileHeight = grid
                    Tilesets = List.map snd tilesets @ List.map snd intGrids
                    Layers = List.ofSeq layers
                    Objects = List.ofSeq objects
                    Warnings = List.ofSeq warnings
                }
        with Unreadable message ->
            Error message

    // ── Into a scene ───────────────────────────────────────────────────

    /// Where a tile layer is painted: a fixed layer (`background`, `overlay`, `object`), one of
    /// the scene's extra layers, or nowhere.
    [<RequireQualifiedAccess>]
    type LayerRole =
        | Fixed of layer: string
        | Extra of id: string * above: bool
        | Skip

    let private prop (key: string) (props: (string * string) list) : string option =
        props
        |> List.tryFind (fun (k, _) -> String.Equals(k, key, StringComparison.OrdinalIgnoreCase))
        |> Option.map (fun (_, v) -> v.Trim())
        |> Option.filter (fun v -> v <> "")

    let private propAny (keys: string list) (props: (string * string) list) =
        keys |> List.tryPick (fun k -> prop k props)

    let private yes (value: string) =
        match value.Trim().ToLowerInvariant() with
        | "true"
        | "1"
        | "yes" -> true
        | _ -> false

    let private flag (key: string) (props: (string * string) list) = prop key props |> Option.map yes

    /// The role of a tile layer: its `tractr` (or `layer`) property — `background`, `overlay`,
    /// `object`, `ignore`, or an extra layer's id — else its name (Ground, Paths, Objects…);
    /// any other name is an extra layer, drawn above the player when its `above` property is
    /// true or its name says Above or Roof.
    let layerRole (layer: MapTileLayer) : LayerRole =
        let above (name: string) =
            flag "above" layer.Properties
            |> Option.defaultValue (name.Contains "above" || name.Contains "roof")

        match
            propAny [ "tractr"; "layer" ] layer.Properties
            |> Option.map (fun s -> s.ToLowerInvariant())
        with
        | Some("ignore" | "skip" | "none") -> LayerRole.Skip
        | Some("background" | "overlay" | "object" as fixedLayer) -> LayerRole.Fixed fixedLayer
        | Some other -> LayerRole.Extra(clipless other, above other)
        | None ->
            let name = layer.Name.Trim().ToLowerInvariant()

            match name with
            | "background"
            | "ground"
            | "floor"
            | "terrain"
            | "base" -> LayerRole.Fixed TileLayers.Background
            | "overlay"
            | "path"
            | "paths"
            | "roads" -> LayerRole.Fixed TileLayers.Overlay
            | "object"
            | "objects"
            | "walls"
            | "buildings" -> LayerRole.Fixed TileLayers.Object
            | _ -> LayerRole.Extra(clipless layer.Name, above name)

    /// What to import: the file (re-import matches it, with the LDtk level), the scene to make or
    /// update (`SceneId`; absent: the earlier import's, else a new one named `SceneName`), and
    /// `Image`, which loads a tileset image by its full path through the editor's image checks
    /// (a PNG data URL and its size).
    type MapImportRequest =
        {
            Path: string
            Kind: string
            Level: string option
            SceneId: string option
            SceneName: string
            Image: string -> (string * int * int) option
        }

    /// The earlier import of `path` (and `level`) whose scene still exists.
    let previousImport (project: GameProject) (path: string) (level: string option) : ImportSource option =
        defaultArg project.Imports []
        |> List.tryFind (fun source ->
            (source.Kind = ImportKinds.Tiled || source.Kind = ImportKinds.Ldtk)
            && String.Equals(source.Path, path, StringComparison.OrdinalIgnoreCase)
            && source.Level = level
            && project.Scenes |> List.exists (fun s -> s.Id = source.TargetId))

    /// Minutes of the day from `8:30`, `14:00`, `8:30pm` or a number of minutes.
    let minuteOf (text: string) : float option =
        let text = text.Trim().ToLowerInvariant()
        let pm = text.EndsWith "pm"
        let am = text.EndsWith "am"

        let text =
            if pm || am then
                text.Substring(0, text.Length - 2).Trim()
            else
                text

        match text.Split ':' with
        | [| h; m |] ->
            match Int32.TryParse h, Int32.TryParse m with
            | (true, h), (true, m) when h >= 0 && h < 24 && m >= 0 && m < 60 ->
                let h =
                    if pm && h < 12 then h + 12
                    elif am && h = 12 then 0
                    else h

                Some(float (h * 60 + m))
            | _ -> None
        | [| n |] ->
            match Double.TryParse(n, Globalization.NumberStyles.Float, Globalization.CultureInfo.InvariantCulture) with
            | true, v when v >= 0.0 && v < 1440.0 -> Some(Math.Floor v)
            | _ -> None
        | _ -> None

    /// Imports `source` into `project` as a scene (#233): a new one, or over the scene an earlier
    /// import of the same file made. Every Tiled tile becomes a terrain — the one its `terrain`
    /// property names (matched, or created), else one made for that tile with its art — with
    /// `walkable`, `plantable`, `water` and `fishable` from its properties. Objects become
    /// markers by their type: `door`, `player`, `npc`, `schedule`, `node`, `machine`, `light`,
    /// `event`. The report lists what did not map, with the file and the tile.
    let import (project: GameProject) (request: MapImportRequest) (source: MapSource) : Result<MapImportResult, string> =
        if source.Width < 1 || source.Height < 1 then
            Error "The map has no tiles."
        elif source.Width > MaxSide || source.Height > MaxSide then
            Error(sprintf "The map is %d×%d tiles; scenes can be at most %d a side." source.Width source.Height MaxSide)
        elif source.TileWidth < 1 || source.TileHeight < 1 then
            Error "The map has no tile size."
        else
            let report = ResizeArray<Problem>(source.Warnings)
            let width, height = source.Width, source.Height
            let previous = previousImport project request.Path request.Level

            let sceneId =
                match request.SceneId, previous with
                | Some id, _ -> id
                | None, Some p -> p.TargetId
                | None, None ->
                    Defaults.slugId
                        (if request.SceneName = "" then
                             nn (Path.GetFileNameWithoutExtension request.Path)
                         else
                             request.SceneName)
                        (project.Scenes |> List.map (fun s -> s.Id))
                        "map"

            let existingScene = project.Scenes |> List.tryFind (fun s -> s.Id = sceneId)

            let at (layer: string) (x: int) (y: int) =
                sprintf "%s layer '%s' (%d, %d)" source.File layer x y

            let target x y =
                Some(NavigationTarget.Scene(sceneId, x, y))

            let add severity code message path x y =
                report.Add(problem severity code message path (target x y))

            // ── Tilesets → assets and terrains ──
            let builtinTerrains = Builtin.terrains ()
            let projectTerrains = defaultArg project.Terrains []
            let terrainIndex = Collections.Generic.Dictionary<string, TerrainDefinition>()

            for t in builtinTerrains @ projectTerrains do
                terrainIndex.[t.Id] <- t

            let createdTerrains = ResizeArray<TerrainDefinition>()
            let assets = ResizeArray<CustomAsset>(project.CustomAssets)

            let upsertAsset (asset: CustomAsset) =
                match Seq.tryFindIndex (fun (a: CustomAsset) -> a.Id = asset.Id) assets with
                | Some i ->
                    assets.[i] <-
                        { assets.[i] with
                            DataUrl = asset.DataUrl
                            Width = asset.Width
                            Height = asset.Height
                        }
                | None -> assets.Add asset

            let imageAssets =
                Collections.Generic.Dictionary<string, (string * int * int) option>()

            let imageAsset (tileset: MapTileset) (suffix: string) (relative: string) : (string * int * int) option =
                let key = tileset.Source + "|" + relative

                match imageAssets.TryGetValue key with
                | true, known -> known
                | _ ->
                    let id = "tileset-" + clipless tileset.Name + suffix

                    let made =
                        match request.Image(resolve tileset.Source relative) with
                        | Some(dataUrl, w, h) ->
                            upsertAsset
                                { CustomAsset.Default with
                                    Id = id
                                    Name = fileName relative
                                    Type = CustomAssetTypes.Tile
                                    DataUrl = dataUrl
                                    Width = Some(float w)
                                    Height = Some(float h)
                                }

                            Some(id, w, h)
                        | None ->
                            report.Add(
                                warn "import.tilesetImage" (sprintf "The image %s of tileset '%s' could not be imported." relative tileset.Name) source.File
                            )

                            None

                    imageAssets.[key] <- made
                    made

            let tilesets = source.Tilesets |> List.sortByDescending (fun t -> t.FirstGid)

            let tilesetOf (gid: int64) =
                tilesets |> List.tryFind (fun t -> int64 t.FirstGid <= gid)

            let terrainFor = Collections.Generic.Dictionary<string, string option>()

            /// The terrain of a tile of a tileset painted on `layer`.
            let terrainOf (tileset: MapTileset) (local: int) (fixedLayer: string) : string option =
                let key = sprintf "%s|%d|%d" tileset.Source tileset.FirstGid local

                match terrainFor.TryGetValue key with
                | true, known -> known
                | _ ->
                    let props =
                        tileset.Tiles
                        |> List.tryFind (fun (id, _) -> id = local)
                        |> Option.map snd
                        |> Option.defaultValue []

                    let named = prop "terrain" props |> Option.map clipless

                    let visual () =
                        match prop "@image" props with
                        | Some own ->
                            imageAsset tileset (sprintf "-%d" local) own
                            |> Option.map (fun (id, _, _) -> { VisualRef.Default with AssetId = id })
                        | None ->
                            match tileset.Image with
                            | None -> None
                            | Some image ->
                                imageAsset tileset "" image
                                |> Option.bind (fun (id, w, _) ->
                                    let columns =
                                        if tileset.Columns > 0 then
                                            tileset.Columns
                                        else
                                            max
                                                1
                                                ((w - 2 * tileset.Margin + tileset.Spacing)
                                                 / (tileset.TileWidth + tileset.Spacing))

                                    let col = local % columns
                                    let row = local / columns

                                    Some
                                        { VisualRef.Default with
                                            AssetId = id
                                            Frame =
                                                Some
                                                    { ArtFrame.Default with
                                                        X = float (tileset.Margin + col * (tileset.TileWidth + tileset.Spacing))
                                                        Y = float (tileset.Margin + row * (tileset.TileHeight + tileset.Spacing))
                                                        Width = float tileset.TileWidth
                                                        Height = float tileset.TileHeight
                                                    }
                                        })

                    let make (id: string) (art: VisualRef option) =
                        let water = flag "water" props |> Option.defaultValue false

                        { TerrainDefinition.Default with
                            Id = id
                            Name = defaultArg (prop "name" props) (sprintf "%s %d" tileset.Name local)
                            Layer = fixedLayer
                            Color = defaultArg (prop "color" props) "#808080"
                            Visual = art
                            Walkable =
                                flag "walkable" props
                                |> Option.orElse (flag "collision" props |> Option.map not)
                                |> Option.defaultValue (fixedLayer <> TileLayers.Object)
                            Plantable = defaultArg (flag "plantable" props) false
                            WaterSource = water
                            Fishable = defaultArg (flag "fishable" props) water
                        }

                    let chosen =
                        match named with
                        | Some id when terrainIndex.ContainsKey id -> Some id
                        | Some id ->
                            let terrain = make id (visual ())
                            terrainIndex.[id] <- terrain
                            createdTerrains.Add terrain
                            Some id
                        | None ->
                            match visual () with
                            | None -> None
                            | art ->
                                let id = sprintf "%s-%d" (clipless tileset.Name) local
                                let terrain = make id art
                                terrainIndex.[id] <- terrain
                                createdTerrains.Add terrain
                                Some id

                    terrainFor.[key] <- chosen
                    chosen

            // ── Tile layers → cells ──
            let count = width * height
            let background = Array.create<string option> count None
            let overlay = Array.create<string option> count None
            let objectLayer = Array.create<string option> count None
            let extras = ResizeArray<string * bool * string option[]>()

            let roles =
                let roles = source.Layers |> List.map (fun layer -> layer, layerRole layer)

                if roles |> List.exists (fun (_, r) -> r = LayerRole.Fixed TileLayers.Background) then
                    roles
                else
                    // Without a layer named for it, the bottom layer is the background.
                    let mutable placed = false

                    roles
                    |> List.map (fun (layer, role) ->
                        match role with
                        | LayerRole.Extra _ when not placed ->
                            placed <- true
                            layer, LayerRole.Fixed TileLayers.Background
                        | _ -> layer, role)

            for layer, role in roles do
                let cells, terrainLayer =
                    match role with
                    | LayerRole.Fixed l when l = TileLayers.Background -> Some background, l
                    | LayerRole.Fixed l when l = TileLayers.Overlay -> Some overlay, l
                    | LayerRole.Fixed l -> Some objectLayer, l
                    | LayerRole.Extra(id, above) ->
                        match extras |> Seq.tryFind (fun (e, _, _) -> e = id) with
                        | Some(_, _, cells) -> Some cells, TileLayers.Overlay
                        | None ->
                            let cells = Array.create<string option> count None
                            extras.Add(id, above, cells)
                            Some cells, TileLayers.Overlay
                    | LayerRole.Skip -> None, ""

                match cells with
                | None -> ()
                | Some cells ->
                    let unmapped = Collections.Generic.Dictionary<int64, int * int * int>()
                    let mutable flipped = false

                    for y in 0 .. min height layer.Height - 1 do
                        for x in 0 .. min width layer.Width - 1 do
                            let raw = layer.Data.[y * layer.Width + x]

                            if raw <> 0L then
                                if raw &&& FlipMask <> 0L && not flipped then
                                    flipped <- true

                                    add Severity.Warning "import.flippedTile" "Flipped or rotated tiles are imported unflipped." (at layer.Name x y) x y

                                let gid = raw &&& ~~~FlipMask

                                let terrain =
                                    tilesetOf gid
                                    |> Option.bind (fun tileset ->
                                        let local = int (gid - int64 tileset.FirstGid)

                                        if tileset.TileCount > 0 && local >= tileset.TileCount then
                                            None
                                        else
                                            terrainOf tileset local terrainLayer)

                                match terrain with
                                | Some id -> cells.[y * width + x] <- Some id
                                | None ->
                                    match unmapped.TryGetValue gid with
                                    | true, (fx, fy, n) -> unmapped.[gid] <- (fx, fy, n + 1)
                                    | _ -> unmapped.[gid] <- (x, y, 1)

                    for KeyValue(gid, (x, y, n)) in unmapped do
                        add
                            Severity.Warning
                            "import.unmappedTile"
                            (sprintf "Tile %d has no terrain: give it a terrain property or a tileset image (%d tile%s)." gid n (if n = 1 then "" else "s"))
                            (at layer.Name x y)
                            x
                            y

            // ── Objects → markers ──
            let transitions = ResizeArray<SceneTransition>()
            let nodes = Collections.Generic.Dictionary<int * int, string>()
            let machines = Collections.Generic.Dictionary<int * int, string>()
            let lightCells = Array.create<string option> count None
            let mutable npcs = project.Npcs
            let mutable events = project.Events
            let mutable player = project.Player
            let mutable startScene = project.StartSceneId
            let placedNpcs = ResizeArray<string>()

            let nodeTypes = project.NodeTypes @ Builtin.nodeTypes () @ Builtin.mineNodeTypes ()

            let machineTypes = project.MachineTypes @ Builtin.machineTypes ()

            let findNpc (key: string) =
                npcs
                |> List.tryFind (fun n ->
                    n.Id = key
                    || String.Equals(n.Name, key, StringComparison.OrdinalIgnoreCase)
                    || n.Id = clipless key)

            for o in source.Objects do
                let left = o.X
                let top = if o.BottomAnchored then o.Y - o.Height else o.Y
                let clampX v = max 0 (min (width - 1) v)
                let clampY v = max 0 (min (height - 1) v)
                let x0 = clampX (int (Math.Floor(left / float source.TileWidth)))
                let y0 = clampY (int (Math.Floor(top / float source.TileHeight)))

                let x1 =
                    clampX (max x0 (int (Math.Ceiling((left + o.Width) / float source.TileWidth)) - 1))

                let y1 =
                    clampY (max y0 (int (Math.Ceiling((top + o.Height) / float source.TileHeight)) - 1))

                let path = at o.Layer x0 y0
                let label = if o.Name <> "" then o.Name else o.Type
                let props = o.Properties
                let kind = clipless (if o.Type <> "" then o.Type else o.Name)
                let point (key: string) = propAny [ key ] props

                let warnAt code message =
                    add Severity.Warning code message path x0 y0

                match kind with
                | "door"
                | "transition"
                | "warp"
                | "exit" ->
                    match propAny [ "to"; "scene"; "toScene"; "target" ] props with
                    | None -> add Severity.Error "import.doorTarget" (sprintf "Door '%s' has no `to` scene." label) path x0 y0
                    | Some toScene ->
                        let toScene = if toScene = "self" then sceneId else toScene

                        if
                            toScene <> sceneId
                            && not (project.Scenes |> List.exists (fun s -> s.Id = toScene))
                        then
                            warnAt "import.doorScene" (sprintf "Door '%s' leads to scene '%s', which the game doesn't have yet." label toScene)

                        let tx = point "toX" |> Option.map floatOf |> Option.defaultValue 0.0
                        let ty = point "toY" |> Option.map floatOf |> Option.defaultValue 0.0

                        for y in y0..y1 do
                            for x in x0..x1 do
                                transitions.Add
                                    { SceneTransition.Default with
                                        FromX = float x
                                        FromY = float y
                                        ToSceneId = toScene
                                        ToX = Math.Floor tx
                                        ToY = Math.Floor ty
                                        Locked = flag "locked" props |> Option.filter id
                                    }
                | "player"
                | "player-start"
                | "start"
                | "spawn" ->
                    player <-
                        { player with
                            X = float x0
                            Y = float y0
                            SceneId = sceneId
                        }

                    startScene <- sceneId
                | "npc"
                | "character" ->
                    let key =
                        propAny [ "npc"; "id"; "character" ] props
                        |> Option.defaultValue (
                            if o.Name <> "" && clipless o.Name <> kind then
                                o.Name
                            else
                                ""
                        )

                    match findNpc key with
                    | Some npc ->
                        npcs <-
                            npcs
                            |> List.map (fun n ->
                                if n.Id = npc.Id then
                                    { n with
                                        SceneId = sceneId
                                        X = float x0
                                        Y = float y0
                                    }
                                else
                                    n)

                        placedNpcs.Add npc.Id
                    | None -> warnAt "import.unknownNpc" (sprintf "No character '%s': add them in Characters, then import again." key)
                | "schedule"
                | "stop"
                | "schedule-stop" ->
                    let key = propAny [ "npc"; "character" ] props |> Option.defaultValue o.Name

                    match findNpc key, propAny [ "time"; "minute"; "at" ] props |> Option.bind minuteOf with
                    | None, _ -> warnAt "import.unknownNpc" (sprintf "No character '%s' for schedule stop '%s'." key label)
                    | _, None -> warnAt "import.stopTime" (sprintf "Schedule stop '%s' has no `time` (8:30 or 510)." label)
                    | Some npc, Some minute ->
                        let stop =
                            { NpcScheduleEntry.Default with
                                Minute = minute
                                SceneId = sceneId
                                X = float x0
                                Y = float y0
                            }

                        let replace (stops: NpcScheduleEntry list) =
                            (stops |> List.filter (fun s -> not (s.Minute = minute && s.SceneId = sceneId)))
                            @ [ stop ]
                            |> List.sortBy (fun s -> s.Minute)

                        npcs <-
                            npcs
                            |> List.map (fun n ->
                                if n.Id <> npc.Id then
                                    n
                                else
                                    match prop "variant" props with
                                    | Some variant when defaultArg n.Schedules [] |> List.exists (fun v -> v.Id = variant) ->
                                        { n with
                                            Schedules =
                                                n.Schedules
                                                |> Option.map (
                                                    List.map (fun v ->
                                                        if v.Id = variant then
                                                            { v with Stops = replace v.Stops }
                                                        else
                                                            v)
                                                )
                                        }
                                    | Some variant ->
                                        warnAt "import.unknownVariant" (sprintf "%s has no schedule '%s'." n.Name variant)
                                        n
                                    | None ->
                                        { n with
                                            Schedule = Some(replace (defaultArg n.Schedule []))
                                        })
                | "node" ->
                    let key = propAny [ "node"; "nodeType" ] props |> Option.defaultValue o.Name

                    match nodeTypes |> List.tryFind (fun t -> t.Id = key || t.Id = "node-" + clipless key) with
                    | Some nodeType -> nodes.[(x0, y0)] <- nodeType.Id
                    | None -> warnAt "import.unknownNode" (sprintf "No node type '%s'." key)
                | "machine" ->
                    let key = propAny [ "machine"; "machineType" ] props |> Option.defaultValue o.Name

                    match machineTypes |> List.tryFind (fun t -> t.Id = key || t.Id = clipless key) with
                    | Some machine -> machines.[(x0, y0)] <- machine.Id
                    | None -> warnAt "import.unknownMachine" (sprintf "No machine type '%s'." key)
                | "light"
                | "lamp" ->
                    let radius =
                        point "radius"
                        |> Option.map floatOf
                        |> Option.defaultValue 3.0
                        |> fun r -> max 1.0 (min 16.0 (Math.Round r))

                    let brightness =
                        point "brightness"
                        |> Option.map floatOf
                        |> Option.defaultValue 100.0
                        |> fun b -> max 1.0 (min 100.0 (Math.Round b))

                    let color = defaultArg (point "color") ""

                    let color =
                        if color.Length = 9 && color.StartsWith "#" then
                            "#" + color.Substring 3
                        else
                            color

                    let id =
                        sprintf "light-%g-%s-%g" radius (if color = "" then "warm" else clipless color) brightness

                    if not (terrainIndex.ContainsKey id) then
                        let terrain =
                            { TerrainDefinition.Default with
                                Id = id
                                Name = sprintf "Light (%g tiles)" radius
                                Layer = TileLayers.Overlay
                                Color = "transparent"
                                Light =
                                    Some
                                        { LightSource.Default with
                                            Radius = radius
                                            Color = color
                                            Brightness = brightness
                                        }
                            }

                        terrainIndex.[id] <- terrain
                        createdTerrains.Add terrain

                    lightCells.[y0 * width + x0] <- Some id
                | "event"
                | "trigger" ->
                    let key = propAny [ "event"; "id" ] props |> Option.defaultValue o.Name

                    let interact =
                        prop "trigger" props
                        |> Option.exists (fun t -> t.ToLowerInvariant() = "interact")

                    let area =
                        let x2, y2 =
                            (if x1 > x0 then Some(float x1) else None), (if y1 > y0 then Some(float y1) else None)

                        if interact then
                            EventCondition.InteractTile
                                {
                                    X = float x0
                                    Y = float y0
                                    X2 = x2
                                    Y2 = y2
                                }
                        else
                            EventCondition.EnterTile
                                {
                                    X = float x0
                                    Y = float y0
                                    X2 = x2
                                    Y2 = y2
                                }

                    let isArea (c: EventCondition) =
                        match c with
                        | EventCondition.EnterTile _
                        | EventCondition.InteractTile _ -> true
                        | _ -> false

                    let id =
                        if key = "" then
                            Defaults.nextId "event" (events |> List.map (fun e -> e.Id))
                        else
                            key

                    match events |> List.tryFind (fun e -> e.Id = id) with
                    | Some _ ->
                        events <-
                            events
                            |> List.map (fun e ->
                                if e.Id <> id then
                                    e
                                else
                                    { e with
                                        SceneId = sceneId
                                        Conditions = area :: (e.Conditions |> List.filter (isArea >> not))
                                    })
                    | None ->
                        events <-
                            events
                            @ [
                                { GameEvent.Default with
                                    Id = id
                                    Name = (if o.Name <> "" then o.Name else id)
                                    SceneId = sceneId
                                    Trigger =
                                        (if interact then
                                             EventTriggers.Interact
                                         else
                                             EventTriggers.Enter)
                                    Conditions = [ area ]
                                    Active = true
                                }
                            ]

                        add Severity.Info "import.newEvent" (sprintf "Event '%s' was created without outcomes: add them in Events." id) path x0 y0
                | _ ->
                    warnAt
                        "import.unmappedObject"
                        (sprintf "Object '%s' has no Tractr meaning: give it a type (door, player, npc, schedule, node, machine, light or event)." label)

            // ── The scene ──
            let allTerrains = terrainIndex.Values |> List.ofSeq

            let extraLayers =
                let layerFor (id: string) (above: bool) =
                    let known =
                        existingScene
                        |> Option.bind (fun s -> s.Layers)
                        |> Option.bind (List.tryFind (fun l -> l.Id = id))

                    match known with
                    | Some l ->
                        { l with
                            Above = (if above then Some true else None)
                        }
                    | None ->
                        { SceneLayer.Default with
                            Id = id
                            Name = id
                            Above = (if above then Some true else None)
                        }

                [
                    for id, above, _ in extras do
                        yield layerFor id above
                    if lightCells |> Array.exists Option.isSome then
                        yield layerFor "lights" false
                ]

            let oldTile (x: int) (y: int) : Tile option =
                existingScene
                |> Option.bind (fun s -> List.tryItem y s.Tiles)
                |> Option.bind (List.tryItem x)
                |> Option.filter (fun t -> int t.X = x && int t.Y = y)

            let tiles =
                [
                    for y in 0 .. height - 1 ->
                        [
                            for x in 0 .. width - 1 ->
                                let i = y * width + x
                                let old = oldTile x y
                                let fresh = AuthoringTiles.CreateEmptyTile(float x, float y, TileTypes.Grass)
                                let start = defaultArg old fresh
                                let bg = defaultArg background.[i] TileTypes.Grass
                                let ov = overlay.[i]
                                let ob = objectLayer.[i]

                                let blocks (id: string option) =
                                    id |> Option.exists (fun id -> not (AuthoringTiles.WalkableIn(allTerrains, id)))

                                let decor =
                                    [
                                        for id, _, cells in extras do
                                            match cells.[i] with
                                            | Some terrain -> yield id, terrain
                                            | None -> ()
                                        match lightCells.[i] with
                                        | Some terrain -> yield "lights", terrain
                                        | None -> ()
                                    ]

                                let node =
                                    match nodes.TryGetValue((x, y)) with
                                    | true, typeId ->
                                        match start.Node with
                                        | Some n when n.TypeId = typeId -> Some n
                                        | _ ->
                                            let health =
                                                nodeTypes
                                                |> List.tryFind (fun t -> t.Id = typeId)
                                                |> Option.map (fun t -> t.Health)

                                            Some
                                                { TileNode.Default with
                                                    TypeId = typeId
                                                    RemainingHealth = defaultArg health 1.0
                                                }
                                    | _ -> None

                                let machine =
                                    match machines.TryGetValue((x, y)) with
                                    | true, typeId ->
                                        match start.Machine with
                                        | Some m when m.TypeId = typeId -> Some m
                                        | _ ->
                                            Some
                                                { TileMachine.Default with
                                                    TypeId = typeId
                                                }
                                    | _ -> None

                                { start with
                                    X = float x
                                    Y = float y
                                    Type = (ob |> Option.orElse ov |> Option.defaultValue bg)
                                    Background = bg
                                    Overlay = ov
                                    Object = ob
                                    Collision = blocks (Some bg) || blocks ov || blocks ob
                                    Visuals = None
                                    Decor = (if decor.IsEmpty then None else Some decor)
                                    Node = node
                                    Machine = machine
                                }
                        ]
                ]

            let npcIds =
                let before = existingScene |> Option.map (fun s -> s.Npcs) |> Option.defaultValue []

                before
                @ (placedNpcs
                   |> Seq.filter (fun id -> not (List.contains id before))
                   |> Seq.distinct
                   |> List.ofSeq)

            let scene =
                { (defaultArg existingScene Scene.Default) with
                    Id = sceneId
                    Name =
                        (match existingScene with
                         | Some s -> s.Name
                         | None ->
                             if request.SceneName = "" then
                                 nn (Path.GetFileNameWithoutExtension request.Path)
                             else
                                 request.SceneName)
                    Width = float width
                    Height = float height
                    Tiles = tiles
                    Transitions = List.ofSeq transitions
                    Npcs = npcIds
                    Layers = (if extraLayers.IsEmpty then None else Some extraLayers)
                }

            let scenes =
                match existingScene with
                | Some _ -> project.Scenes |> List.map (fun s -> if s.Id = sceneId then scene else s)
                | None -> project.Scenes @ [ scene ]

            let terrains =
                let created = List.ofSeq createdTerrains

                let replaced =
                    projectTerrains
                    |> List.map (fun t ->
                        match created |> List.tryFind (fun c -> c.Id = t.Id) with
                        | Some c -> c
                        | None -> t)

                replaced
                @ (created
                   |> List.filter (fun c -> not (projectTerrains |> List.exists (fun t -> t.Id = c.Id))))

            // A re-import refreshes the terrains it made itself (their art and properties).
            let terrains =
                terrains
                |> List.map (fun t ->
                    match terrainIndex.TryGetValue t.Id with
                    | true, fresh when createdTerrains.Contains fresh -> fresh
                    | _ -> t)

            let importSource =
                { ImportSource.Default with
                    Kind = request.Kind
                    TargetId = sceneId
                    Path = request.Path
                    Level = request.Level
                }

            let imports =
                (defaultArg project.Imports []
                 |> List.filter (fun s ->
                     not (
                         (s.Kind = ImportKinds.Tiled || s.Kind = ImportKinds.Ldtk)
                         && s.TargetId = sceneId
                     )))
                @ [ importSource ]

            Ok
                {
                    Project =
                        { project with
                            Scenes = scenes
                            Terrains = (if terrains.IsEmpty then project.Terrains else Some terrains)
                            CustomAssets = List.ofSeq assets
                            Npcs = npcs
                            Events = events
                            Player = player
                            StartSceneId = startScene
                            Imports = Some imports
                        }
                    SceneId = sceneId
                    Created = existingScene.IsNone
                    Terrains = createdTerrains |> Seq.map (fun t -> t.Id) |> List.ofSeq
                    Report = List.ofSeq report
                }
