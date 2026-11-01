//! The NPC looks an appearance picks, from the fixture the F# Problems check reads too
//! (`fixtures/art/npc-appearances.json`, `BuiltinLooks` in FarmEngine.Authoring), so the tip
//! Problems shows names the sheet the game really draws.

use farm_render::BuiltinArt;
use serde_json::Value;

#[test]
fn npc_appearances_pick_the_sheets_the_shared_fixture_lists() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/art/npc-appearances.json");
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("fixture JSON");
    let art = BuiltinArt::embedded();
    let cases = fixture["cases"].as_array().expect("cases");
    assert!(!cases.is_empty());
    for case in cases {
        let appearance = case["appearance"].as_str().expect("appearance");
        let sheet = case["sheet"].as_str().expect("sheet");
        let drawn = art.npc_entry(Some(appearance)).expect("a sheet");
        assert_eq!(drawn.name, sheet, "appearance {appearance:?}");
    }
}
