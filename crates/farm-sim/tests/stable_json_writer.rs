//! The stable JSON writer (`stable_json::stringify`, straight from `Serialize`) writes exactly the
//! text of the reference implementation (`stable_json::stringify_via_value`: `serde_json::to_value`,
//! then the tree with sorted keys), on every golden fixture, on the states, effects, content and
//! projects of every golden replay, on random play and on random JSON values.

mod strategies;

use farm_sim::replay::{self, ReplayInput};
use farm_sim::schema::GameProject;
use farm_sim::{engine, stable_json, state, CommandRules, EngineContext, HookBus};
use proptest::prelude::*;
use serde::Serialize;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

fn golden_dir() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", "..", "fixtures", "golden"].iter().collect()
}

fn json_files(dir: &Path, found: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> =
        std::fs::read_dir(dir).expect("fixture folder").map(|e| e.unwrap().path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            json_files(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "json") {
            found.push(path);
        }
    }
}

fn read_json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

/// The writer's text of `value`, checked against the reference.
fn same_text<T: Serialize + ?Sized>(what: &str, value: &T) -> String {
    let fast = stable_json::stringify(value);
    let reference = stable_json::stringify_via_value(value);
    assert!(
        fast == reference,
        "{what}: the writer and the reference differ\n writer: {fast:.300}\n   ref.: {reference:.300}"
    );
    fast
}

#[test]
fn every_golden_fixture_writes_like_the_reference() {
    let mut files = Vec::new();
    json_files(&golden_dir(), &mut files);
    assert!(files.len() > 40, "found the fixtures ({})", files.len());
    let mut checked = 0;
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        // The v8 primitives hold lone surrogates (`"\ud800"`), which serde_json cannot read.
        let Ok(value) = serde_json::from_str::<Value>(&text) else { continue };
        checked += 1;
        let text = same_text(&path.display().to_string(), &value);
        assert_eq!(text, stable_json::stringify_value_tree(&value), "{}", path.display());
        assert_eq!(text, stable_json::stringify_value(&value), "{}", path.display());
    }
    assert!(checked > 40, "checked the fixtures ({checked})");
}

#[test]
fn golden_replays_write_like_the_reference_at_every_step() {
    let mut files = Vec::new();
    json_files(&golden_dir().join("replays"), &mut files);
    assert!(files.len() > 20, "found the replays ({})", files.len());
    for path in files {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let source = read_json(&path);
        if source["project"].is_null() {
            continue; // index.json
        }
        let project: GameProject = serde_json::from_value(source["project"].clone()).expect("project");
        let content = state::create_content_from_project(&project);
        same_text(&format!("{name} project"), &project);
        same_text(&format!("{name} content"), &content);
        let ctx = EngineContext::new(content).with_rules(CommandRules::Scripted);
        let mut game = state::create_game_state(&project, source["seed"].as_str());
        same_text(&format!("{name} initial state"), &game);
        for (index, step) in source["steps"].as_array().expect("steps").iter().enumerate() {
            let input: ReplayInput = serde_json::from_value(step["input"].clone()).expect("input");
            same_text(&format!("{name} step {index} input"), &input);
            let effects = match &input {
                ReplayInput::Command { command } => engine::apply_command(&ctx, &mut game, command),
                ReplayInput::Tick { ticks } => engine::advance_tick(&ctx, &mut game, *ticks),
            };
            same_text(&format!("{name} step {index} effects"), &effects);
            same_text(&format!("{name} step {index} state"), &game);
        }
        let kept = state::apply_state_to_project_json(&source["project"], &game).expect("keep changes");
        same_text(&format!("{name} final project"), &kept);
    }
}

static STARTER: LazyLock<(GameProject, farm_sim::GameContent, strategies::Ids)> = LazyLock::new(|| {
    let project = strategies::golden_project("starter-farm");
    let content = state::create_content_from_project(&project);
    let ids = strategies::Ids::of(&project);
    (project, content, ids)
});

/// A JSON key: array indices and near-indices, escapes, non-ASCII on both sides of the
/// surrogate range, and keys that share long prefixes.
fn key() -> impl Strategy<Value = String> {
    prop_oneof![
        prop::sample::select(vec![
            "0",
            "1",
            "2",
            "10",
            "01",
            "-1",
            "4294967294",
            "4294967295",
            "1e3",
            "",
            "a",
            "A",
            "ab",
            "abcdefg",
            "abcdefgh",
            "abcdefghi",
            "abcdefgZ",
            "abcdefgAA",
            "\u{0}",
            "a\u{0}",
            "\"",
            "\\",
            "\n",
            "\u{7f}",
            "\u{e9}",
            "\u{FFFF}",
            "\u{1F600}",
            "type",
            "soilState",
            "soilStatus",
            "abcdefg\u{FFFF}",
            "abcdefg\u{1F600}",
            "abcdefg\u{e9}",
        ])
        .prop_map(str::to_owned),
        "[a-c0-9]{0,10}",
        "\\PC{0,6}",
        "[\\x00-\\x20\"\\\\a\u{e000}-\u{e002}\u{10000}]{0,4}",
    ]
}

/// Any JSON value serde_json can hold: finite doubles of every size, 64-bit integers of both
/// signs, strings with escapes, nested arrays and objects.
fn json_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::from),
        any::<u64>().prop_map(Value::from),
        any::<f64>().prop_map(|x| serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number)),
        prop::sample::select(vec![0.1, -0.0, 1e21, 1e-7, 123456789.125, 2.5, 5e-324, f64::MAX])
            .prop_map(|x| serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number)),
        key().prop_map(Value::String),
    ];
    leaf.prop_recursive(4, 48, 8, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            prop::collection::vec((key(), inner), 0..8)
                .prop_map(|entries| Value::Object(entries.into_iter().collect())),
        ]
    })
}

/// A map serialized with duplicate keys (later ones win, as in a JS object) and mixed key types.
#[derive(Debug)]
struct Entries(Vec<(Key, Value)>);

#[derive(Debug, Clone)]
enum Key {
    Text(String),
    Int(i64),
    Char(char),
}

impl Serialize for Key {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Key::Text(text) => serializer.serialize_str(text),
            Key::Int(n) => serializer.serialize_i64(*n),
            Key::Char(c) => serializer.serialize_char(*c),
        }
    }
}

impl Serialize for Entries {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

fn entries() -> impl Strategy<Value = Entries> {
    let key = prop_oneof![
        key().prop_map(Key::Text),
        prop::sample::select(vec![0i64, 1, 2, 10, -1, 4294967294, 4294967295, i64::MIN]).prop_map(Key::Int),
        prop::sample::select(vec!['0', 'a', '"', '\u{1F600}']).prop_map(Key::Char),
    ];
    prop::collection::vec((key, json_value()), 0..12).prop_map(Entries)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    #[test]
    fn random_play_writes_like_the_reference(
        seed in strategies::seed(),
        inputs in strategies::inputs(&STARTER.2, 40, 400),
    ) {
        let (project, content, _) = &*STARTER;
        let ctx = EngineContext::with_hooks(content.clone(), HookBus::new());
        let mut game = state::create_game_state(project, Some(&seed));
        let result = replay::run_replay(&ctx, &mut game, &inputs);
        same_text("inputs", &inputs);
        same_text("effects", &result.effects);
        same_text("state", &game);
        same_text("hook events", &ctx.drain_hook_events());
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    #[test]
    fn random_json_writes_like_the_reference(value in json_value()) {
        let text = same_text("value", &value);
        prop_assert_eq!(text, stable_json::stringify_value_tree(&value));
    }

    #[test]
    fn maps_with_duplicate_and_typed_keys_write_like_the_reference(entries in entries()) {
        same_text("entries", &entries);
        let nested: Map<String, Value> = [("x".to_owned(), Value::Null)].into_iter().collect();
        same_text("nested", &(&entries, &nested, [&entries]));
    }
}
