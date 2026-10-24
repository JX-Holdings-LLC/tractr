//! Long-term goals (M6): the shipping bin, collections, bundles and achievements.
//!
//! - **Shipping**: what a shipping bin (a placed object with `shipping`) holds is sold overnight
//!   at the item's value for its quality; the game keeps a log of the units shipped per item and
//!   of last night's shipment.
//! - **Collections** fill as the player finds, ships or donates their items, giving their
//!   rewards at their counts.
//! - **Bundles** take items handed in line by line; complete, they run their outcomes.
//! - **Achievements** unlock the moment their conditions hold.

use crate::effects::{message_levels, Effect};
use crate::engine_types::{Effects, EngineContext};
use crate::events::{self, ConditionScope};
use crate::messages;
use crate::schema::{
    collection_kinds, event_triggers, BundleProgress, CollectionDefinition, GameState, InventorySlot, ShippedLine,
};
use crate::{economy, inventory, substitution};

/// The collection `id`.
pub fn collection<'a>(ctx: &'a EngineContext, id: &str) -> Option<&'a CollectionDefinition> {
    ctx.content.collections.iter().find(|collection| collection.id == id)
}

/// How many items collection `id` holds.
pub fn collected(state: &GameState, id: &str) -> usize {
    state.collections.get(id).map_or(0, |progress| progress.items.len())
}

/// Whether collection `id` holds every one of its items.
pub fn collection_complete(ctx: &EngineContext, state: &GameState, id: &str) -> bool {
    collection(ctx, id).is_some_and(|definition| collected(state, id) >= definition.items.len())
}

/// Units shipped of `item_id` (empty: of everything).
pub fn shipped(state: &GameState, item_id: &str) -> u64 {
    if item_id.is_empty() {
        state.shipped.values().map(|count| u64::from(*count)).sum()
    } else {
        state.shipped.get(item_id).map_or(0, |count| u64::from(*count))
    }
}

/// The night's shipment: every shipping bin is emptied and what it held is paid for.
pub fn ship_overnight(ctx: &EngineContext, state: &mut GameState) -> Effects {
    let mut lines: Vec<ShippedLine> = Vec::new();
    let mut sold: Vec<InventorySlot> = Vec::new();
    for scene in &mut state.world.scenes {
        for row in &mut scene.tiles {
            for tile in row.iter_mut() {
                let Some(machine) = tile.machine.as_mut() else { continue };
                if machine.contents.is_empty() || !ctx.machine_type(&machine.type_id).is_some_and(|kind| kind.shipping)
                {
                    continue;
                }
                sold.append(&mut machine.contents);
            }
        }
    }
    if sold.is_empty() {
        state.last_shipment.clear();
        return Vec::new();
    }
    let mut total: i64 = 0;
    let mut units: u32 = 0;
    for slot in &sold {
        // What the item is worth now (its current definition), at its quality.
        let item = ctx.item(&slot.item.id).unwrap_or(&slot.item);
        let money = economy::quality_value(&ctx.content, item, slot.quality.as_deref())
            .max(0)
            .saturating_mul(i64::from(slot.quantity));
        total = total.saturating_add(money);
        units = units.saturating_add(slot.quantity);
        let count = state.shipped.entry(slot.item.id.clone()).or_insert(0);
        *count = count.saturating_add(slot.quantity);
        match lines.iter_mut().find(|line| line.item_id == slot.item.id && line.quality == slot.quality) {
            Some(line) => {
                line.quantity = line.quantity.saturating_add(slot.quantity);
                line.money = line.money.saturating_add(money);
            }
            None => lines.push(ShippedLine {
                item_id: slot.item.id.clone(),
                quality: slot.quality.clone(),
                quantity: slot.quantity,
                money,
            }),
        }
    }
    state.player.money = state.player.money.saturating_add(total);
    let mut effects = vec![Effect::say(message_levels::SUCCESS, messages::SHIPPED.with(&[&units, &total]))];
    let item_ids: Vec<String> = lines.iter().map(|line| line.item_id.clone()).collect();
    state.last_shipment = lines;
    for item_id in &item_ids {
        effects.extend(note(ctx, state, collection_kinds::SHIPPED, item_id));
    }
    let subjects: Vec<&str> = item_ids.iter().map(String::as_str).collect();
    effects.extend(events::fire_trigger(ctx, state, &events::Trigger::about(event_triggers::SELL, &subjects)));
    effects
}

/// Puts `item_id` into every collection of `kind` that wants it and lacks it, giving the
/// rewards its new count reaches.
pub fn note(ctx: &EngineContext, state: &mut GameState, kind: &str, item_id: &str) -> Effects {
    let mut effects = Vec::new();
    for definition in &ctx.content.collections {
        if definition.kind != kind || !definition.items.iter().any(|id| id == item_id) {
            continue;
        }
        let progress = state.collections.entry(definition.id.clone()).or_default();
        if progress.items.iter().any(|id| id == item_id) {
            continue;
        }
        progress.items.push(item_id.to_owned());
        let name = ctx.item(item_id).map_or(item_id, |item| item.name.as_str());
        effects.push(Effect::say(message_levels::SUCCESS, messages::COLLECTION_ADDED.with(&[&name, &definition.name])));
        effects.extend(reward(ctx, state, definition));
    }
    effects
}

/// Gives the rewards of `definition` that its count has reached and that were not given yet.
fn reward(ctx: &EngineContext, state: &mut GameState, definition: &CollectionDefinition) -> Effects {
    let mut effects = Vec::new();
    loop {
        let progress = state.collections.get(&definition.id).cloned().unwrap_or_default();
        let Some(next) = definition.rewards.get(progress.rewarded as usize) else { break };
        if progress.items.len() < next.count as usize {
            break;
        }
        if let Some(entry) = state.collections.get_mut(&definition.id) {
            entry.rewarded = entry.rewarded.saturating_add(1);
        }
        if !next.message.is_empty() {
            let text = substitution::fill(&ctx.content, state, &next.message).into_owned();
            effects.push(Effect::message(message_levels::SUCCESS, text));
        }
        effects.extend(events::apply_outcomes(ctx, state, &next.outcomes, 0));
    }
    effects
}

/// Whether some collection fills as items are found (the inventory is looked at after each step).
fn finds_items(ctx: &EngineContext) -> bool {
    ctx.content.collections.iter().any(|definition| definition.kind == collection_kinds::FOUND)
}

/// After every command and tick: items held for the first time join their collections, and
/// achievements whose conditions now hold unlock. Nothing happens in a game without either.
pub fn after_step(ctx: &EngineContext, state: &mut GameState) -> Effects {
    let mut effects = Vec::new();
    if finds_items(ctx) {
        let held: Vec<String> = state.player.inventory.iter().map(|slot| slot.item.id.clone()).collect();
        for item_id in held {
            let wanted = ctx.content.collections.iter().any(|definition| {
                definition.kind == collection_kinds::FOUND
                    && definition.items.contains(&item_id)
                    && !state.collections.get(&definition.id).is_some_and(|progress| progress.items.contains(&item_id))
            });
            if wanted {
                effects.extend(note(ctx, state, collection_kinds::FOUND, &item_id));
            }
        }
    }
    if !ctx.content.achievements.is_empty() {
        effects.extend(check_achievements(ctx, state));
    }
    effects
}

/// Unlocks every achievement whose conditions hold (one with none never unlocks by itself).
pub fn check_achievements(ctx: &EngineContext, state: &mut GameState) -> Effects {
    let mut effects = Vec::new();
    for achievement in &ctx.content.achievements {
        if achievement.conditions.is_empty() || state.achievements.contains_key(&achievement.id) {
            continue;
        }
        let scope = ConditionScope { salt: &achievement.id, ..ConditionScope::default() };
        if !events::conditions_hold(ctx, state, &achievement.conditions, &scope) {
            continue;
        }
        state.achievements.insert(achievement.id.clone(), state.clock.day);
        effects.push(Effect::AchievementUnlocked { achievement_id: achievement.id.clone() });
        effects.push(Effect::say(message_levels::SUCCESS, messages::ACHIEVEMENT_UNLOCKED.with(&[&achievement.name])));
        effects.extend(events::fire_trigger(
            ctx,
            state,
            &events::Trigger::about(event_triggers::ACHIEVEMENT, &[achievement.id.as_str()]),
        ));
    }
    effects
}

/// Whether the player stands where `scene_id` (absent: anywhere) wants hand-ins.
fn at_place(state: &GameState, scene_id: Option<&str>) -> bool {
    scene_id.filter(|id| !id.is_empty()).is_none_or(|id| state.player.scene_id == id)
}

/// Donate `item_id` to collection `collection_id`: one is taken from the inventory.
pub fn handle_donate(ctx: &EngineContext, state: &mut GameState, collection_id: &str, item_id: &str) -> Effects {
    let Some(definition) = collection(ctx, collection_id) else { return Vec::new() };
    if definition.kind != collection_kinds::DONATED || !definition.items.iter().any(|id| id == item_id) {
        return vec![Effect::say(message_levels::ERROR, messages::NOT_WANTED.with(&[&definition.name]))];
    }
    if state.collections.get(collection_id).is_some_and(|progress| progress.items.iter().any(|id| id == item_id)) {
        return vec![Effect::say(message_levels::INFO, &messages::ALREADY_DONATED)];
    }
    if !at_place(state, definition.scene_id.as_deref()) {
        return vec![Effect::say(message_levels::INFO, messages::HAND_IN_THERE.with(&[&definition.name]))];
    }
    if inventory::count_item(&state.player.inventory, item_id) == 0 {
        return vec![Effect::say(message_levels::ERROR, &messages::DONT_HAVE_ITEM)];
    }
    state.player.inventory = inventory::remove_item(&state.player.inventory, item_id, 1);
    note(ctx, state, collection_kinds::DONATED, item_id)
}

/// The rank of a crop quality id (its place among the game's tiers; unknown and normal: 0).
fn quality_rank(ctx: &EngineContext, quality: Option<&str>) -> usize {
    quality.map_or(0, |quality| ctx.crop_qualities().iter().position(|tier| tier.id == quality).unwrap_or(0))
}

/// Hand in what line `line` of bundle `bundle_id` still needs, of its quality or better (lowest
/// qualifying quality first). The bundle completes once enough lines are full.
pub fn handle_hand_in(ctx: &EngineContext, state: &mut GameState, bundle_id: &str, line: i32) -> Effects {
    let Some(bundle) = ctx.content.bundles.iter().find(|bundle| bundle.id == bundle_id) else { return Vec::new() };
    let Some(wanted) = usize::try_from(line).ok().and_then(|line| bundle.items.get(line)) else { return Vec::new() };
    let line = line as usize;
    let mut progress = state.bundles.get(bundle_id).cloned().unwrap_or_default();
    if progress.complete {
        return vec![Effect::say(message_levels::INFO, &messages::BUNDLE_DONE_ALREADY)];
    }
    if !at_place(state, bundle.scene_id.as_deref()) {
        return vec![Effect::say(message_levels::INFO, messages::HAND_IN_THERE.with(&[&bundle.name]))];
    }
    progress.handed.resize(bundle.items.len(), 0);
    let need = wanted.quantity.saturating_sub(progress.handed[line]);
    if need == 0 {
        return Vec::new();
    }
    let floor = quality_rank(ctx, wanted.quality.as_deref());
    let mut qualities: Vec<Option<String>> = Vec::new();
    for slot in state.player.inventory.iter().filter(|slot| slot.item.id == wanted.item_id) {
        if quality_rank(ctx, slot.quality.as_deref()) >= floor && !qualities.contains(&slot.quality) {
            qualities.push(slot.quality.clone());
        }
    }
    qualities.sort_by_key(|quality| quality_rank(ctx, quality.as_deref()));
    let mut inventory = state.player.inventory.clone();
    let mut taken = 0;
    for quality in &qualities {
        let held = inventory::count_item_with_quality(&inventory, &wanted.item_id, quality.as_deref());
        let take = u32::try_from(held).unwrap_or(u32::MAX).min(need - taken);
        inventory = inventory::remove_item_with_quality(&inventory, &wanted.item_id, quality.as_deref(), take);
        taken += take;
        if taken == need {
            break;
        }
    }
    if taken == 0 {
        return vec![Effect::say(message_levels::ERROR, &messages::NOTHING_TO_HAND_IN)];
    }
    state.player.inventory = inventory;
    progress.handed[line] = progress.handed[line].saturating_add(taken);
    let full = bundle.items.iter().zip(&progress.handed).filter(|(item, handed)| **handed >= item.quantity).count();
    progress.complete = full >= bundle.lines_needed();
    let complete = progress.complete;
    state.bundles.insert(bundle_id.to_owned(), progress);
    let name = ctx.item(&wanted.item_id).map_or(wanted.item_id.as_str(), |item| item.name.as_str());
    let mut effects = vec![Effect::say(message_levels::SUCCESS, messages::HANDED_IN.with(&[&taken, &name]))];
    if complete {
        effects.push(if bundle.message.is_empty() {
            Effect::say(message_levels::SUCCESS, messages::BUNDLE_COMPLETE.with(&[&bundle.name]))
        } else {
            Effect::message(
                message_levels::SUCCESS,
                substitution::fill(&ctx.content, state, &bundle.message).into_owned(),
            )
        });
        effects.extend(events::apply_outcomes(ctx, state, &bundle.outcomes, 0));
        effects.extend(events::fire_trigger(ctx, state, &events::Trigger::about(event_triggers::BUNDLE, &[bundle_id])));
    }
    effects
}

/// Whether bundle `id` is complete.
pub fn bundle_complete(state: &GameState, id: &str) -> bool {
    state.bundles.get(id).is_some_and(|progress: &BundleProgress| progress.complete)
}
