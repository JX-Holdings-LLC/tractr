//! Creator-defined properties (#244): declared variables with a scope (the player, each scene,
//! each placed machine), bounds, change over game time and threshold effects, read and written
//! by the existing `variable` condition and `setVariable` / `addVariable` outcomes, kept in saves
//! with the game's other variables.
//!
//! The scenario is the roadmap's: two greenhouses with independent pressure, and a player with
//! oxygen beside stamina, in a game that runs round the clock.

use farm_sim::engine::{advance_tick, apply_command};
use farm_sim::schema::{
    EventCondition, EventOutcome, GameProject, GameRules, GameState, PropertyDefinition, PropertyRange, PropertyRate,
    PropertyThreshold, Scene,
};
use farm_sim::{hash_state, properties, state, units, Command, Effect, Effects, EngineContext};
use std::path::PathBuf;

const MILLI: i64 = 1000;

fn starter_farm_project() -> GameProject {
    let path: PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "..", "..", "fixtures", "golden", "content", "starter-farm.json"].iter().collect();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let fixture: serde_json::Value = serde_json::from_str(&text).expect("valid fixture JSON");
    serde_json::from_value(fixture["project"].clone()).expect("starter-farm project is a GameProject")
}

fn variable(name: &str, compare: &str, value: i64) -> EventCondition {
    EventCondition::Variable { name: name.into(), compare: compare.into(), value: Some(value * MILLI), text: None }
}

fn outcome(json: serde_json::Value) -> EventOutcome {
    serde_json::from_value(json).expect("an outcome")
}

fn greenhouse(base: &Scene, id: &str, pressure: Option<i64>) -> Scene {
    Scene {
        id: id.into(),
        name: id.into(),
        indoor: Some(true),
        properties: pressure.map(|value| [("pressure".to_owned(), value * MILLI)].into_iter().collect()),
        ..base.clone()
    }
}

/// The alien greenhouses: `pressure` per scene (it leaks 6 an hour; greenhouse B starts lower),
/// `oxygen` for the player (it drains 30 an hour where the pressure is under 50 and comes back
/// 60 an hour elsewhere), and stamina as it always was.
fn greenhouses(mutate: impl FnOnce(&mut GameProject)) -> (EngineContext, GameState) {
    let mut project = starter_farm_project();
    // An in-game minute per tick, round the clock.
    project.settings.time.minutes_per_real_second = units::MINUTE;
    project.settings.rules = Some(GameRules { sleep: Some("none".into()), ..GameRules::default() });
    let base = project.scenes[0].clone();
    project.scenes.push(greenhouse(&base, "greenhouse-a", None));
    project.scenes.push(greenhouse(&base, "greenhouse-b", Some(40)));
    project.properties = Some(vec![
        PropertyDefinition {
            id: "pressure".into(),
            name: "Pressure".into(),
            scope: "scene".into(),
            unit: Some("kPa".into()),
            min: 0,
            max: 100 * MILLI,
            initial: 90 * MILLI,
            scenes: Some(vec!["greenhouse-a".into(), "greenhouse-b".into()]),
            rates: vec![PropertyRate { per_hour: -6 * MILLI, conditions: Vec::new() }],
            thresholds: vec![PropertyThreshold {
                compare: "lt".into(),
                value: 50 * MILLI,
                message: Some("The greenhouse is losing pressure!".into()),
                outcomes: vec![outcome(
                    serde_json::json!({ "type": "addVariable", "variable": "alarms", "amount": 1 }),
                )],
            }],
            hud: Some("meter".into()),
            ..PropertyDefinition::default()
        },
        PropertyDefinition {
            id: "oxygen".into(),
            name: "Oxygen".into(),
            scope: "player".into(),
            unit: Some("%".into()),
            rates: vec![
                PropertyRate { per_hour: -30 * MILLI, conditions: vec![variable("pressure", "lt", 50)] },
                PropertyRate {
                    per_hour: 60 * MILLI,
                    conditions: vec![EventCondition::Not { conditions: vec![variable("pressure", "lt", 50)] }],
                },
            ],
            thresholds: vec![PropertyThreshold {
                compare: "le".into(),
                value: 0,
                message: Some("You are out of air.".into()),
                outcomes: vec![outcome(serde_json::json!({ "type": "modifyEnergy", "amount": -25 }))],
            }],
            hud: Some("meter".into()),
            ..PropertyDefinition::default()
        },
    ]);
    mutate(&mut project);
    let ctx = EngineContext::new(state::create_content_from_project(&project));
    let state = state::create_game_state(&project, Some("greenhouses"));
    (ctx, state)
}

fn enter(state: &mut GameState, scene: &str) {
    state.player.scene_id = scene.to_owned();
}

fn read(ctx: &EngineContext, state: &GameState, name: &str) -> i64 {
    properties::read(ctx, state, name).unwrap_or_else(|| panic!("{name} is a property")) / MILLI
}

fn hours(ctx: &EngineContext, state: &mut GameState, hours: u64) -> Effects {
    advance_tick(ctx, state, hours * 60)
}

fn said(effects: &Effects, text: &str) -> usize {
    effects.iter().filter(|effect| matches!(effect, Effect::Message { text: said, .. } if said.contains(text))).count()
}

#[test]
fn two_greenhouses_keep_their_own_pressure() {
    let (ctx, mut state) = greenhouses(|_| {});
    // Each scene starts at its own value: the property's, or the scene's.
    assert_eq!(read(&ctx, &state, "pressure@greenhouse-a"), 90);
    assert_eq!(read(&ctx, &state, "pressure@greenhouse-b"), 40);
    // A scene the property does not list has none, and the bare name means where the player is.
    assert_eq!(properties::read(&ctx, &state, "pressure"), None);
    enter(&mut state, "greenhouse-a");
    assert_eq!(read(&ctx, &state, "pressure"), 90);
    assert_eq!(read(&ctx, &state, "pressure@here"), 90);
    enter(&mut state, "greenhouse-b");
    assert_eq!(read(&ctx, &state, "pressure"), 40);

    // Both leak 6 an hour, wherever the player is.
    hours(&ctx, &mut state, 2);
    assert_eq!(read(&ctx, &state, "pressure@greenhouse-a"), 78);
    assert_eq!(read(&ctx, &state, "pressure@greenhouse-b"), 28);

    // Pumping one up (an action's outcome, written where the player stands) leaves the other.
    let pump = outcome(serde_json::json!({ "type": "addVariable", "variable": "pressure", "amount": 50 }));
    farm_sim::events::apply_outcomes(&ctx, &mut state, std::slice::from_ref(&pump), 0);
    assert_eq!(read(&ctx, &state, "pressure@greenhouse-b"), 78);
    assert_eq!(read(&ctx, &state, "pressure@greenhouse-a"), 78);
    // Naming the other scene writes there; a write cannot leave the bounds.
    let flood =
        outcome(serde_json::json!({ "type": "setVariable", "variable": "pressure@greenhouse-a", "amount": 500 }));
    farm_sim::events::apply_outcomes(&ctx, &mut state, std::slice::from_ref(&flood), 0);
    assert_eq!(read(&ctx, &state, "pressure@greenhouse-a"), 100);
    assert_eq!(read(&ctx, &state, "pressure@greenhouse-b"), 78);
    // The values are the game's variables, so saves, test states and plugins see them.
    assert_eq!(state.flags.get("pressure@greenhouse-a"), Some(&serde_json::json!(100)));
    assert_eq!(state.flags.get("pressure@greenhouse-b"), Some(&serde_json::json!(78)));
}

#[test]
fn oxygen_drains_beside_stamina_and_thresholds_fire_once() {
    let (ctx, mut state) = greenhouses(|_| {});
    let energy = state.player.energy;
    assert_eq!(read(&ctx, &state, "oxygen"), 100);
    // In the thin greenhouse oxygen drains 30 an hour; stamina is untouched by it.
    enter(&mut state, "greenhouse-b");
    hours(&ctx, &mut state, 2);
    assert_eq!(read(&ctx, &state, "oxygen"), 40);
    assert_eq!(state.player.energy, energy);
    // It runs out after 3 h 20 min: the threshold says so once and takes 25 energy, once.
    let effects = hours(&ctx, &mut state, 3);
    assert_eq!(read(&ctx, &state, "oxygen"), 0);
    assert_eq!(said(&effects, "out of air"), 1, "{effects:?}");
    assert_eq!(state.player.energy, energy - units::points(25));
    let effects = hours(&ctx, &mut state, 1);
    assert_eq!(said(&effects, "out of air"), 0);
    assert_eq!(state.player.energy, energy - units::points(25));
    // Where the pressure holds (greenhouse A, pumped back up) it comes back at 60 an hour, up to
    // its most.
    let pump =
        outcome(serde_json::json!({ "type": "setVariable", "variable": "pressure@greenhouse-a", "amount": 100 }));
    farm_sim::events::apply_outcomes(&ctx, &mut state, std::slice::from_ref(&pump), 0);
    enter(&mut state, "greenhouse-a");
    hours(&ctx, &mut state, 1);
    assert_eq!(read(&ctx, &state, "oxygen"), 60);
    hours(&ctx, &mut state, 1);
    assert_eq!(read(&ctx, &state, "oxygen"), 100);
    // Crossing again fires again.
    enter(&mut state, "greenhouse-b");
    let effects = hours(&ctx, &mut state, 4);
    assert_eq!(said(&effects, "out of air"), 1);
    assert_eq!(state.player.energy, energy - units::points(50));
}

#[test]
fn a_scenes_threshold_runs_for_that_scene() {
    let (ctx, mut state) = greenhouses(|_| {});
    // Greenhouse A falls under 50 after 6 h 40 min of leaking from 90; B started under it.
    let effects = hours(&ctx, &mut state, 7);
    assert_eq!(said(&effects, "losing pressure"), 1, "{effects:?}");
    assert_eq!(state.flags.get("alarms"), Some(&serde_json::json!(1)));
    assert_eq!(read(&ctx, &state, "pressure@greenhouse-a"), 48);
}

#[test]
fn a_shift_crosses_midnight_and_reloads_where_it_was() {
    let (ctx, mut state) = greenhouses(|_| {});
    enter(&mut state, "greenhouse-a");
    // From 6:00 to 1:30 the next morning: the date rolls under the player.
    let effects = advance_tick(&ctx, &mut state, 19 * 60 + 30);
    assert_eq!(state.clock.day, 2);
    assert_eq!(units::whole_minute(state.clock.time_minutes), 90);
    assert_eq!(effects.iter().filter(|effect| matches!(effect, Effect::DayStarted { .. })).count(), 1);
    // 19.5 hours of leaking, the midnight included: 90 − 117 is under the floor.
    assert_eq!(read(&ctx, &state, "pressure@greenhouse-a"), 0);
    assert_eq!(read(&ctx, &state, "pressure@greenhouse-b"), 0);

    // Save mid-shift and load: the same game, and it goes on the same.
    let saved = serde_json::to_string(&state).unwrap();
    let mut loaded: GameState = serde_json::from_str(&saved).unwrap();
    assert_eq!(hash_state(&loaded), hash_state(&state));
    let pump = Command::PluginMutation {
        plugin_id: "script/pump".into(),
        mutation: serde_json::from_value(
            serde_json::json!({ "type": "setVariable", "variable": "pressure@greenhouse-b", "value": 70 }),
        )
        .unwrap(),
    };
    for game in [&mut state, &mut loaded] {
        apply_command(&ctx, game, &pump);
        advance_tick(&ctx, game, 120);
    }
    assert_eq!(read(&ctx, &loaded, "pressure@greenhouse-b"), 58);
    assert_eq!(read(&ctx, &loaded, "pressure@greenhouse-a"), 0);
    assert_eq!(hash_state(&loaded), hash_state(&state));
}

#[test]
fn an_hour_moves_a_property_by_its_rate_at_any_clock_speed() {
    for rate in [units::MINUTE / 20, units::MINUTE, units::MINUTE * 7, units::MINUTE * 45] {
        let (ctx, mut state) = greenhouses(|project| {
            project.settings.time.minutes_per_real_second = rate;
            // 7 an hour does not divide into minutes evenly.
            project.properties.as_mut().unwrap()[0].rates[0].per_hour = -7 * MILLI;
        });
        while units::whole_minute(state.clock.time_minutes) < 9 * 60 {
            advance_tick(&ctx, &mut state, 1);
        }
        // Three whole hours passed (a fast clock may be some minutes into the fourth).
        let minute = i64::from(units::whole_minute(state.clock.time_minutes));
        let share = |minute: i64| (-7 * MILLI * minute).div_euclid(60);
        let expected = 90 * MILLI + share(minute) - share(360);
        assert_eq!(properties::read(&ctx, &state, "pressure@greenhouse-a"), Some(expected), "rate {rate}");
    }
}

#[test]
fn a_nights_sleep_passes_for_properties_too() {
    // The same greenhouses in a game with beds, leaking 1 an hour.
    let (ctx, mut state) = greenhouses(|project| {
        project.settings.rules = None;
        project.properties.as_mut().unwrap()[0].rates[0].per_hour = -MILLI;
    });
    hours(&ctx, &mut state, 2);
    assert_eq!(read(&ctx, &state, "pressure@greenhouse-a"), 88);
    // Asleep from 8:00 to 6:00: 22 more hours of leaking.
    apply_command(&ctx, &mut state, &Command::Sleep);
    assert_eq!(state.clock.day, 2);
    assert_eq!(read(&ctx, &state, "pressure@greenhouse-a"), 66);
    // Back to bed the minute they got up: a whole day.
    apply_command(&ctx, &mut state, &Command::Sleep);
    assert_eq!(read(&ctx, &state, "pressure@greenhouse-a"), 42);
    // Staying up to the day's end (2:00) and collapsing skips the four hours to 6:00.
    hours(&ctx, &mut state, 20);
    assert_eq!(state.clock.day, 4);
    assert_eq!(read(&ctx, &state, "pressure@greenhouse-a"), 42 - 20 - 4);
}

#[test]
fn crops_grow_only_where_a_property_suits_them() {
    let (ctx, mut state) = greenhouses(|project| {
        // No leak: greenhouse A stays at 90, B at 40.
        project.properties.as_mut().unwrap()[0].rates.clear();
        // Wheat needs at least 60 kPa.
        let wheat = project.custom_crops.get_or_insert_with(Vec::new);
        let mut crop: farm_sim::schema::CustomCropDefinition = serde_json::from_value(serde_json::json!({
            "id": "wheat", "name": "Wheat", "seedCost": 1, "baseHarvestValue": 5, "growthDays": 4, "stages": 4,
            "seasons": ["spring", "summer", "fall", "winter"]
        }))
        .unwrap();
        crop.requirements = Some(farm_sim::schema::GrowthRequirements {
            properties: Some(vec![PropertyRange { property: "pressure".into(), min: Some(60 * MILLI), max: None }]),
            ..Default::default()
        });
        wheat.retain(|existing| existing.id != "wheat");
        wheat.push(crop);
        project.settings.rules = None;
    });
    for scene in ["greenhouse-a", "greenhouse-b"] {
        let scene = state.world.scenes.iter_mut().find(|s| s.id == scene).unwrap();
        scene.tiles[2][2].crop =
            Some(farm_sim::schema::Crop { r#type: "wheat".into(), watered: true, ..Default::default() });
    }
    apply_command(&ctx, &mut state, &Command::Sleep);
    let grown = |state: &GameState, scene: &str| {
        let scene = state.world.scenes.iter().find(|s| s.id == scene).unwrap();
        scene.tiles[2][2].crop.as_ref().unwrap().days_grown.unwrap_or(0)
    };
    assert_eq!(grown(&state, "greenhouse-a"), 1, "90 kPa suits it");
    assert_eq!(grown(&state, "greenhouse-b"), 0, "40 kPa holds it back");
}

#[test]
fn a_machines_property_is_its_own() {
    let (ctx, mut state) = greenhouses(|project| {
        let machine = project.machine_types[0].id.clone();
        project.properties.as_mut().unwrap().push(PropertyDefinition {
            id: "charge".into(),
            name: "Charge".into(),
            scope: "object".into(),
            initial: 0,
            objects: Some(vec![machine]),
            rates: vec![PropertyRate { per_hour: 10 * MILLI, conditions: Vec::new() }],
            ..PropertyDefinition::default()
        });
    });
    let machine = ctx.content.machine_types[0].id.clone();
    let scene = state.player.scene_id.clone();
    let place = |state: &mut GameState, x: usize| {
        let scene = state.world.scenes.iter_mut().find(|s| s.id == state.player.scene_id).unwrap();
        scene.tiles[1][x].machine =
            Some(farm_sim::schema::TileMachine { type_id: machine.clone(), ..Default::default() });
    };
    place(&mut state, 1);
    hours(&ctx, &mut state, 2);
    place(&mut state, 3);
    hours(&ctx, &mut state, 1);
    // Each machine charges from when it was placed.
    assert_eq!(read(&ctx, &state, &format!("charge@{scene}:1,1")), 30);
    assert_eq!(read(&ctx, &state, &format!("charge@{scene}:3,1")), 10);
    // The id alone is the machine the player faces: an action reads and changes that one.
    assert_eq!(properties::read(&ctx, &state, "charge"), None, "facing no machine");
    state.player.x = units::tile_center(3);
    state.player.y = units::tile_center(2);
    state.player.direction = "up".into();
    assert_eq!(read(&ctx, &state, "charge"), 10);
    let top_up = outcome(serde_json::json!({ "type": "addVariable", "variable": "charge", "amount": 5 }));
    farm_sim::events::apply_outcomes(&ctx, &mut state, std::slice::from_ref(&top_up), 0);
    assert_eq!(read(&ctx, &state, &format!("charge@{scene}:3,1")), 15);
    assert_eq!(read(&ctx, &state, &format!("charge@{scene}:1,1")), 30);
    let drain = outcome(serde_json::json!({ "type": "addVariable", "variable": "charge", "amount": -5 }));
    farm_sim::events::apply_outcomes(&ctx, &mut state, std::slice::from_ref(&drain), 0);
    // A write to a machine that is not there changes nothing, and is not a plain variable.
    let nowhere =
        outcome(serde_json::json!({ "type": "setVariable", "variable": format!("charge@{scene}:5,1"), "amount": 9 }));
    farm_sim::events::apply_outcomes(&ctx, &mut state, std::slice::from_ref(&nowhere), 0);
    assert!(!state.flags.contains_key(&format!("charge@{scene}:5,1")));
    // A tile without a machine has none, and a machine that is taken away takes its value.
    assert_eq!(properties::read(&ctx, &state, &format!("charge@{scene}:5,1")), None);
    let index = state.world.scenes.iter().position(|s| s.id == scene).unwrap();
    state.world.scenes[index].tiles[1][1].machine = None;
    hours(&ctx, &mut state, 1);
    assert!(!state.flags.contains_key(&format!("charge@{scene}:1,1")));
    assert_eq!(read(&ctx, &state, &format!("charge@{scene}:3,1")), 20);
}

/// A machine's rate can read the scene it stands in: a tank charges only while its greenhouse
/// holds pressure.
#[test]
fn a_machines_rate_reads_its_own_scene() {
    let (ctx, mut state) = greenhouses(|project| {
        project.properties.as_mut().unwrap()[0].rates.clear();
        project.properties.as_mut().unwrap().push(PropertyDefinition {
            id: "charge".into(),
            name: "Charge".into(),
            scope: "object".into(),
            initial: 0,
            rates: vec![PropertyRate {
                per_hour: 10 * MILLI,
                conditions: vec![EventCondition::Not { conditions: vec![variable("pressure", "lt", 50)] }],
            }],
            ..PropertyDefinition::default()
        });
    });
    let machine = ctx.content.machine_types[0].id.clone();
    for scene in ["greenhouse-a", "greenhouse-b"] {
        let scene = state.world.scenes.iter_mut().find(|s| s.id == scene).unwrap();
        scene.tiles[1][1].machine =
            Some(farm_sim::schema::TileMachine { type_id: machine.clone(), ..Default::default() });
    }
    // The player is in neither greenhouse: each tank reads its own room (90 kPa and 40 kPa).
    hours(&ctx, &mut state, 3);
    assert_eq!(read(&ctx, &state, "charge@greenhouse-a:1,1"), 30);
    assert_eq!(read(&ctx, &state, "charge@greenhouse-b:1,1"), 0);
    // The HUD's reader: the player's value, the scene's where the player is, none elsewhere.
    let pressure = &ctx.content.properties[0];
    assert_eq!(properties::value_in_view(&state, pressure), None);
    enter(&mut state, "greenhouse-b");
    assert_eq!(properties::value_in_view(&state, pressure), Some(40 * MILLI));
    assert!(properties::value_in_view(&state, &ctx.content.properties[1]).is_some(), "the player's oxygen");
    assert_eq!(properties::value_in_view(&state, &ctx.content.properties[2]), None);
}

#[test]
fn a_game_without_properties_is_untouched() {
    let project = starter_farm_project();
    let ctx = EngineContext::new(state::create_content_from_project(&project));
    let mut state = state::create_game_state(&project, Some("plain"));
    advance_tick(&ctx, &mut state, 200);
    assert!(state.flags.keys().all(|name| !name.contains('@')));
    // A plain variable still is one.
    let add = outcome(serde_json::json!({ "type": "addVariable", "variable": "coins", "amount": 3 }));
    farm_sim::events::apply_outcomes(&ctx, &mut state, std::slice::from_ref(&add), 0);
    assert_eq!(state.flags.get("coins"), Some(&serde_json::json!(3)));
    assert_eq!(properties::read(&ctx, &state, "coins"), None);
}

#[test]
fn property_definitions_read_and_write_as_project_json() {
    let json = serde_json::json!({
        "id": "pressure", "name": "Pressure", "scope": "scene", "unit": "kPa", "min": 0, "max": 100, "initial": 90.5,
        "scenes": ["greenhouse-a"],
        "rates": [{ "perHour": -6, "conditions": [{ "type": "flag", "flag": "leak", "value": true }] }],
        "thresholds": [{ "compare": "lt", "value": 50, "message": "Low!", "outcomes": [] }],
        "hud": "meter", "color": "#44aaff"
    });
    let definition: PropertyDefinition = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(definition.initial, 90_500);
    assert_eq!(serde_json::to_value(&definition).unwrap(), json);
    // The defaults: a player's 0 to 100, starting full.
    let default: PropertyDefinition = serde_json::from_value(serde_json::json!({ "id": "o2" })).unwrap();
    assert_eq!((default.scope.as_str(), default.min, default.max, default.initial), ("player", 0, 100_000, 100_000));
}
