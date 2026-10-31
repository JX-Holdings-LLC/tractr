/// farmc (src/FarmEngine.Cli) through its entry point: exit codes and messages.
module FarmEngine.Export.Tests.CliTests

open System
open System.IO
open FarmEngine.Authoring
open FarmEngine.Authoring.Net
open FarmEngine.Cli
open FarmEngine.Export.Tests.Support
open Xunit

/// The exit code and what farmc wrote to stdout and stderr.
let private farmc (args: string list) : int * string * string =
    use out = new StringWriter()
    use err = new StringWriter()
    let code = Program.run (Array.ofList args) out err
    code, out.ToString(), err.ToString()

[<Fact>]
let ``help and version print and exit 0`` () =
    for help in
        [
            [ "--help" ]
            [ "-h" ]
            [ "help" ]
            [ "compile"; "--help" ]
            [ "export"; "x.json"; "-h" ]
        ] do
        let code, out, err = farmc help
        Assert.Equal(0, code)
        Assert.Contains("farmc compile <project> --out <game.cart>", out)
        Assert.Equal("", err)

    let code, out, _ = farmc [ "--version" ]
    Assert.Equal(0, code)
    Assert.StartsWith("farmc ", out)
    Assert.DoesNotContain("+", out)

[<Fact>]
let ``usage errors exit 2 and say what is wrong`` () =
    let cases =
        [
            [], ""
            [ "build" ], "Unknown command build."
            [ "--verbose" ], "Unknown option --verbose."
            [ "compile"; "p.json" ], "compile needs --out."
            [ "compile"; "--out"; "x.cart" ], "compile needs a project file."
            [ "compile"; "a.json"; "b.json"; "--out"; "x.cart" ], "compile takes one project file."
            [ "compile"; "p.json"; "--out"; "x.cart"; "--target"; "web" ], "compile takes no --target"
            [ "compile"; "p.json"; "--frobnicate" ], "Unknown option --frobnicate."
            [ "export"; "p.json"; "--out" ], "--out needs a value."
            // A flag is never taken as the value of the one before it.
            [ "export"; "p.json"; "--out"; "--no-archive" ], "--out needs a value."
            [ "export"; "p.json"; "--target"; "--out"; "dist" ], "--target needs a value."
        ]

    for args, message in cases do
        let code, out, err = farmc args
        Assert.True((code = 2), sprintf "%A exited %d" args code)
        Assert.Equal("", out)
        Assert.Contains(message, err)
        Assert.Contains("Usage:", err)

[<Fact>]
let ``options parse in any order`` () =
    match
        Program.parseArguments
            [
                "--out"
                "dist"
                "--no-archive"
                "game.json"
                "--target"
                "web"
                "--target"
                "linux-x64"
            ]
            Program.emptyArguments
    with
    | Ok parsed ->
        Assert.Equal<string list>([ "game.json" ], parsed.Positional)
        Assert.Equal<string list>([ "web"; "linux-x64" ], parsed.Targets)
        Assert.Equal(Some "dist", parsed.Out)
        Assert.False parsed.Archives
    | Error message -> failwith message

[<Fact>]
let ``a bad path or a refused project exits 1 with the message, never a stack trace`` () =
    use temp = new TempDir()

    for path in [ ""; temp.Sub "missing.json"; temp.Path ] do
        let code, _, err = farmc [ "compile"; path; "--out"; temp.Sub "x.cart" ]
        Assert.True((code = 1), sprintf "%A exited %d: %s" path code err)
        Assert.DoesNotContain("   at ", err)
        Assert.True(err.Trim().Length > 0)

    let broken = temp.Sub "broken.json"
    File.WriteAllText(broken, "{ \"schemaVersion\": 1, ")
    let code, _, err = farmc [ "compile"; broken; "--out"; temp.Sub "x.cart" ]
    Assert.Equal(1, code)
    Assert.Contains("not valid JSON", err)
    // Deep nesting is a parse error, not a crashed process.
    let deep = temp.Sub "deep.json"
    File.WriteAllText(deep, "{\"a\":" + String('[', 100_000) + String(']', 100_000) + "}")
    let code, _, err = farmc [ "export"; deep; "--out"; temp.Sub "dist" ]
    Assert.Equal(1, code)
    Assert.Contains("Too deeply nested", err)

[<Fact>]
let ``compile writes the cartridge with options in any order`` () =
    use temp = new TempDir()
    let project = temp.Sub "game.json"
    File.WriteAllText(project, ProjectLoad.toText (starter ()))
    let cart = temp.Sub "out/game.cart"
    let code, out, err = farmc [ "compile"; "--out"; cart; project ]
    Assert.True((code = 0), err)
    Assert.Contains("Compiled", out)
    Assert.True(CartridgeReader.hasIdentifier (File.ReadAllBytes cart))

[<Fact>]
let ``compile reads a project folder or the project json in one`` () =
    use temp = new TempDir()
    let folder = temp.Sub "farm"

    for path, bytes in ProjectFolder.toFiles (starter ()) do
        let file = Path.Combine(folder, path.Replace('/', Path.DirectorySeparatorChar))
        Directory.CreateDirectory(Path.GetDirectoryName file) |> ignore
        File.WriteAllBytes(file, bytes)

    for project in [ folder; Path.Combine(folder, ProjectFolder.MainFile) ] do
        let cart = temp.Sub "folder.cart"
        let code, _, err = farmc [ "compile"; project; "--out"; cart ]
        Assert.True((code = 0), err)
        Assert.True(CartridgeReader.hasIdentifier (File.ReadAllBytes cart))
    // A folder missing a file it names says which.
    File.Delete(Path.Combine(folder, "scenes", "scene-farm.json"))
    let code, _, err = farmc [ "compile"; folder; "--out"; temp.Sub "broken.cart" ]
    Assert.Equal(1, code)
    Assert.Contains("scenes/scene-farm.json is missing from the project folder", err)

[<Fact>]
let ``new writes every template as a folder or a single file that loads and checks clean`` () =
    use temp = new TempDir()

    for template in ProjectCatalog.All do
        let folder = temp.Sub template
        let code, out, err = farmc [ "new"; template; folder ]
        Assert.True((code = 0), err)
        Assert.Contains(sprintf "from the %s template" template, out)
        Assert.True(File.Exists(Path.Combine(folder, ProjectFolder.MainFile)))

        match ProjectFiles.load folder with
        | Ok project -> Assert.Equal(template, project.Name)
        | Error errors -> failwith (String.Join("\n", errors))

        let code, out, err = farmc [ "check"; folder ]
        Assert.True((code = 0), out + err)
        Assert.Contains("0 errors, 0 warnings", out)

    let single = temp.Sub "Willow Creek.json"
    let code, _, err = farmc [ "new"; "cozy"; single ]
    Assert.True((code = 0), err)

    match ProjectFiles.load single with
    | Ok project -> Assert.Equal("Willow Creek", project.Name)
    | Error errors -> failwith (String.Join("\n", errors))

[<Fact>]
let ``new refuses an unknown template and a folder that is not empty`` () =
    use temp = new TempDir()
    let code, _, err = farmc [ "new"; "space-opera"; temp.Sub "x" ]
    Assert.Equal(2, code)
    Assert.Contains("Unknown template space-opera (one of starter, blank, cozy, quest).", err)
    let code, _, err = farmc [ "new"; "starter" ]
    Assert.Equal(2, code)
    Assert.Contains("new needs a template and a folder", err)
    File.WriteAllText(temp.Sub "keep.txt", "mine")
    let code, _, err = farmc [ "new"; "starter"; temp.Path ]
    Assert.Equal(1, code)
    Assert.Contains("is not an empty folder", err)
    Assert.Equal<string[]>([| temp.Sub "keep.txt" |], Directory.GetFileSystemEntries temp.Path)

[<Fact>]
let ``check lists the problems and fails on errors, as text or json`` () =
    use temp = new TempDir()
    let project = temp.Sub "game.json"
    let starter = starter ()
    // A quest given by an NPC that does not exist is an error.
    let broken =
        { starter with
            Quests =
                starter.Quests
                |> List.mapi (fun i quest ->
                    if i = 0 then
                        { quest with Giver = Some "npc-nobody" }
                    else
                        quest)
        }

    File.WriteAllText(project, ProjectLoad.toText broken)
    let code, out, _ = farmc [ "check"; project ]
    Assert.Equal(1, code)
    Assert.Contains("error quest.giverMissing | quests[0].giver | ", out)
    Assert.Contains("1 error, ", out)
    let code, out, _ = farmc [ "check"; "--json"; project ]
    Assert.Equal(1, code)

    match Json.parse out with
    | Ok json ->
        Assert.Equal(JBool false, Json.get "ok" json)
        Assert.Equal(JNumber 1.0, Json.get "errors" json)

        let first =
            match Json.get "problems" json with
            | JArray(first :: _) -> first
            | other -> failwithf "no problems: %A" other

        Assert.Equal(JString "error", Json.get "severity" first)
        Assert.Equal(JString "quest.giverMissing", Json.get "code" first)
        Assert.Equal(JString "quests[0].giver", Json.get "path" first)
        Assert.Equal(JString "quest", Json.get "kind" (Json.get "target" first))
    | Error message -> failwith message
    // A project that does not load reports its errors and fails.
    File.WriteAllText(project, "{ \"schemaVersion\": 1 }")
    let code, out, _ = farmc [ "check"; project; "--json" ]
    Assert.Equal(1, code)
    Assert.Contains("\"loadErrors\"", out)
    // Usage.
    let code, _, err = farmc [ "check"; project; "--out"; "x" ]
    Assert.Equal(2, code)
    Assert.Contains("check takes no --out.", err)
    let code, _, err = farmc [ "compile"; project; "--out"; "x"; "--json" ]
    Assert.Equal(2, code)
    Assert.Contains("compile takes no --json", err)

[<Fact>]
let ``convert turns a folder into a single file and back without losing anything`` () =
    use temp = new TempDir()
    let folder = temp.Sub "farm"
    let code, _, err = farmc [ "new"; "quest"; folder ]
    Assert.True((code = 0), err)
    let single = temp.Sub "farm.json"
    let code, _, err = farmc [ "convert"; folder; "--out"; single ]
    Assert.True((code = 0), err)
    let back = temp.Sub "farm-again"
    let code, _, err = farmc [ "convert"; single; "--out"; back ]
    Assert.True((code = 0), err)

    let files (root: string) =
        Directory.GetFiles(root, "*", SearchOption.AllDirectories)
        |> Array.map (fun f -> Path.GetRelativePath(root, f), File.ReadAllBytes f)
        |> Array.sortBy fst

    Assert.Equal<(string * byte[])[]>(files folder, files back)
    // The project.json of a folder converts like the folder; an existing --out is refused.
    let code, _, err =
        farmc [ "convert"; Path.Combine(folder, ProjectFolder.MainFile); "--out"; single ]

    Assert.Equal(1, code)
    Assert.Contains("already exists", err)
    let code, _, err = farmc [ "convert"; folder ]
    Assert.Equal(2, code)
    Assert.Contains("convert needs --out.", err)
