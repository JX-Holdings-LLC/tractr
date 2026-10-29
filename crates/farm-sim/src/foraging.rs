//! Seasonal gathering and deterministic morning forage. Old scenes consume no extra RNG.
use crate::engine_types::EngineContext;
use crate::rng::Rng;
use crate::schema::{ForageSpawnTable, GameState, Scene, Tile, TileNode};

pub fn in_season(seasons: Option<&[String]>, season: &str) -> bool {
    seasons.is_none_or(|seasons| seasons.is_empty() || seasons.iter().any(|s| s == season))
}

/// Static eligibility, also useful to creator tools before a scene is populated.
pub fn eligible_tile(ctx: &EngineContext, scene: &Scene, table: &ForageSpawnTable, tile: &Tile) -> bool {
    !tile.collision
        && tile.object.is_none()
        && tile.overlay.is_none()
        && tile.crop.is_none()
        && tile.item.is_none()
        && tile.machine.is_none()
        && tile.node.is_none()
        && tile.ladder_down != Some(true)
        && table.terrains.contains(&tile.background)
        && ctx.terrain(&tile.background).is_some_and(|t| t.walkable)
        && !scene.transitions.iter().any(|door| door.from_x == tile.x && door.from_y == tile.y)
        && table.region.as_ref().is_none_or(|r| {
            tile.x >= r.x
                && tile.y >= r.y
                && tile.x < r.x.saturating_add(r.width)
                && tile.y < r.y.saturating_add(r.height)
        })
}

pub fn morning(ctx: &EngineContext, state: &mut GameState) {
    let luck = crate::skills::stat_bonus(ctx, state, "luck");
    let day = state.clock.day;
    let season = &state.clock.season;
    let arrivals: Vec<_> = state
        .world
        .scenes
        .iter()
        .flat_map(|scene| &scene.transitions)
        .map(|door| (door.to_scene_id.clone(), door.to_x, door.to_y))
        .collect();
    let mut rng = Rng::new(state.rng.clone());
    for scene in &mut state.world.scenes {
        for row in &mut scene.tiles {
            for tile in row {
                let Some(node) = tile.node.as_mut() else { continue };
                if node.spawned_on_day.is_some()
                    && (node.spawned_season.as_ref().is_some_and(|s| s != season)
                        || node.expires_on_day.is_some_and(|expires| day >= expires))
                {
                    tile.node = None;
                    continue;
                }
                let Some(definition) = ctx.node_type(&node.type_id) else { continue };
                if !in_season(definition.seasons.as_deref(), season) {
                    if node.dormant_health.is_none() {
                        node.dormant_health = Some(node.remaining_health);
                    }
                    node.remaining_health = 0;
                } else if let Some(health) = node.dormant_health.take() {
                    node.remaining_health = health;
                }
            }
        }
        let tables = scene.forage_spawns.clone().unwrap_or_default();
        for table in &tables {
            if !in_season(table.seasons.as_deref(), season)
                || table.scene_tags.as_ref().is_some_and(|tags| {
                    tags.iter().any(|tag| !scene.tags.as_ref().is_some_and(|actual| actual.contains(tag)))
                })
            {
                continue;
            }
            let entries: Vec<_> = table
                .entries
                .iter()
                .filter_map(|e| {
                    ctx.node_type(&e.node_type_id)
                        .filter(|n| in_season(n.seasons.as_deref(), season))
                        .map(|node| (e, node))
                })
                .collect();
            let mut weights: Vec<_> = entries.iter().map(|(e, _)| e.weight).collect();
            crate::buffs::luck_weights(&mut weights, luck);
            if entries.is_empty() {
                continue;
            }
            let mut tiles: Vec<_> = scene
                .tiles
                .iter()
                .flatten()
                .filter(|tile| eligible_tile(ctx, scene, table, tile))
                .filter(|tile| !arrivals.iter().any(|(id, x, y)| id == &scene.id && *x == tile.x && *y == tile.y))
                .filter(|tile| {
                    !(scene.id == state.player.scene_id
                        && tile.x == crate::units::tile_of(state.player.x)
                        && tile.y == crate::units::tile_of(state.player.y))
                })
                .filter(|tile| {
                    !state.npcs.values().any(|npc| {
                        npc.scene_id == scene.id
                            && crate::units::tile_of(npc.x) == tile.x
                            && crate::units::tile_of(npc.y) == tile.y
                    })
                })
                .map(|tile| (tile.x, tile.y))
                .collect();
            tiles.retain(|(x, y)| {
                !state.buildings.iter().any(|building| {
                    building.scene_id == scene.id
                        && crate::buildings::definition(ctx, &building.building_id).is_some_and(|definition| {
                            let (width, height) = definition.size();
                            *x >= building.x
                                && *y >= building.y
                                && *x < building.x.saturating_add(width)
                                && *y < building.y.saturating_add(height)
                        })
                }) && !state.animals.iter().any(|animal| {
                    animal.scene_id == scene.id
                        && crate::units::tile_of(animal.x) == *x
                        && crate::units::tile_of(animal.y) == *y
                })
            });
            for _ in 0..table.maximum.min(1024).min(tiles.len() as u32) {
                let Some(index) = rng.weighted(&weights) else { break };
                let tile_index = rng.int(0, tiles.len() as i64 - 1) as usize;
                let (x, y) = tiles.swap_remove(tile_index);
                let (entry, definition) = entries[index];
                if let Some(tile) = scene.tile_mut(x, y) {
                    tile.node = Some(TileNode {
                        type_id: entry.node_type_id.clone(),
                        remaining_health: definition.health,
                        spawned_on_day: Some(day),
                        spawned_season: Some(season.clone()),
                        expires_on_day: table.lifetime_days.map(|days| day.saturating_add(days.max(1))),
                        ..TileNode::default()
                    });
                }
            }
        }
    }
    state.rng = rng.state;
}
