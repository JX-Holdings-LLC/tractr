namespace FarmEngine.Authoring.Net

open System
open System.Collections.Generic
open System.IO
open FarmEngine.Authoring
open FarmEngine.Schemas

/// Projects on disk: the file I/O the pure core (`ProjectLoad`, `ProjectFolder`) leaves to its
/// host. A project is a folder (`project.json` and its `scenes/`, `npcs/`, … files), the
/// `project.json` of one, or a single-file project JSON. farmc reads and writes projects through
/// this module; a script that references `FarmEngine.Authoring.Net.dll` can too:
///
/// ```fsharp
/// match ProjectFiles.load "path/to/WillowCreek" with
/// | Ok project -> Problems.collect project
/// | Error errors -> ...
/// ```
module ProjectFiles =
    /// The folder `path` names as a project folder: the folder itself, or the folder of a
    /// `project.json`. `None` for a single-file project (or a path that does not exist).
    let folderOf (path: string) : string option =
        if Directory.Exists path then
            Some path
        elif Path.GetFileName path = ProjectFolder.MainFile then
            Option.ofObj (Path.GetDirectoryName(Path.GetFullPath path))
        else
            None

    /// A project folder's files (what `ProjectFolder.load` reads): every project file under
    /// `folder` (`ProjectFolder.isProjectFile`), by its `/`-separated relative path. Other files
    /// in the folder (a README, `.git`) are left out.
    let readFolder (folder: string) : IReadOnlyDictionary<string, byte[]> =
        let files = Dictionary<string, byte[]>(StringComparer.Ordinal)

        for file in Directory.EnumerateFiles(folder, "*", SearchOption.AllDirectories) do
            let relative =
                Path.GetRelativePath(folder, file).Replace(Path.DirectorySeparatorChar, '/')

            if ProjectFolder.isProjectFile relative then
                files.[relative] <- File.ReadAllBytes file

        files

    /// The loaded project at `path` (a folder, the `project.json` of one, or a single file),
    /// validated like the editor opens it, or why it could not be loaded.
    let loadResult (path: string) : LoadResult<GameProject> =
        match folderOf path with
        | Some folder -> ProjectFolder.load (readFolder folder)
        | None -> ProjectLoad.loadProjectText (File.ReadAllText path)

    /// `loadResult` as a `Result`.
    let load (path: string) : Result<GameProject, string list> =
        let loaded = loadResult path

        match loaded.Ok, loaded.Data with
        | true, Some project -> Ok project
        | true, None -> Error [ "The project loaded without project data." ]
        | false, _ -> Error loaded.Errors

    /// The project at `path` as single-file project JSON text, without validating it: a folder
    /// is joined (`ProjectFolder.joinText`), a single file is read as it is.
    let readSingleFileText (path: string) : Result<string, string list> =
        match folderOf path with
        | Some folder -> ProjectFolder.joinText (readFolder folder)
        | None -> Ok(File.ReadAllText path)

    /// Whether `path` is free to write a new project to: it does not exist, or it is an empty
    /// folder.
    let isFree (path: string) : bool =
        if File.Exists path then
            false
        elif Directory.Exists path then
            Seq.isEmpty (Directory.EnumerateFileSystemEntries path)
        else
            true

    /// Write `project` as a project folder (`ProjectFolder.toFiles`) into `folder`, creating it.
    /// Files of the folder format that the project no longer has are deleted; anything else in
    /// the folder is left alone. Returns the paths written.
    let writeFolder (folder: string) (project: GameProject) : string list =
        Directory.CreateDirectory folder |> ignore
        let files = ProjectFolder.toFiles project
        let written = files |> List.map fst |> Set.ofList

        for path in (readFolder folder).Keys |> List.ofSeq do
            if not (written.Contains path) then
                File.Delete(Path.Combine(folder, path.Replace('/', Path.DirectorySeparatorChar)))

        for path, bytes in files do
            let file = Path.Combine(folder, path.Replace('/', Path.DirectorySeparatorChar))

            match Path.GetDirectoryName file with
            | null -> ()
            | directory -> Directory.CreateDirectory directory |> ignore

            File.WriteAllBytes(file, bytes)

        files |> List.map fst

    /// Write single-file project JSON `text` to `path`, creating its folder.
    let writeSingleFileText (path: string) (text: string) : unit =
        let full = Path.GetFullPath path

        match Path.GetDirectoryName full with
        | null -> ()
        | directory -> Directory.CreateDirectory directory |> ignore

        File.WriteAllText(full, (if text.EndsWith '\n' then text else text + "\n"))

    /// Write `project` as a single-file project (`ProjectFolder.toSingleFileText`) to `path`.
    let writeSingleFile (path: string) (project: GameProject) : unit =
        writeSingleFileText path (ProjectFolder.toSingleFileText project)
