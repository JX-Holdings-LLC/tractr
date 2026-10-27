//! Buildings (M5): a builder (a shop that lists buildings) puts one up on a free spot of a farm
//! scene for money and materials; it stands after its nights of building, with a door into an
//! interior made from its template scene. Upgrades work the same way on a standing building
//! (the player's house too), and animals live in buildings that house their species.

use crate::effects::{message_levels, Effect};
use crate::engine_types::{Effects, EngineContext};
use crate::messages::{self, Message};
use crate::schema::{
    event_triggers, AnimalSpeciesDefinition, AnimalState, BuildingDefinition, BuildingState, GameState,
    RecipeIngredient, Scene, SceneTransition,
};
use crate::units;
use crate::world::{pathfinding, world_movement};
use crate::{events, inventory};

/// The id prefix of buildings put up in play (`building-1`, …), also their interiors' scene ids.
pub const BUILDING_PREFIX: &str = "building-";

/// The building kind `id`.
pub fn definition<'a>(ctx: &'a EngineContext, id: &str) -> Option<&'a BuildingDefinition> {
    ctx.content.building_types.iter().find(|definition| definition.id == id)
}

/// `id` and every kind it upgrades from, nearest first (cycles stop).
pub fn lineage<'a>(ctx: &'a EngineContext, id: &'a str) -> Vec<&'a str> {
    let mut chain = vec![id];
    let mut current = id;
    while let Some(previous) = definition(ctx, current).and_then(|definition| definition.upgrade_of.as_deref()) {
        if chain.contains(&previous) || chain.len() > 32 {
            break;
        }
        chain.push(previous);
        current = previous;
    }
    chain
}

/// Whether a building of kind `building_id` counts as `kind` (it is one, or an upgrade of one).
pub fn is_kind(ctx: &EngineContext, building_id: &str, kind: &str) -> bool {
    lineage(ctx, building_id).contains(&kind)
}

/// Standing buildings of `kind` (upgrades counting).
pub fn count_standing(ctx: &EngineContext, state: &GameState, kind: &str) -> u32 {
    let count = state.buildings.iter().filter(|b| b.built() && is_kind(ctx, &b.building_id, kind)).count();
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// The scenes a kind may be built in: its own list, else the farm scene, else the start scene.
pub fn allowed_scenes<'a>(ctx: &'a EngineContext, definition: &'a BuildingDefinition) -> Vec<&'a str> {
    if !definition.scenes.is_empty() {
        return definition.scenes.iter().map(String::as_str).collect();
    }
    let farm = ctx.content.settings.farm.as_ref().map(|farm| farm.scene_id.as_str()).filter(|id| !id.is_empty());
    vec![farm.unwrap_or(ctx.content.start_scene_id.as_str())]
}

/// Whether the inventory holds every material.
fn has_materials(state: &GameState, materials: &[RecipeIngredient]) -> bool {
    materials.iter().all(|m| inventory::count_item(&state.player.inventory, &m.item_id) >= u64::from(m.quantity))
}

fn pay(state: &mut GameState, cost: i64, materials: &[RecipeIngredient]) {
    state.player.money = state.player.money.saturating_sub(cost.max(0));
    for material in materials {
        state.player.inventory = inventory::remove_item(&state.player.inventory, &material.item_id, material.quantity);
    }
}

/// Why the cost can't be paid, if it can't.
fn cost_refusal(state: &GameState, cost: i64, materials: &[RecipeIngredient]) -> Option<Effects> {
    if state.player.money < cost {
        return Some(vec![Effect::say(message_levels::ERROR, &messages::NOT_ENOUGH_MONEY)]);
    }
    if !has_materials(state, materials) {
        return Some(vec![Effect::say(message_levels::ERROR, &messages::MISSING_MATERIALS)]);
    }
    None
}

/// Why a building of `definition` can't go with its top-left corner on tile `(x, y)` of
/// `scene_id`: outside the scenes it may be built in, off the map, or on a tile that is not
/// free (blocked, planted, holding anything, a door, an NPC, an animal or the player). `None`
/// when it can. The build menu asks this for its placement preview.
pub fn site_refusal(
    ctx: &EngineContext,
    state: &GameState,
    definition: &BuildingDefinition,
    scene_id: &str,
    x: i32,
    y: i32,
) -> Option<Message> {
    if !allowed_scenes(ctx, definition).contains(&scene_id) {
        return Some(messages::CANT_BUILD_HERE.with(&[&definition.name]));
    }
    let scene = world_movement::find_scene(state, scene_id).or_else(|| ctx.content_scene(scene_id))?;
    let (width, height) = definition.size();
    let fits = x >= 0
        && y >= 0
        && x.checked_add(width).is_some_and(|right| right <= scene.width)
        && y.checked_add(height).is_some_and(|bottom| bottom <= scene.height);
    if !fits {
        return Some(messages::NO_ROOM_TO_BUILD.with(&[]));
    }
    for ty in y..y + height {
        for tx in x..x + width {
            if !tile_free(ctx, state, scene, tx, ty) {
                return Some(messages::NO_ROOM_TO_BUILD.with(&[]));
            }
        }
    }
    None
}

/// Whether tile `(x, y)` of `scene` is free ground a building may cover.
fn tile_free(ctx: &EngineContext, state: &GameState, scene: &Scene, x: i32, y: i32) -> bool {
    let Some(tile) = scene.tile(x, y) else { return false };
    if pathfinding::tile_blocks(tile, &ctx.content.node_types, &ctx.content.machine_types)
        || tile.crop.is_some()
        || tile.node.is_some()
        || tile.machine.is_some()
        || tile.item.is_some()
        || tile.ladder_down == Some(true)
    {
        return false;
    }
    if crate::crafting::keeps_clear(ctx, state, &scene.id, x, y) {
        return false;
    }
    let (px, py) = (units::tiles(x), units::tiles(y));
    let on_tile = |ex: i32, ey: i32| units::tile_of(ex) == x && units::tile_of(ey) == y;
    if state.npcs.values().any(|npc| npc.scene_id == scene.id && npc.x == px && npc.y == py) {
        return false;
    }
    if state.animals.iter().any(|animal| animal.scene_id == scene.id && on_tile(animal.x, animal.y)) {
        return false;
    }
    if state.monsters.iter().any(|monster| monster.scene_id == scene.id && on_tile(monster.x, monster.y)) {
        return false;
    }
    if state.player.scene_id == scene.id {
        let half = world_movement::PLAYER_HALF_WIDTH;
        let (left, right) = (units::tile_of(state.player.x - half), units::tile_of(state.player.x + half - 1));
        let (top, bottom) = (units::tile_of(state.player.y - half), units::tile_of(state.player.y + half - 1));
        if (left..=right).contains(&x) && (top..=bottom).contains(&y) {
            return false;
        }
    }
    true
}

/// Whether the open shop builds `building_id` (its `buildings` list).
fn shop_builds(ctx: &EngineContext, state: &GameState, building_id: &str) -> bool {
    let Some(session) = &state.shop else { return false };
    crate::economy::find_shop(ctx, state, &session.shop_id)
        .is_some_and(|shop| shop.buildings.iter().any(|id| id == building_id))
}

/// Whether the open shop sells animals of `species_id`.
fn shop_sells_animal(ctx: &EngineContext, state: &GameState, species_id: &str) -> bool {
    let Some(session) = &state.shop else { return false };
    crate::economy::find_shop(ctx, state, &session.shop_id)
        .is_some_and(|shop| shop.animals.iter().any(|id| id == species_id))
}

/// `orderBuilding`, `upgradeBuilding` and `buyAnimal` under [`crate::CommandRules::Player`]:
/// only from an open shop that offers them.
pub fn order_refusal(ctx: &EngineContext, state: &GameState, building_id: &str) -> Option<Effects> {
    (!shop_builds(ctx, state, building_id)).then(|| vec![Effect::say(message_levels::ERROR, &messages::NOT_SOLD_HERE)])
}

/// See [`order_refusal`].
pub fn animal_refusal(ctx: &EngineContext, state: &GameState, species_id: &str) -> Option<Effects> {
    (!shop_sells_animal(ctx, state, species_id))
        .then(|| vec![Effect::say(message_levels::ERROR, &messages::NOT_SOLD_HERE)])
}

/// The id the next building put up gets: one past the highest `building-N` on the map.
fn next_building_id(state: &GameState) -> String {
    let highest = state
        .buildings
        .iter()
        .filter_map(|building| building.id.strip_prefix(BUILDING_PREFIX)?.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    format!("{BUILDING_PREFIX}{}", highest.saturating_add(1))
}

/// Order a building with its top-left tile at `(x, y)` of `scene_id`: it is paid for now, and
/// its footprint is a building site until it stands.
pub fn handle_order(
    ctx: &EngineContext,
    state: &mut GameState,
    building_id: &str,
    scene_id: &str,
    x: i32,
    y: i32,
) -> Effects {
    let Some(definition) = definition(ctx, building_id) else {
        return vec![Effect::say(message_levels::ERROR, &messages::UNKNOWN_BUILDING)];
    };
    if definition.upgrade_of.is_some() {
        return vec![Effect::say(message_levels::ERROR, &messages::ONLY_AN_UPGRADE)];
    }
    if let Some(refused) = cost_refusal(state, definition.cost, &definition.materials) {
        return refused;
    }
    if let Some(reason) = site_refusal(ctx, state, definition, scene_id, x, y) {
        return vec![Effect::say(message_levels::ERROR, reason)];
    }
    let Some(scene_index) = world_movement::ensure_scene(ctx, state, scene_id) else {
        return Vec::new();
    };
    pay(state, definition.cost, &definition.materials);
    let (width, height) = definition.size();
    let scene = &mut state.world.scenes[scene_index];
    for ty in y..y + height {
        for tx in x..x + width {
            if let Some(tile) = scene.tile_mut(tx, ty) {
                tile.collision = true;
            }
        }
    }
    let days = definition.build_days.max(1);
    state.buildings.push(BuildingState {
        id: next_building_id(state),
        building_id: building_id.to_owned(),
        scene_id: scene_id.to_owned(),
        x,
        y,
        interior_scene_id: String::new(),
        days_left: days,
        upgrade_to: None,
    });
    vec![Effect::say(message_levels::SUCCESS, messages::BUILDING_ORDERED.with(&[&definition.name, &days]))]
}

/// Upgrade standing building `id` into kind `building_id` (whose `upgradeOf` is its kind).
pub fn handle_upgrade(ctx: &EngineContext, state: &mut GameState, id: &str, building_id: &str) -> Effects {
    let Some(index) = state.buildings.iter().position(|building| building.id == id) else {
        return vec![Effect::say(message_levels::ERROR, &messages::UNKNOWN_BUILDING)];
    };
    let Some(target) = definition(ctx, building_id) else {
        return vec![Effect::say(message_levels::ERROR, &messages::UNKNOWN_BUILDING)];
    };
    let building = &state.buildings[index];
    if building.days_left > 0 {
        return vec![Effect::say(message_levels::INFO, &messages::BUILDING_BUSY)];
    }
    if target.upgrade_of.as_deref() != Some(building.building_id.as_str()) {
        return vec![Effect::say(message_levels::ERROR, &messages::NOT_AN_UPGRADE)];
    }
    if let Some(refused) = cost_refusal(state, target.cost, &target.materials) {
        return refused;
    }
    pay(state, target.cost, &target.materials);
    let days = target.build_days.max(1);
    let building = &mut state.buildings[index];
    building.upgrade_to = Some(building_id.to_owned());
    building.days_left = days;
    vec![Effect::say(message_levels::SUCCESS, messages::UPGRADE_ORDERED.with(&[&target.name, &days]))]
}

/// The night's building work: every site and upgrade under way is a night nearer, and those
/// whose nights are done stand in the morning (their interiors made, their doors placed).
pub fn advance_construction(ctx: &EngineContext, state: &mut GameState) -> Effects {
    let mut finished = Vec::new();
    for index in 0..state.buildings.len() {
        let building = &mut state.buildings[index];
        if building.days_left == 0 {
            continue;
        }
        building.days_left -= 1;
        if building.days_left > 0 {
            continue;
        }
        if let Some(upgrade) = building.upgrade_to.take() {
            building.building_id = upgrade;
        }
        finished.push(index);
    }
    let mut effects = Vec::new();
    for index in finished {
        let building = state.buildings[index].clone();
        let Some(definition) = definition(ctx, &building.building_id) else { continue };
        stand(ctx, state, index, definition);
        effects.push(Effect::say(message_levels::SUCCESS, messages::BUILDING_DONE.with(&[&definition.name])));
        ctx.emit(crate::hooks::HookEvent::Build(crate::hooks::BuildHookPayload {
            building_id: building.building_id.clone(),
            scene_id: building.scene_id.clone(),
        }));
        let kinds = lineage(ctx, &building.building_id);
        effects.extend(events::fire_trigger(ctx, state, &events::Trigger::about(event_triggers::BUILD, &kinds)));
    }
    effects
}

/// The door's tile on the map and the tile in front of it (where the player comes out).
fn outside_door(definition: &BuildingDefinition, building: &BuildingState) -> ((i32, i32), (i32, i32)) {
    let (dx, dy) = definition.door_offset();
    let door = (building.x + dx, building.y + dy);
    (door, (door.0, door.1 + 1))
}

/// The interior's door tile and the tile above it (where the player arrives).
fn inside_door(definition: &BuildingDefinition, interior: &Scene) -> ((i32, i32), (i32, i32)) {
    let door = match &definition.interior_door {
        Some(point) => (point.x, point.y),
        None => (interior.width / 2, interior.height - 1),
    };
    (door, (door.0, door.1 - 1))
}

/// Makes building `index` stand: its footprint solid but for the door, and the interior of its
/// (new) kind behind the door, keeping the objects and animals an earlier interior held.
fn stand(ctx: &EngineContext, state: &mut GameState, index: usize, definition: &BuildingDefinition) {
    let building = state.buildings[index].clone();
    let Some(outside_index) = world_movement::ensure_scene(ctx, state, &building.scene_id) else { return };
    let (width, height) = definition.size();
    let ((door_x, door_y), (front_x, front_y)) = outside_door(definition, &building);
    let template = definition.interior_scene_id.as_deref().and_then(|id| ctx.content_scene(id)).cloned();
    {
        let outside = &mut state.world.scenes[outside_index];
        for ty in building.y..building.y + height {
            for tx in building.x..building.x + width {
                if let Some(tile) = outside.tile_mut(tx, ty) {
                    tile.collision = template.is_none() || (tx, ty) != (door_x, door_y);
                }
            }
        }
    }
    let Some(mut interior) = template else { return };

    let interior_id =
        if building.interior_scene_id.is_empty() { building.id.clone() } else { building.interior_scene_id.clone() };
    let previous = state.world.scenes.iter().position(|scene| scene.id == interior_id);
    interior.id.clone_from(&interior_id);
    interior.name = match previous {
        Some(previous) => state.world.scenes[previous].name.clone(),
        None => definition.name.clone(),
    };
    // Interiors made in play are the engine's (the editor's scene lists leave them out); an
    // authored interior (the house) stays authored.
    if interior_id.starts_with(BUILDING_PREFIX) {
        interior.generated = Some(true);
    }
    interior.building = Some(building.id.clone());
    crate::world::tiles::normalize_scene_grid(&mut interior);
    let ((inner_x, inner_y), (arrive_x, arrive_y)) = inside_door(definition, &interior);
    interior.transitions.retain(|t| (t.from_x, t.from_y) != (inner_x, inner_y));
    interior.transitions.push(SceneTransition {
        from_x: inner_x,
        from_y: inner_y,
        to_scene_id: building.scene_id.clone(),
        to_x: front_x,
        to_y: front_y,
        ..SceneTransition::default()
    });
    match previous {
        Some(previous) => {
            // An upgrade: what stood on the old floor stays where the new floor has room.
            let old = std::mem::replace(&mut state.world.scenes[previous], interior);
            let new = &mut state.world.scenes[previous];
            for row in &old.tiles {
                for tile in row {
                    let Some(machine) = &tile.machine else { continue };
                    if let Some(target) = new.tile_mut(tile.x, tile.y) {
                        if target.machine.is_none() && !target.collision {
                            target.machine = Some(machine.clone());
                        }
                    }
                }
            }
            let free = |scene: &Scene, x: i32, y: i32| {
                scene.tile(x, y).is_some_and(|tile| {
                    !pathfinding::tile_blocks(tile, &ctx.content.node_types, &ctx.content.machine_types)
                })
            };
            let arrival = (units::tiles(arrive_x), units::tiles(arrive_y));
            for animal in state.animals.iter_mut().filter(|animal| animal.scene_id == interior_id) {
                if !free(&state.world.scenes[previous], units::tile_of(animal.x), units::tile_of(animal.y)) {
                    (animal.x, animal.y) = arrival;
                }
            }
        }
        None => state.world.scenes.push(interior),
    }
    state.buildings[index].interior_scene_id = interior_id.clone();
    // The way in: the door tile leads to the tile inside the interior's door.
    let outside = &mut state.world.scenes[outside_index];
    outside.transitions.retain(|t| t.to_scene_id != interior_id && (t.from_x, t.from_y) != (door_x, door_y));
    outside.transitions.push(SceneTransition {
        from_x: door_x,
        from_y: door_y,
        to_scene_id: interior_id,
        to_x: arrive_x,
        to_y: arrive_y,
        ..SceneTransition::default()
    });
}

/// The building on the map that houses `species` (its kind or an upgrade of it is in the
/// species' `housing`).
pub fn houses(ctx: &EngineContext, species: &AnimalSpeciesDefinition, building: &BuildingState) -> bool {
    building.built()
        && !building.interior_scene_id.is_empty()
        && lineage(ctx, &building.building_id).iter().any(|kind| species.housing.iter().any(|id| id == kind))
}

/// Animals living in building `id`.
pub fn occupants(state: &GameState, id: &str) -> u32 {
    let count = state.animals.iter().filter(|animal| animal.building_id.as_deref() == Some(id)).count();
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// The animals building `building` has room for.
pub fn capacity(ctx: &EngineContext, building: &BuildingState) -> u32 {
    definition(ctx, &building.building_id).map_or(0, |definition| definition.capacity)
}

/// A free tile of the interior of `building` for a new animal: walkable, not a door, with no
/// animal on it; scanned row by row.
pub fn free_spot(ctx: &EngineContext, state: &GameState, building: &BuildingState) -> Option<(i32, i32)> {
    let scene = world_movement::find_scene(state, &building.interior_scene_id)?;
    for y in 0..scene.height {
        for x in 0..scene.width {
            let Some(tile) = scene.tile(x, y) else { continue };
            if pathfinding::tile_blocks(tile, &ctx.content.node_types, &ctx.content.machine_types)
                || tile.machine.is_some()
                || scene.transitions.iter().any(|t| (t.from_x, t.from_y) == (x, y))
            {
                continue;
            }
            let taken = state.animals.iter().any(|animal| {
                animal.scene_id == scene.id && units::tile_of(animal.x) == x && units::tile_of(animal.y) == y
            });
            let arrival =
                scene.transitions.iter().any(|t| t.to_scene_id != scene.id && (t.from_x, t.from_y - 1) == (x, y));
            if !taken && !arrival {
                return Some((x, y));
            }
        }
    }
    None
}

/// The id the next animal gets: one past the highest `animal-N`.
fn next_animal_id(state: &GameState) -> String {
    let highest = state
        .animals
        .iter()
        .filter_map(|animal| animal.id.strip_prefix("animal-")?.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    format!("animal-{}", highest.saturating_add(1))
}

/// A new animal of `species` in `building` on its free tile `(x, y)`.
pub fn new_animal(
    state: &GameState,
    species: &AnimalSpeciesDefinition,
    building: &BuildingState,
    name: &str,
    (x, y): (i32, i32),
) -> AnimalState {
    let name = name.trim();
    let name = if name.is_empty() {
        let count = state.animals.iter().filter(|animal| animal.species_id == species.id).count();
        format!("{} {}", species.name, count + 1)
    } else {
        name.chars().take(crate::wardrobe::MAX_NAME_CHARS).collect()
    };
    AnimalState {
        id: next_animal_id(state),
        species_id: species.id.clone(),
        name,
        scene_id: building.interior_scene_id.clone(),
        x: units::tiles(x),
        y: units::tiles(y),
        building_id: Some(building.id.clone()),
        ..AnimalState::default()
    }
}

/// Buy an animal of `species_id` into building `building` (on the map), which must house the
/// species and have room.
pub fn handle_buy_animal(
    ctx: &EngineContext,
    state: &mut GameState,
    species_id: &str,
    building: &str,
    name: &str,
) -> Effects {
    let Some(species) = ctx.animal_species(species_id) else {
        return vec![Effect::say(message_levels::ERROR, &messages::UNKNOWN_ANIMAL)];
    };
    let Some(home) = state.buildings.iter().find(|candidate| candidate.id == building).cloned() else {
        return vec![Effect::say(message_levels::ERROR, &messages::UNKNOWN_BUILDING)];
    };
    if !houses(ctx, species, &home) {
        return vec![Effect::say(message_levels::ERROR, messages::CANT_HOUSE.with(&[&species.name]))];
    }
    if occupants(state, &home.id) >= capacity(ctx, &home) {
        return vec![Effect::say(message_levels::ERROR, &messages::BUILDING_FULL)];
    }
    if state.player.money < species.purchase_cost {
        return vec![Effect::say(message_levels::ERROR, &messages::NOT_ENOUGH_MONEY)];
    }
    let Some(spot) = free_spot(ctx, state, &home) else {
        return vec![Effect::say(message_levels::ERROR, &messages::BUILDING_FULL)];
    };
    let animal = new_animal(state, species, &home, name, spot);
    let home_name = definition(ctx, &home.building_id).map_or(home.id.as_str(), |d| d.name.as_str()).to_owned();
    state.player.money = state.player.money.saturating_sub(species.purchase_cost.max(0));
    let message = messages::BOUGHT_ANIMAL.with(&[&animal.name, &home_name]);
    state.animals.push(animal);
    vec![Effect::say(message_levels::SUCCESS, message)]
}
