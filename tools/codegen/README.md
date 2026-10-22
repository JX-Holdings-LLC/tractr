# Code generators

Generated sources are checked in, so a build never needs these tools. CI runs
`tools/codegen/check.sh` (in the "Build & test (Linux)" job), which fails when
a committed file is not what its generator makes.

| Generated file | Generator | Run after |
|---|---|---|
| `crates/farm-cart-schema/src/cart_generated.rs`, `save_generated.rs` | `flatc --rust` 25.2.10 over `schemas/*.fbs` ([schemas/README.md](../../schemas/README.md)) | a `.fbs` change |
| `src/FarmEngine.Authoring/SchemaJson.fs` (the JSON decoders and encoders of the schema records) | `dotnet fsi tools/codegen/schema-json.fsx` (loads `Schema.fs` and reflects over its records; no build needed) | a schema record or union change |
| The codec and JSON-key tables in `src/FarmEngine.Authoring.Net/RecordJson.fs` | `python3 tools/codegen/record-json.py` (reads the decoders and encoders in `SchemaJson.fs`) | a `SchemaJson.fs` change |
| `src/FarmEngine.Authoring.Net/RecordWith.fs` (the `WithField` extensions the C# app builds records with) | `dotnet build -c Release src/FarmEngine.Authoring && dotnet fsi tools/codegen/record-with.fsx` (reflects over the built schema records) | a schema record change |

A schema change is: edit `Schema.fs`, then run the last three in table order
(`SchemaJson.fs` first, since `RecordJson.fs` is read from it).

```sh
tools/codegen/check.sh                  # check all four (downloads flatc on x86_64 Linux)
dotnet fsi tools/codegen/schema-json.fsx --check
python3 tools/codegen/record-json.py --check
dotnet fsi tools/codegen/record-with.fsx --check
```

`check.sh` uses `$FLATC`, else a `flatc` 25.2.10 on `PATH`, else (on x86_64
Linux) the release binary, downloaded once into `target/codegen` and checked
against its sha256.

## SchemaJson.fs

`schema-json.fsx` writes one decoder and one encoder per record of `Schema.fs`
(`let rec`/`and`, records in ordinal name order, then each union's case
records in case order and the union), from:

- the record fields, in order, and their types: `string`, `float`, `bool`,
  `int`, `uint32`, `Json`, another record or union, `'T list`,
  `(string * 'T) list` (an ordered map) and `option`. `'T option` is left out
  when `None`; `'T option option` is absent (`None`), `null` (`Some None`) or
  a value. An `Extra` field keeps undeclared keys (zod `.passthrough()`);
  without one they are dropped;
- the record's `Default`: a missing key decodes to the field's value there (a
  nested record at its own `Default` is written `Type.Default`);
- a union whose cases each carry one record is a zod discriminated union on
  `type`, with the tag from the union's `Type` member;
- three attributes for what the types do not say (defined at the top of
  `Schema.fs`; the generator reads them and nothing reads them at run time):
  - `[<JsonKey "selectedNPCId">]`: the JSON key, when it is not the field name
    in camelCase;
  - `[<JsonNull>]`: an `option` zod declares `.nullable()`, written as `null`
    when `None`;
  - `[<JsonDecode "…">]`: the decoder, as an F# expression over `key`, `path`
    and `value`, when it is not the one the type gives (the event flags'
    `boolean | number | string` values, the lenient rng `algorithm` literal).

It `#load`s `Json.fs` and `Schema.fs` instead of reading the built assembly
like `record-with.fsx`, because a record change usually stops the old
`SchemaJson.fs` from compiling. Attributes in the F# source were chosen over a
separate spec file so that a field and what its JSON needs stay on adjacent
lines, and over doc-comment markers so that a typo is a compile error.

The Rust serde types (`crates/farm-sim/src/schema`) are not generated: the
parity tests (`fixtures/parity`, `crates/farm-sim/tests/fsharp_parity.rs`)
check that Rust reads and writes every F# field. They do not compare the
defaults of missing keys; that would need a Rust-side table from record names
to serde types, which is left for when the Rust schema is generated too.
