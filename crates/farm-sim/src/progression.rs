//! Permanent player upgrades. Never discard occupied inventory slots when shrinking a bag.
use crate::engine_types::EngineContext;
use crate::schema::GameState;
use crate::units;

/// Apply a permanent upgrade in authoring units, bounded by the project's limits.
/// Existing player values above a newly lowered cap are preserved.
pub fn apply(ctx: &EngineContext, state: &mut GameState, kind: &str, amount: i64) {
    let limits = ctx.content.settings.progression.clone().unwrap_or_default();
    match kind {
        "setInventorySize" | "addInventorySize" => {
            let old = state.player.max_inventory_size;
            let desired = if kind == "setInventorySize" { amount } else { i64::from(old).saturating_add(amount) };
            let occupied = u32::try_from(state.player.inventory.len()).unwrap_or(u32::MAX);
            let cap = limits.max_inventory_size.min(1024).max(old).max(occupied);
            state.player.max_inventory_size =
                u32::try_from(desired.clamp(i64::from(occupied), i64::from(cap))).unwrap_or(cap);
        }
        "addMaxEnergy" => {
            let old = state.player.max_energy;
            let cap = limits.max_energy.max(old).max(1);
            let desired = i64::from(old).saturating_add(amount.saturating_mul(i64::from(units::ENERGY_POINT)));
            state.player.max_energy = desired.clamp(1, i64::from(cap)) as i32;
            state.player.energy = state.player.energy.min(crate::buffs::max_energy(ctx, state));
        }
        "addMaxHealth" => {
            let Some(base) = crate::combat::max_health(&ctx.content.settings) else { return };
            let old = state.player.max_health.unwrap_or(base).max(1);
            let cap = limits.max_health.max(old).max(1);
            state.player.max_health = Some(i64::from(old).saturating_add(amount).clamp(1, i64::from(cap)) as i32);
            let max = crate::combat::player_max_health(ctx, state).unwrap_or(old);
            state.player.health = state.player.health.map(|health| health.min(max));
        }
        _ => {}
    }
}

pub const UPGRADES: &[&str] = &["setInventorySize", "addInventorySize", "addMaxEnergy", "addMaxHealth"];

/// Hand the tool to the current shop, charging money/materials only after every check passes.
pub fn order_tool(ctx: &EngineContext, state: &mut GameState, item_id: &str) -> crate::engine_types::Effects {
    let Some(session) = &state.shop else { return vec![] };
    let Some(shop) = crate::economy::find_shop(ctx, state, &session.shop_id) else { return vec![] };
    if !crate::commerce::shop_is_open(ctx, state, &shop) {
        return vec![];
    }
    let Some(rule) = shop.tool_upgrades.iter().flatten().find(|rule| rule.from_item_id == item_id) else {
        return vec![];
    };
    let Some(index) = state.player.inventory.iter().position(|slot| slot.item.id == item_id && slot.quantity == 1)
    else {
        return vec![];
    };
    let Some(target) = ctx.item(&rule.to_item_id) else { return vec![] };
    let from = &state.player.inventory[index].item;
    if from.tool_type.is_none()
        || from.tool_type != target.tool_type
        || target.tool_tier.unwrap_or(1) != from.tool_tier.unwrap_or(1).saturating_add(1)
        || rule.cost < 0
        || state.player.money < rule.cost
        || rule.nights == 0
        || state.player.tool_upgrades.len() >= 32
    {
        return vec![];
    }
    let mut required = std::collections::BTreeMap::<&str, u32>::new();
    for ingredient in &rule.materials {
        if ingredient.item_id == item_id || ingredient.quantity == 0 {
            return vec![];
        }
        let quantity = required.entry(&ingredient.item_id).or_default();
        *quantity = quantity.saturating_add(ingredient.quantity);
    }
    for (id, count) in &required {
        let held = state
            .player
            .inventory
            .iter()
            .filter(|slot| slot.item.id == *id)
            .fold(0_u32, |sum, slot| sum.saturating_add(slot.quantity));
        if held < *count {
            return vec![];
        }
    }
    let pending = crate::schema::PendingToolUpgrade {
        shop_id: shop.id.clone(),
        item: target.clone(),
        slot: index as u32,
        ready_day: state.clock.day.saturating_add(rule.nights),
        was_equipped: state.player.equipped_tool.as_deref() == Some(item_id),
    };
    state.player.money = state.player.money.saturating_sub(rule.cost);
    state.player.inventory.remove(index);
    for (id, count) in required {
        state.player.inventory = crate::inventory::remove_item(&state.player.inventory, id, count);
    }
    if state.player.equipped_tool.as_deref() == Some(item_id) {
        state.player.equipped_tool = None;
    }
    state.player.tool_upgrades.push(pending);
    vec![]
}

/// Collect a finished tool at its original shop; a full inventory leaves the order safe there.
pub fn collect_tool(ctx: &EngineContext, state: &mut GameState, item_id: &str) -> crate::engine_types::Effects {
    let Some(session) = &state.shop else { return vec![] };
    let Some(shop) = crate::economy::find_shop(ctx, state, &session.shop_id) else { return vec![] };
    if !crate::commerce::shop_is_open(ctx, state, &shop) {
        return vec![];
    }
    let Some(index) = state.player.tool_upgrades.iter().position(|order| {
        order.shop_id == session.shop_id && order.item.id == item_id && order.ready_day <= state.clock.day
    }) else {
        return vec![];
    };
    if state.player.inventory.len() >= state.player.max_inventory_size as usize {
        return vec![];
    }
    let order = state.player.tool_upgrades.remove(index);
    let slot = (order.slot as usize).min(state.player.inventory.len());
    if order.was_equipped && state.player.equipped_tool.is_none() {
        state.player.equipped_tool = Some(order.item.id.clone());
    }
    state
        .player
        .inventory
        .insert(slot, crate::schema::InventorySlot { item: order.item, quantity: 1, ..Default::default() });
    vec![]
}

/// Unambiguous key for a one-time stock upgrade, including namespaced ids.
pub fn purchase_key(shop: &str, upgrade: &str) -> String {
    format!("{}:{shop}{upgrade}", shop.len())
}
