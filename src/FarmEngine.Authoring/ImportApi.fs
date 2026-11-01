namespace FarmEngine.Authoring

open System
open System.Collections.Generic
open System.IO
open FarmEngine.Schemas

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
        let extension = Path.GetExtension(fileName).ToLowerInvariant()

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
    static member PreviewSpriteSheet(project: GameProject, path: string, sheet: SpriteSheetSource, names: IEnumerable<KeyValuePair<string, string>>) : IReadOnlyList<ClipMapping> =
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
            |> List.tryFind (fun s -> (s.Kind = ImportKinds.Tiled || s.Kind = ImportKinds.Ldtk) && s.TargetId = sceneId)
        with
        | Some source -> source
        | None -> null

    /// The level identifiers of an LDtk project.
    static member LdtkLevels(text: string) : IReadOnlyList<string> =
        MapImport.ldtkLevels text |> ResizeArray :> IReadOnlyList<string>

    /// Whether `path` is an LDtk project (by its extension, or a `.json` holding LDtk's keys).
    static member IsLdtk(path: string, text: string) : bool =
        Path.GetExtension(path).ToLowerInvariant() = ".ldtk"
        || (Path.GetExtension(path).ToLowerInvariant() = ".json"
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
        (
            project: GameProject,
            path: string,
            text: string,
            level: string | null,
            source: MapSource,
            image: Func<string, ImportedImage | null>
        ) : MapImportResult =
        let request: MapImport.MapImportRequest =
            {
                Path = path
                Kind = (if SourceImports.IsLdtk(path, text) then ImportKinds.Ldtk else ImportKinds.Tiled)
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
