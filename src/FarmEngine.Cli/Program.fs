module FarmEngine.Cli.Program

open System
open System.IO
open System.Reflection
open FarmEngine.Authoring
open FarmEngine.Authoring.Net
open FarmEngine.Export
open FarmEngine.Schemas

let private usageText =
    String.concat
        Environment.NewLine
        [
            "Usage:"
            "  farmc new <template> <folder>"
            "  farmc check <project> [--json]"
            "  farmc convert <project> --out <path>"
            "  farmc compile <project> --out <game.cart>"
            "  farmc export <project> [--target windows-x64|linux-x64|web]... --out <dir> [--templates <dir>] [--no-archive]"
            "  farmc --help | --version"
            ""
            "<project> is a project folder (or the project.json in one) or a single-file project."
            "Options may come in any order."
            ""
            "new        creates a project from a template (starter, cozy, quest or blank) in a new or"
            "           empty folder; a <folder> ending in .json is written as a single file."
            "check      lists the project's Problems, like the editor's Problems tab, and fails when"
            "           any is an error. --json prints them as JSON (severity, code, path, message)."
            "convert    turns a project folder into a single-file project, or a single file into a"
            "           folder, at a new --out path."
            "compile    builds the cartridge the player runs."
            "export     does what Export Game does in the editor (docs/EXPORT.md). Without --target it"
            "           exports the project's saved targets. Templates come from --templates, then"
            "           the FARM_PLAYER_TEMPLATES variable, then players/ next to farmc."
            ""
            "Exit codes: 0 done, 1 the command failed (or check found errors), 2 the command line is"
            "wrong."
        ]

/// The version farmc was built at (`-p:Version=…`, without a source revision suffix).
let version () : string =
    let assembly = Assembly.GetExecutingAssembly()

    match assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>() with
    | null -> string (assembly.GetName().Version)
    | attribute ->
        let text = attribute.InformationalVersion

        match text.IndexOf '+' with
        | -1 -> text
        | plus -> text.Substring(0, plus)

/// The command line after the command: positional arguments and options, in any order.
type Arguments =
    {
        Positional: string list
        Targets: string list
        Out: string option
        Templates: string option
        Archives: bool
        Json: bool
        Help: bool
    }

/// No arguments yet (where `parseArguments` starts).
let emptyArguments =
    {
        Positional = []
        Targets = []
        Out = None
        Templates = None
        Archives = true
        Json = false
        Help = false
    }

/// Options that take a value (the next argument).
let private valued = set [ "--target"; "--out"; "--templates" ]

let private isOption (arg: string) =
    arg.StartsWith("--", StringComparison.Ordinal) || arg = "-h"

/// One parser for every command. A value that looks like an option (`--out --no-archive`) is an
/// error, not a folder named "--no-archive".
let rec parseArguments (args: string list) (acc: Arguments) : Result<Arguments, string> =
    match args with
    | [] ->
        Ok
            { acc with
                Positional = List.rev acc.Positional
            }
    | flag :: rest when valued.Contains flag ->
        match rest with
        | value :: rest when not (isOption value) && value.Length > 0 ->
            let acc =
                match flag with
                | "--target" ->
                    { acc with
                        Targets = acc.Targets @ [ value ]
                    }
                | "--out" -> { acc with Out = Some value }
                | _ -> { acc with Templates = Some value }

            parseArguments rest acc
        | _ -> Error(sprintf "%s needs a value." flag)
    | "--no-archive" :: rest -> parseArguments rest { acc with Archives = false }
    | "--json" :: rest -> parseArguments rest { acc with Json = true }
    | ("--help" | "-h") :: rest -> parseArguments rest { acc with Help = true }
    | option :: _ when isOption option -> Error(sprintf "Unknown option %s." option)
    | positional :: rest ->
        parseArguments
            rest
            { acc with
                Positional = positional :: acc.Positional
            }

/// The loaded project (a folder, the `project.json` of one, or a single file), or why it could
/// not be loaded.
let private load (projectPath: string) : Result<GameProject, string list> = ProjectFiles.load projectPath

let private export (args: Arguments) (out: TextWriter) (err: TextWriter) : int =
    match args.Positional, args.Out with
    | [ projectPath ], Some folder ->
        match load projectPath with
        | Error errors ->
            for error in errors do
                err.WriteLine error

            1
        | Ok project ->
            let targets =
                if args.Targets.IsEmpty then
                    (Exporter.identity project).Targets
                else
                    args.Targets

            let report =
                Exporter.run
                    {
                        Targets = targets
                        OutputFolder = folder
                        TemplatesFolder = args.Templates
                        CreateArchives = args.Archives
                        EditorVersion = None
                    }
                    project

            let text = Exporter.format report
            if report.Ok then out.Write text else err.Write text
            if report.Ok then 0 else 1
    | _ -> 2

let private compile (args: Arguments) (out: TextWriter) (err: TextWriter) : int =
    match args.Positional, args.Out with
    | [ projectPath ], Some outputPath when args.Targets.IsEmpty && args.Templates.IsNone && args.Archives ->
        match load projectPath with
        | Error errors ->
            for error in errors do
                err.WriteLine error

            1
        | Ok project ->
            let bytes = CartridgeCompiler.Compile project
            let absoluteOutput = Path.GetFullPath outputPath

            match Path.GetDirectoryName absoluteOutput with
            | null -> invalidOp "Output path has no directory."
            | directory -> Directory.CreateDirectory(directory) |> ignore

            File.WriteAllBytes(absoluteOutput, bytes)
            out.WriteLine(sprintf "Compiled %d bytes to %s" bytes.Length absoluteOutput)
            0
    | _ -> 2

/// The text of one problem: `error code | path | message` (the Problems tab's copy format).
let formatProblem (problem: Problem) : string =
    sprintf "%s %s | %s | %s" problem.SeverityName problem.Code problem.Path problem.Message

/// The problems as JSON: `{ ok, errors, warnings, infos, problems: [{ severity, code, path,
/// message, target? }] }` (`target` names the editor to open: `{ kind, id }`).
let problemsJson (problems: Problem list) : Json =
    let count severity =
        problems
        |> List.filter (fun p -> p.Severity = severity)
        |> List.length
        |> float
        |> JNumber

    JObject
        [
            "ok", JBool(not (Problems.blocksExport problems))
            "errors", count Severity.Error
            "warnings", count Severity.Warning
            "infos", count Severity.Info
            "problems",
            JArray(
                problems
                |> List.map (fun p ->
                    JObject
                        [
                            "severity", JString p.SeverityName
                            "code", JString p.Code
                            "path", JString p.Path
                            "message", JString p.Message
                            if p.HasTarget then
                                "target",
                                JObject
                                    [
                                        "kind", JString p.TargetKind
                                        match p.TargetId with
                                        | Null -> ()
                                        | NonNull id -> "id", JString id
                                    ]
                        ])
            )
        ]

let private check (args: Arguments) (out: TextWriter) (err: TextWriter) : int =
    match args.Positional, args.Out with
    | [ projectPath ], None when args.Targets.IsEmpty && args.Templates.IsNone && args.Archives ->
        match load projectPath with
        | Error errors ->
            // A project that does not load has schema errors only: report them the same way.
            if args.Json then
                out.WriteLine(
                    Json.stringifyIndented (
                        JObject
                            [
                                "ok", JBool false
                                "loaded", JBool false
                                "loadErrors", JArray(errors |> List.map JString)
                            ]
                    )
                )
            else
                for error in errors do
                    err.WriteLine("error " + error)

            1
        | Ok project ->
            let problems = Problems.collect project

            if args.Json then
                out.WriteLine(Json.stringifyIndented (problemsJson problems))
            else
                for problem in problems do
                    out.WriteLine(formatProblem problem)

                let count severity =
                    problems |> List.filter (fun p -> p.Severity = severity) |> List.length

                let plural n (word: string) =
                    sprintf "%d %s%s" n word (if n = 1 then "" else "s")

                out.WriteLine(
                    sprintf
                        "%s, %s, %s."
                        (plural (count Severity.Error) "error")
                        (plural (count Severity.Warning) "warning")
                        (plural (count Severity.Info) "tip")
                )

            if Problems.blocksExport problems then 1 else 0
    | _ -> 2

let private newProject (args: Arguments) (out: TextWriter) (err: TextWriter) : int =
    match args.Positional with
    | [ template; path ] when
        args.Out.IsNone
        && args.Targets.IsEmpty
        && args.Templates.IsNone
        && args.Archives
        && not args.Json
        ->
        if not (Seq.contains template ProjectCatalog.All) then
            2
        elif not (ProjectFiles.isFree path) then
            err.WriteLine(sprintf "%s already exists and is not an empty folder." path)
            1
        else
            let full = Path.GetFullPath path
            let singleFile = full.EndsWith(".json", StringComparison.OrdinalIgnoreCase)

            let name =
                let fileName =
                    if singleFile then
                        Path.GetFileNameWithoutExtension full
                    else
                        Path.GetFileName(Path.TrimEndingDirectorySeparator full)

                match fileName with
                | NonNull text when not (String.IsNullOrWhiteSpace text) -> text
                | _ -> "My Farm"

            let now = float (DateTimeOffset.UtcNow.ToUnixTimeMilliseconds())
            let project = ProjectCatalog.CreateNewProject(template, name, null, now)

            if singleFile then
                ProjectFiles.writeSingleFile full project
            else
                ProjectFiles.writeFolder full project |> ignore

            out.WriteLine(sprintf "Created \"%s\" from the %s template in %s" name template full)
            0
    | _ -> 2

let private convert (args: Arguments) (out: TextWriter) (err: TextWriter) : int =
    match args.Positional, args.Out with
    | [ projectPath ], Some outputPath when args.Targets.IsEmpty && args.Templates.IsNone && args.Archives && not args.Json ->
        if not (File.Exists projectPath || Directory.Exists projectPath) then
            err.WriteLine(sprintf "%s does not exist." projectPath)
            1
        elif not (ProjectFiles.isFree outputPath) then
            err.WriteLine(sprintf "%s already exists and is not an empty folder; choose a new --out." outputPath)
            1
        else
            let full = Path.GetFullPath outputPath

            match ProjectFiles.folderOf projectPath with
            | Some _ ->
                // Folder to single file: joined as it is, so nothing is lost or reordered.
                match ProjectFiles.readSingleFileText projectPath with
                | Error errors ->
                    for error in errors do
                        err.WriteLine error

                    1
                | Ok text ->
                    ProjectFiles.writeSingleFileText full text
                    out.WriteLine(sprintf "Wrote the single-file project %s" full)
                    0
            | None ->
                // Single file to folder: the folder layout is written from the loaded project.
                match load projectPath with
                | Error errors ->
                    for error in errors do
                        err.WriteLine error

                    1
                | Ok project ->
                    let files = ProjectFiles.writeFolder full project
                    out.WriteLine(sprintf "Wrote the project folder %s (%d files)" full files.Length)
                    0
    | _ -> 2

/// What a usage error of compile, export or convert says.
let private problemFor (command: string) (args: Arguments) : string =
    match args.Positional, args.Out with
    | [], _ -> sprintf "%s needs a project file." command
    | _ :: _ :: _, _ -> sprintf "%s takes one project file." command
    | _, None -> sprintf "%s needs --out." command
    | _ when command = "compile" || command = "convert" -> sprintf "%s takes no --target, --templates or --no-archive." command
    | _ -> sprintf "Wrong arguments for %s." command

/// What a usage error says beyond the usage text.
let private problem (command: string) (args: Arguments) : string =
    let extras = not args.Targets.IsEmpty || args.Templates.IsSome || not args.Archives

    match command, args.Positional with
    | "new", ([] | [ _ ]) -> "new needs a template and a folder (farmc new starter my-farm)."
    | "new", [ template; _ ] when not (Seq.contains template ProjectCatalog.All) ->
        sprintf "Unknown template %s (one of %s)." template (String.Join(", ", ProjectCatalog.All))
    | "new", [ _; _ ] -> "new takes no options."
    | "new", _ -> "new takes a template and one folder."
    | "check", [] -> "check needs a project file."
    | "check", _ :: _ :: _ -> "check takes one project file."
    | "check", _ when args.Out.IsSome -> "check takes no --out."
    | "check", _ when extras -> "check takes no --target, --templates or --no-archive."
    | _ when command <> "check" && args.Json -> sprintf "%s takes no --json (only check does)." command
    | _ -> problemFor command args

/// farmc with its output streams: the exit code (0 done, 1 failed, 2 usage). Never throws.
let run (argv: string[]) (out: TextWriter) (err: TextWriter) : int =
    let usageError (message: string) =
        if message.Length > 0 then
            err.WriteLine message

        err.WriteLine usageText
        2

    try
        match List.ofArray argv with
        | [ "--version" ] ->
            out.WriteLine("farmc " + version ())
            0
        | [ "--help" ]
        | [ "-h" ]
        | [ "help" ] ->
            out.WriteLine usageText
            0
        | ("new" | "check" | "convert" | "compile" | "export" as command) :: rest ->
            match parseArguments rest emptyArguments with
            | Error message -> usageError message
            | Ok args when args.Help ->
                out.WriteLine usageText
                0
            | Ok args ->
                let code =
                    match command with
                    | "new" -> newProject args out err
                    | "check" -> check args out err
                    | "convert" -> convert args out err
                    | "compile" when not args.Json -> compile args out err
                    | "export" when not args.Json -> export args out err
                    | _ -> 2

                if code = 2 then usageError (problem command args) else code
        | [] -> usageError ""
        | command :: _ when isOption command -> usageError (sprintf "Unknown option %s." command)
        | command :: _ -> usageError (sprintf "Unknown command %s." command)
    with error ->
        // A bad path, an unreadable file, a project Problems refuses, a failed export: the
        // message, never a stack trace.
        err.WriteLine error.Message
        1

[<EntryPoint>]
let main argv = run argv Console.Out Console.Error
