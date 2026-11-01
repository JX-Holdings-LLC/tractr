namespace FarmEngine.Authoring

open System
open System.Collections.Generic
open System.Text
open FarmEngine.Schemas

/// How a table cell is written and read.
[<RequireQualifiedAccess>]
type ColumnKind =
    | Text
    /// Text that may be empty (absent from the entry).
    | OptionalText
    | Number
    /// A number that may be empty (absent from the entry).
    | OptionalNumber
    | Bool
    /// Words separated by commas (`spring, summer`).
    | Words
    /// Words that may be empty (absent from the entry).
    | OptionalWords
    /// Ingredients: `2 wood, 1 stone`.
    | Ingredients
    /// Worked out from the entry; never written.
    | Computed

/// A column of a content table: the entry's JSON key it shows (`value`, `seasons`) and its header.
type TableColumn =
    {
        Key: string
        Header: string
        Kind: ColumnKind
    }

    member this.ReadOnly = this.Kind = ColumnKind.Computed || this.Key = "id"

/// One row of a table: its id (an entry id, or `shop/item` for a shop's stock) and its cells, in
/// column order.
type TableRow = { Id: string; Cells: string list }

/// A cell that cannot be written: its row (1-based, the CSV line when importing), column and why.
type CellError =
    {
        Row: int
        Column: string
        Message: string
    }

/// A cell an import or a bulk edit changes.
type CellChange =
    {
        Column: string
        Before: string
        After: string
    }

/// A row an import or a bulk edit changes, and how.
type RowChange =
    { Id: string; Changes: CellChange list }

/// What an import would do (#226): rows it adds, changes and (when the file lacks them) removes,
/// and the cells it cannot read. Nothing is applied until `ContentTables.applyImport`.
type TableDiff =
    {
        Category: string
        Added: RowChange list
        Changed: RowChange list
        Removed: string list
        Invalid: CellError list
        /// The rows after the import, in order (internal: what `applyImport` writes).
        Rows: (string * Json) list
    }

    member this.HasChanges =
        not (this.Added.IsEmpty && this.Changed.IsEmpty && this.Removed.IsEmpty)

/// Content as tables (#226): items, crops, recipes, shop stock, fish tables and node drops, a
/// row per entry (or per stock entry, fish or drop) and a column per field, with computed
/// read-only columns (profit per harvest, per craft and per machine hour). Cells are text, so a
/// table can be edited inline, pasted into many rows at once and saved to and read from CSV
/// (matched by id); every write is one undo step (a `Batch`).
module ContentTables =
    // ── Categories ─────────────────────────────────────────────────────────

    let private col key header kind : TableColumn =
        {
            Key = key
            Header = header
            Kind = kind
        }

    /// A category: its columns, its rows as JSON (the entry, or a parent's id plus a child), and
    /// how rows go back into the project.
    type private Category =
        {
            Id: string
            Name: string
            Columns: TableColumn list
            Rows: GameProject -> (string * Json) list
            /// The id of a new row from its JSON (an entry's `id`, `shop/item` for stock).
            IdOf: Json -> string
            /// Edits that make the project hold `rows` (in order) in place of the current ones.
            Write: GameProject -> (string * Json) list -> Result<Edit list, string * string>
            Computed: GameProject -> Json -> string -> string
            /// A new entry's JSON before its cells are written.
            Blank: Json
            /// Rows are children of other entries (their id is `parent/item`).
            Children: bool
        }

    let private str (json: Json) =
        Json.asString json |> Option.defaultValue ""

    let private idKey (json: Json) = Json.get "id" json |> str

    let private decodeOr (decode: Path -> Json -> 'T) (id: string) (json: Json) : Result<'T, string * string> =
        try
            Ok(decode [] json)
        with DecodeError(path, message) ->
            Error(id, (if path = "" then message else path + ": " + message))

    let rec private allOk (results: Result<'T, 'E> list) : Result<'T list, 'E> =
        match results with
        | [] -> Ok []
        | Ok v :: rest -> allOk rest |> Result.map (fun vs -> v :: vs)
        | Error e :: _ -> Error e

    /// A category of entries with ids, kept in a list of the project.
    let private entries
        (id: string)
        (name: string)
        (columns: TableColumn list)
        (get: GameProject -> 'T list)
        (idOf: 'T -> string)
        (encode: 'T -> Json)
        (decode: Path -> Json -> 'T)
        (upsert: 'T -> Edit)
        (remove: string -> Edit)
        (blank: 'T)
        (computed: GameProject -> Json -> string -> string)
        : Category =
        {
            Id = id
            Name = name
            Columns = columns
            Rows = fun project -> get project |> List.map (fun e -> idOf e, encode e)
            IdOf = idKey
            Write =
                fun project rows ->
                    let current = get project |> List.map (fun e -> idOf e, e) |> dict
                    let wanted = rows |> List.map fst |> Set.ofList

                    rows
                    |> List.map (fun (rowId, json) ->
                        decodeOr decode rowId json
                        |> Result.map (fun entry ->
                            match current.TryGetValue rowId with
                            | true, existing when existing = entry -> None
                            | _ -> Some(upsert entry)))
                    |> allOk
                    |> Result.map (fun upserts ->
                        let removals =
                            get project
                            |> List.map idOf
                            |> List.filter (fun i -> not (wanted.Contains i))
                            |> List.map remove

                        List.choose (fun x -> x) upserts @ removals)
            Computed = computed
            Blank = encode blank
            Children = false
        }

    /// A category of child rows (a shop's stock entries): `parent/childKey` ids, a `#2` on repeats.
    let private children
        (id: string)
        (name: string)
        (parentKey: string)
        (childKey: string)
        (columns: TableColumn list)
        (parents: GameProject -> 'P list)
        (parentId: 'P -> string)
        (childrenOf: 'P -> 'C list)
        (withChildren: 'P -> 'C list -> 'P)
        (encode: 'C -> Json)
        (decode: Path -> Json -> 'C)
        (upsert: 'P -> Edit)
        (blank: 'C)
        : Category =
        let rowsOf (parent: 'P) =
            let seen = Dictionary<string, int>()

            childrenOf parent
            |> List.map (fun child ->
                let json = encode child
                let key = parentId parent + "/" + str (Json.get childKey json)

                let n =
                    match seen.TryGetValue key with
                    | true, n -> n + 1
                    | _ -> 1

                seen.[key] <- n
                (if n = 1 then key else sprintf "%s#%d" key n), Json.set parentKey (JString(parentId parent)) json)

        {
            Id = id
            Name = name
            Columns = columns
            Rows = fun project -> parents project |> List.collect rowsOf
            IdOf = fun json -> str (Json.get parentKey json) + "/" + str (Json.get childKey json)
            Write =
                fun project rows ->
                    let byParent = Dictionary<string, ResizeArray<string * Json>>()

                    for rowId, json in rows do
                        let p = str (Json.get parentKey json)

                        if not (byParent.ContainsKey p) then
                            byParent.[p] <- ResizeArray()

                        byParent.[p].Add(rowId, Json.remove parentKey json)

                    let known = parents project |> List.map parentId |> Set.ofList

                    match byParent.Keys |> Seq.tryFind (fun p -> not (known.Contains p)) with
                    | Some missing ->
                        let rowId = fst byParent.[missing].[0]
                        Error(rowId, sprintf "there is no %s %s" parentKey missing)
                    | None ->
                        parents project
                        |> List.map (fun parent ->
                            let wanted =
                                match byParent.TryGetValue(parentId parent) with
                                | true, list -> List.ofSeq list
                                | _ -> []

                            wanted
                            |> List.map (fun (rowId, json) -> decodeOr decode rowId json)
                            |> allOk
                            |> Result.map (fun kids ->
                                if kids = childrenOf parent then
                                    None
                                else
                                    Some(upsert (withChildren parent kids))))
                        |> allOk
                        |> Result.map (List.choose (fun x -> x))
            Computed = fun _ _ _ -> ""
            Blank = encode blank
            Children = true
        }

    let private noComputed (_: GameProject) (_: Json) (_: string) = ""

    let private money (value: float) =
        JsNumber.format (JsNumber.roundHalfAway value)

    let private categories: Category list =
        [
            entries
                "items"
                "Items"
                [
                    col "id" "Id" ColumnKind.Text
                    col "name" "Name" ColumnKind.Text
                    col "type" "Kind" ColumnKind.Text
                    col "value" "Value" ColumnKind.Number
                    col "stackable" "Stackable" ColumnKind.Bool
                    col "maxStack" "Max stack" ColumnKind.Number
                    col "energy" "Energy" ColumnKind.OptionalNumber
                    col "health" "Health" ColumnKind.OptionalNumber
                    col "description" "Description" ColumnKind.Text
                    col "tags" "Tags" ColumnKind.OptionalWords
                ]
                (fun p -> p.Items)
                (fun (i: Item) -> i.Id)
                SchemaJson.encodeItem
                SchemaJson.decodeItem
                UpsertItem
                RemoveItem
                { Item.Default with
                    Type = "material"
                    Stackable = true
                    MaxStack = 99.0
                }
                noComputed
            entries
                "crops"
                "Crops"
                [
                    col "id" "Id" ColumnKind.Text
                    col "name" "Name" ColumnKind.Text
                    col "seedCost" "Seed cost" ColumnKind.Number
                    col "baseHarvestValue" "Harvest value" ColumnKind.Number
                    col "profit" "Profit per harvest" ColumnKind.Computed
                    col "growthDays" "Growth days" ColumnKind.OptionalNumber
                    col "stages" "Stages" ColumnKind.Number
                    col "seasons" "Seasons" ColumnKind.Words
                    col "canRegrow" "Regrows" ColumnKind.Bool
                    col "regrowthDays" "Regrowth days" ColumnKind.OptionalNumber
                    col "yieldMin" "Yield min" ColumnKind.Number
                    col "yieldMax" "Yield max" ColumnKind.Number
                    col "harvestItemId" "Harvest item" ColumnKind.OptionalText
                    col "tags" "Tags" ColumnKind.OptionalWords
                ]
                (fun p -> defaultArg p.CustomCrops [])
                (fun (c: CustomCropDefinition) -> c.Id)
                SchemaJson.encodeCustomCropDefinition
                SchemaJson.decodeCustomCropDefinition
                UpsertCrop
                RemoveCrop
                { CustomCropDefinition.Default with
                    Stages = 4.0
                    GrowthDays = Some 4.0
                    YieldMin = 1.0
                    YieldMax = 1.0
                }
                (fun _ json key ->
                    match key with
                    | "profit" ->
                        let crop = SchemaJson.decodeCustomCropDefinition [] json
                        money (Readouts.cropProfit (Readouts.cropOfCustom crop))
                    | _ -> "")
            entries
                "recipes"
                "Recipes"
                [
                    col "id" "Id" ColumnKind.Text
                    col "name" "Name" ColumnKind.Text
                    col "category" "Category" ColumnKind.Text
                    col "machineTypeId" "Machine" ColumnKind.OptionalText
                    col "processingMinutes" "Minutes" ColumnKind.Number
                    col "inputs" "Inputs" ColumnKind.Ingredients
                    col "outputs" "Outputs" ColumnKind.Ingredients
                    col "profit" "Profit per craft" ColumnKind.Computed
                    col "profitPerHour" "Profit per machine hour" ColumnKind.Computed
                    col "tags" "Tags" ColumnKind.OptionalWords
                ]
                (fun p -> p.Recipes)
                (fun (r: RecipeDefinition) -> r.Id)
                SchemaJson.encodeRecipeDefinition
                SchemaJson.decodeRecipeDefinition
                UpsertRecipe
                RemoveRecipe
                RecipeDefinition.Default
                (fun project json key ->
                    let recipe = SchemaJson.decodeRecipeDefinition [] json

                    match key with
                    | "profit" -> money (Readouts.recipeProfit project recipe)
                    | "profitPerHour" ->
                        Readouts.recipeProfitPerHour project recipe
                        |> Option.map money
                        |> Option.defaultValue ""
                    | _ -> "")
            children
                "shopStock"
                "Shop stock"
                "shopId"
                "itemId"
                [
                    col "id" "Id" ColumnKind.Text
                    col "shopId" "Shop" ColumnKind.Text
                    col "itemId" "Item" ColumnKind.Text
                    col "price" "Price" ColumnKind.OptionalNumber
                    col "seasons" "Seasons" ColumnKind.OptionalWords
                    col "dailyLimit" "Daily limit" ColumnKind.OptionalNumber
                ]
                (fun p -> p.Shops)
                (fun (s: ShopDefinition) -> s.Id)
                (fun s -> s.Stock)
                (fun s stock -> { s with Stock = stock })
                SchemaJson.encodeShopStockEntry
                SchemaJson.decodeShopStockEntry
                UpsertShop
                ShopStockEntry.Default
            children
                "fishTables"
                "Fish tables"
                "tableId"
                "itemId"
                [
                    col "id" "Id" ColumnKind.Text
                    col "tableId" "Table" ColumnKind.Text
                    col "itemId" "Fish" ColumnKind.Text
                    col "weight" "Weight" ColumnKind.Number
                    col "difficulty" "Difficulty" ColumnKind.Number
                ]
                (fun p -> p.FishTables)
                (fun (t: FishTable) -> t.Id)
                (fun t -> t.Entries)
                (fun t entries -> { t with Entries = entries })
                SchemaJson.encodeFishTableEntry
                SchemaJson.decodeFishTableEntry
                UpsertFishTable
                { FishTableEntry.Default with
                    Weight = 1.0
                }
            children
                "nodeDrops"
                "Node drops"
                "nodeTypeId"
                "itemId"
                [
                    col "id" "Id" ColumnKind.Text
                    col "nodeTypeId" "Node" ColumnKind.Text
                    col "itemId" "Item" ColumnKind.Text
                    col "min" "Min" ColumnKind.Number
                    col "max" "Max" ColumnKind.Number
                    col "weight" "Weight" ColumnKind.Number
                    col "seasons" "Seasons" ColumnKind.OptionalWords
                ]
                (fun p -> p.NodeTypes)
                (fun (n: NodeTypeDefinition) -> n.Id)
                (fun n -> n.Drops)
                (fun n drops -> { n with Drops = drops })
                SchemaJson.encodeNodeDrop
                SchemaJson.decodeNodeDrop
                UpsertNodeType
                { NodeDrop.Default with
                    Min = 1.0
                    Max = 1.0
                    Weight = 1.0
                }
        ]

    /// The categories a table can show: `items`, `crops`, `recipes`, `shopStock`, `fishTables`,
    /// `nodeDrops`.
    let Categories: string list = categories |> List.map (fun c -> c.Id)

    let private find (category: string) =
        match categories |> List.tryFind (fun c -> c.Id = category) with
        | Some c -> c
        | None -> invalidArg "category" (sprintf "%s is not a table (%s)" category (String.Join(", ", Categories)))

    /// The category's name for a heading (`Shop stock`).
    let name (category: string) = (find category).Name

    /// The category's columns, in order.
    let columns (category: string) : TableColumn list = (find category).Columns

    // ── Cells ──────────────────────────────────────────────────────────────

    let private wordsText (json: Json) =
        match json with
        | JArray values -> values |> List.map str |> String.concat ", "
        | _ -> ""

    let private cellOf (category: Category) (project: GameProject) (rowId: string) (json: Json) (column: TableColumn) =
        match column.Kind with
        | ColumnKind.Computed ->
            try
                category.Computed project json column.Key
            with DecodeError _ ->
                ""
        | _ when column.Key = "id" -> rowId
        | _ ->
            match Json.tryGet column.Key json with
            | None
            | Some JNull -> ""
            | Some(JString s) -> s
            | Some(JNumber n) -> JsNumber.format n
            | Some(JBool b) -> if b then "true" else "false"
            | Some(JArray _ as list) when column.Kind = ColumnKind.Ingredients ->
                Json.elements column.Key list
                |> List.map (fun i ->
                    let quantity =
                        match Json.get "quantity" i with
                        | JNumber n -> n
                        | _ -> 0.0

                    sprintf "%s %s" (JsNumber.format quantity) (str (Json.get "itemId" i)))
                |> String.concat ", "
            | Some(JArray _ as list) -> wordsText list
            | Some other -> Json.stringify other

    /// `json` with `text` written into `column`, or why it cannot be.
    let private writeCell (column: TableColumn) (text: string) (json: Json) : Result<Json, string> =
        let text = text.Trim()

        let number () =
            let n = JsNumber.parse text

            if Double.IsNaN n || Double.IsInfinity n then
                Error(sprintf "%s is not a number" text)
            else
                Ok n

        let words () =
            text.Split([| ','; ';' |], StringSplitOptions.RemoveEmptyEntries ||| StringSplitOptions.TrimEntries)
            |> List.ofArray
            |> List.map JString

        let set value = Ok(Json.set column.Key value json)

        match column.Kind with
        | ColumnKind.Computed -> Error "this column is worked out and cannot be written"
        | ColumnKind.Text -> set (JString text)
        | ColumnKind.OptionalText ->
            if text = "" then
                Ok(Json.remove column.Key json)
            else
                set (JString text)
        | ColumnKind.Number ->
            if text = "" then
                Error "needs a number"
            else
                number () |> Result.bind (JNumber >> set)
        | ColumnKind.OptionalNumber ->
            if text = "" then
                Ok(Json.remove column.Key json)
            else
                number () |> Result.bind (JNumber >> set)
        | ColumnKind.Bool ->
            match text.ToLowerInvariant() with
            | "true"
            | "yes"
            | "1" -> set (JBool true)
            | "false"
            | "no"
            | "0" -> set (JBool false)
            | _ -> Error(sprintf "%s is not true or false" text)
        | ColumnKind.Words -> set (JArray(words ()))
        | ColumnKind.OptionalWords ->
            match words () with
            | [] -> Ok(Json.remove column.Key json)
            | list -> set (JArray list)
        | ColumnKind.Ingredients ->
            let parts =
                text.Split([| ','; ';' |], StringSplitOptions.RemoveEmptyEntries ||| StringSplitOptions.TrimEntries)
                |> List.ofArray
                |> List.map (fun part ->
                    let space = part.IndexOf ' '

                    let quantity, item =
                        if space < 0 then
                            "1", part
                        else
                            part.Substring(0, space), part.Substring(space + 1).Trim()

                    let n = JsNumber.parse quantity

                    if Double.IsNaN n || item = "" then
                        Error(sprintf "%s is not QUANTITY ITEM (2 wood)" part)
                    else
                        Ok(JObject [ "itemId", JString item; "quantity", JNumber n ]))

            match
                parts
                |> List.tryPick (function
                    | Error e -> Some e
                    | Ok _ -> None)
            with
            | Some e -> Error e
            | None ->
                set (
                    JArray(
                        parts
                        |> List.choose (function
                            | Ok v -> Some v
                            | Error _ -> None)
                    )
                )

    /// The table of `category`: a row per entry, cells in column order.
    let rows (category: string) (project: GameProject) : TableRow list =
        let c = find category

        c.Rows project
        |> List.map (fun (rowId, json) ->
            {
                Id = rowId
                Cells = c.Columns |> List.map (cellOf c project rowId json)
            })

    /// The rows whose cells contain `query` (any column, ignoring case), or that carry tag `tag`
    /// (empty: any).
    let filter (query: string) (rows: TableRow list) : TableRow list =
        if String.IsNullOrWhiteSpace query then
            rows
        else
            rows
            |> List.filter (fun row ->
                row.Cells
                |> List.exists (fun cell -> cell.Contains(query.Trim(), StringComparison.OrdinalIgnoreCase)))

    // ── Writing ────────────────────────────────────────────────────────────

    /// Cell writes `(row id, column key, text)` on the current rows: the changed rows, the
    /// cells that cannot be written (rows numbered by `rowNumber`).
    let private writeCells
        (c: Category)
        (project: GameProject)
        (rowNumber: string -> int)
        (writes: (string * string * string) list)
        : (string * Json) list * CellError list =
        let current = c.Rows project
        let rows = Dictionary<string, Json>()

        for rowId, json in current do
            rows.[rowId] <- json

        let errors = ResizeArray<CellError>()

        for rowId, key, text in writes do
            match c.Columns |> List.tryFind (fun col -> col.Key = key), rows.TryGetValue rowId with
            | None, _ ->
                errors.Add
                    {
                        Row = rowNumber rowId
                        Column = key
                        Message = "there is no such column"
                    }
            | _, (false, _) ->
                errors.Add
                    {
                        Row = rowNumber rowId
                        Column = key
                        Message = sprintf "there is no row %s" rowId
                    }
            | Some column, (true, json) when column.ReadOnly ->
                if cellOf c project rowId json column <> text.Trim() then
                    errors.Add
                        {
                            Row = rowNumber rowId
                            Column = column.Header
                            Message = "this column cannot be changed here"
                        }
            | Some column, (true, json) ->
                match writeCell column text json with
                | Ok next -> rows.[rowId] <- next
                | Error message ->
                    errors.Add
                        {
                            Row = rowNumber rowId
                            Column = column.Header
                            Message = message
                        }

        current |> List.map (fun (rowId, _) -> rowId, rows.[rowId]), List.ofSeq errors

    let private toEdit (c: Category) (project: GameProject) (label: string) (rows: (string * Json) list) (rowNumber: string -> int) =
        match c.Write project rows with
        | Ok edits -> Ok(Batch(label, edits))
        | Error(rowId, message) ->
            Error
                [
                    {
                        Row = rowNumber rowId
                        Column = ""
                        Message = message
                    }
                ]

    /// The edit that writes the cells `(row id, column key, text)`: one inline edit, or a paste
    /// into many rows; one undo step. Every cell is checked first, so nothing is written when one
    /// cannot be.
    let setCells (category: string) (project: GameProject) (writes: (string * string * string) list) : Result<Edit, CellError list> =
        let c = find category
        let order = c.Rows project |> List.mapi (fun i (rowId, _) -> rowId, i + 1) |> dict

        let rowNumber rowId =
            match order.TryGetValue rowId with
            | true, n -> n
            | _ -> 0

        match writeCells c project rowNumber writes with
        | rows, [] -> toEdit c project (sprintf "Edit %s" (c.Name.ToLowerInvariant())) rows rowNumber
        | _, errors -> Error errors

    /// A block of text pasted at a cell (a spreadsheet's tab-separated rows): it fills `rowIds`
    /// from the first, and the columns from `columnKey` onwards. One undo step.
    let paste (category: string) (project: GameProject) (rowIds: string list) (columnKey: string) (text: string) : Result<Edit, CellError list> =
        let c = find category
        let keys = c.Columns |> List.map (fun col -> col.Key)

        match List.tryFindIndex ((=) columnKey) keys with
        | None ->
            Error
                [
                    {
                        Row = 0
                        Column = columnKey
                        Message = "there is no such column"
                    }
                ]
        | Some start ->
            let lines =
                text.Replace("\r\n", "\n").TrimEnd('\n').Split '\n'
                |> Array.map (fun line -> line.Split '\t')

            let writes =
                [
                    for r, line in Array.indexed lines do
                        if r < rowIds.Length then
                            for k, cell in Array.indexed line do
                                if start + k < keys.Length then
                                    yield rowIds.[r], keys.[start + k], cell
                ]

            setCells category project writes

    // ── CSV ────────────────────────────────────────────────────────────────

    let private csvField (text: string) = Csv.field text

    /// CSV text (RFC 4180) as rows of fields.
    let parseCsv (text: string) : Result<string list list, string> = Csv.parse text

    /// The category's table as CSV: a header of column keys, then a row per entry (computed
    /// columns included; an import ignores them).
    let toCsv (category: string) (project: GameProject) : string =
        let c = find category
        let sb = StringBuilder()

        sb.Append(String.Join(",", c.Columns |> List.map (fun col -> csvField col.Key))).Append("\r\n")
        |> ignore

        for row in rows category project do
            sb.Append(String.Join(",", row.Cells |> List.map csvField)).Append("\r\n")
            |> ignore

        sb.ToString()

    /// What importing `csv` into `category` would change (matched by the `id` column; a row
    /// without an id is new). Rows the file lacks are listed in `Removed` and only removed when
    /// `applyImport` is asked to.
    let diff (category: string) (project: GameProject) (csv: string) : TableDiff =
        let c = find category

        let empty =
            {
                Category = category
                Added = []
                Changed = []
                Removed = []
                Invalid = []
                Rows = []
            }

        match parseCsv csv with
        | Error message ->
            { empty with
                Invalid =
                    [
                        {
                            Row = 0
                            Column = ""
                            Message = message
                        }
                    ]
            }
        | Ok [] ->
            { empty with
                Invalid =
                    [
                        {
                            Row = 1
                            Column = ""
                            Message = "the file is empty"
                        }
                    ]
            }
        | Ok(header :: lines) ->
            let errors = ResizeArray<CellError>()

            let header =
                header
                |> List.map (fun key ->
                    let key = key.Trim()

                    match c.Columns |> List.tryFind (fun col -> col.Key = key || col.Header = key) with
                    | Some col -> Some col
                    | None ->
                        errors.Add
                            {
                                Row = 1
                                Column = key
                                Message = "there is no such column"
                            }

                        None)

            let idIndex =
                header
                |> List.tryFindIndex (fun col -> col |> Option.exists (fun col -> col.Key = "id"))

            let current = c.Rows project
            let currentById = current |> dict
            let result = ResizeArray<string * Json>()
            let seen = HashSet<string>()
            let added = ResizeArray<RowChange>()
            let changed = ResizeArray<RowChange>()

            lines
            |> List.iteri (fun index fields ->
                let rowNumber = index + 2
                let fields = fields @ List.replicate (max 0 (header.Length - fields.Length)) ""

                let given =
                    idIndex |> Option.map (fun i -> fields.[i].Trim()) |> Option.defaultValue ""

                let existing =
                    if given <> "" && currentById.ContainsKey given then
                        Some currentById.[given]
                    else
                        None

                let mutable json =
                    match existing with
                    | Some json -> json
                    | None -> c.Blank

                let mutable ok = true

                for k, col in List.indexed header do
                    match col with
                    | Some col when
                        not col.ReadOnly
                        || (existing.IsNone && col.Key <> "id" && col.Kind <> ColumnKind.Computed)
                        ->
                        match writeCell col fields.[k] json with
                        | Ok next -> json <- next
                        | Error message ->
                            ok <- false

                            errors.Add
                                {
                                    Row = rowNumber
                                    Column = col.Header
                                    Message = message
                                }
                    | _ -> ()

                let rowId =
                    match existing with
                    | Some _ -> given
                    | None ->
                        // A new entry takes the id it was given; a new child row is `parent/item`.
                        if c.Children then
                            // Another row for the same parent and item: `#2`, `#3`…
                            let baseId = c.IdOf json
                            let mutable candidate = baseId
                            let mutable n = 1

                            while seen.Contains candidate || currentById.ContainsKey candidate do
                                n <- n + 1
                                candidate <- sprintf "%s#%d" baseId n

                            candidate
                        else
                            json <- Json.set "id" (JString given) json
                            given

                if ok then
                    if rowId = "" || rowId.EndsWith "/" then
                        errors.Add
                            {
                                Row = rowNumber
                                Column = "id"
                                Message = "a new row needs an id"
                            }
                    elif not (seen.Add rowId) then
                        errors.Add
                            {
                                Row = rowNumber
                                Column = "id"
                                Message = sprintf "%s comes twice" rowId
                            }
                    else
                        let cells (j: Json) =
                            c.Columns |> List.map (fun col -> col, cellOf c project rowId j col)

                        match existing with
                        | Some before ->
                            let changes =
                                List.zip (cells before) (cells json)
                                |> List.choose (fun ((col, b), (_, a)) ->
                                    if b <> a && col.Kind <> ColumnKind.Computed then
                                        Some
                                            {
                                                Column = col.Header
                                                Before = b
                                                After = a
                                            }
                                    else
                                        None)

                            if not changes.IsEmpty then
                                changed.Add { Id = rowId; Changes = changes }
                        | None ->
                            added.Add
                                {
                                    Id = rowId
                                    Changes =
                                        cells json
                                        |> List.filter (fun (col, a) -> a <> "" && col.Kind <> ColumnKind.Computed)
                                        |> List.map (fun (col, a) ->
                                            {
                                                Column = col.Header
                                                Before = ""
                                                After = a
                                            })
                                }

                        result.Add(rowId, json))

            // Check the rows decode before showing the diff as applicable.
            match c.Write project (List.ofSeq result) with
            | Error(rowId, message) ->
                let line =
                    result |> Seq.tryFindIndex (fun (r, _) -> r = rowId) |> Option.defaultValue -1

                errors.Add
                    {
                        Row = line + 2
                        Column = ""
                        Message = message
                    }
            | Ok _ -> ()

            {
                Category = category
                Added = List.ofSeq added
                Changed = List.ofSeq changed
                Removed = current |> List.map fst |> List.filter (fun r -> not (seen.Contains r))
                Invalid = List.ofSeq errors
                Rows = List.ofSeq result
            }

    /// The edit an import makes (one undo step): its added and changed rows, and — when
    /// `removeMissing` — no longer the rows the file lacks. Refused while the diff has invalid
    /// cells.
    let applyImport (project: GameProject) (diff: TableDiff) (removeMissing: bool) : Result<Edit, CellError list> =
        if not diff.Invalid.IsEmpty then
            Error diff.Invalid
        else
            let c = find diff.Category

            let rows =
                if removeMissing then
                    diff.Rows
                else
                    // Rows the file lacks stay where they were; new rows go at the end.
                    let byId = diff.Rows |> dict

                    (c.Rows project
                     |> List.map (fun (rowId, json) ->
                         match byId.TryGetValue rowId with
                         | true, next -> rowId, next
                         | _ -> rowId, json))
                    @ (diff.Rows
                       |> List.filter (fun (rowId, _) -> not ((c.Rows project) |> List.exists (fun (r, _) -> r = rowId))))

            toEdit c project (sprintf "Import %s" (c.Name.ToLowerInvariant())) rows (fun _ -> 0)

    /// The row json of every row (bulk edits and templates read it).
    let internal rowJson (category: string) (project: GameProject) : (string * Json) list = (find category).Rows project

    /// The edit that makes the category hold `rows` (bulk edits and templates write through it).
    let internal writeRows (category: string) (project: GameProject) (label: string) (rows: (string * Json) list) =
        let c = find category
        let order = rows |> List.mapi (fun i (rowId, _) -> rowId, i + 1) |> dict

        toEdit c project label rows (fun rowId ->
            match order.TryGetValue rowId with
            | true, n -> n
            | _ -> 0)

    /// A cell's text from a row's JSON.
    let internal cellText (category: string) (project: GameProject) (rowId: string) (json: Json) (column: TableColumn) =
        cellOf (find category) project rowId json column

    /// `json` with `text` written into `column`.
    let internal write (column: TableColumn) (text: string) (json: Json) = writeCell column text json

    /// A blank entry of the category.
    let internal blank (category: string) = (find category).Blank

    /// The id of a row from its JSON.
    let internal idOf (category: string) (json: Json) = (find category).IdOf json
