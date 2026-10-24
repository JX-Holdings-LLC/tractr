//! Animals (port of `Animals.cs` / animals.ts).
//!
//! Animals & ranching (M4c). Interacting with an animal feeds it (if you carry its feed),
//! collects a ready product, or pets it. The nightly pass ages animals, rolls products
//! (fed + adult + interval) and adjusts mood.
//!
//! Housing (M5): animals bought from a shop live in a building that houses their species
//! (`crate::buildings`); a species may have more products than its first, each on its own
//! interval, a product's quality can follow the animal's mood, and adults in a building with
//! room may have young overnight. Animals stay where they are placed.

use crate::effects::{message_levels, Effect};
use crate::engine_types::{Effects, EngineContext};
use crate::inventory;
use crate::messages;
use crate::rng::Rng;
use crate::schema::{AnimalSpeciesDefinition, AnimalState, GameState, ProductProgress};
use crate::units;

/// The animal species `species_id`.
pub fn species_by_id<'a>(ctx: &'a EngineContext, species_id: &str) -> Option<&'a AnimalSpeciesDefinition> {
    ctx.animal_species(species_id)
}

/// The animal standing on tile `(x, y)` (animal positions are tile-aligned).
pub fn animal_at<'a>(state: &'a GameState, scene_id: &str, x: i32, y: i32) -> Option<&'a AnimalState> {
    let (x, y) = (units::tiles(x), units::tiles(y));
    state.animals.iter().find(|animal| animal.scene_id == scene_id && animal.x == x && animal.y == y)
}

/// TS `updateAnimal(state, id, patch)`: the patch is applied to every animal with that id (in place).
fn update_animal(state: &mut GameState, animal_id: &str, patch: impl Fn(&mut AnimalState)) {
    for animal in &mut state.animals {
        if animal.id == animal_id {
            patch(animal);
        }
    }
}

/// `!string.IsNullOrEmpty(species.FeedItemId)`: the feed item when the species needs one.
fn feed_item_id(species: &AnimalSpeciesDefinition) -> Option<&str> {
    species.feed_item_id.as_deref().filter(|id| !id.is_empty())
}

/// Feed → collect → pet, in that priority.
///
/// `animal` is a copy of the animal being interacted with (the C# passes the record).
pub fn handle_animal_interaction(ctx: &EngineContext, state: &mut GameState, animal: &AnimalState) -> Effects {
    let Some(species) = species_by_id(ctx, &animal.species_id) else {
        return Vec::new();
    };

    // 1. Feed (needs the species' feed item in inventory)
    if !animal.fed_today {
        if let Some(feed_item_id) = feed_item_id(species) {
            let has_feed = state.player.inventory.iter().any(|slot| slot.item.id == feed_item_id);
            if has_feed {
                let inventory = inventory::remove_item(&state.player.inventory, feed_item_id, 1);
                update_animal(state, &animal.id, |a| {
                    a.fed_today = true;
                    a.mood = 100.min(animal.mood.saturating_add(5));
                });
                state.player.inventory = inventory;
                return vec![Effect::say(message_levels::SUCCESS, messages::FED_ANIMAL.with(&[&animal.name]))];
            }
        }
    }

    // 2. Collect a ready product: the first one, then the others in order (M5), at the quality
    // the animal's mood earns.
    let quality = mood_quality(ctx, species, animal.mood);
    let ready = if animal.product_ready && !species.product_item_id.is_empty() {
        Some((None, species.product_item_id.as_str()))
    } else {
        species.products.iter().enumerate().find_map(|(index, product)| {
            animal
                .more_products
                .get(index)
                .filter(|progress| progress.ready)
                .map(|_| (Some(index), product.item_id.as_str()))
        })
    };
    if let Some((which, item_id)) = ready {
        if let Some(product) = ctx.item(item_id) {
            let added = inventory::add_item_with_quality(
                &state.player.inventory,
                product,
                quality.as_deref(),
                1,
                state.player.max_inventory_size,
                None,
            );
            if !added.added {
                return vec![Effect::say(message_levels::ERROR, &messages::INVENTORY_FULL)];
            }
            state.player.inventory = added.inventory;
            update_animal(state, &animal.id, |a| match which {
                None => {
                    a.product_ready = false;
                    a.days_since_product = 0;
                }
                Some(index) => {
                    if let Some(progress) = a.more_products.get_mut(index) {
                        *progress = ProductProgress::default();
                    }
                }
            });
            return vec![Effect::say(
                message_levels::SUCCESS,
                messages::COLLECTED_PRODUCT.with(&[&product.name, &animal.name]),
            )];
        }
    }

    // 3. Pet
    if !animal.petted_today {
        update_animal(state, &animal.id, |a| {
            a.petted_today = true;
            a.mood = 100.min(animal.mood.saturating_add(8));
        });
        return vec![Effect::say(message_levels::SUCCESS, messages::ANIMAL_HAPPY.with(&[&animal.name]))];
    }

    vec![Effect::say(message_levels::INFO, messages::ANIMAL_CONTENT.with(&[&animal.name]))]
}

/// Nightly pass for animals: age, mood, product rolls.
///
/// C# returned a new state; here the animals advance in place.
pub fn advance_animals_nightly(ctx: &EngineContext, state: &mut GameState) {
    if state.animals.is_empty() {
        return;
    }

    for animal in &mut state.animals {
        let Some(species) = species_by_id(ctx, &animal.species_id) else {
            continue;
        };

        let needs_feed = feed_item_id(species).is_some();
        let was_cared_for = !needs_feed || animal.fed_today;
        let mood_delta = (if was_cared_for { 4 } else { -15 }) + (if animal.petted_today { 4 } else { 0 });
        let age_days = animal.age_days.saturating_add(1);
        let is_adult = age_days >= species.days_to_adult;

        let mut days_since_product = animal.days_since_product;
        let mut product_ready = animal.product_ready;
        if is_adult && was_cared_for && !product_ready {
            days_since_product = days_since_product.saturating_add(1);
            if days_since_product >= species.product_interval_days && animal.mood >= 30 {
                product_ready = true;
            }
        }

        // More products (M5), each on its own interval and mood.
        if !species.products.is_empty() {
            animal.more_products.resize(species.products.len(), ProductProgress::default());
            for (product, progress) in species.products.iter().zip(animal.more_products.iter_mut()) {
                if is_adult && was_cared_for && !progress.ready {
                    progress.days = progress.days.saturating_add(1);
                    if progress.days >= product.interval_days && animal.mood >= product.min_mood {
                        progress.ready = true;
                    }
                }
            }
        }

        animal.age_days = age_days;
        animal.mood = animal.mood.saturating_add(mood_delta).clamp(0, 100);
        animal.fed_today = false;
        animal.petted_today = false;
        animal.days_since_product = days_since_product;
        animal.product_ready = product_ready;
    }
}

/// The product quality `mood` earns (M5): the best of the species' `moodQualities` whose
/// `minMood` it reaches, as the slot records it; `None` for normal quality.
pub fn mood_quality(ctx: &EngineContext, species: &AnimalSpeciesDefinition, mood: i32) -> Option<String> {
    let best = species.mood_qualities.iter().filter(|tier| mood >= tier.min_mood).max_by_key(|tier| tier.min_mood)?;
    ctx.slot_quality(Some(&best.quality))
}

/// Young born overnight (M5): for each building with room and each species with `breeding`
/// living there, the adults in the mood for it give one roll of the species' chance (from the
/// game's random stream, only when there is something to roll for).
pub fn breed_overnight(ctx: &EngineContext, state: &mut GameState) -> Effects {
    let mut effects = Vec::new();
    if state.animals.is_empty() || ctx.content.animal_species.iter().all(|species| species.breeding.is_none()) {
        return effects;
    }
    let buildings = state.buildings.clone();
    for building in &buildings {
        for species in ctx.content.animal_species.iter() {
            let Some(breeding) = &species.breeding else { continue };
            let parents = state.animals.iter().any(|animal| {
                animal.building_id.as_deref() == Some(building.id.as_str())
                    && animal.species_id == species.id
                    && animal.age_days >= species.days_to_adult
                    && animal.mood >= breeding.min_mood
            });
            if !parents || crate::buildings::occupants(state, &building.id) >= crate::buildings::capacity(ctx, building)
            {
                continue;
            }
            let mut rng = Rng::new(state.rng.clone());
            let born = rng.chance(breeding.chance);
            state.rng = rng.state;
            if !born {
                continue;
            }
            let Some(spot) = crate::buildings::free_spot(ctx, state, building) else { continue };
            let name = format!("Baby {}", species.name);
            let baby = crate::buildings::new_animal(state, species, building, &name, spot);
            effects.push(Effect::say(message_levels::SUCCESS, messages::ANIMAL_BORN.with(&[&species.name])));
            state.animals.push(baby);
        }
    }
    effects
}
