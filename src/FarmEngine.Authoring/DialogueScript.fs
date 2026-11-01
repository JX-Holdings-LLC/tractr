namespace FarmEngine.Authoring

open System
open System.Text
open System.Text.RegularExpressions
open FarmEngine.Schemas

/// A mistake in a dialogue script: its line (1-based) and what is wrong.
type ScriptError = { Line: int; Message: string }

/// What a dialogue script holds: an NPC's dialogues and lines, and cutscenes.
type ScriptDocument =
    {
        /// The `@npc` the file is for.
        Npc: string option
        Dialogues: Dialogue list
        /// The `=== lines` block; None when the file has none.
        Lines: NpcLine list option
        Cutscenes: Cutscene list
        /// Where each part starts: `dialogue[0]`, `dialogue[0].options[1]`, `lines[2]`,
        /// `cutscenes[0].steps[3]` → its line.
        Map: (string * int) list
    }

/// Dialogue scripts (#225): an NPC's dialogue and lines, or cutscenes, as text a writer edits,
/// read into the same records the forms edit (the engine is unchanged). The printer and the
/// parser are inverses for everything the format covers; `isLossless` says whether an NPC is
/// covered, and the project folder keeps any other NPC as JSON. See docs/DIALOGUE-SCRIPTS.md.
///
/// ```text
/// @npc bram
///
/// === bram.intro
/// @expression sad
/// Wheel turns, stones don't. The axle on my grindstone cracked clean through.
/// * What do you need? -> offer quest-silent-mill #line:o-1
/// * [if flag mill-fixed] Hear that? -> bram.fixed
/// * Good luck.
///   ~ give item bread 2
///
/// === bram.lines
/// [morning] Mornin'. Flour on my boots, flour in my beard. #line:l-1
/// [weather rain] Rain keeps the river high. Good for the wheel, bad for my knees.
///
/// === cutscene mill-reopens
/// @name The mill reopens
/// bram@happy: She turns again!
/// ? Help him celebrate?
/// * Of course. -> party
/// * Not now.
/// > {"type":"wait","seconds":1}
/// ```
module DialogueScript =
    /// The extension of a dialogue script in a project folder.
    [<Literal>]
    let Extension = ".dialogue"

    // ── Text ───────────────────────────────────────────────────────────────

    let private tagPattern = Regex(@"(^|\s)#line:([^\s""]+)$", RegexOptions.CultureInvariant)

    let private startsStructural (text: string) =
        text.Length > 0 && "\"*~@=[?>:/\\{".IndexOf text.[0] >= 0

    /// Whether `text` can be written as it is (else it is written in quotes, JSON string rules).
    let private plain (text: string) =
        text.Length > 0
        && text.Trim() = text
        && not (startsStructural text)
        && not (text.Contains "->")
        && not (text.Contains "#line:")
        && text |> Seq.forall (fun c -> not (Char.IsControl c))

    let private textOut (text: string) =
        if plain text then text else Json.quote text

    let private idText (id: string) = ConditionText.word id

    /// An id after `->`: quoted when it would read as a keyword.
    let private targetText (id: string) =
        if id = "->" || id = "offer" || id = "shop" || id = "action" then
            Json.quote id
        else
            idText id

    /// Whether `id` can be written as a line tag.
    let private taggable (id: string) =
        id.Length > 0
        && id |> Seq.forall (fun c -> not (Char.IsWhiteSpace c) && c <> '"' && not (Char.IsControl c))

    let private tag (id: string option) =
        match id with
        | Some id when taggable id -> " #line:" + id
        | Some id -> " #line:" + Json.quote id // reads back as an error: not lossless
        | None -> ""

    // ── Printing ───────────────────────────────────────────────────────────

    let private optionConditions (option: DialogueOption) : EventCondition list =
        [
            match option.RequiresFlag with
            | Some flag -> EventCondition.Flag { Flag = flag; Value = true }
            | None -> ()
            match option.HiddenIfFlag with
            | Some flag -> EventCondition.Not { Conditions = [ EventCondition.Flag { Flag = flag; Value = true } ] }
            | None -> ()
            match option.RequiresItem with
            | Some item -> EventCondition.HasItem { ItemId = item; Quantity = 1.0 }
            | None -> ()
            match option.RequiresFriendship with
            | Some points -> EventCondition.Friendship { NpcId = ""; Min = points }
            | None -> ()
        ]

    let private extraText (extra: (string * Json) list) = Json.stringify (JObject extra)

    type private Printer() =
        let sb = StringBuilder()
        let map = ResizeArray<string * int>()
        let mutable line = 1

        member _.Line(text: string) =
            sb.Append(text).Append('\n') |> ignore
            line <- line + 1

        member _.Mark(path: string) = map.Add(path, line)
        member _.Text = sb.ToString()
        member _.Map = List.ofSeq map

    let private printOption (p: Printer) (option: DialogueOption) =
        let groups =
            [
                match optionConditions option with
                | [] -> ()
                | conditions -> "[if " + ConditionText.printList conditions + "] "
                match option.Once with
                | Some true -> "[once] "
                | Some false -> "[once off] "
                | None -> ()
            ]
            |> String.concat ""

        let targets =
            [
                match option.NextDialogueId with
                | Some id -> " -> " + targetText id
                | None -> ()
                match option.OfferQuestId with
                | Some id -> " -> offer " + targetText id
                | None -> ()
                match option.OpenShopId with
                | Some id -> " -> shop " + targetText id
                | None -> ()
                match option.ActionId with
                | Some id -> " -> action " + targetText id
                | None -> ()
            ]
            |> String.concat ""

        p.Line("* " + groups + textOut option.Text + targets + tag option.Id)

        match option.GiveItem with
        | Some item ->
            match option.GiveItemQuantity with
            | Some n -> p.Line(sprintf "  ~ give item %s %s" (idText item) (ConditionText.number n))
            | None -> p.Line(sprintf "  ~ give item %s" (idText item))
        | None ->
            match option.GiveItemQuantity with
            | Some n -> p.Line(sprintf "  ~ give quantity %s" (ConditionText.number n))
            | None -> ()

        match option.GiveMoney with
        | Some n -> p.Line("  ~ give money " + ConditionText.number n)
        | None -> ()

        match option.TakeMoney with
        | Some n -> p.Line("  ~ take money " + ConditionText.number n)
        | None -> ()

        match option.EventFlag with
        | Some flag -> p.Line("  ~ set flag " + idText flag)
        | None -> ()

        if not option.Extra.IsEmpty then
            p.Line("  ~ extra " + extraText option.Extra)

    let private dialogueHeader (npcId: string) (id: string) =
        let reserved =
            id = "lines"
            || id = npcId + ".lines"
            || id = "cutscene"
            || id = "dialogue"

        if reserved then
            "=== dialogue " + idText id
        else
            "=== " + idText id

    let private printDialogue (p: Printer) (npcId: string) (index: int) (dialogue: Dialogue) =
        p.Mark(sprintf "dialogue[%d]" index)
        p.Line(dialogueHeader npcId dialogue.Id)

        if dialogue.NpcId <> npcId then
            p.Line("@npc " + idText dialogue.NpcId)

        match dialogue.Expression with
        | Some expression -> p.Line("@expression " + idText expression)
        | None -> ()

        if not dialogue.Extra.IsEmpty then
            p.Line("@extra " + extraText dialogue.Extra)

        if dialogue.Text <> "" then
            for segment in dialogue.Text.Split '\n' do
                p.Line(if segment = "" then "\"\"" else textOut segment)

        dialogue.Options
        |> List.iteri (fun k option ->
            p.Mark(sprintf "dialogue[%d].options[%d]" index k)
            printOption p option)

    let private printLines (p: Printer) (npcId: string) (lines: NpcLine list) =
        p.Line(if npcId = "" then "=== lines" else "=== " + idText (npcId + ".lines"))

        lines
        |> List.iteri (fun k line ->
            p.Mark(sprintf "lines[%d]" k)

            let groups =
                [
                    match line.Conditions with
                    | [] -> ()
                    | conditions -> "[" + ConditionText.printList conditions + "] "
                    match line.Weight with
                    | Some w -> "[weight " + ConditionText.number w + "] "
                    | None -> ()
                    match line.Expression with
                    | Some e -> "[@" + idText e + "] "
                    | None -> ()
                ]
                |> String.concat ""

            p.Line(groups + textOut line.Text + tag line.Id))

    let private speakerPattern = Regex(@"^[A-Za-z0-9_.\-]+$", RegexOptions.CultureInvariant)

    /// A step the short forms cover (`SPEAKER: TEXT`, `? QUESTION` with its answers, `> jump`);
    /// others are written as JSON.
    let private printStep (p: Printer) (step: CutsceneStep) =
        match step.Label with
        | Some label -> p.Line("@label " + idText label)
        | None -> ()

        let speakerOk (s: string option) =
            s |> Option.forall (fun s -> speakerPattern.IsMatch s)

        let say =
            { CutsceneStep.Default with
                Type = CutsceneStepTypes.Say
                Label = step.Label
                Text = step.Text
                Speaker = step.Speaker
                Expression = step.Expression
                Id = step.Id
            }

        let choice =
            { CutsceneStep.Default with
                Type = CutsceneStepTypes.Choice
                Label = step.Label
                Text = step.Text
                Options = step.Options
                Id = step.Id
            }

        let jump =
            { CutsceneStep.Default with
                Type = CutsceneStepTypes.Jump
                Label = step.Label
                Goto = step.Goto
                Conditions = step.Conditions
            }

        if step = say && step.Text.IsSome && speakerOk step.Speaker && speakerOk step.Expression then
            let speaker = defaultArg step.Speaker ""

            let expression =
                match step.Expression with
                | Some e -> "@" + e
                | None -> ""

            p.Line(speaker + expression + ": " + textOut step.Text.Value + tag step.Id)
        elif step = choice then
            match step.Text with
            | Some text -> p.Line("? " + textOut text + tag step.Id)
            | None -> p.Line("?" + tag step.Id)

            for choice in step.Options do
                let goto = if choice.Goto = "" then "" else " -> " + targetText choice.Goto
                p.Line("* " + textOut choice.Text + goto + tag choice.Id)
        elif step = jump && step.Goto.IsSome then
            let conditions =
                match step.Conditions with
                | [] -> ""
                | conditions -> " if " + ConditionText.printList conditions

            p.Line("> jump " + idText step.Goto.Value + conditions)
        else
            let json = SchemaJson.encodeCutsceneStep { step with Label = None }
            p.Line("> " + Json.stringify json)

    let private printCutscene (p: Printer) (index: int) (cutscene: Cutscene) =
        p.Mark(sprintf "cutscenes[%d]" index)
        p.Line("=== cutscene " + idText cutscene.Id)

        if cutscene.Name <> "" then
            p.Line("@name " + textOut cutscene.Name)

        if not cutscene.Skippable then
            p.Line "@skippable no"

        if not cutscene.Extra.IsEmpty then
            p.Line("@extra " + extraText cutscene.Extra)

        cutscene.Steps
        |> List.iteri (fun k step ->
            p.Mark(sprintf "cutscenes[%d].steps[%d]" index k)
            printStep p step)

    /// The script of `document` and where each part starts.
    let printDocument (document: ScriptDocument) : string * (string * int) list =
        let p = Printer()
        let npcId = defaultArg document.Npc ""

        match document.Npc with
        | Some npc -> p.Line("@npc " + idText npc)
        | None -> ()

        document.Dialogues
        |> List.iteri (fun i dialogue ->
            p.Line ""
            printDialogue p npcId i dialogue)

        match document.Lines with
        | Some lines ->
            p.Line ""
            printLines p npcId lines
        | None -> ()

        document.Cutscenes
        |> List.iteri (fun i cutscene ->
            p.Line ""
            printCutscene p i cutscene)

        p.Text, p.Map

    /// An NPC's dialogues and lines as a script.
    let documentOfNpc (npc: Npc) : ScriptDocument =
        {
            Npc = Some npc.Id
            Dialogues = npc.Dialogue
            Lines = npc.Lines
            Cutscenes = []
            Map = []
        }

    /// An NPC's dialogues and lines as a script.
    let printNpc (npc: Npc) : string = documentOfNpc npc |> printDocument |> fst

    /// Cutscenes as a script.
    let printCutscenes (cutscenes: Cutscene list) : string =
        {
            Npc = None
            Dialogues = []
            Lines = None
            Cutscenes = cutscenes
            Map = []
        }
        |> printDocument
        |> fst

    // ── Parsing ────────────────────────────────────────────────────────────

    type private Block =
        | NoBlock
        | DialogueBlock of Dialogue * optionsStarted: bool
        | LinesBlock of NpcLine list
        | CutsceneBlock of Cutscene * pendingLabel: string option

    /// Reads quoted or plain text from the start of `rest`; plain text stops at `->` when
    /// `arrows`. Returns the text and what follows it.
    let private readText (arrows: bool) (rest: string) : Result<string * string, string> =
        let rest = rest.TrimStart()

        if rest.StartsWith "\"" then
            let e = ConditionText.endOfString rest 0

            if e < 0 then
                Error "a quote is not closed"
            else
                match Json.parse (rest.Substring(0, e)) with
                | Ok(JString s) -> Ok(s, rest.Substring e)
                | _ -> Error "the quoted text is not valid (JSON string rules)"
        else
            let stop = if arrows then rest.IndexOf "->" else -1

            if stop < 0 then
                Ok(rest.Trim(), "")
            else
                Ok(rest.Substring(0, stop).Trim(), rest.Substring stop)

    /// The line's `#line:ID` tag and the line without it.
    let private splitTag (line: string) : string * string option =
        let trimmed = line.TrimEnd()
        let m = tagPattern.Match trimmed

        if m.Success then
            trimmed.Substring(0, m.Index).TrimEnd(), Some m.Groups.[2].Value
        else
            trimmed, None

    /// The `[...]` groups at the start of `rest` (their contents) and what follows.
    let private readGroups (rest: string) : Result<string list * string, string> =
        let rec loop (acc: string list) (rest: string) =
            let rest = rest.TrimStart()

            if rest.StartsWith "[" then
                let e = ConditionText.endOfBalanced rest 0

                if e < 0 then
                    Error "a [ is not closed"
                else
                    loop (rest.Substring(1, e - 2).Trim() :: acc) (rest.Substring e)
            else
                Ok(List.rev acc, rest)

        loop [] rest

    /// The words of `text` (bare or quoted).
    let private wordsOf (text: string) : Result<string list, string> =
        match ConditionText.tokenize text with
        | Error e -> Error e
        | Ok tokens ->
            let values =
                tokens
                |> List.map (fun token ->
                    match token with
                    | ConditionText.Word w -> Some w
                    | ConditionText.Quoted q -> Some q
                    | _ -> None)

            if values |> List.forall Option.isSome then
                Ok(List.choose id values)
            else
                Error(sprintf "%s: expected words" text)

    let private parseNumber (text: string) =
        let n = JsNumber.parse text

        if String.IsNullOrWhiteSpace text || Double.IsNaN n || Double.IsInfinity n then
            None
        else
            Some n

    let private parseExtra (text: string) : Result<(string * Json) list, string> =
        match Json.parse text with
        | Ok(JObject members) -> Ok members
        | Ok _ -> Error "@extra needs a JSON object"
        | Error e -> Error("@extra is not valid JSON: " + e)

    let private parseOption (npcId: string) (body: string) : Result<DialogueOption, string> =
        let body, id = splitTag body

        match readGroups body with
        | Error e -> Error e
        | Ok(groups, rest) ->
            let mutable option = { DialogueOption.Default with Id = id }
            let mutable error = None
            let fail (message: string) = if error.IsNone then error <- Some message

            for group in groups do
                match group with
                | "once" -> option <- { option with Once = Some true }
                | "once off" -> option <- { option with Once = Some false }
                | _ ->
                    let conditions = if group.StartsWith "if " then group.Substring 3 else group

                    match ConditionText.parseList conditions with
                    | Error e -> fail e
                    | Ok conditions ->
                        for condition in conditions do
                            let hidden =
                                match condition with
                                | EventCondition.Flag c when not c.Value -> Some c.Flag
                                | EventCondition.Not n ->
                                    match n.Conditions with
                                    | [ EventCondition.Flag c ] when c.Value -> Some c.Flag
                                    | _ -> None
                                | _ -> None

                            match condition, hidden with
                            | EventCondition.Flag c, _ when c.Value && option.RequiresFlag.IsNone ->
                                option <- { option with RequiresFlag = Some c.Flag }
                            | _, Some flag when option.HiddenIfFlag.IsNone -> option <- { option with HiddenIfFlag = Some flag }
                            | EventCondition.HasItem c, _ when c.Quantity = 1.0 && option.RequiresItem.IsNone ->
                                option <- { option with RequiresItem = Some c.ItemId }
                            | EventCondition.Friendship c, _ when (c.NpcId = "" || c.NpcId = npcId) && option.RequiresFriendship.IsNone ->
                                option <- { option with RequiresFriendship = Some c.Min }
                            | other, _ ->
                                fail (
                                    sprintf
                                        "an option can check one flag, one 'not flag', one item and this NPC's friendship; %s is for lines, events and quests"
                                        (ConditionText.print other)
                                )

            match readText true rest with
            | Error e -> Error e
            | Ok(text, targets) ->
                option <- { option with Text = text }

                match ConditionText.tokenize targets with
                | Error e -> fail e
                | Ok tokens ->
                    let value (token: ConditionText.Token) =
                        match token with
                        | ConditionText.Word w -> Some w
                        | ConditionText.Quoted q -> Some q
                        | _ -> None

                    let rec targetsLoop (tokens: ConditionText.Token list) =
                        match tokens with
                        | [] -> ()
                        | ConditionText.Word "->" :: ConditionText.Word kind :: id :: rest when
                            (kind = "offer" || kind = "shop" || kind = "action") && (value id).IsSome
                            ->
                            let id = (value id).Value

                            match kind with
                            | "offer" when option.OfferQuestId.IsNone -> option <- { option with OfferQuestId = Some id }
                            | "shop" when option.OpenShopId.IsNone -> option <- { option with OpenShopId = Some id }
                            | "action" when option.ActionId.IsNone -> option <- { option with ActionId = Some id }
                            | _ -> fail (sprintf "the option already has -> %s" kind)

                            targetsLoop rest
                        | ConditionText.Word "->" :: id :: rest when (value id).IsSome && id <> ConditionText.Word "->" ->
                            if option.NextDialogueId.IsSome then
                                fail "the option already goes to a dialogue"
                            else
                                option <- { option with NextDialogueId = value id }

                            targetsLoop rest
                        | token :: _ ->
                            fail (
                                sprintf
                                    "expected -> DIALOGUE, -> offer QUEST, -> shop SHOP or -> action ACTION, found %s"
                                    (defaultArg (value token) "punctuation")
                            )

                    targetsLoop tokens

                match error with
                | Some e -> Error e
                | None -> Ok option

    let private applyEffect (option: DialogueOption) (body: string) : Result<DialogueOption, string> =
        let body = body.Trim()

        if body.StartsWith "extra " then
            if not option.Extra.IsEmpty then
                Error "the option already has ~ extra"
            else
                parseExtra (body.Substring 6) |> Result.map (fun extra -> { option with Extra = extra })
        else
            match wordsOf body with
            | Error e -> Error e
            | Ok words ->
                let number (text: string) =
                    match parseNumber text with
                    | Some n -> Ok n
                    | None -> Error(sprintf "%s is not a number" text)

                match words with
                | [ "give"; "item"; item ] when option.GiveItem.IsNone -> Ok { option with GiveItem = Some item }
                | [ "give"; "item"; item; n ] when option.GiveItem.IsNone ->
                    number n
                    |> Result.map (fun n ->
                        { option with
                            GiveItem = Some item
                            GiveItemQuantity = Some n
                        })
                | [ "give"; "quantity"; n ] when option.GiveItemQuantity.IsNone ->
                    number n |> Result.map (fun n -> { option with GiveItemQuantity = Some n })
                | [ "give"; "money"; n ] when option.GiveMoney.IsNone -> number n |> Result.map (fun n -> { option with GiveMoney = Some n })
                | [ "take"; "money"; n ] when option.TakeMoney.IsNone -> number n |> Result.map (fun n -> { option with TakeMoney = Some n })
                | [ "set"; "flag"; flag ] when option.EventFlag.IsNone -> Ok { option with EventFlag = Some flag }
                | "give" :: "item" :: _
                | "give" :: "money" :: _
                | "take" :: "money" :: _
                | "set" :: "flag" :: _ when words.Length <= 4 -> Error "the option already has this effect"
                | _ -> Error "expected ~ give item ITEM [N], ~ give money N, ~ take money N, ~ set flag FLAG or ~ extra {…}"

    let private parseLine (body: string) : Result<NpcLine, string> =
        let body, id = splitTag body

        match readGroups body with
        | Error e -> Error e
        | Ok(groups, rest) ->
            let mutable line = { NpcLine.Default with Id = id }
            let mutable error = None
            let fail (message: string) = if error.IsNone then error <- Some message

            for group in groups do
                if group.StartsWith "@" then
                    match wordsOf (group.Substring 1) with
                    | Ok [ expression ] when line.Expression.IsNone -> line <- { line with Expression = Some expression }
                    | _ -> fail "write [@EXPRESSION] once"
                elif group.StartsWith "weight " then
                    match parseNumber (group.Substring 7), line.Weight with
                    | Some w, None -> line <- { line with Weight = Some w }
                    | _ -> fail "write [weight N] once"
                else
                    let conditions = if group.StartsWith "if " then group.Substring 3 else group

                    match ConditionText.parseList conditions with
                    | Ok conditions -> line <- { line with Conditions = line.Conditions @ conditions }
                    | Error e -> fail e

            match readText false rest with
            | Error e -> fail e
            | Ok(text, _) -> line <- { line with Text = text }

            match error with
            | Some e -> Error e
            | None -> Ok line

    let private sayPattern =
        Regex(@"^([A-Za-z0-9_.\-]*)(@([A-Za-z0-9_.\-]+))?:(\s|$)", RegexOptions.CultureInvariant)

    /// Reads a script (see the module notes). Every mistake is reported with its line.
    let parse (text: string) : Result<ScriptDocument, ScriptError list> =
        let errors = ResizeArray<ScriptError>()
        let map = ResizeArray<string * int>()
        let dialogues = ResizeArray<Dialogue>()
        let cutscenes = ResizeArray<Cutscene>()
        let mutable lines: NpcLine list option = None
        let mutable npc: string option = None
        let mutable block = NoBlock
        let mutable textLines = ResizeArray<string>()

        let close () =
            match block with
            | DialogueBlock(dialogue, _) ->
                let text = String.Join("\n", textLines)
                dialogues.Add { dialogue with Text = text; Options = List.rev dialogue.Options }
            | LinesBlock found -> lines <- Some(List.rev found)
            | CutsceneBlock(cutscene, _) -> cutscenes.Add { cutscene with Steps = List.rev cutscene.Steps }
            | NoBlock -> ()

            textLines <- ResizeArray()
            block <- NoBlock

        let source = text.Replace("\r\n", "\n").Split '\n'

        source
        |> Array.iteri (fun index raw ->
            let number = index + 1
            let error message = errors.Add { Line = number; Message = message }
            let line = raw.Trim()

            if line = "" || line.StartsWith "//" then
                ()
            elif line.StartsWith "===" then
                close ()

                match wordsOf (line.Substring 3) with
                | Ok [ "cutscene"; id ] ->
                    map.Add(sprintf "cutscenes[%d]" cutscenes.Count, number)
                    block <- CutsceneBlock({ Cutscene.Default with Id = id }, None)
                | Ok [ "dialogue"; id ] ->
                    map.Add(sprintf "dialogue[%d]" dialogues.Count, number)

                    block <-
                        DialogueBlock(
                            { Dialogue.Default with
                                Id = id
                                NpcId = defaultArg npc ""
                            },
                            false
                        )
                | Ok [ id ] when id = "lines" || (npc.IsSome && id = npc.Value + ".lines") ->
                    if lines.IsSome then
                        error "the file already has a lines block"

                    block <- LinesBlock []
                | Ok [ id ] ->
                    map.Add(sprintf "dialogue[%d]" dialogues.Count, number)

                    block <-
                        DialogueBlock(
                            { Dialogue.Default with
                                Id = id
                                NpcId = defaultArg npc ""
                            },
                            false
                        )
                | Ok _ -> error "write === DIALOGUE-ID, === NPC.lines or === cutscene CUTSCENE-ID"
                | Error e -> error e
            else
                match block with
                | NoBlock ->
                    if line.StartsWith "@npc" && (line.Length = 4 || Char.IsWhiteSpace line.[4]) then
                        match wordsOf (line.Substring 4) with
                        | Ok [ id ] when npc.IsNone && dialogues.Count = 0 -> npc <- Some id
                        | Ok [ _ ] -> error "@npc comes once, before the first block"
                        | _ -> error "write @npc NPC-ID"
                    else
                        error "this line is outside a block: start one with === DIALOGUE-ID"
                | DialogueBlock(dialogue, started) ->
                    if line.StartsWith "@" then
                        let space = line.IndexOf ' '
                        let name = if space < 0 then line else line.Substring(0, space)
                        let value = if space < 0 then "" else line.Substring(space + 1).Trim()

                        if started then
                            error (name + " goes before the options")
                        else
                            match name with
                            | "@expression" ->
                                match wordsOf value with
                                | Ok [ e ] when dialogue.Expression.IsNone -> block <- DialogueBlock({ dialogue with Expression = Some e }, false)
                                | _ -> error "write @expression NAME once"
                            | "@npc" ->
                                match wordsOf value with
                                | Ok [ id ] -> block <- DialogueBlock({ dialogue with NpcId = id }, false)
                                | _ -> error "write @npc NPC-ID"
                            | "@extra" ->
                                match parseExtra value with
                                | Ok extra when dialogue.Extra.IsEmpty -> block <- DialogueBlock({ dialogue with Extra = extra }, false)
                                | Ok _ -> error "the dialogue already has @extra"
                                | Error e -> error e
                            | other -> error (sprintf "%s is not a dialogue setting (@expression, @npc, @extra)" other)
                    elif line.StartsWith "*" then
                        map.Add(sprintf "dialogue[%d].options[%d]" dialogues.Count dialogue.Options.Length, number)

                        match parseOption dialogue.NpcId (line.Substring 1) with
                        | Ok option -> block <- DialogueBlock({ dialogue with Options = option :: dialogue.Options }, true)
                        | Error e ->
                            error e
                            block <- DialogueBlock({ dialogue with Options = DialogueOption.Default :: dialogue.Options }, true)
                    elif line.StartsWith "~" then
                        match dialogue.Options with
                        | last :: earlier ->
                            match applyEffect last (line.Substring 1) with
                            | Ok option -> block <- DialogueBlock({ dialogue with Options = option :: earlier }, true)
                            | Error e -> error e
                        | [] -> error "an effect (~) belongs under an option (*)"
                    elif started then
                        error "text after the options: put the dialogue's text before its first option"
                    else
                        match readText false line with
                        | Ok(text, "") -> textLines.Add text
                        | Ok _ -> error "unexpected text after the quoted text"
                        | Error e -> error e
                | LinesBlock found ->
                    if line.StartsWith "@" || line.StartsWith "*" || line.StartsWith "~" then
                        error "a lines block holds only lines: [CONDITIONS] TEXT"
                    else
                        map.Add(sprintf "lines[%d]" found.Length, number)

                        match parseLine line with
                        | Ok parsed -> block <- LinesBlock(parsed :: found)
                        | Error e ->
                            error e
                            block <- LinesBlock(NpcLine.Default :: found)
                | CutsceneBlock(cutscene, label) ->
                    let stepIndex = cutscene.Steps.Length
                    let mark () = map.Add(sprintf "cutscenes[%d].steps[%d]" cutscenes.Count stepIndex, number)

                    let add (step: CutsceneStep) =
                        block <- CutsceneBlock({ cutscene with Steps = { step with Label = label } :: cutscene.Steps }, None)

                    if line.StartsWith "@" then
                        let space = line.IndexOf ' '
                        let name = if space < 0 then line else line.Substring(0, space)
                        let value = if space < 0 then "" else line.Substring(space + 1).Trim()

                        match name with
                        | "@label" ->
                            match wordsOf value with
                            | Ok [ l ] when label.IsNone -> block <- CutsceneBlock(cutscene, Some l)
                            | _ -> error "write @label NAME before a step"
                        | "@name" when cutscene.Steps.IsEmpty ->
                            match readText false value with
                            | Ok(name, "") -> block <- CutsceneBlock({ cutscene with Name = name }, label)
                            | _ -> error "write @name TEXT"
                        | "@skippable" when cutscene.Steps.IsEmpty ->
                            match value with
                            | "yes" -> block <- CutsceneBlock({ cutscene with Skippable = true }, label)
                            | "no" -> block <- CutsceneBlock({ cutscene with Skippable = false }, label)
                            | _ -> error "write @skippable yes or @skippable no"
                        | "@extra" when cutscene.Steps.IsEmpty ->
                            match parseExtra value with
                            | Ok extra -> block <- CutsceneBlock({ cutscene with Extra = extra }, label)
                            | Error e -> error e
                        | other -> error (sprintf "%s is not a cutscene setting here (@name, @skippable, @extra before the steps; @label before a step)" other)
                    elif line.StartsWith ">" then
                        mark ()
                        let body = line.Substring(1).Trim()

                        if body.StartsWith "{" then
                            match Json.parse body with
                            | Ok json ->
                                try
                                    add (SchemaJson.decodeCutsceneStep [] json)
                                with DecodeError(path, message) ->
                                    error (sprintf "the step is not valid: %s%s" (if path = "" then "" else path + ": ") message)
                            | Error e -> error ("the step is not valid JSON: " + e)
                        elif body.StartsWith "jump " then
                            let rest = body.Substring(5).Trim()
                            let at = rest.IndexOf " if "

                            let target, conditions =
                                if at < 0 then rest, "" else rest.Substring(0, at), rest.Substring(at + 4)

                            match wordsOf target, ConditionText.parseList conditions with
                            | Ok [ goto ], Ok conditions ->
                                add
                                    { CutsceneStep.Default with
                                        Type = CutsceneStepTypes.Jump
                                        Goto = Some goto
                                        Conditions = conditions
                                    }
                            | Ok _, Ok _ -> error "write > jump LABEL [if CONDITIONS]"
                            | Error e, _
                            | _, Error e -> error e
                        else
                            error "write > {STEP JSON} or > jump LABEL [if CONDITIONS]"
                    elif line.StartsWith "?" then
                        mark ()
                        let body, id = splitTag (line.Substring 1)

                        let text =
                            if body.Trim() = "" then
                                Ok None
                            else
                                match readText false body with
                                | Ok(t, "") -> Ok(Some t)
                                | Ok _ -> Error "unexpected text after the quoted text"
                                | Error e -> Error e

                        match text with
                        | Ok text ->
                            add
                                { CutsceneStep.Default with
                                    Type = CutsceneStepTypes.Choice
                                    Text = text
                                    Id = id
                                }
                        | Error e -> error e
                    elif line.StartsWith "*" then
                        match cutscene.Steps with
                        | last :: earlier when last.Type = CutsceneStepTypes.Choice && label.IsNone ->
                            let body, id = splitTag (line.Substring 1)

                            match readText true body with
                            | Ok(text, rest) ->
                                match wordsOf rest with
                                | Ok [] ->
                                    let choice: CutsceneChoice = { Text = text; Goto = ""; Id = id }
                                    block <- CutsceneBlock({ cutscene with Steps = { last with Options = last.Options @ [ choice ] } :: earlier }, None)
                                | Ok [ "->"; goto ] ->
                                    let choice: CutsceneChoice = { Text = text; Goto = goto; Id = id }
                                    block <- CutsceneBlock({ cutscene with Steps = { last with Options = last.Options @ [ choice ] } :: earlier }, None)
                                | Ok _ -> error "write * ANSWER -> LABEL"
                                | Error e -> error e
                            | Error e -> error e
                        | _ -> error "an answer (*) belongs right under a question (?)"
                    else
                        let m = sayPattern.Match line

                        if not m.Success then
                            error "write SPEAKER: TEXT (or : TEXT for a narrator), ? QUESTION, * ANSWER, > STEP or @label NAME"
                        else
                            mark ()
                            let body, id = splitTag (line.Substring m.Length)

                            match readText false body with
                            | Ok(text, "") ->
                                let speaker = m.Groups.[1].Value

                                add
                                    { CutsceneStep.Default with
                                        Type = CutsceneStepTypes.Say
                                        Text = Some text
                                        Speaker = if speaker = "" then None else Some speaker
                                        Expression = if m.Groups.[3].Success then Some m.Groups.[3].Value else None
                                        Id = id
                                    }
                            | Ok _ -> error "unexpected text after the quoted text"
                            | Error e -> error e)

        match block with
        | CutsceneBlock(_, Some _) -> errors.Add { Line = source.Length; Message = "@label needs a step after it" }
        | _ -> ()

        close ()

        if errors.Count > 0 then
            Error(List.ofSeq errors)
        else
            Ok
                {
                    Npc = npc
                    Dialogues = List.ofSeq dialogues
                    Lines = lines
                    Cutscenes = List.ofSeq cutscenes
                    Map = List.ofSeq map
                }

    // ── NPCs ───────────────────────────────────────────────────────────────

    /// `npc` with the dialogues and lines of its script `text`. The script must be for this NPC
    /// (its `@npc`) and hold no cutscenes.
    let applyToNpc (npc: Npc) (text: string) : Result<Npc, ScriptError list> =
        match parse text with
        | Error errors -> Error errors
        | Ok document ->
            match document.Npc with
            | Some id when id <> npc.Id -> Error [ { Line = 1; Message = sprintf "this script is for %s, not %s" id npc.Id } ]
            | _ when not document.Cutscenes.IsEmpty ->
                Error [ { Line = 1; Message = "an NPC's script holds its dialogues and lines; put cutscenes in their own script" } ]
            | _ ->
                Ok
                    { npc with
                        Dialogue = document.Dialogues
                        Lines = document.Lines
                    }

    /// Whether `npc`'s dialogues and lines read back from their script exactly as they are.
    let isLossless (npc: Npc) : bool =
        match applyToNpc npc (printNpc npc) with
        | Ok back -> back.Dialogue = npc.Dialogue && back.Lines = npc.Lines
        | Error _ -> false

    // ── Stable ids ─────────────────────────────────────────────────────────

    let private fnv (text: string) =
        let mutable hash = 2166136261u

        for b in Encoding.UTF8.GetBytes text do
            hash <- (hash ^^^ uint32 b) * 16777619u

        hash

    /// A new id with `prefix` from `seed`, not in `taken` (which it joins).
    let newId (taken: Collections.Generic.HashSet<string>) (prefix: string) (seed: string) =
        let mutable n = 0
        let mutable id = sprintf "%s-%06x" prefix (fnv seed &&& 0xFFFFFFu)

        while not (taken.Add id) do
            n <- n + 1
            id <- sprintf "%s-%06x" prefix (fnv (seed + "#" + string n) &&& 0xFFFFFFu)

        id

    /// `npc` with an id on every line and option that lacks one (made from its text, so the same
    /// project always gets the same ids). The same instance when all have ids.
    let withIds (npc: Npc) : Npc =
        let taken = Collections.Generic.HashSet<string>()

        for line in defaultArg npc.Lines [] do
            line.Id |> Option.iter (taken.Add >> ignore)

        for dialogue in npc.Dialogue do
            for option in dialogue.Options do
                option.Id |> Option.iter (taken.Add >> ignore)

        let lines =
            npc.Lines
            |> Option.map (
                List.mapi (fun k line ->
                    match line.Id with
                    | Some _ -> line
                    | None -> { line with Id = Some(newId taken "l" (sprintf "%s|line|%d|%s" npc.Id k line.Text)) })
            )

        let dialogue =
            npc.Dialogue
            |> List.map (fun d ->
                let options =
                    d.Options
                    |> List.mapi (fun k option ->
                        match option.Id with
                        | Some _ -> option
                        | None -> { option with Id = Some(newId taken "o" (sprintf "%s|%s|%d|%s" npc.Id d.Id k option.Text)) })

                if options = d.Options then d else { d with Options = options })

        if lines = npc.Lines && dialogue = npc.Dialogue then
            npc
        else
            { npc with Lines = lines; Dialogue = dialogue }

    /// `cutscene` with an id on every line, question and answer that lacks one.
    let cutsceneWithIds (cutscene: Cutscene) : Cutscene =
        let taken = Collections.Generic.HashSet<string>()

        for step in cutscene.Steps do
            step.Id |> Option.iter (taken.Add >> ignore)

            for choice in step.Options do
                choice.Id |> Option.iter (taken.Add >> ignore)

        let steps =
            cutscene.Steps
            |> List.mapi (fun k step ->
                let hasText = step.Text.IsSome

                let step =
                    if hasText && step.Id.IsNone then
                        { step with Id = Some(newId taken "s" (sprintf "%s|%d|%s" cutscene.Id k (defaultArg step.Text ""))) }
                    else
                        step

                let options =
                    step.Options
                    |> List.mapi (fun c choice ->
                        match choice.Id with
                        | Some _ -> choice
                        | None -> { choice with Id = Some(newId taken "c" (sprintf "%s|%d|%d|%s" cutscene.Id k c choice.Text)) })

                if options = step.Options then step else { step with Options = options })

        if steps = cutscene.Steps then
            cutscene
        else
            { cutscene with Steps = steps }

    // ── Editing ────────────────────────────────────────────────────────────

    /// The edit that puts script `text` into the project: the NPC's dialogues and lines (for an
    /// `@npc` script) and its cutscenes, with ids given to new lines. One undo step.
    let edit (project: GameProject) (text: string) : Result<Edit, ScriptError list> =
        match parse text with
        | Error errors -> Error errors
        | Ok document ->
            let npcEdit =
                match document.Npc with
                | None when document.Dialogues.IsEmpty && document.Lines.IsNone -> Ok []
                | None -> Error [ { Line = 1; Message = "dialogues and lines need @npc NPC-ID at the top" } ]
                | Some id ->
                    match project.Npcs |> List.tryFind (fun n -> n.Id = id) with
                    | None -> Error [ { Line = 1; Message = sprintf "there is no NPC %s" id } ]
                    | Some npc ->
                        let next =
                            withIds
                                { npc with
                                    Dialogue = document.Dialogues
                                    Lines = document.Lines
                                }

                        Ok [ UpsertNpc next ]

            match npcEdit with
            | Error errors -> Error errors
            | Ok edits ->
                let cutsceneEdits =
                    document.Cutscenes |> List.map (fun c -> UpsertCutscene(cutsceneWithIds c))

                Ok(Batch("Edit dialogue script", edits @ cutsceneEdits))

    // ── Problems ───────────────────────────────────────────────────────────

    /// The line of the script part `path` (relative: `dialogue[0].options[1].text`) is in: the
    /// longest part of the map that starts it.
    let lineOf (map: (string * int) list) (path: string) : int option =
        map
        |> List.filter (fun (part, _) ->
            path = part
            || path.StartsWith(part + ".", StringComparison.Ordinal)
            || path.StartsWith(part + "[", StringComparison.Ordinal))
        |> List.sortByDescending (fun (part, _) -> part.Length)
        |> List.tryHead
        |> Option.map snd

    /// The project's problems inside NPC `npcId`'s dialogues and lines, each with its line in the
    /// NPC's script (as `printNpc` writes it).
    let npcProblems (project: GameProject) (problems: Problem list) (npcId: string) : (int * Problem) list =
        match project.Npcs |> List.tryFindIndex (fun n -> n.Id = npcId) with
        | None -> []
        | Some index ->
            let npc = project.Npcs.[index]
            let _, map = printDocument (documentOfNpc npc)
            let prefix = sprintf "npcs[%d]." index

            let dialogueIndex =
                npc.Dialogue |> List.mapi (fun i d -> d.Id, i) |> dict

            problems
            |> List.choose (fun problem ->
                if problem.Path.StartsWith(prefix, StringComparison.Ordinal) || problem.Path + "." = prefix then
                    let rest = if problem.Path.Length > prefix.Length then problem.Path.Substring prefix.Length else ""

                    match lineOf map rest with
                    | Some line -> Some(line, problem)
                    | None when rest = "" || rest = "dialogue" || rest = "lines" ->
                        // A check that names the dialogue (and the dialogue it links to) in its message.
                        let quoted (id: string) = problem.Message.Contains("\"" + id + "\"")

                        let found =
                            npc.Dialogue
                            |> List.indexed
                            |> List.filter (fun (_, d) -> quoted d.Id)
                            |> List.tryPick (fun (i, d) ->
                                let option =
                                    d.Options
                                    |> List.tryFindIndex (fun o -> o.NextDialogueId |> Option.exists quoted)

                                match option with
                                | Some k -> lineOf map (sprintf "dialogue[%d].options[%d]" i k)
                                | None -> lineOf map (sprintf "dialogue[%d]" i))

                        if rest = "lines" || found.IsSome || rest = "dialogue" then
                            Some(defaultArg found (defaultArg (lineOf map "lines[0]") 1), problem)
                        else
                            None
                    | None -> None
                elif problem.Path.StartsWith("dialogues[", StringComparison.Ordinal) then
                    // A problem in the project's list of dialogues: the NPC's copy of it.
                    let close = problem.Path.IndexOf ']'

                    match Int32.TryParse(problem.Path.Substring(10, close - 10)) with
                    | true, i when i < project.Dialogues.Length ->
                        match dialogueIndex.TryGetValue project.Dialogues.[i].Id with
                        | true, k ->
                            lineOf map (sprintf "dialogue[%d]%s" k (problem.Path.Substring(close + 1)))
                            |> Option.map (fun line -> line, problem)
                        | _ -> None
                    | _ -> None
                else
                    None)
            |> List.sortBy fst

    /// Problems a script's text has on its own, with lines: what does not parse, and what it
    /// names that the project lacks (dialogues it goes to, quests, shops, actions, items,
    /// expressions the NPC has no portrait for, NPCs, labels).
    let check (project: GameProject) (text: string) : ScriptError list =
        match parse text with
        | Error errors -> errors
        | Ok document ->
            let errors = ResizeArray<ScriptError>()
            let lineAt path = lineOf document.Map path |> Option.defaultValue 1
            let add path message = errors.Add { Line = lineAt path; Message = message }

            let npc =
                document.Npc |> Option.bind (fun id -> project.Npcs |> List.tryFind (fun n -> n.Id = id))

            match document.Npc, npc with
            | Some id, None -> errors.Add { Line = 1; Message = sprintf "there is no NPC %s" id }
            | _ -> ()

            let dialogueIds =
                Set.ofList (
                    (document.Dialogues |> List.map (fun d -> d.Id))
                    @ (project.Npcs |> List.collect (fun n -> n.Dialogue) |> List.map (fun d -> d.Id))
                    @ (project.Dialogues |> List.map (fun d -> d.Id))
                )

            let has (ids: string seq) (id: string) = Seq.contains id ids
            let quests = project.Quests |> List.map (fun q -> q.Id)
            let shops = project.Shops |> List.map (fun s -> s.Id)
            let actions = project.Actions |> List.map (fun a -> a.Id)
            let items = project.Items |> List.map (fun i -> i.Id)

            let portraits =
                npc
                |> Option.bind (fun n -> n.Portraits)
                |> Option.defaultValue []
                |> List.map (fun p -> p.Expression)

            let checkExpression path (expression: string option) =
                match expression, npc with
                | Some e, Some n when not (List.contains e portraits) ->
                    add path (sprintf "%s has no portrait for the expression %s" n.Name e)
                | _ -> ()

            let seen = Collections.Generic.HashSet<string>()

            document.Dialogues
            |> List.iteri (fun i d ->
                let path = sprintf "dialogue[%d]" i

                if not (seen.Add d.Id) then
                    add path (sprintf "the dialogue %s comes twice" d.Id)

                checkExpression path d.Expression

                d.Options
                |> List.iteri (fun k o ->
                    let path = sprintf "%s.options[%d]" path k

                    match o.NextDialogueId with
                    | Some id when not (dialogueIds.Contains id) -> add path (sprintf "there is no dialogue %s" id)
                    | _ -> ()

                    match o.OfferQuestId with
                    | Some id when not (has quests id) -> add path (sprintf "there is no quest %s" id)
                    | _ -> ()

                    match o.OpenShopId with
                    | Some id when not (has shops id) -> add path (sprintf "there is no shop %s" id)
                    | _ -> ()

                    match o.ActionId with
                    | Some id when not (has actions id) -> add path (sprintf "there is no action %s" id)
                    | _ -> ()

                    for item in [ o.GiveItem; o.RequiresItem ] |> List.choose id do
                        if not (has items item) then
                            add path (sprintf "there is no item %s" item)))

            document.Lines
            |> Option.defaultValue []
            |> List.iteri (fun k line -> checkExpression (sprintf "lines[%d]" k) line.Expression)

            let npcIds = project.Npcs |> List.map (fun n -> n.Id)

            document.Cutscenes
            |> List.iteri (fun c cutscene ->
                let labels = cutscene.Steps |> List.choose (fun s -> s.Label) |> Set.ofList

                cutscene.Steps
                |> List.iteri (fun k step ->
                    let path = sprintf "cutscenes[%d].steps[%d]" c k

                    match step.Speaker with
                    | Some s when s <> "player" && not (has npcIds s) -> add path (sprintf "there is no NPC %s" s)
                    | _ -> ()

                    for goto in (step.Goto |> Option.toList) @ (step.Options |> List.map (fun o -> o.Goto)) do
                        if goto <> "" && not (labels.Contains goto) then
                            add path (sprintf "there is no @label %s in this cutscene" goto)))

            errors |> Seq.sortBy (fun e -> e.Line) |> List.ofSeq
