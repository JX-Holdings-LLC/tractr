/// Problems that read the whole project (flags set anywhere outcomes live, events that fire
/// once, NPC looks), templates without warnings, the built-in food kind and the folder helpers.
module FarmEngine.Authoring.Tests.AuthoringEdgesTests

open System.IO
open Xunit
open FarmEngine.Authoring
open FarmEngine.Authoring.Tests.TestProjects
open FarmEngine.Schemas

let private codes (code: string) (project: GameProject) : Problem list =
    Problems.collect project |> List.filter (fun p -> p.Code = code)

let private flagCondition (flag: string) (value: bool) =
    EventCondition.Flag
        { FlagCondition.Default with
            Flag = flag
            Value = value
        }

let private setFlag (flag: string) =
    { EventOutcome.Default with
        Type = EventOutcomeTypes.SetFlag
        FlagName = Some flag
    }

let private message (text: string) =
    { EventOutcome.Default with
        Type = EventOutcomeTypes.Message
        Message = Some text
    }

/// The starter project with an event that checks `flag`, so the flag lint has something to say.
let private checking (flag: string) (project: GameProject) =
    project
    |> apply (
        UpsertEvent
            { Defaults.newEvent project with
                Id = "event-check-" + flag
                Trigger = EventTriggers.Interact
                Conditions =
                    [
                        EventCondition.InteractTile
                            { InteractTileCondition.Default with
                                X = 1.0
                                Y = 1.0
                            }
                        flagCondition flag true
                    ]
                Outcomes = [ message "Lit!" ]
            }
    )

[<Fact>]
let ``a flag a cutscene outcomes step sets is known to the flag lint`` () =
    let project = checking "lanterns-lit" (starter ())
    Assert.NotEmpty(codes "event.conditionUnknownFlag" project)

    let cutscene =
        { Cutscene.Default with
            Id = "cutscene-lanterns"
            Name = "Lanterns"
            Steps =
                [
                    { CutsceneStep.Default with
                        Type = "outcomes"
                        Outcomes = [ setFlag "lanterns-lit" ]
                    }
                ]
        }

    let withCutscene =
        { project with
            Cutscenes = Some [ cutscene ]
        }

    Assert.Empty(codes "event.conditionUnknownFlag" withCutscene)

[<Fact>]
let ``flags set by quest onFail and nested outcome lists count too`` () =
    let project = checking "quest-gave-up" (starter ())

    let failing =
        { project with
            Quests =
                project.Quests
                |> List.mapi (fun i quest ->
                    if i = 0 then
                        { quest with
                            OnFail = Some [ setFlag "quest-gave-up" ]
                        }
                    else
                        quest)
        }

    Assert.Empty(codes "event.conditionUnknownFlag" failing)
    // A clearFlag or a variable outcome makes a name known; a message does not.
    let viaJson (outcome: string) =
        let json = ProjectLoad.toJson project

        let actions =
            match Json.get "actions" json with
            | JArray actions -> actions
            | _ -> []

        let action =
            match Json.parse (sprintf "{\"id\":\"action-x\",\"name\":\"X\",\"outcomes\":[%s]}" outcome) with
            | Ok action -> action
            | Error e -> failwith e

        projectOf (Json.set "actions" (JArray(actions @ [ action ])) json)

    Assert.Empty(codes "event.conditionUnknownFlag" (viaJson "{\"type\":\"clearFlag\",\"flagName\":\"quest-gave-up\"}"))
    Assert.NotEmpty(codes "event.conditionUnknownFlag" (viaJson "{\"type\":\"message\",\"message\":\"quest-gave-up\"}"))

[<Fact>]
let ``an enter event that fires once is not reported as firing on every step`` () =
    let project = starter ()

    let enter repeatable conditions outcomes =
        project
        |> apply (
            UpsertEvent
                { Defaults.newEvent project with
                    Id = "event-welcome"
                    SceneId = "scene-farm"
                    Trigger = EventTriggers.Enter
                    Repeatable = repeatable
                    Conditions = conditions
                    Outcomes = outcomes
                }
        )

    let reported (p: GameProject) =
        codes "event.enterWithoutTile" p
        |> List.exists (fun problem -> problem.Path = sprintf "events[%d].conditions" (p.Events.Length - 1))

    // Repeatable without a tile: fires on every step.
    Assert.True(reported (enter true [] [ message "Welcome!" ]))
    // Not repeatable: fires once.
    Assert.False(reported (enter false [] [ message "Welcome!" ]))
    // Repeatable, but its outcome sets the flag its condition needs false: once.
    Assert.False(reported (enter true [ flagCondition "welcomed" false ] [ message "Welcome!"; setFlag "welcomed" ]))
    // A flag it needs false that nothing here sets does not stop it.
    Assert.True(reported (enter true [ flagCondition "welcomed" false ] [ message "Welcome!"; setFlag "other" ]))

[<Fact>]
let ``every template has no Problems at warning level or above`` () =
    for template in ProjectCatalog.All do
        let project = ProjectCatalog.CreateProjectForTemplate(template, fixedTime)

        let loud =
            Problems.collect project |> List.filter (fun p -> p.Severity <> Severity.Info)

        Assert.True(loud.IsEmpty, sprintf "%s: %s" template (describe loud))

[<Fact>]
let ``the fishing minigame needs no result tiers, other minigames do`` () =
    let project = starter ()
    Assert.Contains(project.Minigames, fun m -> m.Id = ExtensibilitySchema.FishingMinigameId && m.ResultTiers.IsEmpty)
    Assert.Empty(codes "minigame.noTiers" project)

    let other =
        { project with
            Minigames =
                project.Minigames
                @ [
                    { MinigameDef.Default with
                        Id = "minigame-darts"
                        Name = "Darts"
                    }
                ]
        }

    let problem = Assert.Single(codes "minigame.noTiers" other)
    Assert.Equal(sprintf "minigames[%d].resultTiers" (other.Minigames.Length - 1), problem.Path)

/// `fixtures/art/npc-appearances.json`: the cases the Rust renderer checks too.
let private appearanceCases () =
    let path =
        Path.Combine(__SOURCE_DIRECTORY__, "..", "..", "fixtures", "art", "npc-appearances.json")

    match Json.parse (File.ReadAllText path) with
    | Ok json ->
        match Json.get "cases" json with
        | JArray cases ->
            cases
            |> List.map (fun case ->
                let text key =
                    Json.get key case |> Json.asString |> Option.defaultValue ""

                text "appearance", text "sheet", Json.get "exact" case = JBool true)
        | _ -> failwith "no cases"
    | Error message -> failwith message

[<Fact>]
let ``NPC looks pick the sheets the renderer picks`` () =
    let cases = appearanceCases ()
    Assert.NotEmpty cases

    for appearance, sheet, exact in cases do
        Assert.Equal(sheet, BuiltinLooks.npcSheet appearance)
        Assert.Equal(exact, (BuiltinLooks.exactNpcSheet appearance).IsSome)

[<Fact>]
let ``the built-in looks list matches the art pack's manifest`` () =
    let path =
        Path.Combine(__SOURCE_DIRECTORY__, "..", "..", "assets", "builtin-art", "manifest.json")

    let names =
        match Json.parse (File.ReadAllText path) with
        | Ok json ->
            match Json.get "entries" json with
            | JArray entries -> entries |> List.choose (fun e -> Json.get "name" e |> Json.asString)
            | _ -> []
        | Error message -> failwith message

    Assert.NotEmpty names
    // Add a new look to BuiltinLooks.Entries (src/FarmEngine.Authoring/BuiltinLooks.fs).
    Assert.Equal<string list>(List.sort names, List.sort BuiltinLooks.Entries)

[<Fact>]
let ``an NPC appearance that names no built-in look gets a tip naming its fallback`` () =
    let project = starter ()

    let looking (appearance: string) =
        { project with
            Npcs =
                project.Npcs
                |> List.mapi (fun i npc -> if i = 0 then { npc with Appearance = appearance } else npc)
        }

    let tip = Assert.Single(codes "npc.appearanceFallback" (looking "wizard"))
    Assert.Equal(Severity.Info, tip.Severity)
    Assert.Equal("npcs[0].appearance", tip.Path)
    Assert.Contains("npc-villager", tip.Message)
    Assert.Contains("npc-merchant", (Assert.Single(codes "npc.appearanceFallback" (looking "shopkeeper"))).Message)

    for quiet in [ "farmer"; "merchant"; "villager"; "" ] do
        Assert.Empty(codes "npc.appearanceFallback" (looking quiet))
    // An NPC with its own art does not use the built-in looks.
    let ownArt =
        { project with
            Npcs =
                project.Npcs
                |> List.mapi (fun i npc ->
                    if i = 0 then
                        { npc with
                            Appearance = "wizard"
                            Visual =
                                Some
                                    { VisualRef.Default with
                                        AssetId = "asset-wizard"
                                    }
                        }
                    else
                        npc)
        }

    Assert.Empty(codes "npc.appearanceFallback" ownArt)

[<Fact>]
let ``food is a built-in edible kind and the cooked template items are food`` () =
    let food =
        Builtin.itemCategories ()
        |> List.find (fun category -> category.Id = ItemTypes.Food)

    Assert.True food.Edible
    Assert.True food.Giftable
    Assert.True food.Sellable
    Assert.Equal(0.0, food.Energy)
    let project = starter ()

    for id in [ "food-veggie-soup"; "food-fruit-tart" ] do
        let item = project.Items |> List.find (fun item -> item.Id = id)
        Assert.Equal(ItemTypes.Food, item.Type)
        Assert.True(defaultArg item.Energy 0.0 > 0.0, id)

[<Fact>]
let ``folder helpers take toFiles' list and give a project's single-file text`` () =
    let project = starter ()
    let files = ProjectFolder.toFiles project

    let loaded = ProjectFolder.loadList files
    Assert.True(loaded.Ok, String.concat "\n" loaded.Errors)
    Assert.Equal(Ok(ProjectFolder.toSingleFileText project), ProjectFolder.joinListText files)
    Assert.Equal(ProjectFolder.joinText (ProjectFolder.ofList files), ProjectFolder.joinListText files)
    Assert.True((ProjectLoad.loadProjectText (ProjectFolder.toSingleFileText project)).Ok)
