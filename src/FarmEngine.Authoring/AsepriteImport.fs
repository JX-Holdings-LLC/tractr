namespace FarmEngine.Authoring

open System
open System.IO
open System.IO.Compression
open System.Text
open FarmEngine.Schemas

/// One frame of a sprite sheet: where it is on the sheet and how long it shows.
type SheetFrame =
    {
        X: int
        Y: int
        Width: int
        Height: int
        DurationMs: int
    }

/// An Aseprite tag: frames `From`..`To` (0-based, inclusive) played in `Direction` (`forward`,
/// `reverse`, `pingpong`, `pingpong_reverse`). `Repeat` 0 loops forever.
type SheetTag =
    {
        Name: string
        From: int
        To: int
        Direction: string
        Repeat: int
    }

/// An Aseprite slice: a named rectangle of the first frame (an icon, a portrait).
type SheetSlice =
    {
        Name: string
        X: int
        Y: int
        Width: int
        Height: int
    }

/// What an Aseprite export or file holds: the sheet's frames, tags and slices. A `.aseprite`
/// file also brings its frames drawn onto one sheet (`Png`); an export names its sheet image
/// (`Image`), which the editor imports through its image checks.
type SpriteSheetSource =
    {
        Image: string option
        SheetWidth: int
        SheetHeight: int
        Frames: SheetFrame list
        Tags: SheetTag list
        Slices: SheetSlice list
        Png: byte[] option
        Warnings: string list
    }

/// One tag or slice and the clip it becomes, for the import preview. `Status` is `new`, `update`
/// (a clip of that name is replaced) or `kept` (the clip of an earlier import keeps its name).
type ClipMapping =
    {
        Source: string
        Clip: string
        IsSlice: bool
        Frames: int
        Status: string
    }

/// An Aseprite import to run: the file (`Path`, which re-import matches), the asset's name and
/// type when it is new, the sheet image as a PNG data URL with its size, and the clip names the
/// creator chose in the preview (`Names`: tag or `slice:` name → clip name; absent ones keep the
/// earlier import's name, else the Tractr name of the tag). `TargetId` re-imports into that asset.
type AsepriteRequest =
    {
        Path: string
        Name: string
        AssetType: string
        Sheet: SpriteSheetSource
        DataUrl: string
        Width: int
        Height: int
        Names: (string * string) list
        TargetId: string option
    }

/// What an import did.
type AsepriteImportResult =
    {
        Project: GameProject
        AssetId: string
        Created: bool
        Added: string list
        Updated: string list
        Removed: string list
        Warnings: string list
    }

/// Aseprite sprite sheets (#232): the JSON of a sheet export (array or hash form) and `.aseprite`
/// files, their tags as animation clips and their slices as single frames, and re-import that
/// updates the clips of the same asset so everything assigned to it keeps pointing at it.
[<CompilationRepresentation(CompilationRepresentationFlags.ModuleSuffix)>]
module AsepriteImport =
    /// The largest sheet side and area the editor imports (docs/dev/IMAGE-SAFETY.md).
    [<Literal>]
    let MaxSide = 8192

    [<Literal>]
    let MaxPixels = 16777216

    /// Most frames a file may have (a clip holds at most 1024).
    [<Literal>]
    let MaxFrames = 1024

    /// The prefix that keeps slice names apart from tag names in `ImportSource.Names`.
    [<Literal>]
    let SlicePrefix = "slice:"

    /// Game ticks a frame of `ms` milliseconds shows for (20 ticks a second), at least one.
    let ticks (ms: int) : float =
        max 1.0 (Math.Round(float (max 0 ms) / 50.0, MidpointRounding.AwayFromZero))

    /// The Tractr name of a tag or slice: lowercase words joined by `-` (`walk_down`, `Walk Down`
    /// and `walkDown` are all `walk-down`), `clip` when nothing is left.
    let clipName (name: string) : string =
        let sb = StringBuilder()
        let mutable dash = false
        let mutable previous = ' '

        for c in name.Trim() do
            if Char.IsLetterOrDigit c && c < '\u0080' then
                if Char.IsUpper c && (Char.IsLower previous || Char.IsDigit previous) then
                    dash <- true

                if dash && sb.Length > 0 then
                    sb.Append '-' |> ignore

                dash <- false
                sb.Append(Char.ToLowerInvariant c) |> ignore
            else
                dash <- true

            previous <- c

        if sb.Length = 0 then "clip" else sb.ToString()

    // ── JSON exports ───────────────────────────────────────────────────

    let private number (key: string) (json: Json) : int option =
        match Json.tryGet key json with
        | Some(JNumber n) when not (Double.IsNaN n) && abs n < 1e9 -> Some(int (Math.Truncate n))
        | Some(JString s) ->
            match Int32.TryParse s with
            | true, n -> Some n
            | _ -> None
        | _ -> None

    let private text (key: string) (json: Json) : string option =
        match Json.tryGet key json with
        | Some(JString s) -> Some s
        | _ -> None

    let private rect (json: Json) : (int * int * int * int) option =
        match number "x" json, number "y" json, number "w" json, number "h" json with
        | Some x, Some y, Some w, Some h when x >= 0 && y >= 0 && w > 0 && h > 0 -> Some(x, y, w, h)
        | _ -> None

    /// Reads the JSON Aseprite writes beside a sheet (File ▸ Export Sprite Sheet, or
    /// `aseprite --sheet s.png --data s.json`), with its frames as an array or as a hash.
    let parseJson (source: string) : Result<SpriteSheetSource, string> =
        match Json.parse source with
        | Error message -> Error("The JSON could not be read: " + message)
        | Ok json ->
            let frameEntries =
                match Json.tryGet "frames" json with
                | Some(JArray items) -> items
                | Some(JObject members) -> members |> List.map snd
                | _ -> []

            if frameEntries.IsEmpty then
                Error "This is not an Aseprite sheet export: it has no frames."
            elif frameEntries.Length > MaxFrames then
                Error(sprintf "The sheet has %d frames; at most %d can be imported." frameEntries.Length MaxFrames)
            else
                let warnings = ResizeArray<string>()

                let frames =
                    frameEntries
                    |> List.mapi (fun i entry ->
                        match Json.tryGet "frame" entry |> Option.bind rect with
                        | Some(x, y, w, h) ->
                            if Json.tryGet "trimmed" entry = Some(JBool true) then
                                warnings.Add(sprintf "Frame %d was trimmed: export without Trim so frames keep their place." i)

                            Some
                                {
                                    X = x
                                    Y = y
                                    Width = w
                                    Height = h
                                    DurationMs = defaultArg (number "duration" entry) 100
                                }
                        | None -> None)

                if frames |> List.exists Option.isNone then
                    Error "A frame of the sheet has no rectangle."
                else
                    let frames = frames |> List.choose id
                    let meta = defaultArg (Json.tryGet "meta" json) (JObject [])
                    let size = defaultArg (Json.tryGet "size" meta) (JObject [])

                    let tags =
                        match Json.tryGet "frameTags" meta with
                        | Some(JArray items) ->
                            items
                            |> List.choose (fun tag ->
                                match text "name" tag, number "from" tag, number "to" tag with
                                | Some name, Some from, Some until ->
                                    Some
                                        {
                                            Name = name
                                            From = from
                                            To = until
                                            Direction = defaultArg (text "direction" tag) "forward"
                                            Repeat = defaultArg (number "repeat" tag) 0
                                        }
                                | _ -> None)
                        | _ -> []

                    let first = frames.Head

                    let slices =
                        match Json.tryGet "slices" meta with
                        | Some(JArray items) ->
                            items
                            |> List.choose (fun slice ->
                                let bounds =
                                    match Json.tryGet "keys" slice with
                                    | Some(JArray(key :: _)) -> Json.tryGet "bounds" key |> Option.bind rect
                                    | _ -> None

                                match text "name" slice, bounds with
                                | Some name, Some(x, y, w, h) ->
                                    Some
                                        {
                                            Name = name
                                            X = first.X + x
                                            Y = first.Y + y
                                            Width = w
                                            Height = h
                                        }
                                | _ -> None)
                        | _ -> []

                    Ok
                        {
                            Image = text "image" meta
                            SheetWidth = defaultArg (number "w" size) 0
                            SheetHeight = defaultArg (number "h" size) 0
                            Frames = frames
                            Tags = tags
                            Slices = slices
                            Png = None
                            Warnings = List.ofSeq warnings
                        }

    // ── .aseprite files ────────────────────────────────────────────────

    type private Layer =
        {
            Flags: int
            Kind: int
            Child: int
            Blend: int
            Opacity: int
            Name: string
        }

    [<RequireQualifiedAccess>]
    type private CelData =
        | Pixels of width: int * height: int * data: byte[]
        | Linked of frame: int
        | Unsupported

    type private Cel =
        {
            Layer: int
            X: int
            Y: int
            Opacity: int
            Z: int
            Data: CelData
        }

    exception private Damaged of string

    let private damaged message = raise (Damaged message)

    type private Reader(bytes: byte[], start: int, finish: int) =
        let mutable pos = start

        member _.Position
            with get () = pos
            and set value = pos <- value

        member _.Left = finish - pos

        member _.Need(count: int) =
            if count < 0 || pos + count > finish then
                damaged "The file is cut short or damaged."

        member this.Byte() =
            this.Need 1
            pos <- pos + 1
            int bytes.[pos - 1]

        member this.Word() =
            this.Need 2
            pos <- pos + 2
            int bytes.[pos - 2] ||| (int bytes.[pos - 1] <<< 8)

        member this.Short() =
            let w = this.Word()
            if w >= 0x8000 then w - 0x10000 else w

        member this.DWord() : int64 =
            this.Need 4
            pos <- pos + 4

            int64 bytes.[pos - 4]
            ||| (int64 bytes.[pos - 3] <<< 8)
            ||| (int64 bytes.[pos - 2] <<< 16)
            ||| (int64 bytes.[pos - 1] <<< 24)

        member this.Long() : int =
            let d = this.DWord()
            int (if d >= 0x80000000L then d - 0x100000000L else d)

        member this.Skip(count: int) =
            this.Need count
            pos <- pos + count

        member this.Bytes(count: int) =
            this.Need count
            pos <- pos + count
            Array.sub bytes (pos - count) count

        member this.String() =
            let length = this.Word()
            Encoding.UTF8.GetString(this.Bytes length)

    let private inflate (data: byte[]) (expected: int) : byte[] =
        use input = new MemoryStream(data)
        use z = new ZLibStream(input, CompressionMode.Decompress)
        let output = Array.zeroCreate<byte> expected
        let mutable read = 0
        let mutable finished = false

        while not finished && read < expected do
            let n =
                try
                    z.Read(output, read, expected - read)
                with :? InvalidDataException ->
                    damaged "A cel's pixels are damaged."

            if n = 0 then finished <- true else read <- read + n

        if read < expected then
            damaged "A cel's pixels are cut short."

        output

    /// Straight-alpha "normal" blending of one pixel with `opacity` (0..255) over `dst`.
    let private blend (dst: byte[]) (at: int) (r: int) (g: int) (b: int) (a: int) (opacity: int) =
        let sa = a * opacity / 255

        if sa > 0 then
            let da = int dst.[at + 3]
            let keep = da * (255 - sa) / 255
            let outA = sa + keep

            let mixC (s: int) (d: int) = (s * sa + d * keep + outA / 2) / outA

            dst.[at] <- byte (mixC r (int dst.[at]))
            dst.[at + 1] <- byte (mixC g (int dst.[at + 1]))
            dst.[at + 2] <- byte (mixC b (int dst.[at + 2]))
            dst.[at + 3] <- byte outA

    /// Reads an `.aseprite` (or `.ase`) file: its layers composited into each frame (visible
    /// layers, normal blending, layer and cel opacity), the frames laid out left to right on one
    /// sheet (wrapping at 8192 pixels), its tags and its slices. RGBA, grayscale and indexed files
    /// are read; tilemap layers are left out with a warning.
    let parseFile (bytes: byte[]) : Result<SpriteSheetSource, string> =
        try
            if bytes.Length < 128 then
                damaged "This is not an Aseprite file."

            let header = Reader(bytes, 0, 128)
            header.Skip 4

            if header.Word() <> 0xA5E0 then
                damaged "This is not an Aseprite file."

            let frameCount = header.Word()
            let width = header.Word()
            let height = header.Word()
            let depth = header.Word()
            let flags = int (header.DWord())
            header.Skip 10
            let transparent = header.Byte()

            if frameCount < 1 then
                damaged "The file has no frames."

            if frameCount > MaxFrames then
                damaged (sprintf "The file has %d frames; at most %d can be imported." frameCount MaxFrames)

            if
                width < 1
                || height < 1
                || width > MaxSide
                || height > MaxSide
                || width * height > MaxPixels
            then
                damaged "Use sprites up to 8192 pixels per side and 16 megapixels."

            if depth <> 32 && depth <> 16 && depth <> 8 then
                damaged (sprintf "Colour depth %d is not one Aseprite writes." depth)

            let bpp = depth / 8
            let warnings = ResizeArray<string>()
            let layers = ResizeArray<Layer>()
            let palette = Array.create 256 (0, 0, 0, 255)
            let mutable tags: SheetTag list = []
            let slices = ResizeArray<SheetSlice>()
            let durations = Array.zeroCreate<int> frameCount
            let cels = Array.init frameCount (fun _ -> ResizeArray<Cel>())
            let mutable position = 128

            for frame in 0 .. frameCount - 1 do
                let r = Reader(bytes, position, bytes.Length)
                let frameBytes = r.DWord()

                if frameBytes < 16L || int64 position + frameBytes > int64 bytes.Length then
                    damaged "A frame of the file is cut short."

                let frameEnd = position + int frameBytes
                let r = Reader(bytes, position + 4, frameEnd)

                if r.Word() <> 0xF1FA then
                    damaged "A frame of the file is damaged."

                let oldChunks = r.Word()
                durations.[frame] <- r.Word()
                r.Skip 2
                let newChunks = r.DWord()
                let chunks = if newChunks = 0L then int64 oldChunks else newChunks
                let mutable chunkStart = r.Position
                let mutable chunk = 0L

                while chunk < chunks && chunkStart + 6 <= frameEnd do
                    let c = Reader(bytes, chunkStart, frameEnd)
                    let size = c.DWord()

                    if size < 6L || int64 chunkStart + size > int64 frameEnd then
                        damaged "A chunk of the file is damaged."

                    let chunkEnd = chunkStart + int size
                    let c = Reader(bytes, chunkStart + 4, chunkEnd)
                    let kind = c.Word()

                    match kind with
                    | 0x2004 ->
                        let layerFlags = c.Word()
                        let layerKind = c.Word()
                        let child = c.Word()
                        c.Skip 4
                        let blendMode = c.Word()
                        let opacity = c.Byte()
                        c.Skip 3
                        let name = c.String()

                        layers.Add
                            {
                                Flags = layerFlags
                                Kind = layerKind
                                Child = child
                                Blend = blendMode
                                Opacity = (if flags &&& 1 <> 0 then opacity else 255)
                                Name = name
                            }
                    | 0x2005 ->
                        let layer = c.Word()
                        let x = c.Short()
                        let y = c.Short()
                        let opacity = c.Byte()
                        let celKind = c.Word()
                        let z = c.Short()
                        c.Skip 5

                        let data =
                            match celKind with
                            | 0
                            | 2 ->
                                let w = c.Word()
                                let h = c.Word()

                                if w > MaxSide || h > MaxSide || w * h > MaxPixels then
                                    damaged "A cel of the file is too large."

                                let expected = w * h * bpp

                                let pixels =
                                    if celKind = 0 then
                                        c.Bytes expected
                                    else
                                        inflate (c.Bytes c.Left) expected

                                CelData.Pixels(w, h, pixels)
                            | 1 -> CelData.Linked(c.Word())
                            | _ -> CelData.Unsupported

                        cels.[frame].Add
                            {
                                Layer = layer
                                X = x
                                Y = y
                                Opacity = opacity
                                Z = z
                                Data = data
                            }
                    | 0x2018 ->
                        let count = c.Word()
                        c.Skip 8

                        tags <-
                            [
                                for _ in 1..count do
                                    let from = c.Word()
                                    let until = c.Word()
                                    let direction = c.Byte()
                                    let repeat = c.Word()
                                    c.Skip 10
                                    let name = c.String()

                                    yield
                                        {
                                            Name = name
                                            From = from
                                            To = until
                                            Direction =
                                                (match direction with
                                                 | 1 -> "reverse"
                                                 | 2 -> "pingpong"
                                                 | 3 -> "pingpong_reverse"
                                                 | _ -> "forward")
                                            Repeat = repeat
                                        }
                            ]
                    | 0x2019 ->
                        c.Skip 4
                        let first = int (c.DWord())
                        let last = int (c.DWord())
                        c.Skip 8

                        for index in first..last do
                            let entryFlags = c.Word()
                            let red = c.Byte()
                            let green = c.Byte()
                            let blue = c.Byte()
                            let alpha = c.Byte()

                            if index >= 0 && index < 256 then
                                palette.[index] <- (red, green, blue, alpha)

                            if entryFlags &&& 1 <> 0 then
                                c.String() |> ignore
                    | 0x0004
                    | 0x0011 ->
                        // The old palette chunks, read only when no new one came first.
                        let packets = c.Word()
                        let mutable index = 0

                        for _ in 1..packets do
                            index <- index + c.Byte()
                            let count = c.Byte()
                            let count = if count = 0 then 256 else count

                            for _ in 1..count do
                                let scale (v: int) =
                                    if kind = 0x0011 then v * 255 / 63 else v

                                let red = scale (c.Byte())
                                let green = scale (c.Byte())
                                let blue = scale (c.Byte())

                                if index < 256 then
                                    let _, _, _, a = palette.[index]

                                    if a = 255 then
                                        palette.[index] <- (red, green, blue, 255)

                                index <- index + 1
                    | 0x2022 ->
                        let keys = c.DWord()
                        let sliceFlags = c.DWord()
                        c.Skip 4
                        let name = c.String()

                        if keys >= 1L then
                            c.Skip 4
                            let x = c.Long()
                            let y = c.Long()
                            let w = int (c.DWord())
                            let h = int (c.DWord())

                            if w > 0 && h > 0 && x >= 0 && y >= 0 && x + w <= width && y + h <= height then
                                slices.Add
                                    {
                                        Name = name
                                        X = x
                                        Y = y
                                        Width = w
                                        Height = h
                                    }
                            else
                                warnings.Add(sprintf "Slice '%s' reaches outside the sprite and was left out." name)

                            ignore sliceFlags
                    | _ -> ()

                    chunkStart <- chunkEnd
                    chunk <- chunk + 1L

                position <- frameEnd

            // Layer visibility, with the groups each layer sits in.
            let visible = Array.create layers.Count true
            let parents = Collections.Generic.List<int * bool>()

            for index in 0 .. layers.Count - 1 do
                let layer = layers.[index]

                while parents.Count > 0 && fst parents.[parents.Count - 1] >= layer.Child do
                    parents.RemoveAt(parents.Count - 1)

                let parentVisible = parents.Count = 0 || snd parents.[parents.Count - 1]
                let shown = parentVisible && layer.Flags &&& 1 <> 0
                visible.[index] <- shown

                if layer.Kind = 1 then
                    parents.Add(layer.Child, shown)

            for index in 0 .. layers.Count - 1 do
                let layer = layers.[index]

                if visible.[index] && layer.Kind = 2 then
                    warnings.Add(sprintf "Tilemap layer '%s' was left out: flatten it in Aseprite to import it." layer.Name)

                if visible.[index] && layer.Kind = 0 && layer.Blend <> 0 then
                    warnings.Add(sprintf "Layer '%s' uses a blend mode; it was drawn as Normal." layer.Name)

            let columns = max 1 (min frameCount (MaxSide / width))
            let rows = (frameCount + columns - 1) / columns

            if rows * height > MaxSide || columns * width * rows * height > MaxPixels then
                damaged "The frames do not fit on one sheet of 8192 pixels a side and 16 megapixels."

            let sheetWidth = columns * width
            let sheetHeight = rows * height
            let sheet = Array.zeroCreate<byte>(sheetWidth * sheetHeight * 4)

            let pixelAt (data: byte[]) (i: int) =
                match bpp with
                | 4 -> int data.[i * 4], int data.[i * 4 + 1], int data.[i * 4 + 2], int data.[i * 4 + 3]
                | 2 ->
                    let v = int data.[i * 2]
                    v, v, v, int data.[i * 2 + 1]
                | _ ->
                    let index = int data.[i]

                    if index = transparent then 0, 0, 0, 0 else palette.[index]

            // A linked cel shows the cel of its layer in another frame.
            let rec resolve (cel: Cel) (hops: int) =
                match cel.Data with
                | CelData.Linked other when hops < 4 && other >= 0 && other < frameCount ->
                    cels.[other]
                    |> Seq.tryFind (fun c -> c.Layer = cel.Layer)
                    |> Option.bind (fun target -> resolve target (hops + 1))
                | CelData.Pixels _ -> Some cel
                | _ -> None

            for frame in 0 .. frameCount - 1 do
                let ox = (frame % columns) * width
                let oy = (frame / columns) * height

                let ordered =
                    cels.[frame]
                    |> Seq.indexed
                    |> Seq.sortBy (fun (i, cel) -> cel.Layer + cel.Z, cel.Z, i)
                    |> Seq.map snd

                for cel in ordered do
                    let layerOk =
                        cel.Layer >= 0
                        && cel.Layer < layers.Count
                        && visible.[cel.Layer]
                        && layers.[cel.Layer].Kind = 0

                    if layerOk then
                        match resolve cel 0 with
                        | Some({ Data = CelData.Pixels(w, h, data) } as source) ->
                            let opacity = cel.Opacity * layers.[cel.Layer].Opacity / 255

                            for py in 0 .. h - 1 do
                                let y = source.Y + py

                                if y >= 0 && y < height then
                                    for px in 0 .. w - 1 do
                                        let x = source.X + px

                                        if x >= 0 && x < width then
                                            let r, g, b, a = pixelAt data (py * w + px)
                                            let at = ((oy + y) * sheetWidth + ox + x) * 4
                                            blend sheet at r g b a opacity
                        | _ -> ()

            let frames =
                [
                    for frame in 0 .. frameCount - 1 ->
                        {
                            X = (frame % columns) * width
                            Y = (frame / columns) * height
                            Width = width
                            Height = height
                            DurationMs = durations.[frame]
                        }
                ]

            Ok
                {
                    Image = None
                    SheetWidth = sheetWidth
                    SheetHeight = sheetHeight
                    Frames = frames
                    Tags = tags
                    Slices = List.ofSeq slices
                    Png = Some(Png.encodeRgba sheetWidth sheetHeight sheet)
                    Warnings = List.ofSeq warnings
                }
        with Damaged message ->
            Error message

    // ── Clips ──────────────────────────────────────────────────────────

    /// The frame indices a tag plays, in order.
    let tagFrames (frameCount: int) (tag: SheetTag) : int list =
        let from = max 0 (min tag.From tag.To)
        let until = min (frameCount - 1) (max tag.From tag.To)

        if from > until then
            []
        else
            let forward = [ from..until ]
            let backward = List.rev forward

            let inner (xs: int list) =
                if xs.Length <= 2 then
                    []
                else
                    xs |> List.tail |> List.take (xs.Length - 2)

            match tag.Direction with
            | "reverse" -> backward
            | "pingpong" -> forward @ inner backward
            | "pingpong_reverse" -> backward @ inner forward
            | _ -> forward

    /// The sources of a sheet, in order: its tags, then its slices (`slice:` names). A sheet
    /// with neither is one clip of every frame (`idle`).
    let private sources (sheet: SpriteSheetSource) : (string * bool * ArtFrame list * bool) list =
        let frame (f: SheetFrame) =
            { ArtFrame.Default with
                X = float f.X
                Y = float f.Y
                Width = float f.Width
                Height = float f.Height
                Ticks = ticks f.DurationMs
            }

        let frames = Array.ofList sheet.Frames

        let tags =
            sheet.Tags
            |> List.map (fun tag -> tag.Name, false, tagFrames frames.Length tag |> List.map (fun i -> frame frames.[i]), tag.Repeat = 0)
            |> List.filter (fun (_, _, frames, _) -> not frames.IsEmpty)

        let slices =
            sheet.Slices
            |> List.map (fun slice ->
                SlicePrefix + slice.Name,
                true,
                [
                    { ArtFrame.Default with
                        X = float slice.X
                        Y = float slice.Y
                        Width = float slice.Width
                        Height = float slice.Height
                        Ticks = 6.0
                    }
                ],
                false)

        match tags @ slices with
        | [] when frames.Length > 0 -> [ "idle", false, frames |> Array.map frame |> List.ofArray, true ]
        | all -> all

    let private displayName (source: string) =
        if source.StartsWith(SlicePrefix, StringComparison.Ordinal) then
            source.Substring SlicePrefix.Length
        else
            source

    /// The earlier import of `path` (or into `targetId`) whose asset still exists.
    let previousImport (project: GameProject) (path: string) (targetId: string option) : ImportSource option =
        let exists (id: string) =
            project.CustomAssets |> List.exists (fun a -> a.Id = id)

        defaultArg project.Imports []
        |> List.tryFind (fun source ->
            source.Kind = ImportKinds.Aseprite
            && exists source.TargetId
            && (match targetId with
                | Some id -> source.TargetId = id
                | None -> String.Equals(source.Path, path, StringComparison.OrdinalIgnoreCase)))

    /// Each tag or slice and its clip name: the creator's choice, else the earlier import's, else
    /// the Tractr name; names stay unique among the clips the asset keeps.
    let private chooseNames (sheet: SpriteSheetSource) (chosen: (string * string) list) (previous: (string * string) list) (asset: CustomAsset option) =
        let previousClips = previous |> List.map snd |> Set.ofList

        let handMade =
            asset
            |> Option.bind (fun a -> a.Animations)
            |> Option.defaultValue []
            |> List.map (fun c -> c.Name)
            |> List.filter (fun name -> not (previousClips.Contains name))
            |> Set.ofList

        let lookup (key: string) (pairs: (string * string) list) =
            pairs |> List.tryFind (fun (k, _) -> k = key) |> Option.map snd

        let taken = Collections.Generic.HashSet<string>()

        sources sheet
        |> List.map (fun (source, isSlice, frames, loops) ->
            let explicitName =
                lookup source chosen
                |> Option.orElse (lookup source previous)
                |> Option.map (fun name -> name.Trim())
                |> Option.filter (fun name -> name.Length > 0)

            let wanted = defaultArg explicitName (clipName (displayName source))

            let rec unique (name: string) n =
                let candidate = if n = 1 then name else sprintf "%s-%d" name n

                // A hand-made clip of the same name is replaced only when the creator asked for it.
                if taken.Contains candidate || (explicitName.IsNone && handMade.Contains candidate) then
                    unique name (n + 1)
                else
                    candidate

            let name = unique wanted 1
            taken.Add name |> ignore
            source, name, isSlice, frames, loops)

    /// The preview of importing `sheet` from `path`: each tag and slice with the clip it becomes.
    let preview (project: GameProject) (path: string) (targetId: string option) (sheet: SpriteSheetSource) (chosen: (string * string) list) : ClipMapping list =
        let previous = previousImport project path targetId

        let asset =
            previous
            |> Option.bind (fun source -> project.CustomAssets |> List.tryFind (fun a -> a.Id = source.TargetId))
            |> Option.orElse (
                targetId
                |> Option.bind (fun id -> project.CustomAssets |> List.tryFind (fun a -> a.Id = id))
            )

        let previousNames =
            previous |> Option.bind (fun s -> s.Names) |> Option.defaultValue []

        let existing =
            asset
            |> Option.bind (fun a -> a.Animations)
            |> Option.defaultValue []
            |> List.map (fun c -> c.Name)
            |> Set.ofList

        chooseNames sheet chosen previousNames asset
        |> List.map (fun (source, name, isSlice, frames, _) ->
            {
                Source = displayName source
                Clip = name
                IsSlice = isSlice
                Frames = frames.Length
                Status =
                    (if previousNames |> List.exists (fun (k, v) -> k = source && v = name) then
                         "kept"
                     elif existing.Contains name then
                         "update"
                     else
                         "new")
            })

    /// Imports `request` into `project`: a new asset, or the asset an earlier import of the same
    /// file (or `TargetId`) made. Re-import keeps the asset's id, name, type and the clips made by
    /// hand; replaces the image and the frames of the imported clips (keeping each clip's place
    /// and its Loop setting); adds clips for new tags; and removes the clips of tags that are gone.
    let import (project: GameProject) (request: AsepriteRequest) : AsepriteImportResult =
        let previous = previousImport project request.Path request.TargetId

        let targetId =
            previous
            |> Option.map (fun s -> s.TargetId)
            |> Option.orElse (
                request.TargetId
                |> Option.filter (fun id -> project.CustomAssets |> List.exists (fun a -> a.Id = id))
            )

        let existing =
            targetId
            |> Option.bind (fun id -> project.CustomAssets |> List.tryFind (fun a -> a.Id = id))

        let previousNames =
            previous |> Option.bind (fun s -> s.Names) |> Option.defaultValue []

        let chosen = chooseNames request.Sheet request.Names previousNames existing

        let imported =
            chosen
            |> List.map (fun (_, name, _, frames, loops) ->
                name,
                {
                    Name = name
                    Loop = loops
                    Frames = frames
                })

        let importedNames = imported |> List.map fst |> Set.ofList
        let previousClips = previousNames |> List.map snd |> Set.ofList

        let oldClips =
            existing |> Option.bind (fun a -> a.Animations) |> Option.defaultValue []

        let kept =
            oldClips
            |> List.choose (fun clip ->
                match imported |> List.tryFind (fun (name, _) -> name = clip.Name) with
                | Some(_, fresh) -> Some { fresh with Loop = clip.Loop }
                | None when previousClips.Contains clip.Name -> None
                | None -> Some clip)

        let oldNames = oldClips |> List.map (fun c -> c.Name) |> Set.ofList

        let added =
            imported
            |> List.filter (fun (name, _) -> not (oldNames.Contains name))
            |> List.map snd

        let clips = kept @ added

        let removed =
            oldClips
            |> List.filter (fun c -> previousClips.Contains c.Name && not (importedNames.Contains c.Name))
            |> List.map (fun c -> c.Name)

        let updated =
            oldClips
            |> List.filter (fun c -> importedNames.Contains c.Name)
            |> List.map (fun c -> c.Name)

        let id =
            match existing with
            | Some asset -> asset.Id
            | None -> Defaults.nextId "art" (project.CustomAssets |> List.map (fun a -> a.Id))

        let asset =
            match existing with
            | Some asset ->
                { asset with
                    DataUrl = request.DataUrl
                    Width = Some(float request.Width)
                    Height = Some(float request.Height)
                    Animations = Some clips
                }
            | None ->
                { CustomAsset.Default with
                    Id = id
                    Name = request.Name
                    Type = request.AssetType
                    DataUrl = request.DataUrl
                    Width = Some(float request.Width)
                    Height = Some(float request.Height)
                    Animations = Some clips
                }

        let assets =
            match existing with
            | Some _ -> project.CustomAssets |> List.map (fun a -> if a.Id = id then asset else a)
            | None -> project.CustomAssets @ [ asset ]

        let source =
            { ImportSource.Default with
                Kind = ImportKinds.Aseprite
                TargetId = id
                Path = request.Path
                Names = Some(chosen |> List.map (fun (source, name, _, _, _) -> source, name))
            }

        let imports =
            let others =
                defaultArg project.Imports []
                |> List.filter (fun s -> not (s.Kind = ImportKinds.Aseprite && s.TargetId = id))

            others @ [ source ]

        let sizeWarning =
            if
                request.Sheet.SheetWidth > 0
                && (request.Sheet.SheetWidth <> request.Width
                    || request.Sheet.SheetHeight <> request.Height)
            then
                [
                    sprintf
                        "The JSON describes a %d×%d sheet but the image is %d×%d."
                        request.Sheet.SheetWidth
                        request.Sheet.SheetHeight
                        request.Width
                        request.Height
                ]
            else
                []

        let outside =
            chosen
            |> List.collect (fun (_, name, _, frames, _) ->
                if
                    frames
                    |> List.exists (fun f -> f.X + f.Width > float request.Width || f.Y + f.Height > float request.Height)
                then
                    [ sprintf "Clip '%s' has frames outside the image." name ]
                else
                    [])

        {
            Project =
                { project with
                    CustomAssets = assets
                    Imports = Some imports
                }
            AssetId = id
            Created = existing.IsNone
            Added = added |> List.map (fun c -> c.Name)
            Updated = updated
            Removed = removed
            Warnings = request.Sheet.Warnings @ sizeWarning @ outside
        }
