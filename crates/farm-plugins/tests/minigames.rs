//! Pack-defined minigames in the sandbox (docs/PLUGINS.md "Minigames"): grants, the
//! start/update/draw protocol, every limit, failures and determinism.

mod common;

use common::read_json;
use farm_plugins::{
    plugin_specs_from_packs, MinigameControls, MinigameFrame, MinigameLimits, PackMinigame, PluginError,
    PluginErrorKind, PluginHost, PluginHostOptions, WasmPluginHost,
};
use farm_runtime::canvas::{CanvasCommand, CanvasPointer, CANVAS_WIDTH};
use farm_sim::schema::{ContentPack, PackManifest, PackPermissions, PackPlugin};
use serde_json::{json, Value};

const KIND: &str = "arcade:dodge";

/// A pack `arcade` whose manifest allows `allowed` minigames, with one plugin defining `defines`.
fn pack_with(allowed: Option<&[&str]>, defines: Option<&[&str]>, source: &str) -> ContentPack {
    let strings = |names: &[&str]| names.iter().map(|name| (*name).to_owned()).collect::<Vec<_>>();
    ContentPack {
        manifest: PackManifest {
            id: "arcade".into(),
            name: "Arcade".into(),
            version: "1.0.0".into(),
            permissions: PackPermissions {
                hooks: vec!["onDayStart".into()],
                mutations: Some(vec!["message".into()]),
                minigames: allowed.map(strings),
                ..Default::default()
            },
            ..Default::default()
        },
        plugins: vec![PackPlugin {
            id: "games".into(),
            hooks: vec!["onDayStart".into()],
            minigames: defines.map(strings),
            source: source.into(),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn host_for(pack: &ContentPack, options: PluginHostOptions) -> WasmPluginHost {
    WasmPluginHost::new(plugin_specs_from_packs([pack]), options)
}

fn arcade(source: &str) -> WasmPluginHost {
    host_for(&pack_with(Some(&["dodge"]), Some(&["dodge"]), source), PluginHostOptions::default())
}

fn start(host: &WasmPluginHost, config: Value) -> Result<PackMinigame, PluginError> {
    host.start_minigame(KIND, &config, 7).expect("the plugin defines the kind")
}

/// A counter game: each frame with the action pressed counts; three presses finish with a
/// score of presses / 3. It also counts its own calls in a top-level variable.
const COUNTER: &str = r#"
var calls = 0;
api.on('onDayStart', p => [{ type: 'message', text: 'day ' + p.day + ' after ' + calls + ' minigame calls' }]);
api.minigame('dodge', {
  start: (config, info) => { calls++; return { presses: 0, need: config.need || 3, seed: info.seed, w: info.width, calls: calls }; },
  update: (s, input, dt) => {
    calls++;
    if (input.actionPressed) s.presses++;
    s.calls = calls;
    s.t = (s.t || 0) + dt;
    return s.presses >= s.need ? { state: s, done: { score: Math.min(1, s.presses / 3) } } : { state: s };
  },
  draw: s => [
    { type: 'rect', x: 0, y: 0, w: s.w, h: 240, color: 'panel' },
    { type: 'text', x: 200, y: 120, text: 'Presses: ' + s.presses, align: 'center', color: '#fff' },
    { type: 'sprite', x: 10, y: 10, w: 32, h: 32, item: 'coin' },
  ],
});
"#;

fn press() -> MinigameControls {
    MinigameControls { action: true, action_pressed: true, ..MinigameControls::default() }
}

#[test]
fn a_pack_minigame_starts_runs_and_finishes_with_a_score() {
    let host = arcade(COUNTER);
    assert!(host.init_errors().is_empty(), "{:?}", host.init_errors());
    assert!(host.defines_minigame(KIND));
    let mut game = start(&host, json!({ "need": 3 })).unwrap();
    assert_eq!(game.kind(), KIND);
    assert_eq!(game.plugin_id(), "arcade:games");
    assert_eq!(game.state()["seed"], 7);
    assert_eq!(game.state()["w"], f64::from(CANVAS_WIDTH));
    assert_eq!(game.canvas().pack_id, "arcade");
    assert_eq!(game.canvas().commands.len(), 3);
    assert!(matches!(&game.canvas().commands[1], CanvasCommand::Text { text, .. } if text == "Presses: 0"));
    assert!(game.last_fuel_used() > 0);

    assert_eq!(game.frame(&MinigameControls::default(), 0.016), Ok(MinigameFrame::Running));
    assert_eq!(game.frame(&press(), 0.016), Ok(MinigameFrame::Running));
    assert_eq!(game.frame(&press(), 0.016), Ok(MinigameFrame::Running));
    assert!(matches!(&game.canvas().commands[1], CanvasCommand::Text { text, .. } if text == "Presses: 2"));
    assert_eq!(game.frame(&press(), 0.016), Ok(MinigameFrame::Finished { score: 1.0 }));
    assert_eq!(game.score(), Some(1.0));
    // Finished: later frames change nothing.
    let state = game.state().clone();
    assert_eq!(game.frame(&press(), 0.016), Ok(MinigameFrame::Finished { score: 1.0 }));
    assert_eq!(game.state(), &state);
    // Frame times are clamped: a stalled frame is at most a quarter second.
    let mut game = start(&host, json!({})).unwrap();
    game.frame(&MinigameControls::default(), 10.0).unwrap();
    game.frame(&MinigameControls::default(), f64::NAN).unwrap();
    game.frame(&MinigameControls::default(), -1.0).unwrap();
    assert_eq!(game.state()["t"], 0.25);
}

#[test]
fn sessions_get_their_own_instance_and_hooks_keep_theirs() {
    let mut host = arcade(COUNTER);
    let mut first = start(&host, json!({})).unwrap();
    for _ in 0..5 {
        first.frame(&MinigameControls::default(), 0.016).unwrap();
    }
    assert_eq!(first.state()["calls"], 6);
    // A new session starts from the plugin's source again: its variables start over.
    let second = start(&host, json!({})).unwrap();
    assert_eq!(second.state()["calls"], 1);
    // The hook handlers run in their own instance, which never saw a minigame call.
    let results = host.dispatch("onDayStart", r#"{"day":2,"season":"spring","year":1}"#);
    assert_eq!(common::texts(&results), "day 2 after 0 minigame calls");
}

#[test]
fn kinds_need_the_manifest_permission_and_the_plugin_declaration() {
    let options = PluginHostOptions::default;
    // Granted: the manifest allows it and the plugin declares it (either spelling).
    let host = host_for(&pack_with(Some(&["arcade:dodge"]), Some(&["dodge"]), COUNTER), options());
    assert!(host.start_minigame(KIND, &json!({}), 1).is_some_and(|started| started.is_ok()));
    for (allowed, defines) in [
        (None, Some(&["dodge"][..])),
        (Some(&[][..]), Some(&["dodge"][..])),
        (Some(&["dodge"][..]), None),
        (Some(&["other:dodge"][..]), Some(&["dodge"][..])),
        (Some(&["Dodge"][..]), Some(&["Dodge"][..])),
    ] {
        let host = host_for(&pack_with(allowed, defines, COUNTER), options());
        assert!(!host.defines_minigame(KIND), "{allowed:?} {defines:?}");
        assert!(host.start_minigame(KIND, &json!({}), 1).is_none(), "{allowed:?} {defines:?}");
    }
    // Another pack's kind is never this plugin's.
    let host = host_for(&pack_with(Some(&["dodge"]), Some(&["dodge", "other:dodge"]), COUNTER), options());
    assert_eq!(host.specs().next().unwrap().granted_minigames, ["arcade:dodge"]);
    assert!(host.start_minigame("other:dodge", &json!({}), 1).is_none());
}

fn failure(result: Result<PackMinigame, PluginError>) -> PluginError {
    result.expect_err("the start fails")
}

fn source_with(update: &str, draw: &str) -> String {
    format!("api.minigame('dodge', {{ start: () => ({{ n: 0 }}), update: {update}, draw: {draw} }});")
}

/// A session that started, then failed in its first frame.
fn failed_frame_with(options: PluginHostOptions, update: &str, draw: &str) -> (PackMinigame, PluginError) {
    let host = host_for(&pack_with(Some(&["dodge"]), Some(&["dodge"]), &source_with(update, draw)), options);
    let mut game = start(&host, json!({})).unwrap();
    let error = game.frame(&MinigameControls::default(), 0.016).expect_err("the frame fails");
    (game, error)
}

/// Frames with fuel to spare, to reach the other limits.
fn roomy() -> PluginHostOptions {
    PluginHostOptions {
        minigame: MinigameLimits { fuel_per_frame: 500_000_000, ..MinigameLimits::default() },
        ..PluginHostOptions::default()
    }
}

fn failed_frame(update: &str, draw: &str) -> (PackMinigame, PluginError) {
    failed_frame_with(PluginHostOptions::default(), update, draw)
}

#[test]
fn running_out_of_fuel_ends_the_session_as_failed() {
    let (mut game, error) = failed_frame("s => { for (;;) {} }", "s => []");
    assert_eq!(error.kind, PluginErrorKind::Timeout);
    assert_eq!(error.message, "minigame arcade:dodge failed: exceeded its fuel budget for a frame (10000000 fuel)");
    assert_eq!(error.plugin_id, "arcade:games");
    assert_eq!(error.hook.as_deref(), Some("minigame:dodge"));
    assert!(game.last_fuel_used() > 10_000_000);
    // Every later frame says the same, without running anything.
    assert_eq!(game.frame(&MinigameControls::default(), 0.016), Err(error.clone()));
    assert_eq!(game.failure(), Some(&error));

    // draw shares the frame's budget.
    let (_, error) = failed_frame("s => ({ state: { n: 1 } })", "s => { if (s.n) { for (;;) {} } return []; }");
    assert_eq!(error.kind, PluginErrorKind::Timeout);
    assert!(error.message.ends_with("for a frame (10000000 fuel)"), "{}", error.message);
    // Starting has its own budget.
    let limits = MinigameLimits { start_fuel: 2_000_000, ..MinigameLimits::default() };
    let host = host_for(
        &pack_with(
            Some(&["dodge"]),
            Some(&["dodge"]),
            &source_with("s => ({})", "s => []").replace("({ n: 0 })", "{ for (;;) {} }"),
        ),
        PluginHostOptions { minigame: limits, ..PluginHostOptions::default() },
    );
    let error = failure(host.start_minigame(KIND, &json!({}), 0).unwrap());
    assert_eq!(error.kind, PluginErrorKind::Timeout);
    assert_eq!(error.message, "minigame arcade:dodge failed: exceeded its fuel budget for starting (2000000 fuel)");
}

#[test]
fn draw_lists_and_states_beyond_the_limits_end_the_session() {
    // 257 commands: refused at the first draw (the start).
    let many = "s => { var out = []; for (var i = 0; i < 257; i++) out.push({ type: 'rect', x: i, y: 0, w: 1, h: 1 }); return out; }";
    let host = arcade(&source_with("s => ({ state: s })", many));
    let error = failure(start(&host, json!({})));
    assert_eq!(error.message, "minigame arcade:dodge failed: draw returned 257 draw commands; at most 256 are allowed");
    // Too much text, and a malformed command, refused in a frame.
    let wordy = "s => s.n ? [{ type: 'text', x: 0, y: 0, text: 'é'.repeat(201) }] : []";
    let (_, error) = failed_frame("s => ({ state: { n: 1 } })", wordy);
    assert_eq!(
        error.message,
        "minigame arcade:dodge failed: draw command 0: text is 201 characters; at most 200 are allowed"
    );
    let (_, error) = failed_frame("s => ({ state: { n: 1 } })", "s => s.n ? [{ type: 'circle' }] : []");
    assert_eq!(
        error.message,
        "minigame arcade:dodge failed: draw command 0: unknown type \"circle\" (rect, text or sprite)"
    );
    let (_, error) = failed_frame("s => ({ state: { n: 1 } })", "s => s.n ? 'nothing' : []");
    assert_eq!(error.message, "minigame arcade:dodge failed: draw must return an array of draw commands");
    // A state over 16 KiB (`{"big":"…"}` with 2^14 characters).
    let big = "s => { var b = 'x'; for (var i = 0; i < 14; i++) b += b; return { state: { big: b } }; }";
    let (_, error) = failed_frame(big, "s => []");
    assert_eq!(
        error.message,
        "minigame arcade:dodge failed: update returned a state of 16394 bytes; at most 16384 are allowed"
    );
    // An answer over the 1 MiB the host reads at all (with the fuel to build it).
    let (_, error) = failed_frame_with(
        roomy(),
        "s => { var b = 'x'; for (var i = 0; i < 20; i++) b += b; return { state: { a: b } }; }",
        "s => []",
    );
    assert_eq!(error.message, "minigame arcade:dodge failed: update and draw returned more than 1048576 bytes");
}

#[test]
fn throws_and_malformed_answers_end_the_session() {
    let (_, error) = failed_frame("s => { throw new Error('kaput'); }", "s => []");
    assert_eq!(error.kind, PluginErrorKind::Threw);
    assert_eq!(error.message, "minigame arcade:dodge failed: threw in update: kaput");
    let cases = [
        ("s => 3", "update must return an object { state, done }"),
        ("s => undefined", "update must return an object { state, done }"),
        ("s => ({ state: s, score: 1 })", "update returned unknown key \"score\" (only state, done and sounds)"),
        ("s => ({ done: { score: 2 } })", "done.score must be a number from 0 to 1"),
        ("s => ({ done: true })", "update's done must be an object { score } or { success }"),
        ("s => ({ done: {} })", "done must have a score from 0 to 1 or success true/false"),
    ];
    for (update, expected) in cases {
        let (_, error) = failed_frame(update, "s => []");
        assert_eq!(error.message, format!("minigame arcade:dodge failed: {expected}"), "{update}");
    }
    // `done: { success }` scores 1 or 0.
    for (success, score) in [(true, 1.0), (false, 0.0)] {
        let host = arcade(&source_with(&format!("s => ({{ done: {{ success: {success} }} }})"), "s => []"));
        let mut game = start(&host, json!({})).unwrap();
        assert_eq!(game.frame(&MinigameControls::default(), 0.0), Ok(MinigameFrame::Finished { score }));
    }
    // Deep recursion and the memory budget are overruns too (with fuel to spare to reach them).
    let (_, error) = failed_frame_with(roomy(), "function deep(s) { return deep(s) + 1; }", "s => []");
    assert_eq!(error.kind, PluginErrorKind::Budget);
    assert_eq!(error.message, "minigame arcade:dodge failed: exceeded its recursion budget in a frame");
    let (_, error) = failed_frame_with(
        roomy(),
        "s => { var chunk = 'x'.repeat(1000), keep = []; for (;;) keep.push(chunk.repeat(1000)); }",
        "s => []",
    );
    assert_eq!(error.kind, PluginErrorKind::Budget);
    assert_eq!(error.message, "minigame arcade:dodge failed: exceeded its memory budget in a frame");
}

#[test]
fn starts_that_cannot_work_say_why() {
    // The plugin is declared to define the kind but never calls api.minigame.
    let host = arcade("api.on('onDayStart', () => []);");
    let error = failure(start(&host, json!({})));
    assert_eq!(error.kind, PluginErrorKind::Init);
    assert_eq!(
        error.message,
        "minigame arcade:dodge failed: the plugin does not define it (api.minigame('dodge', { start, update, draw }))"
    );
    // A plugin that does not compile.
    let host = arcade("api.minigame('dodge', {");
    assert!(!host.init_errors().is_empty());
    let error = failure(start(&host, json!({})));
    assert!(
        error.message.starts_with("minigame arcade:dodge failed: plugin arcade:games is disabled"),
        "{}",
        error.message
    );
    // A start that throws, and a game without draw.
    let host =
        arcade("api.minigame('dodge', { start: () => { throw new Error('no'); }, update: s => ({}), draw: s => [] });");
    assert_eq!(failure(start(&host, json!({}))).message, "minigame arcade:dodge failed: threw in start: no");
    let host = arcade("api.minigame('dodge', { update: s => ({}) });");
    assert_eq!(
        failure(start(&host, json!({}))).message,
        "minigame arcade:dodge failed: threw in draw: minigame dodge has no draw function"
    );
    // api.minigame checks its arguments.
    let host = arcade("api.minigame('dodge');");
    assert!(
        host.init_errors()[0].message.contains("api.minigame(name, { start, update, draw })"),
        "{:?}",
        host.init_errors()
    );
}

/// Keeps what `update` saw of the pointer, and asks for a sound per press: `pop` on the canvas's
/// left half, `ui` on its right half.
const POINTER: &str = r#"
api.minigame('dodge', {
  start: () => ({ seen: [] }),
  update: (s, input) => {
    var p = input.pointer;
    s.seen.push(p === null ? null : [p.x, p.y, p.down, p.pressed, p.released]);
    return { state: s, sounds: p && p.pressed ? [p.x < 200 ? 'pop' : 'ui'] : [] };
  },
  draw: s => [],
});
"#;

#[test]
fn update_gets_the_pointer_and_answers_sounds() {
    let host = arcade(POINTER);
    let mut game = start(&host, json!({})).unwrap();
    assert!(game.sounds().is_empty());
    let pointer = |x: f32, down: bool, pressed: bool, released: bool| MinigameControls {
        pointer: Some(CanvasPointer::at(x, 120.0, down, pressed, released)),
        ..MinigameControls::default()
    };
    game.frame(&MinigameControls::default(), 0.016).unwrap();
    assert!(game.sounds().is_empty());
    game.frame(&pointer(50.5, true, true, false), 0.016).unwrap();
    assert_eq!(game.sounds(), ["pop"]);
    game.frame(&pointer(80.0, true, false, false), 0.016).unwrap();
    assert!(game.sounds().is_empty(), "sounds are per frame");
    game.frame(&pointer(300.0, false, true, true), 0.016).unwrap();
    assert_eq!(game.sounds(), ["ui"]);
    assert_eq!(
        game.state()["seen"],
        json!([null, [50.5, 120, true, true, false], [80, 120, true, false, false], [300, 120, false, true, true]])
    );

    // Up to eight sounds a frame, played in order; the frame that finishes plays its own.
    let host = arcade(&source_with(
        "s => ({ done: { score: 1 }, sounds: ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'harvest'] })",
        "s => []",
    ));
    let mut game = start(&host, json!({})).unwrap();
    assert_eq!(game.frame(&MinigameControls::default(), 0.016), Ok(MinigameFrame::Finished { score: 1.0 }));
    assert_eq!(game.sounds(), ["a", "b", "c", "d", "e", "f", "g", "harvest"]);
    game.frame(&MinigameControls::default(), 0.016).unwrap();
    assert!(game.sounds().is_empty(), "a finished session plays nothing more");
    // `sounds: null` is no sounds.
    let host = arcade(&source_with("s => ({ sounds: null })", "s => []"));
    let mut game = start(&host, json!({})).unwrap();
    assert_eq!(game.frame(&MinigameControls::default(), 0.016), Ok(MinigameFrame::Running));
    assert!(game.sounds().is_empty());
}

#[test]
fn sounds_beyond_the_limits_end_the_session() {
    let long = "x".repeat(257);
    let cases = [
        ("s => ({ sounds: 'pop' })", "update's sounds must be a list of sound ids".to_owned()),
        ("s => ({ sounds: [1] })", "sound 0 must be a sound id".to_owned()),
        ("s => ({ sounds: ['pop', ''] })", "sound 1 must be a sound id".to_owned()),
        (&*format!("s => ({{ sounds: ['{long}'] }})"), "sound 0: at most 256 characters".to_owned()),
        ("s => ({ sounds: Array(9).fill('pop') })", "update returned 9 sounds; at most 8 are allowed".to_owned()),
    ];
    for (update, expected) in cases {
        let (game, error) = failed_frame(update, "s => []");
        assert_eq!(error.kind, PluginErrorKind::Threw);
        assert_eq!(error.message, format!("minigame arcade:dodge failed: {expected}"), "{update}");
        assert!(game.sounds().is_empty());
    }
    // The cap is a limit like the others.
    let options = PluginHostOptions {
        minigame: MinigameLimits { max_sounds_per_frame: 1, ..MinigameLimits::default() },
        ..PluginHostOptions::default()
    };
    let (_, error) = failed_frame_with(options, "s => ({ sounds: ['a', 'b'] })", "s => []");
    assert_eq!(error.message, "minigame arcade:dodge failed: update returned 2 sounds; at most 1 are allowed");
}

/// Every frame's state and draw list of a 120-frame session of the example pack, with input
/// that depends only on the frame number.
/// One frame of a run: the state, the draw list and the fuel it used.
type Frame = (Value, Vec<CanvasCommand>, u64);

fn example_run(seed: u32) -> (Vec<Frame>, Option<f64>) {
    let pack: ContentPack = serde_json::from_value(read_json("fixtures/projects/lantern-night-mod.json")).unwrap();
    let host = host_for(&pack, PluginHostOptions::default());
    assert!(host.init_errors().is_empty(), "{:?}", host.init_errors());
    let config = serde_json::to_value(&pack.content.minigames[0].config).unwrap();
    let mut game = host.start_minigame("lantern-night:lantern-catch", &config, seed).unwrap().unwrap();
    let mut frames = Vec::new();
    let mut score = None;
    for frame in 0..2000u32 {
        let input = MinigameControls {
            left: frame % 90 < 30,
            right: frame % 90 >= 60,
            action: frame % 7 == 0,
            action_pressed: frame % 7 == 0,
            ..MinigameControls::default()
        };
        match game.frame(&input, 1.0 / 60.0).unwrap() {
            MinigameFrame::Running => {}
            MinigameFrame::Finished { score: done } => {
                score = Some(done);
                break;
            }
        }
        frames.push((game.state().clone(), game.canvas().commands.clone(), game.last_fuel_used()));
    }
    (frames, score)
}

#[test]
fn the_same_start_and_input_give_the_same_frames_every_run() {
    let (frames, score) = example_run(12345);
    let score = score.expect("the example game ends");
    assert!((0.0..=1.0).contains(&score));
    assert!(frames.len() > 100, "{}", frames.len());
    // Fuel stays far below the frame budget.
    assert!(frames.iter().all(|(_, _, fuel)| *fuel < MinigameLimits::default().fuel_per_frame / 4));
    // Same seed, same input: every state, draw list and fuel count is the same.
    assert_eq!(example_run(12345), (frames.clone(), Some(score)));
    // Another seed drops other lanterns.
    assert_ne!(example_run(54321).0[0].0, frames[0].0);
}
