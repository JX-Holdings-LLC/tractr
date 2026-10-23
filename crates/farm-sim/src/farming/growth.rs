//! Growth requirements (M1): what a crop needs to grow beyond water and its seasons — light or
//! dark, a temperature range, and neighbours (crops, a terrain, water). Where it may be planted
//! (terrains, scene tags, fertilizers) is checked when it is planted
//! (`farming_actions::plant_seed`); this module decides, each night, which crops a scene holds
//! back.

use crate::engine_types::EngineContext;
use crate::schema::{light_needs, neighbor_kinds, GrowthRequirements, NeighborRequirement, Scene, Tile};
use indexmap::IndexSet;

/// The tiles of `scene` whose live crop does not grow tonight because a growth requirement is
/// not met. `season_temperature` is the new season's temperature, which a scene without its own
/// has. Empty when no crop in the scene has requirements (every built-in crop).
pub fn unmet_tiles(ctx: &EngineContext, scene: &Scene, season_temperature: Option<i32>) -> IndexSet<(usize, usize)> {
    let mut unmet = IndexSet::new();
    let temperature = scene.temperature.or(season_temperature);
    for (y, row) in scene.tiles.iter().enumerate() {
        for (x, tile) in row.iter().enumerate() {
            let Some(crop) = tile.crop.as_ref().filter(|crop| crop.withered != Some(true)) else { continue };
            let Some(needs) = ctx.content.crops.get(&crop.r#type).and_then(|def| def.requirements.as_ref()) else {
                continue;
            };
            if !grows_here(ctx, scene, (x, y), needs, temperature) {
                unmet.insert((x, y));
            }
        }
    }
    unmet
}

/// Whether a crop with `needs` on tile `(x, y)` of `scene` grows tonight.
pub fn grows_here(
    ctx: &EngineContext,
    scene: &Scene,
    (x, y): (usize, usize),
    needs: &GrowthRequirements,
    temperature: Option<i32>,
) -> bool {
    let Some(tile) = scene.tiles.get(y).and_then(|row| row.get(x)) else { return false };
    if let Some(light) = needs.light.as_deref() {
        let lit = !scene.is_dark() && !layers(tile).any(|id| ctx.terrain(id).is_some_and(|t| t.blocks_light));
        match light {
            light_needs::LIGHT if !lit => return false,
            light_needs::DARK if lit => return false,
            _ => {}
        }
    }
    // A place with no temperature (neither the scene nor the season gives one) suits every crop.
    if let Some(temperature) = temperature {
        if needs.min_temperature.is_some_and(|min| temperature < min)
            || needs.max_temperature.is_some_and(|max| temperature > max)
        {
            return false;
        }
    }
    needs.neighbors.iter().all(|need| neighbors_met(ctx, scene, (x, y), need))
}

/// The terrain ids on a tile's layers.
fn layers(tile: &Tile) -> impl Iterator<Item = &str> {
    std::iter::once(tile.background.as_str()).chain(tile.overlay.as_deref()).chain(tile.object.as_deref())
}

/// Whether at least `need.min` tiles within `need.radius` of `(x, y)` (the tile itself left
/// out) match the neighbour requirement.
fn neighbors_met(ctx: &EngineContext, scene: &Scene, (x, y): (usize, usize), need: &NeighborRequirement) -> bool {
    let radius = i64::from(need.radius.clamp(1, 8));
    let wanted = need.min.max(1);
    let id = need.id.as_deref().unwrap_or("");
    let mut found = 0;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx == 0 && dy == 0 {
                continue;
            }
            let (Ok(nx), Ok(ny)) = (usize::try_from(x as i64 + dx), usize::try_from(y as i64 + dy)) else { continue };
            let Some(tile) = scene.tiles.get(ny).and_then(|row| row.get(nx)) else { continue };
            let matches = match need.kind.as_str() {
                neighbor_kinds::CROP => {
                    tile.crop.as_ref().is_some_and(|crop| crop.withered != Some(true) && crop.r#type == id)
                }
                neighbor_kinds::TERRAIN => layers(tile).any(|layer| layer == id),
                neighbor_kinds::WATER => layers(tile).any(|layer| ctx.terrain(layer).is_some_and(|t| t.water_source)),
                _ => false,
            };
            if matches {
                found += 1;
                if found >= wanted {
                    return true;
                }
            }
        }
    }
    false
}
