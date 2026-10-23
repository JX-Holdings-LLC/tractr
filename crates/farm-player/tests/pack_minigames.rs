//! Pack-defined minigames in a running game (docs/PLUGINS.md "Minigames"): the session starts
//! the plugin's kind, feeds it the game keys every frame, shows its frame, and resolves it once,
//! through the same `resolveMinigame` path as the built-in kinds; a failing plugin ends it as
//! failed (score 0) with a plugin error. The pointer over its canvas reaches it, and the sounds
//! it asks for play like a `playSound` outcome's.

mod common;

use farm_player::session::{PlaySession, SessionEvent};
use farm_sim::schema::{ContentPack, GameProject, MinigameDef, MinigameResultTier, PackInstallation, PackPlugin};
use farm_sim::{hash_state, state, Command, StartState};

const FRAME: f64 = 1.0 / 60.0;

fn read_json(relative: &str) -> serde_json::Value {
    let path = [env!("CARGO_MANIFEST_DIR"), "..", "..", relative].iter().collect::<std::path::PathBuf>();
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn lantern_night() -> ContentPack {
    serde_json::from_value(read_json("fixtures/projects/lantern-night-mod.json")).unwrap()
}

fn project_with(pack: ContentPack) -> GameProject {
    let fixture = read_json("fixtures/golden/content/starter-farm.json");
    let mut project: GameProject = serde_json::from_value(fixture["project"].clone()).unwrap();
    project.content_packs.push(PackInstallation { pack, enabled: true });
    project
}

fn session_with(project: &GameProject) -> PlaySession {
    let content = state::create_content_from_project(project);
    let mut session = PlaySession::new_game(content, &StartState::from_project(project), Some("pack-minigames"));
    session.set_plugins(farm_player::plugins::for_project(project, Default::default()));
    session.set_cosmetic_seed(42);
    assert!(session.plugin_errors().is_empty(), "{:?}", session.plugin_errors());
    session
}

fn toasts(events: &[SessionEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::Toast { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// Plays Lantern Night's minigame from its action key with a fixed key script; returns the
/// final state hash, the toasts and how many frames the minigame ran.
fn play_lantern_catch() -> (String, Vec<String>, usize) {
    let project = project_with(lantern_night());
    let mut session = session_with(&project);
    session.drain_events();
    session.key_down("l");
    session.update(FRAME, false);
    session.key_up("l");
    let opened = session.state().minigame.as_ref().map(|active| active.minigame_id.clone());
    assert_eq!(opened.as_deref(), Some("lantern-night:catch"));
    let view = session.minigame_view().expect("the minigame is mounted").clone();
    assert_eq!(view.kind, "lantern-night:lantern-catch");
    let canvas = view.canvas.expect("a pack minigame shows its own frame");
    assert_eq!(canvas.pack_id, "lantern-night");
    assert!(!canvas.commands.is_empty());

    let mut frames = 0;
    let mut held: Option<&str> = None;
    while session.state().minigame.is_some() {
        assert!(frames < 5_000, "the minigame ends");
        // Sweep left and right, holding the action key now and then (to hurry).
        let want = if frames % 120 < 60 { "a" } else { "d" };
        if held != Some(want) {
            if let Some(key) = held {
                session.key_up(key);
            }
            session.key_down(want);
            held = Some(want);
        }
        if frames % 50 == 0 {
            session.key_down(" ");
        } else if frames % 50 == 10 {
            session.key_up(" ");
        }
        session.update(FRAME, false);
        frames += 1;
        if let Some(view) = session.minigame_view() {
            assert!(view.canvas.is_some());
        }
    }
    assert!(session.plugin_errors().is_empty(), "{:?}", session.plugin_errors());
    (hash_state(session.state()), toasts(&session.drain_events()), frames)
}

#[test]
fn a_pack_minigame_plays_from_the_game_keys_and_resolves_through_its_tiers() {
    let (_, toasts, frames) = play_lantern_catch();
    assert!(frames > 60, "{frames}");
    let tiers = ["The lanterns drift away.", "You caught a few lanterns!", "Every lantern caught!"];
    assert_eq!(toasts.iter().filter(|toast| tiers.contains(&toast.as_str())).count(), 1, "{toasts:?}");
}

#[test]
fn the_same_input_plays_the_same_minigame_every_run() {
    let first = play_lantern_catch();
    assert_eq!(play_lantern_catch(), first);
}

/// A pack `broken` whose minigame kind runs `source`; its def `game` gives an item at score 0
/// and money at score 1.
fn broken_project(source: &str) -> GameProject {
    let mut pack: ContentPack = serde_json::from_value(serde_json::json!({
        "manifest": { "id": "broken", "name": "Broken", "version": "1.0.0", "permissions": { "minigames": ["game"] } },
    }))
    .unwrap();
    pack.plugins.push(PackPlugin {
        id: "p".into(),
        minigames: Some(vec!["game".into()]),
        source: source.into(),
        ..Default::default()
    });
    let tier = |min_score: f64, outcome: serde_json::Value| -> MinigameResultTier {
        serde_json::from_value(serde_json::json!({ "minScore": min_score, "outcomes": [outcome] })).unwrap()
    };
    pack.content.minigames.push(MinigameDef {
        id: "game".into(),
        name: "Game".into(),
        kind: "broken:game".into(),
        result_tiers: vec![
            tier(0.0, serde_json::json!({ "type": "message", "message": "failed tier" })),
            tier(1.0, serde_json::json!({ "type": "message", "message": "won tier" })),
        ],
        ..Default::default()
    });
    project_with(pack)
}

fn open(session: &mut PlaySession) {
    let open = farm_sim::schema::PluginMutation::StartMinigame { minigame_id: "broken:game".into() };
    session.run_command(&Command::PluginMutation { plugin_id: "test".into(), mutation: open });
    assert!(session.state().minigame.is_some());
}

#[test]
fn a_plugin_that_fails_ends_the_minigame_as_failed_with_a_plugin_error() {
    // Throws in its third frame.
    let source = "api.minigame('game', { start: () => ({ n: 0 }), \
                  update: s => { if (s.n === 2) throw new Error('boom'); return { state: { n: s.n + 1 } }; }, \
                  draw: s => [{ type: 'text', x: 10, y: 20, text: 'frame ' + s.n }] });";
    let project = broken_project(source);
    let mut session = session_with(&project);
    open(&mut session);
    session.drain_events();
    for _ in 0..2 {
        session.update(FRAME, false);
        assert!(session.state().minigame.is_some());
    }
    session.update(FRAME, false);
    assert!(session.state().minigame.is_none(), "resolved in the failing frame");
    let events = session.drain_events();
    let expected = "broken:p: [threw] minigame broken:game failed: threw in update: boom";
    assert!(
        events.iter().any(|event| matches!(event, SessionEvent::PluginError { text } if text == expected)),
        "{events:?}"
    );
    assert!(toasts(&events).contains(&"failed tier".to_owned()), "{events:?}");
    assert_eq!(session.plugin_errors(), [expected]);
    // The rest of the game goes on.
    session.update(FRAME, false);

    // One that cannot even start resolves on the next frame, the same way.
    let project = broken_project("api.minigame('game', { draw: s => 'not a list' });");
    let mut session = session_with(&project);
    open(&mut session);
    session.drain_events();
    session.update(FRAME, false);
    assert!(session.state().minigame.is_none());
    let events = session.drain_events();
    assert!(toasts(&events).contains(&"failed tier".to_owned()), "{events:?}");
    assert_eq!(
        session.plugin_errors(),
        ["broken:p: [threw] minigame broken:game failed: draw must return an array of draw commands"]
    );

    // An endless loop is stopped by the frame budget, and resolves as failed too.
    let project = broken_project("api.minigame('game', { update: s => { for (;;) {} }, draw: s => [] });");
    let mut session = session_with(&project);
    open(&mut session);
    session.update(FRAME, false);
    assert!(session.state().minigame.is_none());
    assert_eq!(
        session.plugin_errors(),
        ["broken:p: [timeout] minigame broken:game failed: exceeded its fuel budget for a frame (10000000 fuel)"]
    );
}

#[test]
fn a_finished_pack_minigame_resolves_once_with_its_score() {
    let source = "api.minigame('game', { update: (s, input) => input.actionPressed ? { done: { score: 1 } } : {}, \
                  draw: () => [] });";
    let project = broken_project(source);
    let mut session = session_with(&project);
    open(&mut session);
    session.drain_events();
    session.update(FRAME, false);
    assert!(session.state().minigame.is_some(), "waits for the action key");
    session.key_down("enter");
    session.update(FRAME, false);
    assert!(session.state().minigame.is_none());
    let events = session.drain_events();
    assert_eq!(toasts(&events).iter().filter(|toast| toast.as_str() == "won tier").count(), 1, "{events:?}");
    assert!(session.plugin_errors().is_empty());
    // Without a plugin that defines the kind, the def plays as the neutral fallback.
    let mut project = broken_project(source);
    project.content_packs.last_mut().unwrap().pack.manifest.permissions.minigames = None;
    let mut session = session_with(&project);
    open(&mut session);
    session.update(FRAME, false);
    let view = session.minigame_view().unwrap();
    assert!(view.canvas.is_none());
    assert_eq!(view.kind, "fallback");
}

/// Shows what `update` last saw of the pointer and counts presses; asks for `pop` on a press,
/// and wins with `harvest` on the third release.
const POINTER: &str = "api.minigame('game', { start: () => ({ last: null, presses: 0, releases: 0 }), \
    update: (s, input) => { var p = input.pointer; \
      s.last = p && [Math.round(p.x), Math.round(p.y), p.down, p.pressed, p.released]; \
      if (p && p.pressed) s.presses++; if (p && p.released) s.releases++; \
      return s.releases >= 3 ? { state: s, sounds: ['harvest'], done: { score: 1 } } \
        : { state: s, sounds: p && p.pressed ? ['pop'] : [] }; }, \
    draw: s => [{ type: 'text', x: 0, y: 20, text: JSON.stringify(s) }] });";

fn sounds(events: &[SessionEvent]) -> Vec<&str> {
    events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::Sound { cue } => Some(cue.as_str()),
            _ => None,
        })
        .collect()
}

/// The text the pointer game draws: its state.
fn shown(view: Option<&farm_runtime::host::MinigameView>) -> serde_json::Value {
    let canvas = view.and_then(|view| view.canvas.as_ref()).expect("the pack minigame is up");
    match canvas.commands.first() {
        Some(farm_runtime::canvas::CanvasCommand::Text { text, .. }) => serde_json::from_str(text).unwrap(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_session_feeds_the_pointer_to_the_next_frame_and_plays_the_sounds_it_asks_for() {
    use farm_runtime::canvas::CanvasPointer;
    let project = broken_project(POINTER);
    let mut session = session_with(&project);
    // No pack minigame yet: a pointer goes nowhere.
    session.minigame_pointer(CanvasPointer::at(1.0, 2.0, true, true, false));
    open(&mut session);
    session.drain_events();
    session.update(FRAME, false);
    assert_eq!(shown(session.minigame_view())["last"], serde_json::Value::Null);
    session.minigame_pointer(CanvasPointer::at(120.0, 80.0, true, true, false));
    session.update(FRAME, false);
    assert_eq!(shown(session.minigame_view())["last"], serde_json::json!([120, 80, true, true, false]));
    assert_eq!(sounds(&session.drain_events()), ["pop"]);
    // One frame only: the next one has no pointer unless the UI reports it again.
    session.update(FRAME, false);
    assert_eq!(shown(session.minigame_view())["last"], serde_json::Value::Null);
    assert!(sounds(&session.drain_events()).is_empty());
    // Two reports before a frame keep the last position and both edges (a quick tap).
    session.minigame_pointer(CanvasPointer::at(10.0, 10.0, true, true, false));
    session.minigame_pointer(CanvasPointer::at(30.0, 40.0, false, false, true));
    session.update(FRAME, false);
    assert_eq!(shown(session.minigame_view())["last"], serde_json::json!([30, 40, false, true, true]));
    // Releasing input (focus loss, a menu) drops a pending pointer.
    session.minigame_pointer(CanvasPointer::at(10.0, 10.0, false, false, true));
    session.release_input();
    session.update(FRAME, false);
    assert_eq!(shown(session.minigame_view())["releases"], 1);
    session.drain_events();
    // Two more taps win: a press sound, then the winning frame's own sound as it resolves.
    for _ in 0..2 {
        session.minigame_pointer(CanvasPointer::at(10.0, 10.0, false, true, true));
        session.update(FRAME, false);
    }
    assert!(session.state().minigame.is_none());
    let events = session.drain_events();
    assert_eq!(sounds(&events), ["pop", "harvest"]);
    assert!(toasts(&events).contains(&"won tier".to_owned()), "{events:?}");
    assert!(session.plugin_errors().is_empty(), "{:?}", session.plugin_errors());
}

#[test]
fn clicks_and_taps_on_the_minigame_window_reach_the_pack_minigame() {
    use farm_player::InputEvent;
    let stores = common::Stores::new();
    let mut player = common::new_game_of(&stores, &broken_project(POINTER));
    let open = farm_sim::schema::PluginMutation::StartMinigame { minigame_id: "broken:game".into() };
    player.run_command(&Command::PluginMutation { plugin_id: "test".into(), mutation: open }).unwrap();
    common::idle(&mut player, 3);
    let state = |player: &farm_player::Player| shown(player.session().and_then(|session| session.minigame_view()));
    let step = |player: &mut farm_player::Player, events: &[InputEvent]| -> Vec<String> {
        let output = player.step(common::FRAME, events, common::SIZE.0, common::SIZE.1).unwrap();
        output.sounds.into_iter().map(|sound| sound.cue).collect()
    };
    let canvas = player.widget_rect(farm_ui::WidgetId::new("minigame-canvas")).expect("the canvas is drawn");
    let (x, y) = (canvas.x + canvas.width / 2.0, canvas.y + canvas.height / 2.0);

    // A click outside the window is not routed (the backdrop takes it).
    let (ox, oy) = (4.0, 4.0);
    let down = InputEvent::PointerDown { x: ox, y: oy, button: Default::default() };
    step(&mut player, &[InputEvent::PointerMove { x: ox, y: oy }, down]);
    step(&mut player, &[InputEvent::PointerUp { x: ox, y: oy, button: Default::default() }]);
    step(&mut player, &[]);
    assert_eq!(state(&player)["presses"], 0);
    assert_eq!(state(&player)["last"], serde_json::Value::Null);

    // A click on the canvas: the minigame's next frame sees the press at the canvas center.
    let down = InputEvent::PointerDown { x, y, button: Default::default() };
    let mut cues = step(&mut player, &[InputEvent::PointerMove { x, y }, down]);
    cues.extend(step(&mut player, &[]));
    assert_eq!(state(&player)["last"], serde_json::json!([200, 120, true, true, false]));
    assert!(cues.iter().any(|cue| cue == "pop"), "{cues:?}");
    step(&mut player, &[InputEvent::PointerUp { x, y, button: Default::default() }]);
    step(&mut player, &[]);
    assert_eq!(state(&player)["last"], serde_json::json!([200, 120, false, false, true]));
    assert_eq!((state(&player)["presses"].clone(), state(&player)["releases"].clone()), (1.into(), 1.into()));
    // Hovering with the mouse up reports the position.
    step(&mut player, &[InputEvent::PointerMove { x: canvas.x + canvas.width / 4.0, y }]);
    step(&mut player, &[]);
    assert_eq!(state(&player)["last"], serde_json::json!([100, 120, false, false, false]));

    // Two taps within one frame each (a touch screen) finish it with the winning sound.
    let mut cues = Vec::new();
    for _ in 0..2 {
        let tap = [
            InputEvent::PointerMove { x, y },
            InputEvent::PointerDown { x, y, button: Default::default() },
            InputEvent::PointerUp { x, y, button: Default::default() },
        ];
        cues.extend(step(&mut player, &tap));
        cues.extend(step(&mut player, &[]));
    }
    assert!(player.state().unwrap().minigame.is_none(), "won");
    assert!(cues.iter().any(|cue| cue == "harvest"), "{cues:?}");
    assert!(player.plugin_errors().is_empty(), "{:?}", player.plugin_errors());
}
