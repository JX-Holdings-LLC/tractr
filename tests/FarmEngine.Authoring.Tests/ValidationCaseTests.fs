/// The validators on every golden, fixture, sample and deliberately broken project: each broken
/// case (`BrokenProjects`) reaches the check it was written for, the load path reports the same
/// findings as the checks it runs, and the `ContentLints` findings on the golden, fixture and
/// sample projects are pinned to the ones recorded when the C# validators were retired
/// (`fixtures/projects/content-lints.json`).
module FarmEngine.Authoring.Tests.ValidationCaseTests

open System
open System.IO
open Xunit
open FarmEngine.Authoring
open FarmEngine.Schemas
open FarmEngine.Authoring.Tests.TestProjects

/// The result, or the exception type and message.
let private attempt (f: unit -> 'T) : Result<'T, string> =
    try
        Ok(f ())
    with ex ->
        Error(ex.GetType().FullName + ": " + ex.Message)

// ── Inputs ───────────────────────────────────────────────────────────────────

let private baseDir = AppContext.BaseDirectory

let private readFile (path: string) : Json =
    match Json.parse (File.ReadAllText path) with
    | Ok json -> json
    | Error message -> failwithf "%s: %s" path message

let private files (dir: string) (pattern: string) =
    Directory.GetFiles(Path.Combine(baseDir, dir), pattern) |> Array.sortWith (fun a b -> String.CompareOrdinal(a, b)) |> List.ofArray

/// Project-shaped JSON from every golden and fixture, by name.
let private jsonSources () : (string * (unit -> Json)) list =
    let projectKey (path: string) () =
        let json = readFile path
        if Json.has "project" json then Json.get "project" json else json
    [ for path in files "Golden/content" "*.json" -> "content/" + Path.GetFileName path, projectKey path
      for path in files "Golden/replays" "*.json" do
          if Path.GetFileName path <> "index.json" then
              yield "replays/" + Path.GetFileName path, projectKey path
      yield "fixtures/fixture-project.json", (fun () -> readFile (Path.Combine(baseDir, "Fixtures", "fixture-project.json")))
      yield "fixtures/packs-project.json", (fun () -> readFile (Path.Combine(baseDir, "Fixtures", "packs-project.json"))) ]

/// Sample, template and default projects built in code, by name.
let private codeSources () : (string * (unit -> GameProject)) list =
    [ yield "starter", starter
      yield "blank", blank
      yield "cozy factory", (fun () -> ProjectCatalog.CreateCozyFarmProject(0.0))
      yield "quest factory", (fun () -> ProjectCatalog.CreateQuestRpgProject(0.0))
      for id, _ in templates () -> "template " + id, (fun () -> ProjectCatalog.CreateProjectForTemplate(id, 0.0)) ]

let private decode (decoder: Path -> Json -> 'T) (json: Json) : 'T option =
    match Decode.run decoder json with
    | Ok value -> Some value
    | Error _ -> None

/// The JSON when it is an object, whatever its validity.
let private asObject (raw: Json) : Json option =
    match raw with
    | JObject _ -> Some raw
    | _ -> None

let jsonSourceNames () : seq<obj[]> = jsonSources () |> Seq.map (fun (name, _) -> [| box name |])
let codeSourceNames () : seq<obj[]> = codeSources () |> Seq.map (fun (name, _) -> [| box name |])
let brokenCaseNames () : seq<obj[]> = BrokenProjects.cases |> Seq.map (fun c -> [| box c.Name |])

let private brokenCase (name: string) = BrokenProjects.cases |> List.find (fun c -> c.Name = name)

// ── The load path reports what the checks report ─────────────────────────────

/// Loading reports the checks' findings (capped at 20) and refuses exactly when they find any;
/// the checks never throw.
let private assertLoadMatchesChecks (label: string) (raw: Json) =
    let result = ProjectLoad.loadProject raw
    match asObject raw |> Option.bind (decode SchemaJson.decodeGameProject) with
    | Some project ->
        let errors = SchemaChecks.validateProject project
        Assert.Equal<string list>(List.truncate 20 errors, if result.Ok then [] else result.Errors |> List.truncate 20)
        Assert.Equal(errors.IsEmpty, result.Ok)
    | None -> Assert.False(result.Ok, label + ": an unreadable project loaded")

[<Theory>]
[<MemberData(nameof jsonSourceNames)>]
let ``golden and fixture projects load and check without throwing`` (name: string) =
    let raw = (jsonSources () |> List.find (fun (n, _) -> n = name) |> snd) ()
    let mutable checkedForms = 0
    match decode SchemaJson.decodeGameProject raw with
    | Some project ->
        Assert.True((attempt (fun () -> SchemaChecks.validateProject project, SchemaChecks.lintProject project)) |> Result.isOk)
        checkedForms <- checkedForms + 1
    | None -> ()
    if (asObject raw).IsSome then
        assertLoadMatchesChecks name raw
    Assert.True(checkedForms > 0, name + ": nothing checked")

[<Theory>]
[<MemberData(nameof codeSourceNames)>]
let ``sample and template projects are valid`` (name: string) =
    let project = (codeSources () |> List.find (fun (n, _) -> n = name) |> snd) ()
    Assert.Empty(SchemaChecks.validateProject project)
    assertLoadMatchesChecks name (ProjectLoad.toJson project)

// ── Content lints: the recorded findings ─────────────────────────────────────

/// The findings the F# `ContentLints` matched with the C# `Validation`, finding for finding, when
/// the C# engine was retired. After an intended lint change, rerun this test with
/// FARM_RECORD_CONTENT_LINTS=1 to rewrite the file, and review its diff.
let private contentLintsFile = Path.Combine(__SOURCE_DIRECTORY__, "..", "..", "fixtures", "projects", "content-lints.json")

let private severityName (severity: Severity) =
    match severity with
    | Severity.Error -> "error"
    | Severity.Warning -> "warning"
    | Severity.Info -> "info"

let private lintLines (project: GameProject) : string list =
    match attempt (fun () -> ContentLints.validateProjectContent project) with
    | Ok lints -> [ for l in lints -> sprintf "%s|%s|%s|%s" (severityName l.Severity) l.Category l.Message (defaultArg l.Subject "") ]
    | Error message -> [ "throws|" + message ]

/// Findings per project: every golden and fixture, and every code-built sample.
let private contentLintFindings () : (string * string list) list =
    [ for name, load in jsonSources () do
          match decode SchemaJson.decodeGameProject (load ()) with
          | Some project -> yield name, lintLines project
          | None -> ()
      for name, build in codeSources () -> name, lintLines (build ()) ]

[<Fact>]
let ``content lints report the recorded findings on golden, fixture and sample projects`` () =
    let actual = contentLintFindings ()
    if Environment.GetEnvironmentVariable "FARM_RECORD_CONTENT_LINTS" = "1" then
        let json = JObject [ for name, lines in actual -> name, JArray(List.map JString lines) ]
        File.WriteAllText(contentLintsFile, Json.stringifyIndented json + "\n")
    else
        let recorded =
            match readFile (Path.Combine(baseDir, "Fixtures", "content-lints.json")) with
            | JObject members -> members
            | _ -> failwith "content-lints.json is not an object"
        Assert.Equal<string list>(List.map fst recorded, List.map fst actual)
        for name, lines in actual do
            let expected = field name recorded |> function JArray items -> List.choose Json.asString items | _ -> []
            Assert.True((expected = lines), sprintf "%s: recorded\n%s\nactual\n%s" name (String.concat "\n" expected) (String.concat "\n" lines))

// ── Broken projects ──────────────────────────────────────────────────────────

let private hasPrefix (prefix: string) (entries: string list) =
    entries |> List.exists (fun e -> e.StartsWith(prefix, StringComparison.Ordinal))

[<Theory>]
[<MemberData(nameof brokenCaseNames)>]
let ``every broken project reaches the check it was written for`` (name: string) =
    let case = brokenCase name
    let raw = BrokenProjects.json case |> Net.JsonInterop.ofNode
    match BrokenProjects.tryProject case with
    | Error issue ->
        // The typed parse refuses it; so does loading, with the same issue.
        Assert.True(case.Expect |> List.exists (function BrokenProjects.Decode _ -> true | _ -> false), sprintf "%s does not parse: %s" name issue)
        Assert.False (ProjectLoad.loadProject raw).Ok
    | Ok project ->
        Assert.True(attempt (fun () -> SchemaChecks.lintProject project) |> Result.isOk)
        // Loading refuses another schema version before the checks run.
        if case.Typed project = project && project.SchemaVersion = ProjectSchema.CurrentProjectSchemaVersion then
            assertLoadMatchesChecks name raw
    for expect in case.Expect do
        match expect with
        | BrokenProjects.Parse prefix ->
            let errors = SchemaChecks.validateProject (BrokenProjects.project case)
            Assert.True(hasPrefix prefix errors, sprintf "%s: expected %s in\n%s" name prefix (String.concat "\n" errors))
        | BrokenProjects.Lint prefix ->
            let lints = SchemaChecks.lintProject (BrokenProjects.project case)
            Assert.True(hasPrefix prefix lints, sprintf "%s: expected %s in\n%s" name prefix (String.concat "\n" lints))
        | BrokenProjects.Content text ->
            let messages = ContentLints.validateProjectContent (BrokenProjects.project case) |> List.map (fun l -> l.Message)
            Assert.True(messages |> List.exists (fun m -> m.Contains(text, StringComparison.Ordinal)),
                        sprintf "%s: expected %s in\n%s" name text (String.concat "\n" messages))
        | BrokenProjects.NoContent text ->
            let messages = ContentLints.validateProjectContent (BrokenProjects.project case) |> List.map (fun l -> l.Message)
            Assert.False(messages |> List.exists (fun m -> m.Contains(text, StringComparison.Ordinal)), sprintf "%s: unexpected %s" name text)
        | BrokenProjects.Decode issue ->
            Assert.Equal(Error issue, BrokenProjects.tryProject case)

[<Fact>]
let ``the combined broken project hits the 20-error cap`` () =
    let case = brokenCase "everything at once"
    let result = ProjectLoad.loadProject (BrokenProjects.json case |> Net.JsonInterop.ofNode)
    Assert.False result.Ok
    Assert.True((result.Errors.Length = 20), String.concat "\n" result.Errors)
    Assert.True(SchemaChecks.validateProject (BrokenProjects.project case) |> List.length > 20)

