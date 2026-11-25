//! The headless run (`farm-player --headless`, #222): load a cartridge, start a new game (or load a
//! save), put it in a test state, replay commands, ticks and debug shortcuts, check a hash and the
//! replay's expectations, and report the outcome.
//!
//! A replay file:
//!
//! ```json
//! { "seed": "s", "autoStartQuests": true,
//!   "testState": { "id": "late", "day": 20, "sceneId": "town", "x": 4, "y": 6 },
//!   "inputs": [
//!     { "kind": "tick", "ticks": 20 },
//!     { "kind": "command", "command": { "type": "sleep" } },
//!     { "kind": "debug", "action": { "type": "giveItem", "itemId": "wood", "quantity": 10 } },
//!     { "kind": "debug", "action": { "type": "skipDay", "days": 3 } }
//!   ],
//!   "expect": [
//!     { "quest": "quest-roots", "status": "completed" },
//!     { "flag": "mill-fixed", "value": true },
//!     { "variable": "bridges", "value": 2 },
//!     { "money": 500 }, { "minMoney": 100 },
//!     { "item": "wood", "quantity": 10 },
//!     { "scene": "town" }
//!   ],
//!   "expectedHash": "…" }
//! ```
//!
//! Everything stays deterministic: debug shortcuts and test states change the state directly,
//! without randomness, so a replay that uses them reproduces its hash.
//!
//! The run goes through [`PlaySession`], like the windowed game and Play Mode: a cartridge's
//! project scripts and pack plugins hear the same hooks and their mutations apply at the same
//! points, so a scripted game's replay tests the rules the game ships with. A cartridge without
//! plugins replays exactly as the engine alone does ([`run_inputs`]), which keeps the
//! simulation-only golden replays reproducible.

use crate::session::PlaySession;
use farm_cart::save_file::{self, SaveTarget};
use farm_plugins::PluginHostOptions;
use farm_sim::effects::Effect;
use farm_sim::replay::ReplayInput;
use farm_sim::schema::{GameState, TestState};
use farm_sim::test_states::{self, DebugOp};
use farm_sim::{engine, hash, state, units, EngineContext};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One entry of a headless replay: an engine input (as in the golden replays) or a debug shortcut.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum HeadlessInput {
    Engine(ReplayInput),
    Debug(DebugInput),
}

/// `{"kind":"debug","action":{…}}`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebugInput {
    pub kind: DebugKind,
    pub action: DebugOp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DebugKind {
    Debug,
}

impl From<ReplayInput> for HeadlessInput {
    fn from(input: ReplayInput) -> Self {
        Self::Engine(input)
    }
}

impl From<DebugOp> for HeadlessInput {
    fn from(action: DebugOp) -> Self {
        Self::Debug(DebugInput { kind: DebugKind::Debug, action })
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplayFile {
    pub seed: Option<String>,
    #[serde(default)]
    pub auto_start_quests: bool,
    /// Where the game starts: applied after New Game (or after loading a save).
    pub test_state: Option<TestState>,
    #[serde(default)]
    pub inputs: Vec<HeadlessInput>,
    pub expected_hash: Option<String>,
    /// What must hold at the end; a run that misses one fails.
    #[serde(default)]
    pub expect: Vec<Value>,
}

impl ReplayFile {
    /// Reads a replay file, with a message that says which entry is wrong.
    pub fn parse(json: &str) -> Result<Self, String> {
        let raw: Value = serde_json::from_str(json).map_err(|error| error.to_string())?;
        if let Some(inputs) = raw.get("inputs").and_then(Value::as_array) {
            for (index, input) in inputs.iter().enumerate() {
                if let Err(error) = serde_json::from_value::<HeadlessInput>(input.clone()) {
                    let detail = match input.get("kind").and_then(Value::as_str) {
                        Some("debug") => serde_json::from_value::<DebugInput>(input.clone())
                            .err()
                            .map_or_else(|| error.to_string(), |e| e.to_string()),
                        Some(_) => serde_json::from_value::<ReplayInput>(input.clone())
                            .err()
                            .map_or_else(|| error.to_string(), |e| e.to_string()),
                        None => "an input needs a kind: command, tick or debug".to_owned(),
                    };
                    return Err(format!("inputs[{index}]: {detail}"));
                }
            }
        }
        let file: Self = serde_json::from_value(raw).map_err(|error| error.to_string())?;
        for (index, expectation) in file.expect.iter().enumerate() {
            Expectation::parse(expectation).map_err(|error| format!("expect[{index}]: {error}"))?;
        }
        Ok(file)
    }
}

/// What a replay expects at its end (`expect`).
#[derive(Debug, Clone, PartialEq)]
pub enum Expectation {
    Quest {
        quest: String,
        status: String,
    },
    Flag {
        flag: String,
        value: Value,
    },
    Variable {
        variable: String,
        value: Value,
    },
    Money {
        money: i64,
    },
    MinMoney {
        money: i64,
    },
    /// At least `quantity` of the item in the inventory.
    Item {
        item: String,
        quantity: u32,
    },
    Scene {
        scene: String,
    },
}

impl Expectation {
    pub fn parse(value: &Value) -> Result<Self, String> {
        let object = value.as_object().ok_or("an expectation is an object")?;
        let text = |key: &str| -> Result<String, String> {
            object.get(key).and_then(Value::as_str).map(str::to_owned).ok_or_else(|| format!("'{key}' must be a text"))
        };
        let only = |keys: &[&str]| -> Result<(), String> {
            match object.keys().find(|key| !keys.contains(&key.as_str())) {
                Some(key) => Err(format!("unknown key '{key}' (with {})", keys.join(", "))),
                None => Ok(()),
            }
        };
        let whole = |key: &str| -> Result<i64, String> {
            object.get(key).and_then(units::json_int).ok_or_else(|| format!("'{key}' must be a whole number"))
        };
        if object.contains_key("quest") {
            only(&["quest", "status"])?;
            let status = text("status")?;
            if !farm_sim::schema::quest_statuses::ALL.contains(&status.as_str()) {
                return Err(format!(
                    "status '{status}' is not one of {}",
                    farm_sim::schema::quest_statuses::ALL.join(", ")
                ));
            }
            return Ok(Self::Quest { quest: text("quest")?, status });
        }
        if object.contains_key("flag") {
            only(&["flag", "value"])?;
            return Ok(Self::Flag {
                flag: text("flag")?,
                value: object.get("value").cloned().unwrap_or(Value::Bool(true)),
            });
        }
        if object.contains_key("variable") {
            only(&["variable", "value"])?;
            let value = object.get("value").cloned().ok_or("'value' is missing")?;
            return Ok(Self::Variable { variable: text("variable")?, value });
        }
        if object.contains_key("money") {
            only(&["money"])?;
            return Ok(Self::Money { money: whole("money")? });
        }
        if object.contains_key("minMoney") {
            only(&["minMoney"])?;
            return Ok(Self::MinMoney { money: whole("minMoney")? });
        }
        if object.contains_key("item") {
            only(&["item", "quantity"])?;
            let quantity = match object.get("quantity") {
                None => 1,
                Some(_) => u32::try_from(whole("quantity")?).map_err(|_| "'quantity' must not be negative")?,
            };
            return Ok(Self::Item { item: text("item")?, quantity });
        }
        if object.contains_key("scene") {
            only(&["scene"])?;
            return Ok(Self::Scene { scene: text("scene")? });
        }
        Err("an expectation names a quest, flag, variable, money, minMoney, item or scene".to_owned())
    }

    /// Why `state` misses this expectation, or `None` when it holds.
    pub fn check(&self, state: &GameState) -> Option<String> {
        match self {
            Self::Quest { quest, status } => {
                let actual = test_states::quest_status(state, quest);
                (actual != status).then(|| format!("quest '{quest}' is {actual}, expected {status}"))
            }
            Self::Flag { flag, value } => {
                let actual = state.flags.get(flag);
                let holds = match value {
                    Value::Bool(expected) => farm_sim::text::truthy(actual) == *expected,
                    expected => actual.is_some_and(|actual| same_value(actual, expected)),
                };
                (!holds).then(|| format!("flag '{flag}' is {}, expected {value}", shown(actual)))
            }
            Self::Variable { variable, value } => {
                let actual = state.flags.get(variable);
                (!actual.is_some_and(|actual| same_value(actual, value)))
                    .then(|| format!("variable '{variable}' is {}, expected {value}", shown(actual)))
            }
            Self::Money { money } => {
                let actual = state.player.money;
                (actual != *money).then(|| format!("money is {actual}, expected {money}"))
            }
            Self::MinMoney { money } => {
                let actual = state.player.money;
                (actual < *money).then(|| format!("money is {actual}, expected at least {money}"))
            }
            Self::Item { item, quantity } => {
                let actual: u32 = state
                    .player
                    .inventory
                    .iter()
                    .filter(|slot| &slot.item.id == item)
                    .map(|slot| slot.quantity)
                    .fold(0, u32::saturating_add);
                (actual < *quantity)
                    .then(|| format!("the inventory holds {actual} of '{item}', expected at least {quantity}"))
            }
            Self::Scene { scene } => {
                let actual = &state.player.scene_id;
                (actual != scene).then(|| format!("the player is in scene '{actual}', expected '{scene}'"))
            }
        }
    }
}

fn same_value(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => units::milli_of_number(a) == units::milli_of_number(b),
        _ => actual == expected,
    }
}

fn shown(value: Option<&Value>) -> String {
    value.map_or_else(|| "unset".to_owned(), Value::to_string)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadlessReport {
    pub game_id: String,
    pub game_version: String,
    pub content_hash: String,
    pub state_hash: String,
    pub effects: Vec<Effect>,
    pub warnings: Vec<String>,
    /// Where the run ended: day, season, year, time, scene and tile, money.
    pub clock: ReportClock,
    /// Every quest of the game with its status.
    pub quests: IndexMap<String, String>,
    /// Flags holding `true` or `false`.
    pub flags: IndexMap<String, Value>,
    /// Flags holding a number or a text.
    pub variables: IndexMap<String, Value>,
    pub money: i64,
    /// What the test state and the debug shortcuts could not apply (content the game lacks).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// What the game's scripts and plugins got wrong during the run (a failed load, a throw, an
    /// overrun, a refused mutation), oldest first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub plugin_errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportClock {
    pub day: u32,
    pub season: String,
    pub day_of_season: u32,
    pub year: u32,
    pub minute: u32,
    pub scene_id: String,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone)]
pub struct HeadlessRun {
    pub report: HeadlessReport,
    /// The final state as a JSON save (debugging).
    pub save_json: String,
    /// The final state as a binary `FGSV` save (what the player writes to its save slots).
    pub save_binary: Vec<u8>,
}

/// Runs `inputs` on `state` in order and returns the final hash, the effects and the notes of
/// the debug shortcuts. Engine inputs run exactly as [`farm_sim::replay::run_replay`] runs them.
pub fn run_inputs(
    ctx: &EngineContext,
    state: &mut GameState,
    inputs: &[HeadlessInput],
) -> (String, Vec<Effect>, Vec<String>) {
    let mut effects = Vec::new();
    let mut notes = Vec::new();
    for input in inputs {
        match input {
            HeadlessInput::Engine(ReplayInput::Tick { ticks }) => {
                effects.extend(engine::advance_tick(ctx, state, *ticks))
            }
            HeadlessInput::Engine(ReplayInput::Command { command }) => {
                effects.extend(engine::apply_command(ctx, state, command));
            }
            HeadlessInput::Debug(debug) => {
                let (more, more_notes) = test_states::apply_debug(ctx, state, &debug.action);
                effects.extend(more);
                notes.extend(more_notes);
            }
        }
    }
    (hash::hash_state(state), effects, notes)
}

/// Runs `inputs` on a session in order, as the game runs them: one tick at a time with the plugin
/// mutations before each, commands and debug shortcuts as steps plugins hear, and everything
/// plugins answered applied by the end. Returns the final hash, the effects and the notes of the
/// debug shortcuts. Without plugins this is [`run_inputs`], step for step.
pub fn run_session_inputs(session: &mut PlaySession, inputs: &[HeadlessInput]) -> (String, Vec<Effect>, Vec<String>) {
    let mut notes = Vec::new();
    session.record_effects();
    for input in inputs {
        match input {
            HeadlessInput::Engine(ReplayInput::Tick { ticks }) => session.step_ticks(*ticks),
            HeadlessInput::Engine(ReplayInput::Command { command }) => session.run_command(command),
            HeadlessInput::Debug(debug) => notes.extend(session.apply_debug_op(&debug.action)),
        }
    }
    // The save and the report hold what plugins answered to the last step too.
    session.settle();
    (hash::hash_state(session.state()), session.take_effects(), notes)
}

/// Run a compiled cartridge with optional replay and incoming save, through the same session the
/// windowed game runs: the cartridge's project scripts and pack plugins hear every step and
/// their mutations apply, so a scripted game replays with its own rules. A bad cartridge, save,
/// replay, hash expectation or missed `expect` entry is an error; the caller decides how to
/// report it.
pub fn run_cartridge(bytes: &[u8], replay: &ReplayFile, load_save: Option<&[u8]>) -> Result<HeadlessRun, String> {
    run_loaded(farm_cart::load_cartridge(bytes)?, replay, load_save)
}

/// [`run_cartridge`] for a cartridge already read.
pub fn run_loaded(
    cart: farm_cart::LoadedCartridge,
    replay: &ReplayFile,
    load_save: Option<&[u8]>,
) -> Result<HeadlessRun, String> {
    let target = SaveTarget::for_cartridge(&cart);
    let content_hash = hash::hash_state(&cart.content);
    let scripts = cart.content.settings.scripts.clone();

    let (mut session, warnings) = if let Some(save) = load_save {
        let loaded = save_file::load_save_bytes(save, &target, &cart.content);
        if !loaded.ok {
            return Err(loaded.errors.join("\n"));
        }
        let state = loaded.state.ok_or("Save loaded without a game state.")?;
        (PlaySession::new(cart.content, state), loaded.warnings)
    } else {
        let initial = state::create_game_state_from_start(&cart.start, replay.seed.as_deref());
        let mut session = PlaySession::new(cart.content, initial);
        if replay.auto_start_quests {
            session.auto_start_quests();
        }
        (session, Vec::new())
    };
    // Plugins attach as in the game: `onLoad` with their saved data, then the auto-start's hooks.
    session.set_plugins(crate::plugins::for_cartridge(&cart.plugins, PluginHostOptions::default(), scripts.as_ref()));
    let mut notes = match &replay.test_state {
        Some(test) => session.apply_test_state(test),
        None => Vec::new(),
    };

    let (state_hash, effects, input_notes) = run_session_inputs(&mut session, &replay.inputs);
    notes.extend(input_notes);
    let plugin_errors = session.plugin_errors();
    let game_state = session.state();
    if let Some(expected) = &replay.expected_hash {
        if *expected != state_hash {
            return Err(format!("Replay hash mismatch: expected {expected}, got {state_hash}."));
        }
    }
    let failures: Vec<String> = replay
        .expect
        .iter()
        .enumerate()
        .filter_map(|(index, raw)| match Expectation::parse(raw) {
            Ok(expectation) => expectation.check(game_state).map(|why| format!("expect[{index}]: {why}")),
            Err(error) => Some(format!("expect[{index}]: {error}")),
        })
        .collect();
    if !failures.is_empty() {
        return Err(format!(
            "{} of {} expectations failed:\n  {}",
            failures.len(),
            replay.expect.len(),
            failures.join("\n  ")
        ));
    }
    let save_json = save_file::write_save(game_state, &target);
    let mut preview = save_file::SavePreview::of_state(game_state);
    preview.farm_name = cart.info.title.clone();
    let save_binary = save_file::write_save_binary(game_state, &target, &preview);
    let mut report = report(session.context(), game_state, target, content_hash, state_hash, effects, warnings, notes);
    report.plugin_errors = plugin_errors;
    Ok(HeadlessRun { report, save_json, save_binary })
}

#[allow(clippy::too_many_arguments)]
fn report(
    ctx: &EngineContext,
    state: &GameState,
    target: SaveTarget,
    content_hash: String,
    state_hash: String,
    effects: Vec<Effect>,
    warnings: Vec<String>,
    notes: Vec<String>,
) -> HeadlessReport {
    let (mut flags, mut variables) = (IndexMap::new(), IndexMap::new());
    for (flag, value) in &state.flags {
        match value {
            Value::Bool(_) => flags.insert(flag.clone(), value.clone()),
            _ => variables.insert(flag.clone(), value.clone()),
        };
    }
    let quests = ctx
        .content
        .quests
        .iter()
        .map(|quest| (quest.id.clone(), test_states::quest_status(state, &quest.id).to_owned()))
        .collect();
    let clock = &state.clock;
    HeadlessReport {
        game_id: target.game_id,
        game_version: target.game_version,
        content_hash,
        state_hash,
        effects,
        warnings,
        clock: ReportClock {
            day: clock.day,
            season: clock.season.clone(),
            day_of_season: clock.day_of_season,
            year: clock.year,
            minute: units::whole_minute(clock.time_minutes),
            scene_id: state.player.scene_id.clone(),
            x: units::tile_of(state.player.x),
            y: units::tile_of(state.player.y),
        },
        quests,
        flags,
        variables,
        money: state.player.money,
        notes,
        plugin_errors: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use farm_sim::replay::ReplayInput;

    const CART: &[u8] = include_bytes!("../../../fixtures/golden/cartridges/fixture-project.cart");
    const QUESTS: &[u8] = include_bytes!("../../../fixtures/golden/cartridges/quest-rpg.cart");

    #[test]
    fn replay_and_save_are_deterministic() {
        let replay = ReplayFile { inputs: vec![ReplayInput::Tick { ticks: 20 }.into()], ..ReplayFile::default() };
        let first = run_cartridge(CART, &replay, None).unwrap();
        let second = run_cartridge(CART, &replay, None).unwrap();
        assert_eq!(first.report.state_hash, second.report.state_hash);
        assert_eq!(first.save_json, second.save_json);
        assert_eq!(first.report.game_id, "local.project-1");
        let loaded = run_cartridge(CART, &ReplayFile::default(), Some(first.save_json.as_bytes())).unwrap();
        assert_eq!(first.report.state_hash, loaded.report.state_hash);
        assert_eq!(first.save_binary, second.save_binary);
        let binary = run_cartridge(CART, &ReplayFile::default(), Some(&first.save_binary)).unwrap();
        assert_eq!(first.report.state_hash, binary.report.state_hash);
    }

    #[test]
    fn engine_inputs_hash_as_the_golden_replays_do() {
        let cart = farm_cart::load_cartridge(CART).unwrap();
        let ctx = EngineContext::new(cart.content.clone());
        let inputs = vec![
            ReplayInput::Tick { ticks: 30 },
            ReplayInput::Command { command: farm_sim::Command::Sleep },
            ReplayInput::Tick { ticks: 5 },
        ];
        let mut a = state::create_game_state_from_start(&cart.start, Some("s"));
        let mut b = a.clone();
        let golden = farm_sim::replay::run_replay(&ctx, &mut a, &inputs);
        let headless: Vec<HeadlessInput> = inputs.into_iter().map(Into::into).collect();
        let (hash, effects, notes) = run_inputs(&ctx, &mut b, &headless);
        assert_eq!(golden.hash, hash);
        assert_eq!(golden.effects, effects);
        assert!(notes.is_empty());
    }

    #[test]
    fn rejects_bad_hash_and_wrong_game_save() {
        let replay = ReplayFile { expected_hash: Some("wrong".to_owned()), ..ReplayFile::default() };
        assert!(run_cartridge(CART, &replay, None).unwrap_err().contains("Replay hash mismatch"));
        let save = run_cartridge(CART, &ReplayFile::default(), None).unwrap().save_json;
        let wrong = save.replace("local.project-1", "another.game");
        assert!(run_cartridge(CART, &ReplayFile::default(), Some(wrong.as_bytes()))
            .unwrap_err()
            .contains("different game"));
    }

    #[test]
    fn the_report_lists_quests_flags_and_variables() {
        let replay = ReplayFile::parse(
            r#"{"autoStartQuests":true,"inputs":[
                {"kind":"debug","action":{"type":"setFlag","flag":"mill-fixed"}},
                {"kind":"debug","action":{"type":"setFlag","flag":"bridges","value":2}},
                {"kind":"debug","action":{"type":"setMoney","amount":1234}}]}"#,
        )
        .unwrap();
        let run = run_cartridge(QUESTS, &replay, None).unwrap();
        let report = serde_json::to_value(&run.report).unwrap();
        assert_eq!(report["flags"]["mill-fixed"], true);
        assert_eq!(report["variables"]["bridges"], 2);
        assert_eq!(report["money"], 1234);
        let quests = report["quests"].as_object().unwrap();
        assert!(!quests.is_empty());
        assert!(quests.values().any(|status| status == "active"), "{quests:?}");
    }

    #[test]
    fn expectations_fail_the_run_with_what_was_found() {
        let cart = farm_cart::load_cartridge(QUESTS).unwrap();
        let quest = cart.content.quests[0].id.clone();
        let json = format!(
            r#"{{"inputs":[{{"kind":"debug","action":{{"type":"setQuest","questId":"{quest}","status":"completed"}}}}],
                "expect":[{{"quest":"{quest}","status":"completed"}},{{"flag":"mill-fixed","value":true}},
                          {{"money":5}}]}}"#
        );
        let error = run_cartridge(QUESTS, &ReplayFile::parse(&json).unwrap(), None).unwrap_err();
        assert!(error.starts_with("2 of 3 expectations failed"), "{error}");
        assert!(error.contains("expect[1]: flag 'mill-fixed' is unset, expected true"), "{error}");
        assert!(error.contains("expect[2]: money is"), "{error}");
        assert!(!error.contains("expect[0]"), "{error}");

        let json = format!(
            r#"{{"testState":{{"id":"t","quests":{{"{quest}":"completed"}},"money":5}},
            "expect":[{{"quest":"{quest}","status":"completed"}},{{"money":5}},{{"minMoney":1}}]}}"#
        );
        run_cartridge(QUESTS, &ReplayFile::parse(&json).unwrap(), None).unwrap();
    }

    #[test]
    fn bad_entries_say_where_they_are() {
        let error =
            ReplayFile::parse(r#"{"inputs":[{"kind":"tick","ticks":1},{"kind":"debug","action":{"type":"nope"}}]}"#)
                .unwrap_err();
        assert!(error.starts_with("inputs[1]:"), "{error}");
        let error = ReplayFile::parse(r#"{"expect":[{"quest":"q","status":"done"}]}"#).unwrap_err();
        assert!(error.contains("expect[0]: status 'done'"), "{error}");
        let error = ReplayFile::parse(r#"{"expect":[{"flag":"f","valu":1}]}"#).unwrap_err();
        assert!(error.contains("unknown key 'valu'"), "{error}");
        // The golden replays' shape still reads.
        ReplayFile::parse(r#"{"inputs":[{"kind":"tick","ticks":20},{"kind":"command","command":{"type":"sleep"}}]}"#)
            .unwrap();
    }

    #[test]
    fn debug_shortcuts_reach_late_states_deterministically() {
        let cart = farm_cart::load_cartridge(CART).unwrap();
        let item = cart.content.items[0].id.clone();
        let scene = cart.content.scenes[0].id.clone();
        let (x, y) = ((cart.content.scenes[0].width - 1).min(1), (cart.content.scenes[0].height - 1).min(2));
        let json = format!(
            r#"{{"seed":"x","inputs":[
                {{"kind":"debug","action":{{"type":"giveItem","itemId":"{item}","quantity":3}}}},
                {{"kind":"debug","action":{{"type":"skipDay","days":3}}}},
                {{"kind":"debug","action":{{"type":"teleport","sceneId":"{scene}","x":{x},"y":{y},"facing":"up"}}}},
                {{"kind":"debug","action":{{"type":"addMoney","amount":5}}}}],
              "expect":[{{"item":"{item}","quantity":3}},{{"scene":"{scene}"}}]}}"#
        );
        let replay = ReplayFile::parse(&json).unwrap();
        let first = run_cartridge(CART, &replay, None).unwrap();
        let second = run_cartridge(CART, &replay, None).unwrap();
        assert_eq!(first.report.state_hash, second.report.state_hash);
        assert_eq!(first.report.clock.day, 4);
        assert_eq!((first.report.clock.x, first.report.clock.y), (x, y));
        assert!(first.report.notes.is_empty(), "{:?}", first.report.notes);
    }

    /// The fixture game with a project script that gives a carp every morning (the roadmap's
    /// Willow Lane check).
    fn scripted() -> (farm_cart::LoadedCartridge, String) {
        let mut cart = farm_cart::load_cartridge(CART).unwrap();
        let item = cart.content.items[0].id.clone();
        let source = format!(
            "api.on('onDayStart', function () {{ return [{{ type: 'giveItem', itemId: '{item}', quantity: 1 }}]; }});"
        );
        let spec = farm_plugins::script_spec("carp", &source);
        cart.plugins.push(farm_cart::CartPlugin {
            id: spec.id,
            pack_id: spec.pack_id,
            source: spec.source,
            granted_hooks: spec.granted_hooks,
            granted_mutations: spec.granted_mutations,
            granted_minigames: spec.granted_minigames,
            granted_reads: spec.granted_reads,
        });
        (cart, item)
    }

    fn held(save_json: &str, item: &str) -> u64 {
        let save: Value = serde_json::from_str(save_json).unwrap();
        let slots = save["state"]["player"]["inventory"].as_array().unwrap();
        slots.iter().filter(|slot| slot["item"]["id"] == item).filter_map(|slot| slot["quantity"].as_u64()).sum()
    }

    #[test]
    fn a_scripted_morning_replays_with_the_scripts_rules() {
        let (cart, item) = scripted();
        let before = held(&run_loaded(cart.clone(), &ReplayFile::default(), None).unwrap().save_json, &item);
        let sleep = r#"{"kind":"command","command":{"type":"sleep"}}"#;
        // The save written right after the sleep already holds the morning's carp …
        let morning = ReplayFile::parse(&format!(r#"{{"inputs":[{sleep}]}}"#)).unwrap();
        let first = run_loaded(cart.clone(), &morning, None).unwrap();
        assert!(first.report.plugin_errors.is_empty(), "{:?}", first.report.plugin_errors);
        assert_eq!(held(&first.save_json, &item), before + 1);
        // … exactly once however long the run goes on, and however often the save is loaded.
        let later = ReplayFile::parse(&format!(r#"{{"inputs":[{sleep},{{"kind":"tick","ticks":20}}]}}"#)).unwrap();
        let uninterrupted = run_loaded(cart.clone(), &later, None).unwrap();
        assert_eq!(held(&uninterrupted.save_json, &item), before + 1);
        let ticks = ReplayFile::parse(r#"{"inputs":[{"kind":"tick","ticks":20}]}"#).unwrap();
        for save in [first.save_json.as_bytes(), &first.save_binary[..]] {
            let reloaded = run_loaded(cart.clone(), &ReplayFile::default(), Some(save)).unwrap();
            assert_eq!(held(&reloaded.save_json, &item), before + 1);
            let resumed = run_loaded(cart.clone(), &ticks, Some(save)).unwrap();
            assert_eq!(resumed.report.state_hash, uninterrupted.report.state_hash, "resumed = uninterrupted");
        }
        // A second morning from the resumed save gives the second carp, not a third.
        let again = run_loaded(cart.clone(), &morning, Some(first.save_json.as_bytes())).unwrap();
        assert_eq!(held(&again.save_json, &item), before + 2);
        // The debug shortcut's mornings are mornings too.
        let skip = ReplayFile::parse(r#"{"inputs":[{"kind":"debug","action":{"type":"skipDay","days":3}}]}"#).unwrap();
        assert_eq!(held(&run_loaded(cart, &skip, None).unwrap().save_json, &item), before + 3);
    }

    #[test]
    fn a_broken_script_is_reported() {
        let (mut cart, _) = scripted();
        cart.plugins[0].source = "api.on('onDayStart', function () { throw new Error('no carp today'); });".to_owned();
        let replay = ReplayFile::parse(r#"{"inputs":[{"kind":"command","command":{"type":"sleep"}}]}"#).unwrap();
        let run = run_loaded(cart, &replay, None).unwrap();
        assert!(run.report.plugin_errors.iter().any(|error| error.contains("no carp today")), "{:?}", run.report);
    }

    /// Without plugins the session-run replay is the engine's own, effect for effect.
    #[test]
    fn a_cartridge_without_plugins_replays_as_the_engine_alone() {
        let mut cart = farm_cart::load_cartridge(CART).unwrap();
        // The fixture game ships a pack plugin; this is the game without it.
        cart.plugins.clear();
        let inputs: Vec<HeadlessInput> = vec![
            ReplayInput::Tick { ticks: 30 }.into(),
            ReplayInput::Command { command: farm_sim::Command::Sleep }.into(),
            ReplayInput::Tick { ticks: 45 }.into(),
        ];
        let ctx = EngineContext::new(cart.content.clone());
        let mut direct = state::create_game_state_from_start(&cart.start, Some("s"));
        let (hash, effects, _) = run_inputs(&ctx, &mut direct, &inputs);
        let replay = ReplayFile { seed: Some("s".to_owned()), inputs, ..ReplayFile::default() };
        let run = run_loaded(cart, &replay, None).unwrap();
        assert_eq!(run.report.state_hash, hash);
        assert_eq!(run.report.effects, effects);
    }
}
