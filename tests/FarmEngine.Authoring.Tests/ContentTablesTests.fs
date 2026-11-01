/// #226 and #227: content tables, CSV import with a diff, bulk edits, tags and templates.
module FarmEngine.Authoring.Tests.ContentTablesTests

open Xunit
open FarmEngine.Authoring
open FarmEngine.Authoring.Tests.TestProjects
open FarmEngine.Schemas

let private ok (result: Result<'T, CellError list>) =
    match result with
    | Ok value -> value
    | Error errors -> failwithf "%A" errors

let private cell (category: string) (project: GameProject) (rowId: string) (key: string) =
    let columns = ContentTables.columns category
    let index = columns |> List.findIndex (fun c -> c.Key = key)
    (ContentTables.rows category project |> List.find (fun r -> r.Id = rowId)).Cells.[index]

[<Fact>]
let ``every category has a table of the template's content`` () =
    let project = starter ()

    for category in ContentTables.Categories do
        let columns = ContentTables.columns category
        Assert.Equal("id", (List.head columns).Key)

        for row in ContentTables.rows category project do
            Assert.Equal(columns.Length, row.Cells.Length)
            Assert.Equal(row.Id, List.head row.Cells)

[<Fact>]
let ``a table written back unchanged changes nothing`` () =
    for _, project in templates () do
        for category in ContentTables.Categories do
            let diff =
                ContentTables.diff category project (ContentTables.toCsv category project)

            Assert.True(diff.Invalid.IsEmpty, sprintf "%s: %A" category diff.Invalid)
            Assert.False(diff.HasChanges, sprintf "%s: %A %A" category diff.Added diff.Changed)
            let edit = ok (ContentTables.applyImport project diff true)
            Assert.Same(project, Document.run project edit)

[<Fact>]
let ``computed columns show crop and recipe profit`` () =
    let project = starter ()

    for crop in defaultArg project.CustomCrops [] do
        let expected = JsNumber.format (crop.BaseHarvestValue - crop.SeedCost)
        Assert.Equal(expected, cell "crops" project crop.Id "profit")

    for recipe in project.Recipes do
        Assert.Equal(JsNumber.format (JsNumber.roundHalfAway (Readouts.recipeProfit project recipe)), cell "recipes" project recipe.Id "profit")

[<Fact>]
let ``inline edits and pastes are one undo step and checked first`` () =
    let project = starter ()

    let items =
        ContentTables.rows "items" project
        |> List.map (fun r -> r.Id)
        |> List.truncate 3

    let edit = ok (ContentTables.paste "items" project items "value" "11\n12\n13")
    let document = Document.apply edit (Document.create project)
    Assert.Equal(1, document.Past.Length)

    for id, value in List.zip items [ "11"; "12"; "13" ] do
        Assert.Equal(value, cell "items" document.Project id "value")

    match ContentTables.setCells "items" project [ items.[0], "value", "lots" ] with
    | Error [ e ] ->
        Assert.Equal("Value", e.Column)
        Assert.Equal(1, e.Row)
    | other -> failwithf "%A" other

[<Fact>]
let ``an import shows what it adds changes and removes before it applies`` () =
    let project = starter ()
    let item = project.Items |> List.head

    let csv =
        "id,name,value,stackable\r\n"
        + sprintf "%s,Renamed,77,true\r\n" item.Id
        + "new-thing,New thing,5,yes\r\n"
        + "\"bad, one\",Bad,cheap,true\r\n"

    let diff = ContentTables.diff "items" project csv

    Assert.Equal<CellError list>(
        [
            {
                Row = 4
                Column = "Value"
                Message = "cheap is not a number"
            }
        ],
        diff.Invalid
    )

    let diff = ContentTables.diff "items" project (csv.Replace("cheap", "3"))
    Assert.True(diff.Invalid.IsEmpty, sprintf "%A" diff.Invalid)
    Assert.Equal<string list>([ "new-thing"; "bad, one" ], diff.Added |> List.map (fun r -> r.Id))
    let change = diff.Changed |> List.exactlyOne
    Assert.Equal(item.Id, change.Id)
    Assert.Contains(change.Changes, fun c -> c.Column = "Value" && c.After = "77")
    Assert.Equal(project.Items.Length - 1, diff.Removed.Length)

    let kept = Document.run project (ok (ContentTables.applyImport project diff false))
    Assert.Equal(project.Items.Length + 2, kept.Items.Length)
    Assert.Equal("Renamed", (kept.Items |> List.find (fun i -> i.Id = item.Id)).Name)

    let replaced =
        Document.run project (ok (ContentTables.applyImport project diff true))

    Assert.Equal<string list>([ item.Id; "new-thing"; "bad, one" ], replaced.Items |> List.map (fun i -> i.Id))

[<Fact>]
let ``shop stock rows are matched by shop and item`` () =
    let project = starter ()
    let shop = project.Shops |> List.find (fun s -> not s.Stock.IsEmpty)
    let stock = List.head shop.Stock
    let rowId = shop.Id + "/" + stock.ItemId
    let edit = ok (ContentTables.setCells "shopStock" project [ rowId, "price", "999" ])
    let changed = Document.run project edit

    let after =
        (changed.Shops |> List.find (fun s -> s.Id = shop.Id)).Stock |> List.head

    Assert.Equal(Some 999.0, after.Price)
    Assert.Equal(shop.Stock.Length, (changed.Shops |> List.find (fun s -> s.Id = shop.Id)).Stock.Length)

    let csv =
        ContentTables.toCsv "shopStock" project
        + sprintf ",%s,%s,5,,\r\n" shop.Id (List.head project.Items).Id

    let diff = ContentTables.diff "shopStock" project csv
    Assert.True(diff.Invalid.IsEmpty, sprintf "%A" diff.Invalid)
    Assert.Equal(1, diff.Added.Length)
    let grown = Document.run project (ok (ContentTables.applyImport project diff true))
    Assert.Equal(shop.Stock.Length + 1, (grown.Shops |> List.find (fun s -> s.Id = shop.Id)).Stock.Length)

[<Fact>]
let ``csv fields with commas quotes and line breaks read back`` () =
    match ContentTables.parseCsv "a,\"b, c\",\"say \"\"hi\"\"\"\r\n\"two\nlines\",x,\r\n" with
    | Ok rows -> Assert.Equal<string list list>([ [ "a"; "b, c"; "say \"hi\"" ]; [ "two\nlines"; "x"; "" ] ], rows)
    | Error e -> failwith e

// ── Bulk edits ────────────────────────────────────────────────────────────

[<Fact>]
let ``bulk edits set and adjust a field across the selection in one step`` () =
    let project = starter ()
    let ids = project.Items |> List.map (fun i -> i.Id) |> List.truncate 2

    let times =
        ok (Result.mapError (fun e -> [ { Row = 0; Column = ""; Message = e } ]) (BulkEdit.parseOperation "× 2"))

    let preview = BulkEdit.preview "items" project ids "value" times
    Assert.True(preview.Invalid.IsEmpty)

    let document =
        Document.apply (ok (BulkEdit.apply "items" project preview)) (Document.create project)

    Assert.Equal(1, document.Past.Length)

    for id in ids do
        let before = (project.Items |> List.find (fun i -> i.Id = id)).Value
        Assert.Equal(before * 2.0, (document.Project.Items |> List.find (fun i -> i.Id = id)).Value)

    Assert.Equal(Ok(BulkOperation.Set "50"), BulkEdit.parseOperation "= 50")
    Assert.Equal(Ok(BulkOperation.Add 1.0), BulkEdit.parseOperation "+ 1")
    Assert.Equal(Ok(BulkOperation.Add -2.0), BulkEdit.parseOperation "- 2")
    Assert.Equal(Ok(BulkOperation.AddWord "summer"), BulkEdit.parseOperation "+ summer")

[<Fact>]
let ``tags select entries for a bulk edit`` () =
    let project = starter ()
    let crops = defaultArg project.CustomCrops [] |> List.map (fun c -> c.Id)
    Assert.NotEmpty crops

    let tagged =
        Document.run project (ok (BulkEdit.tag "crops" project [ List.head crops ] "summer-crop" false))

    Assert.Equal<string list>([ "summer-crop" ], BulkEdit.tags "crops" tagged)

    let selected =
        BulkEdit.select
            "crops"
            tagged
            { BulkSelection.All with
                Tags = [ "summer-crop" ]
            }

    Assert.Equal<string list>([ List.head crops ], selected)

    let preview =
        BulkEdit.preview "crops" tagged selected "seasons" (BulkOperation.AddWord "fall")

    let changed = Document.run tagged (ok (BulkEdit.apply "crops" tagged preview))

    let crop =
        (defaultArg changed.CustomCrops [])
        |> List.find (fun c -> c.Id = List.head crops)

    Assert.Contains("fall", crop.Seasons)
    // A list operation on a number is refused with the entry named.
    let wrong =
        BulkEdit.preview "crops" tagged selected "seedCost" (BulkOperation.AddWord "x")

    Assert.NotEmpty wrong.Invalid

// ── Templates ─────────────────────────────────────────────────────────────

[<Fact>]
let ``a crop group template makes new groups with their ids linked`` () =
    let project = starter ()
    let harvest = (List.head project.Items).Id

    let crop =
        { (defaultArg project.CustomCrops [] |> List.head) with
            HarvestItemId = Some harvest
        }

    let project = Document.run project (UpsertCrop crop)

    let project =
        Document.run
            project
            (ContentTemplates.save project "crop-group" "Crop group" [ "crops", crop.Id; "items", harvest ]
             |> function
                 | Ok e -> e
                 | Error e -> failwith e)

    let template = (defaultArg project.ContentTemplates []) |> List.exactlyOne
    let columns = ContentTemplates.columns template |> List.map fst
    Assert.Contains("1.id", columns)
    Assert.Contains("2.name", columns)

    let edit =
        ok (
            ContentTemplates.make
                project
                template
                [
                    [
                        "1.id", "moonberry"
                        "1.name", "Moonberry"
                        "2.id", "moonberry-fruit"
                        "2.name", "Moonberry"
                        "2.value", "80"
                    ]
                    [ "1.id", "sunberry"; "2.id", "sunberry-fruit"; "1.seedCost", "12" ]
                ]
        )

    let document = Document.apply edit (Document.create project)
    Assert.Equal(1, document.Past.Length)
    let made = defaultArg document.Project.CustomCrops []
    let moonberry = made |> List.find (fun c -> c.Id = "moonberry")
    Assert.Equal(Some "moonberry-fruit", moonberry.HarvestItemId)
    Assert.Equal(12.0, (made |> List.find (fun c -> c.Id = "sunberry")).SeedCost)
    Assert.Equal(Some "sunberry-fruit", (made |> List.find (fun c -> c.Id = "sunberry")).HarvestItemId)
    Assert.Equal(80.0, (document.Project.Items |> List.find (fun i -> i.Id = "moonberry-fruit")).Value)

    match ContentTemplates.make project template [ [ "1.id", crop.Id ] ] with
    | Error errors -> Assert.Contains(errors, fun e -> e.Message.Contains "taken")
    | Ok _ -> failwith "expected the taken id to be refused"
