//! Loading saves: the save version check, the parse into [`GameState`] and the `GameState`
//! checks (from `SchemaValidation.ValidateGameState`).
//!
//! Saves belong to the player's machine, so Rust owns their format (docs/LANGUAGES.md "Saves
//! (Rust only)"). A save is the `GameState` JSON in authoring units. Only the current save
//! version ([`CURRENT_SAVE_VERSION`]) loads; there are no migrations from older ones. Parsing
//! quantizes the state onto the engine's integer grids (`farm_sim::units`: positions to 1/8192
//! tile, energy to 1/1000 point, …); the zod `int()` refinements are checked on the JSON as
//! read. [`load_game_state`] never panics on bad input.

use farm_sim::schema::primitives::{directions, quest_statuses};
use farm_sim::schema::save::{SaveLoadResult, CURRENT_SAVE_VERSION};
use farm_sim::schema::GameState;
use farm_sim::units;
use serde_json::Value;

/// The most errors a failed parse reports (zod `issues.slice(0, 20)`).
pub(crate) const MAX_ERRORS: usize = 20;

/// Validate serialized simulation state. This is deliberately independent from the project
/// schema: authored content and a player's live runtime state evolve on different schedules.
/// Never panics. Loaders that own the JSON tree use [`load_game_state_owned`], which does not
/// copy it.
pub fn load_game_state(raw: &Value) -> SaveLoadResult {
    load_game_state_owned(raw.clone())
}

/// [`load_game_state`] taking the tree by value: a save's state is parsed and checked in place,
/// without the copies a crafted multi-megabyte save would multiply (#80).
pub fn load_game_state_owned(raw: Value) -> SaveLoadResult {
    if !raw.is_object() {
        return fail("Save state is not an object".to_owned());
    }
    let current = f64::from(CURRENT_SAVE_VERSION);
    match raw.get("meta").and_then(|meta| meta.get("saveVersion")).and_then(Value::as_f64) {
        Some(version) if version == current => {}
        Some(version) if version > current => {
            return fail(format!(
                "Save version {} is newer than this engine supports ({CURRENT_SAVE_VERSION}). Update the engine.",
                units::format_number(version)
            ))
        }
        Some(version) => {
            return fail(format!(
                "Save version {} is not supported (this engine reads version {CURRENT_SAVE_VERSION}).",
                units::format_number(version)
            ))
        }
        None => {
            return fail(format!(
                "Save state has no meta.saveVersion (this engine reads version {CURRENT_SAVE_VERSION})."
            ))
        }
    }

    // Whole doubles as integers (`1.0` → `1`): stable JSON writes them that way, so a state that
    // went through a save compares and hashes the same as before (#142).
    let raw_state = units::canonical_json(raw);
    match parse_game_state(&raw_state) {
        Ok(data) => {
            let errors = validate_game_state_json(&raw_state, &data);
            if errors.is_empty() {
                SaveLoadResult { ok: true, data: Some(data), errors }
            } else {
                SaveLoadResult { ok: false, data: None, errors }
            }
        }
        Err(error) => fail(error),
    }
}

/// [`load_game_state`] over JSON text. Never panics.
pub fn load_game_state_json(json: &str) -> SaveLoadResult {
    match serde_json::from_str::<Value>(json) {
        Ok(raw) => load_game_state_owned(raw),
        Err(error) => fail(format!("Save state is not valid JSON: {error}")),
    }
}

/// Re-validate an in-memory state. Never panics.
pub fn revalidate_game_state(state: &GameState) -> SaveLoadResult {
    match serde_json::to_value(state) {
        Ok(raw) => load_game_state_owned(raw),
        Err(error) => fail(format!("Save state could not be serialized: {error}")),
    }
}

/// Port of `SchemaValidation.ValidateGameState`: the zod refinements serde's types don't express
/// (integers, enums, ranges). Returns at most [`MAX_ERRORS`] messages as `path: message`.
pub fn validate_game_state(state: &GameState) -> Vec<String> {
    match serde_json::to_value(state) {
        Ok(raw) => validate_game_state_json(&raw, state),
        Err(error) => vec![format!("(root): {error}")],
    }
}

/// [`validate_game_state`] for a state parsed from `raw`: the `int()` refinements are checked on
/// the numbers as the JSON wrote them (parsing rounds them onto the integer grids), the rest on
/// the parsed state.
pub fn validate_game_state_json(raw: &Value, state: &GameState) -> Vec<String> {
    let mut errors = Vec::new();
    let mut error = |path: String, message: String| errors.push(format!("{path}: {message}"));
    // `path` names the error, `at` walks the raw JSON (absent keys took their defaults).
    let int = |path: String, at: &[&str], error: &mut dyn FnMut(String, String)| {
        let value = at.iter().try_fold(raw, |node, key| node.get(key)).and_then(Value::as_f64);
        if let Some(value) = value.filter(|value| !units::is_integer(*value)) {
            error(path, format!("Expected integer, received {}", units::format_number(value)));
        }
    };
    let one_of = |path: String, value: &str, allowed: &[&str], error: &mut dyn FnMut(String, String)| {
        if !allowed.contains(&value) {
            let expected: Vec<String> = allowed.iter().map(|a| format!("'{a}'")).collect();
            error(path, format!("Invalid enum value. Expected {}, received '{value}'", expected.join(" | ")));
        }
    };

    int("meta.saveVersion".into(), &["meta", "saveVersion"], &mut error);
    int("clock.tick".into(), &["clock", "tick"], &mut error);
    int("clock.day".into(), &["clock", "day"], &mut error);
    int("clock.year".into(), &["clock", "year"], &mut error);

    let player = &state.player;
    one_of("player.direction".into(), &player.direction, directions::ALL, &mut error);
    for (axis, value) in [("dx", player.move_intent.dx), ("dy", player.move_intent.dy)] {
        let path = format!("player.moveIntent.{axis}");
        int(path.clone(), &["player", "moveIntent", axis], &mut error);
        if value < -1 {
            error(path.clone(), "Number must be greater than or equal to -1".into());
        }
        if value > 1 {
            error(path, "Number must be less than or equal to 1".into());
        }
    }
    for name in player.skills.keys() {
        int(format!("player.skills.{name}.level"), &["player", "skills", name, "level"], &mut error);
    }

    for (id, npc) in &state.npcs {
        int(format!("npcs.{id}.x"), &["npcs", id, "x"], &mut error);
        int(format!("npcs.{id}.y"), &["npcs", id, "y"], &mut error);
        if npc.patrol_index.is_some() {
            int(format!("npcs.{id}.patrolIndex"), &["npcs", id, "patrolIndex"], &mut error);
        }
        for i in 0..npc.path.as_ref().map_or(0, Vec::len) {
            let index = i.to_string();
            int(format!("npcs.{id}.path.{i}.x"), &["npcs", id, "path", &index, "x"], &mut error);
            int(format!("npcs.{id}.path.{i}.y"), &["npcs", id, "path", &index, "y"], &mut error);
        }
    }
    for (id, quest) in &state.quests {
        one_of(format!("quests.{id}.status"), &quest.status, quest_statuses::ALL, &mut error);
    }

    int("mine.deepestFloor".into(), &["mine", "deepestFloor"], &mut error);
    int("mine.currentFloor".into(), &["mine", "currentFloor"], &mut error);

    if state.rng.algorithm != "xoshiro128ss" {
        error("rng.algorithm".into(), "Invalid literal value, expected \"xoshiro128ss\"".into());
    }

    errors.truncate(MAX_ERRORS);
    errors
}

// ---- parsing -----------------------------------------------------------------------------------

/// zod `safeParse` without the refinements: deserialize into [`GameState`] (defaults applied,
/// unknown keys dropped). Type errors are reported zod-style
/// (`player.money: Expected number, received string`).
fn parse_game_state(state: &Value) -> Result<GameState, String> {
    serde_path_to_error::deserialize::<_, GameState>(state).map_err(|error| {
        let path: Vec<String> = error
            .path()
            .iter()
            .filter_map(|segment| match segment {
                serde_path_to_error::Segment::Map { key } => Some(key.clone()),
                serde_path_to_error::Segment::Seq { index } => Some(index.to_string()),
                serde_path_to_error::Segment::Enum { variant } => Some(variant.clone()),
                serde_path_to_error::Segment::Unknown => None,
            })
            .collect();
        let found = path.iter().try_fold(state, |node, segment| match node {
            Value::Object(map) => map.get(segment),
            Value::Array(items) => segment.parse::<usize>().ok().and_then(|i| items.get(i)),
            _ => None,
        });
        let message = error.inner().to_string();
        let expected = expected_kind(&message);
        match (expected, found) {
            (Some(expected), Some(node)) => {
                format!("{}: Expected {expected}, received {}", path.join("."), kind_name(node))
            }
            _ => format!("{}: {message}", path.join(".")),
        }
    })
}

/// The zod type name serde expected, from its `invalid type: …, expected …` message.
fn expected_kind(message: &str) -> Option<&'static str> {
    let expected = message.split(", expected ").nth(1)?;
    Some(if expected.starts_with("f64") || expected.starts_with("u32") || expected.contains("number") {
        "number"
    } else if expected.contains("string") {
        "string"
    } else if expected.starts_with("a boolean") {
        "boolean"
    } else if expected.contains("sequence") || expected.contains("array") {
        "array"
    } else {
        "object"
    })
}

fn kind_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn fail(error: String) -> SaveLoadResult {
    SaveLoadResult { ok: false, data: None, errors: vec![error] }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn minimal_v5() -> Value {
        json!({
            "meta": { "saveVersion": 5, "engineSeed": "t", "packs": [] },
            "clock": { "tick": 0, "timeMinutes": 360, "day": 1, "season": "spring", "year": 1, "weatherId": "sun" },
            "world": { "scenes": [] },
            "player": { "x": 1.5, "y": 1.5, "moveIntent": { "dx": 0, "dy": 0 }, "direction": "down", "sceneId": "s",
                        "inventory": [], "maxInventorySize": 20, "money": 0, "energy": 100, "maxEnergy": 100,
                        "skills": {}, "activeQuests": [], "completedQuests": [] },
            "npcs": {}, "quests": {}, "dialogue": null, "shop": null, "minigame": null,
            "shopPurchasesToday": {}, "social": {}, "animals": [], "mine": { "deepestFloor": 0, "currentFloor": 0 },
            "flags": {}, "quarantinedItems": [], "rng": { "algorithm": "xoshiro128ss", "s": [1, 2, 3, 4] }
        })
    }

    #[test]
    fn a_current_save_loads() {
        let result = load_game_state(&minimal_v5());
        assert!(result.ok, "{:?}", result.errors);
        assert_eq!(result.data.expect("data").meta.save_version, 5);
    }

    #[test]
    fn values_are_quantized_onto_the_grids() {
        let mut raw = minimal_v5();
        raw["player"]["x"] = json!(1.00001);
        raw["player"]["energy"] = json!(97.25);
        raw["clock"]["timeMinutes"] = json!(390.05);
        let result = load_game_state(&raw);
        assert!(result.ok, "{:?}", result.errors);
        let data = result.data.expect("data");
        assert_eq!(data.player.x, 8192); // 1.00001 × 8192 = 8192.08 → 8192
        assert_eq!(data.player.energy, 97_250);
        assert_eq!(data.clock.time_minutes, 390_050_000);
        let written = serde_json::to_value(&data).expect("serializes");
        assert_eq!(written["player"]["x"], json!(1));
        assert_eq!(written["player"]["energy"], json!(97.25));
    }

    #[test]
    fn other_save_versions_are_refused() {
        let errors = |version: Value| {
            let mut raw = minimal_v5();
            raw["meta"]["saveVersion"] = version;
            load_game_state(&raw).errors
        };
        assert_eq!(errors(json!(4)), vec!["Save version 4 is not supported (this engine reads version 5)."]);
        assert_eq!(errors(json!(2.5)), vec!["Save version 2.5 is not supported (this engine reads version 5)."]);
        assert_eq!(errors(json!(6)), vec!["Save version 6 is newer than this engine supports (5). Update the engine."]);
        assert_eq!(errors(Value::Null), vec!["Save state has no meta.saveVersion (this engine reads version 5)."]);
        let mut raw = minimal_v5();
        raw["meta"] = json!("y".repeat(100_000));
        assert!(!load_game_state(&raw).ok);
        assert_eq!(load_game_state(&json!([1, 2, 3])).errors, vec!["Save state is not an object"]);
    }

    #[test]
    fn type_errors_are_reported_zod_style() {
        let mut raw = minimal_v5();
        raw["player"]["money"] = json!("lots");
        let result = load_game_state(&raw);
        assert!(!result.ok);
        assert_eq!(result.errors, vec!["player.money: Expected number, received string"]);
    }

    #[test]
    fn refinements_are_checked() {
        let mut raw = minimal_v5();
        raw["player"]["direction"] = json!("north");
        raw["player"]["moveIntent"]["dx"] = json!(2);
        raw["clock"]["day"] = json!(1.5);
        let result = load_game_state(&raw);
        assert!(!result.ok);
        assert_eq!(
            result.errors,
            vec![
                "clock.day: Expected integer, received 1.5",
                "player.direction: Invalid enum value. Expected 'up' | 'down' | 'left' | 'right', received 'north'",
                "player.moveIntent.dx: Number must be less than or equal to 1",
            ]
        );
    }

    #[test]
    fn invalid_json_text_fails_softly() {
        let result = load_game_state_json("{ not json");
        assert!(!result.ok);
        assert!(result.errors[0].starts_with("Save state is not valid JSON: "));
    }

    #[test]
    fn a_string_where_an_object_belongs_is_a_type_error() {
        let mut raw = minimal_v5();
        raw["clock"] = json!("x".repeat(100_000));
        let result = load_game_state_owned(raw);
        assert!(!result.ok);
        assert_eq!(result.errors, vec!["clock: Expected object, received string"]);
    }
}
