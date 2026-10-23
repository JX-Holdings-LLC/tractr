/// Loading projects and exported games (`ProjectLoad`, `ProjectJson`): the current schema version
/// and version 1 (read as the current one) load, and a refusal always says why.
module FarmEngine.Authoring.Tests.ProjectLoadTests

open System.Text.Json.Nodes
open Xunit
open FarmEngine.Authoring
open FarmEngine.Authoring.Net
open FarmEngine.Schemas

let private current = JsNumber.format ProjectSchema.CurrentProjectSchemaVersion

let private starterText () = ProjectLoad.toText (TestProjects.starter ())

/// The starter project's JSON with `patch` applied.
let private patched (patch: JsonObject -> unit) : string =
    match JsonNode.Parse(starterText ()) with
    | :? JsonObject as o ->
        patch o
        o.ToJsonString()
    | _ -> failwith "the starter project is not an object"

let private stableProject (data: GameProject option) =
    match data with
    | None -> "null"
    | Some project -> Json.stableStringify (SchemaJson.encodeGameProject project)

[<Fact>]
let ``a project of the current schema version loads`` () =
    let result = ProjectJson.loadProjectText (starterText ())
    Assert.True(result.Ok, String.concat "\n" result.Errors)
    Assert.Equal(ProjectSchema.CurrentProjectSchemaVersion, result.Data.Value.SchemaVersion)

[<Fact>]
let ``a version 1 project loads as the current version`` () =
    let result = ProjectJson.loadProjectText (patched (fun o -> o["schemaVersion"] <- JsonValue.Create 1))
    Assert.True(result.Ok, String.concat "\n" result.Errors)
    Assert.Equal(ProjectSchema.CurrentProjectSchemaVersion, result.Data.Value.SchemaVersion)

[<Fact>]
let ``other schema versions are refused`` () =
    let errors (text: string) = (ProjectJson.loadProjectText text).Errors
    Assert.Equal<string list>(
        [ "Project schema version 0 is not supported (this engine reads version " + current + ")." ],
        errors (patched (fun o -> o["schemaVersion"] <- JsonValue.Create 0)))
    Assert.Equal<string list>(
        [ "Project schema version 3 is newer than this engine supports (" + current + "). Update the engine." ],
        errors (patched (fun o -> o["schemaVersion"] <- JsonValue.Create 3)))
    Assert.Equal<string list>(
        [ "Project data has no schemaVersion (this engine reads version " + current + ")." ],
        errors (patched (fun o -> o.Remove "schemaVersion" |> ignore)))
    Assert.Equal<string list>([ "Project data is not an object" ], errors "[1,2,3]")

let private malformed () : (string * string) list =
    let arrayOf (items: JsonNode list) =
        let array = JsonArray()
        for item in items do
            array.Add item
        array
    [ "money is a string", patched (fun o -> o.["player"].["money"] <- JsonValue.Create "lots")
      "scenes is a string", patched (fun o -> o["scenes"] <- JsonValue.Create "not-a-list")
      "tile row is a number", patched (fun o -> o.["scenes"].[0].["tiles"] <- arrayOf [ JsonValue.Create 3 ])
      "null tile", patched (fun o -> o.["scenes"].[0].["tiles"] <- arrayOf [ arrayOf [ null ] ])
      "customCrops is an object", patched (fun o -> o["customCrops"] <- JsonObject())
      "null event", patched (fun o -> o["events"] <- arrayOf [ null ])
      "name is a number", """{"name":42}"""
      "array payload", "[1,2,3]"
      "empty object", "{}" ]

let malformedCases () : seq<obj[]> = malformed () |> Seq.map (fun (name, _) -> [| box name |])

/// Whatever the input, loading reports instead of throwing, the result is deterministic, and a
/// refused load says why.
[<Theory>]
[<MemberData(nameof malformedCases)>]
let ``loading never throws and always explains a refusal`` (name: string) =
    let text = malformed () |> List.find (fun (n, _) -> n = name) |> snd
    let first = ProjectJson.loadProjectText text
    let second = ProjectJson.loadProject (JsonNode.Parse text)
    Assert.Equal(first.Ok, second.Ok)
    Assert.Equal<string list>(first.Errors, second.Errors)
    Assert.Equal(stableProject first.Data, stableProject second.Data)
    Assert.True(first.Ok || not first.Errors.IsEmpty, name + ": refused without an error")

[<Fact>]
let ``malformed values are reported where they are`` () =
    let errors name = (ProjectJson.loadProjectText (malformed () |> List.find (fun (n, _) -> n = name) |> snd)).Errors
    Assert.Equal<string list>([ "player.money: Expected number, received string" ], errors "money is a string")
    Assert.Equal<string list>([ "scenes: Expected array, received string" ], errors "scenes is a string")

[<Fact>]
let ``loading does not change the caller's node`` () =
    let raw = JsonNode.Parse(starterText ())
    let before = raw.ToJsonString()
    ProjectJson.loadProject raw |> ignore
    Assert.Equal(before, raw.ToJsonString())

[<Fact>]
let ``invalid JSON text is reported, not thrown`` () =
    let result = ProjectJson.loadProjectText "garbage"
    Assert.False result.Ok
    Assert.StartsWith("Project data is not valid JSON: ", result.Errors.[0])

[<Fact>]
let ``whole-number fields with a fraction are reported`` () =
    let project = TestProjects.starter ()
    let item = { project.Items.Head with Value = 12.5 }
    let problems = Problems.collect { project with Items = item :: project.Items.Tail }
    let found = problems |> List.filter (fun p -> p.Code = "numbers.offGrid")
    Assert.Equal<string list>([ "items[0].value" ], found |> List.map (fun p -> p.Path))
    Assert.EndsWith("it plays as 13", found.Head.Message)
    Assert.Empty(Problems.collect project |> List.filter (fun p -> p.Code = "numbers.offGrid"))

/// Web projects (the Romance pattern) can carry an NPC dialogue that differs from its copy in the
/// project list; the NPC's copy is the one the game plays (#43).
[<Fact>]
let ``loading reconciles dialogue copies with the NPC's copy, which the game plays`` () =
    let project = ProjectCatalog.CreateInitialProject 0.0
    let npc = project.Npcs |> List.find (fun n -> n.Id = "npc-farmer")
    let greeting = npc.Dialogue |> List.find (fun d -> d.Id = "dialogue-farmer-greeting")
    let invitation = { DialogueOption.Default with Text = "Would you like to go on a date?"; EventFlag = Some "date-asked" }
    let played = { greeting with Options = greeting.Options @ [ invitation ] }
    let onlyOnNpc = { DialogueOption.Default with Text = "Only on the NPC" } |> fun o -> { greeting with Id = "dialogue-npc-only"; Options = [ o ] }
    let mismatched =
        { project with
            Npcs = project.Npcs |> List.map (fun n -> if n.Id = npc.Id then { n with Dialogue = (n.Dialogue |> List.map (fun d -> if d.Id = greeting.Id then played else d)) @ [ onlyOnNpc ] } else n) }
    Assert.Equal(greeting, mismatched.Dialogues |> List.find (fun d -> d.Id = greeting.Id))
    let loaded =
        match (ProjectLoad.loadProjectText (ProjectLoad.toText mismatched)) with
        | { Ok = true; Data = Some loaded } -> loaded
        | result -> failwithf "%A" result.Errors
    Assert.Equal(played, loaded.Dialogues |> List.find (fun d -> d.Id = greeting.Id))
    Assert.Equal(onlyOnNpc, List.last loaded.Dialogues)
    Assert.Equal(mismatched.Dialogues.Length + 1, loaded.Dialogues.Length)
    Assert.Empty(Problems.collect loaded |> List.filter (fun p -> p.Code = "dialogue.copyMismatch" || p.Code = "dialogue.notInProject"))
    // Saving what the Dialogue editor shows keeps the NPC-only option.
    let shown = ProjectContent.Dialogues mismatched |> Seq.find (fun d -> d.Id = greeting.Id)
    let saved = Document.run mismatched (UpsertDialogue shown)
    let onNpc = (saved.Npcs |> List.find (fun n -> n.Id = npc.Id)).Dialogue |> List.find (fun d -> d.Id = greeting.Id)
    Assert.Contains(invitation, onNpc.Options)
    // A project whose copies agree loads as the same value.
    Assert.Same(project, DialogueCopies.reconcile project)
