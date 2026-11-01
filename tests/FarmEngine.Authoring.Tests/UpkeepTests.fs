module FarmEngine.Authoring.Tests.UpkeepTests

open Xunit
open FarmEngine.Authoring
open FarmEngine.Schemas
open FarmEngine.Authoring.Tests.TestProjects

let private template id =
    ProjectCatalog.CreateProjectForTemplate(id, fixedTime)

let private codes (prefix: string) (project: GameProject) =
    Problems.collect project |> List.filter (fun p -> p.Code.StartsWith prefix)

let private ofSeverity severity (problems: Problem list) =
    problems |> List.filter (fun p -> p.Severity = severity)

// ── The content graph (#231) ─────────────────────────────────────────────────

let templateIds: obj[] seq = ProjectCatalog.All |> Seq.map (fun id -> [| box id |])

[<Theory; MemberData(nameof templateIds)>]
let ``the graph and upkeep checks add no warnings or errors to the templates`` (id: string) =
    let project = template id

    let added =
        Problems.collect project
        |> List.filter (fun p ->
            (p.Code.StartsWith "graph."
             || p.Code.StartsWith "testState."
             || p.Code.StartsWith "saveMigration.")
            && p.Severity <> Severity.Info)

    Assert.True(List.isEmpty added, sprintf "%A" (added |> List.map (fun p -> p.Code, p.Message)))

let private withItem (item: Item) (project: GameProject) =
    { project with
        Items = project.Items @ [ item ]
    }

let private item id name =
    { Item.Default with
        Id = id
        Name = name
        Type = ItemTypes.Material
        Stackable = true
        MaxStack = 99.0
    }

let private recipe id inputs output =
    { RecipeDefinition.Default with
        Id = id
        Name = id
        Inputs = inputs |> List.map (fun i -> { ItemId = i; Quantity = 1.0 })
        Outputs = [ { ItemId = output; Quantity = 1.0 } ]
        Category = "crafting"
    }

[<Fact>]
let ``an ingredient nothing gives gates its recipe and is a warning`` () =
    let project =
        template "blank"
        |> withItem (item "moon-dust" "Moon Dust")
        |> withItem (item "moon-cake" "Moon Cake")

    let project =
        { project with
            Recipes = project.Recipes @ [ recipe "bake-moon-cake" [ "moon-dust" ] "moon-cake" ]
        }

    let graph = codes "graph." project
    let noSource = graph |> List.filter (fun p -> p.Code = "graph.itemNoSource")
    Assert.Contains(noSource, fun p -> p.Message.Contains "Moon Dust" && p.Severity = Severity.Warning)
    Assert.Contains(graph, fun p -> p.Code = "graph.recipeGated" && p.Severity = Severity.Warning)
    // The cake has a source, the recipe, but it needs the dust nobody gives.
    Assert.Contains(graph, fun p -> p.Code = "graph.itemUnobtainable" && p.Message.Contains "Moon Cake")

    let entry =
        ContentGraph.ofProject project
        |> ContentGraph.tryFind GraphKinds.Item "moon-cake"
        |> Option.get

    Assert.Equal("recipe", (List.exactlyOne entry.Sources).Via)
    Assert.Equal<(string * string) list>([ GraphKinds.Item, "moon-dust" ], (List.exactlyOne entry.Sources).Needs)

    // Sold in a shop, the dust is obtainable and so is the cake.
    let shop =
        { ShopDefinition.Default with
            Id = "moon-shop"
            Name = "Moon Shop"
            Stock =
                [
                    { ShopStockEntry.Default with
                        ItemId = "moon-dust"
                    }
                ]
        }

    let fixedProject =
        { project with
            Shops = project.Shops @ [ shop ]
        }

    let after = codes "graph." fixedProject
    Assert.DoesNotContain(after, fun p -> p.Message.Contains "Moon Dust")
    Assert.DoesNotContain(after, fun p -> p.Code <> "graph.itemNoUse" && p.Message.Contains "Moon")

[<Fact>]
let ``circular and dead-end quest prerequisites are found`` () =
    let quest id prerequisites autoStart =
        { Quest.Default with
            Id = id
            Name = id
            Prerequisites = Some prerequisites
            AutoStart = Some autoStart
            Objectives =
                [
                    { QuestObjective.Default with
                        Id = "o"
                        Type = QuestObjectiveTypes.Talk
                    }
                ]
        }

    let project = template "blank"

    let project =
        { project with
            Quests =
                project.Quests
                @ [
                    quest "q-a" [ "q-b" ] true
                    quest "q-b" [ "q-a" ] true
                    quest "q-lonely" [] false
                    quest "q-after-lonely" [ "q-lonely" ] true
                ]
        }

    let graph = codes "graph." project

    let cycle =
        graph |> List.filter (fun p -> p.Code = "graph.questCycle") |> List.exactlyOne

    Assert.Contains("q-a → q-b → q-a", cycle.Message)
    Assert.Contains(graph, fun p -> p.Code = "graph.questDeadEnd" && p.Message.Contains "q-lonely")

    Assert.Contains(
        graph,
        fun p ->
            p.Code = "graph.questNeverStarts"
            && p.Severity = Severity.Info
            && p.Message.Contains "q-lonely"
    )

// ── Test states (#229) ──────────────────────────────────────────────────────

[<Fact>]
let ``test states naming missing content are reported and round-trip as files`` () =
    let project = template "starter"
    let scene = project.Scenes.Head

    let good =
        { TestState.Default with
            Id = "late"
            Name = "Late game"
            Day = Some 20.0
            SceneId = Some scene.Id
            X = Some 1.0
            Y = Some 1.0
            Flags = [ "mill-fixed", JBool true ]
            Variables = [ "bridges", JNumber 2.0 ]
            Money = Some 500.0
        }

    let bad =
        { TestState.Default with
            Id = "broken"
            SceneId = Some "scene-gone"
            Quests = [ "quest-gone", "completed" ]
            Friendship = [ "npc-gone", 250.0 ]
            Inventory =
                Some
                    [
                        { TestStateItem.Default with
                            ItemId = "item-gone"
                        }
                    ]
            Facing = Some "sideways"
        }

    let project = Document.run project (SetTestStates [ good; bad ])
    let problems = codes "testState." project
    Assert.All(problems, fun p -> Assert.StartsWith("testStates[1]", p.Path))

    Assert.Equal<string list>(
        [
            "testState.scene"
            "testState.facing"
            "testState.quest"
            "testState.npc"
            "testState.item"
        ],
        problems |> List.map (fun p -> p.Code)
    )

    match TestStates.ofText (TestStates.toText good) with
    | Ok decoded -> Assert.Equal(good, decoded)
    | Error message -> failwith message

    Assert.StartsWith("{\"type\":\"applyTestState\",\"state\":{\"id\":\"late\"", TestStates.applyAction good)
    Assert.Equal("late-2", TestStates.newId (Document.run project (SetTestStates [ good; { good with Id = "late-1" } ])) "Late")
    // Not compiled into the game.
    Assert.DoesNotContain("mill-fixed", System.Text.Encoding.UTF8.GetString(CartridgeCompiler.Compile project))

// ── Save migrations (#242) ──────────────────────────────────────────────────

[<Fact>]
let ``ids the last export had are orphaned until a rule covers them`` () =
    let project = template "starter"
    let content = ContentCompiler.compile project
    let exported = SaveMigrations.remember content project |> Option.get
    let project = Document.run project (SetExportSettings(Some exported))
    Assert.True((SaveMigrations.remember content project).IsNone)
    Assert.Empty(codes "saveMigration." project)

    let gone = project.Items.Head
    let without = Document.run project (RemoveItem gone.Id)
    Assert.True(SaveMigrations.removalNeedsRule SaveMigrationKinds.Item gone.Id project)
    let orphan = codes "saveMigration.orphan" without |> List.exactlyOne
    Assert.Equal(Severity.Warning, orphan.Severity)
    Assert.Contains(gone.Id, orphan.Message)

    let rule = SaveMigrations.removeItem gone.Id (Some gone.Name) 5.0

    let covered =
        Document.run without (SetSaveMigrations(SaveMigrations.upsert rule without))

    Assert.Empty(codes "saveMigration." covered)
    Assert.False(SaveMigrations.removalNeedsRule SaveMigrationKinds.Item gone.Id covered)

    // The rule reaches the cartridge's content.
    let compiled = ContentCompiler.compile covered
    Assert.Equal<SaveMigration list>([ rule ], defaultArg compiled.SaveMigrations [])

[<Fact>]
let ``bad save migration rules are reported`` () =
    let project = template "starter"
    let kept = project.Items.Head.Id

    let rules =
        [
            { SaveMigration.Default with
                Kind = "npc"
                From = "x"
            }
            SaveMigrations.rename SaveMigrationKinds.Item "old-thing" "no-such-item"
            SaveMigrations.rename SaveMigrationKinds.Item kept "whatever"
            SaveMigrations.rename SaveMigrationKinds.Item "old-thing" kept
        ]

    let problems =
        codes "saveMigration." (Document.run project (SetSaveMigrations rules))

    Assert.Equal<string list>(
        [
            "saveMigration.kind"
            "saveMigration.to"
            "saveMigration.inactive"
            "saveMigration.to"
            "saveMigration.duplicate"
        ],
        problems |> List.map (fun p -> p.Code)
    )

    Assert.Equal(Severity.Info, (problems |> List.find (fun p -> p.Code = "saveMigration.inactive")).Severity)
