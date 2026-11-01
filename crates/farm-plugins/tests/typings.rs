//! `docs/plugin-api/tractr-plugins.d.ts` (the TypeScript typings of the plugin API, #241) stays in
//! step with the engine: every hook, mutation type and read the engine knows is in it.

mod common;

use common::repo_path;
use farm_plugins::MUTATION_TYPES;
use farm_sim::hooks::hook_names;

fn typings() -> String {
    std::fs::read_to_string(repo_path("docs/plugin-api/tractr-plugins.d.ts")).expect("the typings exist")
}

#[test]
fn every_mutation_type_is_typed() {
    let text = typings();
    for kind in MUTATION_TYPES.iter().chain(&["saveData"]) {
        assert!(text.contains(&format!("type: '{kind}'")), "{kind} is missing from tractr-plugins.d.ts");
    }
}

#[test]
fn every_hook_is_typed() {
    let text = typings();
    for hook in hook_names::ALL {
        assert!(text.contains(&format!("  {hook}: {{")), "{hook} is missing from HookPayloads in tractr-plugins.d.ts");
    }
}

#[test]
fn every_read_is_typed() {
    let text = typings();
    for kind in [
        "player",
        "inventory",
        "itemCount",
        "date",
        "tile",
        "animals",
        "buildings",
        "entities",
        "timers",
        "npc",
        "npcs",
        "quest",
        "quests",
        "flag",
        "variable",
        "item",
        "crop",
        "definition",
    ] {
        assert!(farm_plugins::read_scope(kind).is_some(), "{kind} is a read");
        assert!(text.contains(&format!("  {kind}(")), "{kind} is missing from GameState in tractr-plugins.d.ts");
        assert!(farm_plugins::PRELUDE.contains(&format!("{kind}: function")), "the prelude has state.{kind}");
    }
}
