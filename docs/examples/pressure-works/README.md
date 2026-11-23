# Pressure Works

A small workshop game that is not a farm, made to show two parts of a
project ([Creator guide](../../CREATOR-GUIDE.md#rules-games-that-are-not-farms)):

- **Rules.** Nobody sleeps: the clock runs round the whole day and the date
  rolls over at midnight. There is no farming, and a player out of energy is
  worn out instead of collapsing (`settings.rules` in
  [project.json](project.json)).
- **Properties.** Each greenhouse has its own `pressure`, which leaks 6 kPa
  an hour (Greenhouse B starts at 40, Greenhouse A at 90) and raises an alarm
  when it falls below 50. The player has `oxygen` beside energy: it drains in
  a greenhouse whose pressure is low and comes back elsewhere, and running
  out costs energy. The **Run the pump** action (P) adds pressure to the
  greenhouse the player stands in.

Open the folder in Tractr, or check and play it from the command line:

```sh
dotnet run --project src/FarmEngine.Cli -- check docs/examples/pressure-works
dotnet run --project src/FarmEngine.Cli -- compile docs/examples/pressure-works --out pressure-works.cart
cargo run -p farm-player -- --cart pressure-works.cart
```

The files in [replays](replays) play it without a window
([Checking a game](../../PLAYER.md#checking-a-game-with---headless)) and
state what must hold at the end:

| Replay | What it checks |
|---|---|
| `round-the-clock.json` | 25 hours pass without a bedtime or a collapse: the date rolls at midnight and the money is untouched. |
| `two-greenhouses.json` | Two hours in Greenhouse B: its pressure and Greenhouse A's fall independently, and oxygen drains while energy stays. |
| `pump.json` | Pumping raises the pressure of the greenhouse the player is in, and oxygen recovers. |
| `night-shift-first-half.json`, `night-shift-second-half.json` | A shift that crosses midnight, saved at 1:30 and loaded again, ends exactly as the same shift played without stopping. |

```sh
cargo run -p farm-player -- --headless --cart pressure-works.cart --replay docs/examples/pressure-works/replays/two-greenhouses.json
```

The engine's tests play these replays on every change
(`crates/farm-player/tests/examples.rs`).
