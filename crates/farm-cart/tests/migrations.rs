//! Save migrations (#242): saves written by an earlier version of a game load in the update that
//! renamed, replaced or removed what they hold, and say so once.

use farm_cart::save_file::{self, cart_hash, SaveTarget};
use farm_sim::schema::{GameContent, GameState, InventorySlot, SaveMigration};
use farm_sim::state;

const CART: &[u8] = include_bytes!("../../../fixtures/golden/cartridges/fixture-project.cart");

/// Version 1.0 of the game and a save of it holding 3 of its first item and 2 of its second.
fn version_one() -> (GameContent, SaveTarget, GameState, [String; 2]) {
    let cart = farm_cart::load_cartridge(CART).unwrap();
    let content = cart.content.clone();
    let mut state = state::create_game_state_from_start(&cart.start, Some("migrations"));
    let ids = [content.items[0].id.clone(), content.items[1].id.clone()];
    state.player.inventory =
        vec![InventorySlot::new(content.items[0].clone(), 3), InventorySlot::new(content.items[1].clone(), 2)];
    let target =
        SaveTarget { game_id: "test.migrations".into(), game_version: "1.0".into(), cart_hash: cart_hash(&content) };
    (content, target, state, ids)
}

fn rule(kind: &str, from: &str, to: Option<&str>) -> SaveMigration {
    SaveMigration { kind: kind.into(), from: from.into(), to: to.map(str::to_owned), ..SaveMigration::default() }
}

/// Version 1.1: `renamed` gets a new id and name, `removed` leaves the game, with `rules`.
fn version_two(
    content: &GameContent,
    renamed: &str,
    removed: &str,
    rules: Vec<SaveMigration>,
) -> (GameContent, SaveTarget) {
    let mut next = content.clone();
    for item in &mut next.items {
        if item.id == renamed {
            item.id = format!("{renamed}-v2");
            item.name = "Shiny Tool".into();
        }
    }
    next.items.retain(|item| item.id != removed);
    next.save_migrations = rules;
    let target =
        SaveTarget { game_id: "test.migrations".into(), game_version: "1.1".into(), cart_hash: cart_hash(&next) };
    (next, target)
}

fn count(state: &GameState, id: &str) -> u32 {
    state.player.inventory.iter().filter(|slot| slot.item.id == id).map(|slot| slot.quantity).sum()
}

#[test]
fn renamed_and_bought_back_items_reach_old_saves_once() {
    let (content, target, state, [a, b]) = version_one();
    let old_name = content.items[0].name.clone();
    let save = save_file::write_save(&state, &target);
    let money = state.player.money;

    let mut refund = rule("item", &b, None);
    refund.money = Some(7);
    let (next, next_target) = version_two(&content, &a, &b, vec![rule("item", &a, Some(&format!("{a}-v2"))), refund]);
    let loaded = save_file::load_save(&save, &next_target, &next);
    assert!(loaded.ok, "{:?}", loaded.errors);
    let migrated = loaded.state.unwrap();
    assert_eq!(count(&migrated, &format!("{a}-v2")), 3);
    assert_eq!(count(&migrated, &b), 0);
    assert_eq!(migrated.player.money, money + 14);
    assert!(migrated.quarantined_items.is_empty(), "{:?}", migrated.quarantined_items);
    assert_eq!(loaded.migrations.len(), 2);
    assert_eq!(loaded.migrations[0], format!("Your old {old_name} is now Shiny Tool."));
    assert!(loaded.migrations[1].contains("you received $14"), "{}", loaded.migrations[1]);
    assert!(loaded.warnings.contains(&loaded.migrations[0]));

    // Saved again by 1.1, the save holds the new ids: nothing more to say.
    let again = save_file::write_save(&migrated, &next_target);
    let reloaded = save_file::load_save(&again, &next_target, &next);
    assert!(reloaded.migrations.is_empty());
    let mut other = next_target.clone();
    other.cart_hash = "another build".into();
    assert!(save_file::load_save(&again, &other, &next).migrations.is_empty());
}

#[test]
fn replacements_multiply_and_items_without_a_rule_still_go_to_quarantine() {
    let (content, target, state, [a, b]) = version_one();
    let save = save_file::write_save(&state, &target);
    let mut twice = rule("item", &a, Some(&format!("{a}-v2")));
    twice.quantity = Some(2);
    let (next, next_target) = version_two(&content, &a, &b, vec![twice]);
    let loaded = save_file::load_save(&save, &next_target, &next);
    let migrated = loaded.state.unwrap();
    assert_eq!(count(&migrated, &format!("{a}-v2")), 6);
    assert!(loaded.migrations[0].contains("became 6 \u{d7} Shiny Tool"), "{}", loaded.migrations[0]);
    assert_eq!(loaded.quarantined, vec![b.clone()]);
    assert_eq!(migrated.quarantined_items[0].item.id, b);
}

#[test]
fn a_rule_whose_old_id_is_back_in_the_game_does_nothing() {
    let (content, target, state, [a, _]) = version_one();
    let save = save_file::write_save(&state, &target);
    let mut next = content.clone();
    next.save_migrations = vec![rule("item", &a, Some("nothing-like-it"))];
    let next_target = SaveTarget { cart_hash: cart_hash(&next), ..target };
    let loaded = save_file::load_save(&save, &next_target, &next);
    assert!(loaded.migrations.is_empty());
    assert_eq!(count(&loaded.state.unwrap(), &a), 3);
}

#[test]
fn a_moved_scene_takes_the_player_and_keeps_what_they_did_there() {
    let (content, target, mut state, _) = version_one();
    let old = content.scenes[0].id.clone();
    state.player.scene_id = old.clone();
    state.world.scenes[0].tiles[0][0].soil_moisture = 77;
    let save = save_file::write_save(&state, &target);

    let mut next = content.clone();
    next.scenes[0].id = "moved-farm".into();
    next.scenes[0].name = "New Farm".into();
    if next.start_scene_id == old {
        next.start_scene_id = "moved-farm".into();
    }
    let mut moved = rule("scene", &old, Some("moved-farm"));
    (moved.x, moved.y) = (Some(1), Some(1));
    next.save_migrations = vec![moved];
    let next_target = SaveTarget { cart_hash: cart_hash(&next), ..target };
    let loaded = save_file::load_save(&save, &next_target, &next);
    let migrated = loaded.state.unwrap();
    assert_eq!(migrated.player.scene_id, "moved-farm");
    assert_eq!(migrated.player.x, farm_sim::units::tile_center(1));
    let scene = migrated.world.scenes.iter().find(|scene| scene.id == "moved-farm").unwrap();
    assert_eq!(scene.tiles[0][0].soil_moisture, 77);
    assert!(!migrated.world.scenes.iter().any(|scene| scene.id == old));
    assert_eq!(loaded.migrations, vec!["The place you saved in has moved: you are now in New Farm.".to_owned()]);
}
