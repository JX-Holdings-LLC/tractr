//! Calendar, daily routines and shops must agree across save/resume and boundaries.
use farm_sim::{commerce, economy, events, game_time, schema::*, state, substitution, units, EngineContext};
use serde_json::json;

fn engine(mutator: impl FnOnce(&mut GameProject)) -> (EngineContext, GameState) {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/golden/content/starter-farm.json")).unwrap();
    let mut project: GameProject = serde_json::from_value(fixture["project"].clone()).unwrap();
    mutator(&mut project);
    (
        EngineContext::new(state::create_content_from_project(&project)),
        state::create_game_state(&project, Some("calendar-depth")),
    )
}

fn week() -> Vec<CalendarWeekday> {
    serde_json::from_value(json!([
        {"id":"sun", "name":"Sun", "words":{"es":"Sol"}},
        {"id":"moon", "name":"Moon"},
        {"id":"star", "name":"Star"}
    ]))
    .unwrap()
}

#[test]
fn weekday_crosses_seasons_years_and_overflow_safely() {
    let calendar = CalendarConfig { weekdays: Some(week()), weekday_offset: Some(2), ..Default::default() };
    assert_eq!(game_time::weekday(&calendar, 1).unwrap().id, "star");
    assert_eq!(game_time::weekday(&calendar, 2).unwrap().id, "sun");
    assert_eq!(game_time::weekday_name(&calendar, 2, "es").as_deref(), Some("Sol"));
    assert_eq!(game_time::weekday_name(&calendar, 2, "de").as_deref(), Some("Sun"));
    assert_eq!(game_time::weekday(&calendar, u32::MAX).unwrap().id, "moon");
    assert!(game_time::weekday(&CalendarConfig::default(), 1).is_none());
    assert!(game_time::weekday(&CalendarConfig { weekdays: Some(vec![]), ..Default::default() }, 1).is_none());
}

#[test]
fn weekday_condition_and_text_use_the_same_date() {
    let (ctx, mut state) = engine(|project| {
        project.settings.calendar.weekdays = Some(week());
        project.settings.locale = "es".into();
    });
    state.clock.day = 4;
    let condition = EventCondition::Weekday { weekday_ids: vec!["sun".into(), "star".into()] };
    assert!(events::condition_met(&ctx, &state, &condition, None));
    assert_eq!(substitution::fill(&ctx.content, &state, "Today: {weekday}"), "Today: Sol");
    state.clock.day = 5;
    assert!(!events::condition_met(&ctx, &state, &condition, None));
}

#[test]
fn schedule_selection_is_saved_and_changes_on_weather_or_morning() {
    let (ctx, mut state) = engine(|project| {
        project.npcs = vec![serde_json::from_value(json!({
            "id":"keeper", "sceneId":"farm", "schedule":[{"minute":360,"sceneId":"farm","x":2,"y":2}],
            "schedules":[
                {"id":"rain", "name":"Rain", "conditions":[{"type":"weather","weatherIds":["rain"]}], "stops":[{"minute":360,"sceneId":"house","x":1,"y":1}]},
                {"id":"story", "name":"Story", "conditions":[{"type":"flag","flag":"story","value":true}], "stops":[]}
            ]
        })).unwrap()];
    });
    farm_sim::npcs::npc_movement::select_schedules(&ctx, &mut state);
    assert!(state.npcs["keeper"].schedule_selection.as_ref().unwrap().variant.is_none());
    state.flags.insert("story".into(), json!(true));
    farm_sim::npcs::npc_movement::select_schedules(&ctx, &mut state);
    assert!(
        state.npcs["keeper"].schedule_selection.as_ref().unwrap().variant.is_none(),
        "story changes wait until morning"
    );
    state.clock.weather_id = "rain".into();
    farm_sim::npcs::npc_movement::select_schedules(&ctx, &mut state);
    assert_eq!(state.npcs["keeper"].schedule_selection.as_ref().unwrap().variant.as_deref(), Some("rain"));
    let resumed: GameState = serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
    assert_eq!(resumed.npcs, state.npcs);
    state.clock.day += 1;
    state.clock.weather_id = "clear".into();
    farm_sim::npcs::npc_movement::select_schedules(&ctx, &mut state);
    assert_eq!(state.npcs["keeper"].schedule_selection.as_ref().unwrap().variant.as_deref(), Some("story"));
}

#[test]
fn shop_checks_hours_weekdays_and_keeper_on_every_transaction() {
    let (ctx, mut state) = engine(|project| {
        project.settings.calendar.weekdays = Some(week());
        project.shops = vec![serde_json::from_value(json!({"id":"shop","hours":{"open":540,"close":1020},"closedOn":{"weekdays":["moon"]},"keeperNpcId":"keeper"})).unwrap()];
    });
    state.clock.day = 1;
    state.clock.time_minutes = units::minutes(539);
    state.npcs.insert("keeper".into(), NpcState { scene_id: state.player.scene_id.clone(), ..Default::default() });
    assert!(!commerce::shop_is_open(&ctx, &state, &ctx.content.shops[0]));
    state.clock.time_minutes = units::minutes(540);
    assert!(commerce::shop_is_open(&ctx, &state, &ctx.content.shops[0]));
    assert!(economy::handle_open_shop(&ctx, &mut state, "shop").is_empty());
    state.clock.time_minutes = units::minutes(1020);
    assert!(!economy::handle_buy_item(&ctx, &mut state, "anything", 1).is_empty());
    state.clock.time_minutes = units::minutes(600);
    state.clock.day = 2;
    assert!(!commerce::shop_is_open(&ctx, &state, &ctx.content.shops[0]));
    state.clock.day = 3;
    state.npcs.get_mut("keeper").unwrap().scene_id = "away".into();
    assert!(!commerce::shop_is_open(&ctx, &state, &ctx.content.shops[0]));
}

#[test]
fn prices_stack_and_saturation_expires_after_save_resume() {
    let (ctx, mut state) = engine(|project| {
        project.settings.calendar.weekdays = Some(week());
        project.shops = vec![serde_json::from_value(json!({"id":"shop", "priceModifiers":[
            {"weekdays":["sun"],"buyMultiplier":0.8,"sellMultiplier":1.5},
            {"conditions":[{"type":"flag","flag":"sale","value":true}],"buyMultiplier":0.5}
        ], "saturation":{"quantityPerStep":10,"reductionPerStep":0.1,"minimumMultiplier":0.25,"recoveryDays":3}}))
        .unwrap()];
    });
    state.clock.day = 1;
    state.flags.insert("sale".into(), json!(true));
    let shop = &ctx.content.shops[0];
    assert_eq!(commerce::price(&ctx, &state, shop, "crop", 100, false), 40);
    assert_eq!(commerce::price(&ctx, &state, shop, "crop", 100, true), 150);
    commerce::record_sale(&mut state, shop, "crop", 20);
    assert_eq!(commerce::price(&ctx, &state, shop, "crop", 100, true), 120);
    state = serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
    assert_eq!(commerce::price(&ctx, &state, shop, "crop", 100, true), 120);
    state.clock.day = 4;
    commerce::expire_sales(&ctx, &mut state);
    assert_eq!(commerce::price(&ctx, &state, shop, "crop", 100, true), 150);
    assert!(state.shop_sales.is_empty());
}

fn festival_engine() -> (EngineContext, GameState) {
    engine(|project| {
        let mut grounds = project.scenes[0].clone();
        grounds.id = "festival-grounds".into();
        grounds.npcs.clear();
        project.scenes.push(grounds);
        project.settings.calendar.festivals = vec![serde_json::from_value(json!({
            "id":"fair","name":"Fair","seasonId":"spring","day":1,
            "program":{"sceneId":"festival-grounds","entranceSceneId":"scene-farm","entranceX":2,"entranceY":2,
                "arrivalX":4,"arrivalY":6,"openMinute":540,"closeMinute":1020,
                "attendance":[{"npcId":"npc-merchant","x":3,"y":3}],
                "activity":{"x":4,"y":3,"contest":{"itemId":"prize-crop","consumeItem":true,"rewards":[{"type":"giveMoney","amount":100}]}},
                "onMiss":[{"type":"setFlag","flagName":"missed-fair"}]}
        })).unwrap()];
    })
}

#[test]
fn festival_visit_restores_guests_and_contest_cannot_reward_twice() {
    let (ctx, mut state) = festival_engine();
    let original = state.npcs["npc-merchant"].clone();
    state.clock.time_minutes = units::minutes(600);
    assert!(farm_sim::festivals::interact(&ctx, &mut state, 2, 2).is_some());
    assert_eq!(state.player.scene_id, "festival-grounds");
    assert_eq!(state.npcs["npc-merchant"].scene_id, "festival-grounds");
    assert!(farm_sim::festivals::attending(&state, "npc-merchant"));
    let money = state.player.money;
    assert!(!farm_sim::festivals::interact(&ctx, &mut state, 4, 3).unwrap().is_empty());
    assert_eq!(state.player.money, money, "no qualifying item, no prize");
    state.player.inventory.push(InventorySlot {
        item: Item { id: "prize-crop".into(), ..Default::default() },
        quantity: 2,
        ..Default::default()
    });
    farm_sim::festivals::interact(&ctx, &mut state, 4, 3);
    assert_eq!(state.player.money, money + 100);
    assert_eq!(inventory_count(&state, "prize-crop"), 1);
    state = serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
    farm_sim::festivals::interact(&ctx, &mut state, 4, 3);
    assert_eq!(state.player.money, money + 100);
    farm_sim::festivals::leave(&ctx, &mut state, true);
    assert_eq!(state.player.scene_id, "scene-farm");
    assert_eq!(state.npcs["npc-merchant"], original);
    assert_eq!(state.clock.time_minutes, units::minutes(1020));
    assert!(state.festival.is_none());
}

fn inventory_count(state: &GameState, id: &str) -> u32 {
    state.player.inventory.iter().filter(|slot| slot.item.id == id).map(|slot| slot.quantity).sum()
}

#[test]
fn festival_hours_missed_outcomes_and_keep_changes() {
    let (ctx, mut state) = festival_engine();
    state.clock.time_minutes = units::minutes(500);
    assert!(farm_sim::festivals::interact(&ctx, &mut state, 2, 2).is_some());
    assert!(state.festival.is_none());
    farm_sim::festivals::finish_day(&ctx, &mut state);
    assert_eq!(state.flags.get("missed-fair"), Some(&json!(true)));
    state.clock.time_minutes = units::minutes(600);
    farm_sim::festivals::interact(&ctx, &mut state, 2, 2);
    let kept = KeptState::of_state(&state).unwrap();
    let mut reconstructed = state.clone();
    reconstructed.festival = None;
    reconstructed.festival_visits.clear();
    kept.apply_to(&mut reconstructed);
    assert_eq!(reconstructed.festival, state.festival);
    assert_eq!(reconstructed.festival_visits, state.festival_visits);
    state.clock.time_minutes = units::minutes(1020);
    farm_sim::festivals::advance(&ctx, &mut state);
    assert!(state.festival.is_none());
}

#[test]
fn blocked_festival_arrival_does_not_move_guests_or_start_session() {
    let (ctx, mut state) = festival_engine();
    let guests = state.npcs.clone();
    state.clock.time_minutes = units::minutes(600);
    for scene in &mut state.world.scenes {
        if scene.id == "festival-grounds" {
            for row in &mut scene.tiles {
                for tile in row {
                    tile.collision = true;
                }
            }
        }
    }
    farm_sim::festivals::interact(&ctx, &mut state, 2, 2);
    assert!(state.festival.is_none());
    assert_eq!(state.npcs, guests);
    assert_eq!(state.player.scene_id, "scene-farm");
}

#[test]
fn event_warp_out_of_festival_keeps_its_destination() {
    let (ctx, mut state) = festival_engine();
    let original = state.npcs["npc-merchant"].clone();
    state.clock.time_minutes = units::minutes(600);
    farm_sim::festivals::interact(&ctx, &mut state, 2, 2);
    farm_sim::world::world_movement::put_player(&mut state, "scene-farm", units::tiles(8), units::tiles(4));
    farm_sim::festivals::advance(&ctx, &mut state);
    assert!(state.festival.is_none());
    assert_eq!(state.player.x, units::tiles(8));
    assert_eq!(state.player.y, units::tiles(4));
    assert_eq!(state.npcs["npc-merchant"], original);
}

#[test]
fn refused_festival_action_can_be_retried_after_meeting_its_conditions() {
    let (original, mut state) = festival_engine();
    let mut content = original.content.clone();
    content.actions.push(
        serde_json::from_value(json!({
            "id":"contest-entry", "conditions":[{"type":"flag","flag":"ready","value":true}],
            "outcomes":[{"type":"giveMoney","amount":10}]
        }))
        .unwrap(),
    );
    content.settings.calendar.festivals[0].program.as_mut().unwrap().activity =
        Some(FestivalActivity { x: 4, y: 3, action_id: Some("contest-entry".into()), ..Default::default() });
    let ctx = EngineContext::new(content);
    state.clock.time_minutes = units::minutes(600);
    farm_sim::festivals::interact(&ctx, &mut state, 2, 2);
    farm_sim::festivals::interact(&ctx, &mut state, 4, 3);
    assert!(!state.festival_visits["fair"].activity_done);
    let money = state.player.money;
    state.flags.insert("ready".into(), json!(true));
    farm_sim::festivals::interact(&ctx, &mut state, 4, 3);
    assert_eq!(state.player.money, money + 10);
    assert!(state.festival_visits["fair"].activity_done);
}
