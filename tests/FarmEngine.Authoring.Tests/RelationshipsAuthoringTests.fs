module FarmEngine.Authoring.Tests.RelationshipsAuthoringTests

// M3 and M4 in the authoring core: the Problems the new conditions, outcomes, triggers, heart
// events and relationship rules raise, what removing an NPC takes with it, the People edit, the
// event form's trigger-dependent fields and Keep changes for the new state.

open Xunit
open FarmEngine.Authoring
open FarmEngine.Authoring.Tests.TestProjects
open FarmEngine.Schemas

let private codes (project: GameProject) = Problems.collect project |> List.map (fun p -> p.Code)

let private withEvent (event: GameEvent) (project: GameProject) = project |> apply (UpsertEvent event)

let private event (project: GameProject) trigger conditions outcomes =
    { Defaults.newEvent project with Trigger = trigger; Conditions = conditions; Outcomes = outcomes }

[<Fact>]
let ``the new conditions and outcomes report what they miss`` () =
    let project = starter ()
    let outcome kind = { EventOutcome.Default with Type = kind }
    let broken =
        event project EventTriggers.Tick
            [ EventCondition.All { Conditions = [ EventCondition.Variable { VariableCondition.Default with Name = "never-set" } ] }
              EventCondition.Hearts { NpcId = "npc-ghost"; Min = Some 1.0; Max = None }
              EventCondition.Scene { SceneIds = [ "scene-ghost" ] } ]
            [ { outcome EventOutcomeTypes.AddShopStock with ShopId = Some "shop-ghost"; ItemId = Some "item-ghost" }
              { outcome EventOutcomeTypes.PlaceNode with TileX = Some 0.0; TileY = Some 0.0; NodeTypeId = Some "node-ghost" }
              outcome EventOutcomeTypes.SetVariable
              { outcome EventOutcomeTypes.SetWeather with WeatherId = Some "hail" } ]
    let found = codes (project |> withEvent broken)
    for code in
        [ "event.conditionUnknownVariable"; "event.conditionUnknownNpc"; "event.conditionUnknownScene"; "event.outcomeUnknownShop"
          "event.outcomeUnknownItem"; "event.outcomeUnknownNodeType"; "event.outcomeMissingField"; "event.outcomeUnknownWeather" ] do
        Assert.Contains(code, found)
    // A variable an outcome sets is known.
    let set = event project EventTriggers.DayStart [] [ { outcome EventOutcomeTypes.AddVariable with Variable = Some "never-set"; Amount = Some 1.0 } ]
    Assert.DoesNotContain("event.conditionUnknownVariable", codes (project |> withEvent broken |> withEvent { set with Id = "event-set" }))

[<Fact>]
let ``time events need a minute and subjects must exist`` () =
    let project = starter ()
    let time = event project EventTriggers.Time [] [ { EventOutcome.Default with Type = EventOutcomeTypes.Message; Message = Some "Noon" } ]
    Assert.Contains("event.timeWithoutMinute", codes (project |> withEvent time))
    Assert.DoesNotContain("event.timeWithoutMinute", codes (project |> withEvent { time with Minute = Some 720.0 }))
    let sold = { time with Trigger = EventTriggers.Sell; Subject = Some "item-ghost" }
    Assert.Contains("event.unknownSubject", codes (project |> withEvent sold))
    let item = project.Items.Head.Id
    Assert.DoesNotContain("event.unknownSubject", codes (project |> withEvent { sold with Subject = Some item }))

[<Fact>]
let ``heart events and relationship rules name what exists`` () =
    let project = starter ()
    let heart = { Defaults.newHeartEvent project with NpcId = "npc-ghost"; CutsceneId = "cutscene-ghost"; SceneId = Some "scene-ghost" }
    let withHeart = project |> apply (UpsertHeartEvent heart)
    for code in [ "heartEvent.unknownNpc"; "heartEvent.unknownCutscene"; "heartEvent.unknownScene" ] do
        Assert.Contains(code, codes withHeart)
    let rules = { RelationshipSettings.Default with Marriage = true; DatingItemId = Some "item-ghost" }
    let withRules = project |> apply (SettingsForm.People(project, false, rules))
    for code in [ "relationships.unknownItem"; "relationships.noProposal"; "relationships.noRomanceable" ] do
        Assert.Contains(code, codes withRules)

[<Fact>]
let ``removing an NPC takes its heart events with it`` () =
    let project = starter ()
    let npc = project.Npcs.Head
    let withHeart = project |> apply (UpsertHeartEvent { Defaults.newHeartEvent project with NpcId = npc.Id })
    Assert.Equal(1, withHeart.HeartEvents.Value.Length)
    let removed = withHeart |> apply (RemoveNpc npc.Id)
    Assert.True(removed.HeartEvents.IsNone)

[<Fact>]
let ``the People edit sets the creator and the rules as one step`` () =
    let project = starter ()
    let doc = Document.create project |> Document.apply (SettingsForm.People(project, true, RelationshipSettings.Default))
    Assert.Equal(Some true, doc.Project.Settings.CharacterCreator)
    Assert.Equal(Some RelationshipSettings.Default, doc.Project.Settings.Relationships)
    Assert.Equal(project, (Document.undo doc).Project)
    // Nothing changed: an empty batch.
    Assert.Same(doc.Project, (doc |> Document.apply (SettingsForm.People(doc.Project, true, RelationshipSettings.Default))).Project)

[<Fact>]
let ``an event's subject and minute follow its trigger`` () =
    Assert.Contains("Subject", Vocabulary.hiddenProperties "GameEvent" EventTriggers.Tick)
    Assert.Contains("Minute", Vocabulary.hiddenProperties "GameEvent" EventTriggers.Tick)
    Assert.DoesNotContain("Minute", Vocabulary.hiddenProperties "GameEvent" EventTriggers.Time)
    Assert.DoesNotContain("Subject", Vocabulary.hiddenProperties "GameEvent" EventTriggers.Harvest)
    Assert.Equal(Some(FieldRole.Reference(ReferenceKind.Item, Some "(any item)")), Vocabulary.roleFor "GameEvent" "Subject" EventTriggers.Sell)
    Assert.Equal(Some(FieldRole.Reference(ReferenceKind.Npc, Some "(anyone)")), Vocabulary.roleFor "GameEvent" "Subject" EventTriggers.Friendship)
    Assert.Equal(Some(FieldRole.Reference(ReferenceKind.Quest, Some "(any quest)")), Vocabulary.roleFor "GameEvent" "Subject" EventTriggers.Quest)
    Assert.Equal(FieldKind.Conditions, (Vocabulary.conditionFields "any" |> List.exactlyOne).Kind)

[<Fact>]
let ``keep changes writes the player character and keeps children and shop changes`` () =
    let project = starter ()
    let state =
        Json.parse
            """{"player":{"x":3,"y":4,"direction":"down","sceneId":"scene-farm","inventory":[],"maxInventorySize":24,"money":5,
                "energy":100,"maxEnergy":100,"skills":{},"activeQuests":[],"completedQuests":[],"name":"Ada","farmName":"Dew Farm",
                "look":{"shirt":"#c23a3a"}},
               "clock":{"tick":40,"timeMinutes":400,"day":1,"season":"spring","year":1,"weatherId":"sun"},
               "world":{"scenes":[]},"npcs":{},"quests":{},"flags":{},"animals":[],"social":{},"quarantinedItems":[],
               "mine":{"deepestFloor":0,"currentFloor":0},"rng":{"algorithm":"xoshiro128ss","s":[1,2,3,4]},
               "music":"track","children":[{"name":"Pip","parentId":"npc-a","bornDay":3}],
               "shopChanges":{"shop":{"removed":["seed"]}}}"""
        |> function
            | Ok json -> json
            | Error error -> failwith error
    match Playtest.applyState project state with
    | Error error -> failwith error
    | Ok kept ->
        Assert.Equal(Some "Ada", kept.Player.Name)
        Assert.Equal(Some "Dew Farm", kept.Player.FarmName)
        Assert.Equal(Some "#c23a3a", kept.Player.Look |> Option.bind (fun look -> look.Shirt))
        let keptState = Json.stringify kept.KeptState.Value
        Assert.Contains("\"music\":\"track\"", keptState)
        Assert.Contains("\"children\":[", keptState)
        Assert.Contains("\"shopChanges\":{", keptState)
