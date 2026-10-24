//! M3: event logic for creators who don't write code — variables, condition groups, chance, the
//! new triggers (day start, a time of day, harvest, sell, craft, catch, pick up, friendship, a
//! quest) and the new outcomes (move an NPC, weather, experience, shop stock, nodes and
//! machines).

mod common;

use common::{flag, make_engine};
use farm_sim::schema::{
    event_fired_flag, event_triggers, EventCondition, EventOutcome, GameEvent, GameProject, GameState, OutcomeKind,
    ShopSession, WeatherTypeDefinition,
};
use farm_sim::{content_builtin, economy, engine, events, units, Command, EngineContext};
use serde_json::json;

fn outcome(kind: OutcomeKind, edit: impl FnOnce(&mut EventOutcome)) -> EventOutcome {
    let mut outcome = EventOutcome::of(kind);
    edit(&mut outcome);
    outcome
}

fn event(id: &str, trigger: &str, conditions: Vec<EventCondition>, outcomes: Vec<EventOutcome>) -> GameEvent {
    GameEvent {
        id: id.to_owned(),
        name: id.to_owned(),
        trigger: trigger.to_owned(),
        conditions,
        outcomes,
        active: true,
        ..GameEvent::default()
    }
}

fn set_flag(name: &str) -> EventOutcome {
    outcome(OutcomeKind::SetFlag, |o| o.flag_name = Some(name.to_owned()))
}

fn variable(name: &str, compare: &str, value: Option<f64>, text: Option<&str>) -> EventCondition {
    EventCondition::Variable {
        name: name.to_owned(),
        compare: compare.to_owned(),
        value: value.map(units::amount),
        text: text.map(str::to_owned),
    }
}

fn holds(ctx: &EngineContext, state: &GameState, condition: &EventCondition) -> bool {
    events::condition_met(ctx, state, condition, None)
}

fn run(ctx: &EngineContext, state: &mut GameState, outcomes: &[EventOutcome]) {
    events::apply_outcomes(ctx, state, outcomes, 0);
}

fn with_events(events: Vec<GameEvent>) -> impl FnOnce(&mut GameProject) {
    move |project| project.events = events
}

#[test]
fn variables_hold_numbers_and_text_and_compare() {
    let (ctx, mut state) = make_engine(|_| {}, "vars");
    run(
        &ctx,
        &mut state,
        &[
            outcome(OutcomeKind::SetVariable, |o| {
                o.variable = Some("coins".into());
                o.amount = Some(units::amount(3.0));
            }),
            outcome(OutcomeKind::AddVariable, |o| {
                o.variable = Some("coins".into());
                o.amount = Some(units::amount(2.5));
            }),
            outcome(OutcomeKind::AddVariable, |o| {
                o.variable = Some("visits".into());
                o.amount = Some(units::amount(1.0));
            }),
            outcome(OutcomeKind::SetVariable, |o| {
                o.variable = Some("mood".into());
                o.text = Some("sunny".into());
            }),
        ],
    );
    assert_eq!(flag(&state, "coins"), Some(&json!(5.5)));
    // A whole number is stored as an integer, so it survives a save as it was.
    assert_eq!(flag(&state, "visits"), Some(&json!(1)));
    assert_eq!(flag(&state, "mood"), Some(&json!("sunny")));

    assert!(holds(&ctx, &state, &variable("coins", "gt", Some(5.0), None)));
    assert!(holds(&ctx, &state, &variable("coins", "le", Some(5.5), None)));
    assert!(!holds(&ctx, &state, &variable("coins", "eq", Some(5.0), None)));
    assert!(holds(&ctx, &state, &variable("visits", "eq", Some(1.0), None)));
    // Unset is 0 and "".
    assert!(holds(&ctx, &state, &variable("never", "eq", Some(0.0), None)));
    assert!(holds(&ctx, &state, &variable("never", "eq", None, Some(""))));
    assert!(holds(&ctx, &state, &variable("mood", "eq", None, Some("sunny"))));
    assert!(holds(&ctx, &state, &variable("mood", "ne", None, Some("rainy"))));
    // A true flag counts as 1; an unknown comparison never holds.
    state.flags.insert("door".into(), json!(true));
    assert!(holds(&ctx, &state, &variable("door", "ge", Some(1.0), None)));
    assert!(!holds(&ctx, &state, &variable("door", "about", Some(1.0), None)));
}

#[test]
fn groups_combine_conditions() {
    let (ctx, mut state) = make_engine(|_| {}, "groups");
    state.flags.insert("a".into(), json!(true));
    let a = EventCondition::Flag { flag: "a".into(), value: true };
    let b = EventCondition::Flag { flag: "b".into(), value: true };
    assert!(!holds(&ctx, &state, &EventCondition::All { conditions: vec![a.clone(), b.clone()] }));
    assert!(holds(&ctx, &state, &EventCondition::Any { conditions: vec![a.clone(), b.clone()] }));
    assert!(holds(&ctx, &state, &EventCondition::Not { conditions: vec![b.clone()] }));
    assert!(!holds(&ctx, &state, &EventCondition::Not { conditions: vec![a.clone(), b.clone()] }));
    // Groups nest; empty groups: all holds, any and not-any as expected.
    let nested = EventCondition::All {
        conditions: vec![
            a.clone(),
            EventCondition::Not { conditions: vec![EventCondition::Any { conditions: vec![b] }] },
        ],
    };
    assert!(holds(&ctx, &state, &nested));
    assert!(holds(&ctx, &state, &EventCondition::All { conditions: vec![] }));
    assert!(!holds(&ctx, &state, &EventCondition::Any { conditions: vec![] }));
    assert!(holds(&ctx, &state, &EventCondition::Not { conditions: vec![] }));
}

#[test]
fn chance_rolls_without_touching_the_random_stream() {
    let (ctx, mut state) = make_engine(|_| {}, "chance");
    let rng = state.rng.clone();
    let never = EventCondition::Chance { chance: 0 };
    let always = EventCondition::Chance { chance: units::PROBABILITY_ONE };
    let half = EventCondition::Chance { chance: units::chance(0.5) };
    assert!(!holds(&ctx, &state, &never));
    assert!(holds(&ctx, &state, &always));
    // The same tick rolls the same; over many ticks about half hold.
    assert_eq!(holds(&ctx, &state, &half), holds(&ctx, &state, &half));
    let mut held = 0;
    for tick in 0..400 {
        state.clock.tick = tick;
        held += u32::from(holds(&ctx, &state, &half));
    }
    assert!((140..=260).contains(&held), "{held} of 400");
    assert_eq!(state.rng, rng);
}

#[test]
fn scene_and_day_of_season_conditions() {
    let (ctx, mut state) = make_engine(|_| {}, "where");
    assert!(holds(&ctx, &state, &EventCondition::Scene { scene_ids: vec!["scene-test".into()] }));
    assert!(!holds(&ctx, &state, &EventCondition::Scene { scene_ids: vec!["scene-town".into()] }));
    state.clock.day = 5;
    state.clock.day_of_season = 5;
    let within = EventCondition::DayOfSeason { min_day: Some(3), max_day: Some(7) };
    let after = EventCondition::DayOfSeason { min_day: Some(6), max_day: None };
    assert!(holds(&ctx, &state, &within));
    assert!(!holds(&ctx, &state, &after));
}

#[test]
fn day_start_events_run_each_morning() {
    let mut morning = event(
        "morning",
        event_triggers::DAY_START,
        vec![],
        vec![outcome(OutcomeKind::AddVariable, |o| {
            o.variable = Some("mornings".into());
            o.amount = Some(units::amount(1.0));
        })],
    );
    morning.repeatable = true;
    let (ctx, mut state) = make_engine(with_events(vec![morning]), "morning");
    engine::apply_command(&ctx, &mut state, &Command::Sleep);
    engine::apply_command(&ctx, &mut state, &Command::Sleep);
    assert_eq!(flag(&state, "mornings"), Some(&json!(2)));
}

#[test]
fn time_events_fire_when_the_clock_reaches_their_minute() {
    let mut noon = event("seven", event_triggers::TIME, vec![], vec![set_flag("seven")]);
    noon.minute = Some(units::minutes(7 * 60));
    let (ctx, mut state) = make_engine(with_events(vec![noon]), "time");
    // 6:00 → 6:59: not yet.
    let ticks_per_minute = u64::from(units::MINUTE / ctx.content.settings.time.minutes_per_real_second);
    engine::advance_tick(&ctx, &mut state, ticks_per_minute * 59);
    assert_eq!(flag(&state, "seven"), None);
    engine::advance_tick(&ctx, &mut state, ticks_per_minute * 2);
    assert_eq!(flag(&state, "seven"), Some(&json!(true)));
    assert_eq!(flag(&state, &event_fired_flag("seven")), Some(&json!(true)));
}

#[test]
fn subjects_pick_what_a_trigger_is_about() {
    let mut sold_seeds = event("sold-seeds", event_triggers::SELL, vec![], vec![set_flag("sold-seeds")]);
    sold_seeds.subject = Some("seed-wheat".into());
    let mut sold_other = event("sold-other", event_triggers::SELL, vec![], vec![set_flag("sold-other")]);
    sold_other.subject = Some("crop-tomato".into());
    let sold_any = event("sold-any", event_triggers::SELL, vec![], vec![set_flag("sold-any")]);
    let (ctx, mut state) = make_engine(
        |project| {
            project.events = vec![sold_seeds, sold_other, sold_any];
            project.shops = vec![content_builtin::create_default_shop()];
        },
        "sell",
    );
    state.shop = Some(ShopSession { shop_id: "shop-general".into() });
    engine::apply_command(
        &ctx,
        &mut state,
        &Command::SellItem { item_id: "seed-wheat".into(), quantity: 1, quality: None },
    );
    assert_eq!(flag(&state, "sold-seeds"), Some(&json!(true)));
    assert_eq!(flag(&state, "sold-any"), Some(&json!(true)));
    assert_eq!(flag(&state, "sold-other"), None);
}

#[test]
fn quest_and_friendship_triggers() {
    let mut started = event("started", event_triggers::QUEST, vec![], vec![set_flag("quest-moved")]);
    started.subject = Some("quest-extra".into());
    let mut friend = event(
        "friend",
        event_triggers::FRIENDSHIP,
        vec![EventCondition::Hearts { npc_id: "npc-test".into(), min: Some(1), max: None }],
        vec![set_flag("one-heart")],
    );
    friend.subject = Some("npc-test".into());
    let (ctx, mut state) = make_engine(
        |project| {
            project.events = vec![started, friend];
            let mut quest = project.quests[0].clone();
            quest.id = "quest-extra".into();
            quest.status = "available".into();
            project.quests.push(quest);
        },
        "triggers",
    );
    run(&ctx, &mut state, &[outcome(OutcomeKind::StartQuest, |o| o.quest_id = Some("quest-extra".into()))]);
    assert_eq!(flag(&state, "quest-moved"), Some(&json!(true)));

    let befriend = |amount: f64| {
        outcome(OutcomeKind::ModifyFriendship, |o| {
            o.npc_id = Some("npc-test".into());
            o.amount = Some(units::amount(amount));
        })
    };
    run(&ctx, &mut state, &[befriend(50.0)]);
    assert_eq!(flag(&state, "one-heart"), None);
    run(&ctx, &mut state, &[befriend(100.0)]);
    assert_eq!(flag(&state, "one-heart"), Some(&json!(true)));
}

#[test]
fn an_event_that_sets_off_its_own_trigger_stops() {
    let mut echo = event(
        "echo",
        event_triggers::FRIENDSHIP,
        vec![],
        vec![
            outcome(OutcomeKind::ModifyFriendship, |o| {
                o.npc_id = Some("npc-test".into());
                o.amount = Some(units::amount(1.0));
            }),
            outcome(OutcomeKind::AddVariable, |o| {
                o.variable = Some("echoes".into());
                o.amount = Some(units::amount(1.0));
            }),
        ],
    );
    echo.repeatable = true;
    let (ctx, mut state) = make_engine(with_events(vec![echo]), "echo");
    run(
        &ctx,
        &mut state,
        &[outcome(OutcomeKind::ModifyFriendship, |o| {
            o.npc_id = Some("npc-test".into());
            o.amount = Some(units::amount(1.0));
        })],
    );
    assert_eq!(flag(&state, "echoes"), Some(&json!(events::MAX_TRIGGER_DEPTH)));
}

#[test]
fn outcomes_move_npcs_change_weather_and_give_experience() {
    let (ctx, mut state) = make_engine(
        |project| {
            project.weather.types.push(WeatherTypeDefinition {
                id: "rain".into(),
                name: "Rain".into(),
                ..WeatherTypeDefinition::default()
            })
        },
        "outcomes",
    );
    run(
        &ctx,
        &mut state,
        &[
            outcome(OutcomeKind::MoveNpc, |o| {
                o.npc_id = Some("npc-test".into());
                o.x = Some(4);
                o.y = Some(1);
            }),
            outcome(OutcomeKind::SetWeather, |o| o.weather_id = Some("rain".into())),
            outcome(OutcomeKind::SetWeather, |o| o.weather_id = Some("hail".into())),
            outcome(OutcomeKind::GiveExperience, |o| {
                o.skill = Some("fishing".into());
                o.amount = Some(units::amount(60.0));
            }),
        ],
    );
    let npc = &state.npcs["npc-test"];
    assert_eq!((npc.x, npc.y, npc.scene_id.as_str()), (units::tiles(4), units::tiles(1), "scene-test"));
    assert_eq!(state.clock.weather_id, "rain");
    assert_eq!(state.player.skills["fishing"].xp, 60);
    assert_eq!(state.player.skills["fishing"].level, 1);
}

#[test]
fn shop_stock_changes_are_kept_in_the_game() {
    let (ctx, mut state) = make_engine(|project| project.shops = vec![content_builtin::create_default_shop()], "stock");
    let sells = |state: &GameState, item: &str| {
        economy::find_shop(&ctx, state, "shop-general").unwrap().stock.iter().any(|entry| entry.item_id == item)
    };
    assert!(sells(&state, "seed-wheat"));
    assert!(!sells(&state, "material-wood"));
    run(
        &ctx,
        &mut state,
        &[
            outcome(OutcomeKind::AddShopStock, |o| {
                o.shop_id = Some("shop-general".into());
                o.item_id = Some("material-wood".into());
                o.amount = Some(units::amount(7.0));
                o.item_quantity = Some(3);
            }),
            outcome(OutcomeKind::RemoveShopStock, |o| {
                o.shop_id = Some("shop-general".into());
                o.item_id = Some("seed-wheat".into());
            }),
        ],
    );
    assert!(!sells(&state, "seed-wheat"));
    let shop = economy::find_shop(&ctx, &state, "shop-general").unwrap();
    let wood = shop.stock.iter().find(|entry| entry.item_id == "material-wood").unwrap();
    assert_eq!((wood.price, wood.daily_limit), (Some(7), Some(3)));
    // Buying uses the changed stock.
    state.shop = Some(ShopSession { shop_id: "shop-general".into() });
    engine::apply_command(&ctx, &mut state, &Command::BuyItem { item_id: "material-wood".into(), quantity: 1 });
    assert!(state.player.inventory.iter().any(|slot| slot.item.id == "material-wood"));
    assert_eq!(state.player.money, 93);
}

#[test]
fn nodes_and_machines_are_placed_and_removed() {
    let (ctx, mut state) =
        make_engine(|project| project.machine_types = content_builtin::create_default_machine_types(), "tiles");
    let at = |o: &mut EventOutcome, x: i32, y: i32| {
        o.tile_x = Some(x);
        o.tile_y = Some(y);
    };
    run(
        &ctx,
        &mut state,
        &[
            outcome(OutcomeKind::PlaceNode, |o| {
                at(o, 0, 0);
                o.node_type_id = Some("node-rock".into());
            }),
            outcome(OutcomeKind::PlaceMachine, |o| {
                at(o, 5, 0);
                o.machine_type_id = Some("machine-furnace".into());
            }),
            // Not onto the node, and not onto a wall.
            outcome(OutcomeKind::PlaceMachine, |o| {
                at(o, 0, 0);
                o.machine_type_id = Some("machine-furnace".into());
            }),
            outcome(OutcomeKind::PlaceNode, |o| {
                at(o, 4, 4);
                o.node_type_id = Some("node-rock".into());
            }),
        ],
    );
    let tile = |state: &GameState, x: usize, y: usize| state.world.scenes[0].tiles[y][x].clone();
    assert_eq!(tile(&state, 0, 0).node.map(|node| node.type_id), Some("node-rock".to_owned()));
    assert!(tile(&state, 0, 0).machine.is_none());
    assert_eq!(tile(&state, 5, 0).machine.map(|machine| machine.type_id), Some("machine-furnace".to_owned()));
    assert!(tile(&state, 4, 4).node.is_none());
    run(
        &ctx,
        &mut state,
        &[outcome(OutcomeKind::RemoveNode, |o| at(o, 0, 0)), outcome(OutcomeKind::RemoveMachine, |o| at(o, 5, 0))],
    );
    assert!(tile(&state, 0, 0).node.is_none() && tile(&state, 5, 0).machine.is_none());
}

#[test]
fn messages_fill_in_placeholders() {
    let (ctx, mut state) = make_engine(|_| {}, "words");
    state.player.name = "Ada".into();
    state.flags.insert("coins".into(), json!(4));
    let effects = events::apply_outcomes(
        &ctx,
        &mut state,
        &[outcome(OutcomeKind::Message, |o| {
            o.message = Some("{player} has {var:coins} coins; {npc:npc-test} waves. {unknown} {".into())
        })],
        0,
    );
    let text = match &effects[0] {
        farm_sim::Effect::Message { text, .. } => text.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(text, "Ada has 4 coins; Testy waves. {unknown} {");
}
