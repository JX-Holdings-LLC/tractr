//! Placed objects (M5): chests and shipping bins that store items, gates that open and close,
//! and objects whose area effects water the soil (sprinklers) or protect crops (scarecrows)
//! overnight. Objects are machine types placed on a tile (`placeMachine`, picked up with
//! `pickUpMachine`); furniture and decoration are machine types with none of this.

use crate::effects::{message_levels, Effect};
use crate::engine_types::{Effects, EngineContext};
use crate::inventory;
use crate::messages;
use crate::schema::{
    area_effects, soil_states, GameState, InventorySlot, MachineTypeDefinition, StorageSession, TileMachine,
    MAX_AREA_RADIUS,
};
use crate::world::world_movement;
use std::collections::BTreeSet;

/// What interacting with the object on a tile does (before the machine rules): open a chest or
/// bin, or swing a gate. `None` when the object does neither.
pub fn interact(
    ctx: &EngineContext,
    state: &mut GameState,
    scene_id: &str,
    x: i32,
    y: i32,
    machine: &TileMachine,
) -> Option<Effects> {
    let definition = ctx.machine_type(&machine.type_id)?;
    if definition.storage_slots() > 0 {
        state.storage = Some(StorageSession { scene_id: scene_id.to_owned(), x, y });
        return Some(Vec::new());
    }
    if definition.gate {
        let open = machine.open != Some(true);
        if !open && player_in_the_way(state, scene_id, x, y) {
            return Some(vec![Effect::say(message_levels::INFO, &messages::GATE_BLOCKED)]);
        }
        if let Some(target) = machine_mut(state, scene_id, x, y) {
            target.open = open.then_some(true);
        }
        let message = if open { &messages::GATE_OPENED } else { &messages::GATE_CLOSED };
        return Some(vec![Effect::say(message_levels::INFO, message.with(&[&definition.name]))]);
    }
    None
}

/// Whether the player's box overlaps tile `(x, y)` of the scene (a gate must not close on them).
fn player_in_the_way(state: &GameState, scene_id: &str, x: i32, y: i32) -> bool {
    if state.player.scene_id != scene_id {
        return false;
    }
    let half_w = world_movement::PLAYER_HALF_WIDTH;
    let half_h = world_movement::PLAYER_HALF_HEIGHT;
    let (px, py) = (state.player.x, state.player.y);
    let left = crate::units::tile_of(px - half_w);
    let right = crate::units::tile_of(px + half_w - 1);
    let top = crate::units::tile_of(py - half_h);
    let bottom = crate::units::tile_of(py + half_h - 1);
    (left..=right).contains(&x) && (top..=bottom).contains(&y)
}

/// The placed object on tile `(x, y)` of scene `scene_id`, mutable.
pub(crate) fn machine_mut<'a>(state: &'a mut GameState, scene_id: &str, x: i32, y: i32) -> Option<&'a mut TileMachine> {
    state
        .world
        .scenes
        .iter_mut()
        .find(|scene| scene.id == scene_id)
        .and_then(|scene| scene.tile_mut(x, y))
        .and_then(|tile| tile.machine.as_mut())
}

/// The open storage's object and its type, when the session still points at one.
pub fn open_storage<'a>(
    ctx: &'a EngineContext,
    state: &'a GameState,
) -> Option<(&'a TileMachine, &'a MachineTypeDefinition)> {
    let session = state.storage.as_ref()?;
    let machine = world_movement::find_scene(state, &session.scene_id)?.tile(session.x, session.y)?.machine.as_ref()?;
    let definition = ctx.machine_type(&machine.type_id)?;
    (definition.storage_slots() > 0).then_some((machine, definition))
}

/// Closes the storage when its object is gone (picked up, replaced by an event, its map
/// changed): a modal must not outlive what it shows.
pub fn forget_gone(ctx: &EngineContext, state: &mut GameState) {
    if state.storage.is_some() && open_storage(ctx, state).is_none() {
        state.storage = None;
    }
}

/// Close the open chest or bin.
pub fn handle_close(state: &mut GameState) -> Effects {
    state.storage = None;
    Vec::new()
}

/// Put `quantity` of an item (of `quality`, else any, lowest quality first) from the inventory
/// into the open storage; what does not fit stays in the inventory.
pub fn handle_store(
    ctx: &EngineContext,
    state: &mut GameState,
    item_id: &str,
    quantity: u32,
    quality: Option<&str>,
) -> Effects {
    let Some((machine, definition)) = open_storage(ctx, state) else {
        state.storage = None;
        return Vec::new();
    };
    let slots = definition.storage_slots();
    // A shipping bin takes what can be sold.
    if definition.shipping {
        let unsellable = state
            .player
            .inventory
            .iter()
            .find(|slot| slot.item.id == item_id)
            .filter(|slot| ctx.item_category(&slot.item.r#type).is_some_and(|category| !category.sellable));
        if let Some(slot) = unsellable {
            return vec![Effect::say(message_levels::ERROR, messages::CANT_SHIP.with(&[&slot.item.name]))];
        }
    }
    let mut contents = machine.contents.clone();
    let mut inventory = state.player.inventory.clone();
    let mut moved = 0_u32;
    // The qualities held, lowest first (the tiers' order), or just the one asked for.
    let mut qualities: Vec<Option<String>> = Vec::new();
    for slot in inventory.iter().filter(|slot| slot.item.id == item_id) {
        if !qualities.contains(&slot.quality) {
            qualities.push(slot.quality.clone());
        }
    }
    let rank = |quality: &Option<String>| {
        quality.as_deref().map_or(0, |q| ctx.crop_qualities().iter().position(|tier| tier.id == q).unwrap_or(0))
    };
    qualities.sort_by_key(rank);
    if let Some(quality) = quality {
        let wanted = ctx.slot_quality(Some(quality));
        qualities.retain(|held| *held == wanted);
    }
    for slot_quality in qualities {
        if moved >= quantity {
            break;
        }
        let held = inventory::count_item_with_quality(&inventory, item_id, slot_quality.as_deref());
        let Some(item) = inventory
            .iter()
            .find(|slot| slot.item.id == item_id && slot.quality == slot_quality)
            .map(|slot| slot.item.clone())
        else {
            continue;
        };
        let want = u64::from(quantity - moved).min(held) as u32;
        let added = inventory::add_item_with_quality(&contents, &item, slot_quality.as_deref(), want, slots, None);
        if added.added_quantity == 0 {
            break;
        }
        contents = added.inventory;
        inventory =
            inventory::remove_item_with_quality(&inventory, item_id, slot_quality.as_deref(), added.added_quantity);
        moved += added.added_quantity;
    }
    if moved == 0 {
        let message = if inventory::count_item(&state.player.inventory, item_id) == 0 {
            &messages::DONT_HAVE_ITEM
        } else {
            &messages::STORAGE_FULL
        };
        return vec![Effect::say(message_levels::ERROR, message)];
    }
    let session = state.storage.clone().unwrap_or_default();
    state.player.inventory = inventory;
    if let Some(target) = machine_mut(state, &session.scene_id, session.x, session.y) {
        target.contents = contents;
    }
    Vec::new()
}

/// Take `quantity` from slot `index` of the open storage into the inventory (what fits).
pub fn handle_take(ctx: &EngineContext, state: &mut GameState, index: i32, quantity: u32) -> Effects {
    let Some((machine, _)) = open_storage(ctx, state) else {
        state.storage = None;
        return Vec::new();
    };
    let Some(slot) = usize::try_from(index).ok().and_then(|index| machine.contents.get(index)).cloned() else {
        return Vec::new();
    };
    let index = index as usize;
    let want = quantity.min(slot.quantity);
    if want == 0 {
        return Vec::new();
    }
    let added = inventory::add_item_with_quality(
        &state.player.inventory,
        &slot.item,
        slot.quality.as_deref(),
        want,
        state.player.max_inventory_size,
        None,
    );
    if added.added_quantity == 0 {
        return vec![Effect::say(message_levels::ERROR, &messages::INVENTORY_FULL)];
    }
    let mut contents = machine.contents.clone();
    if contents[index].quantity > added.added_quantity {
        contents[index].quantity -= added.added_quantity;
    } else {
        contents.remove(index);
    }
    let session = state.storage.clone().unwrap_or_default();
    state.player.inventory = added.inventory;
    if let Some(target) = machine_mut(state, &session.scene_id, session.x, session.y) {
        target.contents = contents;
    }
    Vec::new()
}

/// The tiles each area effect covers tonight, by scene: `(water, protect)` sets of `(x, y)`.
/// Empty when no placed object has an area effect.
pub(crate) struct AreaCover {
    scenes: Vec<SceneCover>,
}

/// One scene's covered tiles: its id, the watered tiles and the protected tiles.
type SceneCover = (String, BTreeSet<(i32, i32)>, BTreeSet<(i32, i32)>);

impl AreaCover {
    /// What the placed objects of every scene cover.
    pub fn of(ctx: &EngineContext, state: &GameState) -> Self {
        let mut scenes = Vec::new();
        if ctx.content.machine_types.iter().all(|definition| definition.area_effects.is_empty()) {
            return Self { scenes };
        }
        for scene in &state.world.scenes {
            let mut water = BTreeSet::new();
            let mut protect = BTreeSet::new();
            for (y, row) in scene.tiles.iter().enumerate() {
                for (x, tile) in row.iter().enumerate() {
                    let Some(definition) = tile.machine.as_ref().and_then(|machine| ctx.machine_type(&machine.type_id))
                    else {
                        continue;
                    };
                    for effect in &definition.area_effects {
                        let target = match effect.effect.as_str() {
                            area_effects::WATER => &mut water,
                            area_effects::PROTECT => &mut protect,
                            _ => continue,
                        };
                        let radius = effect.radius.min(MAX_AREA_RADIUS) as i32;
                        let (cx, cy) = (x as i32, y as i32);
                        for ty in cy - radius..=cy + radius {
                            for tx in cx - radius..=cx + radius {
                                if scene.contains(tx, ty) {
                                    target.insert((tx, ty));
                                }
                            }
                        }
                    }
                }
            }
            if !water.is_empty() || !protect.is_empty() {
                scenes.push((scene.id.clone(), water, protect));
            }
        }
        Self { scenes }
    }

    /// Whether any object covers anything.
    pub fn is_empty(&self) -> bool {
        self.scenes.is_empty()
    }

    fn scene(&self, scene_id: &str) -> Option<&SceneCover> {
        self.scenes.iter().find(|(id, ..)| id == scene_id)
    }

    /// Whether an object protects tile `(x, y)` of the scene.
    pub fn protects(&self, scene_id: &str, x: usize, y: usize) -> bool {
        self.scene(scene_id).is_some_and(|(_, _, protect)| protect.contains(&(x as i32, y as i32)))
    }
}

/// Sprinklers in the morning (M5): the soil their area covers is watered, as rain waters it.
pub(crate) fn water_areas(ctx: &EngineContext, state: &mut GameState, cover: &AreaCover) {
    for (scene_id, water, _) in &cover.scenes {
        if water.is_empty() {
            continue;
        }
        let day = state.clock.day;
        let Some(scene) = state.world.scenes.iter_mut().find(|scene| &scene.id == scene_id) else { continue };
        for &(x, y) in water {
            let Some(tile) = scene.tile_mut(x, y) else { continue };
            if !ctx.plantable(&tile.background) {
                continue;
            }
            tile.soil_moisture = 100;
            if tile.soil_state.as_deref() != Some(soil_states::FERTILIZED) {
                tile.soil_state = Some(soil_states::WATERED.to_owned());
            }
            if let Some(crop) = tile.crop.as_mut().filter(|crop| crop.withered != Some(true)) {
                crop.watered = true;
                crop.last_watered_day = Some(day);
                crop.days_without_water = 0;
            }
        }
    }
}

/// Whether the object's contents keep it from being picked up.
pub fn holds_items(machine: &TileMachine) -> bool {
    !crate::schema::storage_is_empty(&machine.contents)
}

/// The contents of a slot list as `(item id, quantity)` pairs (summary text, tests).
pub fn summarize(contents: &[InventorySlot]) -> Vec<(String, u32)> {
    contents.iter().map(|slot| (slot.item.id.clone(), slot.quantity)).collect()
}
