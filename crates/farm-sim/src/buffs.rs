//! Timed, saved stat modifiers with integer arithmetic and bounded active state.
use crate::engine_types::EngineContext;
use crate::schema::{ActiveBuff, BuffDefinition, GameState};
use crate::units;

pub const STATS: &[&str] = &[
    "craftingSpeed",
    "sellPrice",
    "maxHealth",
    "moveSpeed",
    "toolEnergyCost",
    "luck",
    "skillLevel",
    "maxEnergy",
    "attack",
    "defense",
    "fishingEscape",
    "quality",
    "yield",
];

pub fn now(ctx: &EngineContext, state: &GameState) -> i64 {
    crate::crafting::absolute_minute_in(ctx.content.settings.time.minutes_per_day(), state)
}

pub fn apply(ctx: &EngineContext, state: &mut GameState, definition: &BuffDefinition, source: &str) {
    if definition.id.is_empty()
        || definition.id.len() > 128
        || source.len() > 256
        || !STATS.contains(&definition.stat.as_str())
        || definition.duration_minutes == 0
    {
        return;
    }
    let now = now(ctx, state);
    state.player.buffs.retain(|buff| buff.expires_at_minute > now);
    let expires_at_minute =
        now.saturating_add(i64::from(definition.duration_minutes.min(525_600)) * i64::from(units::MINUTE));
    let replacement = ActiveBuff { definition: definition.clone(), source: source.to_owned(), expires_at_minute };
    // A repeated buff id refreshes its duration even when it came from another item.
    if let Some(active) = state.player.buffs.iter_mut().find(|buff| buff.definition.id == definition.id) {
        *active = replacement;
    } else if state.player.buffs.len() < 64 {
        state.player.buffs.push(replacement);
    }
}

pub fn stat_bonus(ctx: &EngineContext, state: &GameState, stat: &str) -> i32 {
    stat_bonus_at(state, stat, now(ctx, state))
}

pub fn stat_bonus_at(state: &GameState, stat: &str, now: i64) -> i32 {
    let mut strongest = 0_i32;
    let mut additive = 0_i32;
    for buff in &state.player.buffs {
        if buff.expires_at_minute <= now || buff.definition.stat != stat {
            continue;
        }
        let amount = buff.definition.amount;
        if buff.definition.additive {
            additive = additive.saturating_add(amount);
        } else if amount.unsigned_abs() > strongest.unsigned_abs() {
            strongest = amount;
        }
    }
    strongest.saturating_add(additive)
}

pub fn active(ctx: &EngineContext, state: &GameState, id: &str) -> bool {
    let now = now(ctx, state);
    state.player.buffs.iter().any(|buff| buff.definition.id == id && buff.expires_at_minute > now)
}

pub fn max_energy(ctx: &EngineContext, state: &GameState) -> i32 {
    let bonus = crate::skills::stat_bonus(ctx, state, "maxEnergy");
    state.player.max_energy.saturating_add(bonus.saturating_mul(units::ENERGY_POINT)).max(1)
}

/// Adjust weighted rewards without drawing randomness. Positive luck flattens relative
/// rarity; negative luck sharpens it. Disabled entries stay disabled and zero luck is exact.
pub fn luck_weights(weights: &mut [u32], luck: i32) {
    let luck = luck.clamp(-100, 100);
    let largest = weights.iter().copied().max().unwrap_or(0);
    if luck == 0 || largest == 0 {
        return;
    }
    for weight in weights.iter_mut().filter(|weight| **weight > 0) {
        *weight = if luck > 0 {
            (i64::from(*weight) + i64::from(largest) * i64::from(luck) / 100).min(i64::from(u32::MAX)) as u32
        } else {
            let factor = i64::from(largest) + i64::from(largest - *weight) * i64::from(luck) / 100;
            (u64::from(*weight) * factor as u64 / u64::from(largest)).max(1) as u32
        };
    }
}

#[cfg(test)]
mod tests {
    use super::luck_weights;

    #[test]
    fn luck_preserves_disabled_rewards_and_handles_extreme_weights() {
        for luck in [i32::MIN, -100, 0, 100, i32::MAX] {
            let mut weights = [0, 1, u32::MAX];
            luck_weights(&mut weights, luck);
            assert_eq!(weights[0], 0);
            assert!(weights[1] > 0 && weights[2] > 0);
            if luck == 0 {
                assert_eq!(weights, [0, 1, u32::MAX]);
            }
            let mut empty = [0, 0];
            luck_weights(&mut empty, luck);
            assert_eq!(empty, [0, 0]);
        }
    }
}
