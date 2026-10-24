//! Cutscenes (M2): the runner plays each step type over ticks, branches on choices and jumps,
//! skips, refuses what does not apply, holds the player and the clock like a modal, and is
//! deterministic and saved with the game.

mod common;

use common::{flag_is_true, make_engine};
use farm_sim::cutscenes::{self, CutsceneView};
use farm_sim::schema::{Cutscene, DialogueState, EventOutcome, GameState, OutcomeKind};
use farm_sim::{engine, events, hash_state, units, Command, CommandRules, Effect, EngineContext};
use serde_json::{json, Value};

/// A context and state whose content holds one cutscene `scene` with these steps (JSON), and
/// whose project is changed by `mutate` first.
fn with_cutscene(steps: Value, mutate: impl FnOnce(&mut farm_sim::GameProject)) -> (EngineContext, GameState) {
    let cutscene: Cutscene = serde_json::from_value(json!({"id": "scene", "name": "Scene", "steps": steps})).unwrap();
    make_engine(
        |project| {
            project.cutscenes = Some(vec![cutscene]);
            mutate(project);
        },
        "cutscenes",
    )
}

/// Starts `scene` the way events do (the `startCutscene` outcome).
fn start(ctx: &EngineContext, state: &mut GameState) {
    let outcome =
        EventOutcome { cutscene_id: Some("scene".to_owned()), ..EventOutcome::of(OutcomeKind::StartCutscene) };
    events::apply_outcomes(ctx, state, &[outcome], 0);
    assert!(state.cutscene.is_some(), "the cutscene started");
}

/// A context and a state with `scene` started.
fn playing(steps: Value) -> (EngineContext, GameState) {
    let (ctx, mut state) = with_cutscene(steps, |_| {});
    start(&ctx, &mut state);
    (ctx, state)
}

fn view(ctx: &EngineContext, state: &GameState) -> CutsceneView {
    cutscenes::view(&ctx.content, state).expect("a cutscene plays")
}

fn line(ctx: &EngineContext, state: &GameState) -> Option<String> {
    view(ctx, state).line.map(|line| line.text)
}

fn tick(ctx: &EngineContext, state: &mut GameState, ticks: u64) -> Vec<Effect> {
    engine::advance_tick(ctx, state, ticks)
}

fn run(ctx: &EngineContext, state: &mut GameState, command: Command) -> Vec<Effect> {
    engine::apply_command(ctx, state, &command)
}

fn set_flag(name: &str) -> Value {
    json!({"type": "outcomes", "outcomes": [{"type": "setFlag", "flagName": name}]})
}

// --- lines and choices ---

#[test]
fn say_lines_wait_for_the_player_and_the_cutscene_ends_after_the_last_step() {
    let (ctx, mut state) = playing(json!([
        {"type": "say", "speaker": "npc-test", "text": "Hello there."},
        {"type": "say", "speaker": "player", "text": "Hi!"},
        {"type": "say", "text": "Time passed."},
    ]));
    tick(&ctx, &mut state, 1);
    let shown = view(&ctx, &state).line.unwrap();
    assert_eq!(shown.text, "Hello there.");
    assert_eq!(shown.speaker.as_deref(), Some("npc-test"));
    assert_eq!(shown.speaker_name.as_deref(), Some("Testy"));
    assert!(shown.options.is_empty());
    // The line stays until the player moves on.
    tick(&ctx, &mut state, 200);
    assert_eq!(line(&ctx, &state).as_deref(), Some("Hello there."));

    run(&ctx, &mut state, Command::AdvanceCutscene);
    let shown = view(&ctx, &state).line.unwrap();
    assert_eq!((shown.text.as_str(), shown.speaker.as_deref(), shown.speaker_name), ("Hi!", Some("player"), None));
    run(&ctx, &mut state, Command::AdvanceCutscene);
    let shown = view(&ctx, &state).line.unwrap();
    assert_eq!((shown.speaker, shown.speaker_name), (None, None), "a narrator");
    run(&ctx, &mut state, Command::AdvanceCutscene);
    assert_eq!(state.cutscene, None);
    assert!(cutscenes::view(&ctx.content, &state).is_none());
}

#[test]
fn choices_branch_to_their_labels() {
    let steps = json!([
        {"type": "choice", "text": "Which way?", "options": [
            {"text": "Left", "goto": "left"},
            {"text": "Right", "goto": "right"},
            {"text": "Stay"},
        ]},
        set_flag("stayed"),
        {"type": "end"},
        {"type": "outcomes", "label": "left", "outcomes": [{"type": "setFlag", "flagName": "went-left"}]},
        {"type": "end"},
        {"type": "outcomes", "label": "right", "outcomes": [{"type": "setFlag", "flagName": "went-right"}]},
    ]);
    for (index, flag) in [(0, "went-left"), (1, "went-right"), (2, "stayed")] {
        let (ctx, mut state) = playing(steps.clone());
        tick(&ctx, &mut state, 1);
        let shown = view(&ctx, &state).line.unwrap();
        assert_eq!(shown.text, "Which way?");
        assert_eq!(shown.options, vec!["Left", "Right", "Stay"]);
        // Advancing does not answer a question.
        let before = state.clone();
        run(&ctx, &mut state, Command::AdvanceCutscene);
        assert_eq!(state, before);
        run(&ctx, &mut state, Command::ChooseCutsceneOption { index });
        assert!(flag_is_true(&state, flag), "{flag}");
        assert_eq!(state.cutscene, None, "the branch ran to its end");
        for other in ["went-left", "went-right", "stayed"].into_iter().filter(|other| *other != flag) {
            assert!(!state.flags.contains_key(other), "{other} after option {index}");
        }
    }
}

#[test]
fn a_choice_without_options_is_a_line() {
    let (ctx, mut state) = playing(json!([{"type": "choice", "text": "Hmm."}, set_flag("after")]));
    tick(&ctx, &mut state, 1);
    let before = state.clone();
    run(&ctx, &mut state, Command::ChooseCutsceneOption { index: 0 });
    assert_eq!(state, before, "there is nothing to choose");
    run(&ctx, &mut state, Command::AdvanceCutscene);
    assert!(flag_is_true(&state, "after"));
}

#[test]
fn jumps_follow_their_conditions() {
    let steps = json!([
        {"type": "jump", "goto": "known", "conditions": [{"type": "flag", "flag": "met"}]},
        set_flag("stranger"),
        {"type": "end"},
        {"type": "outcomes", "label": "known", "outcomes": [{"type": "setFlag", "flagName": "friend"}]},
    ]);
    let (ctx, mut state) = playing(steps.clone());
    tick(&ctx, &mut state, 1);
    assert!(flag_is_true(&state, "stranger") && !state.flags.contains_key("friend"));
    assert_eq!(state.cutscene, None);

    let (ctx, mut state) = playing(steps);
    state.flags.insert("met".to_owned(), Value::Bool(true));
    tick(&ctx, &mut state, 1);
    assert!(flag_is_true(&state, "friend") && !state.flags.contains_key("stranger"));

    // A jump without conditions always jumps; to an unknown label it goes on.
    let (ctx, mut state) =
        playing(json!([{"type": "jump", "goto": "nowhere"}, set_flag("on"), {"type": "jump", "goto": "on"}]));
    tick(&ctx, &mut state, 1);
    assert!(flag_is_true(&state, "on"));
}

// --- timed steps ---

#[test]
fn waits_last_their_seconds() {
    let (ctx, mut state) = playing(json!([{"type": "wait", "seconds": 1}, set_flag("waited")]));
    // The first tick begins the wait; twenty more end it.
    tick(&ctx, &mut state, 20);
    assert!(!state.flags.contains_key("waited"));
    tick(&ctx, &mut state, 1);
    assert!(flag_is_true(&state, "waited"));
    assert_eq!(state.cutscene, None);

    // Without seconds a wait lasts one second; zero seconds go straight on.
    let (ctx, mut state) = playing(json!([{"type": "wait"}, {"type": "wait", "seconds": 0}, set_flag("waited")]));
    tick(&ctx, &mut state, 20);
    assert!(!state.flags.contains_key("waited"));
    tick(&ctx, &mut state, 1);
    assert!(flag_is_true(&state, "waited"));
}

#[test]
fn moves_walk_actors_along_a_path_and_wait_for_them() {
    let (ctx, mut state) = playing(json!([
        {"type": "move", "actor": "player", "x": 1, "y": 4},
        set_flag("arrived"),
        {"type": "say", "text": "Here."},
    ]));
    let start = state.player.x;
    // The walk begins the tick after the step does, like every timer.
    tick(&ctx, &mut state, 1);
    assert_eq!(state.player.x, start);
    tick(&ctx, &mut state, 1);
    assert!(state.player.x < start, "the player set off");
    assert_eq!(state.player.direction, "left");
    let walking = view(&ctx, &state);
    assert_eq!(walking.actors.len(), 1);
    assert!(walking.actors[0].moving && walking.actors[0].actor == "player");
    assert!(!state.flags.contains_key("arrived"));
    // Two tiles at the player's speed (4.5 tiles a second) take nine ticks.
    tick(&ctx, &mut state, 8);
    assert_eq!((state.player.x, state.player.y), (units::tile_center(1), units::tile_center(4)));
    assert!(flag_is_true(&state, "arrived"));
    assert!(view(&ctx, &state).actors.is_empty());
    // The walk does not hold the player's own movement: none was asked for.
    assert_eq!(state.player.move_intent, Default::default());
}

#[test]
fn npcs_walk_too_and_moves_that_do_not_wait_go_straight_on() {
    let (ctx, mut state) = playing(json!([
        {"type": "move", "actor": "npc-test", "x": 1, "y": 3, "wait": false},
        set_flag("on-the-way"),
        {"type": "wait", "seconds": 2},
    ]));
    tick(&ctx, &mut state, 1);
    assert!(flag_is_true(&state, "on-the-way"));
    tick(&ctx, &mut state, 1);
    let npc = &state.npcs["npc-test"];
    assert!(npc.y > units::tiles(1) && npc.y < units::tiles(3), "{}", npc.y);
    let walking = view(&ctx, &state);
    assert_eq!(walking.actors[0].actor, "npc-test");
    assert_eq!(walking.actors[0].direction, "down");
    tick(&ctx, &mut state, 20);
    let npc = &state.npcs["npc-test"];
    assert_eq!((npc.x, npc.y), (units::tiles(1), units::tiles(3)));
    assert!(npc.path.is_none(), "the walk is the cutscene's, not the schedule's");
    let still = view(&ctx, &state);
    assert_eq!((still.actors[0].direction.as_str(), still.actors[0].moving), ("down", false));
}

#[test]
fn moves_skip_missing_actors_and_blocked_tiles_and_jump_where_no_path_goes() {
    let (ctx, mut state) = with_cutscene(
        json!([
            {"type": "move", "actor": "npc-nobody", "x": 1, "y": 1},
            {"type": "move", "actor": "player", "x": 4, "y": 4},
            {"type": "move", "actor": "player", "x": 40, "y": 1},
            {"type": "move", "actor": "npc-test", "x": 0, "y": 0},
            set_flag("done"),
        ]),
        |project| {
            // Wall off the top-left corner tile (0, 0).
            for (x, y) in [(1, 0), (0, 1), (1, 1)] {
                project.scenes[0].tiles[y][x].collision = true;
            }
            project.npcs[0].x = units::tiles(2);
            project.npcs[0].y = units::tiles(2);
        },
    );
    start(&ctx, &mut state);
    let player = (state.player.x, state.player.y);
    tick(&ctx, &mut state, 1);
    assert!(flag_is_true(&state, "done"), "every move finished at once");
    assert_eq!((state.player.x, state.player.y), player, "the wall and the tile outside the scene were skipped");
    let npc = &state.npcs["npc-test"];
    assert_eq!((npc.x, npc.y), (0, 0), "no path reaches the corner: the NPC is put there");
}

#[test]
fn faces_turn_the_player_and_npcs() {
    let (ctx, mut state) = playing(json!([
        {"type": "face", "actor": "player", "direction": "down"},
        {"type": "face", "actor": "npc-test", "direction": "left"},
        {"type": "face", "actor": "npc-test", "direction": "sideways"},
        {"type": "say", "text": "..."},
    ]));
    tick(&ctx, &mut state, 1);
    assert_eq!(state.player.direction, "down");
    let shown = view(&ctx, &state);
    assert_eq!(shown.actors.len(), 1);
    assert_eq!((shown.actors[0].actor.as_str(), shown.actors[0].direction.as_str()), ("npc-test", "left"));
}

#[test]
fn emotes_show_over_their_actor_for_their_seconds() {
    let (ctx, mut state) = playing(json!([
        {"type": "emote", "actor": "npc-test", "emote": "heart"},
        {"type": "emote", "actor": "player", "emote": "dance"},
        set_flag("done"),
    ]));
    tick(&ctx, &mut state, 1);
    let shown = view(&ctx, &state);
    assert_eq!(shown.emotes.len(), 1);
    let emote = &shown.emotes[0];
    assert_eq!((emote.actor.as_str(), emote.emote.as_str(), emote.progress), ("npc-test", "heart", 0));
    assert_eq!((emote.x, emote.y), (units::tile_center(1), units::tile_center(1)));
    tick(&ctx, &mut state, 15);
    assert_eq!(view(&ctx, &state).emotes[0].progress, 500);
    // 1.5 seconds by default; an unknown emote is skipped.
    tick(&ctx, &mut state, 14);
    assert!(!state.flags.contains_key("done"));
    tick(&ctx, &mut state, 1);
    assert!(flag_is_true(&state, "done"));
}

#[test]
fn the_camera_pans_follows_and_returns_to_the_player() {
    let (ctx, mut state) = playing(json!([
        {"type": "camera", "x": 0, "y": 0, "seconds": 1},
        {"type": "camera", "actor": "npc-test", "seconds": 0},
        {"type": "say", "text": "Look."},
        {"type": "camera", "seconds": 0.5},
        {"type": "say", "text": "Back."},
    ]));
    let player = (state.player.x, state.player.y);
    tick(&ctx, &mut state, 1);
    let camera = view(&ctx, &state).camera.unwrap();
    assert_eq!((camera.x, camera.y), player);
    assert_eq!((camera.to_x, camera.to_y, camera.progress), (units::tile_center(0), units::tile_center(0), 0));
    tick(&ctx, &mut state, 10);
    let camera = view(&ctx, &state).camera.unwrap();
    assert_eq!(camera.progress, 500);
    assert_eq!(camera.x, (player.0 + units::tile_center(0)) / 2);
    tick(&ctx, &mut state, 10);
    // On the NPC at once, following it.
    let camera = view(&ctx, &state).camera.unwrap();
    assert_eq!(camera.actor.as_deref(), Some("npc-test"));
    assert_eq!((camera.x, camera.y), (units::tile_center(1), units::tile_center(1)));
    state.npcs.get_mut("npc-test").unwrap().x = units::tiles(2);
    assert_eq!(view(&ctx, &state).camera.unwrap().x, units::tile_center(2));
    assert_eq!(line(&ctx, &state).as_deref(), Some("Look."));

    run(&ctx, &mut state, Command::AdvanceCutscene);
    assert_eq!(view(&ctx, &state).camera.unwrap().actor.as_deref(), Some("player"));
    tick(&ctx, &mut state, 10);
    assert_eq!(view(&ctx, &state).camera, None, "back on the player");
    assert_eq!(line(&ctx, &state).as_deref(), Some("Back."));
}

#[test]
fn fades_go_out_to_a_colour_and_back_in() {
    let (ctx, mut state) = playing(json!([
        {"type": "fade", "mode": "out", "color": "#ffffff", "seconds": 1},
        {"type": "say", "text": "..."},
        {"type": "fade", "mode": "in", "seconds": 0.5},
        {"type": "fade", "mode": "sideways"},
        {"type": "fade", "mode": "out"},
        {"type": "say", "text": "Dark."},
    ]));
    tick(&ctx, &mut state, 6);
    let fade = view(&ctx, &state).fade.unwrap();
    assert_eq!((fade.color.as_str(), fade.alpha), ("#ffffff", 250));
    tick(&ctx, &mut state, 15);
    assert_eq!(view(&ctx, &state).fade.unwrap().alpha, 1000);
    run(&ctx, &mut state, Command::AdvanceCutscene);
    tick(&ctx, &mut state, 5);
    let fade = view(&ctx, &state).fade.unwrap();
    assert_eq!((fade.color.as_str(), fade.alpha), ("#ffffff", 500), "fading in keeps the colour");
    tick(&ctx, &mut state, 5);
    // In, then out to black over the default second (the unknown mode is skipped).
    tick(&ctx, &mut state, 20);
    let fade = view(&ctx, &state).fade.unwrap();
    assert_eq!((fade.color.as_str(), fade.alpha), ("#000000", 1000));
    assert_eq!(line(&ctx, &state).as_deref(), Some("Dark."));
    run(&ctx, &mut state, Command::AdvanceCutscene);
    assert_eq!(state.cutscene, None, "the screen clears with the end");
}

#[test]
fn music_and_sounds_play() {
    let (ctx, mut state) = playing(json!([
        {"type": "music", "soundId": "theme-love"},
        {"type": "sound", "soundId": "chime"},
        {"type": "sound"},
        {"type": "say", "text": "..."},
        {"type": "music", "soundId": ""},
        {"type": "say", "text": "..."},
    ]));
    let effects = tick(&ctx, &mut state, 1);
    assert_eq!(state.music.as_deref(), Some("theme-love"));
    assert_eq!(effects, vec![Effect::Sound { id: "chime".to_owned() }]);
    run(&ctx, &mut state, Command::AdvanceCutscene);
    assert_eq!(state.music, None, "empty: the place's music again");
}

// --- robustness ---

#[test]
fn unknown_steps_are_skipped_and_steps_that_loop_without_waiting_end_the_cutscene() {
    let (ctx, mut state) = playing(json!([{"type": "teleport"}, {}, set_flag("on")]));
    tick(&ctx, &mut state, 1);
    assert!(flag_is_true(&state, "on"));

    let (ctx, mut state) = playing(json!([{"type": "jump", "label": "loop", "goto": "loop"}]));
    let effects = tick(&ctx, &mut state, 1);
    assert_eq!(state.cutscene, None);
    assert!(common::has_message(&effects, |text| text.contains("loop")), "{effects:?}");
}

#[test]
fn a_cutscene_the_content_lacks_ends() {
    let (ctx, mut state) = playing(json!([{"type": "say", "text": "..."}]));
    let ctx = EngineContext::new(farm_sim::GameContent { cutscenes: Vec::new(), ..ctx.content.clone() });
    let before = state.clone();
    run(&ctx, &mut state, Command::AdvanceCutscene);
    assert_eq!(state, before, "refused");
    tick(&ctx, &mut state, 1);
    assert_eq!(state.cutscene, None);
}

#[test]
fn one_cutscene_plays_at_a_time() {
    let (ctx, mut state) = playing(json!([
        {"type": "outcomes", "outcomes": [{"type": "startCutscene", "cutsceneId": "scene"}]},
        {"type": "say", "text": "Once."},
    ]));
    tick(&ctx, &mut state, 1);
    assert_eq!(line(&ctx, &state).as_deref(), Some("Once."));
    assert_eq!(state.cutscene.as_ref().unwrap().step, 1);
}

// --- skipping ---

#[test]
fn skipping_runs_outcomes_music_and_moves_and_stops_at_a_choice() {
    let (ctx, mut state) = playing(json!([
        {"type": "move", "actor": "player", "x": 1, "y": 4},
        {"type": "move", "actor": "npc-test", "x": 1, "y": 2, "wait": false},
        {"type": "say", "text": "Skipped."},
        {"type": "music", "soundId": "theme"},
        {"type": "sound", "soundId": "chime"},
        {"type": "emote", "actor": "player", "emote": "idea"},
        {"type": "fade", "mode": "out"},
        set_flag("ran"),
        {"type": "jump", "goto": "ask"},
        set_flag("jumped-over"),
        {"type": "choice", "label": "ask", "text": "Well?", "options": [{"text": "Yes"}]},
        set_flag("answered"),
    ]));
    tick(&ctx, &mut state, 2);
    let effects = run(&ctx, &mut state, Command::SkipCutscene);
    assert!(effects.is_empty(), "no sound: {effects:?}");
    assert_eq!((state.player.x, state.player.y), (units::tile_center(1), units::tile_center(4)));
    let npc = &state.npcs["npc-test"];
    assert_eq!((npc.x, npc.y), (units::tiles(1), units::tiles(2)));
    assert_eq!(state.music.as_deref(), Some("theme"));
    assert!(flag_is_true(&state, "ran") && !state.flags.contains_key("jumped-over"));
    let shown = view(&ctx, &state);
    assert_eq!(shown.line.unwrap().text, "Well?");
    assert_eq!((shown.fade, shown.emotes.len(), shown.camera), (None, 0, None));
    run(&ctx, &mut state, Command::ChooseCutsceneOption { index: 0 });
    assert!(flag_is_true(&state, "answered"));
    assert_eq!(state.cutscene, None);
}

#[test]
fn skipping_to_the_end_ends_it() {
    let (ctx, mut state) = playing(json!([{"type": "say", "text": "..."}, set_flag("ran"), {"type": "say"}]));
    tick(&ctx, &mut state, 1);
    run(&ctx, &mut state, Command::SkipCutscene);
    assert!(flag_is_true(&state, "ran"));
    assert_eq!(state.cutscene, None);
}

#[test]
fn unskippable_cutscenes_refuse_to_skip() {
    let (ctx, mut state) = with_cutscene(json!([{"type": "say", "text": "Listen."}, set_flag("ran")]), |project| {
        project.cutscenes.as_mut().unwrap()[0].skippable = false;
    });
    start(&ctx, &mut state);
    tick(&ctx, &mut state, 1);
    assert!(!view(&ctx, &state).skippable);
    let before = state.clone();
    run(&ctx, &mut state, Command::SkipCutscene);
    assert_eq!(state, before);
    // Scripts are refused nothing, but the handler still keeps the cutscene going.
    let scripted = EngineContext::new(ctx.content.clone()).with_rules(CommandRules::Scripted);
    run(&scripted, &mut state, Command::SkipCutscene);
    assert_eq!(state, before);
}

// --- a modal ---

#[test]
fn a_cutscene_refuses_other_commands_and_freezes_the_player() {
    let (ctx, mut state) = playing(json!([{"type": "say", "text": "Hold on."}]));
    tick(&ctx, &mut state, 1);
    ctx.drain_hook_events();
    let before = state.clone();
    for command in [
        Command::Move { dir: "left".to_owned() },
        Command::Interact,
        Command::Sleep,
        Command::UseTool { tool: "hoe".to_owned() },
        Command::CloseDialogue,
        Command::ChooseCutsceneOption { index: 0 },
        Command::ChooseCutsceneOption { index: -1 },
    ] {
        run(&ctx, &mut state, command.clone());
        assert_eq!(state, before, "{command:?} was refused");
    }
    assert!(ctx.drain_hook_events().is_empty(), "refused commands emit no onCommand");
    // The intent is stored but the body stays put, and the clock waits (pauseInModals).
    run(&ctx, &mut state, Command::SetMoveIntent { dx: 1, dy: 0 });
    let (x, minutes) = (state.player.x, state.clock.time_minutes);
    tick(&ctx, &mut state, 40);
    assert_eq!((state.player.x, state.clock.time_minutes), (x, minutes));
    assert!(engine::modal_open(&state));
}

#[test]
fn cutscene_commands_without_a_cutscene_are_refused() {
    let (ctx, mut state) = with_cutscene(json!([{"type": "say", "text": "..."}]), |_| {});
    let before = state.clone();
    for command in [Command::AdvanceCutscene, Command::ChooseCutsceneOption { index: 0 }, Command::SkipCutscene] {
        run(&ctx, &mut state, command);
        assert_eq!(state, before);
    }
    assert!(ctx.drain_hook_events().is_empty());
}

#[test]
fn with_the_clock_running_directed_npcs_keep_off_their_schedules() {
    let (ctx, mut state) = with_cutscene(
        json!([{"type": "face", "actor": "npc-test", "direction": "up"}, {"type": "wait", "seconds": 120}]),
        |project| {
            project.settings.time.pause_in_modals = Some(false);
            let npc = &mut project.npcs[0];
            npc.can_move = true;
            npc.move_pattern = Some("patrol".to_owned());
            npc.patrol_points = Some(vec![farm_sim::schema::GridPoint { x: 1, y: 3 }]);
        },
    );
    start(&ctx, &mut state);
    let minutes = state.clock.time_minutes;
    tick(&ctx, &mut state, 20 * 30);
    assert!(state.clock.time_minutes > minutes, "the clock runs");
    let npc = &state.npcs["npc-test"];
    assert_eq!((npc.x, npc.y), (units::tiles(1), units::tiles(1)), "the patrol waits");
    // Once the cutscene ends the patrol goes on.
    cutscenes::stop(&mut state);
    tick(&ctx, &mut state, 20 * 10);
    assert_ne!(state.npcs["npc-test"].y, units::tiles(1));
}

#[test]
fn a_dialogue_opened_by_an_outcome_holds_the_cutscene_until_it_closes() {
    let (ctx, mut state) = playing(json!([
        {"type": "outcomes", "outcomes": [{"type": "startDialogue", "npcId": "npc-test", "dialogueId": "dlg-2"}]},
        {"type": "say", "text": "After the talk."},
    ]));
    tick(&ctx, &mut state, 1);
    assert_eq!(
        state.dialogue,
        Some(DialogueState { npc_id: "npc-test".to_owned(), dialogue_id: "dlg-2".to_owned(), line: None })
    );
    let shown = view(&ctx, &state);
    assert!(shown.line.is_none() && !shown.skippable);
    // The dialogue's commands apply, the cutscene's wait.
    let before = state.clone();
    run(&ctx, &mut state, Command::SkipCutscene);
    assert_eq!(state, before);
    tick(&ctx, &mut state, 50);
    assert_eq!(state.cutscene, before.cutscene);
    run(&ctx, &mut state, Command::CloseDialogue);
    assert_eq!(state.dialogue, None);
    assert_eq!(line(&ctx, &state).as_deref(), Some("After the talk."));
    tick(&ctx, &mut state, 1);
    run(&ctx, &mut state, Command::AdvanceCutscene);
    assert_eq!(state.cutscene, None);
}

#[test]
fn a_night_ends_the_cutscene() {
    let (ctx, mut state) = playing(json!([{"type": "move", "actor": "player", "x": 1, "y": 4}]));
    tick(&ctx, &mut state, 1);
    let scripted = EngineContext::new(ctx.content.clone()).with_rules(CommandRules::Scripted);
    run(&scripted, &mut state, Command::Sleep);
    assert_eq!(state.cutscene, None);
}

// --- determinism and saves ---

/// A cutscene with every kind of timed step, and the commands (by tick) that play it through.
fn script() -> (EngineContext, GameState, Vec<Option<Command>>) {
    let (ctx, state) = playing(json!([
        {"type": "fade", "mode": "in", "seconds": 0.5},
        {"type": "move", "actor": "npc-test", "x": 2, "y": 3, "wait": false},
        {"type": "move", "actor": "player", "x": 0, "y": 5},
        {"type": "camera", "actor": "npc-test", "seconds": 0.75},
        {"type": "emote", "actor": "npc-test", "emote": "exclaim"},
        {"type": "say", "speaker": "npc-test", "text": "Oh!"},
        {"type": "choice", "text": "?", "options": [{"text": "a", "goto": "a"}, {"text": "b"}]},
        {"type": "wait", "seconds": 0.3},
        {"type": "outcomes", "label": "a", "outcomes": [{"type": "giveMoney", "amount": 5}]},
        {"type": "fade", "mode": "out"},
    ]));
    let mut inputs = vec![None; 140];
    inputs[100] = Some(Command::AdvanceCutscene);
    inputs[101] = Some(Command::ChooseCutsceneOption { index: 0 });
    (ctx, state, inputs)
}

/// Plays [`script`], one tick after each input; the hash after every tick, and the end state.
fn play_script() -> (Vec<String>, GameState) {
    let (ctx, mut state, inputs) = script();
    let mut hashes = Vec::new();
    for input in &inputs {
        if let Some(command) = input {
            run(&ctx, &mut state, command.clone());
        }
        tick(&ctx, &mut state, 1);
        hashes.push(hash_state(&state));
    }
    (hashes, state)
}

#[test]
fn cutscenes_are_deterministic_at_any_tick_batching() {
    let (once, state) = play_script();
    assert_eq!(play_script().0, once);
    assert_eq!(state.player.money, 105, "the branch paid out");
    assert_eq!((state.player.x, state.player.y), (units::tile_center(0), units::tile_center(5)));
    assert_eq!(state.cutscene, None);

    // advance_tick(n) is n × advance_tick(1) under a cutscene too.
    let (ctx, mut whole, _) = script();
    let mut single = whole.clone();
    tick(&ctx, &mut whole, 90);
    for _ in 0..90 {
        tick(&ctx, &mut single, 1);
    }
    assert_eq!(whole, single);
    assert_eq!(line(&ctx, &whole).as_deref(), Some("Oh!"));
}

#[test]
fn a_cutscene_in_the_middle_of_its_steps_saves_and_loads() {
    let (ctx, mut state, _) = script();
    let mut seen = (false, false, false);
    for _ in 0..80 {
        tick(&ctx, &mut state, 1);
        let running = state.cutscene.clone().unwrap();
        seen.0 |= !running.walks.is_empty();
        seen.1 |= running.camera.is_some();
        seen.2 |= !running.emotes.is_empty() && !running.facing.is_empty();
        let json = farm_sim::stable_stringify(&state);
        let loaded: GameState = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, state);
        assert_eq!(hash_state(&loaded), hash_state(&state));
    }
    assert_eq!(seen, (true, true, true), "walks, a pan and an emote were saved");

    // A game loaded mid-walk plays on exactly like the running one.
    let (ctx, mut state, _) = script();
    tick(&ctx, &mut state, 20);
    assert_eq!(state.cutscene.as_ref().unwrap().walks.len(), 2);
    let mut loaded: GameState = serde_json::from_str(&farm_sim::stable_stringify(&state)).unwrap();
    tick(&ctx, &mut state, 40);
    tick(&ctx, &mut loaded, 40);
    assert_eq!(loaded, state);

    // A state without a cutscene writes no cutscene key at all.
    let (_, idle) = with_cutscene(json!([]), |_| {});
    let json: Value = serde_json::from_str(&farm_sim::stable_stringify(&idle)).unwrap();
    assert!(json.get("cutscene").is_none(), "{:?}", json.get("cutscene"));
}

#[test]
fn the_view_serializes_in_authoring_units() {
    let (ctx, mut state) = playing(json!([
        {"type": "camera", "x": 2, "y": 2, "seconds": 1},
        {"type": "say", "speaker": "npc-test", "text": "Hi"},
    ]));
    tick(&ctx, &mut state, 11);
    let json = serde_json::to_value(view(&ctx, &state)).unwrap();
    assert_eq!(json["cutsceneId"], "scene");
    assert_eq!(json["camera"]["toX"], 2.5);
    assert_eq!(json["camera"]["progress"], 0.5);
    assert!(json.get("line").is_none() && json.get("fade").is_none());
    tick(&ctx, &mut state, 10);
    let json = serde_json::to_value(view(&ctx, &state)).unwrap();
    assert_eq!(json["line"], json!({"text": "Hi", "speaker": "npc-test", "speakerName": "Testy", "options": []}));
}
