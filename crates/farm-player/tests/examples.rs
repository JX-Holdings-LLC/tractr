//! The example project Pressure Works (`docs/examples/pressure-works`), played headless: a
//! workshop without sleep or farming (#243) whose two greenhouses keep their own pressure while
//! the player has oxygen beside energy (#244). Its cartridge is compiled by the authoring core
//! (`fixtures/golden/cartridges/pressure-works.cart`, kept current by the F# tests), so these
//! replays go the whole way: project files → cartridge → player.
//!
//! Each replay states what must hold at its end (`expect`); a miss fails the run.

use farm_player::{run_cartridge, ReplayFile};
use std::path::PathBuf;

fn root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

fn cartridge() -> Vec<u8> {
    std::fs::read(root().join("fixtures").join("golden").join("cartridges").join("pressure-works.cart")).unwrap()
}

fn replay(name: &str) -> ReplayFile {
    let path = root().join("docs").join("examples").join("pressure-works").join("replays").join(name);
    ReplayFile::parse(&std::fs::read_to_string(&path).unwrap()).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// 25 hours pass without a bedtime: no collapse, no penalty, the date rolls at midnight.
#[test]
fn the_workshop_runs_round_the_clock() {
    let run = run_cartridge(&cartridge(), &replay("round-the-clock.json"), None).unwrap();
    let report = &run.report;
    assert_eq!((report.clock.day, report.clock.minute), (2, 7 * 60));
    assert!(!report.effects.iter().any(|effect| format!("{effect:?}").contains("collapsed")), "{:?}", report.effects);
    assert!(report.plugin_errors.is_empty());
}

/// The greenhouses keep their own pressure, oxygen drains beside energy, and the pump raises
/// the pressure of the greenhouse the player stands in.
#[test]
fn two_greenhouses_keep_their_own_pressure() {
    let run = run_cartridge(&cartridge(), &replay("two-greenhouses.json"), None).unwrap();
    // Stamina is its own meter: two hours of thin air cost oxygen, not energy.
    let save: serde_json::Value = serde_json::from_str(&run.save_json).unwrap();
    assert_eq!(save["state"]["player"]["energy"], save["state"]["player"]["maxEnergy"]);
    run_cartridge(&cartridge(), &replay("pump.json"), None).unwrap();
}

/// A shift that crosses midnight, saved at 1:30 and loaded again, ends exactly as the same shift
/// played without stopping.
#[test]
fn a_night_shift_reloads_where_it_was() {
    let cart = cartridge();
    let first = replay("night-shift-first-half.json");
    let second = replay("night-shift-second-half.json");
    let saved = run_cartridge(&cart, &first, None).unwrap();
    assert_eq!((saved.report.clock.day, saved.report.clock.minute), (2, 90), "saved after midnight");
    let whole = ReplayFile {
        inputs: first.inputs.iter().chain(&second.inputs).cloned().collect(),
        expect: second.expect.clone(),
        ..first.clone()
    };
    let uninterrupted = run_cartridge(&cart, &whole, None).unwrap();
    for save in [saved.save_json.as_bytes(), &saved.save_binary[..]] {
        let resumed = run_cartridge(&cart, &second, Some(save)).unwrap();
        assert_eq!(resumed.report.state_hash, uninterrupted.report.state_hash);
        assert_eq!(resumed.report.variables, uninterrupted.report.variables);
    }
}
