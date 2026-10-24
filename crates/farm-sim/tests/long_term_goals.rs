//! M6: long-term goals — the shipping bin, collections, bundles, achievements and mail.

mod common;

use common::{flag_is_true, has_message, make_engine, slot};
use farm_sim::schema::{
    AchievementDefinition, ActionDef, BundleDefinition, BundleItem, CollectionDefinition, CollectionReward,
    EventCondition, EventOutcome, GameEvent, GameState, InventorySlot, Item, MachineTypeDefinition, MailDefinition,
    NpcBirthday, OutcomeKind, RecipeIngredient, TileMachine,
};
use farm_sim::{engine, mail, Command, CommandRules, Effect, EngineContext};

fn run(ctx: &EngineContext, state: &mut GameState, command: Command) -> Vec<Effect> {
    engine::apply_command(ctx, state, &command)
}

fn crop_item(id: &str, value: i64) -> Item {
    Item {
        id: id.to_owned(),
        name: id.to_owned(),
        r#type: "crop".to_owned(),
        stackable: true,
        max_stack: 99,
        value,
        ..Item::default()
    }
}

fn give(state: &mut GameState, item: &Item, quantity: u32, quality: Option<&str>) {
    state.player.inventory.push(InventorySlot { item: item.clone(), quantity, quality: quality.map(str::to_owned) });
}

fn set_flag(flag: &str) -> EventOutcome {
    EventOutcome { flag_name: Some(flag.to_owned()), ..EventOutcome::of(OutcomeKind::SetFlag) }
}

#[test]
fn the_shipping_bin_sells_overnight_and_keeps_a_log() {
    let (ctx, mut state) = make_engine(
        |project| {
            project.items.push(crop_item("melon", 40));
            project.items.push(Item { r#type: "quest".into(), ..crop_item("letter", 0) });
            project.machine_types.push(MachineTypeDefinition {
                id: "bin".into(),
                name: "Shipping bin".into(),
                shipping: true,
                ..MachineTypeDefinition::default()
            });
        },
        "ship",
    );
    let melon = ctx.item("melon").unwrap().clone();
    give(&mut state, &melon, 2, None);
    give(&mut state, &melon, 1, Some("gold"));
    let letter = ctx.item("letter").unwrap().clone();
    give(&mut state, &letter, 1, None);
    state.world.scenes[0].tiles[3][3].machine = Some(TileMachine { type_id: "bin".into(), ..TileMachine::default() });
    run(&ctx, &mut state, Command::Interact);
    assert_eq!(
        farm_sim::overlay::overlay_view(&ctx, &state).storage.map(|bin| (bin.shipping, bin.slots)),
        Some((true, 24))
    );
    run(&ctx, &mut state, Command::StoreItem { item_id: "melon".into(), quantity: 3, quality: None });
    // Quest items can't be sold, so they can't be shipped.
    let refused = run(&ctx, &mut state, Command::StoreItem { item_id: "letter".into(), quantity: 1, quality: None });
    assert!(has_message(&refused, |text| text.ends_with("can't be shipped.")));
    run(&ctx, &mut state, Command::CloseStorage);
    let money = state.player.money;
    let night = run(&ctx, &mut state, Command::Sleep);
    // Gold sells at 1.5×: 40 + 40 + 60.
    assert_eq!(state.player.money, money + 140);
    assert!(has_message(&night, |text| text == "Shipped 3 item(s) overnight for $140."));
    assert_eq!(state.shipped.get("melon"), Some(&3));
    assert_eq!(state.last_shipment.len(), 2);
    assert!(state.world.scenes[0].tiles[3][3].machine.as_ref().unwrap().contents.is_empty());
    run(&ctx, &mut state, Command::Sleep);
    assert!(state.last_shipment.is_empty(), "nothing shipped the next night");
    assert_eq!(state.shipped.get("melon"), Some(&3));
}

#[test]
fn collections_fill_as_items_are_found_and_give_their_rewards() {
    let (ctx, mut state) = make_engine(
        |project| {
            project.items.push(crop_item("shell", 1));
            project.items.push(crop_item("pearl", 1));
            project.collections = Some(vec![CollectionDefinition {
                id: "beach".into(),
                name: "Beach finds".into(),
                items: vec!["shell".into(), "pearl".into(), "seed-wheat".into()],
                rewards: vec![
                    CollectionReward { count: 2, message: "Halfway!".into(), outcomes: vec![set_flag("half")] },
                    CollectionReward { count: 3, message: String::new(), outcomes: vec![set_flag("all")] },
                ],
                ..CollectionDefinition::default()
            }]);
            project.actions.push(ActionDef {
                id: "find".into(),
                name: "Find".into(),
                outcomes: vec![EventOutcome {
                    item_id: Some("shell".into()),
                    item_quantity: Some(1),
                    ..EventOutcome::of(OutcomeKind::GiveItem)
                }],
                ..ActionDef::default()
            });
        },
        "collect",
    );
    // The seeds held from the start count with the first step.
    let first = run(&ctx, &mut state, Command::SetMoveIntent { dx: 0, dy: 0 });
    assert!(has_message(&first, |text| text == "seed-wheat added to the Beach finds!"
        || text.ends_with("added to the Beach finds!")));
    assert_eq!(state.collections["beach"].items, vec!["seed-wheat".to_owned()]);
    let found = run(&ctx, &mut state, Command::PerformAction { action_id: "find".into() });
    assert!(has_message(&found, |text| text == "Halfway!"));
    assert!(flag_is_true(&state, "half"));
    assert!(!flag_is_true(&state, "all"));
    assert_eq!(state.collections["beach"].rewarded, 1);
    // A second shell adds nothing.
    run(&ctx, &mut state, Command::PerformAction { action_id: "find".into() });
    assert_eq!(state.collections["beach"].items.len(), 2);
    let cond = EventCondition::Collection { collection_id: "beach".into(), min: None };
    assert!(!farm_sim::events::condition_met(&ctx, &state, &cond, None));
}

#[test]
fn donations_are_taken_where_the_collection_wants_them() {
    let (ctx, mut state) = make_engine(
        |project| {
            project.items.push(crop_item("fossil", 1));
            project.collections = Some(vec![CollectionDefinition {
                id: "museum".into(),
                name: "Museum".into(),
                kind: "donated".into(),
                items: vec!["fossil".into()],
                scene_id: Some("museum-hall".into()),
                ..CollectionDefinition::default()
            }]);
        },
        "donate",
    );
    let fossil = ctx.item("fossil").unwrap().clone();
    give(&mut state, &fossil, 1, None);
    let away = run(&ctx, &mut state, Command::DonateItem { collection_id: "museum".into(), item_id: "fossil".into() });
    assert!(has_message(&away, |text| text == "Hand that in at the Museum."));
    state.world.scenes[0].id = "museum-hall".into();
    state.player.scene_id = "museum-hall".into();
    run(&ctx, &mut state, Command::DonateItem { collection_id: "museum".into(), item_id: "fossil".into() });
    assert_eq!(state.collections["museum"].items, vec!["fossil".to_owned()]);
    assert_eq!(farm_sim::inventory::count_item(&state.player.inventory, "fossil"), 0);
    let again = run(&ctx, &mut state, Command::DonateItem { collection_id: "museum".into(), item_id: "fossil".into() });
    assert!(has_message(&again, |text| text == "That has been handed in already."));
}

#[test]
fn bundles_take_items_line_by_line_and_fix_the_world_when_complete() {
    let (ctx, mut state) = make_engine(
        |project| {
            project.items.push(crop_item("parsnip", 10));
            project.bundles = Some(vec![BundleDefinition {
                id: "spring".into(),
                name: "Spring crops".into(),
                group: "Pantry".into(),
                items: vec![
                    BundleItem { item_id: "parsnip".into(), quantity: 3, quality: Some("silver".into()) },
                    BundleItem { item_id: "seed-wheat".into(), quantity: 2, quality: None },
                ],
                outcomes: vec![set_flag("bridge-fixed")],
                ..BundleDefinition::default()
            }]);
            project.events.push(GameEvent {
                id: "bundle-done".into(),
                name: "Bundle done".into(),
                trigger: "bundle".into(),
                subject: Some("spring".into()),
                active: true,
                outcomes: vec![set_flag("celebrated")],
                ..GameEvent::default()
            });
        },
        "bundle",
    );
    let parsnip = ctx.item("parsnip").unwrap().clone();
    give(&mut state, &parsnip, 5, None);
    let low = run(&ctx, &mut state, Command::HandInBundle { bundle_id: "spring".into(), line: 0 });
    assert!(has_message(&low, |text| text == "You have nothing to hand in for that."), "normal parsnips are too poor");
    give(&mut state, &parsnip, 1, Some("silver"));
    give(&mut state, &parsnip, 4, Some("gold"));
    run(&ctx, &mut state, Command::HandInBundle { bundle_id: "spring".into(), line: 0 });
    assert_eq!(state.bundles["spring"].handed, vec![3, 0]);
    // The silver one went first.
    assert_eq!(farm_sim::inventory::count_item_with_quality(&state.player.inventory, "parsnip", Some("gold")), 2);
    assert!(!state.bundles["spring"].complete);
    let done = run(&ctx, &mut state, Command::HandInBundle { bundle_id: "spring".into(), line: 1 });
    assert!(has_message(&done, |text| text == "Spring crops complete!"));
    assert!(state.bundles["spring"].complete);
    assert!(flag_is_true(&state, "bridge-fixed"));
    assert!(flag_is_true(&state, "celebrated"));
}

#[test]
fn achievements_unlock_the_moment_their_conditions_hold() {
    let (ctx, mut state) = make_engine(
        |project| {
            project.achievements = Some(vec![AchievementDefinition {
                id: "rich".into(),
                name: "Getting rich".into(),
                conditions: vec![EventCondition::Money { min: 150 }],
                ..AchievementDefinition::default()
            }]);
            project.actions.push(ActionDef {
                id: "pay".into(),
                name: "Pay".into(),
                outcomes: vec![EventOutcome { amount: Some(100_000), ..EventOutcome::of(OutcomeKind::GiveMoney) }],
                ..ActionDef::default()
            });
        },
        "achieve",
    );
    assert!(run(&ctx, &mut state, Command::SetMoveIntent { dx: 0, dy: 0 }).is_empty());
    let paid = run(&ctx, &mut state, Command::PerformAction { action_id: "pay".into() });
    assert!(paid
        .iter()
        .any(|effect| matches!(effect, Effect::AchievementUnlocked { achievement_id } if achievement_id == "rich")));
    assert!(has_message(&paid, |text| text == "Achievement unlocked: Getting rich"));
    assert_eq!(state.achievements.get("rich"), Some(&1));
    let cond = EventCondition::Achievement { achievement_id: "rich".into() };
    assert!(farm_sim::events::condition_met(&ctx, &state, &cond, None));
    // Once.
    let again = run(&ctx, &mut state, Command::PerformAction { action_id: "pay".into() });
    assert!(!again.iter().any(|effect| matches!(effect, Effect::AchievementUnlocked { .. })));
}

#[test]
fn letters_arrive_in_the_morning_and_hand_over_what_they_hold() {
    let (ctx, mut state) = make_engine(
        |project| {
            project.items.push(crop_item("cookie", 3));
            project.npcs[0].birthday = Some(NpcBirthday { season: "spring".into(), day: 3 });
            project.mail = Some(vec![
                MailDefinition {
                    id: "welcome".into(),
                    subject: "Welcome".into(),
                    from: "npc-test".into(),
                    text: "Hello {player}!".into(),
                    attachments: vec![RecipeIngredient { item_id: "cookie".into(), quantity: 2 }],
                    money: 10,
                    outcomes: vec![set_flag("read-welcome")],
                    ..MailDefinition::default()
                },
                MailDefinition {
                    id: "birthday".into(),
                    subject: "A birthday".into(),
                    text: "It's Testy's birthday.".into(),
                    conditions: vec![EventCondition::Birthday { npc_id: "npc-test".into() }],
                    repeat: true,
                    ..MailDefinition::default()
                },
            ]);
            project.actions.push(ActionDef {
                id: "post".into(),
                name: "Post".into(),
                outcomes: vec![EventOutcome {
                    mail_id: Some("welcome".into()),
                    ..EventOutcome::of(OutcomeKind::SendMail)
                }],
                ..ActionDef::default()
            });
        },
        "mail",
    );
    let ctx = ctx.with_rules(CommandRules::Scripted);
    run(&ctx, &mut state, Command::PerformAction { action_id: "post".into() });
    assert_eq!(state.mail_queue, vec!["welcome".to_owned()]);
    assert!(state.mailbox.is_empty());
    let morning = run(&ctx, &mut state, Command::Sleep);
    assert!(has_message(&morning, |text| text == "You've got mail (1)."));
    assert_eq!(mail::unread(&state), 1);
    let money = state.player.money;
    let read = run(&ctx, &mut state, Command::ReadMail { index: 0 });
    assert!(has_message(&read, |text| text.starts_with("Received 2")));
    assert!(flag_is_true(&state, "read-welcome"));
    assert_eq!(state.player.money, money + 10);
    assert!(state.mailbox[0].read && state.mailbox[0].taken);
    // Reading again hands over nothing more.
    run(&ctx, &mut state, Command::ReadMail { index: 0 });
    assert_eq!(farm_sim::inventory::count_item(&state.player.inventory, "cookie"), 2);
    // The birthday letter comes on day 3 by itself.
    run(&ctx, &mut state, Command::Sleep);
    assert_eq!(state.mailbox.len(), 2);
    assert_eq!(state.mailbox[1].mail_id, "birthday");
    run(&ctx, &mut state, Command::Sleep);
    assert_eq!(state.mailbox.len(), 2, "not the day after");
    let _ = slot;
}
