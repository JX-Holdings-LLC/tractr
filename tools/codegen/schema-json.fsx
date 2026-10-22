// Regenerates src/FarmEngine.Authoring/SchemaJson.fs (the JSON decoders and encoders of the schema
// records) from src/FarmEngine.Authoring/Schema.fs: dotnet fsi tools/codegen/schema-json.fsx
// With --check it writes nothing and exits 1 when SchemaJson.fs is not what it would generate
// (tools/codegen/check.sh, CI).
//
// It loads Schema.fs itself (no build needed, so a record change that the old SchemaJson.fs no
// longer compiles against can still be regenerated) and reflects over the records and unions:
//   - field order, types and names come from the records; the JSON key is the field name in
//     camelCase, or its `[<JsonKey>]`;
//   - the value a missing key decodes to is the field's value in the record's `Default`;
//   - `option` is left out when `None`, or written as `null` with `[<JsonNull>]`;
//     `'T option option` is absent / `null` / a value;
//   - an `Extra` field keeps undeclared keys (zod `.passthrough()`); without one they are dropped;
//   - a union whose cases each carry one record is a zod discriminated union on `type`; its
//     `Type` member gives each case's tag;
//   - `[<JsonDecode>]` replaces a field's decoder with the F# expression it holds.
#load "../../src/FarmEngine.Authoring/Json.fs"
#load "../../src/FarmEngine.Authoring/Schema.fs"

open System
open System.Globalization
open System.Reflection
open Microsoft.FSharp.Reflection
open FarmEngine.Authoring
open FarmEngine.Schemas

let schemaTypes =
    typeof<GameProject>.Assembly.GetTypes()
    |> Array.filter (fun t -> t.Namespace = typeof<GameProject>.Namespace)

let unions =
    schemaTypes
    |> Array.filter (fun t -> FSharpType.IsUnion t && not t.IsNested)
    |> Array.sortBy (fun t -> t.Name)

/// Each union with its cases: (case, payload record type, `type` tag).
let unionCases =
    [
        for u in unions ->
            let cases =
                [
                    for case in FSharpType.GetUnionCases u ->
                        match case.GetFields() with
                        | [| field |] when FSharpType.IsRecord field.PropertyType ->
                            let payload = field.PropertyType
                            let defaultValue = payload.GetProperty("Default").GetValue(null)
                            let instance = FSharpValue.MakeUnion(case, [| defaultValue |])
                            let tag = u.GetProperty("Type").GetValue(instance) :?> string
                            case, payload, tag
                        | _ -> failwithf "%s.%s: each case of a schema union carries one record" u.Name case.Name
                ]
            u, cases
    ]

let payloadTag = dict [ for _, cases in unionCases do for _, payload, tag in cases -> payload, tag ]

let records =
    schemaTypes
    |> Array.filter (fun t -> FSharpType.IsRecord t && not (payloadTag.ContainsKey t))
    |> Array.sortWith (fun a b -> String.CompareOrdinal(a.Name, b.Name))

let isGen (d: Type) (t: Type) = t.IsGenericType && t.GetGenericTypeDefinition() = d
let isOption = isGen typedefof<option<_>>
let isList = isGen typedefof<list<_>>
let arg (t: Type) = t.GetGenericArguments().[0]

/// `(string * 'T) list`, an ordered map (`z.record`): the `'T`.
let dictValue (t: Type) =
    if isList t && FSharpType.IsTuple(arg t) then
        match FSharpType.GetTupleElements(arg t) with
        | [| k; v |] when k = typeof<string> -> Some v
        | _ -> None
    else
        None

let rec decoder (t: Type) : string =
    if t = typeof<string> then "Decode.string"
    elif t = typeof<float> then "Decode.number"
    elif t = typeof<bool> then "Decode.boolean"
    elif t = typeof<int> then "Decode.int32"
    elif t = typeof<uint32> then "Decode.uint32"
    elif t = typeof<Json> then "Decode.json"
    elif FSharpType.IsRecord t || FSharpType.IsUnion t && t.Namespace = typeof<GameProject>.Namespace then "decode" + t.Name
    else
        match dictValue t with
        | Some v -> sprintf "(Decode.dict %s)" (decoder v)
        | None when isList t -> sprintf "(Decode.list %s)" (decoder (arg t))
        | None -> failwithf "no decoder for %s" t.FullName

/// The encoder, as a function.
let rec encoder (t: Type) : string =
    if t = typeof<string> then "JString"
    elif t = typeof<float> then "JNumber"
    elif t = typeof<bool> then "JBool"
    elif t = typeof<int> || t = typeof<uint32> then "(float >> JNumber)"
    elif t = typeof<Json> then "id"
    elif FSharpType.IsRecord t || FSharpType.IsUnion t && t.Namespace = typeof<GameProject>.Namespace then "encode" + t.Name
    else
        match dictValue t with
        | Some v -> sprintf "(Encode.dict %s)" (encoder v)
        | None when isList t -> sprintf "(Encode.list %s)" (encoder (arg t))
        | None -> failwithf "no encoder for %s" t.FullName

/// The encoder applied to `x`.
let encode (t: Type) (x: string) = if t = typeof<Json> then x else encoder t + " " + x

let floatLiteral (n: float) =
    if Double.IsNaN n || Double.IsInfinity n then failwithf "default %f has no F# literal here" n
    elif n = Math.Floor n && abs n < 1e15 then n.ToString("0.0", CultureInfo.InvariantCulture)
    else
        let text = n.ToString("R", CultureInfo.InvariantCulture)
        if text.Contains "E" then failwithf "default %s: add an exponent form to schema-json.fsx" text
        text

let stringLiteral (s: string) =
    if s |> Seq.exists (fun c -> c < ' ' || c = '\\' || c = '"') then
        failwithf "default %A: add escaping to schema-json.fsx" s
    "\"" + s + "\""

/// A default value as F# source: a nested record at its own `Default` is `Type.Default`.
let rec literal (t: Type) (value: obj) : string =
    if t = typeof<string> then stringLiteral (value :?> string)
    elif t = typeof<float> then floatLiteral (value :?> float)
    elif t = typeof<bool> then (if value :?> bool then "true" else "false")
    elif t = typeof<int> then string (value :?> int)
    elif t = typeof<uint32> then string (value :?> uint32) + "u"
    elif isOption t then
        match value with
        | null -> "None"
        | _ -> sprintf "Some(%s)" (literal (arg t) (t.GetProperty("Value").GetValue value))
    elif isList t then
        let items = [ for item in (value :?> Collections.IEnumerable) -> literal (arg t) item ]
        if items.IsEmpty then "[]" else "[ " + String.concat "; " items + " ]"
    elif FSharpType.IsTuple t then
        let parts = Array.map2 literal (FSharpType.GetTupleElements t) (FSharpValue.GetTupleFields value)
        "(" + String.concat ", " parts + ")"
    elif FSharpType.IsRecord t then
        if value.Equals(t.GetProperty("Default").GetValue(null)) then
            t.Name + ".Default"
        else
            let fields =
                Array.map2
                    (fun (p: PropertyInfo) v -> p.Name + " = " + literal p.PropertyType v)
                    (FSharpType.GetRecordFields t)
                    (FSharpValue.GetRecordFields value)
            sprintf "({ %s } : %s)" (String.concat "; " fields) t.Name
    else
        failwithf "default of type %s: add it to schema-json.fsx" t.FullName

let attribute<'A when 'A :> Attribute> (p: PropertyInfo) : 'A option =
    match Attribute.GetCustomAttribute(p, typeof<'A>) with
    | null -> None
    | a -> Some(a :?> 'A)

let jsonKey (p: PropertyInfo) =
    match attribute<JsonKeyAttribute> p with
    | Some a -> a.Key
    | None -> string (Char.ToLowerInvariant p.Name.[0]) + p.Name.Substring 1

let lines = ResizeArray<string>()
let add (s: string) = lines.Add s
let mutable first = true

let emitRecord (t: Type) (tag: string option) =
    let fields = FSharpType.GetRecordFields t
    let hasExtra = fields |> Array.exists (fun p -> p.Name = "Extra")
    let declared = fields |> Array.filter (fun p -> p.Name <> "Extra")
    let defaults = t.GetProperty("Default").GetValue(null)
    let defaultOf (p: PropertyInfo) = p.GetValue defaults
    add (sprintf "    %s decode%s (path: Path) (json: Json) : %s =" (if first then "let rec" else "and") t.Name t.Name)
    first <- false
    add "        let members = Decode.object path json"
    for p in declared do
        add (sprintf "        let mutable v%s = %s" p.Name (literal p.PropertyType (defaultOf p)))
    if hasExtra then add "        let extra = ResizeArray<string * Json>()"
    add "        for (key, value) in members do"
    add "            match key with"
    for p in declared do
        let ft = p.PropertyType
        let read =
            match attribute<JsonDecodeAttribute> p with
            | Some a -> a.Expression
            | None when isOption ft && isOption (arg ft) -> sprintf "Decode.optionalNullable %s (key :: path) value" (decoder (arg (arg ft)))
            | None when isOption ft -> sprintf "Decode.optional %s (key :: path) value" (decoder (arg ft))
            | None -> sprintf "%s (key :: path) value" (decoder ft)
        add (sprintf "            | \"%s\" -> v%s <- %s" (jsonKey p) p.Name read)
    if tag.IsSome then add "            | \"type\" -> ()"
    add (if hasExtra then "            | _ -> extra.Add((key, value))" else "            | _ -> ()")
    let assigns = [ for p in declared -> sprintf "%s = v%s" p.Name p.Name ] @ (if hasExtra then [ "Extra = List.ofSeq extra" ] else [])
    add (sprintf "        { %s }" (String.concat "; " assigns))
    add ""
    add (sprintf "    and encode%s (value: %s) : Json =" t.Name t.Name)
    add "        JObject("
    add "            ["
    match tag with
    | Some tag -> add (sprintf "                yield \"type\", JString \"%s\"" tag)
    | None -> ()
    for p in declared do
        let ft = p.PropertyType
        let key = jsonKey p
        let writesNull = (attribute<JsonNullAttribute> p).IsSome
        if isOption ft && isOption (arg ft) then
            if writesNull then failwithf "%s.%s: an option option writes null for Some None already" t.Name p.Name
            add (sprintf "                match value.%s with" p.Name)
            add (sprintf "                | Some(Some n) -> yield \"%s\", %s" key (encode (arg (arg ft)) "n"))
            add (sprintf "                | Some None -> yield \"%s\", JNull" key)
            add "                | None -> ()"
        elif isOption ft then
            add (sprintf "                match value.%s with" p.Name)
            add (sprintf "                | Some v -> yield \"%s\", %s" key (encode (arg ft) "v"))
            add (if writesNull then sprintf "                | None -> yield \"%s\", JNull" key else "                | None -> ()")
        else
            if writesNull then failwithf "%s.%s: [<JsonNull>] is for option fields" t.Name p.Name
            add (sprintf "                yield \"%s\", %s" key (encode ft ("value." + p.Name)))
    if hasExtra then add "                yield! value.Extra"
    add "            ]"
    add "        )"
    add ""

let emitUnion (u: Type) (cases: (UnionCaseInfo * Type * string) list) =
    add (sprintf "    and decode%s (path: Path) (json: Json) : %s =" u.Name u.Name)
    add "        match Decode.discriminator path json with"
    for case, payload, tag in cases do
        add (sprintf "        | \"%s\" -> %s.%s(decode%s path json)" tag u.Name case.Name payload.Name)
    add "        | other -> Decode.unknownDiscriminator path other"
    add ""
    add (sprintf "    and encode%s (value: %s) : Json =" u.Name u.Name)
    add "        match value with"
    for case, payload, _ in cases do
        add (sprintf "        | %s.%s c -> encode%s c" u.Name case.Name payload.Name)

add "namespace FarmEngine.Schemas"
add ""
add "open FarmEngine.Authoring"
add ""
add "// Generated by tools/codegen/schema-json.fsx from the records in Schema.fs. Do not edit."
add ""
add "/// JSON for the schema records (Schema.fs): `decode…` reads a `Json` value the way the"
add "/// retired System.Text.Json contract did (camelCase names, absent keys take the field's default,"
add "/// undeclared keys go to `Extra` or are dropped) and fails on the first value of the wrong kind with"
add "/// a zod-style issue (`scenes.0.width: Expected number, received string`). `encode…` writes declared"
add "/// fields in order, then `Extra`; absent options are left out, except the fields zod declares"
add "/// `.nullable()`, which are written as `null`."
add "module SchemaJson ="
for t in records do
    emitRecord t None
for u, cases in unionCases do
    for _, payload, tag in cases do
        emitRecord payload (Some tag)
    emitUnion u cases

let target = IO.Path.Combine(__SOURCE_DIRECTORY__, "..", "..", "src", "FarmEngine.Authoring", "SchemaJson.fs")
let generated = String.concat "\n" lines + "\n"
if fsi.CommandLineArgs |> Array.contains "--check" then
    // Line endings as checked out do not matter (a Windows checkout may have CRLF).
    if IO.File.ReadAllText(target).Replace("\r\n", "\n") <> generated then
        eprintfn "src/FarmEngine.Authoring/SchemaJson.fs is out of date: run dotnet fsi tools/codegen/schema-json.fsx"
        exit 1
    printfn "src/FarmEngine.Authoring/SchemaJson.fs is current (%d records, %d unions)" (records.Length + payloadTag.Count) unions.Length
else
    IO.File.WriteAllText(target, generated)
    printfn "%d lines" lines.Count
