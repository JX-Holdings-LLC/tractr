module FarmEngine.Authoring.Tests.QuestsMinesTests

open Xunit
open FarmEngine.Authoring
open FarmEngine.Authoring.Tests.TestProjects
open FarmEngine.Schemas

[<Fact>]
let ``named mines and request boards edit roundtrip and compile`` () =
    let project = starter ()

    let mine =
        { Defaults.newMine project with
            Config = { project.Mine with Enabled = true }
        }

    let quest =
        { Defaults.newQuest project with
            DeadlineDays = Some 3.0
            DeadlineDay = Some 10.0
            OnFail = Some [ Defaults.defaultOutcome "message" ]
        }

    let board =
        { Defaults.newRequestBoard project with
            Entries =
                [
                    { RequestBoardEntry.Default with
                        QuestId = quest.Id
                        MinQuantity = Some 2.0
                        MaxQuantity = Some 4.0
                        MinReward = Some 20.0
                        MaxReward = Some 50.0
                    }
                ]
        }

    let changed =
        project
        |> apply (UpsertMine mine)
        |> apply (UpsertQuest quest)
        |> apply (UpsertRequestBoard board)

    Assert.Contains(mine.Id, Defaults.allIds changed)
    Assert.Contains(board.Id, Defaults.allIds changed)

    let decoded =
        Decode.run SchemaJson.decodeGameProject (SchemaJson.encodeGameProject changed)

    match decoded with
    | Error error -> failwith error
    | Ok restored ->
        Assert.Equal(changed.Mines, restored.Mines)
        Assert.Equal(changed.RequestBoards, restored.RequestBoards)
        Assert.Equal(quest, restored.Quests |> List.find (fun q -> q.Id = quest.Id))

    Assert.True((changed |> apply (RemoveMine mine.Id)).Mines.IsNone)
    Assert.True((changed |> apply (RemoveRequestBoard board.Id)).RequestBoards.IsNone)
    let content = ContentCompiler.compile changed
    Assert.Equal(changed.Mines, content.Mines)
    Assert.Equal(changed.RequestBoards, content.RequestBoards)

[<Fact>]
let ``new quest and mine fields stay absent in legacy projects`` () =
    let project = starter ()
    let json = SchemaJson.encodeGameProject project
    Assert.Equal(None, Json.tryGet "mines" json)
    Assert.Equal(None, Json.tryGet "requestBoards" json)
    let quest = SchemaJson.encodeQuest (Defaults.newQuest project)
    Assert.Equal(None, Json.tryGet "deadlineDays" quest)
    Assert.Equal(None, Json.tryGet "onFail" quest)

[<Fact>]
let ``Problems reports duplicate mine ids and missing board templates`` () =
    let project = starter ()

    let mine =
        { Defaults.newMine project with
            Id = "mine"
        }

    let board =
        { Defaults.newRequestBoard project with
            SceneId = "missing-scene"
            Entries =
                [
                    { RequestBoardEntry.Default with
                        QuestId = "missing-quest"
                    }
                ]
        }

    let changed =
        { project with
            Mines = Some [ mine ]
            RequestBoards = Some [ board ]
        }

    let codes = Problems.collect changed |> List.map (fun problem -> problem.Code)
    Assert.Contains("mine.duplicateId", codes)
    Assert.Contains("requestBoard.location", codes)
    Assert.Contains("requestBoard.quest", codes)

[<Fact>]
let ``Keep changes retains named mines and accepted request snapshots`` () =
    let json text =
        match Json.parse text with
        | Ok value -> value
        | Error error -> failwith error

    let project = SchemaJson.encodeGameProject (starter ())

    let state =
        json
            """{"clock":{"tick":0},"mine":{"currentFloor":6,"deepestFloor":6},"mines":{"mine":{"currentFloor":0,"deepestFloor":10}},"currentMineId":"volcano","requestBoards":{"town":2},"quests":{"errand":{"status":"active","deadlineDay":5,"request":{"id":"errand","name":"Rolled request"}}}}"""

    let kept = Playtest.applyStateJson project state |> Json.get "keptState"
    Assert.Equal(JString "volcano", Json.get "currentMineId" kept)
    Assert.Equal(JNumber 10.0, Json.get "mines" kept |> Json.get "mine" |> Json.get "deepestFloor")
    Assert.Equal(JNumber 5.0, Json.get "questDeadlines" kept |> Json.get "errand")
    Assert.Equal(JNumber 2.0, Json.get "requestBoards" kept |> Json.get "town")
    Assert.Equal(JString "Rolled request", Json.get "requests" kept |> Json.get "errand" |> Json.get "name")

[<Fact>]
let ``mine and board map markers keep their entry ids and follow scene resizing`` () =
    let project = starter ()
    let scene = project.Scenes.Head

    let mine =
        { Defaults.newMine project with
            Config =
                { MineConfig.Default with
                    Enabled = true
                    EntranceSceneId = Some scene.Id
                    EntranceX = Some 7.0
                    EntranceY = Some 7.0
                }
        }

    let board =
        { Defaults.newRequestBoard project with
            SceneId = scene.Id
            X = 6.0
            Y = 6.0
        }

    let changed =
        { project with
            Mines = Some [ mine ]
            RequestBoards = Some [ board ]
        }

    let markers = MapMarkers.ForScene(changed, scene.Id)
    Assert.Contains(markers, fun marker -> marker.Kind = "mine" && marker.TargetId = mine.Id)
    Assert.Contains(markers, fun marker -> marker.Kind = "request-board" && marker.TargetId = board.Id)
    let resized = Cleanup.clampToScene scene.Id 4 4 changed
    Assert.Equal(Some 3.0, resized.Mines.Value.Head.Config.EntranceX)
    Assert.Equal(3.0, resized.RequestBoards.Value.Head.X)
