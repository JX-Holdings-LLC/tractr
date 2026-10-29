mod common;

use farm_sim::{fishing, foraging, hash_state, schema::*, units};
use serde_json::json;

fn table(value: serde_json::Value) -> FishTable {
    serde_json::from_value(value).unwrap()
}

#[test]
fn fishing_filters_time_weather_season_and_conditions() {
    let (ctx, mut state) = common::make_engine(
        |project| {
            project.fish_tables = vec![table(json!({"id":"rain-night","junkChance":0,
            "filter":{"startMinute":1000,"endMinute":1200,"weatherIds":["rain"],"seasons":["spring"],"condition":{"type":"flag","flag":"fishing-open","value":true}},
            "entries":[{"itemId":"fish-carp","weight":1,"difficulty":0}]}))];
        },
        "filters",
    );
    assert!(fishing::active_fish_table(&ctx, &state).is_none());
    state.clock.time_minutes = units::minutes(1100);
    state.clock.weather_id = "rain".into();
    assert!(fishing::active_fish_table(&ctx, &state).is_none());
    state.flags.insert("fishing-open".into(), json!(true));
    assert!(fishing::active_fish_table(&ctx, &state).is_some());
    state.clock.time_minutes = units::minutes(1200);
    assert!(fishing::active_fish_table(&ctx, &state).is_some());
    state.clock.time_minutes = units::minutes(1201);
    assert!(fishing::active_fish_table(&ctx, &state).is_none());
    state.clock.time_minutes = units::minutes(1100);
    state.clock.season = "summer".into();
    assert!(fishing::active_fish_table(&ctx, &state).is_none());
}

#[test]
fn layered_fishing_merges_eligible_entries_while_legacy_keeps_first_table() {
    for layered in [false, true] {
        let (ctx, mut state) = common::make_engine(
            |p| {
                p.fish_tables = vec![
                    table(json!({"id":"first","junkChance":0,"layered":layered,
                "entries":[{"itemId":"fish-carp","weight":0,"difficulty":0}]})),
                    table(
                        json!({"id":"second","junkChance":0,"entries":[{"itemId":"fish-perch","weight":1,"difficulty":0}]}),
                    ),
                ];
            },
            "layered",
        );
        assert_eq!(fishing::resolve_fishing(&ctx, &mut state, 1, None).caught, layered);
        assert_eq!(state.player.inventory.iter().any(|s| s.item.id == "fish-perch"), layered);
    }
}

#[test]
fn catches_award_authored_xp_quality_size_and_consume_gear_then_survive_keep_changes() {
    let (ctx, mut state) = common::make_engine(
        |p| {
            p.fish_tables = vec![table(json!({"id":"sized","junkChance":0,"entries":[{
            "itemId":"fish-carp","weight":1,"difficulty":1,
            "rules":{"quality":true,"xp":31,"minSize":12,"maxSize":12}}]}))];
            let item: Item =
                serde_json::from_value(json!({"id":"tackle","name":"Tackle","stackable":true,"maxStack":10,
            "fishingGear":{"kind":"tackle","escapeReduction":100,"qualityBonus":1}}))
                .unwrap();
            p.player.inventory.push(InventorySlot::new(item, 2));
        },
        "quality",
    );
    assert!(fishing::resolve_fishing(&ctx, &mut state, 1, Some(units::PROBABILITY_ONE)).caught);
    let fish = state.player.inventory.iter().find(|s| s.item.id == "fish-carp").unwrap();
    assert!(fish.quality.is_some());
    assert_eq!(state.player.inventory.iter().find(|s| s.item.id == "tackle").unwrap().quantity, 1);
    assert_eq!(state.fish_catches[0].size, 12);
    assert_eq!(state.player.skills.get("fishing").unwrap().xp, 31);
    let mut restored = state.clone();
    restored.fish_catches.clear();
    KeptState::of_state(&state).unwrap().apply_to(&mut restored);
    assert_eq!(restored.fish_catches, state.fish_catches);
    let saved = serde_json::to_value(&state).unwrap();
    assert_eq!(serde_json::from_value::<GameState>(saved).unwrap().fish_catches, state.fish_catches);
}

#[test]
fn traps_and_ponds_fill_without_overwriting_uncollected_output() {
    let (ctx, mut state) = common::make_engine(
        |p| {
            p.fish_tables =
                vec![table(json!({"id":"trap-table","junkChance":0,"entries":[{"itemId":"fish-perch","weight":1}]}))];
            p.machine_types = vec![
                serde_json::from_value(
                    json!({"id":"trap","fishProduction":{"tableId":"trap-table","everyDays":2,"quantity":1}}),
                )
                .unwrap(),
                serde_json::from_value(
                    json!({"id":"pond","fishProduction":{"speciesItemId":"fish-carp","everyDays":1,"quantity":2}}),
                )
                .unwrap(),
            ];
            p.scenes[0].tiles[0][0].machine = Some(TileMachine { type_id: "trap".into(), ..Default::default() });
            p.scenes[0].tiles[0][1].machine = Some(TileMachine { type_id: "pond".into(), ..Default::default() });
        },
        "production",
    );
    state.clock.day = 2;
    fishing::produce_overnight(&ctx, &mut state);
    assert_eq!(
        state.world.scenes[0].tiles[0][0].machine.as_ref().unwrap().output.as_ref().unwrap()[0].item_id,
        "fish-perch"
    );
    assert_eq!(state.world.scenes[0].tiles[0][1].machine.as_ref().unwrap().output.as_ref().unwrap()[0].quantity, 2);
    let before = hash_state(&state);
    fishing::produce_overnight(&ctx, &mut state);
    assert_eq!(before, hash_state(&state));
}

fn forage_project(p: &mut GameProject) {
    p.node_types =
        vec![serde_json::from_value(json!({"id":"forage","health":1,"requiredTool":"scythe","seasons":["spring"],
        "drops":[{"itemId":"fish-carp","min":1,"max":1,"weight":1}]}))
        .unwrap()];
    p.scenes[0].forage_spawns =
        Some(vec![serde_json::from_value(json!({"id":"wild","maximum":40,"terrains":["grass"],"lifetimeDays":2,
        "entries":[{"nodeTypeId":"forage","weight":1}]}))
        .unwrap()]);
    p.building_types = Some(vec![BuildingDefinition { id: "barn".into(), width: 2, height: 2, ..Default::default() }]);
    p.buildings = Some(vec![BuildingState {
        id: "barn-1".into(),
        building_id: "barn".into(),
        scene_id: p.scenes[0].id.clone(),
        x: 3,
        y: 0,
        ..Default::default()
    }]);
    p.scenes[0].tiles[0][0].overlay = Some("path".into());
    p.scenes[0].transitions.push(SceneTransition { from_x: 1, from_y: 0, ..Default::default() });
}

#[test]
fn daily_forage_is_deterministic_and_avoids_doors_paths_obstacles_and_actors() {
    let (ctx, mut first) = common::make_engine(forage_project, "forage");
    let mut second = first.clone();
    foraging::morning(&ctx, &mut first);
    foraging::morning(&ctx, &mut second);
    assert_eq!(hash_state(&first), hash_state(&second));
    assert!(first.world.scenes[0].tiles.iter().flatten().any(|t| t.node.is_some()));
    assert!(first.world.scenes[0].tiles[0][0].node.is_none());
    assert!(first.world.scenes[0].tiles[0][1].node.is_none());
    assert!(first.world.scenes[0].tiles[4][4].node.is_none());
    assert!(first.world.scenes[0].tiles[0][3].node.is_none());
    assert!(first.world.scenes[0].tiles[1][4].node.is_none());
    assert!(first.world.scenes[0]
        .tile(units::tile_of(first.player.x), units::tile_of(first.player.y))
        .unwrap()
        .node
        .is_none());
    first.clock.season = "summer".into();
    foraging::morning(&ctx, &mut first);
    assert!(first.world.scenes[0].tiles.iter().flatten().all(|t| t.node.is_none()));
}

#[test]
fn authored_seasonal_nodes_sleep_out_of_season_and_reappear_with_their_health() {
    let (ctx, mut state) = common::make_engine(
        |p| {
            forage_project(p);
            p.scenes[0].forage_spawns = None;
            p.scenes[0].tiles[0][2].node =
                Some(TileNode { type_id: "forage".into(), remaining_health: 1, ..Default::default() });
        },
        "seasons",
    );
    state.clock.season = "summer".into();
    foraging::morning(&ctx, &mut state);
    let node = state.world.scenes[0].tiles[0][2].node.as_ref().unwrap();
    assert_eq!(node.remaining_health, 0);
    assert_eq!(node.dormant_health, Some(1));
    state.clock.season = "spring".into();
    foraging::morning(&ctx, &mut state);
    assert_eq!(state.world.scenes[0].tiles[0][2].node.as_ref().unwrap().remaining_health, 1);
}

#[test]
fn legacy_mornings_do_not_draw_randomness_or_add_serialized_fields() {
    let (ctx, mut state) = common::make_engine(|_| {}, "legacy");
    let before = hash_state(&state);
    foraging::morning(&ctx, &mut state);
    fishing::produce_overnight(&ctx, &mut state);
    assert_eq!(before, hash_state(&state));
    assert!(serde_json::to_value(&state).unwrap().get("fishCatches").is_none());
}

#[test]
fn an_entry_filter_can_remove_a_fish_without_consuming_a_roll() {
    let (ctx, mut state) = common::make_engine(
        |p| {
            p.fish_tables = vec![table(json!({"id":"filtered","junkChance":0,"entries":[{
            "itemId":"fish-carp","weight":1,"rules":{"filter":{"weatherIds":["snow"]}}}]}))];
        },
        "entry-filter",
    );
    state.clock.weather_id = "sun".into();
    let before = state.rng.clone();
    assert!(!fishing::resolve_fishing(&ctx, &mut state, 1, None).caught);
    assert_eq!(before, state.rng);
}

#[test]
fn forage_expires_after_its_lifetime_even_with_spawning_disabled() {
    let (ctx, mut state) = common::make_engine(forage_project, "expiry");
    foraging::morning(&ctx, &mut state);
    assert!(state.world.scenes[0].tiles.iter().flatten().any(|t| t.node.is_some()));
    state.world.scenes[0].forage_spawns = None;
    state.clock.day = state.clock.day.saturating_add(2);
    foraging::morning(&ctx, &mut state);
    assert!(state.world.scenes[0].tiles.iter().flatten().all(|t| t.node.is_none()));
}

#[test]
fn node_drop_seasons_choose_the_current_seasons_items() {
    let (ctx, mut state) = common::make_engine(
        |p| {
            p.node_types =
                vec![serde_json::from_value(json!({"id":"seasonal-drops","health":1,"requiredTool":"scythe",
            "drops":[{"itemId":"fish-carp","min":1,"max":1,"weight":1,"seasons":["spring"]},
                     {"itemId":"fish-perch","min":1,"max":1,"weight":1,"seasons":["summer"]}]}))
                .unwrap()];
            p.scenes[0].tiles[0][2].node =
                Some(TileNode { type_id: "seasonal-drops".into(), remaining_health: 1, ..Default::default() });
        },
        "drops",
    );
    state.clock.season = "summer".into();
    let scene_id = state.world.scenes[0].id.clone();
    assert!(farm_sim::gathering::strike_node(&ctx, &mut state, &scene_id, 2, 0, "scythe", 1, 1).struck);
    assert!(state.player.inventory.iter().any(|slot| slot.item.id == "fish-perch"));
    assert!(!state.player.inventory.iter().any(|slot| slot.item.id == "fish-carp"));
}

#[test]
fn catch_training_matches_the_caught_species_and_skill_debuff_lowers_quality() {
    let (ctx, mut state) = common::make_engine(
        |p| {
            p.fish_tables = vec![table(json!({"id":"subject","junkChance":0,"entries":[{
            "itemId":"fish-carp","weight":1,"difficulty":0,"rules":{"quality":true}}]}))];
            p.settings.skill_definitions = Some(vec![
                serde_json::from_value(
                    json!({"id":"carp-angler","training":[{"action":"catch","subject":"fish-carp","experience":7}]}),
                )
                .unwrap(),
                serde_json::from_value(
                    json!({"id":"perch-angler","training":[{"action":"catch","subject":"fish-perch","experience":9}]}),
                )
                .unwrap(),
            ]);
        },
        "subject",
    );
    state.player.skills.insert("fishing".into(), SkillState { xp: 0, level: 9 });
    assert!(fishing::resolve_fishing(&ctx, &mut state, 1, None).caught);
    assert_eq!(state.player.skills.get("carp-angler").unwrap().xp, 7);
    assert!(!state.player.skills.contains_key("perch-angler"));
    assert_eq!(
        state.player.inventory.iter().find(|s| s.item.id == "fish-carp").unwrap().quality.as_deref(),
        Some("iridium")
    );
    farm_sim::buffs::apply(
        &ctx,
        &mut state,
        &BuffDefinition {
            id: "fatigue".into(),
            stat: "skillLevel".into(),
            amount: -9,
            duration_minutes: 20,
            ..Default::default()
        },
        "test",
    );
    assert!(fishing::resolve_fishing(&ctx, &mut state, 1, None).caught);
    assert!(state.player.inventory.iter().any(|s| s.item.id == "fish-carp" && s.quality.is_none()));
}
