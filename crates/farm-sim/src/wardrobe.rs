//! The player character (M4): clothing worn from the inventory, and the character creator's
//! name, farm name and colours.
//!
//! An item with a `wear` definition is clothing for its slot (hat, shirt, trousers). Wearing one
//! takes it from the inventory into the slot, and what the slot held goes back to the inventory
//! (when there is room; otherwise nothing changes). Taking off returns it. How worn clothes look
//! is the renderer's business: a shirt or trousers colour the player's art, a hat draws its art
//! (or colours the headband).

use crate::effects::{message_levels, Effect};
use crate::engine_types::{Effects, EngineContext};
use crate::inventory;
use crate::messages;
use crate::schema::{wear_slots, GameState, InventorySlot, PlayerLook};

/// The `wearItem` command.
pub fn handle_wear(ctx: &EngineContext, state: &mut GameState, item_id: &str) -> Effects {
    let Some(held) = state.player.inventory.iter().find(|slot| slot.item.id == item_id && slot.quantity > 0) else {
        return vec![Effect::say(message_levels::ERROR, &messages::DONT_HAVE_ITEM)];
    };
    // The item as the content has it now (a save may hold an older copy).
    let item =
        ctx.item(item_id).map_or_else(|| held.item.clone(), |current| inventory::refresh_item(&held.item, current));
    let Some(slot) =
        item.wear.as_ref().map(|wear| wear.slot.clone()).filter(|slot| wear_slots::ALL.contains(&slot.as_str()))
    else {
        return vec![Effect::say(message_levels::INFO, messages::CANT_WEAR.with(&[&item.name]))];
    };
    let mut inventory = inventory::remove_item(&state.player.inventory, item_id, 1);
    let previous =
        state.player.worn.iter().position(|worn| worn.item.wear.as_ref().is_some_and(|wear| wear.slot == slot));
    if let Some(index) = previous {
        let off = &state.player.worn[index];
        let result = inventory::add_item(&inventory, &off.item, 1, state.player.max_inventory_size, None);
        if !result.added {
            return vec![Effect::say(message_levels::ERROR, &messages::INVENTORY_FULL)];
        }
        inventory = result.inventory;
        state.player.worn.remove(index);
    }
    state.player.inventory = inventory;
    let name = item.name.clone();
    state.player.worn.push(InventorySlot { item, quantity: 1, quality: None });
    vec![Effect::say(message_levels::SUCCESS, messages::PUT_ON.with(&[&name]))]
}

/// The `takeOff` command.
pub fn handle_take_off(state: &mut GameState, slot: &str) -> Effects {
    let Some(index) =
        state.player.worn.iter().position(|worn| worn.item.wear.as_ref().is_some_and(|wear| wear.slot == slot))
    else {
        return Vec::new();
    };
    let off = state.player.worn[index].item.clone();
    let result = inventory::add_item(&state.player.inventory, &off, 1, state.player.max_inventory_size, None);
    if !result.added {
        return vec![Effect::say(message_levels::ERROR, &messages::INVENTORY_FULL)];
    }
    state.player.inventory = result.inventory;
    state.player.worn.remove(index);
    vec![Effect::say(message_levels::INFO, messages::TOOK_OFF.with(&[&off.name]))]
}

/// What a worn item makes of `slot` (the item in it), if anything is worn there.
pub fn worn_in<'a>(state: &'a GameState, slot: &str) -> Option<&'a InventorySlot> {
    state.player.worn.iter().find(|worn| worn.item.wear.as_ref().is_some_and(|wear| wear.slot == slot))
}

/// The `setPlayerLook` command: the character creator's answers. Empty names keep the ones the
/// game set; colours that are not `#rrggbb` are dropped.
pub fn handle_set_look(state: &mut GameState, name: &str, farm_name: &str, look: &PlayerLook) -> Effects {
    let name = name.trim();
    if !name.is_empty() {
        state.player.name = name.chars().take(MAX_NAME_CHARS).collect();
    }
    let farm_name = farm_name.trim();
    if !farm_name.is_empty() {
        state.player.farm_name = farm_name.chars().take(MAX_NAME_CHARS).collect();
    }
    let color = |value: &Option<String>| value.as_ref().filter(|value| is_hex_color(value)).cloned();
    let look = PlayerLook {
        skin: color(&look.skin),
        hair: color(&look.hair),
        shirt: color(&look.shirt),
        pants: color(&look.pants),
        accent: color(&look.accent),
    };
    state.player.look = (look != PlayerLook::default()).then_some(look);
    Vec::new()
}

/// `setPlayerLook` under [`crate::CommandRules::Player`]: only before the game's first tick (the
/// character creator runs when a new game starts).
pub fn set_look_refusal(state: &GameState) -> Option<Effects> {
    (state.clock.tick > 0).then(Vec::new)
}

/// The longest name the creator takes, in characters.
pub const MAX_NAME_CHARS: usize = 24;

/// `#rrggbb`.
pub fn is_hex_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
}
