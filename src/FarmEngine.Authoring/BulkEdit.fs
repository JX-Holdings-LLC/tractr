namespace FarmEngine.Authoring

open System
open System.Collections.Generic
open FarmEngine.Schemas

/// What a bulk edit does to one field of every selected entry.
[<RequireQualifiedAccess>]
type BulkOperation =
    /// `= 50`: the field becomes this text.
    | Set of string
    /// `× 1.1` (or `* 1.1`): a number field multiplied.
    | Multiply of float
    /// `+ 1` / `- 1`: a number field increased or decreased.
    | Add of float
    /// `+ summer`: a word added to a list field (seasons, tags).
    | AddWord of string
    /// `- summer`: a word taken out of a list field.
    | RemoveWord of string

/// Which entries a bulk edit selects: those that carry every tag in `Tags`, match `Text` in some
/// column, and — when `Ids` is given — are among those ids (a multi-selection).
type BulkSelection =
    {
        Tags: string list
        Text: string
        Ids: string list option
    }

    static member All = { Tags = []; Text = ""; Ids = None }

/// What a bulk edit would change, before it is applied: a row per selected entry it changes, and
/// the entries it cannot change.
type BulkPreview =
    {
        Changes: RowChange list
        Invalid: CellError list
        /// The rows after the edit (internal: what `BulkEdit.apply` writes).
        Rows: (string * Json) list
    }

/// Bulk edits (#227): select entries of a table category (by tag, text or a multi-selection) and
/// set or adjust one field across all of them, shown first as a preview, applied as one undo step.
/// Tags are the `tags` column every table category has.
module BulkEdit =
    /// The operation `text` stands for: `= 50`, `× 1.1`, `* 1.1`, `+ 1`, `- 2`, `+ summer`,
    /// `- summer`.
    let parseOperation (text: string) : Result<BulkOperation, string> =
        let text = text.Trim()

        if text.Length < 2 then
            Error "write = VALUE, × FACTOR, + N, - N, + WORD or - WORD"
        else
            let rest = text.Substring(1).Trim()
            let n = JsNumber.parse rest
            let isNumber = rest <> "" && not (Double.IsNaN n) && not (Double.IsInfinity n)

            match text.[0] with
            | '=' -> Ok(BulkOperation.Set rest)
            | '×'
            | '*'
            | 'x' when isNumber -> Ok(BulkOperation.Multiply n)
            | '+' when isNumber -> Ok(BulkOperation.Add n)
            | '-' when isNumber -> Ok(BulkOperation.Add(-n))
            | '+' when rest <> "" -> Ok(BulkOperation.AddWord rest)
            | '-' when rest <> "" -> Ok(BulkOperation.RemoveWord rest)
            | _ -> Error "write = VALUE, × FACTOR, + N, - N, + WORD or - WORD"

    let private tagsOf (json: Json) =
        match Json.get "tags" json with
        | JArray tags -> tags |> List.choose Json.asString
        | _ -> []

    /// The ids of the rows of `category` that `selection` selects, in table order.
    let select (category: string) (project: GameProject) (selection: BulkSelection) : string list =
        let columns = ContentTables.columns category
        let ids = selection.Ids |> Option.map Set.ofList

        ContentTables.rowJson category project
        |> List.filter (fun (rowId, json) ->
            let tags = tagsOf json

            (ids |> Option.forall (fun ids -> ids.Contains rowId))
            && (selection.Tags |> List.forall (fun t -> List.contains t tags))
            && (String.IsNullOrWhiteSpace selection.Text
                || columns
                   |> List.exists (fun col ->
                       (ContentTables.cellText category project rowId json col)
                           .Contains(selection.Text.Trim(), StringComparison.OrdinalIgnoreCase))))
        |> List.map fst

    /// Every tag the category's entries carry, in order of first use.
    let tags (category: string) (project: GameProject) : string list =
        ContentTables.rowJson category project |> List.collect (snd >> tagsOf) |> List.distinct

    let private apply1 (column: TableColumn) (operation: BulkOperation) (before: string) : Result<string, string> =
        let number () =
            let n = JsNumber.parse before

            if before.Trim() = "" || Double.IsNaN n then
                Error(sprintf "%s has no number to change" column.Header)
            else
                Ok n

        let words () =
            before.Split([| ','; ';' |], StringSplitOptions.RemoveEmptyEntries ||| StringSplitOptions.TrimEntries)
            |> List.ofArray

        let isList =
            column.Kind = ColumnKind.Words || column.Kind = ColumnKind.OptionalWords

        match operation with
        | BulkOperation.Set text -> Ok text
        | BulkOperation.Multiply factor -> number () |> Result.map (fun n -> JsNumber.format (n * factor))
        | BulkOperation.Add delta -> number () |> Result.map (fun n -> JsNumber.format (n + delta))
        | BulkOperation.AddWord word when isList ->
            let list = words ()
            Ok(String.Join(", ", (if List.contains word list then list else list @ [ word ])))
        | BulkOperation.RemoveWord word when isList -> Ok(String.Join(", ", words () |> List.filter ((<>) word)))
        | _ -> Error(sprintf "%s is not a list: use = to set it" column.Header)

    /// What `operation` on column `columnKey` of the rows `ids` would change.
    let preview (category: string) (project: GameProject) (ids: string list) (columnKey: string) (operation: BulkOperation) : BulkPreview =
        let columns = ContentTables.columns category
        let rows = ContentTables.rowJson category project
        let wanted = Set.ofList ids

        match columns |> List.tryFind (fun c -> c.Key = columnKey) with
        | None ->
            {
                Changes = []
                Invalid = [ { Row = 0; Column = columnKey; Message = "there is no such column" } ]
                Rows = rows
            }
        | Some column when column.ReadOnly ->
            {
                Changes = []
                Invalid = [ { Row = 0; Column = column.Header; Message = "this column cannot be changed" } ]
                Rows = rows
            }
        | Some column ->
            let changes = ResizeArray<RowChange>()
            let invalid = ResizeArray<CellError>()

            let next =
                rows
                |> List.mapi (fun index (rowId, json) ->
                    if not (wanted.Contains rowId) then
                        rowId, json
                    else
                        let before = ContentTables.cellText category project rowId json column

                        match apply1 column operation before |> Result.bind (fun text -> ContentTables.write column text json) with
                        | Ok after ->
                            let afterText = ContentTables.cellText category project rowId after column

                            if afterText <> before then
                                changes.Add
                                    {
                                        Id = rowId
                                        Changes = [ { Column = column.Header; Before = before; After = afterText } ]
                                    }

                            rowId, after
                        | Error message ->
                            invalid.Add { Row = index + 1; Column = column.Header; Message = sprintf "%s: %s" rowId message }
                            rowId, json)

            {
                Changes = List.ofSeq changes
                Invalid = List.ofSeq invalid
                Rows = next
            }

    /// The edit a preview makes: one undo step. Refused while some selected entry cannot change.
    let apply (category: string) (project: GameProject) (preview: BulkPreview) : Result<Edit, CellError list> =
        if not preview.Invalid.IsEmpty then
            Error preview.Invalid
        else
            ContentTables.writeRows category project (sprintf "Bulk edit %s" (ContentTables.name category)) preview.Rows

    /// The edit that adds `tag` to (or with `remove`, takes it from) the rows `ids`.
    let tag (category: string) (project: GameProject) (ids: string list) (tag: string) (remove: bool) : Result<Edit, CellError list> =
        let operation =
            if remove then BulkOperation.RemoveWord tag else BulkOperation.AddWord tag

        apply category project (preview category project ids "tags" operation)

/// Content templates (#227): save an entry, or a group (a seed, its crop and the harvest), and
/// make many new ones from it with a table of the fields that differ.
module ContentTemplates =
    let private templatesOf (project: GameProject) = defaultArg project.ContentTemplates []

    /// The edit that saves the entries `(category, id)` as template `name` (replacing a template
    /// with the same id). One undo step.
    let save (project: GameProject) (id: string) (name: string) (entries: (string * string) list) : Result<Edit, string> =
        let found =
            entries
            |> List.map (fun (category, entryId) ->
                match ContentTables.rowJson category project |> List.tryFind (fun (rowId, _) -> rowId = entryId) with
                | Some(_, json) -> Ok { Kind = category; Entry = Some json }
                | None -> Error(sprintf "there is no %s %s" category entryId))

        match found |> List.tryPick (function | Error e -> Some e | Ok _ -> None) with
        | Some e -> Error e
        | None when entries.IsEmpty -> Error "a template needs at least one entry"
        | None ->
            let template =
                {
                    Id = id
                    Name = name
                    Entries = found |> List.choose (function | Ok e -> Some e | Error _ -> None)
                }

            let others = templatesOf project |> List.filter (fun t -> t.Id <> id)
            Ok(ReplaceProject { project with ContentTemplates = Some(others @ [ template ]) })

    /// The edit that removes template `id`.
    let remove (project: GameProject) (id: string) : Edit =
        let rest = templatesOf project |> List.filter (fun t -> t.Id <> id)
        ReplaceProject { project with ContentTemplates = (if rest.IsEmpty then None else Some rest) }

    /// The columns of the table that makes entries from `template`: `ENTRY.KEY` for each entry
    /// (numbered from 1) and each column of its category that can be written.
    let columns (template: ContentTemplate) : (string * TableColumn) list =
        template.Entries
        |> List.indexed
        |> List.collect (fun (k, entry) ->
            ContentTables.columns entry.Kind
            |> List.filter (fun c -> c.Kind <> ColumnKind.Computed)
            |> List.map (fun c -> sprintf "%d.%s" (k + 1) c.Key, c))

    let private replaceStrings (renames: (string * string) list) (json: Json) =
        let rec walk json =
            match json with
            | JString s ->
                match renames |> List.tryFind (fun (old, _) -> old = s) with
                | Some(_, next) -> JString next
                | None -> json
            | JArray items -> JArray(List.map walk items)
            | JObject members -> JObject(members |> List.map (fun (k, v) -> k, walk v))
            | other -> other

        walk json

    /// The edit that makes one new group of entries per row of `instances` (each a list of
    /// `(column, text)` from `columns`; a column left out keeps the template's value). A new id
    /// replaces the template's id everywhere in the group (the crop's harvest item, the seed's
    /// crop). An entry given no id gets the template's with `-2`, `-3`… One undo step.
    let make (project: GameProject) (template: ContentTemplate) (instances: (string * string) list list) : Result<Edit, CellError list> =
        let errors = ResizeArray<CellError>()
        let taken = HashSet<string>()

        for entry in template.Entries do
            for rowId, _ in ContentTables.rowJson entry.Kind project do
                taken.Add(entry.Kind + "|" + rowId) |> ignore

        let newRows = Dictionary<string, ResizeArray<string * Json>>()
        let cols = columns template |> dict

        instances
        |> List.iteri (fun row cells ->
            let rowNumber = row + 1

            for name, _ in cells do
                if not (cols.ContainsKey name) then
                    errors.Add { Row = rowNumber; Column = name; Message = "there is no such column" }

            // New ids first, so every entry of the group can point at the others.
            let ids =
                template.Entries
                |> List.mapi (fun k entry ->
                    let json = defaultArg entry.Entry JNull
                    let oldId = Json.get "id" json |> Json.asString |> Option.defaultValue ""

                    let given =
                        cells |> List.tryFind (fun (name, _) -> name = sprintf "%d.id" (k + 1)) |> Option.map (snd >> fun s -> s.Trim())

                    let newId =
                        match given with
                        | Some id when id <> "" -> id
                        | _ ->
                            let mutable n = 2

                            while taken.Contains(entry.Kind + "|" + sprintf "%s-%d" oldId n) do
                                n <- n + 1

                            sprintf "%s-%d" oldId n

                    if oldId = "" then None
                    elif not (taken.Add(entry.Kind + "|" + newId)) then
                        errors.Add { Row = rowNumber; Column = sprintf "%d.id" (k + 1); Message = sprintf "%s is taken" newId }
                        None
                    else
                        Some(oldId, newId))

            let renames = ids |> List.choose id

            template.Entries
            |> List.iteri (fun k entry ->
                let mutable json = replaceStrings renames (defaultArg entry.Entry JNull)

                for name, text in cells do
                    match cols.TryGetValue name with
                    | true, column when name.StartsWith(sprintf "%d." (k + 1)) && column.Key <> "id" ->
                        match ContentTables.write column text json with
                        | Ok next -> json <- next
                        | Error message -> errors.Add { Row = rowNumber; Column = name; Message = message }
                    | _ -> ()

                let rowId =
                    match List.tryItem k ids |> Option.flatten with
                    | Some(_, newId) when Json.has "id" json -> newId
                    | _ -> ContentTables.idOf entry.Kind json

                if not (newRows.ContainsKey entry.Kind) then
                    newRows.[entry.Kind] <- ResizeArray()

                newRows.[entry.Kind].Add(rowId, json)))

        if errors.Count > 0 then
            Error(List.ofSeq errors)
        else
            let edits =
                newRows
                |> Seq.map (fun pair ->
                    ContentTables.writeRows pair.Key project "Make from template" (ContentTables.rowJson pair.Key project @ List.ofSeq pair.Value))
                |> List.ofSeq

            match edits |> List.tryPick (function | Error e -> Some e | Ok _ -> None) with
            | Some e -> Error e
            | None -> Ok(Batch("Make from template", edits |> List.choose (function | Ok e -> Some e | Error _ -> None)))
