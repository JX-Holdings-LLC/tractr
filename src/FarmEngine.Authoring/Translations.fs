namespace FarmEngine.Authoring

open System
open System.Collections.Generic
open System.Text
open System.Text.RegularExpressions
open FarmEngine.Schemas

/// A text of the game a translator translates: its stable key, the source text and where it is
/// shown (who says it, where).
type TextField =
    {
        Key: string
        Source: string
        Context: string
    }

/// What a translation file would change in one language.
type TranslationImport =
    {
        Locale: string
        /// `(key, text, source hash)` of every translated text the file holds for a known key.
        Entries: (string * string * string) list
        Added: int
        Changed: int
        Unchanged: int
        /// Keys the game has no text for.
        Unknown: string list
        /// Lines (CSV rows, PO entries) that could not be read.
        Errors: string list
    }

/// The game's own text in other languages (#228). Every text field has a stable key:
///
/// | Key | Text |
/// |---|---|
/// | `item:ID:name`, `item:ID:description` | an item |
/// | `npc:ID:name` | an NPC's name |
/// | `npc:ID:line:LINE` | an NPC's line of the day (`LINE` is its id, or `#3` for the fourth without one) |
/// | `dialogue:ID:text`, `dialogue:ID:option:OPTION` | a dialogue and its answers |
/// | `quest:ID:name`, `quest:ID:description` | a quest |
/// | `cutscene:ID:step:STEP`, `cutscene:ID:step:STEP:choice:CHOICE` | a cutscene's lines, questions and answers |
/// | `mail:ID:subject`, `mail:ID:text` | a letter |
///
/// The project's `settings.locale` is the source language. Each other language has a table
/// (`project.translations`, `strings/<locale>.json` in a project folder) of texts with a hash of
/// the source they were translated from, so a changed source shows as stale. The compiler puts
/// every table in the cartridge and the player shows the language chosen in Settings.
module Translations =
    /// The first 8 bytes of the SHA-256 of `text`, in hex.
    let sourceHash (text: string) : string =
        let hash = Security.Cryptography.SHA256.HashData(Encoding.UTF8.GetBytes text)
        Convert.ToHexString(hash, 0, 8).ToLowerInvariant()

    /// The key part of a line, option or step: its id, else `#INDEX`.
    let private part (id: string option) (index: int) =
        match id with
        | Some id when id <> "" -> id
        | _ -> sprintf "#%d" index

    /// Every text of the game, in project order.
    let catalog (project: GameProject) : TextField list =
        let fields = ResizeArray<TextField>()
        let seen = HashSet<string>()

        let add key source context =
            if not (String.IsNullOrEmpty source) && seen.Add key then
                fields.Add
                    {
                        Key = key
                        Source = source
                        Context = context
                    }

        let npcName (id: string) =
            match project.Npcs |> List.tryFind (fun n -> n.Id = id) with
            | Some npc when npc.Name <> "" -> sprintf "%s (%s)" npc.Name id
            | _ -> id

        for item in project.Items do
            add (sprintf "item:%s:name" item.Id) item.Name "The name of an item"
            add (sprintf "item:%s:description" item.Id) item.Description (sprintf "The description of %s" item.Name)

        let dialogue (d: Dialogue) =
            let who = npcName d.NpcId
            add (sprintf "dialogue:%s:text" d.Id) d.Text (sprintf "%s says (dialogue %s)" who d.Id)

            d.Options
            |> List.iteri (fun k o -> add (sprintf "dialogue:%s:option:%s" d.Id (part o.Id k)) o.Text (sprintf "The player answers %s (dialogue %s)" who d.Id))

        for npc in project.Npcs do
            add (sprintf "npc:%s:name" npc.Id) npc.Name "The name of an NPC"

            defaultArg npc.Lines []
            |> List.iteri (fun k line ->
                let conditions =
                    if line.Conditions.IsEmpty then
                        ""
                    else
                        " when " + ConditionText.printList line.Conditions

                add (sprintf "npc:%s:line:%s" npc.Id (part line.Id k)) line.Text (sprintf "%s says on talking%s" (npcName npc.Id) conditions))

            npc.Dialogue |> List.iter dialogue

        project.Dialogues |> List.iter dialogue

        for quest in project.Quests do
            add (sprintf "quest:%s:name" quest.Id) quest.Name "The name of a quest"
            add (sprintf "quest:%s:description" quest.Id) quest.Description (sprintf "The description of the quest %s" quest.Name)

        for cutscene in defaultArg project.Cutscenes [] do
            cutscene.Steps
            |> List.iteri (fun k step ->
                let key = sprintf "cutscene:%s:step:%s" cutscene.Id (part step.Id k)

                let who =
                    match step.Speaker with
                    | Some "player" -> "The player says"
                    | Some speaker -> sprintf "%s says" (npcName speaker)
                    | None when step.Type = CutsceneStepTypes.Choice -> "A question"
                    | None -> "The narrator says"

                match step.Text with
                | Some text -> add key text (sprintf "%s (cutscene %s)" who cutscene.Id)
                | None -> ()

                step.Options
                |> List.iteri (fun c choice ->
                    add (sprintf "%s:choice:%s" key (part choice.Id c)) choice.Text (sprintf "An answer the player picks (cutscene %s)" cutscene.Id)))

        for mail in defaultArg project.Mail [] do
            add (sprintf "mail:%s:subject" mail.Id) mail.Subject "The subject of a letter"
            add (sprintf "mail:%s:text" mail.Id) mail.Text (sprintf "A letter from %s" (if mail.From = "" then "the game" else npcName mail.From))

        List.ofSeq fields

    /// The languages the game is translated into, in order.
    let locales (project: GameProject) : string list =
        defaultArg project.Translations [] |> List.map (fun t -> t.Locale)

    let private table (project: GameProject) (locale: string) =
        defaultArg project.Translations [] |> List.tryFind (fun t -> t.Locale = locale)

    let private withTable (project: GameProject) (locale: string) (strings: (string * TranslatedText) list) : GameProject =
        let tables = defaultArg project.Translations []
        let next = { Locale = locale; Strings = strings }

        let tables =
            if tables |> List.exists (fun t -> t.Locale = locale) then
                tables |> List.map (fun t -> if t.Locale = locale then next else t)
            else
                tables @ [ next ]

        { project with
            Translations = Some tables
        }

    /// The edit that adds language `locale` (an empty table). One undo step.
    let addLocale (project: GameProject) (locale: string) : Edit =
        match table project locale with
        | Some _ -> Batch("Add language", [])
        | None -> ReplaceProject(withTable project locale [])

    /// The edit that removes language `locale` and its translations.
    let removeLocale (project: GameProject) (locale: string) : Edit =
        let rest =
            defaultArg project.Translations [] |> List.filter (fun t -> t.Locale <> locale)

        ReplaceProject
            { project with
                Translations = (if rest.IsEmpty then None else Some rest)
            }

    /// The edit that sets the translation of `key` (empty text removes it), as translated from
    /// the current source.
    let setText (project: GameProject) (locale: string) (key: string) (text: string) : Edit =
        let source =
            catalog project
            |> List.tryFind (fun f -> f.Key = key)
            |> Option.map (fun f -> f.Source)
            |> Option.defaultValue ""

        let strings =
            table project locale
            |> Option.map (fun t -> t.Strings)
            |> Option.defaultValue []

        let entry =
            {
                Text = text
                Source = sourceHash source
            }

        let strings =
            if text = "" then
                strings |> List.filter (fun (k, _) -> k <> key)
            elif strings |> List.exists (fun (k, _) -> k = key) then
                strings |> List.map (fun (k, v) -> if k = key then k, entry else k, v)
            else
                strings @ [ key, entry ]

        ReplaceProject(withTable project locale strings)

    /// Translation status of one text in `locale`: "missing", "stale" or "done".
    let status (project: GameProject) (locale: string) (field: TextField) : string =
        match
            table project locale
            |> Option.bind (fun t -> t.Strings |> List.tryFind (fun (k, _) -> k = field.Key))
        with
        | None -> "missing"
        | Some(_, entry) when entry.Text = "" -> "missing"
        | Some(_, entry) when entry.Source <> "" && entry.Source <> sourceHash field.Source -> "stale"
        | Some _ -> "done"

    // ── Placeholders ───────────────────────────────────────────────────────

    let private placeholder =
        Regex(@"\{(player|farm|spouse|season|day|year|date|time|money|(npc|item|var):[^{}]+)\}", RegexOptions.CultureInvariant)

    /// The substitutions (`{player}`, `{item:wood}`) in `text`, in order, each once.
    let placeholders (text: string) : string list =
        placeholder.Matches text
        |> Seq.map (fun m -> m.Value)
        |> Seq.distinct
        |> List.ofSeq

    // ── Files ──────────────────────────────────────────────────────────────

    let private csvField (text: string) = Csv.field text

    /// The game's texts for translating into `locale`, as CSV: key, context, source, the
    /// translation so far and its status.
    let exportCsv (project: GameProject) (locale: string) : string =
        let sb = StringBuilder("key,context,source,translation,status\r\n")
        let strings = table project locale |> Option.map (fun t -> dict t.Strings)

        for field in catalog project do
            let text =
                match strings with
                | Some s when s.ContainsKey field.Key -> s.[field.Key].Text
                | _ -> ""

            sb
                .Append(
                    String.Join(
                        ",",
                        [ field.Key; field.Context; field.Source; text; status project locale field ]
                        |> List.map csvField
                    )
                )
                .Append("\r\n")
            |> ignore

        sb.ToString()

    let private poString (text: string) =
        "\""
        + text.Replace("\\", "\\\\").Replace("\"", "\\\"").Replace("\n", "\\n").Replace("\t", "\\t").Replace("\r", "\\r")
        + "\""

    /// The same as a gettext PO file: `msgctxt` is the key, `msgid` the source, `#.` the
    /// context, and a stale translation is marked `fuzzy`.
    let exportPo (project: GameProject) (locale: string) : string =
        let sb = StringBuilder()
        let line (text: string) = sb.Append(text).Append('\n') |> ignore
        line (sprintf "# %s in %s" project.Name locale)
        line "msgid \"\""
        line "msgstr \"\""
        line "\"Content-Type: text/plain; charset=UTF-8\\n\""
        line (sprintf "\"Language: %s\\n\"" locale)
        let strings = table project locale |> Option.map (fun t -> dict t.Strings)

        for field in catalog project do
            line ""
            line ("#. " + field.Context.Replace("\n", " "))

            let state = status project locale field

            if state = "stale" then
                line "#, fuzzy"

            line ("msgctxt " + poString field.Key)
            line ("msgid " + poString field.Source)

            let text =
                match strings with
                | Some s when s.ContainsKey field.Key -> s.[field.Key].Text
                | _ -> ""

            line ("msgstr " + poString text)

        sb.ToString()

    /// What putting `entries` `(key, source as the file has it, translation)` into `locale`
    /// would change.
    let private importOf (project: GameProject) (locale: string) (entries: (string * string option * string) list) (errors: string list) =
        let sources = catalog project |> List.map (fun f -> f.Key, f.Source) |> dict
        let current = table project locale |> Option.map (fun t -> dict t.Strings)
        let mutable added, changed, unchanged = 0, 0, 0
        let unknown = ResizeArray<string>()

        let kept =
            entries
            |> List.choose (fun (key, fileSource, text) ->
                if not (sources.ContainsKey key) then
                    unknown.Add key
                    None
                elif text = "" then
                    None
                else
                    // Translated from the source the file shows (which may be older than the game's).
                    let hash = sourceHash (defaultArg fileSource sources.[key])

                    match current with
                    | Some c when c.ContainsKey key && c.[key].Text = text && c.[key].Source = hash -> unchanged <- unchanged + 1
                    | Some c when c.ContainsKey key -> changed <- changed + 1
                    | _ -> added <- added + 1

                    Some(key, text, hash))

        {
            Locale = locale
            Entries = kept
            Added = added
            Changed = changed
            Unchanged = unchanged
            Unknown = List.ofSeq unknown
            Errors = errors
        }

    /// Reads a CSV translation file (a `key` and a `translation` column; `source` when present
    /// says what was translated).
    let importCsv (project: GameProject) (locale: string) (csv: string) : TranslationImport =
        match Csv.parse csv with
        | Error e -> importOf project locale [] [ e ]
        | Ok [] -> importOf project locale [] [ "the file is empty" ]
        | Ok(header :: rows) ->
            let index name =
                header |> List.tryFindIndex (fun h -> h.Trim().ToLowerInvariant() = name)

            match index "key", index "translation" with
            | Some k, Some t ->
                let s = index "source"
                let errors = ResizeArray<string>()

                let entries =
                    rows
                    |> List.mapi (fun i row ->
                        if row.Length <= max k t then
                            errors.Add(sprintf "line %d: too few columns" (i + 2))
                            None
                        else
                            Some(row.[k].Trim(), (s |> Option.bind (fun s -> List.tryItem s row)), row.[t]))
                    |> List.choose id

                importOf project locale entries (List.ofSeq errors)
            | _ -> importOf project locale [] [ "line 1: the file needs a key and a translation column" ]

    /// Reads a PO translation file (`msgctxt` is the key).
    let importPo (project: GameProject) (locale: string) (po: string) : TranslationImport =
        let errors = ResizeArray<string>()
        let entries = ResizeArray<string * string option * string>()
        let mutable ctxt, id, str = None, None, None
        let mutable current = ""

        let unquote (lineNumber: int) (text: string) =
            let text = text.Trim()

            if text.Length >= 2 && text.StartsWith "\"" && text.EndsWith "\"" then
                let sb = StringBuilder()
                let body = text.Substring(1, text.Length - 2)
                let mutable i = 0

                while i < body.Length do
                    if body.[i] = '\\' && i + 1 < body.Length then
                        match body.[i + 1] with
                        | 'n' -> sb.Append '\n' |> ignore
                        | 't' -> sb.Append '\t' |> ignore
                        | 'r' -> sb.Append '\r' |> ignore
                        | c -> sb.Append c |> ignore

                        i <- i + 2
                    else
                        sb.Append body.[i] |> ignore
                        i <- i + 1

                sb.ToString()
            else
                errors.Add(sprintf "line %d: expected a quoted string" lineNumber)
                ""

        let flush () =
            match ctxt, str with
            | Some key, Some text -> entries.Add(key, id, text)
            | None, Some _ when id <> Some "" -> errors.Add(sprintf "an entry without msgctxt (%s)" (defaultArg id ""))
            | _ -> ()

            ctxt <- None
            id <- None
            str <- None

        po.Replace("\r\n", "\n").Split '\n'
        |> Array.iteri (fun i raw ->
            let n = i + 1
            let line = raw.Trim()

            if line = "" then
                flush ()
            elif line.StartsWith "#" then
                ()
            elif line.StartsWith "msgctxt " then
                if str.IsSome then
                    flush ()

                current <- "ctxt"
                ctxt <- Some(unquote n (line.Substring 8))
            elif line.StartsWith "msgid " then
                if str.IsSome then
                    flush ()

                current <- "id"
                id <- Some(unquote n (line.Substring 6))
            elif line.StartsWith "msgstr " then
                current <- "str"
                str <- Some(unquote n (line.Substring 7))
            elif line.StartsWith "\"" then
                let more = unquote n line

                match current with
                | "ctxt" -> ctxt <- ctxt |> Option.map (fun s -> s + more)
                | "id" -> id <- id |> Option.map (fun s -> s + more)
                | "str" -> str <- str |> Option.map (fun s -> s + more)
                | _ -> errors.Add(sprintf "line %d: a string outside an entry" n)
            else
                errors.Add(sprintf "line %d: not a PO line" n))

        flush ()
        importOf project locale (List.ofSeq entries) (List.ofSeq errors)

    /// The edit an import makes: its translations replace those of the same keys (others stay).
    /// One undo step.
    let applyImport (project: GameProject) (import: TranslationImport) : Edit =
        let strings =
            table project import.Locale
            |> Option.map (fun t -> t.Strings)
            |> Option.defaultValue []

        let incoming =
            import.Entries
            |> List.map (fun (k, text, hash) -> k, { Text = text; Source = hash })

        let byKey = dict incoming

        let merged =
            (strings
             |> List.map (fun (k, v) -> if byKey.ContainsKey k then k, byKey.[k] else k, v))
            @ (incoming
               |> List.filter (fun (k, _) -> not (strings |> List.exists (fun (s, _) -> s = k))))

        ReplaceProject(withTable project import.Locale merged)

    // ── Ids ────────────────────────────────────────────────────────────────

    /// The edit that gives every NPC line, dialogue answer and cutscene line, question and answer
    /// that lacks one a stable id, keeping translations made under its `#INDEX` key. One undo step.
    let assignIds (project: GameProject) : Edit =
        let renames = ResizeArray<string * string>()

        let npcs =
            project.Npcs
            |> List.map (fun npc ->
                let next = DialogueScript.withIds npc

                for k, (before, after) in List.indexed (List.zip (defaultArg npc.Lines []) (defaultArg next.Lines [])) do
                    if before.Id <> after.Id then
                        renames.Add(sprintf "npc:%s:line:#%d" npc.Id k, sprintf "npc:%s:line:%s" npc.Id after.Id.Value)

                for d, d2 in List.zip npc.Dialogue next.Dialogue do
                    for k, (o, o2) in List.indexed (List.zip d.Options d2.Options) do
                        if o.Id <> o2.Id then
                            renames.Add(sprintf "dialogue:%s:option:#%d" d.Id k, sprintf "dialogue:%s:option:%s" d.Id o2.Id.Value)

                next)

        let npcDialogues =
            npcs
            |> List.collect (fun n -> n.Dialogue)
            |> List.map (fun d -> d.Id, d)
            |> dict

        let dialogues =
            project.Dialogues
            |> List.map (fun d ->
                match npcDialogues.TryGetValue d.Id with
                | true, copy when copy.Options.Length = d.Options.Length && copy.Text = d.Text -> copy
                | _ -> d)

        let cutscenes =
            project.Cutscenes
            |> Option.map (
                List.map (fun c ->
                    let next = DialogueScript.cutsceneWithIds c

                    for k, (s, s2) in List.indexed (List.zip c.Steps next.Steps) do
                        let before = sprintf "cutscene:%s:step:%s" c.Id (part s.Id k)
                        let after = sprintf "cutscene:%s:step:%s" c.Id (part s2.Id k)

                        if before <> after then
                            renames.Add(before, after)

                        for j, (ch, ch2) in List.indexed (List.zip s.Options s2.Options) do
                            let b = sprintf "%s:choice:%s" before (part ch.Id j)
                            let a = sprintf "%s:choice:%s" after (part ch2.Id j)

                            if a <> b then
                                renames.Add(b, a)

                    next)
            )

        let renamed = dict renames

        let translations =
            project.Translations
            |> Option.map (
                List.map (fun t ->
                    { t with
                        Strings =
                            t.Strings
                            |> List.map (fun (k, v) ->
                                match renamed.TryGetValue k with
                                | true, next -> next, v
                                | _ -> k, v)
                    })
            )

        ReplaceProject
            { project with
                Npcs = npcs
                Dialogues = dialogues
                Cutscenes = cutscenes
                Translations = translations
            }

    // ── Problems ───────────────────────────────────────────────────────────

    /// Translation problems: languages that lack texts, translations whose source changed since
    /// (stale), keys the game no longer has, text the game font cannot draw and substitutions
    /// a translation drops or adds.
    let check (project: GameProject) (add: Severity -> string -> string -> string -> unit) =
        let tables = defaultArg project.Translations []

        if not tables.IsEmpty then
            let fields = catalog project
            let byKey = fields |> List.map (fun f -> f.Key, f) |> dict
            let seenLocales = HashSet<string>()

            tables
            |> List.iteri (fun i t ->
                let path = sprintf "translations[%d]" i

                if t.Locale.Trim() = "" then
                    add Severity.Error "translation.locale" path "A language has no locale code (es, pt-BR)"
                elif not (seenLocales.Add t.Locale) then
                    add Severity.Error "translation.locale" path (sprintf "The language %s comes twice" t.Locale)
                elif t.Locale = project.Settings.Locale then
                    add
                        Severity.Warning
                        "translation.locale"
                        path
                        (sprintf "%s is the game's own language (Settings → Locale); its table is never shown" t.Locale)

                let strings = dict t.Strings

                let missing =
                    fields
                    |> List.filter (fun f -> not (strings.ContainsKey f.Key) || strings.[f.Key].Text = "")

                if not missing.IsEmpty then
                    add
                        Severity.Warning
                        "translation.missing"
                        path
                        (sprintf
                            "%s lacks %d of %d texts (%s%s); the game shows them in its own language"
                            t.Locale
                            missing.Length
                            fields.Length
                            (missing |> List.truncate 3 |> List.map (fun f -> f.Key) |> String.concat ", ")
                            (if missing.Length > 3 then ", …" else ""))

                for key, entry in t.Strings do
                    let entryPath = sprintf "%s.strings.%s" path key

                    match byKey.TryGetValue key with
                    | false, _ ->
                        add
                            Severity.Info
                            "translation.unknown"
                            entryPath
                            (sprintf "%s: the game has no text %s (it was removed or its id changed)" t.Locale key)
                    | true, field when entry.Text <> "" ->
                        if entry.Source <> "" && entry.Source <> sourceHash field.Source then
                            add
                                Severity.Warning
                                "translation.stale"
                                entryPath
                                (sprintf "%s: %s changed after it was translated (now \"%s\")" t.Locale key field.Source)

                        match FontCoverage.firstMissing entry.Text with
                        | Some rune ->
                            add
                                Severity.Warning
                                "translation.fontCoverage"
                                entryPath
                                (sprintf "%s: the game font cannot draw U+%04X (%s) in %s; it will appear as a box" t.Locale rune.Value (rune.ToString()) key)
                        | None -> ()

                        let wanted = placeholders field.Source
                        let found = placeholders entry.Text
                        let lost = wanted |> List.filter (fun p -> not (List.contains p found))
                        let extra = found |> List.filter (fun p -> not (List.contains p wanted))

                        if not lost.IsEmpty then
                            add Severity.Warning "translation.substitution" entryPath (sprintf "%s: %s lacks %s" t.Locale key (String.concat " " lost))

                        if not extra.IsEmpty then
                            add
                                Severity.Warning
                                "translation.substitution"
                                entryPath
                                (sprintf "%s: %s has %s, which the source lacks" t.Locale key (String.concat " " extra))
                    | _ -> ())

    // ── Compiling ──────────────────────────────────────────────────────────

    /// The tables the cartridge carries: every language's translated texts by key (stale ones
    /// too: an older translation reads better than none).
    let contentStrings (project: GameProject) : (string * (string * string) list) list =
        let known = catalog project |> List.map (fun f -> f.Key) |> HashSet

        defaultArg project.Translations []
        |> List.filter (fun t -> t.Locale <> "" && t.Locale <> project.Settings.Locale)
        |> List.map (fun t ->
            t.Locale,
            t.Strings
            |> List.filter (fun (k, v) -> v.Text <> "" && known.Contains k)
            |> List.map (fun (k, v) -> k, v.Text))
        |> List.filter (fun (_, strings) -> not strings.IsEmpty)
