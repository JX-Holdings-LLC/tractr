//! What must hold of every state the engine reaches from a new game, whatever the content and
//! the commands (#116). The fuzz targets and the property tests check it after every step.

use farm_sim::schema::GameState;
use farm_sim::{inventory, units};

/// The first broken invariant of `state`, if any:
///
/// - money is never negative;
/// - energy is in `0..=max_energy`;
/// - every stack holds at least one unit and at most its item's stack cap;
/// - the inventory has at most `max_inventory_size` slots;
/// - the player stands inside a scene of the world (when it has one), on its grid;
/// - every scene's grid is `height` rows of `width` tiles;
/// - health (M7) is never negative, and every monster is in a scene of the world;
/// - every building (M5) is in a scene of the world, and a standing one with an interior has it;
/// - an open storage (M5) is a placed object of the world.
pub fn check(state: &GameState) -> Result<(), String> {
    let player = &state.player;
    if player.health.is_some_and(|health| health < 0) {
        return Err(format!("health is negative: {:?}", player.health));
    }
    let has_scene = |id: &str| state.world.scenes.iter().any(|scene| scene.id == id);
    if let Some(monster) = state.monsters.iter().find(|monster| !has_scene(&monster.scene_id)) {
        return Err(format!(
            "monster {} is in scene '{}', which the world does not have",
            monster.id, monster.scene_id
        ));
    }
    for building in &state.buildings {
        if !has_scene(&building.scene_id) {
            return Err(format!(
                "building {} is in scene '{}', which the world does not have",
                building.id, building.scene_id
            ));
        }
        if building.built() && !building.interior_scene_id.is_empty() && !has_scene(&building.interior_scene_id) {
            return Err(format!("building {} has no interior '{}'", building.id, building.interior_scene_id));
        }
    }
    if let Some(storage) = &state.storage {
        let placed = state
            .world
            .scenes
            .iter()
            .find(|scene| scene.id == storage.scene_id)
            .and_then(|scene| scene.tile(storage.x, storage.y))
            .is_some_and(|tile| tile.machine.is_some());
        if !placed {
            return Err(format!(
                "the open storage at ({}, {}) of '{}' is gone",
                storage.x, storage.y, storage.scene_id
            ));
        }
    }
    if player.money < 0 {
        return Err(format!("money is negative: {}", player.money));
    }
    if player.energy < 0 || player.energy > player.max_energy {
        return Err(format!("energy {} is outside 0..={}", player.energy, player.max_energy));
    }
    for (index, slot) in player.inventory.iter().enumerate() {
        let cap = inventory::stack_cap(&slot.item);
        if slot.quantity == 0 || slot.quantity > cap {
            return Err(format!("slot {index} holds {} × {} (cap {cap})", slot.quantity, slot.item.id));
        }
    }
    if player.inventory.len() > player.max_inventory_size as usize {
        return Err(format!(
            "{} inventory slots, more than the {} allowed",
            player.inventory.len(),
            player.max_inventory_size
        ));
    }
    for scene in &state.world.scenes {
        let rows_ok = usize::try_from(scene.height).is_ok_and(|height| scene.tiles.len() == height);
        let columns_ok = scene.tiles.iter().all(|row| usize::try_from(scene.width).is_ok_and(|w| row.len() == w));
        if !rows_ok || !columns_ok {
            return Err(format!("scene '{}' is not a {}×{} grid", scene.id, scene.width, scene.height));
        }
    }
    if state.world.scenes.is_empty() {
        // A game without scenes (an empty project) has nowhere to stand.
        return Ok(());
    }
    let Some(scene) = state.world.scenes.iter().find(|scene| scene.id == player.scene_id) else {
        return Err(format!("the player is in scene '{}', which the world does not have", player.scene_id));
    };
    let (x, y) = (units::tile_of(player.x), units::tile_of(player.y));
    if x < 0 || y < 0 || x >= scene.width || y >= scene.height {
        return Err(format!("the player stands on ({x}, {y}), outside the {}×{} scene", scene.width, scene.height));
    }
    Ok(())
}
