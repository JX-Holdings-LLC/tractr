//! Tiles and scene grids (port of `World/Tiles.cs` / world/tiles.ts). Pure: every function
//! returns new tiles.
//!
//! Tile layer rules — extracted from src/lib/game-helpers.ts, behavior-identical
//! (characterization-tested).

use crate::schema::{tile_layers, tile_types, Scene, TerrainDefinition, Tile, VisualRef, MAX_SCENE_SIZE};
use crate::vocabulary;

/// Returns `"background"`, `"overlay"` or `"object"`: the layer of a built-in terrain (an unknown
/// one is a background).
pub fn classify_tile_type(tile_type: &str) -> String {
    vocabulary::layer_of(vocabulary::builtin_terrains(), tile_type).to_owned()
}

/// A plain tile of the built-in terrain `tile_type` ([`create_empty_tile_in`]).
pub fn create_empty_tile(x: i32, y: i32, tile_type: &str) -> Tile {
    create_empty_tile_in(vocabulary::builtin_terrains(), x, y, tile_type)
}

/// A plain tile of `tile_type`, the type routed to its layer over grass; a terrain that can't
/// be walked on collides.
pub fn create_empty_tile_in(terrains: &[TerrainDefinition], x: i32, y: i32, tile_type: &str) -> Tile {
    let layer = vocabulary::layer_of(terrains, tile_type);
    Tile {
        x,
        y,
        r#type: tile_type.to_owned(),
        background: if layer == tile_layers::BACKGROUND { tile_type.to_owned() } else { tile_types::GRASS.to_owned() },
        overlay: (layer == tile_layers::OVERLAY).then(|| tile_type.to_owned()),
        object: (layer == tile_layers::OBJECT).then(|| tile_type.to_owned()),
        collision: !vocabulary::walkable(terrains, tile_type),
        soil_moisture: 0,
        soil_fertility: 0,
        ..Tile::default()
    }
}

/// `tile` with the built-in terrain `new_type` on its layer ([`set_tile_layer_in`]).
pub fn set_tile_layer(tile: &Tile, new_type: &str, visual: Option<&VisualRef>) -> Tile {
    set_tile_layer_in(vocabulary::builtin_terrains(), tile, new_type, visual)
}

/// `tile` with `new_type` on its layer (and `visual` as that layer's art), keeping the other
/// layers. An object sets the tile's collision (whether it can be walked on); a background or
/// overlay that can't be walked on sets it too, and one that can clears what the terrain it
/// replaces had set, unless the object still blocks.
pub fn set_tile_layer_in(
    terrains: &[TerrainDefinition],
    tile: &Tile,
    new_type: &str,
    visual: Option<&VisualRef>,
) -> Tile {
    let layer = vocabulary::layer_of(terrains, new_type);
    let mut updated = tile.clone();
    updated.r#type = new_type.to_owned();
    if visual.is_some() || tile.visuals.is_some() {
        // { ...tile.visuals, [layer]: visual }
        let mut visuals = tile.visuals.clone().unwrap_or_default();
        match layer {
            tile_layers::BACKGROUND => visuals.background = visual.cloned(),
            tile_layers::OVERLAY => visuals.overlay = visual.cloned(),
            _ => visuals.object = visual.cloned(),
        }
        updated.visuals = Some(visuals);
    }

    let walkable = vocabulary::walkable(terrains, new_type);
    let replaced = match layer {
        tile_layers::BACKGROUND => Some(std::mem::replace(&mut updated.background, new_type.to_owned())),
        tile_layers::OVERLAY => updated.overlay.replace(new_type.to_owned()),
        _ => {
            updated.object = Some(new_type.to_owned());
            updated.collision = !walkable;
            return updated;
        }
    };
    if !walkable {
        updated.collision = true;
    } else if replaced.is_some_and(|old| !vocabulary::walkable(terrains, &old)) {
        updated.collision = updated.object.as_deref().is_some_and(|object| !vocabulary::walkable(terrains, object));
    }
    updated
}

/// `tile` with its `layer` cleared: an overlay or object removed (collision follows what is
/// left); a background is never cleared.
pub fn clear_tile_layer_in(terrains: &[TerrainDefinition], tile: &Tile, layer: &str) -> Tile {
    let mut updated = tile.clone();
    match layer {
        tile_layers::OVERLAY => {
            updated.overlay = None;
            if let Some(visuals) = &mut updated.visuals {
                visuals.overlay = None;
            }
        }
        tile_layers::OBJECT => {
            updated.object = None;
            if let Some(visuals) = &mut updated.visuals {
                visuals.object = None;
            }
        }
        _ => return updated,
    }
    updated.r#type = updated.background.clone();
    updated.collision = [Some(updated.background.as_str()), updated.overlay.as_deref(), updated.object.as_deref()]
        .into_iter()
        .flatten()
        .any(|id| !vocabulary::walkable(terrains, id));
    updated
}

/// A `width` × `height` scene of grass.
pub fn create_empty_scene(id: &str, name: &str, width: i32, height: i32) -> Scene {
    let tiles =
        (0..height.max(0)).map(|y| (0..width.max(0)).map(|x| create_empty_tile(x, y, "grass")).collect()).collect();

    Scene {
        id: id.to_owned(),
        name: name.to_owned(),
        width,
        height,
        tiles,
        transitions: Vec::new(),
        npcs: Vec::new(),
        events: Vec::new(),
        ..Scene::default()
    }
}

/// Make a scene's grid match its size, so every tile inside `width × height` exists: the size is
/// clamped to `1..=MAX_SCENE_SIZE`, rows and columns beyond it are cut, and missing ones are
/// filled with grass. Returns whether anything changed. Runs wherever a world enters the engine
/// (new games, loaded saves, pack scenes): a hand-edited project, an import or a pack can carry a
/// grid of another size, and the simulation would otherwise read tiles that are not there.
pub fn normalize_scene_grid(scene: &mut Scene) -> bool {
    let width = scene.width.clamp(1, MAX_SCENE_SIZE);
    let height = scene.height.clamp(1, MAX_SCENE_SIZE);
    let mut changed = width != scene.width || height != scene.height;
    scene.width = width;
    scene.height = height;
    let (columns, rows) = (width as usize, height as usize);
    if scene.tiles.len() != rows {
        changed = true;
        scene.tiles.truncate(rows);
    }
    for (y, row) in scene.tiles.iter_mut().enumerate() {
        if row.len() != columns {
            changed = true;
            row.truncate(columns);
            let start = row.len();
            row.extend((start..columns).map(|x| create_empty_tile(x as i32, y as i32, "grass")));
        }
    }
    for y in scene.tiles.len()..rows {
        scene.tiles.push((0..columns).map(|x| create_empty_tile(x as i32, y as i32, "grass")).collect());
    }
    changed
}
