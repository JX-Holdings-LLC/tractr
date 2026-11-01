namespace FarmEngine.Authoring

open System
open System.Collections.Generic
open System.IO
open FarmEngine.Schemas

module internal ImportPaths =
    /// The lowercase extension of `path` (`.tmx`), empty without one.
    let extension (path: string) =
        match Path.GetExtension path with
        | null -> ""
        | e -> e.ToLowerInvariant()

/// An image the editor imported through its image checks (`ArtImport`), for a map's tileset.
[<AllowNullLiteral>]
type ImportedImage(dataUrl: string, width: int, height: int) =
    member _.DataUrl = dataUrl
    member _.Width = width
    member _.Height = height

/// The C# boundary of the Aseprite, Tiled and LDtk importers (#232, #233). Reading a file that
/// isn't what it claims throws `ArgumentException` with the reason a creator can act on.
[<AbstractClass; Sealed>]
type SourceImports =
    /// File patterns of sprite sheets: Aseprite files and the JSON of a sheet export.
    static member SpriteSheetPatterns: string list = [ "*.aseprite"; "*.ase"; "*.json" ]

    /// File patterns of maps: Tiled maps and LDtk projects.
    static member MapPatterns: string list = [ "*.tmx"; "*.tmj"; "*.json"; "*.ldtk" ]

    /// Reads a `.aseprite`/`.ase` file or the JSON of an Aseprite sheet export.
    static member ReadSpriteSheet(fileName: string, bytes: byte[]) : SpriteSheetSource =
        let extension = ImportPaths.extension fileName

        let result =
            if extension = ".json" then
                AsepriteImport.parseJson (Text.Encoding.UTF8.GetString bytes)
            else
                AsepriteImport.parseFile bytes

        match result with
        | Ok sheet -> sheet
        | Error message -> invalidArg "bytes" message

    /// The PNG data URL of a `.aseprite` file's frames (absent for a JSON export, whose sheet
    /// image the editor imports itself).
    static member SheetDataUrl(sheet: SpriteSheetSource) : string | null =
        match sheet.Png with
        | Some png -> Png.dataUrl png
        | None -> null

    /// Each tag and slice with the clip it would become (the import preview).
    static member PreviewSpriteSheet
        (project: GameProject, path: string, sheet: SpriteSheetSource, names: IEnumerable<KeyValuePair<string, string>>)
        : IReadOnlyList<ClipMapping> =
        AsepriteImport.preview project path None sheet [ for pair in names -> pair.Key, pair.Value ]
        |> ResizeArray
        :> IReadOnlyList<ClipMapping>

    /// Imports (or re-imports) a sprite sheet; `names` are the clip names chosen in the preview.
    static member ImportSpriteSheet
        (
            project: GameProject,
            path: string,
            name: string,
            assetType: string,
            sheet: SpriteSheetSource,
            dataUrl: string,
            width: int,
            height: int,
            names: IEnumerable<KeyValuePair<string, string>>
        ) : AsepriteImportResult =
        AsepriteImport.import
            project
            {
                Path = path
                Name = name
                AssetType = assetType
                Sheet = sheet
                DataUrl = dataUrl
                Width = width
                Height = height
                Names = [ for pair in names -> pair.Key, pair.Value ]
                TargetId = None
            }

    /// Every import record of `kind` whose target still exists (the files the editor watches).
    static member Sources(project: GameProject, kind: string) : IReadOnlyList<ImportSource> =
        defaultArg project.Imports []
        |> List.filter (fun s ->
            s.Kind = kind
            && (project.CustomAssets |> List.exists (fun a -> a.Id = s.TargetId)
                || project.Scenes |> List.exists (fun x -> x.Id = s.TargetId)))
        |> ResizeArray
        :> IReadOnlyList<ImportSource>

    /// The import record of a scene, for Re-import (null when it wasn't imported).
    static member SceneSource(project: GameProject, sceneId: string) : ImportSource | null =
        match
            defaultArg project.Imports []
            |> List.tryFind (fun s ->
                (s.Kind = ImportKinds.Tiled || s.Kind = ImportKinds.Ldtk)
                && s.TargetId = sceneId)
        with
        | Some source -> source
        | None -> null

    /// The level identifiers of an LDtk project.
    static member LdtkLevels(text: string) : IReadOnlyList<string> =
        MapImport.ldtkLevels text |> ResizeArray :> IReadOnlyList<string>

    /// Whether `path` is an LDtk project (by its extension, or a `.json` holding LDtk's keys).
    static member IsLdtk(path: string, text: string) : bool =
        ImportPaths.extension path = ".ldtk"
        || (ImportPaths.extension path = ".json"
            && text.Contains "\"jsonVersion\""
            && text.Contains "\"levels\"")

    /// Reads a map file; `read` loads the files beside it (external tilesets, LDtk levels) by
    /// full path, null when it can't.
    static member ReadMap(path: string, text: string, level: string | null, read: Func<string, string | null>) : MapSource =
        let reader (file: string) =
            match read.Invoke file with
            | null -> None
            | t -> Some t

        let result =
            if SourceImports.IsLdtk(path, text) then
                MapImport.parseLdtk path text (Option.ofObj level) reader
            else
                MapImport.parseTiled path text reader

        match result with
        | Ok source -> source
        | Error message -> invalidArg "text" message

    /// Imports (or re-imports) a map as a scene; `image` imports a tileset image by full path.
    static member ImportMap
        (project: GameProject, path: string, text: string, level: string | null, source: MapSource, image: Func<string, ImportedImage>)
        : MapImportResult =
        let request: MapImport.MapImportRequest =
            {
                Path = path
                Kind =
                    (if SourceImports.IsLdtk(path, text) then
                         ImportKinds.Ldtk
                     else
                         ImportKinds.Tiled)
                Level = Option.ofObj level
                SceneId = None
                SceneName =
                    match Option.ofObj level with
                    | Some l -> l
                    | None ->
                        match Path.GetFileNameWithoutExtension path with
                        | null -> "map"
                        | name -> name
                Image =
                    fun file ->
                        match image.Invoke file with
                        | null -> None
                        | picture -> Some(picture.DataUrl, picture.Width, picture.Height)
            }

        match MapImport.import project request source with
        | Ok result -> result
        | Error message -> invalidArg "source" message

/// The Interface tab's skin form (#236): each control changes one part of the skin, and empty
/// values leave that part out (the built-in look).
[<AbstractClass; Sealed>]
type SkinForm =
    static member private Set (key: string) (value: 'T option) (pairs: (string * 'T) list option) =
        let rest = defaultArg pairs [] |> List.filter (fun (k, _) -> k <> key)

        let all =
            match value with
            | Some v -> rest @ [ key, v ]
            | None -> rest

        if all.IsEmpty then None else Some all

    static member private Text(value: string | null) =
        match value with
        | null -> None
        | v when String.IsNullOrWhiteSpace v -> None
        | v -> Some(v.Trim())

    /// The project's skin, or an empty one.
    static member Current(project: GameProject) : UiSkin =
        defaultArg project.UiSkin UiSkin.Default

    /// The font assets a skin can use.
    static member Fonts(project: GameProject) : IReadOnlyList<CustomAsset> =
        project.CustomAssets
        |> List.filter (fun a -> a.Type = CustomAssetTypes.Font)
        |> ResizeArray
        :> IReadOnlyList<CustomAsset>

    /// A font asset from a `.ttf` or `.otf` file; throws `ArgumentException` when it isn't one.
    static member ReadFont(project: GameProject, fileName: string, bytes: byte[]) : CustomAsset =
        match FontFile.coverage bytes with
        | Error why -> invalidArg "bytes" (sprintf "%s can't be used: %s." fileName why)
        | Ok _ ->
            let mime =
                if ImportPaths.extension fileName = ".otf" then
                    "font/otf"
                else
                    "font/ttf"

            { CustomAsset.Default with
                Id = Defaults.nextId "font" (project.CustomAssets |> List.map (fun a -> a.Id))
                Name = fileName
                Type = CustomAssetTypes.Font
                DataUrl = sprintf "data:%s;base64,%s" mime (Convert.ToBase64String bytes)
            }

    /// `role` is `heading`, `body` or `number`; null or empty uses the built-in font.
    static member WithFont(skin: UiSkin, role: string, assetId: string | null) : UiSkin =
        let id = SkinForm.Text assetId

        match role with
        | "heading" -> { skin with HeadingFont = id }
        | "number" -> { skin with NumberFont = id }
        | _ -> { skin with BodyFont = id }

    static member WithColor(skin: UiSkin, role: string, value: string | null) : UiSkin =
        { skin with
            Colors = SkinForm.Set role (SkinForm.Text value) skin.Colors
        }

    /// Panel art for `part` from `assetId` (null: none) with `edge` source pixels on every side.
    static member WithPanel(skin: UiSkin, part: string, assetId: string | null, edge: float, scale: float) : UiSkin =
        let slice =
            SkinForm.Text assetId
            |> Option.map (fun id ->
                { UiNineSlice.Default with
                    Visual = { VisualRef.Default with AssetId = id }
                    Left = edge
                    Top = edge
                    Right = edge
                    Bottom = edge
                    Scale = (if scale = 1.0 then None else Some scale)
                })

        { skin with
            Panels = SkinForm.Set part slice skin.Panels
        }

    static member WithIcon(skin: UiSkin, name: string, assetId: string | null) : UiSkin =
        let visual =
            SkinForm.Text assetId
            |> Option.map (fun id -> { VisualRef.Default with AssetId = id })

        { skin with
            Icons = SkinForm.Set name visual skin.Icons
        }

    /// The layout; null values are the defaults.
    static member WithLayout
        (skin: UiSkin, hudPosition: string | null, toolbar: bool, clockStyle: string | null, dialoguePosition: string | null, portraitSize: float)
        : UiSkin =
        let layout =
            {
                HudPosition = SkinForm.Text hudPosition |> Option.filter (fun v -> v <> UiHudPositions.Top)
                Toolbar = (if toolbar then None else Some false)
                ClockStyle =
                    SkinForm.Text clockStyle
                    |> Option.filter (fun v -> v <> UiClockStyles.TwelveHour)
                DialoguePosition =
                    SkinForm.Text dialoguePosition
                    |> Option.filter (fun v -> v <> UiDialoguePositions.Bottom)
                PortraitSize =
                    (if portraitSize = 48.0 || portraitSize <= 0.0 then
                         None
                     else
                         Some portraitSize)
            }

        { skin with
            Layout = (if layout = UiLayout.Default then None else Some layout)
        }

    /// The title screen; null values are the defaults.
    static member WithTitle(skin: UiSkin, background: string | null, logo: string | null, scene: string | null, menuPosition: string | null) : UiSkin =
        let visual (id: string | null) =
            SkinForm.Text id
            |> Option.map (fun id -> { VisualRef.Default with AssetId = id })

        let title =
            {
                Background = visual background
                Logo = visual logo
                Scene = SkinForm.Text scene
                MenuPosition =
                    SkinForm.Text menuPosition
                    |> Option.filter (fun v -> v <> UiMenuPositions.Center)
            }

        { skin with
            Title = (if title = UiTitleScreen.Default then None else Some title)
        }

    /// The asset id of a panel part, an icon or a colour already in `skin` (empty when unset).
    static member PanelAsset(skin: UiSkin, part: string) : string =
        defaultArg skin.Panels []
        |> List.tryFind (fun (k, _) -> k = part)
        |> Option.map (fun (_, s) -> s.Visual.AssetId)
        |> Option.defaultValue ""

    static member IconAsset(skin: UiSkin, name: string) : string =
        defaultArg skin.Icons []
        |> List.tryFind (fun (k, _) -> k = name)
        |> Option.map (fun (_, v) -> v.AssetId)
        |> Option.defaultValue ""

    static member Color(skin: UiSkin, role: string) : string =
        defaultArg skin.Colors []
        |> List.tryFind (fun (k, _) -> k = role)
        |> Option.map snd
        |> Option.defaultValue ""

    /// Whether the skin sets nothing (Save then stores no skin at all).
    static member IsEmpty(skin: UiSkin) : bool = skin = UiSkin.Default
