//! The rules of a game (#243, `settings.rules`): farming, stamina and sleep are selectable, and the
//! four things a classic night does together (the date rolling over with the night's work,
//! recovery, the collapse penalty, moving the player) are separate. A project without `rules`
//! plays the classic farming day, tick for tick.

use farm_sim::engine::{advance_tick, apply_command};
use farm_sim::schema::{GameProject, GameRules, GameState, Rules, WakeSpot};
use farm_sim::{hash_state, state, units, Command, Effect, Effects, EngineContext};
use std::path::PathBuf;

fn starter_farm_project() -> GameProject {
    let path: PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "..", "..", "fixtures", "golden", "content", "starter-farm.json"].iter().collect();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let fixture: serde_json::Value = serde_json::from_str(&text).expect("valid fixture JSON");
    serde_json::from_value(fixture["project"].clone()).expect("starter-farm project is a GameProject")
}

fn engine(mutate: impl FnOnce(&mut GameProject)) -> (EngineContext, GameState) {
    let mut project = starter_farm_project();
    mutate(&mut project);
    let ctx = EngineContext::new(state::create_content_from_project(&project));
    let state = state::create_game_state(&project, Some("rules"));
    (ctx, state)
}

fn rules(sleep: &str) -> GameRules {
    GameRules { sleep: Some(sleep.to_owned()), ..GameRules::default() }
}

/// Fast days: an in-game hour per tick.
fn fast(project: &mut GameProject) {
    project.settings.time.minutes_per_real_second = units::MINUTE * 60;
}

fn said(effects: &Effects, text: &str) -> bool {
    effects.iter().any(|effect| matches!(effect, Effect::Message { text: said, .. } if said.contains(text)))
}

fn day_starts(effects: &Effects) -> usize {
    effects.iter().filter(|effect| matches!(effect, Effect::DayStarted { .. })).count()
}

/// Ticks until the day changes; the effects of the tick that changed it.
fn run_to_next_day(ctx: &EngineContext, state: &mut GameState) -> Effects {
    let day = state.clock.day;
    for _ in 0..10_000 {
        let effects = advance_tick(ctx, state, 1);
        if state.clock.day != day {
            return effects;
        }
    }
    panic!("the day never ended");
}

#[test]
fn a_project_without_rules_plays_the_classic_day() {
    let (ctx, mut classic) = engine(fast);
    assert_eq!(Rules::of(&ctx.content.settings), Rules::CLASSIC);
    // Spelling the classic rules out changes nothing: the same hash after a collapse and a sleep.
    let (spelled, mut same) = engine(|project| {
        fast(project);
        project.settings.rules = Some(GameRules {
            sleep: Some("required".into()),
            day_end: Some("collapse".into()),
            exhaustion: Some("collapse".into()),
            farming: Some(true),
            ..GameRules::default()
        });
    });
    let effects = run_to_next_day(&ctx, &mut classic);
    assert!(said(&effects, "collapsed"), "{effects:?}");
    run_to_next_day(&spelled, &mut same);
    apply_command(&ctx, &mut classic, &Command::Sleep);
    apply_command(&spelled, &mut same, &Command::Sleep);
    assert_eq!(classic.clock.day, 3);
    assert_eq!(hash_state(&classic), hash_state(&same));
}

#[test]
fn a_workshop_without_sleep_runs_round_the_clock() {
    let (ctx, mut state) = engine(|project| {
        fast(project);
        project.settings.rules = Some(rules("none"));
    });
    let resolved = Rules::of(&ctx.content.settings);
    assert!(resolved.continuous && !resolved.can_sleep && !resolved.day_end_collapses);
    let money = state.player.money;
    state.player.energy = units::points(10);
    state.dialogue = Some(Default::default());
    // Sleep is not part of the game.
    let effects = apply_command(&ctx, &mut state, &Command::CloseDialogue);
    assert!(effects.is_empty());
    let effects = apply_command(&ctx, &mut state, &Command::Sleep);
    assert!(said(&effects, "no sleeping"), "{effects:?}");
    assert_eq!(state.clock.day, 1);

    // The clock passes the classic day's end (2 AM would be 26:00) without a collapse …
    let mut minutes = Vec::new();
    let mut effects = Vec::new();
    while state.clock.day == 1 {
        minutes.push(units::whole_minute(state.clock.time_minutes));
        effects = advance_tick(&ctx, &mut state, 1);
    }
    assert_eq!(minutes.last(), Some(&(23 * 60)), "the last hour of day 1 is 23:00: {minutes:?}");
    // … and midnight rolls the date: no penalty, the clock carries on from 0:00.
    assert_eq!(day_starts(&effects), 1);
    assert!(!said(&effects, "collapsed"));
    assert_eq!(state.clock.day, 2);
    assert_eq!(units::whole_minute(state.clock.time_minutes), 0);
    assert_eq!(state.player.money, money);
    // Nobody sleeps, so the rollover is the rest: energy is back.
    assert_eq!(state.player.energy, state.player.max_energy);
    // Day 2 runs its 24 hours and rolls again at the next midnight.
    let before = state.clock.tick;
    run_to_next_day(&ctx, &mut state);
    assert_eq!(state.clock.tick - before, 24);
    assert_eq!(state.clock.day, 3);
}

#[test]
fn a_continuous_day_does_not_close_what_the_player_has_open() {
    let (ctx, mut state) = engine(|project| {
        fast(project);
        project.settings.rules = Some(rules("none"));
        // The clock runs on while a shop is open.
        project.settings.time.pause_in_modals = Some(false);
    });
    let shop = ctx.content.shops[0].id.clone();
    state.shop = Some(farm_sim::schema::ShopSession { shop_id: shop });
    run_to_next_day(&ctx, &mut state);
    assert!(state.shop.is_some(), "midnight passes over an open shop");
    // A classic collapse closes it.
    let (ctx, mut state) = engine(|project| {
        fast(project);
        project.settings.time.pause_in_modals = Some(false);
    });
    state.shop = Some(farm_sim::schema::ShopSession { shop_id: ctx.content.shops[0].id.clone() });
    run_to_next_day(&ctx, &mut state);
    assert!(state.shop.is_none());
}

#[test]
fn machines_work_through_a_midnight_nobody_slept() {
    // A job loaded in the evening is done the next morning in a continuous game: the absolute
    // clock runs on across the rollover.
    let (ctx, mut state) = engine(|project| {
        fast(project);
        project.settings.rules = Some(rules("none"));
    });
    let before = farm_sim::buffs::now(&ctx, &state);
    run_to_next_day(&ctx, &mut state);
    let after = farm_sim::buffs::now(&ctx, &state);
    // 6:00 → 24:00 is 18 hours, and the clock did not jump.
    assert_eq!(after - before, 18 * 60 * i64::from(units::MINUTE), "{before} → {after}");
}

#[test]
fn optional_sleep_rolls_the_date_at_the_days_end_without_a_collapse() {
    let (ctx, mut state) = engine(|project| {
        fast(project);
        project.settings.rules = Some(rules("optional"));
    });
    let resolved = Rules::of(&ctx.content.settings);
    assert!(resolved.can_sleep && !resolved.continuous && !resolved.day_end_collapses);
    let money = state.player.money;
    state.player.energy = units::points(40);
    let effects = run_to_next_day(&ctx, &mut state);
    assert_eq!(day_starts(&effects), 1);
    assert!(!said(&effects, "collapsed"));
    assert_eq!(state.player.money, money, "no collapse penalty");
    // The new day starts at the day's start, and a night without sleep gives no energy back.
    assert_eq!(units::whole_minute(state.clock.time_minutes), ctx.content.settings.time.day_start_minute);
    assert_eq!(state.player.energy, units::points(40));
    // Sleeping is still there, and is a night's rest.
    apply_command(&ctx, &mut state, &Command::Sleep);
    assert_eq!(state.clock.day, 3);
    assert_eq!(state.player.energy, state.player.max_energy);

    // A share of the energy can come back with the date.
    let (ctx, mut state) = engine(|project| {
        fast(project);
        project.settings.rules = Some(GameRules { rollover_energy_fraction: Some(250), ..rules("optional") });
    });
    state.player.energy = units::points(40);
    run_to_next_day(&ctx, &mut state);
    assert_eq!(state.player.energy, units::points(65));
}

fn use_hoe(ctx: &EngineContext, state: &mut GameState) -> Effects {
    // Face open grass: the starter farm's player stands on it.
    apply_command(ctx, state, &Command::UseTool { tool: "hoe".into() })
}

#[test]
fn running_out_of_energy_wears_the_player_out_where_it_does_not_collapse_them() {
    let (ctx, mut state) = engine(|project| project.settings.rules = Some(rules("optional")));
    assert!(!Rules::of(&ctx.content.settings).exhaustion_collapses);
    state.player.energy = units::points(1);
    let day = state.clock.day;
    let effects = use_hoe(&ctx, &mut state);
    // The swing that empties the energy works; the day goes on.
    assert_eq!(state.player.energy, 0, "{effects:?}");
    assert_eq!(state.clock.day, day);
    assert!(said(&effects, "worn out"), "{effects:?}");
    // At zero, tools that cost energy do nothing.
    let hash = hash_state(&state);
    let effects = use_hoe(&ctx, &mut state);
    assert!(said(&effects, "too tired"), "{effects:?}");
    assert_eq!(hash_state(&state), hash);
    // Some energy back, and they work again.
    state.player.energy = units::points(50);
    let effects = use_hoe(&ctx, &mut state);
    assert!(!said(&effects, "too tired"), "{effects:?}");

    // The classic rule still collapses.
    let (ctx, mut state) = engine(|_| {});
    state.player.energy = units::points(1);
    let effects = use_hoe(&ctx, &mut state);
    assert!(said(&effects, "collapsed"), "{effects:?}");
    assert_eq!(state.clock.day, 2);
    // And a game can pair optional sleep with collapsing exhaustion.
    let (ctx, mut state) = engine(|project| {
        project.settings.rules = Some(GameRules { exhaustion: Some("collapse".into()), ..rules("optional") });
    });
    state.player.energy = units::points(1);
    use_hoe(&ctx, &mut state);
    assert_eq!(state.clock.day, 2);
}

#[test]
fn a_collapse_can_wake_the_player_somewhere() {
    let (ctx, mut state) = engine(|project| {
        fast(project);
        let scene = project.scenes[0].id.clone();
        project.settings.rules =
            Some(GameRules { collapse_wake: Some(WakeSpot { scene_id: scene, x: 2, y: 3 }), ..GameRules::default() });
    });
    let effects = run_to_next_day(&ctx, &mut state);
    assert!(said(&effects, "collapsed"));
    assert!(effects.iter().any(|effect| matches!(effect, Effect::SceneChanged { .. })), "{effects:?}");
    let scene = state.world.scenes.iter().find(|scene| scene.id == state.player.scene_id).unwrap();
    let (x, y) = (units::tile_of(state.player.x), units::tile_of(state.player.y));
    // On the spot, or the nearest walkable tile to it.
    assert!((x - 2).abs() <= 2 && (y - 3).abs() <= 2, "woke at ({x}, {y})");
    assert!(scene.tile(x, y).is_some());
    // A night's sleep leaves the player where they lay down.
    let (px, py) = (state.player.x, state.player.y);
    state.player.x += units::tiles(1);
    let moved = state.player.x;
    apply_command(&ctx, &mut state, &Command::Sleep);
    assert_eq!((state.player.x, state.player.y), (moved, py));
    assert_ne!(moved, px);
    // A wake spot in a scene the game lacks is ignored.
    let (ctx, mut state) = engine(|project| {
        fast(project);
        project.settings.rules = Some(GameRules {
            collapse_wake: Some(WakeSpot { scene_id: "nowhere".into(), x: 1, y: 1 }),
            ..GameRules::default()
        });
    });
    let scene = state.player.scene_id.clone();
    run_to_next_day(&ctx, &mut state);
    assert_eq!(state.player.scene_id, scene);
}

#[test]
fn a_game_without_farming_leaves_soil_and_crops_alone() {
    let off = |project: &mut GameProject| {
        project.settings.rules = Some(GameRules { farming: Some(false), ..GameRules::default() });
    };
    let (ctx, mut state) = engine(off);
    assert!(!Rules::of(&ctx.content.settings).farming);
    let hash = hash_state(&state);
    let energy = state.player.energy;
    let effects = use_hoe(&ctx, &mut state);
    assert!(said(&effects, "Nothing grows"), "{effects:?}");
    assert_eq!(state.player.energy, energy, "a refused tool costs nothing");
    assert_eq!(hash_state(&state), hash);
    let effects = apply_command(&ctx, &mut state, &Command::UseTool { tool: "watering-can".into() });
    assert!(said(&effects, "Nothing grows"), "{effects:?}");
    // Interacting with soil plants nothing.
    apply_command(&ctx, &mut state, &Command::Interact);
    let crops = |state: &GameState| -> usize {
        state.world.scenes.iter().flat_map(|scene| scene.tiles.iter().flatten()).filter(|t| t.crop.is_some()).count()
    };
    assert_eq!(crops(&state), 0);

    // A crop that is on the map anyway is scenery: nights neither grow nor wither it.
    let (farming, mut grown) = engine(|_| {});
    for state in [&mut state, &mut grown] {
        let tile = &mut state.world.scenes[0].tiles[2][2];
        tile.crop = Some(farm_sim::schema::Crop { r#type: "wheat".into(), watered: true, ..Default::default() });
    }
    let before = state.world.scenes[0].tiles[2][2].crop.clone();
    apply_command(&ctx, &mut state, &Command::Sleep);
    apply_command(&farming, &mut grown, &Command::Sleep);
    assert_eq!(state.world.scenes[0].tiles[2][2].crop, before);
    assert_ne!(grown.world.scenes[0].tiles[2][2].crop, before, "with farming the night touches the crop");
    // The other tools still work: the axe is not a farming tool.
    let effects = apply_command(&ctx, &mut state, &Command::UseTool { tool: "axe".into() });
    assert!(!said(&effects, "Nothing grows"), "{effects:?}");
}

#[test]
fn rules_read_and_write_as_project_json() {
    let json = serde_json::json!({
        "sleep": "none", "dayEnd": "rollover", "exhaustion": "rest", "rolloverEnergyFraction": 0.5,
        "farming": false, "collapseWake": { "sceneId": "scene-farm", "x": 4, "y": 5 }
    });
    let rules: GameRules = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(rules.rollover_energy_fraction, Some(500));
    assert_eq!(serde_json::to_value(&rules).unwrap(), json);
    // Nothing set writes nothing, and an unknown word keeps the classic rule.
    assert_eq!(serde_json::to_value(GameRules::default()).unwrap(), serde_json::json!({}));
    let settings = farm_sim::schema::ProjectSettings {
        rules: Some(GameRules { sleep: Some("sometimes".into()), ..GameRules::default() }),
        ..Default::default()
    };
    assert_eq!(Rules::of(&settings), Rules::CLASSIC);
    assert_eq!(serde_json::to_value(farm_sim::schema::ProjectSettings::default()).unwrap().get("rules"), None);
}
