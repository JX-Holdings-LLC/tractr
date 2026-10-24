# Native numerics

**Status:** implemented (project schema version 2, save version
5; phase 7's last step, [LANGUAGES.md](LANGUAGES.md#phases)).
This file is the reference for the unit and integer type of every quantity the
simulation keeps. Read it before you add a field to `farm-sim`.

## Why

Until v8 the Rust core ran with JavaScript number semantics (`f64` everywhere,
the `js` helpers, FNV-1a over stable JSON) so that it reproduced the
TypeScript engine's goldens bit for bit. Now that the Rust engine is the
reference, the constraints change:

- **Determinism across machines** (replays, saves, future lockstep play) is
  easier to guarantee with integer arithmetic than with floats, whose results
  depend on operation order and, for transcendental functions, on the libm.
- **Exact rules.** "Adds 0.1 energy per tick" drifts in binary floating
  point; "adds 100 milli-points per tick" does not.
- **A cheaper, stronger state hash** over a binary encoding instead of stable
  JSON.

Rendering and audio may keep using floats: they read the state and never
write it.

## The rule

1. `farm-sim` keeps every quantity of the game state and the compiled content
   as an **integer in a fixed unit** (the tables below). Game logic never does
   float arithmetic: the crate has `#![deny(clippy::float_arithmetic)]`, with
   `#[allow]` only in `farm_sim::units` (the conversions below).
2. **JSON stays in authoring units.** Project JSON, cartridge content JSON,
   state JSON and saves keep the numbers creators know
   (tiles, energy points, minutes, probabilities in 0–1). Serde adapters in
   `farm_sim::units` convert at the boundary: reading rounds the number to the
   unit's grid (ties away from zero, like `f64::round`) and saturates into
   the integer type's range (a negative count reads as 0), writing prints the
   exact value of the integer in authoring units (whole values as JSON
   integers, others as the shortest decimal that reads back to the same
   double). A value already on the grid round-trips unchanged; one that isn't
   is moved to the nearest grid point once, when it is read. Each unit is a
   module (`units::energy`, `units::position`, …) with `opt`, `nullable`,
   `vec` and `nested_map` variants for `#[serde(with)]`.
3. **Conversions happen once**, at load. A `f64` read from JSON times a power
   of two or ten, rounded to an integer, is deterministic on every IEEE-754
   machine; nothing after that is.

## Units

| Quantity | Type | Unit | JSON (authoring) |
|---|---|---|---|
| Money, prices, costs, rewards, collapse penalty, animal purchase cost | `i64` | whole gold | gold |
| Price multipliers (`sellPriceMultiplier`), repair cost per durability point (`repairCostPerPoint`), collapse energy fraction | `u32` | 1/1000 | factor (0.5) |
| Event outcome `amount` (money, friendship; applied as whole gold/points, rounded) | `i64` | 1/1000 | number |
| Counts: quantities, stacks, yields, harvest counts, inventory size, floors, stages, weights of drops/fish/rocks/weather | `u32` (`i64` where a difference can be negative) | whole | count |
| Energy, max energy, energy costs | `i32` | 1/1000 point | points (2.5) |
| Absolute day, day of season, year, days grown, days without water, age and interval days | `u32` | whole day | days |
| Time of day, `timeOfDay` condition bounds | `u32` | 1/1000000 minute ("micro-minutes") | minutes (390.5) |
| Absolute in-game time (a machine's `completesAtMinute`) | `i64` | micro-minute | minutes |
| Minutes a machine processes, schedule minutes | `u32` | whole minute | minutes |
| Clock ticks | `u64` | tick (20 per second) | ticks |
| Time rate (`minutesPerRealSecond`) | `u32` | micro-minutes per tick, at load (`rate × 10⁶ / 20`) | minutes per second |
| Position of the player, NPCs, animals, monsters | `i32` | 1/8192 tile (1/256 pixel at the renderer's 32-pixel tiles) | tiles (5.5) |
| Tile coordinates (tiles, scene size, warps, mine entrance `entranceX/Y`, path points) | `i32` | whole tile | tiles |
| Move intent (`dx`, `dy`) | `i32` | whole, truncated toward zero on read | −1..1 |
| Command list index (`chooseDialogueOption.index`) | `i32` | whole; a fractional index reads as −1 and picks nothing | index |
| Movement speed (the player's, a monster's) | `i32` | 1/8192 tile per tick, at load (`speed × 8192 / 20`) | tiles per second |
| Probabilities and chances (mutation, ladder, junk, weather, crop damage, drop chance, pests, breeding), fish difficulty, minigame scores and tier `minScore` | `u64` | threshold out of 2³² (`p × 2³²`, clamped to 0..=2³²) | 0–1 |
| Friendship, mood | `i32` | whole points | points |
| Skill XP, skill levels, curve thresholds | `u32` | whole | whole |
| Tool power, tier, durability; node health | `i32` | whole | whole |
| Density of mine rocks and hazards | `u32` | 1/1000 | fraction |
| Health, damage, defense, monster health (M7) | `i32` | whole points | points |
| Area effect radius, storage slots, building size, capacity, build days, sight, hurt ticks | `u32` | whole | whole |

Screen-facing fields (sprite frame sizes, asset sizes, panel layout) are not
simulation state; they stay `f64` in the schema (`units::screen`) and the
canonical encoding writes only a unit value for them, so they are not hashed.
One project field stays an exact double (`units::exact`): `gameStartTime`, an
editor timestamp whose text is part of the engine seed
(`"<id>:<gameStartTime>"`), so rounding it would change every seeded game.

Anything a creator can type that isn't on the grid is rounded when read: a
price of 12.5 gold becomes 13 (ties away from zero), an energy cost of
0.0004 becomes 0. F# Problems warns about such values (see "Projects (F#)"
below), so they don't surprise anyone.

## Randomness

The RNG is xoshiro128\*\*. Its output is used in integer form; the table
shows what each use replaced:

| Float form | Integer form | Same outcome? |
|---|---|---|
| `next_float() < p` | `(next_u32() as u64) < threshold(p)` with `threshold = round(p × 2³²)` | Yes, whenever `p × 2³²` is an integer (every probability with at most 32 binary digits); otherwise it can differ by one draw in 2³². Built-in decimal chances such as 0.01 or 0.15 are not dyadic, so a draw exactly at the boundary can land on the other side (none of the golden replays does) |
| `floor(next_float() × n) + min` (`next_int`) | `((next_u32() as u64 × n) >> 32) + min` | Yes, exactly (`next_float` is `u / 2³²`) |
| weighted pick: `next_float() × total`, walk the weights | `(next_u32() as u64 × total) >> 32`, walk integer weights | Yes for integer weights |

## State hash

The state hash is **xxh3-64 over the canonical binary encoding** of the state: a serde serializer in `farm_sim::hash` that writes
integers little-endian, booleans as one byte, floats (only free-form JSON
values such as flags carry them) as their IEEE-754 bits, except that a whole
float within ±(2⁵³−1) is written as the integer it equals (stable JSON writes
`1.0` as `1`, which reads back as an integer), strings and
sequences with a `u32` length prefix, options as a tag byte, struct fields
behind a presence byte (so a skipped optional field and a present value never
collide) and enum variants as their index. Map entries are **sorted by their
encoded key**, not written in stored order: a save (stable JSON) does not keep
insertion order, and the hash must survive a save and load. It serializes the
integer fields as integers (the serde adapters know whether they write JSON or
the canonical form, from `is_human_readable`). The hash text is 16 lowercase
hex digits. The cartridge hash in save files (`cart_hash`) uses the same
function.

## Saves

`CURRENT_SAVE_VERSION` is 5 (saves have their own numbering). The
quantization is in the serde adapters, applied when a state is read. Only v5
saves load: other save versions are refused (`farm_cart::load_game_state`). The FlatBuffers `SavePreview` fields keep
their `double` type (a schema field is never retyped); they hold whole
numbers.

## Projects (F#)

`ProjectSchema.CurrentProjectSchemaVersion` is 1, and only version 1 projects load
(`ProjectLoad`); other versions are refused. The values a
project carries into play (player and NPC positions, energy, the current
time of day, animal positions and mood) are on the engine's grid, so the
editor shows what the game will run. Content definitions (prices, chances) keep what the creator typed; the Rust
loader quantizes them, and Problems reports a warning (`numbers.offGrid`)
where that changes a value in a way a player could notice (a fractional
price or count). "Keep changes" (the hosts' synced project) writes the running
state into the project JSON as the editor sent it
(`state::apply_state_to_project_json`), so it never replaces content values
with their quantized form.

## Goldens

Rust is the reference:

- replays, content, save, RNG and hash goldens are recorded from Rust with
  `FARM_RECORD_GOLDENS=1` on the test that checks them (`golden_replays`,
  `golden_content`, `golden_primitives` in `farm-sim`, `golden_saves` in
  `farm-cart`; see `fixtures/golden/SOURCE.txt`) and reviewed as a diff; each
  golden file holds its own inputs (project, seed, input log);
- `fixtures/golden/vectors` keeps fixed RNG and stable-JSON vectors that are
  never re-recorded;
- recording has guardrails, because a self-recorded golden pins whatever the
  engine does, bugs included: the switches (`FARM_RECORD_GOLDENS`,
  `FARM_PLAYER_BLESS`, `FARM_RENDER_BLESS`) are on only when set to `1`; a
  recording run rewrites only the files that change and then **fails** with a
  summary, so it never passes as a test run; `FARM_RECORD_GOLDENS` stamps the
  engine commit it recorded on into `fixtures/golden/SOURCE.txt`; and CI
  refuses to run with any record or bless switch set and fails when the tests
  leave `fixtures/` different from the commit.
