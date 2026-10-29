module FarmEngine.Authoring.Tests.FishingForagingTests

open FarmEngine.Authoring
open FarmEngine.Schemas
open FarmEngine.Authoring.Tests.TestProjects
open Xunit

[<Fact>]
let ``forage editor preview excludes doors paths and blocked tiles`` () =
    let project = starter ()
    let scene = project.Scenes.Head

    let table =
        { ForageSpawnTable.Default with
            Id = "wild"
            Maximum = 2.0
            Terrains = [ "grass"; "soil" ]
        }

    let points = SettingsForms.forageEligibleTiles project scene table
    Assert.NotEmpty(points)

    for point in points do
        let tile = scene.Tiles.[int point.Y].[int point.X]
        Assert.False(tile.Collision)
        Assert.True(tile.Overlay.IsNone)
        Assert.True(tile.Object.IsNone)
        Assert.DoesNotContain(scene.Transitions, fun door -> door.FromX = point.X && door.FromY = point.Y)

[<Fact>]
let ``forage scene edits undo and preserve table schema`` () =
    let project = starter ()

    let table =
        { ForageSpawnTable.Default with
            Id = "wild"
            Maximum = 2.0
            Terrains = [ "grass" ]
            LifetimeDays = Some 3.0
            Entries =
                [
                    {
                        NodeTypeId = "node-rock"
                        Weight = 1.0
                    }
                ]
        }

    let edited =
        Document.create project
        |> Document.apply (SetSceneForageSpawns(project.Scenes.Head.Id, [ table ]))

    Assert.Equal(Some [ table ], edited.Project.Scenes.Head.ForageSpawns)
    Assert.True(obj.ReferenceEquals(project, (Document.undo edited).Project))
    Assert.Equal(table, SchemaJson.decodeForageSpawnTable [] (SchemaJson.encodeForageSpawnTable table))

[<Fact>]
let ``fishing constraints report inverted time and size ranges`` () =
    let project = starter ()

    let rules =
        { FishingRules.Default with
            MinSize = Some 20.0
            MaxSize = Some 10.0
        }

    let filter =
        { FishingFilter.Default with
            StartMinute = Some 1000.0
            EndMinute = Some 600.0
        }

    let table =
        { FishTable.Default with
            Id = "bad"
            Filter = Some filter
            Entries =
                [
                    { FishTableEntry.Default with
                        ItemId = "fish-carp"
                        Weight = 1.0
                        Rules = Some rules
                    }
                ]
        }

    let problems = SchemaChecks.projectLints { project with FishTables = [ table ] }
    Assert.Contains(problems, fun issue -> issue.Message.Contains("Start minute"))
    Assert.Contains(problems, fun issue -> issue.Message.Contains("Minimum size"))

[<Fact>]
let ``keep playtest changes retains catch sizes and forage expiry`` () =
    let project = starter ()

    let node =
        { TileNode.Default with
            TypeId = "node-rock"
            RemainingHealth = 1.0
            SpawnedOnDay = Some 1.0
            SpawnedSeason = Some "spring"
            ExpiresOnDay = Some 4.0
        }

    let scene = project.Scenes.Head

    let tiles =
        scene.Tiles
        |> List.mapi (fun y row ->
            row
            |> List.mapi (fun x tile ->
                if x = 0 && y = 0 then
                    { tile with Node = Some node }
                else
                    tile))

    let project =
        { project with
            Scenes = { scene with Tiles = tiles } :: project.Scenes.Tail
        }

    let encoded = SchemaJson.encodeGameProject project

    let catches =
        JArray
            [
                JObject [ "itemId", JString "fish-carp"; "size", JNumber 42.0; "day", JNumber 1.0 ]
            ]

    let state =
        JObject
            [
                "player", Json.get "player" encoded
                "world", JObject [ "scenes", Json.get "scenes" encoded ]
                "clock",
                JObject
                    [
                        "tick", JNumber 0.0
                        "timeMinutes", JNumber 360.0
                        "day", JNumber 1.0
                        "season", JString "spring"
                        "dayOfSeason", JNumber 1.0
                        "year", JNumber 1.0
                        "weatherId", JString "sun"
                    ]
                "npcs", JObject []
                "quests", JObject []
                "flags", JObject []
                "animals", JArray []
                "social", JObject []
                "quarantinedItems", JArray []
                "mine", JObject [ "deepestFloor", JNumber 0.0; "currentFloor", JNumber 0.0 ]
                "rng",
                JObject
                    [
                        "algorithm", JString "xoshiro128ss"
                        "s", JArray [ JNumber 1.0; JNumber 2.0; JNumber 3.0; JNumber 4.0 ]
                    ]
                "fishCatches", catches
            ]

    match Playtest.applyState project state with
    | Error error -> failwith error
    | Ok kept ->
        Assert.Equal(catches, Json.get "fishCatches" kept.KeptState.Value)
        Assert.Equal(Some node, kept.Scenes.Head.Tiles.Head.Head.Node)
