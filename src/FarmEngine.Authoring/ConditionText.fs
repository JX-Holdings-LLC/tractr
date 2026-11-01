namespace FarmEngine.Authoring

open System
open System.Text
open FarmEngine.Schemas

/// Conditions as text (#225): one syntax for NPC lines, dialogue options, cutscene jumps, events
/// and quests, written and read back without loss. A list is separated by commas:
///
/// ```text
/// flag mill-fixed, season spring summer, time 6:00..11:59, quest silent-mill active
/// not flag met-bram, any(weather rain, season winter), hearts bram 2.., var apples >= 10
/// ```
///
/// | Text | Condition |
/// |---|---|
/// | `flag NAME` (`flag NAME off`) | `flag` (value false) |
/// | `item ID [N]`, `space ID [N]` | `hasItem`, `inventorySpace` |
/// | `day A..B`, `day-of-season A..B`, `year A..B` | `dayRange`, `dayOfSeason`, `yearRange` (either end may be left out) |
/// | `time 6:00..12:00`, `morning`, `afternoon`, `evening` | `timeOfDay` (minutes; the names are 6:00–11:59, 12:00–17:59, 18:00–23:59) |
/// | `season S…`, `weather W…`, `weekday D…`, `scene S…` | `season`, `weather`, `weekday`, `scene` |
/// | `quest ID STATUS`, `festival ID` | `questStatus`, `festivalId` |
/// | `friendship [NPC] N`, `hearts [NPC] A..B`, `relationship [NPC] STATUS`, `birthday NPC` | the social conditions (no NPC: this NPC) |
/// | `var NAME OP VALUE` (`=`, `!=`, `<`, `<=`, `>`, `>=`; text in quotes) | `variable` |
/// | `money N`, `chance P`, `skill S N`, `perk S P C`, `buff ID`, `mine-floor N [MINE]` | the rest of the vocabulary |
/// | `collection ID [N]`, `bundle ID`, `achievement ID`, `shipped ID N`, `building ID N`, `slain ID N` | goals (M6) |
/// | `enter X Y [to X2 Y2]`, `interact X Y [to X2 Y2]` | tile triggers |
/// | `all(…)`, `any(…)`, `not C`, `not(…)` | groups |
/// | `{"type": …}` | any condition as its JSON (what the printer writes when no shorter text reads back the same) |
///
/// A word with spaces or punctuation is written in double quotes (JSON string rules).
module ConditionText =
    /// The named times of day `[morning]` stands for (inclusive minutes).
    let namedTimes =
        [
            "morning", (360.0, 719.0)
            "afternoon", (720.0, 1079.0)
            "evening", (1080.0, 1439.0)
        ]

    let private comparisons =
        [
            "=", VariableComparisons.Eq
            "!=", VariableComparisons.Ne
            "<", VariableComparisons.Lt
            "<=", VariableComparisons.Le
            ">", VariableComparisons.Gt
            ">=", VariableComparisons.Ge
        ]

    // ── Tokens ─────────────────────────────────────────────────────────────

    type internal Token =
        | Word of string
        /// A double-quoted string (never a keyword).
        | Quoted of string
        /// A JSON object written inline.
        | Object of Json
        | Open
        | Close
        | Comma

    let private isPlain (c: char) =
        not (Char.IsWhiteSpace c)
        && c <> '('
        && c <> ')'
        && c <> ','
        && c <> '['
        && c <> ']'
        && c <> '"'
        && c <> '{'
        && c <> '}'

    /// The end (exclusive) of the JSON string starting at `start` (a `"`), or -1.
    let internal endOfString (text: string) (start: int) =
        let mutable i = start + 1
        let mutable found = -1

        while found < 0 && i < text.Length do
            match text.[i] with
            | '\\' -> i <- i + 2
            | '"' -> found <- i + 1
            | _ -> i <- i + 1

        found

    /// The end (exclusive) of the balanced JSON value starting at `start` (a `{` or `[`), or -1.
    let internal endOfBalanced (text: string) (start: int) =
        let mutable depth = 0
        let mutable i = start
        let mutable found = -1

        while found < 0 && i >= 0 && i < text.Length do
            match text.[i] with
            | '"' ->
                let e = endOfString text i
                i <- if e < 0 then -1 else e
            | '{'
            | '[' ->
                depth <- depth + 1
                i <- i + 1
            | '}'
            | ']' ->
                depth <- depth - 1
                i <- i + 1

                if depth = 0 then
                    found <- i
            | _ -> i <- i + 1

        found

    let internal tokenize (text: string) : Result<Token list, string> =
        let tokens = ResizeArray<Token>()
        let mutable i = 0
        let mutable error = None

        while error.IsNone && i < text.Length do
            let c = text.[i]

            if Char.IsWhiteSpace c then
                i <- i + 1
            elif c = '(' then
                tokens.Add Open
                i <- i + 1
            elif c = ')' then
                tokens.Add Close
                i <- i + 1
            elif c = ',' then
                tokens.Add Comma
                i <- i + 1
            elif c = '"' then
                let e = endOfString text i

                if e < 0 then
                    error <- Some "a quote is not closed"
                else
                    match Json.parse (text.Substring(i, e - i)) with
                    | Ok(JString s) ->
                        tokens.Add(Quoted s)
                        i <- e
                    | _ -> error <- Some(sprintf "%s is not a quoted word" (text.Substring(i, e - i)))
            elif c = '{' then
                let e = endOfBalanced text i

                if e < 0 then
                    error <- Some "a { is not closed"
                else
                    match Json.parse (text.Substring(i, e - i)) with
                    | Ok json ->
                        tokens.Add(Object json)
                        i <- e
                    | Error message -> error <- Some("the JSON condition is not valid: " + message)
            elif isPlain c then
                let start = i

                while i < text.Length && isPlain text.[i] do
                    i <- i + 1

                tokens.Add(Word(text.Substring(start, i - start)))
            else
                error <- Some(sprintf "unexpected '%c'" c)

        match error with
        | Some e -> Error e
        | None -> Ok(List.ofSeq tokens)

    // ── Words ──────────────────────────────────────────────────────────────

    /// `word` as the printer writes it: bare when it reads back as itself, else quoted.
    let word (text: string) =
        if text.Length > 0 && text |> Seq.forall isPlain then
            text
        else
            Json.quote text

    let number (value: float) = JsNumber.format value

    let private parseNumber (text: string) : float option =
        if String.IsNullOrWhiteSpace text then
            None
        else
            let n = JsNumber.parse text

            if Double.IsNaN n || Double.IsInfinity n then
                None
            else
                Some n

    /// `6:00` or `390` as minutes.
    let private parseMinutes (text: string) : float option =
        match text.Split ':' with
        | [| hours; minutes |] when minutes.Length = 2 ->
            match Int32.TryParse hours, Int32.TryParse minutes with
            | (true, h), (true, m) when h >= 0 && m >= 0 && m < 60 -> Some(float (h * 60 + m))
            | _ -> None
        | _ -> parseNumber text

    let private minutesText (value: float) =
        if value >= 0.0 && value = Math.Floor value && value < 100000.0 then
            let m = int value
            sprintf "%d:%02d" (m / 60) (m % 60)
        else
            number value

    /// `A..B`, `A..`, `..B` or `A` (both ends) as optional ends.
    let private parseRange (parse: string -> float option) (text: string) : Result<float option * float option, string> =
        let bad () =
            Error(sprintf "%s is not a range (write A..B, A.. or ..B)" text)

        match text.IndexOf ".." with
        | -1 ->
            match parse text with
            | Some v -> Ok(Some v, Some v)
            | None -> bad ()
        | at ->
            let low, high = text.Substring(0, at), text.Substring(at + 2)

            let side (s: string) =
                if s = "" then
                    Ok None
                else
                    match parse s with
                    | Some v -> Ok(Some v)
                    | None -> Error()

            match side low, side high with
            | Ok l, Ok h when l.IsSome || h.IsSome -> Ok(l, h)
            | _ -> bad ()

    let private rangeText (format: float -> string) (low: float option) (high: float option) =
        match low, high with
        | Some l, Some h when l = h -> format l
        | _ ->
            (low |> Option.map format |> Option.defaultValue "")
            + ".."
            + (high |> Option.map format |> Option.defaultValue "")

    // ── Printing ───────────────────────────────────────────────────────────

    let private words (list: string list) =
        list |> List.map word |> String.concat " "

    let rec private shortText (condition: EventCondition) : string option =
        let tile name (x: float) (y: float) (x2: float option) (y2: float option) =
            match x2, y2 with
            | None, None -> sprintf "%s %s %s" name (number x) (number y)
            | Some x2, Some y2 -> sprintf "%s %s %s to %s %s" name (number x) (number y) (number x2) (number y2)
            | _ -> ""

        let text =
            match condition with
            | EventCondition.EnterTile c -> tile "enter" c.X c.Y c.X2 c.Y2
            | EventCondition.InteractTile c -> tile "interact" c.X c.Y c.X2 c.Y2
            | EventCondition.HasItem c ->
                if c.Quantity = 1.0 then
                    "item " + word c.ItemId
                else
                    sprintf "item %s %s" (word c.ItemId) (number c.Quantity)
            | EventCondition.InventorySpace c ->
                if c.Quantity = 1.0 then
                    "space " + word c.ItemId
                else
                    sprintf "space %s %s" (word c.ItemId) (number c.Quantity)
            | EventCondition.Flag c -> "flag " + word c.Flag + (if c.Value then "" else " off")
            | EventCondition.DayRange c -> "day " + rangeText number c.MinDay c.MaxDay
            | EventCondition.DayOfSeason c -> "day-of-season " + rangeText number c.MinDay c.MaxDay
            | EventCondition.YearRange c -> "year " + rangeText number c.MinYear c.MaxYear
            | EventCondition.TimeOfDay c ->
                match
                    namedTimes
                    |> List.tryFind (fun (_, range) -> range = (c.MinMinute, c.MaxMinute))
                with
                | Some(name, _) -> name
                | None -> sprintf "time %s..%s" (minutesText c.MinMinute) (minutesText c.MaxMinute)
            | EventCondition.Season c -> "season " + words c.Seasons
            | EventCondition.Weather c -> "weather " + words c.WeatherIds
            | EventCondition.Weekday c -> "weekday " + words c.WeekdayIds
            | EventCondition.Scene c -> "scene " + words c.SceneIds
            | EventCondition.QuestStatus c -> sprintf "quest %s %s" (word c.QuestId) (word c.Status)
            | EventCondition.FestivalId c -> "festival " + word c.FestivalId
            | EventCondition.Friendship c ->
                if c.NpcId = "" then
                    "friendship " + number c.Min
                else
                    sprintf "friendship %s %s" (word c.NpcId) (number c.Min)
            | EventCondition.Hearts c ->
                let range = rangeText number c.Min c.Max

                if c.NpcId = "" then
                    "hearts " + range
                else
                    sprintf "hearts %s %s" (word c.NpcId) range
            | EventCondition.Relationship c ->
                if c.NpcId = "" then
                    "relationship " + word c.Status
                else
                    sprintf "relationship %s %s" (word c.NpcId) (word c.Status)
            | EventCondition.Birthday c -> "birthday " + word c.NpcId
            | EventCondition.Variable c ->
                match comparisons |> List.tryFind (fun (_, compare) -> compare = c.Compare), c.Value, c.Text with
                | Some(op, _), Some value, None -> sprintf "var %s %s %s" (word c.Name) op (number value)
                | Some(op, _), None, Some text -> sprintf "var %s %s %s" (word c.Name) op (Json.quote text)
                | _ -> ""
            | EventCondition.Money c -> "money " + number c.Min
            | EventCondition.Chance c -> "chance " + number c.Chance
            | EventCondition.SkillLevel c -> sprintf "skill %s %s" (word c.Skill) (number c.Min)
            | EventCondition.Perk c -> sprintf "perk %s %s %s" (word c.Skill) (word c.PerkId) (word c.ChoiceId)
            | EventCondition.Buff c -> "buff " + word c.Id
            | EventCondition.MineFloor c ->
                match c.MineId with
                | None -> "mine-floor " + number c.Min
                | Some mine -> sprintf "mine-floor %s %s" (number c.Min) (word mine)
            | EventCondition.Collection c ->
                match c.Min with
                | None -> "collection " + word c.CollectionId
                | Some min -> sprintf "collection %s %s" (word c.CollectionId) (number min)
            | EventCondition.Bundle c -> "bundle " + word c.BundleId
            | EventCondition.Achievement c -> "achievement " + word c.AchievementId
            | EventCondition.Shipped c -> sprintf "shipped %s %s" (word c.ItemId) (number c.Min)
            | EventCondition.Building c -> sprintf "building %s %s" (word c.BuildingId) (number c.Min)
            | EventCondition.Slain c -> sprintf "slain %s %s" (word c.MonsterId) (number c.Min)
            // A plugin condition (#239) has free-form arguments: it is written as its JSON.
            | EventCondition.Plugin _ -> ""
            | EventCondition.All c -> group "all" c.Conditions
            | EventCondition.Any c -> group "any" c.Conditions
            | EventCondition.Not c ->
                match c.Conditions with
                | [ single ] ->
                    match print single with
                    | inner when not (inner.StartsWith "{") -> "not " + inner
                    | _ -> group "not" c.Conditions
                | conditions -> group "not" conditions

        if text = "" then None else Some text

    and private group (name: string) (conditions: EventCondition list) = name + "(" + printList conditions + ")"

    /// One condition as text: its short form when that reads back as the same condition, else its
    /// JSON.
    and print (condition: EventCondition) : string =
        let json () =
            Json.stringify (SchemaJson.encodeEventCondition condition)

        match shortText condition with
        | Some text ->
            match parseList text with
            | Ok [ back ] when back = condition -> text
            | _ -> json ()
        | None -> json ()

    /// Conditions as text, separated by commas.
    and printList (conditions: EventCondition list) : string =
        conditions |> List.map print |> String.concat ", "

    // ── Parsing ────────────────────────────────────────────────────────────

    and private parseOne (tokens: Token list) : Result<EventCondition * Token list, string> =
        let argsOf (rest: Token list) =
            let args = rest |> List.takeWhile (fun t -> t <> Comma && t <> Close)
            args, List.skip args.Length rest

        let text (token: Token) =
            match token with
            | Word w -> Some w
            | Quoted q -> Some q
            | _ -> None

        let texts (args: Token list) =
            let values = args |> List.map text

            if values |> List.forall Option.isSome then
                Some(List.choose id values)
            else
                None

        let num (token: Token) =
            match token with
            | Word w -> parseNumber w
            | _ -> None

        let ok condition rest = Ok(condition, rest)

        let groupOf name (rest: Token list) (make: EventCondition list -> EventCondition) =
            match rest with
            | Open :: inner ->
                match parseSequence inner with
                | Ok(conditions, Close :: after) -> ok (make conditions) after
                | Ok _ -> Error(sprintf "%s( is not closed" name)
                | Error e -> Error e
            | _ -> Error(sprintf "%s needs its conditions in parentheses: %s(…)" name name)

        match tokens with
        | Object json :: rest ->
            try
                ok (SchemaJson.decodeEventCondition [] json) rest
            with DecodeError(path, message) ->
                Error(sprintf "the JSON condition is not valid: %s%s" (if path = "" then "" else path + ": ") message)
        | Word keyword :: rest ->
            let args, after = argsOf rest
            let fail usage = Error(sprintf "write %s" usage)

            match keyword.ToLowerInvariant() with
            | "all" -> groupOf "all" rest (fun c -> EventCondition.All { Conditions = c })
            | "any" -> groupOf "any" rest (fun c -> EventCondition.Any { Conditions = c })
            | "not" ->
                match rest with
                | Open :: _ -> groupOf "not" rest (fun c -> EventCondition.Not { Conditions = c })
                | _ ->
                    parseOne rest
                    |> Result.map (fun (inner, after) -> EventCondition.Not { Conditions = [ inner ] }, after)
            | name when namedTimes |> List.exists (fun (n, _) -> n = name) && args.IsEmpty ->
                let low, high = namedTimes |> List.find (fun (n, _) -> n = name) |> snd
                ok (EventCondition.TimeOfDay { MinMinute = low; MaxMinute = high }) after
            | "flag" ->
                match args with
                | [ name ] when (text name).IsSome ->
                    ok
                        (EventCondition.Flag
                            {
                                Flag = (text name).Value
                                Value = true
                            })
                        after
                | [ name; Word "off" ] when (text name).IsSome ->
                    ok
                        (EventCondition.Flag
                            {
                                Flag = (text name).Value
                                Value = false
                            })
                        after
                | [ name; Word "on" ] when (text name).IsSome ->
                    ok
                        (EventCondition.Flag
                            {
                                Flag = (text name).Value
                                Value = true
                            })
                        after
                | _ -> fail "flag NAME (or flag NAME off)"
            | "item"
            | "space" as kind ->
                let make id quantity =
                    if kind = "item" then
                        EventCondition.HasItem { ItemId = id; Quantity = quantity }
                    else
                        EventCondition.InventorySpace { ItemId = id; Quantity = quantity }

                match args with
                | [ id ] when (text id).IsSome -> ok (make (text id).Value 1.0) after
                | [ id; n ] when (text id).IsSome && (num n).IsSome -> ok (make (text id).Value (num n).Value) after
                | _ -> fail (kind + " ID [QUANTITY]")
            | "day"
            | "day-of-season"
            | "year" as kind ->
                match args with
                | [ Word range ] ->
                    match parseRange parseNumber range with
                    | Ok(low, high) ->
                        let condition =
                            match kind with
                            | "day" -> EventCondition.DayRange { MinDay = low; MaxDay = high }
                            | "year" -> EventCondition.YearRange { MinYear = low; MaxYear = high }
                            | _ -> EventCondition.DayOfSeason { MinDay = low; MaxDay = high }

                        ok condition after
                    | Error e -> Error e
                | _ -> fail (kind + " A..B")
            | "time" ->
                match args with
                | [ Word range ] ->
                    match parseRange parseMinutes range with
                    | Ok(Some low, Some high) -> ok (EventCondition.TimeOfDay { MinMinute = low; MaxMinute = high }) after
                    | Ok _ -> Error "a time needs both ends: time 6:00..12:00"
                    | Error e -> Error e
                | _ -> fail "time 6:00..12:00"
            | "season"
            | "weather"
            | "weekday"
            | "scene" as kind ->
                match texts args with
                | Some values when not values.IsEmpty ->
                    let condition =
                        match kind with
                        | "season" -> EventCondition.Season { Seasons = values }
                        | "weather" -> EventCondition.Weather { WeatherIds = values }
                        | "weekday" -> EventCondition.Weekday { WeekdayIds = values }
                        | _ -> EventCondition.Scene { SceneIds = values }

                    ok condition after
                | _ -> fail (kind + " ID [ID…]")
            | "quest" ->
                match texts args with
                | Some [ quest; status ] -> ok (EventCondition.QuestStatus { QuestId = quest; Status = status }) after
                | _ -> fail "quest ID STATUS"
            | "festival" ->
                match texts args with
                | Some [ id ] -> ok (EventCondition.FestivalId { FestivalId = id }) after
                | _ -> fail "festival ID"
            | "friendship" ->
                match args with
                | [ n ] when (num n).IsSome -> ok (EventCondition.Friendship { NpcId = ""; Min = (num n).Value }) after
                | [ npc; n ] when (text npc).IsSome && (num n).IsSome ->
                    ok
                        (EventCondition.Friendship
                            {
                                NpcId = (text npc).Value
                                Min = (num n).Value
                            })
                        after
                | _ -> fail "friendship [NPC] POINTS"
            | "hearts" ->
                let make npc range =
                    parseRange parseNumber range
                    |> Result.map (fun (low, high) -> EventCondition.Hearts { NpcId = npc; Min = low; Max = high }, after)

                match args with
                | [ Word range ] -> make "" range
                | [ npc; Word range ] when (text npc).IsSome -> make (text npc).Value range
                | _ -> fail "hearts [NPC] A..B"
            | "relationship" ->
                match texts args with
                | Some [ status ] -> ok (EventCondition.Relationship { NpcId = ""; Status = status }) after
                | Some [ npc; status ] -> ok (EventCondition.Relationship { NpcId = npc; Status = status }) after
                | _ -> fail "relationship [NPC] STATUS"
            | "birthday" ->
                match texts args with
                | Some [ npc ] -> ok (EventCondition.Birthday { NpcId = npc }) after
                | _ -> fail "birthday NPC"
            | "var" ->
                match args with
                | [ name; Word op; value ] when (text name).IsSome ->
                    match comparisons |> List.tryFind (fun (o, _) -> o = op) with
                    | None -> Error(sprintf "%s is not a comparison (=, !=, <, <=, >, >=)" op)
                    | Some(_, compare) ->
                        let condition (v: float option) (t: string option) =
                            EventCondition.Variable
                                {
                                    Name = (text name).Value
                                    Compare = compare
                                    Value = v
                                    Text = t
                                }

                        match value with
                        | Quoted t -> ok (condition None (Some t)) after
                        | Word w ->
                            match parseNumber w with
                            | Some n -> ok (condition (Some n) None) after
                            | None -> ok (condition None (Some w)) after
                        | _ -> fail "var NAME OP VALUE"
                | _ -> fail "var NAME OP VALUE"
            | "money" ->
                match args with
                | [ n ] when (num n).IsSome -> ok (EventCondition.Money { Min = (num n).Value }) after
                | _ -> fail "money AMOUNT"
            | "chance" ->
                match args with
                | [ n ] when (num n).IsSome -> ok (EventCondition.Chance { Chance = (num n).Value }) after
                | _ -> fail "chance 0.25"
            | "skill" ->
                match args with
                | [ skill; n ] when (text skill).IsSome && (num n).IsSome ->
                    ok
                        (EventCondition.SkillLevel
                            {
                                Skill = (text skill).Value
                                Min = (num n).Value
                            })
                        after
                | _ -> fail "skill SKILL LEVEL"
            | "perk" ->
                match texts args with
                | Some [ skill; perk; choice ] ->
                    ok
                        (EventCondition.Perk
                            {
                                Skill = skill
                                PerkId = perk
                                ChoiceId = choice
                            })
                        after
                | _ -> fail "perk SKILL PERK CHOICE"
            | "buff" ->
                match texts args with
                | Some [ id ] -> ok (EventCondition.Buff { Id = id }) after
                | _ -> fail "buff ID"
            | "mine-floor" ->
                match args with
                | [ n ] when (num n).IsSome -> ok (EventCondition.MineFloor { Min = (num n).Value; MineId = None }) after
                | [ n; mine ] when (num n).IsSome && (text mine).IsSome ->
                    ok
                        (EventCondition.MineFloor
                            {
                                Min = (num n).Value
                                MineId = text mine
                            })
                        after
                | _ -> fail "mine-floor FLOOR [MINE]"
            | "collection" ->
                match args with
                | [ id ] when (text id).IsSome ->
                    ok
                        (EventCondition.Collection
                            {
                                CollectionId = (text id).Value
                                Min = None
                            })
                        after
                | [ id; n ] when (text id).IsSome && (num n).IsSome ->
                    ok
                        (EventCondition.Collection
                            {
                                CollectionId = (text id).Value
                                Min = num n
                            })
                        after
                | _ -> fail "collection ID [COUNT]"
            | "bundle" ->
                match texts args with
                | Some [ id ] -> ok (EventCondition.Bundle { BundleId = id }) after
                | _ -> fail "bundle ID"
            | "achievement" ->
                match texts args with
                | Some [ id ] -> ok (EventCondition.Achievement { AchievementId = id }) after
                | _ -> fail "achievement ID"
            | "shipped"
            | "building"
            | "slain" as kind ->
                match args with
                | [ id; n ] when (text id).IsSome && (num n).IsSome ->
                    let id, n = (text id).Value, (num n).Value

                    let condition =
                        match kind with
                        | "shipped" -> EventCondition.Shipped { ItemId = id; Min = n }
                        | "building" -> EventCondition.Building { BuildingId = id; Min = n }
                        | _ -> EventCondition.Slain { MonsterId = id; Min = n }

                    ok condition after
                | _ -> fail (kind + " ID COUNT")
            | "enter"
            | "interact" as kind ->
                let make x y x2 y2 =
                    if kind = "enter" then
                        EventCondition.EnterTile { X = x; Y = y; X2 = x2; Y2 = y2 }
                    else
                        EventCondition.InteractTile { X = x; Y = y; X2 = x2; Y2 = y2 }

                match args |> List.map num, args with
                | [ Some x; Some y ], _ -> ok (make x y None None) after
                | [ Some x; Some y; None; Some x2; Some y2 ], [ _; _; Word "to"; _; _ ] -> ok (make x y (Some x2) (Some y2)) after
                | _ -> fail (kind + " X Y [to X2 Y2]")
            | other -> Error(sprintf "%s is not a condition" other)
        | Quoted q :: _ -> Error(sprintf "\"%s\" is not a condition" q)
        | Comma :: _ -> Error "a condition is missing before ','"
        | Open :: _ -> Error "unexpected '('"
        | Close :: _ -> Error "unexpected ')'"
        | [] -> Error "a condition is missing"

    and private parseSequence (tokens: Token list) : Result<EventCondition list * Token list, string> =
        let rec loop (acc: EventCondition list) (tokens: Token list) =
            match parseOne tokens with
            | Error e -> Error e
            | Ok(condition, Comma :: rest) -> loop (condition :: acc) rest
            | Ok(condition, rest) -> Ok(List.rev (condition :: acc), rest)

        loop [] tokens

    /// Conditions written as text (see the module notes); empty text is no condition.
    and parseList (text: string) : Result<EventCondition list, string> =
        if String.IsNullOrWhiteSpace text then
            Ok []
        else
            match tokenize text with
            | Error e -> Error e
            | Ok tokens ->
                match parseSequence tokens with
                | Ok(conditions, []) -> Ok conditions
                | Ok(_, Close :: _) -> Error "unexpected ')'"
                | Ok(_, _) -> Error "expected ',' between conditions"
                | Error e -> Error e

    /// One condition written as text.
    let parse (text: string) : Result<EventCondition, string> =
        match parseList text with
        | Ok [ condition ] -> Ok condition
        | Ok [] -> Error "a condition is missing"
        | Ok _ -> Error "expected one condition"
        | Error e -> Error e
