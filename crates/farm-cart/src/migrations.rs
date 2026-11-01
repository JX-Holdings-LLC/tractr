//! Save migrations of a game update (#242): the rules a creator ships with a new version of the
//! game ([`SaveMigration`] in the content), applied to a save written by an earlier version when it
//! loads, before ids the content no longer has go to quarantine.
//!
//! - An **item** rule maps an old item id to a new one (`to`, `quantity` units each), or pays for
//!   it (`money` gold a unit), or drops it. It reaches the inventory, worn items, machine and chest
//!   contents, items lying on the ground and quarantine. Inventory slots go back through the
//!   normal add (stack caps, slot limit); what does not fit waits in quarantine and returns at a
//!   later load when there is room.
//! - A **scene** rule renames a saved scene, so the world rebase matches it to the new map; a
//!   player who was in it lands in the new scene (on `x`, `y` when given). A scene removed (no
//!   `to`) sends the player to the start scene.
//!
//! A rule applies only while the content has nothing under its `from` id (an id reused for new
//! content is left alone). Every rule that changed something gives one notice in the player's
//! language, shown once after loading: the save written afterwards no longer holds the old ids.

use farm_sim::messages::{self, Message};
use farm_sim::schema::{save_migration_kinds, GameContent, GameState, InventorySlot, Item, SaveMigration};
use farm_sim::{inventory, units};

/// What one item rule did, across every place the save held the item.
#[derive(Default)]
struct ItemTally {
    units: u32,
    /// The old item's name, as the save had it.
    name: Option<String>,
}

/// Applies `content.save_migrations` to `state`. Returns a notice for each rule that changed
/// something, in rule order.
pub fn apply_migrations(state: &mut GameState, content: &GameContent) -> Vec<Message> {
    let mut notices = Vec::new();
    for rule in &content.save_migrations {
        if rule.from.is_empty() || rule.to.as_deref() == Some(rule.from.as_str()) {
            continue;
        }
        match rule.kind.as_str() {
            save_migration_kinds::ITEM => {
                if content.items.iter().any(|item| item.id == rule.from) {
                    continue;
                }
                notices.extend(migrate_item(state, content, rule));
            }
            save_migration_kinds::SCENE => {
                if content.scenes.iter().any(|scene| scene.id == rule.from) {
                    continue;
                }
                notices.extend(migrate_scene(state, content, rule));
            }
            _ => {}
        }
    }
    notices
}

fn migrate_item(state: &mut GameState, content: &GameContent, rule: &SaveMigration) -> Option<Message> {
    let target: Option<&Item> = rule.to.as_deref().and_then(|to| content.items.iter().find(|item| item.id == to));
    // A rule naming an item the game lacks changes nothing: the old item waits in quarantine.
    if rule.to.is_some() && target.is_none() {
        return None;
    }
    let per_unit = rule.quantity.unwrap_or(1).max(1);
    let mut tally = ItemTally::default();
    let convert = |slot: &InventorySlot, tally: &mut ItemTally| -> Option<InventorySlot> {
        tally.units = tally.units.saturating_add(slot.quantity);
        tally.name.get_or_insert_with(|| slot.item.name.clone());
        target.map(|item| InventorySlot {
            item: item.clone(),
            quantity: slot.quantity.saturating_mul(per_unit),
            quality: slot.quality.clone(),
        })
    };

    // The inventory: converted slots go back through the normal add.
    let mut kept = Vec::with_capacity(state.player.inventory.len());
    let mut added = Vec::new();
    for slot in std::mem::take(&mut state.player.inventory) {
        if slot.item.id == rule.from {
            added.extend(convert(&slot, &mut tally));
        } else {
            kept.push(slot);
        }
    }
    let mut overflow = Vec::new();
    for slot in added {
        let result = inventory::add_item_with_quality(
            &kept,
            &slot.item,
            slot.quality.as_deref(),
            slot.quantity,
            state.player.max_inventory_size,
            None,
        );
        let rejected = result.rejected(slot.quantity);
        kept = result.inventory;
        if rejected > 0 {
            overflow.push(InventorySlot { quantity: rejected, ..slot });
        }
    }
    state.player.inventory = kept;

    let map_slots = |slots: &mut Vec<InventorySlot>, tally: &mut ItemTally| {
        *slots = std::mem::take(slots)
            .into_iter()
            .filter_map(|slot| if slot.item.id == rule.from { convert(&slot, tally) } else { Some(slot) })
            .collect();
    };
    map_slots(&mut state.player.worn, &mut tally);
    map_slots(&mut state.quarantined_items, &mut tally);
    for tile in state.world.scenes.iter_mut().flat_map(|scene| scene.tiles.iter_mut().flatten()) {
        if let Some(machine) = tile.machine.as_mut() {
            map_slots(&mut machine.contents, &mut tally);
        }
        if tile.item.as_ref().is_some_and(|item| item.id == rule.from) {
            let dropped = tile.item.take().map(|item| InventorySlot::new(item, 1));
            if let Some(slot) = dropped {
                tile.item = convert(&slot, &mut tally).map(|slot| slot.item);
            }
        }
    }
    state.quarantined_items.extend(overflow);

    if tally.units == 0 {
        return None;
    }
    let old_name = rule.name.clone().or(tally.name).unwrap_or_else(|| rule.from.clone());
    Some(match (target, rule.money) {
        (Some(item), _) if per_unit == 1 => messages::SAVE_ITEM_RENAMED.with(&[&old_name, &item.name]),
        (Some(item), _) => messages::SAVE_ITEM_REPLACED.with(&[
            &old_name,
            &tally.units,
            &tally.units.saturating_mul(per_unit),
            &item.name,
        ]),
        (None, Some(money)) if money > 0 => {
            let paid = money.saturating_mul(i64::from(tally.units));
            state.player.money = state.player.money.saturating_add(paid);
            messages::SAVE_ITEM_BOUGHT_BACK.with(&[&old_name, &tally.units, &paid])
        }
        (None, _) => messages::SAVE_ITEM_REMOVED.with(&[&old_name, &tally.units]),
    })
}

fn migrate_scene(state: &mut GameState, content: &GameContent, rule: &SaveMigration) -> Option<Message> {
    let to = match rule.to.as_deref() {
        Some(to) if content.scenes.iter().any(|scene| scene.id == to) => to.to_owned(),
        Some(_) => return None,
        // Removed: a player inside goes back to where a new game starts.
        None => content.start_scene_id.clone(),
    };
    let saved_has_target = state.world.scenes.iter().any(|scene| scene.id == to);
    for scene in &mut state.world.scenes {
        if scene.id == rule.from {
            if rule.to.is_some() && !saved_has_target {
                // Keeps what play made of it: the world rebase matches it to the new map.
                to.clone_into(&mut scene.id);
            }
        }
        for transition in &mut scene.transitions {
            if transition.to_scene_id == rule.from {
                to.clone_into(&mut transition.to_scene_id);
            }
        }
    }
    for npc in state.npcs.values_mut() {
        if npc.scene_id == rule.from {
            to.clone_into(&mut npc.scene_id);
        }
    }
    for animal in &mut state.animals {
        if animal.scene_id == rule.from {
            to.clone_into(&mut animal.scene_id);
        }
    }
    for building in &mut state.buildings {
        if building.scene_id == rule.from {
            to.clone_into(&mut building.scene_id);
        }
        if building.interior_scene_id == rule.from {
            to.clone_into(&mut building.interior_scene_id);
        }
    }
    if state.player.scene_id != rule.from {
        return None;
    }
    let scene = content.scenes.iter().find(|scene| scene.id == to);
    let (x, y) = match (rule.x, rule.y, rule.to.is_some()) {
        (Some(x), Some(y), _) => (units::tile_center(x), units::tile_center(y)),
        (_, _, true) => (state.player.x, state.player.y),
        (_, _, false) => scene.map_or((state.player.x, state.player.y), |scene| {
            (units::tile_center(scene.width.div_euclid(2)), units::tile_center(scene.height.div_euclid(2)))
        }),
    };
    state.player.scene_id = to;
    (state.player.x, state.player.y) = (x, y);
    state.music = None;
    let name = scene.map_or_else(|| state.player.scene_id.clone(), |scene| scene.name.clone());
    Some(messages::SAVE_SCENE_MOVED.with(&[&name]))
}
