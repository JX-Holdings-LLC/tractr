# Code conventions

How the schema, the simulation and their tests are laid out. The numbers
follow [NUMERICS.md](NUMERICS.md): integers in fixed units in `farm-sim`,
authoring units in JSON. [ARCHITECTURE.md](ARCHITECTURE.md) describes the Rust / F#
/ C# split.

## Types (schemas)

The schema exists twice: as Rust types in `farm-sim::schema` (serde, the
engine's own) and as F# records in `src/FarmEngine.Authoring/Schema.fs` (the
editor's data model), with their JSON in `SchemaJson.fs`, generated from
`Schema.fs` by `tools/codegen/schema-json.fsx`. Both must round-trip the same
JSON; the parity tests check it (`fixtures/parity`). The rules for the F#
records:

- A JSON object → an F# record in PascalCase with a `static member Default`
  (every field at its schema default). The JSON key is the field name in
  camelCase; a key that is not (acronyms such as `selectedNPCId`) takes
  `[<JsonKey "selectedNPCId">]` on the field.
- A record that keeps keys it does not declare has an
  `Extra: (string * Json) list` field: undeclared keys in order, written back
  after the declared ones. Records without `Extra` drop unknown keys.
- Numbers → **`float`** in authoring units. The engine keeps its own integer
  units ([NUMERICS.md](NUMERICS.md)); the project keeps what the creator typed.
- An optional key → `option`, omitted from JSON when `None`.
- A required key that may be `null` → `option` with `[<JsonNull>]`, and the
  encoder writes `null` for `None`. Optional and nullable →
  `'T option option` (`None` absent, `Some None` null).
- A default value → the decoder's value for a missing key (and the record's
  `Default`).
- A string enum → plain `string` plus a module of `[<Literal>]` constants with
  an `All` list (`Directions.Up`, `SchemaConstants.fs`).
- A list → `'T list`. A map → `(string * 'T) list` (in insertion order).
- Free-form values and scalar unions → `Json`.
- A union tagged by `type` → an F# union whose cases carry one record each
  (`EventCondition.Flag of FlagCondition`), with a `Type` member for the tag.
- A single object with a `type` field that is **not** a tagged union (e.g.
  `EventOutcome`) stays a plain record with a `Type: string` field.
- Helper functions that belong to a schema record → functions in a module
  named after it (`EventsSchema.EventFiredFlag`, …).
- After changing a record, regenerate `SchemaJson.fs`
  (`dotnet fsi tools/codegen/schema-json.fsx`), then the C# helpers:
  `tools/codegen/record-json.py` (codec and key tables) and
  `tools/codegen/record-with.fsx` (the `WithField` extensions). A change to
  what a project stores bumps the project schema version; only the current
  version loads.

## State and numbers (Rust)

- **Numbers:** every quantity in `farm-sim` is an integer in a fixed unit,
  with float arithmetic denied outside the JSON adapters. The unit of each
  quantity, the serde adapters (`farm_sim::units`), randomness and the state
  hash are in [NUMERICS.md](NUMERICS.md). Read it before you add a field.
- Maps → `IndexMap<String, T>` (insertion order). `HashMap`/`HashSet` are
  banned by `clippy.toml`.
- Optional keys → `Option<T>` with `skip_serializing_if`; nullable keys keep
  the key and write `null`. Keep absent and `null` apart where the schema does.
- Free-form values → `serde_json::Value`.
- The engine mutates its own `GameState` in place (`&mut`); content is
  immutable.
- Modules and functions are snake_case; doc comments carry the design
  rationale.

## Determinism rules

| Rule | How |
|---|---|
| Stable sorts | `sort_by`; `sort_unstable*` is banned (`clippy.toml`) |
| String order where it is observable | `text::compare_strings` (UTF-16 code units) |
| Flag values | JavaScript truthiness: `0`, `NaN`, `""` are falsy (`text::truthy`) |
| Out-of-range reads | `get(i)`, never a panic |
| Randomness and time | never in the simulation: the seeded `rng` and the game clock (`Instant`/`SystemTime` are banned) |

## Tests

Rust tests live in the crate (`crates/<crate>/tests/<name>.rs`); schema-only
tests go to `tests/FarmEngine.Authoring.Tests` (`SchemaRecordTests.fs`).
Goldens live in `fixtures/golden/` (recorded, never hand-edited; see
[CONTRIBUTING.md](../../CONTRIBUTING.md)) and the shared project fixtures in
`fixtures/projects/`; the Rust and F# tests both read them.
