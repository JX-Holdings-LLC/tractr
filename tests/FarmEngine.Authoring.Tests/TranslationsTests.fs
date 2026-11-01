/// #228: the game's own text in other languages.
module FarmEngine.Authoring.Tests.TranslationsTests

open Xunit
open FarmEngine.Authoring
open FarmEngine.Authoring.Tests.TestProjects
open FarmEngine.Schemas

let private withSpanish () =
    let project = starter ()
    let project = Document.run project (Translations.addLocale project "es")
    let item = List.head project.Items

    let project =
        Document.run project (Translations.setText project "es" (sprintf "item:%s:name" item.Id) "Cosa")

    project, item

let private codes (project: GameProject) =
    Problems.collect project
    |> List.filter (fun p -> p.Code.StartsWith "translation.")
    |> List.map (fun p -> p.Code)

[<Fact>]
let ``the catalog has a stable key and context for every text`` () =
    let project = starter ()
    let fields = Translations.catalog project
    Assert.NotEmpty fields
    Assert.Equal(fields.Length, fields |> List.map (fun f -> f.Key) |> List.distinct |> List.length)
    let item = List.head project.Items
    Assert.Contains(fields, fun f -> f.Key = sprintf "item:%s:name" item.Id && f.Source = item.Name)
    let npc = List.head project.Npcs
    Assert.Contains(fields, fun f -> f.Key = sprintf "npc:%s:name" npc.Id && f.Context <> "")

    for dialogue in npc.Dialogue do
        if dialogue.Text <> "" then
            Assert.Contains(fields, fun f -> f.Key = sprintf "dialogue:%s:text" dialogue.Id && f.Context.Contains npc.Name)

[<Fact>]
let ``translations report missing stale font and substitution problems`` () =
    let project, item = withSpanish ()
    Assert.Equal<string list>([ "translation.missing" ], codes project)

    // The source changes after translating: stale.
    let renamed =
        Document.run project (UpsertItem { item with Name = item.Name + " II" })

    Assert.Contains("translation.stale", codes renamed)

    // A glyph the font lacks, a dropped substitution.
    let mail =
        { MailDefinition.Default with
            Id = "hello"
            Subject = "Hi {player}"
            Text = "Text"
        }

    let project = { project with Mail = Some [ mail ] }

    let project =
        Document.run project (Translations.setText project "es" "mail:hello:subject" "Hola ☃")

    let found = codes project
    Assert.Contains("translation.fontCoverage", found)
    Assert.Contains("translation.substitution", found)
    Assert.Equal<string list>([ "{player}" ], Translations.placeholders "Hi {player}, {nothing}")

[<Fact>]
let ``csv and po files go out and come back`` () =
    let project, item = withSpanish ()
    let key = sprintf "item:%s:name" item.Id

    let csv = Translations.exportCsv project "es"
    Assert.StartsWith("key,context,source,translation,status", csv)
    Assert.Contains(key, csv)
    let back = Translations.importCsv project "es" csv
    Assert.Empty back.Errors
    Assert.Equal(1, back.Unchanged)
    Assert.Equal(0, back.Added)

    let po = Translations.exportPo project "es"
    Assert.Contains(sprintf "msgctxt \"%s\"" key, po)
    Assert.Contains("msgstr \"Cosa\"", po)
    let edited = po.Replace("msgstr \"Cosa\"", "msgstr \"Objeto\"")
    let import = Translations.importPo project "es" edited
    Assert.Empty import.Errors
    Assert.Equal(1, import.Changed)
    let applied = Document.run project (Translations.applyImport project import)
    let table = (defaultArg applied.Translations []) |> List.exactlyOne
    Assert.Equal("Objeto", (table.Strings |> List.find (fun (k, _) -> k = key) |> snd).Text)

    let unknown =
        Translations.importCsv project "es" "key,translation\r\nnope:x:name,Hola\r\n"

    Assert.Equal<string list>([ "nope:x:name" ], unknown.Unknown)

[<Fact>]
let ``the cartridge carries every language and the compiler applies none`` () =
    let project, item = withSpanish ()
    let section = CartridgeCompiler.PresentationSection project
    let strings = Json.get "contentStrings" section |> Json.get "es"
    Assert.Equal(JString "Cosa", Json.get (sprintf "item:%s:name" item.Id) strings)
    // The content itself keeps the source text.
    let content = ContentCompiler.compile project
    Assert.Equal(item.Name, (content.Items |> List.find (fun i -> i.Id = item.Id)).Name)
    // No translations: no section key, so existing cartridges are unchanged.
    Assert.Equal(None, Json.tryGet "contentStrings" (CartridgeCompiler.PresentationSection(starter ())))

[<Fact>]
let ``assigning ids keeps translations made under an index`` () =
    let project = starter ()

    let npc =
        project.Npcs
        |> List.find (fun n -> not (defaultArg n.Lines []).IsEmpty || not n.Dialogue.IsEmpty)

    let npc =
        { npc with
            Lines =
                Some
                    [
                        { NpcLine.Default with
                            Text = "Hello there."
                        }
                    ]
        }

    let project = Document.run project (UpsertNpc npc)
    let project = Document.run project (Translations.addLocale project "es")
    let indexKey = sprintf "npc:%s:line:#0" npc.Id

    let project =
        Document.run project (Translations.setText project "es" indexKey "Hola.")

    let project = Document.run project (Translations.assignIds project)

    let line =
        (project.Npcs |> List.find (fun n -> n.Id = npc.Id)).Lines.Value
        |> List.exactlyOne

    Assert.True line.Id.IsSome
    let table = (defaultArg project.Translations []) |> List.exactlyOne
    Assert.Equal<string list>([ sprintf "npc:%s:line:%s" npc.Id line.Id.Value ], table.Strings |> List.map fst)

    Assert.Equal(
        "done",
        Translations.status
            project
            "es"
            (Translations.catalog project
             |> List.find (fun f -> f.Key = fst table.Strings.Head))
    )

[<Fact>]
let ``a project folder keeps one strings file per language`` () =
    let project, _ = withSpanish ()
    let files = ProjectFolder.toFiles project
    Assert.Contains(files, fun (path, _) -> path = "strings/es.json")
    let folder = System.Collections.Generic.Dictionary<string, byte[]>(dict files)

    match ProjectFolder.load folder with
    | { Data = Some loaded } -> Assert.Equal<TranslationTable list option>(project.Translations, loaded.Translations)
    | other -> failwithf "%A" other.Errors
