/// M8: a project as a folder of files (`ProjectFolder`).
module FarmEngine.Authoring.Tests.ProjectFolderTests

open System
open System.Collections.Generic
open System.Text
open Xunit
open FarmEngine.Authoring
open FarmEngine.Authoring.Tests.TestProjects
open FarmEngine.Schemas

let private folder (files: (string * byte[]) list) : IReadOnlyDictionary<string, byte[]> =
    Dictionary<string, byte[]>(dict files) :> IReadOnlyDictionary<string, byte[]>

let private textOf (files: (string * byte[]) list) (path: string) =
    files |> List.find (fun (p, _) -> p = path) |> snd |> Encoding.UTF8.GetString

let private joined (project: GameProject) : Json =
    match ProjectFolder.join (folder (ProjectFolder.toFiles project)) with
    | Ok json -> json
    | Error errors -> failwith (String.concat "; " errors)

let private repoJson (parts: string list) : Json =
    let path = IO.Path.Combine(Array.ofList (__SOURCE_DIRECTORY__ :: ".." :: ".." :: parts))
    match Json.parse (IO.File.ReadAllText path) with
    | Ok json -> json
    | Error message -> failwith message

let private png = Convert.FromBase64String "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="

let private withArt (project: GameProject) =
    { project with
        CustomAssets =
            [ { CustomAsset.Default with Id = "hero"; Name = "Hero"; Type = "sprite"; DataUrl = "data:image/png;base64," + Convert.ToBase64String png }
              // Two ids that differ only in case get two files, as on Windows and macOS.
              { CustomAsset.Default with Id = "Hero"; Name = "Hero 2"; Type = "sprite"; DataUrl = "data:image/png;base64," + Convert.ToBase64String png }
              // Not a base64 data URL: it stays inline.
              { CustomAsset.Default with Id = "odd"; Name = "Odd"; Type = "sprite"; DataUrl = "" } ] }

[<Fact>]
let ``every template and fixture project round trips through a folder`` () =
    let projects =
        [ yield! templates ()
          yield "fixture-project", projectOf (readJson [ "Fixtures"; "fixture-project.json" ])
          yield "kitchen-sink", projectOf (Json.get "project" (repoJson [ "fixtures"; "parity"; "kitchen-sink.json" ]))
          yield "art", withArt (starter ()) ]
    for name, project in projects do
        assertSameStable name (Json.stableStringify (ProjectLoad.toJson project)) (Json.stableStringify (joined project))

[<Fact>]
let ``the folder has one file per scene npc dialogue item and asset`` () =
    let project = withArt (starter ())
    let files = ProjectFolder.toFiles project
    let paths = files |> List.map fst
    Assert.Equal(ProjectFolder.MainFile, List.head paths)
    for scene in project.Scenes do Assert.Contains(sprintf "scenes/%s.json" scene.Id, paths)
    for npc in project.Npcs do Assert.Contains(sprintf "npcs/%s.json" npc.Id, paths)
    for item in project.Items do Assert.Contains(sprintf "items/%s.json" item.Id, paths)
    Assert.Equal(project.Dialogues.Length, paths |> List.filter (fun p -> p.StartsWith "dialogue/") |> List.length)
    Assert.Equal<string list>([ "assets/hero.png"; "assets/Hero~2.png" ], paths |> List.filter (fun p -> p.StartsWith "assets/"))
    Assert.Equal<byte[]>(png, files |> List.find (fun (p, _) -> p = "assets/hero.png") |> snd)
    // The project file lists the files in order and keeps no art inline.
    let main = textOf files ProjectFolder.MainFile
    Assert.DoesNotContain("base64", main)
    Assert.Contains("\"scenes/scene-farm.json\"", main)
    Assert.Contains("\"dataFile\": \"assets/hero.png\"", main)
    // A scene's tiles are a palette and one row of indices per row of the map.
    let sceneFile = textOf files "scenes/scene-farm.json"
    Assert.Contains("\"tileGrid\"", sceneFile)
    Assert.DoesNotContain("\"tiles\"", sceneFile)
    let inlined = Json.stringifyIndented (SchemaJson.encodeScene (farm project))
    Assert.True(sceneFile.Length * 4 < inlined.Length, sprintf "the grid (%d) is much smaller than the tiles (%d)" sceneFile.Length inlined.Length)

[<Fact>]
let ``the same project always writes the same files`` () =
    let project = withArt (starter ())
    Assert.Equal<(string * byte[]) list>(ProjectFolder.toFiles project, ProjectFolder.toFiles (roundTrip project))

[<Fact>]
let ``a folder with a missing or broken file says which`` () =
    let files = ProjectFolder.toFiles (starter ())
    let without path = folder (files |> List.filter (fun (p, _) -> p <> path))
    match ProjectFolder.join (without "scenes/scene-farm.json") with
    | Error errors -> Assert.Contains("scenes/scene-farm.json is missing from the project folder", errors)
    | Ok _ -> failwith "expected an error"
    let broken = folder (files |> List.map (fun (p, b) -> if p = "items/crop-wheat.json" then p, Encoding.UTF8.GetBytes "{ nope" else p, b))
    match ProjectFolder.join broken with
    | Error errors -> Assert.Contains(errors, fun e -> e.StartsWith "items/crop-wheat.json is not valid JSON")
    | Ok _ -> failwith "expected an error"
    // A path outside the folder's layout is never read.
    let main = textOf files ProjectFolder.MainFile
    let escaped = main.Replace("\"scenes/scene-farm.json\"", "\"../secret.json\"")
    let sneaky = folder ((ProjectFolder.MainFile, Encoding.UTF8.GetBytes escaped) :: ("../secret.json", Encoding.UTF8.GetBytes "{}") :: List.tail files)
    match ProjectFolder.join sneaky with
    | Error errors -> Assert.Contains("../secret.json is not a file of a project folder", errors)
    | Ok _ -> failwith "expected an error"
    // A bad grid row names the file and the row.
    let grid = folder (files |> List.map (fun (p, b) -> if p = "scenes/scene-farm.json" then p, Encoding.UTF8.GetBytes((Encoding.UTF8.GetString b).Replace("\"rows\": [\n", "\"rows\": [\n      \"99\",\n")) else p, b))
    match ProjectFolder.join grid with
    | Error errors -> Assert.Contains(errors, fun e -> e.StartsWith "scenes/scene-farm.json: tileGrid row 0: \"99\" is not a tile of the palette")
    | Ok _ -> failwith "expected an error"

[<Fact>]
let ``inline entries in a project folder are read as written`` () =
    // A hand-made project.json may keep a scene or an asset inline.
    let project = withArt (starter ())
    let json = ProjectLoad.toJson project
    let files = [ ProjectFolder.MainFile, Encoding.UTF8.GetBytes(Json.stringify json) ]
    match ProjectFolder.load (folder files) with
    | { Ok = true; Data = Some loaded } -> Assert.Equal(project.Scenes.Length, loaded.Scenes.Length)
    | result -> failwith (String.concat "; " result.Errors)
