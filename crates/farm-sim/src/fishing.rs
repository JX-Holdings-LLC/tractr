//! Fishing (port of `Fishing.cs` / fishing.ts).
//!
//! Fishing (M4e): cast at water, resolve deterministically through the seeded RNG. Fish tables
//! filter by scene and season; junk rolls first; per-fish difficulty is offset by rod tier. (The
//! timing minigame is a shell-side flourish planned for M7 — it will resolve through this same
//! deterministic path.)

use crate::effects::{message_levels, Effect};
use crate::engine_types::{Effects, EngineContext};
use crate::inventory;
use crate::messages;
use crate::quests;
use crate::rng::Rng;
use crate::schema::{FishTable, GameState};
use crate::skills;
use crate::units;

/// TS `FishingResult` minus the state (updated in place).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FishingResult {
    pub effects: Effects,
    pub caught: bool,
}

/// Whether a data-driven fish filter matches the current clock and event state.
pub fn filter_matches(ctx: &EngineContext, state: &GameState, filter: Option<&crate::schema::FishingFilter>) -> bool {
    filter.is_none_or(|f| {
        let minute = units::whole_minute(state.clock.time_minutes);
        crate::foraging::in_season(f.seasons.as_deref(), &state.clock.season)
            && f.weather_ids.as_ref().is_none_or(|ids| ids.is_empty() || ids.contains(&state.clock.weather_id))
            && f.start_minute.is_none_or(|start| minute >= start)
            && f.end_minute.is_none_or(|end| minute <= end)
            && f.condition.as_ref().is_none_or(|condition| crate::events::condition_met(ctx, state, condition, None))
    })
}

fn table_matches(ctx: &EngineContext, state: &GameState, table: &FishTable) -> bool {
    table.scene_ids.as_ref().is_none_or(|ids| ids.is_empty() || ids.contains(&state.player.scene_id))
        && crate::foraging::in_season(table.seasons.as_deref(), &state.clock.season)
        && filter_matches(ctx, state, table.filter.as_ref())
        && table
            .entries
            .iter()
            .any(|entry| filter_matches(ctx, state, entry.rules.as_ref().and_then(|r| r.filter.as_ref())))
}

/// First matching table, including scene, season, time, weather and event filters.
pub fn active_fish_table<'a>(ctx: &'a EngineContext, state: &GameState) -> Option<&'a FishTable> {
    ctx.content.fish_tables.iter().find(|table| table_matches(ctx, state, table))
}

fn gear(state: &GameState, kind: &str) -> Option<(String, crate::schema::FishingGear)> {
    state.player.inventory.iter().filter(|slot| slot.quantity > 0).find_map(|slot| {
        slot.item
            .fishing_gear
            .as_ref()
            .filter(|gear| gear.kind == kind)
            .map(|gear| (slot.item.id.clone(), gear.clone()))
    })
}

/// Resolve a cast into water (caller already validated rod + water tile).
/// `score` (0–1, from the fishing minigame when one is declared)
/// reduces the escape chance: a perfect score always lands the fish.
///
/// TS `resolveFishing(ctx, state, rodTier, options?)`; `score` is `options.score`.
/// The chance a hooked fish escapes: `max(0, difficulty − 0.15 × (rodTier − 1)) × (1 − score)`,
/// as an exact fraction (numerator, denominator) of 2³² units: each rod tier above 1 takes 15%
/// off the difficulty, and a minigame score (0–1) scales what is left.
pub fn escape_chance(difficulty: u64, rod_tier: i32, score: Option<u64>) -> (u128, u128) {
    let one = i128::from(units::PROBABILITY_ONE);
    // 20 × (difficulty − 3/20 × (tier − 1) × 2³²)
    let scaled = (20 * i128::from(difficulty) - 3 * (i128::from(rod_tier) - 1) * one).max(0) as u128;
    let skill = u128::from(units::PROBABILITY_ONE - score.unwrap_or(0).min(units::PROBABILITY_ONE));
    (scaled * skill, 20 * units::PROBABILITY_ONE as u128)
}

/// Resolve a cast with a rod of `rod_tier`: roll the catch from the active fish table (junk, or
/// a fish that may escape; a minigame `score` lowers the escape chance), add it to the inventory
/// and grant fishing XP.
pub fn resolve_fishing(ctx: &EngineContext, state: &mut GameState, rod_tier: i32, score: Option<u64>) -> FishingResult {
    let Some(table) = active_fish_table(ctx, state) else {
        return FishingResult {
            effects: vec![Effect::say(message_levels::INFO, &messages::WATER_QUIET)],
            caught: false,
        };
    };

    let mut rng = Rng::new(state.rng.clone());
    let mut effects = Vec::new();
    let mut caught = false;

    // Junk first, then the weighted fish roll, then the escape check.
    // (The float is drawn only when junkChance > 0; `&&` short-circuits.)
    if table.junk_chance > 0
        && rng.chance(table.junk_chance)
        && table.junk_item_id.as_deref().is_some_and(|id| !id.is_empty())
    {
        let junk = table.junk_item_id.as_deref().and_then(|id| ctx.item(id));
        state.rng = rng.state;
        if let Some(junk) = junk {
            let added = inventory::add_item(&state.player.inventory, junk, 1, state.player.max_inventory_size, None);
            if added.added {
                state.player.inventory = added.inventory;
                effects.push(Effect::say(message_levels::INFO, messages::FISHED_UP.with(&[&junk.name])));
            } else {
                effects.push(Effect::say(message_levels::ERROR, &messages::INVENTORY_FULL));
            }
        }
        return FishingResult { effects, caught: false };
    }

    let entries: Vec<_> = ctx
        .content
        .fish_tables
        .iter()
        .filter(|candidate| {
            candidate.id == table.id || (table.layered == Some(true) && table_matches(ctx, state, candidate))
        })
        .flat_map(|table| table.entries.iter().map(move |entry| (table, entry)))
        .filter(|(_, entry)| filter_matches(ctx, state, entry.rules.as_ref().and_then(|r| r.filter.as_ref())))
        .collect();
    let bait = gear(state, "bait");
    let tackle = gear(state, "tackle");
    let mut weights: Vec<u32> = entries
        .iter()
        .map(|(_, entry)| {
            let bonus = bait
                .as_ref()
                .filter(|(_, gear)| gear.fish_ids.is_empty() || gear.fish_ids.contains(&entry.item_id))
                .map_or(0, |(_, gear)| gear.weight_bonus);
            entry.weight.saturating_add(bonus)
        })
        .collect();
    crate::buffs::luck_weights(&mut weights, skills::stat_bonus(ctx, state, "luck"));
    let Some(index) = rng.weighted(&weights) else {
        state.rng = rng.state;
        return FishingResult {
            effects: vec![Effect::say(message_levels::INFO, &messages::NOT_A_NIBBLE)],
            caught: false,
        };
    };
    let (table, entry) = entries[index];

    // Escape roll: difficulty reduced 15% per rod tier above 1, then scaled
    // down by minigame skill (score 1 → no escape chance at all).
    let reduction = (i64::from(tackle.as_ref().map_or(0, |(_, gear)| gear.escape_reduction))
        + i64::from(skills::stat_bonus(ctx, state, "fishingEscape")))
    .clamp(-100, 100);
    let difficulty = (i128::from(entry.difficulty) - i128::from(units::PROBABILITY_ONE) * i128::from(reduction) / 100)
        .clamp(0, i128::from(units::PROBABILITY_ONE)) as u64;
    if rng.chance_below(escape_chance(difficulty, rod_tier, score)) {
        state.rng = rng.state;
        return FishingResult { effects: vec![Effect::say(message_levels::INFO, &messages::GOT_AWAY)], caught: false };
    }

    state.rng = rng.state;
    let fish = ctx.item(&entry.item_id);
    if let Some(fish) = fish {
        let quality = entry.rules.as_ref().filter(|r| r.quality == Some(true)).map(|_| {
            let score_bonus = (score.unwrap_or(0).min(units::PROBABILITY_ONE) * 2 / units::PROBABILITY_ONE) as u32;
            let bonus = i64::from(score_bonus)
                + i64::from(tackle.as_ref().map_or(0, |(_, gear)| gear.quality_bonus))
                + i64::from(skills::stat_bonus(ctx, state, "quality"));
            let quality_level = (i64::from(skills::effective_level(ctx, state, "fishing")) + bonus * 3)
                .clamp(0, i64::from(u32::MAX)) as u32;
            crate::farming::crops::harvest_quality(ctx.crop_qualities(), "normal", quality_level)
        });
        let added = inventory::add_item_with_quality(
            &state.player.inventory,
            fish,
            quality.as_deref(),
            1,
            state.player.max_inventory_size,
            None,
        );
        if added.added {
            state.player.inventory = added.inventory;
            effects.push(Effect::say(message_levels::SUCCESS, messages::CAUGHT_FISH.with(&[&fish.name])));
            caught = true;
        } else {
            effects.push(Effect::say(message_levels::ERROR, &messages::INVENTORY_FULL));
        }
    }

    if caught {
        for (item_id, _) in bait.into_iter().chain(tackle) {
            state.player.inventory = inventory::remove_item(&state.player.inventory, &item_id, 1);
        }
        if let Some(rules) = &entry.rules {
            if let Some(minimum) = rules.min_size {
                let maximum = rules.max_size.unwrap_or(minimum).max(minimum);
                let mut rng = Rng::new(state.rng.clone());
                let size = rng.int(i64::from(minimum), i64::from(maximum)) as u32;
                state.rng = rng.state;
                // Keep the best size per species, bounded by the authored item count.
                if let Some(record) = state.fish_catches.iter_mut().find(|r| r.item_id == entry.item_id) {
                    if size > record.size {
                        record.size = size;
                        record.day = state.clock.day;
                    }
                } else {
                    state.fish_catches.push(crate::schema::FishCatchRecord {
                        item_id: entry.item_id.clone(),
                        size,
                        day: state.clock.day,
                    });
                }
            }
        }
        let xp = entry.rules.as_ref().and_then(|r| r.xp).unwrap_or(8);
        effects.extend(skills::grant_action_xp_subject(ctx, state, "catch", &entry.item_id, "fishing", xp));
        ctx.emit(crate::hooks::HookEvent::FishCatch(crate::hooks::FishCatchHookPayload {
            item_id: entry.item_id.clone(),
            fish_table_id: table.id.clone(),
        }));
        effects.extend(quests::progress_quests(ctx, state, "collect", &entry.item_id, 1));
        effects.extend(quests::progress_quests(ctx, state, "catch", &entry.item_id, 1));
        effects.extend(crate::events::fire_trigger(
            ctx,
            state,
            &crate::events::Trigger::about(crate::schema::event_triggers::CATCH, &[entry.item_id.as_str()]),
        ));
    }

    FishingResult { effects, caught }
}

/// Empty traps and ponds fill overnight. Their output is collected by the ordinary machine UI.
pub fn produce_overnight(ctx: &EngineContext, state: &mut GameState) {
    let jobs: Vec<_> = state
        .world
        .scenes
        .iter()
        .enumerate()
        .flat_map(|(scene_index, scene)| {
            scene.tiles.iter().flatten().filter_map(move |tile| {
                let machine = tile.machine.as_ref()?;
                let production =
                    ctx.content.machine_types.iter().find(|m| m.id == machine.type_id)?.fish_production.as_ref()?;
                (machine.processing.is_none() && machine.output.as_ref().is_none_or(|o| o.is_empty())).then_some((
                    scene_index,
                    tile.x,
                    tile.y,
                    production,
                ))
            })
        })
        .collect();
    let mut rng = Rng::new(state.rng.clone());
    for (scene_index, x, y, production) in jobs {
        if !state.clock.day.is_multiple_of(production.every_days.max(1)) {
            continue;
        }
        let item = if let Some(species) = &production.species_item_id {
            Some(species.clone())
        } else {
            ctx.content
                .fish_tables
                .iter()
                .find(|t| Some(&t.id) == production.table_id.as_ref())
                .filter(|table| {
                    crate::foraging::in_season(table.seasons.as_deref(), &state.clock.season)
                        && filter_matches(ctx, state, table.filter.as_ref())
                        && table
                            .scene_ids
                            .as_ref()
                            .is_none_or(|ids| ids.is_empty() || ids.contains(&state.world.scenes[scene_index].id))
                })
                .and_then(|table| {
                    let entries: Vec<_> = table
                        .entries
                        .iter()
                        .filter(|entry| {
                            filter_matches(ctx, state, entry.rules.as_ref().and_then(|r| r.filter.as_ref()))
                        })
                        .collect();
                    rng.weighted(&entries.iter().map(|e| e.weight).collect::<Vec<_>>())
                        .map(|index| entries[index].item_id.clone())
                })
        };
        if let Some(item_id) = item.filter(|id| ctx.item(id).is_some()) {
            if let Some(machine) = state.world.scenes[scene_index].tile_mut(x, y).and_then(|t| t.machine.as_mut()) {
                machine.output =
                    Some(vec![crate::schema::RecipeIngredient { item_id, quantity: production.quantity.max(1) }]);
            }
        }
    }
    state.rng = rng.state;
}
