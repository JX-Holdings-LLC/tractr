namespace FarmEngine.Authoring

open System.Collections.Generic
open FarmEngine.Schemas

/// The result of reading text a creator wrote (a script, a CSV, a bulk edit): the edit to apply,
/// or what is wrong, each with its line or row.
type ToolResult(edit: Edit option, errors: string list) =
    /// The edit (one undo step), or null when there are errors.
    member _.Edit: Edit | null =
        match edit with
        | Some e -> e
        | None -> null

    member _.Errors: IReadOnlyList<string> =
        errors |> Array.ofList :> IReadOnlyList<string>

    member _.Ok = edit.IsSome && errors.IsEmpty

    static member internal OfScript(result: Result<Edit, ScriptError list>) =
        match result with
        | Ok edit -> ToolResult(Some edit, [])
        | Error errors -> ToolResult(None, errors |> List.map (fun e -> sprintf "Line %d: %s" e.Line e.Message))

    static member internal OfCells(result: Result<Edit, CellError list>) =
        match result with
        | Ok edit -> ToolResult(Some edit, [])
        | Error errors ->
            ToolResult(
                None,
                errors
                |> List.map (fun e ->
                    match e.Row, e.Column with
                    | 0, "" -> e.Message
                    | 0, column -> sprintf "%s: %s" column e.Message
                    | row, "" -> sprintf "Row %d: %s" row e.Message
                    | row, column -> sprintf "Row %d, %s: %s" row column e.Message)
            )

/// The authoring-at-scale tools for C# and other .NET callers (#225–#228): dialogue scripts,
/// content tables and CSV, bulk edits, templates and translations, with .NET collections.
[<AbstractClass; Sealed>]
type AuthoringTools =
    // ── Dialogue scripts ──────────────────────────────────────────────────

    /// NPC `npcId`'s dialogues and lines as a script ("" for an unknown NPC).
    static member NpcScript(project: GameProject, npcId: string) : string =
        match project.Npcs |> List.tryFind (fun n -> n.Id = npcId) with
        | Some npc -> DialogueScript.printNpc npc
        | None -> ""

    /// The project's cutscenes as a script.
    static member CutsceneScript(project: GameProject) : string =
        DialogueScript.printCutscenes (defaultArg project.Cutscenes [])

    /// What is wrong with a script, by line ("Line 4: …"); empty when it reads.
    static member CheckScript(project: GameProject, text: string) : IReadOnlyList<string> =
        DialogueScript.check project text
        |> List.map (fun e -> sprintf "Line %d: %s" e.Line e.Message)
        |> Array.ofList
        :> IReadOnlyList<string>

    /// The edit a script makes (one undo step), or its mistakes.
    static member ApplyScript(project: GameProject, text: string) : ToolResult =
        ToolResult.OfScript(DialogueScript.edit project text)

    /// The project's problems in NPC `npcId`'s dialogue, as "Line N: message" for its script.
    static member ScriptProblems(project: GameProject, npcId: string) : IReadOnlyList<string> =
        DialogueScript.npcProblems project (Problems.collect project) npcId
        |> List.map (fun (line, p) -> sprintf "Line %d: %s" line p.Message)
        |> Array.ofList
        :> IReadOnlyList<string>

    // ── Tables ────────────────────────────────────────────────────────────

    static member TableCategories: IReadOnlyList<string> =
        ContentTables.Categories |> Array.ofList :> IReadOnlyList<string>

    static member TableName(category: string) : string = ContentTables.name category

    static member TableColumns(category: string) : IReadOnlyList<TableColumn> =
        ContentTables.columns category |> Array.ofList :> IReadOnlyList<TableColumn>

    /// The rows whose cells contain `filter` (all for an empty one), each carrying tag `tag` when
    /// given.
    static member TableRows(project: GameProject, category: string, filter: string, tag: string | null) : IReadOnlyList<TableRow> =
        let ids =
            BulkEdit.select
                category
                project
                { BulkSelection.All with
                    Tags =
                        (match tag with
                         | null
                         | "" -> []
                         | t -> [ t ])
                    Text = filter
                }
            |> Set.ofList

        ContentTables.rows category project
        |> List.filter (fun r -> ids.Contains r.Id)
        |> Array.ofList
        :> IReadOnlyList<TableRow>

    static member SetCell(project: GameProject, category: string, rowId: string, columnKey: string, text: string) : ToolResult =
        ToolResult.OfCells(ContentTables.setCells category project [ rowId, columnKey, text ])

    static member Paste(project: GameProject, category: string, rowIds: seq<string>, columnKey: string, text: string) : ToolResult =
        ToolResult.OfCells(ContentTables.paste category project (List.ofSeq rowIds) columnKey text)

    static member ExportCsv(project: GameProject, category: string) : string = ContentTables.toCsv category project
    static member ImportDiff(project: GameProject, category: string, csv: string) : TableDiff = ContentTables.diff category project csv

    static member ApplyImport(project: GameProject, diff: TableDiff, removeMissing: bool) : ToolResult =
        ToolResult.OfCells(ContentTables.applyImport project diff removeMissing)

    // ── Bulk edits and templates ──────────────────────────────────────────

    static member Tags(project: GameProject, category: string) : IReadOnlyList<string> =
        BulkEdit.tags category project |> Array.ofList :> IReadOnlyList<string>

    /// The preview of `operation` (`= 50`, `× 1.1`, `+ 1`, `+ summer`) on column `columnKey` of
    /// the rows `rowIds`; None when the operation does not read.
    static member BulkPreview(project: GameProject, category: string, rowIds: seq<string>, columnKey: string, operation: string) : BulkPreview option =
        match BulkEdit.parseOperation operation with
        | Ok op -> Some(BulkEdit.preview category project (List.ofSeq rowIds) columnKey op)
        | Error _ -> None

    static member BulkApply(project: GameProject, category: string, preview: BulkPreview) : ToolResult =
        ToolResult.OfCells(BulkEdit.apply category project preview)

    static member Tag(project: GameProject, category: string, rowIds: seq<string>, tag: string, remove: bool) : ToolResult =
        ToolResult.OfCells(BulkEdit.tag category project (List.ofSeq rowIds) tag remove)

    static member SaveTemplate(project: GameProject, id: string, name: string, entries: seq<struct (string * string)>) : ToolResult =
        match ContentTemplates.save project id name (entries |> Seq.map (fun (struct (c, i)) -> c, i) |> List.ofSeq) with
        | Ok edit -> ToolResult(Some edit, [])
        | Error e -> ToolResult(None, [ e ])

    static member TemplateColumns(template: ContentTemplate) : IReadOnlyList<string> =
        ContentTemplates.columns template |> List.map fst |> Array.ofList :> IReadOnlyList<string>

    /// New groups from `template`, one per row of `(column, text)` cells.
    static member MakeFromTemplate(project: GameProject, template: ContentTemplate, rows: seq<IReadOnlyDictionary<string, string>>) : ToolResult =
        let instances =
            rows
            |> Seq.map (fun row -> row |> Seq.map (fun pair -> pair.Key, pair.Value) |> List.ofSeq)
            |> List.ofSeq

        ToolResult.OfCells(ContentTemplates.make project template instances)

    // ── Translations ──────────────────────────────────────────────────────

    static member TextFields(project: GameProject) : IReadOnlyList<TextField> =
        Translations.catalog project |> Array.ofList :> IReadOnlyList<TextField>

    static member Locales(project: GameProject) : IReadOnlyList<string> =
        Translations.locales project |> Array.ofList :> IReadOnlyList<string>

    static member AddLocale(project: GameProject, locale: string) : Edit = Translations.addLocale project locale

    static member RemoveLocale(project: GameProject, locale: string) : Edit =
        Translations.removeLocale project locale

    static member SetTranslation(project: GameProject, locale: string, key: string, text: string) : Edit =
        Translations.setText project locale key text

    static member TranslationStatus(project: GameProject, locale: string, field: TextField) : string =
        Translations.status project locale field

    static member ExportTranslationCsv(project: GameProject, locale: string) : string = Translations.exportCsv project locale
    static member ExportTranslationPo(project: GameProject, locale: string) : string = Translations.exportPo project locale

    /// Reads a translation file: PO when it looks like one (`msgid`), else CSV.
    static member ReadTranslations(project: GameProject, locale: string, text: string) : TranslationImport =
        if text.Contains "msgid " then
            Translations.importPo project locale text
        else
            Translations.importCsv project locale text

    static member ApplyTranslations(project: GameProject, import: TranslationImport) : Edit = Translations.applyImport project import
    static member AssignTextIds(project: GameProject) : Edit = Translations.assignIds project
