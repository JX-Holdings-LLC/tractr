// Regenerates schemas/project.schema.json (a JSON Schema, draft 2020-12, of a single-file project)
// from src/FarmEngine.Authoring/Schema.fs and SchemaConstants.fs:
//   dotnet fsi tools/codegen/project-schema.fsx
// With --check it writes nothing and exits 1 when the file is not what it would generate
// (tools/codegen/check.sh, CI).
//
// Like schema-json.fsx it loads the sources (no build needed) and reflects over the records:
//   - one `$defs` entry per record reachable from `GameProject`, its properties in field order,
//     keyed like SchemaJson.fs (camelCase, or the field's `[<JsonKey>]`, e.g. `targetNPCId`);
//   - descriptions are the `///` doc comments of the records and fields in Schema.fs;
//   - a field documented "One of `X`" takes `X.All` from SchemaConstants.fs: an `enum` when loading
//     refuses other values (SchemaChecks.fs checks `X.All`), else `anyOf` the `enum` and any string
//     (tile types, crop qualities and tools can be extended by the project);
//   - a primitive field's `default` is its value in the record's `Default` (what a missing key
//     decodes to), so no key is required, except `schemaVersion` and a union case's `type`;
//   - `option` may be absent; `[<JsonNull>]` and `'T option option` also allow `null`;
//   - a record with an `Extra` field keeps undeclared keys (`additionalProperties: true`); any
//     other record drops them, so the schema refuses them (a typo shows in an editor);
//   - a union whose cases each carry one record is `oneOf` the cases, each with its `type` const.
#load "../../src/FarmEngine.Authoring/Json.fs"
#load "../../src/FarmEngine.Authoring/Schema.fs"
#load "../../src/FarmEngine.Authoring/SchemaConstants.fs"

open System
open System.Collections.Generic
open System.Reflection
open System.Text.RegularExpressions
open Microsoft.FSharp.Reflection
open FarmEngine.Authoring
open FarmEngine.Schemas

let root = IO.Path.Combine(__SOURCE_DIRECTORY__, "..", "..")

// ── Doc comments ─────────────────────────────────────────────────────────────

let typeDocs = Dictionary<string, string>()
let fieldDocs = Dictionary<string * string, string>()

do
    let lines =
        IO.File.ReadAllLines(IO.Path.Combine(root, "src", "FarmEngine.Authoring", "Schema.fs"))

    let typeLine = Regex(@"^(?:type|and)\s+(?:private\s+)?(\w+)")
    let fieldLine = Regex(@"^ {8}(\w+)\s*:")
    let docLine = Regex(@"^\s*///\s?(.*)$")
    let attributeLine = Regex(@"^\s*\[<.*>\]\s*$")
    let pending = ResizeArray<string>()
    let mutable current = ""

    let take () =
        let text = String.Join(" ", pending).Trim()
        pending.Clear()
        text

    for line in lines do
        let doc = docLine.Match line

        if doc.Success then
            pending.Add(doc.Groups.[1].Value.Trim())
        elif attributeLine.IsMatch line then
            ()
        else
            let t = typeLine.Match line
            let f = fieldLine.Match line

            if t.Success then
                current <- t.Groups.[1].Value
                let text = take ()

                if text.Length > 0 then
                    typeDocs.[current] <- text
            elif f.Success && current.Length > 0 then
                let text = take ()

                if text.Length > 0 then
                    fieldDocs.[(current, f.Groups.[1].Value)] <- text
            else
                pending.Clear()

// ── Enums from SchemaConstants.fs ────────────────────────────────────────────

// fsi compiles each #load into its own assembly, so look in all of them.
let constantTypes =
    AppDomain.CurrentDomain.GetAssemblies()
    |> Array.collect (fun a ->
        try
            a.GetTypes()
        with _ ->
            [||])
    |> Array.filter (fun t ->
        not (isNull t.Namespace)
        && t.Namespace.EndsWith "FarmEngine.Schemas"
        && t.IsAbstract
        && t.IsSealed)

/// `X.All` for the constant module `X`, if it has one.
let enumValues (name: string) : string list option =
    constantTypes
    |> Array.tryFind (fun t -> t.Name = name)
    |> Option.bind (fun t ->
        match t.GetProperty("All", BindingFlags.Public ||| BindingFlags.Static) with
        | null -> None
        | p ->
            match p.GetValue null with
            | :? (string list) as values -> Some values
            | _ -> None)

let oneOfPattern = Regex(@"One of `(\w+)`")

/// The vocabularies loading refuses other values of (SchemaChecks.fs checks `X.All`): a strict
/// `enum`. The others (tile types, crop qualities, tools, …) a project can extend.
let strictEnums =
    let text =
        IO.File.ReadAllText(IO.Path.Combine(root, "src", "FarmEngine.Authoring", "SchemaChecks.fs"))

    // Outcomes are shared with content packs, whose outcome types loading does not check.
    let open' = set [ "EventOutcomeTypes" ]

    Regex.Matches(text, @"\b([A-Z]\w*)\.All\b")
    |> Seq.map (fun m -> m.Groups.[1].Value)
    |> Seq.filter (fun name -> not (open'.Contains name))
    |> Set.ofSeq

// ── Types ────────────────────────────────────────────────────────────────────

let isGen (d: Type) (t: Type) =
    t.IsGenericType && t.GetGenericTypeDefinition() = d

let isOption = isGen typedefof<option<_>>
let isList = isGen typedefof<list<_>>
let arg (t: Type) = t.GetGenericArguments().[0]

let dictValue (t: Type) =
    if isList t && FSharpType.IsTuple(arg t) then
        match FSharpType.GetTupleElements(arg t) with
        | [| k; v |] when k = typeof<string> -> Some v
        | _ -> None
    else
        None

let isSchemaUnion (t: Type) =
    FSharpType.IsUnion t
    && t.Namespace = typeof<GameProject>.Namespace
    && not t.IsNested

let attribute<'A when 'A :> Attribute> (p: PropertyInfo) : 'A option =
    match Attribute.GetCustomAttribute(p, typeof<'A>) with
    | null -> None
    | a -> Some(a :?> 'A)

let jsonKey (p: PropertyInfo) =
    match attribute<JsonKeyAttribute> p with
    | Some a -> a.Key
    | None -> string (Char.ToLowerInvariant p.Name.[0]) + p.Name.Substring 1

/// Each union case: (payload record, `type` tag).
let unionCases (u: Type) =
    [
        for case in FSharpType.GetUnionCases u ->
            match case.GetFields() with
            | [| field |] when FSharpType.IsRecord field.PropertyType ->
                let payload = field.PropertyType

                let instance =
                    FSharpValue.MakeUnion(case, [| payload.GetProperty("Default").GetValue(null) |])

                payload, (u.GetProperty("Type").GetValue(instance) :?> string)
            | _ -> failwithf "%s.%s: each case of a schema union carries one record" u.Name case.Name
    ]

let tags = Dictionary<Type, string>()
let pendingDefs = Queue<Type>()
let seen = HashSet<Type>()

let reference (t: Type) =
    if seen.Add t then
        pendingDefs.Enqueue t

    JObject [ "$ref", JString("#/$defs/" + t.Name) ]

let rec schemaOf (t: Type) : Json =
    if t = typeof<string> then
        JObject [ "type", JString "string" ]
    elif t = typeof<float> then
        JObject [ "type", JString "number" ]
    elif t = typeof<bool> then
        JObject [ "type", JString "boolean" ]
    elif t = typeof<int> || t = typeof<uint32> then
        JObject [ "type", JString "integer" ]
    elif t = typeof<Json> then
        JObject []
    elif FSharpType.IsRecord t || isSchemaUnion t then
        reference t
    else
        match dictValue t with
        | Some v -> JObject [ "type", JString "object"; "additionalProperties", schemaOf v ]
        | None when isList t -> JObject [ "type", JString "array"; "items", schemaOf (arg t) ]
        | None when isOption t -> schemaOf (arg t)
        | None -> failwithf "no schema for %s" t.FullName

let nullable (schema: Json) =
    JObject [ "anyOf", JArray [ schema; JObject [ "type", JString "null" ] ] ]

let defaultOf (t: Type) (value: obj) : Json option =
    if t = typeof<string> then
        Some(JString(value :?> string))
    elif t = typeof<float> then
        Some(JNumber(value :?> float))
    elif t = typeof<bool> then
        Some(JBool(value :?> bool))
    elif t = typeof<int> then
        Some(JNumber(float (value :?> int)))
    elif t = typeof<uint32> then
        Some(JNumber(float (value :?> uint32)))
    else
        None

let withDescription (text: string option) (members: (string * Json) list) =
    match text with
    | Some text when text.Length > 0 -> ("description", JString text) :: members
    | _ -> members

let property (record: Type) (defaults: obj) (p: PropertyInfo) : Json =
    let ft = p.PropertyType

    let doc =
        match fieldDocs.TryGetValue((record.Name, p.Name)) with
        | true, text -> Some text
        | _ -> None

    let inner =
        if isOption ft && isOption (arg ft) then
            nullable (schemaOf (arg (arg ft)))
        elif isOption ft && (attribute<JsonNullAttribute> p).IsSome then
            nullable (schemaOf (arg ft))
        else
            schemaOf ft

    // "One of `X`": the values of `X.All`, suggested but not required.
    let inner =
        match doc |> Option.map oneOfPattern.Match with
        | Some m when m.Success ->
            match enumValues m.Groups.[1].Value, inner with
            | Some values, JObject [ "type", JString "string" ] when strictEnums.Contains m.Groups.[1].Value ->
                JObject [ "enum", JArray(values |> List.map JString) ]
            | Some values, JObject [ "type", JString "string" ] ->
                JObject
                    [
                        "anyOf",
                        JArray
                            [
                                JObject [ "enum", JArray(values |> List.map JString) ]
                                JObject [ "type", JString "string" ]
                            ]
                    ]
            | _ -> inner
        | _ -> inner

    let members =
        match inner with
        | JObject members -> members
        | other -> [ "allOf", JArray [ other ] ]

    let withDefault =
        if isOption ft then
            members
        else
            match defaultOf ft (p.GetValue defaults) with
            | Some value -> members @ [ "default", value ]
            | None -> members

    JObject(withDescription doc withDefault)

let definition (t: Type) : Json =
    let doc =
        match typeDocs.TryGetValue t.Name with
        | true, text -> Some text
        | _ -> None

    if isSchemaUnion t then
        let cases = unionCases t

        for payload, tag in cases do
            tags.[payload] <- tag

        JObject(withDescription doc [ "oneOf", JArray [ for payload, _ in cases -> reference payload ] ])
    else
        let fields = FSharpType.GetRecordFields t
        let hasExtra = fields |> Array.exists (fun p -> p.Name = "Extra")
        let declared = fields |> Array.filter (fun p -> p.Name <> "Extra")
        let defaults = t.GetProperty("Default").GetValue(null)

        let tag =
            match tags.TryGetValue t with
            | true, tag -> Some tag
            | _ -> None

        let properties =
            [
                match tag with
                | Some tag -> yield "type", JObject [ "const", JString tag ]
                | None -> ()
                for p in declared do
                    yield jsonKey p, property t defaults p
            ]

        JObject(
            withDescription
                doc
                [
                    yield "type", JString "object"
                    yield "properties", JObject properties
                    match tag with
                    | Some _ -> yield "required", JArray [ JString "type" ]
                    | None when t = typeof<GameProject> -> yield "required", JArray [ JString "schemaVersion" ]
                    | None -> ()
                    yield "additionalProperties", JBool hasExtra
                ]
        )

reference typeof<GameProject> |> ignore
let defs = ResizeArray<string * Json>()

while pendingDefs.Count > 0 do
    let t = pendingDefs.Dequeue()
    defs.Add(t.Name, definition t)

let schema =
    JObject
        [
            "$schema", JString "https://json-schema.org/draft/2020-12/schema"
            "title", JString "Tractr project"
            "description",
            JString(
                sprintf
                    "A single-file Tractr project (schema version %s; version %s also loads). Generated by tools/codegen/project-schema.fsx from src/FarmEngine.Authoring/Schema.fs; do not edit. A project folder's project.json names its scenes, npcs, dialogues and items by file path instead, and its scene files hold a tileGrid in place of tiles (docs/PROJECT-FILES.md)."
                    (JsNumber.format ProjectSchema.CurrentProjectSchemaVersion)
                    (JsNumber.format ProjectSchema.ClassicProjectSchemaVersion)
            )
            "$ref", JString "#/$defs/GameProject"
            "$defs",
            JObject(
                defs
                |> Seq.sortWith (fun (a, _) (b, _) -> String.CompareOrdinal(a, b))
                |> List.ofSeq
            )
        ]

let target = IO.Path.Combine(root, "schemas", "project.schema.json")
let generated = Json.stringifyIndented schema + "\n"

if fsi.CommandLineArgs |> Array.contains "--check" then
    if
        not (IO.File.Exists target)
        || IO.File.ReadAllText(target).Replace("\r\n", "\n") <> generated
    then
        eprintfn "schemas/project.schema.json is out of date: run dotnet fsi tools/codegen/project-schema.fsx"
        exit 1

    printfn "schemas/project.schema.json is current (%d definitions)" defs.Count
else
    IO.File.WriteAllText(target, generated)
    printfn "%d definitions" defs.Count
