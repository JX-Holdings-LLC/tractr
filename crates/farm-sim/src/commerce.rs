//! Optional opening rules and deterministic, saved shop pricing.
use crate::{
    events, game_time,
    schema::{GameState, ShopDefinition},
    units, EngineContext,
};

/// Hours use [open, close); overnight ranges are allowed. A keeper must be in the
/// player's scene. Every transaction rechecks this, including a shop left open overnight.
pub fn shop_is_open(ctx: &EngineContext, state: &GameState, shop: &ShopDefinition) -> bool {
    let festival_shops: Vec<_> = ctx
        .content
        .settings
        .calendar
        .festivals
        .iter()
        .filter(|festival| festival.program.as_ref().is_some_and(|p| p.shop_id.as_deref() == Some(&shop.id)))
        .collect();
    if !festival_shops.is_empty()
        && !festival_shops
            .iter()
            .any(|festival| state.festival.as_ref().is_some_and(|session| session.festival_id == festival.id))
    {
        return false;
    }
    if let Some(hours) = &shop.hours {
        let minute = state.clock.time_minutes / units::MINUTE;
        let open = if hours.open < hours.close {
            minute >= hours.open && minute < hours.close
        } else if hours.open > hours.close {
            minute >= hours.open || minute < hours.close
        } else {
            false
        };
        if !open {
            return false;
        }
    }
    if let Some(closed) = &shop.closed_on {
        if closed.seasons.contains(&state.clock.season)
            || game_time::weekday(&ctx.content.settings.calendar, state.clock.day)
                .is_some_and(|day| closed.weekdays.contains(&day.id))
            || game_time::festival_today(&ctx.content.settings.calendar, &state.clock)
                .is_some_and(|festival| closed.festivals.contains(&festival.id))
        {
            return false;
        }
    }
    shop.keeper_npc_id
        .as_ref()
        .is_none_or(|id| state.npcs.get(id).is_some_and(|npc| npc.scene_id == state.player.scene_id))
}

/// Price modifiers multiply in authored order. A sale uses the quoted price for its
/// whole quantity; saturation changes the next transaction, and expires after N days.
pub fn price(
    ctx: &EngineContext,
    state: &GameState,
    shop: &ShopDefinition,
    item_id: &str,
    base: i64,
    selling: bool,
) -> i64 {
    let mut price = base.max(0);
    for modifier in shop.price_modifiers.iter().flatten() {
        if modifier.item_id.as_ref().is_some_and(|id| id != item_id)
            || (!modifier.seasons.is_empty() && !modifier.seasons.contains(&state.clock.season))
            || (!modifier.weekdays.is_empty()
                && !game_time::weekday(&ctx.content.settings.calendar, state.clock.day)
                    .is_some_and(|day| modifier.weekdays.contains(&day.id)))
            || !events::conditions_hold(
                ctx,
                state,
                &modifier.conditions,
                &events::ConditionScope { salt: &shop.id, ..Default::default() },
            )
        {
            continue;
        }
        let multiplier =
            if selling { modifier.sell_multiplier } else { modifier.buy_multiplier }.unwrap_or(units::MILLI_ONE);
        price = price.saturating_mul(i64::from(multiplier)) / i64::from(units::MILLI_ONE);
    }
    if selling {
        if let Some(rule) = &shop.saturation {
            let sold = state
                .shop_sales
                .get(&shop.id)
                .and_then(|items| items.get(item_id))
                .map(|days| {
                    days.iter()
                        .filter(|(day, _)| state.clock.day.saturating_sub(**day) < rule.recovery_days)
                        .fold(0u64, |sum, (_, quantity)| sum.saturating_add(u64::from(*quantity)))
                })
                .unwrap_or(0);
            let reduction =
                (sold / u64::from(rule.quantity_per_step.max(1))).saturating_mul(u64::from(rule.reduction_per_step));
            let multiplier = u64::from(units::MILLI_ONE)
                .saturating_sub(reduction)
                .max(u64::from(rule.minimum_multiplier.min(units::MILLI_ONE)));
            price = price.saturating_mul(multiplier as i64) / i64::from(units::MILLI_ONE);
        }
    }
    price
}

/// Record only shops that use saturation; legacy saves stay byte-for-byte unchanged.
pub fn record_sale(state: &mut GameState, shop: &ShopDefinition, item_id: &str, quantity: u32) {
    let Some(rule) = &shop.saturation else { return };
    let days = state.shop_sales.entry(shop.id.clone()).or_default().entry(item_id.to_owned()).or_default();
    days.retain(|day, _| state.clock.day.saturating_sub(*day) < rule.recovery_days);
    let sold = days.entry(state.clock.day).or_default();
    *sold = sold.saturating_add(quantity);
}

/// Forget expired sales even when the player never sells the item again.
pub fn expire_sales(ctx: &EngineContext, state: &mut GameState) {
    let day = state.clock.day;
    state.shop_sales.retain(|shop_id, items| {
        let Some(rule) = ctx.shop(shop_id).and_then(|shop| shop.saturation.as_ref()) else { return false };
        items.retain(|_, days| {
            days.retain(|sold_day, _| day.saturating_sub(*sold_day) < rule.recovery_days);
            !days.is_empty()
        });
        !items.is_empty()
    });
}
