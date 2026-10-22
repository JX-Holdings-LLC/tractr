//! The built-in catalogs pinned (`fixtures/projects/content-builtin.json`: items, machines,
//! recipes, animals, fish, nodes, the shop, crops, multipliers, the calendar and tools), and the
//! pack locale passes over `fixtures/projects/packs-project.json`.
//!
//! The catalog holds numbers in authoring units, so one that is off the engine's grid (a 0.15
//! chance, say) comes back as the nearest grid value: the comparison allows that difference
//! (1e-3) and nothing else.

use farm_sim::schema::GameProject;
use farm_sim::{content_builtin, packs, state};
use indexmap::IndexMap;
use serde_json::Value;
use std::path::PathBuf;

fn fixture(name: &str) -> Value {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "fixtures", "projects", name].iter().collect();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&text).expect("valid fixture JSON")
}

/// Equal JSON trees, numbers up to the v9 grids.
fn differences(path: &str, expected: &Value, actual: &Value, out: &mut Vec<String>) {
    match (expected, actual) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap_or(f64::NAN), b.as_f64().unwrap_or(f64::NAN));
            if (a - b).abs() > 1e-3 {
                out.push(format!("{path}: pinned {a}, engine {b}"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            for key in a.keys().chain(b.keys().filter(|key| !a.contains_key(*key))) {
                let child = format!("{path}.{key}");
                differences(&child, a.get(key).unwrap_or(&Value::Null), b.get(key).unwrap_or(&Value::Null), out);
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (index, (x, y)) in a.iter().zip(b).enumerate() {
                differences(&format!("{path}.{index}"), x, y, out);
            }
        }
        _ if expected == actual => {}
        _ => out.push(format!("{path}: pinned {expected}, engine {actual}")),
    }
}

fn assert_same_within_grid(key: &str, expected: &Value, actual: &Value) {
    let mut out = Vec::new();
    differences("", expected, actual, &mut out);
    assert!(out.is_empty(), "{key}: differs from the pinned catalog beyond the number grids:\n  {}", out.join("\n  "));
}

#[test]
fn built_in_content_matches_the_pinned_catalog() {
    let expected = fixture("content-builtin.json");
    let mut actual: IndexMap<&str, Value> = IndexMap::new();
    let mut put = |key: &'static str, value: &dyn erased::Serialize| {
        actual.insert(key, value.to_value());
    };
    put("items", &content_builtin::create_default_items());
    put("machineTypes", &content_builtin::create_default_machine_types());
    put("recipes", &content_builtin::create_default_recipes());
    put("animals", &content_builtin::create_default_animal_species());
    put("fish", &content_builtin::create_default_fish_tables());
    put("mineNodes", &content_builtin::mine_node_types());
    put("bands", &content_builtin::create_default_mine_bands());
    put("nodes", &content_builtin::default_node_types());
    put("shop", &content_builtin::create_default_shop());
    put("crops", &content_builtin::crop_definitions());
    // The multipliers are thousandths; the catalog holds the factors.
    let factors = |milli: IndexMap<String, u32>| -> IndexMap<String, f64> {
        milli.into_iter().map(|(key, value)| (key, f64::from(value) / 1000.0)).collect()
    };
    put("quality", &factors(content_builtin::quality_multipliers()));
    put("mutation", &factors(content_builtin::mutation_multipliers()));
    put("days", &content_builtin::DAYS_PER_SEASON);
    put("seasons", &content_builtin::SEASON_ORDER);
    put("tools", &content_builtin::tool_definitions());
    assert_same_within_grid("content-builtin", &expected, &serde_json::to_value(&actual).expect("serializes"));
}

/// Minimal object-safe serialization so the built-in table above can mix value types.
mod erased {
    pub trait Serialize {
        fn to_value(&self) -> serde_json::Value;
    }

    impl<T: serde::Serialize> Serialize for T {
        fn to_value(&self) -> serde_json::Value {
            serde_json::to_value(self).expect("engine types always serialize")
        }
    }
}

#[test]
fn locales_use_later_packs_and_fall_back_to_authored_text() {
    let project: GameProject = serde_json::from_value(fixture("packs-project.json")).expect("a GameProject");
    let base = state::create_base_content_from_project(&project);
    let merged = packs::merge_packs_into_content(&base, &project.content_packs).content;
    assert_ne!(packs::apply_locale_strings(merged.clone(), &project.content_packs, "fr"), merged);
    assert_eq!(packs::apply_locale_strings(merged.clone(), &project.content_packs, "xx"), merged);
    assert_eq!(packs::apply_locale_strings(merged.clone(), &project.content_packs, ""), merged);
    // The one-pass merge used at play time localizes the same way.
    for locale in ["fr", "xx", ""] {
        let localized = packs::apply_locale_strings(merged.clone(), &project.content_packs, locale);
        let one_pass = packs::merge_packs_into_content_localized(&base, &project.content_packs, locale);
        assert_eq!(one_pass.content, localized, "{locale}");
    }
}
