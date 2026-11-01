/// #225: dialogue scripts and the condition text they share with events and quests.
module FarmEngine.Authoring.Tests.DialogueScriptTests

open System.Collections.Generic
open Xunit
open FsCheck
open FsCheck.FSharp
open FarmEngine.Authoring
open FarmEngine.Authoring.Tests.TestProjects
open FarmEngine.Schemas

let private issueExample =
    """@npc bram

=== bram.intro
@expression sad
Wheel turns, stones don't. The axle on my grindstone cracked clean through.
* What do you need? -> offer quest-silent-mill
* [if flag mill-fixed] Hear that? -> bram.fixed
* Good luck.

=== bram.lines
[morning] Mornin'. Flour on my boots, flour in my beard.
[weather rain] Rain keeps the river high. Good for the wheel, bad for my knees.
[quest quest-silent-mill active] Ten logs and five good stones.
"""

let private parsed (text: string) =
    match DialogueScript.parse text with
    | Ok document -> document
    | Error errors -> failwithf "%A" errors

[<Fact>]
let ``the example of the issue reads into the dialogue records`` () =
    let document = parsed issueExample
    Assert.Equal(Some "bram", document.Npc)
    let intro = List.exactlyOne document.Dialogues
    Assert.Equal("bram.intro", intro.Id)
    Assert.Equal("bram", intro.NpcId)
    Assert.Equal(Some "sad", intro.Expression)
    Assert.Equal("Wheel turns, stones don't. The axle on my grindstone cracked clean through.", intro.Text)

    match intro.Options with
    | [ offer; fixedOption; bye ] ->
        Assert.Equal("What do you need?", offer.Text)
        Assert.Equal(Some "quest-silent-mill", offer.OfferQuestId)
        Assert.Equal(None, offer.NextDialogueId)
        Assert.Equal(Some "mill-fixed", fixedOption.RequiresFlag)
        Assert.Equal(Some "bram.fixed", fixedOption.NextDialogueId)
        Assert.Equal("Good luck.", bye.Text)

        Assert.Equal(
            { DialogueOption.Default with
                Text = "Good luck."
            },
            bye
        )
    | other -> failwithf "%A" other

    match document.Lines with
    | Some [ morning; rain; quest ] ->
        Assert.Equal<EventCondition list>([ EventCondition.TimeOfDay { MinMinute = 360.0; MaxMinute = 719.0 } ], morning.Conditions)
        Assert.Equal<EventCondition list>([ EventCondition.Weather { WeatherIds = [ "rain" ] } ], rain.Conditions)

        Assert.Equal<EventCondition list>(
            [
                EventCondition.QuestStatus
                    {
                        QuestId = "quest-silent-mill"
                        Status = "active"
                    }
            ],
            quest.Conditions
        )

        Assert.Equal("Ten logs and five good stones.", quest.Text)
    | other -> failwithf "%A" other

[<Fact>]
let ``the printer writes the example back as written`` () =
    let document = parsed issueExample
    let text, _ = DialogueScript.printDocument document
    Assert.Equal(issueExample, text)

[<Fact>]
let ``options carry their effects ids and every target`` () =
    let text =
        """@npc bram
=== bram.shop
Need anything?
* [if not flag banned, item coin, friendship 250] [once] Show me your wares. -> shop bram-shop -> action wave #line:o-1
  ~ give item bread 2
  ~ give money 10
  ~ take money 3
  ~ set flag saw-shop
  ~ extra {"mood":"glad"}
* "Quoted -> text" -> bram.shop
"""

    let dialogue = (parsed text).Dialogues |> List.exactlyOne

    match dialogue.Options with
    | [ wares; quoted ] ->
        Assert.Equal(Some "banned", wares.HiddenIfFlag)
        Assert.Equal(Some "coin", wares.RequiresItem)
        Assert.Equal(Some 250.0, wares.RequiresFriendship)
        Assert.Equal(Some true, wares.Once)
        Assert.Equal(Some "bram-shop", wares.OpenShopId)
        Assert.Equal(Some "wave", wares.ActionId)
        Assert.Equal(Some "o-1", wares.Id)
        Assert.Equal(Some "bread", wares.GiveItem)
        Assert.Equal(Some 2.0, wares.GiveItemQuantity)
        Assert.Equal(Some 10.0, wares.GiveMoney)
        Assert.Equal(Some 3.0, wares.TakeMoney)
        Assert.Equal(Some "saw-shop", wares.EventFlag)
        Assert.Equal<(string * Json) list>([ "mood", JString "glad" ], wares.Extra)
        Assert.Equal("Quoted -> text", quoted.Text)
        Assert.Equal(Some "bram.shop", quoted.NextDialogueId)
    | other -> failwithf "%A" other

[<Fact>]
let ``mistakes are reported with their line`` () =
    let text =
        """@npc bram
=== bram.intro
Hello.
* [if season spring] Nice weather. -> bram.intro
~ give potato
=== bram.lines
[time 6:00..] Morning.
stray
"""

    match DialogueScript.parse text with
    | Ok _ -> failwith "expected errors"
    | Error errors ->
        let lines = errors |> List.map (fun e -> e.Line)
        Assert.Equal<int list>([ 4; 5; 7 ], lines)
        Assert.Contains("for lines, events and quests", errors.[0].Message)

[<Fact>]
let ``check names what the script refers to that the project lacks`` () =
    let project = starter ()
    let npc = project.Npcs |> List.head

    let text =
        sprintf
            """@npc %s
=== %s.new
@expression grumpy
Hi.
* Go. -> nowhere
* Quest. -> offer no-quest
"""
            npc.Id
            npc.Id

    let errors = DialogueScript.check project text
    Assert.Contains(errors, fun e -> e.Line = 5 && e.Message.Contains "no dialogue nowhere")
    Assert.Contains(errors, fun e -> e.Line = 6 && e.Message.Contains "no quest no-quest")
    Assert.Contains(errors, fun e -> e.Line = 2 && e.Message.Contains "grumpy")

[<Fact>]
let ``every NPC of the templates reads back from its script`` () =
    for name, project in templates () do
        for npc in project.Npcs do
            let text = DialogueScript.printNpc npc

            match DialogueScript.applyToNpc npc text with
            | Ok back ->
                Assert.True(back.Dialogue = npc.Dialogue, sprintf "%s/%s dialogue:\n%s" name npc.Id text)
                Assert.True(back.Lines = npc.Lines, sprintf "%s/%s lines:\n%s" name npc.Id text)
            | Error errors -> failwithf "%s/%s: %A\n%s" name npc.Id errors text

[<Fact>]
let ``the script edit gives new lines ids and is one undo step`` () =
    let project = starter ()
    let npc = project.Npcs |> List.head

    let text =
        sprintf "@npc %s\n=== %s.lines\n[season spring] Spring again.\n[season summer] Hot one.\n" npc.Id npc.Id

    match DialogueScript.edit project text with
    | Error errors -> failwithf "%A" errors
    | Ok edit ->
        let document = Document.apply edit (Document.create project)
        Assert.Equal(1, document.Past.Length)
        let changed = document.Project.Npcs |> List.find (fun n -> n.Id = npc.Id)
        let lines = changed.Lines |> Option.defaultValue []
        Assert.Equal(2, lines.Length)
        Assert.True(lines |> List.forall (fun l -> l.Id.IsSome))
        Assert.Equal(2, lines |> List.choose (fun l -> l.Id) |> List.distinct |> List.length)
        // The same text gets the same ids.
        let again =
            Document.run project edit
            |> fun p -> p.Npcs |> List.find (fun n -> n.Id = npc.Id)

        Assert.Equal<NpcLine list option>(changed.Lines, again.Lines)

[<Fact>]
let ``problems inside an NPC's dialogue point at the script line`` () =
    let project = starter ()
    let npc = project.Npcs |> List.head

    let broken =
        { npc with
            Dialogue =
                npc.Dialogue
                @ [
                    { Dialogue.Default with
                        Id = "broken"
                        NpcId = npc.Id
                        Text = "Hi"
                        Options =
                            [
                                { DialogueOption.Default with
                                    Text = "Go"
                                    NextDialogueId = Some "missing-dialogue"
                                }
                            ]
                    }
                ]
        }

    let project = apply (UpsertNpc broken) project
    let problems = Problems.collect project
    let located = DialogueScript.npcProblems project problems npc.Id

    let text =
        DialogueScript.printNpc (project.Npcs |> List.find (fun n -> n.Id = npc.Id))

    let lines = text.Split '\n'
    Assert.NotEmpty located

    for line, problem in located do
        Assert.True(line >= 1 && line <= lines.Length, sprintf "%d: %s" line problem.Message)

    Assert.Contains(
        located,
        fun (line, problem) ->
            problem.Message.Contains "missing-dialogue"
            && lines.[line - 1].Contains "-> missing-dialogue"
    )

[<Fact>]
let ``cutscenes read back with their lines choices and other steps`` () =
    let cutscene =
        { Cutscene.Default with
            Id = "mill"
            Name = "The mill reopens"
            Skippable = false
            Steps =
                [
                    { CutsceneStep.Default with
                        Type = CutsceneStepTypes.Say
                        Text = Some "She turns again!"
                        Speaker = Some "bram"
                        Expression = Some "happy"
                        Id = Some "s-1"
                    }
                    { CutsceneStep.Default with
                        Type = CutsceneStepTypes.Say
                        Text = Some "A narrator speaks."
                    }
                    { CutsceneStep.Default with
                        Type = CutsceneStepTypes.Choice
                        Text = Some "Celebrate?"
                        Options =
                            [
                                {
                                    Text = "Yes"
                                    Goto = "party"
                                    Id = Some "c-1"
                                }
                                { Text = "No"; Goto = ""; Id = None }
                            ]
                    }
                    { CutsceneStep.Default with
                        Type = CutsceneStepTypes.Jump
                        Goto = Some "end"
                        Conditions = [ EventCondition.Flag { Flag = "tired"; Value = true } ]
                    }
                    { CutsceneStep.Default with
                        Type = CutsceneStepTypes.Wait
                        Seconds = Some 1.5
                        Label = Some "party"
                    }
                    { CutsceneStep.Default with
                        Type = CutsceneStepTypes.End
                        Label = Some "end"
                    }
                ]
        }

    let text = DialogueScript.printCutscenes [ cutscene ]
    Assert.Contains("bram@happy: She turns again! #line:s-1", text)
    Assert.Contains("> jump end if flag tired", text)
    let document = parsed text
    Assert.Equal<Cutscene list>([ cutscene ], document.Cutscenes)

// ── Conditions ────────────────────────────────────────────────────────────

[<Fact>]
let ``condition text reads the short forms`` () =
    let cases =
        [
            "flag a", EventCondition.Flag { Flag = "a"; Value = true }
            "flag a off", EventCondition.Flag { Flag = "a"; Value = false }
            "item wood 10", EventCondition.HasItem { ItemId = "wood"; Quantity = 10.0 }
            "day 3..", EventCondition.DayRange { MinDay = Some 3.0; MaxDay = None }
            "time 22:00..2:00",
            EventCondition.TimeOfDay
                {
                    MinMinute = 1320.0
                    MaxMinute = 120.0
                }
            "evening",
            EventCondition.TimeOfDay
                {
                    MinMinute = 1080.0
                    MaxMinute = 1439.0
                }
            "season spring \"late summer\"",
            EventCondition.Season
                {
                    Seasons = [ "spring"; "late summer" ]
                }
            "hearts bram 2..5",
            EventCondition.Hearts
                {
                    NpcId = "bram"
                    Min = Some 2.0
                    Max = Some 5.0
                }
            "var apples >= 10",
            EventCondition.Variable
                {
                    Name = "apples"
                    Compare = VariableComparisons.Ge
                    Value = Some 10.0
                    Text = None
                }
            "var mood = \"glad\"",
            EventCondition.Variable
                {
                    Name = "mood"
                    Compare = VariableComparisons.Eq
                    Value = None
                    Text = Some "glad"
                }
            "not flag met",
            EventCondition.Not
                {
                    Conditions = [ EventCondition.Flag { Flag = "met"; Value = true } ]
                }
            "any(weather rain, season winter)",
            EventCondition.Any
                {
                    Conditions =
                        [
                            EventCondition.Weather { WeatherIds = [ "rain" ] }
                            EventCondition.Season { Seasons = [ "winter" ] }
                        ]
                }
            "enter 1 2 to 3 4",
            EventCondition.EnterTile
                {
                    X = 1.0
                    Y = 2.0
                    X2 = Some 3.0
                    Y2 = Some 4.0
                }
            """{"type":"chance","chance":0.5}""", EventCondition.Chance { Chance = 0.5 }
        ]

    for text, expected in cases do
        match ConditionText.parse text with
        | Ok condition -> Assert.True((condition = expected), sprintf "%s → %A" text condition)
        | Error e -> failwithf "%s: %s" text e

[<Fact>]
let ``condition text explains a mistake`` () =
    Assert.Equal(Error "write flag NAME (or flag NAME off)", ConditionText.parseList "flag")
    Assert.Equal(Error "rainy is not a condition", ConditionText.parseList "rainy")
    Assert.Equal(Error "any( is not closed", ConditionText.parseList "any(flag a")

// ── Properties ────────────────────────────────────────────────────────────

let private idGen =
    Gen.elements [ "a"; "bram"; "quest-1"; "x.y"; "with space"; "q\"uote"; "lines"; "->" ]

let private textGen =
    Gen.elements
        [
            "Hello."
            ""
            "  padded "
            "Two\nlines"
            "* starts like an option"
            "[bracket] text"
            "arrow -> inside"
            "ends #line:x"
            "{player} and {item:wood}"
            "Ünïcödé — ✓"
            "\"quoted\""
            "// not a comment"
            "back\\slash"
        ]

let private numberGen = Gen.elements [ 0.0; 1.0; 2.5; -3.0; 250.0; 1e21; 0.1 ]

let private optionOf (g: Gen<'T>) =
    Gen.oneof [ Gen.constant None; Gen.map Some g ]

let rec private conditionGen (depth: int) : Gen<EventCondition> =
    let leaf =
        Gen.oneof
            [
                Gen.map2 (fun f v -> EventCondition.Flag { Flag = f; Value = v }) idGen (Gen.elements [ true; false ])
                Gen.map2 (fun i q -> EventCondition.HasItem { ItemId = i; Quantity = q }) idGen numberGen
                Gen.map2 (fun a b -> EventCondition.DayRange { MinDay = a; MaxDay = b }) (optionOf numberGen) (optionOf numberGen)
                Gen.map2 (fun a b -> EventCondition.TimeOfDay { MinMinute = a; MaxMinute = b }) numberGen numberGen
                Gen.map (fun s -> EventCondition.Season { Seasons = s }) (Gen.listOf idGen)
                Gen.map2 (fun q s -> EventCondition.QuestStatus { QuestId = q; Status = s }) idGen idGen
                Gen.map3 (fun n a b -> EventCondition.Hearts { NpcId = n; Min = a; Max = b }) idGen (optionOf numberGen) (optionOf numberGen)
                Gen.map2 (fun n m -> EventCondition.Friendship { NpcId = n; Min = m }) (Gen.elements [ ""; "bram"; "42" ]) numberGen
                Gen.map4
                    (fun n c v t ->
                        EventCondition.Variable
                            {
                                Name = n
                                Compare = c
                                Value = v
                                Text = t
                            })
                    idGen
                    (Gen.elements (VariableComparisons.All @ [ "odd" ]))
                    (optionOf numberGen)
                    (optionOf textGen)
                Gen.map (fun c -> EventCondition.Chance { Chance = c }) numberGen
                Gen.map3 (fun s p c -> EventCondition.Perk { Skill = s; PerkId = p; ChoiceId = c }) idGen idGen idGen
                Gen.map2 (fun m i -> EventCondition.MineFloor { Min = m; MineId = i }) numberGen (optionOf idGen)
                Gen.map4
                    (fun x y x2 y2 -> EventCondition.EnterTile { X = x; Y = y; X2 = x2; Y2 = y2 })
                    numberGen
                    numberGen
                    (optionOf numberGen)
                    (optionOf numberGen)
            ]

    if depth <= 0 then
        leaf
    else
        let inner = Gen.listOfLength 2 (conditionGen (depth - 1))

        Gen.oneof
            [
                leaf
                Gen.map (fun c -> EventCondition.All { Conditions = c }) inner
                Gen.map (fun c -> EventCondition.Any { Conditions = c }) inner
                Gen.map (fun c -> EventCondition.Not { Conditions = c }) (Gen.oneof [ inner; Gen.map List.singleton (conditionGen (depth - 1)) ])
            ]

let private optionGen: Gen<DialogueOption> =
    gen {
        let! text = textGen
        let! next = optionOf idGen
        let! give = optionOf idGen
        let! quantity = optionOf numberGen
        let! take = optionOf numberGen
        let! money = optionOf numberGen
        let! flag = optionOf idGen
        let! once = optionOf (Gen.elements [ true; false ])
        let! hidden = optionOf idGen
        let! item = optionOf idGen
        let! requires = optionOf idGen
        let! shop = optionOf idGen
        let! quest = optionOf idGen
        let! friendship = optionOf numberGen
        let! action = optionOf idGen
        let! id = optionOf (Gen.elements [ "o-1"; "o-2"; "x" ])
        let! extra = Gen.elements [ []; [ "k", JNumber 1.0 ] ]

        return
            {
                Text = text
                NextDialogueId = next
                GiveItem = give
                GiveItemQuantity = quantity
                TakeMoney = take
                GiveMoney = money
                EventFlag = flag
                Once = once
                HiddenIfFlag = hidden
                RequiresItem = item
                RequiresFlag = requires
                OpenShopId = shop
                OfferQuestId = quest
                RequiresFriendship = friendship
                ActionId = action
                Id = id
                Extra = extra
            }
    }

let private npcGen: Gen<Npc> =
    gen {
        let! dialogues =
            Gen.listOf (
                gen {
                    let! id =
                        Gen.elements [ "bram.intro"; "lines"; "bram.lines"; "d 2"; "cutscene"; "x" ]

                    let! npcId = Gen.elements [ "bram"; "bram"; "other" ]
                    let! text = textGen
                    let! options = Gen.listOf optionGen
                    let! expression = optionOf idGen

                    return
                        { Dialogue.Default with
                            Id = id
                            NpcId = npcId
                            Text = text
                            Options = options
                            Expression = expression
                        }
                }
            )

        let! lines =
            optionOf (
                Gen.listOf (
                    gen {
                        let! text = textGen
                        let! conditions = Gen.listOf (conditionGen 2)
                        let! expression = optionOf idGen
                        let! weight = optionOf numberGen
                        let! id = optionOf (Gen.elements [ "l-1"; "l-2" ])

                        return
                            {
                                Text = text
                                Conditions = conditions
                                Expression = expression
                                Weight = weight
                                Id = id
                            }
                    }
                )
            )

        return
            { Npc.Default with
                Id = "bram"
                Dialogue = dialogues
                Lines = lines
            }
    }

[<FsCheck.Xunit.Property(MaxTest = 300)>]
let ``every condition reads back from its text`` () =
    Prop.forAll (Arb.fromGen (conditionGen 3)) (fun condition ->
        let text = ConditionText.print condition
        ConditionText.parse text = Ok condition)

[<FsCheck.Xunit.Property(MaxTest = 200)>]
let ``an NPC the printer covers reads back exactly`` () =
    Prop.forAll (Arb.fromGen npcGen) (fun npc ->
        // Options with conditions an option cannot hold are not covered; everything else is.
        let text = DialogueScript.printNpc npc

        match DialogueScript.applyToNpc npc text with
        | Ok back -> back.Dialogue = npc.Dialogue && back.Lines = npc.Lines
        | Error errors -> failwithf "%A\n%s" errors text)

[<Fact>]
let ``an NPC's dialogue goes to a script in a project folder and back`` () =
    let project = starter ()
    let files = ProjectFolder.toFiles project
    let scripts = files |> List.filter (fun (p, _) -> p.StartsWith "scripts/")
    Assert.NotEmpty scripts

    for path, bytes in scripts do
        Assert.EndsWith(DialogueScript.Extension, path)
        Assert.StartsWith("@npc ", System.Text.Encoding.UTF8.GetString bytes)

    let npcFile =
        files
        |> List.find (fun (p, _) -> p.StartsWith "npcs/")
        |> snd
        |> System.Text.Encoding.UTF8.GetString

    Assert.Contains(ProjectFolder.ScriptKey, npcFile)
    Assert.DoesNotContain("\"lines\"", npcFile)

    let folder =
        Dictionary<string, byte[]>(dict files) :> IReadOnlyDictionary<string, byte[]>

    match ProjectFolder.load folder with
    | { Data = Some loaded } -> Assert.Equal<Npc list>(project.Npcs, loaded.Npcs)
    | other -> failwithf "%A" other.Errors

    // A script edited outside the editor is read; a broken one says where.
    let path, _ = List.head scripts
    let edited = Dictionary<string, byte[]>(dict files)

    edited.[path] <-
        System.Text.Encoding.UTF8.GetBytes(
            System.Text.Encoding.UTF8.GetString edited.[path]
            + "\n=== new.one\nHello there.\n* [season] Bad.\n"
        )

    match ProjectFolder.join edited with
    | Ok _ -> failwith "expected an error"
    | Error errors -> Assert.Contains(errors, fun e -> e.StartsWith(path + ":") && e.Contains "write season")
