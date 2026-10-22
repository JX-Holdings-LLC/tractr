//! Loading save states against the goldens: `fixtures/golden/saves` holds a current (v5) save, an
//! older (v4) and a future (v6) one and malformed ones, each with what this engine makes of it
//! (`FARM_RECORD_GOLDENS=1` rewrites the results, keeping the inputs).

use farm_cart::load_game_state;
use farm_sim::{hash_state, stable_stringify};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[path = "../../farm-sim/tests/recording/mod.rs"]
mod recording;

static RECORDER: recording::Recorder = recording::Recorder::new("FARM_RECORD_GOLDENS", "saves");

fn golden_dir(relative: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", "..", "fixtures", "golden", relative].iter().collect()
}

fn fixtures(dir: &Path) -> Vec<(String, Value)> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("list {}: {e}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let name = path.file_stem().expect("stem").to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&path).expect("read fixture");
            (name, serde_json::from_str(&text).expect("valid fixture JSON"))
        })
        .collect()
}

fn record(input: &Value) -> Value {
    let result = load_game_state(input);
    let mut fixture = json!({
        "input": input,
        "result": {
            "ok": result.ok,
            "errors": result.errors,
        },
    });
    if let Some(data) = &result.data {
        fixture["stable"] = Value::String(stable_stringify(data));
        fixture["hash"] = Value::String(hash_state(data));
    }
    fixture
}

#[test]
fn saves_load_as_recorded() {
    let dir = golden_dir("saves");
    let recorded = fixtures(&dir);
    assert!(recorded.iter().any(|(name, _)| name == "save-v5-current"), "expected the current save golden");
    if RECORDER.enabled() {
        for (name, fixture) in &recorded {
            let text = serde_json::to_string_pretty(&record(&fixture["input"])).expect("encode") + "\n";
            RECORDER.write(&dir.join(format!("{name}.json")), text.as_bytes()).expect("write golden");
        }
        RECORDER.finish();
    }
    let mut failures = Vec::new();
    for (name, fixture) in &recorded {
        let actual = record(&fixture["input"]);
        if &actual != fixture {
            failures.push(format!(
                "{name}: differs from the recorded golden\nrecorded: {}\nnow:      {}",
                fixture["result"], actual["result"]
            ));
            if actual.get("hash") != fixture.get("hash") {
                failures.push(format!("{name}: hash {:?} vs recorded {:?}", actual.get("hash"), fixture.get("hash")));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
