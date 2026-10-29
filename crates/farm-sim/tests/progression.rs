//! Permanent progression, timed modifiers, and configurable training regressions.
mod fixture_project;
use farm_sim::schema::*;
use farm_sim::{buffs, progression, skills, units};
use fixture_project::make_engine;

#[test]
fn permanent_upgrades_clamp_without_discarding_items_and_health_survives_sleep() {
    let (ctx, mut state) = make_engine("upgrades", |project| {
        project.settings.progression =
            Some(ProgressionLimits { max_inventory_size: 30, max_energy: units::points(120), max_health: 150 });
        project.settings.combat = Some(CombatSettings { enabled: true, ..Default::default() });
    });
    let inventory = state.player.inventory.clone();
    progression::apply(&ctx, &mut state, "setInventorySize", -100);
    assert_eq!(state.player.max_inventory_size as usize, inventory.len());
    assert_eq!(state.player.inventory, inventory);
    progression::apply(&ctx, &mut state, "addInventorySize", i64::MAX);
    assert_eq!(state.player.max_inventory_size, 30);
    progression::apply(&ctx, &mut state, "addMaxEnergy", i64::MAX);
    assert_eq!(state.player.max_energy, units::points(120));
    progression::apply(&ctx, &mut state, "addMaxHealth", 1000);
    assert_eq!(state.player.max_health, Some(150));
    farm_sim::combat::restore_overnight(&ctx, &mut state);
    assert_eq!(state.player.health, Some(150));
    let restored: GameState = serde_json::from_value(serde_json::to_value(&state).unwrap()).unwrap();
    assert_eq!(restored.player.max_health, Some(150));
}

#[test]
fn buffs_refresh_stack_expire_and_round_trip_without_changing_base_values() {
    let (ctx, mut state) = make_engine("buffs", |_| {});
    let buff = BuffDefinition {
        id: "coffee".into(),
        stat: "moveSpeed".into(),
        amount: 20,
        duration_minutes: 10,
        ..Default::default()
    };
    buffs::apply(&ctx, &mut state, &buff, "coffee");
    buffs::apply(&ctx, &mut state, &buff, "coffee");
    assert_eq!(state.player.buffs.len(), 1);
    buffs::apply(&ctx, &mut state, &BuffDefinition { id: "soup".into(), amount: 30, ..buff.clone() }, "soup");
    assert_eq!(skills::stat_bonus(&ctx, &state, "moveSpeed"), 30);
    buffs::apply(&ctx, &mut state, &BuffDefinition { id: "magic".into(), amount: 5, additive: true, ..buff }, "event");
    assert_eq!(skills::stat_bonus(&ctx, &state, "moveSpeed"), 35);
    let restored: GameState = serde_json::from_value(serde_json::to_value(&state).unwrap()).unwrap();
    assert_eq!(restored.player.buffs, state.player.buffs);
    state.clock.time_minutes += units::minutes(10);
    assert!(!buffs::active(&ctx, &state, "coffee"));
    assert_eq!(skills::stat_bonus(&ctx, &state, "moveSpeed"), 0);
}

#[test]
fn authored_training_replaces_legacy_xp_and_level_rewards_run_once() {
    let (ctx, mut state) = make_engine("skills", |project| {
        project.settings.skill_level_curve = vec![0, 5, 10];
        project.settings.skill_definitions = Some(vec![SkillDefinition {
            id: "gardening".into(),
            name: "Gardening".into(),
            training: vec![SkillTraining { action: "harvest".into(), experience: 5, subject: None }],
            rewards: vec![SkillReward {
                level: 1,
                stat: "yield".into(),
                amount: 2,
                outcomes: Some(vec![EventOutcome {
                    r#type: "addMaxEnergy".into(),
                    amount: Some(5000),
                    ..Default::default()
                }]),
            }],
            ..Default::default()
        }]);
    });
    let initial = state.player.max_energy;
    skills::grant_action_xp(&ctx, &mut state, "harvest", "farming", 8);
    assert_eq!(state.player.skills["gardening"].xp, 5);
    assert_eq!(skills::stat_bonus(&ctx, &state, "yield"), 2);
    assert_eq!(state.player.max_energy, initial + units::points(5));
    skills::grant_action_xp(&ctx, &mut state, "harvest", "farming", 8);
    assert_eq!(state.player.max_energy, initial + units::points(5));
}

#[test]
fn blacksmith_rejects_insufficient_materials_then_holds_and_returns_tool_in_slot() {
    let (mut ctx, mut state) = make_engine("smith", |_| {});
    let original = state.player.inventory.iter().find(|slot| slot.item.tool_type.is_some()).unwrap().item.clone();
    let mut upgraded = original.clone();
    upgraded.id = "upgraded-tool".into();
    upgraded.tool_tier = Some(original.tool_tier.unwrap_or(1) + 1);
    ctx.content.items.push(upgraded.clone());
    ctx.content.shops.push(ShopDefinition {
        id: "smith".into(),
        tool_upgrades: Some(vec![ToolUpgrade {
            from_item_id: original.id.clone(),
            to_item_id: upgraded.id.clone(),
            cost: 20,
            materials: vec![RecipeIngredient { item_id: "seed-wheat".into(), quantity: 11 }],
            nights: 2,
        }]),
        ..Default::default()
    });
    // Rebuild indexes after adding content.
    let ctx = farm_sim::EngineContext::new(ctx.content);
    state.shop = Some(ShopSession { shop_id: "smith".into() });
    let before = state.clone();
    progression::order_tool(&ctx, &mut state, &original.id);
    assert_eq!(state, before);
    let mut content = ctx.content;
    content.shops.last_mut().unwrap().tool_upgrades.as_mut().unwrap()[0].materials[0].quantity = 2;
    let ctx = farm_sim::EngineContext::new(content);
    let index = state.player.inventory.iter().position(|slot| slot.item.id == original.id).unwrap();
    state.player.equipped_tool = Some(original.id.clone());
    progression::order_tool(&ctx, &mut state, &original.id);
    assert_eq!(state.player.money, before.player.money - 20);
    assert_eq!(state.player.tool_upgrades.len(), 1);
    assert!(state.player.equipped_tool.is_none());
    assert!(!state.player.inventory.iter().any(|slot| slot.item.id == original.id));
    progression::collect_tool(&ctx, &mut state, &upgraded.id);
    assert_eq!(state.player.tool_upgrades.len(), 1);
    state.clock.day += 2;
    progression::collect_tool(&ctx, &mut state, &upgraded.id);
    assert!(state.player.tool_upgrades.is_empty());
    assert_eq!(state.player.inventory[index].item.id, upgraded.id);
    assert_eq!(state.player.equipped_tool.as_deref(), Some(upgraded.id.as_str()));
}

#[test]
fn perks_can_only_be_chosen_once_and_kept_state_preserves_progression() {
    let (ctx, mut state) = make_engine("perks", |project| {
        project.settings.skill_definitions = Some(vec![SkillDefinition {
            id: "farming".into(),
            perks: Some(vec![SkillPerk {
                id: "profession".into(),
                level: 1,
                choices: vec![SkillPerkChoice {
                    id: "grower".into(),
                    name: "Grower".into(),
                    stat: "yield".into(),
                    amount: 2,
                    ..Default::default()
                }],
            }]),
            ..Default::default()
        }]);
    });
    skills::choose_perk(&ctx, &mut state, "farming", "profession", "grower");
    assert!(state.player.perks.is_empty());
    state.player.skills.insert("farming".into(), SkillState { xp: 50, level: 1 });
    skills::choose_perk(&ctx, &mut state, "farming", "profession", "grower");
    assert_eq!(skills::stat_bonus(&ctx, &state, "yield"), 2);
    skills::choose_perk(&ctx, &mut state, "farming", "profession", "grower");
    assert_eq!(state.player.perks.len(), 1);
    let kept = KeptGoals::of_state(&state).unwrap();
    let mut restored = GameState::default();
    kept.apply_to(&mut restored);
    assert_eq!(restored.player.perks, state.player.perks);
}

#[test]
fn tool_service_refuses_after_hours_without_losing_pending_order() {
    let (mut ctx, mut state) = make_engine("closed", |_| {});
    ctx.content.shops.push(ShopDefinition {
        id: "closed".into(),
        hours: Some(ShopHours { open: 900, close: 1000 }),
        ..Default::default()
    });
    let ctx = farm_sim::EngineContext::new(ctx.content);
    state.shop = Some(ShopSession { shop_id: "closed".into() });
    state.player.tool_upgrades.push(PendingToolUpgrade {
        shop_id: "closed".into(),
        item: Item { id: "ready".into(), ..Default::default() },
        ready_day: state.clock.day,
        slot: 0,
        was_equipped: false,
    });
    let before = state.clone();
    progression::collect_tool(&ctx, &mut state, "ready");
    assert_eq!(state, before);
    state.clock.time_minutes = units::minutes(900);
    progression::collect_tool(&ctx, &mut state, "ready");
    assert!(state.player.tool_upgrades.is_empty());
}

#[test]
fn skill_pack_ids_and_training_references_are_namespaced() {
    let pack: ContentPack = serde_json::from_value(serde_json::json!({
        "manifest": {"id": "garden"}, "content": {
            "items": [{"id":"plant"}],
            "skills": [{"id": "gardening", "name": "Gardening", "training":[{"action":"harvest", "subject":"plant", "experience":1}]}],
            "actions": [{"id": "train", "outcomes": [{"type": "giveExperience", "skill": "gardening", "amount": 5}]}]
        }
    }))
    .unwrap();
    let rewritten = farm_sim::packs::namespace_pack(&pack);
    assert_eq!(rewritten.content.skills.as_ref().unwrap()[0].id, "garden:gardening");
    assert_eq!(rewritten.content.skills.as_ref().unwrap()[0].training[0].subject.as_deref(), Some("garden:plant"));
    assert_eq!(rewritten.content.actions[0].outcomes[0].skill.as_deref(), Some("garden:gardening"));
}

#[test]
fn subject_specific_training_only_rewards_matching_actions() {
    let (ctx, mut state) = make_engine("subjects", |project| {
        project.settings.skill_definitions = Some(vec![SkillDefinition {
            id: "mining".into(),
            training: vec![SkillTraining { action: "gather".into(), subject: Some("node-rock".into()), experience: 3 }],
            ..Default::default()
        }]);
    });
    skills::grant_action_xp_subject(&ctx, &mut state, "gather", "node-tree", "mining", 4);
    assert!(!state.player.skills.contains_key("mining"));
    skills::grant_action_xp_subject(&ctx, &mut state, "gather", "node-rock", "mining", 4);
    assert_eq!(state.player.skills["mining"].xp, 3);
}

#[test]
fn recursive_experience_rewards_are_bounded_and_next_grant_recovers() {
    let (ctx, mut state) = make_engine("recursive", |project| {
        project.settings.skill_level_curve = (0..1000).collect();
        project.settings.skill_definitions = Some(vec![SkillDefinition {
            id: "loop".into(),
            rewards: (1..1000)
                .map(|level| SkillReward {
                    level,
                    outcomes: Some(vec![EventOutcome {
                        r#type: "giveExperience".into(),
                        skill: Some("loop".into()),
                        amount: Some(1000),
                        ..Default::default()
                    }]),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }]);
    });
    skills::grant_xp(&ctx, &mut state, "loop", 1);
    let first = state.player.skills["loop"].xp;
    assert!(first > 1 && first < 20);
    skills::grant_xp(&ctx, &mut state, "loop", 1);
    assert!(state.player.skills["loop"].xp > first);
}

#[test]
fn temporary_health_does_not_become_permanent_and_expiry_clamps_resources() {
    let (ctx, mut state) = make_engine("temporary-health", |project| {
        project.settings.combat = Some(CombatSettings { enabled: true, ..Default::default() });
    });
    let base = farm_sim::combat::player_max_health(&ctx, &state).unwrap();
    let buff = BuffDefinition {
        id: "health".into(),
        stat: "maxHealth".into(),
        amount: 30,
        duration_minutes: 1,
        ..Default::default()
    };
    buffs::apply(&ctx, &mut state, &buff, "event");
    progression::apply(&ctx, &mut state, "addMaxHealth", 5);
    assert_eq!(state.player.max_health, Some(base + 5));
    assert_eq!(farm_sim::combat::player_max_health(&ctx, &state), Some(base + 35));
    buffs::apply(&ctx, &mut state, &BuffDefinition { id: "energy".into(), stat: "maxEnergy".into(), ..buff }, "event");
    state.player.health = Some(base + 35);
    state.player.energy = buffs::max_energy(&ctx, &state);
    state.clock.time_minutes += units::minutes(1);
    farm_sim::engine::advance_tick(&ctx, &mut state, 1);
    assert!(state.player.buffs.is_empty());
    assert_eq!(state.player.health, Some(base + 5));
    assert!(state.player.energy <= state.player.max_energy);
}

#[test]
fn sleep_removes_temporary_caps_before_refilling_and_signed_levels_clamp() {
    let (ctx, mut state) = make_engine("sleep-buffs", |_| {});
    let buff = BuffDefinition {
        id: "energy".into(),
        stat: "maxEnergy".into(),
        amount: 50,
        duration_minutes: 10000,
        ..Default::default()
    };
    buffs::apply(&ctx, &mut state, &buff, "event");
    state.player.skills.insert("farming".into(), SkillState { xp: 100, level: 4 });
    buffs::apply(
        &ctx,
        &mut state,
        &BuffDefinition { id: "weaken".into(), stat: "skillLevel".into(), amount: -3, ..buff },
        "event",
    );
    assert_eq!(skills::effective_level(&ctx, &state, "farming"), 1);
    assert_eq!(skills::effective_level(&ctx, &state, "fishing"), 0);
    farm_sim::game_time::perform_sleep(&ctx, &mut state, farm_sim::game_time::SleepOptions { collapsed: false });
    assert!(state.player.buffs.is_empty());
    assert!(state.player.energy <= state.player.max_energy);
    assert_eq!(skills::effective_level(&ctx, &state, "farming"), 4);
}

#[test]
fn optional_authored_lists_preserve_explicit_empty_arrays_and_purchase_keys_do_not_collide() {
    let item: Item = serde_json::from_value(serde_json::json!({"id":"food", "buffs":[]})).unwrap();
    assert_eq!(serde_json::to_value(item).unwrap()["buffs"], serde_json::json!([]));
    let shop: ShopDefinition = serde_json::from_value(serde_json::json!({"id":"smith", "toolUpgrades":[]})).unwrap();
    assert_eq!(serde_json::to_value(shop).unwrap()["toolUpgrades"], serde_json::json!([]));
    assert_ne!(progression::purchase_key("pack:store", "bag"), progression::purchase_key("pack", "store:bag"));
}

#[test]
fn upgrade_stock_charges_once_without_adding_an_item() {
    let (mut ctx, mut state) = make_engine("upgrade-stock", |_| {});
    let item_id = ctx.content.items[0].id.clone();
    ctx.content.shops.push(ShopDefinition {
        id: "upgrades".into(),
        stock: vec![ShopStockEntry {
            item_id: item_id.clone(),
            price: Some(20),
            upgrade: Some(PlayerUpgrade { id: "bag".into(), kind: "addInventorySize".into(), amount: 6 }),
            ..Default::default()
        }],
        ..Default::default()
    });
    let ctx = farm_sim::EngineContext::new(ctx.content);
    state.shop = Some(ShopSession { shop_id: "upgrades".into() });
    let before = state.clone();
    farm_sim::economy::handle_buy_item(&ctx, &mut state, &item_id, 1);
    assert_eq!(state.player.inventory, before.player.inventory);
    assert_eq!(state.player.max_inventory_size, before.player.max_inventory_size + 6);
    assert_eq!(state.player.money, before.player.money - 20);
    let upgraded = state.clone();
    farm_sim::economy::handle_buy_item(&ctx, &mut state, &item_id, 1);
    assert_eq!(state, upgraded);
}
