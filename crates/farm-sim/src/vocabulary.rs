//! The world vocabulary of a game (M1): its terrains, tools, item categories, crop qualities and
//! mutations, read from the content. A list the content leaves empty (content built by hand, in
//! tests and benchmarks) reads as the built-in one, so every game has the vocabulary the engine
//! had before it became content.

use crate::content_builtin;
use crate::schema::{
    tile_layers, CropMutationDefinition, CropQualityDefinition, GameContent, ItemCategoryDefinition, TerrainDefinition,
    ToolDefinition,
};
use std::sync::OnceLock;

fn builtin<T>(cell: &'static OnceLock<Vec<T>>, make: fn() -> Vec<T>) -> &'static [T] {
    cell.get_or_init(make)
}

static TERRAINS: OnceLock<Vec<TerrainDefinition>> = OnceLock::new();
static TOOLS: OnceLock<Vec<ToolDefinition>> = OnceLock::new();
static ITEM_CATEGORIES: OnceLock<Vec<ItemCategoryDefinition>> = OnceLock::new();
static CROP_QUALITIES: OnceLock<Vec<CropQualityDefinition>> = OnceLock::new();
static CROP_MUTATIONS: OnceLock<Vec<CropMutationDefinition>> = OnceLock::new();

/// The built-in terrains.
pub fn builtin_terrains() -> &'static [TerrainDefinition] {
    builtin(&TERRAINS, content_builtin::terrains)
}

/// The built-in tool `id`.
pub fn builtin_tool(id: &str) -> Option<&'static ToolDefinition> {
    builtin(&TOOLS, content_builtin::tools).iter().find(|tool| tool.id == id)
}

/// The game's terrains.
pub fn terrains(content: &GameContent) -> &[TerrainDefinition] {
    if content.terrains.is_empty() {
        builtin_terrains()
    } else {
        &content.terrains
    }
}

/// The game's tools.
pub fn tools(content: &GameContent) -> &[ToolDefinition] {
    if content.tools.is_empty() {
        builtin(&TOOLS, content_builtin::tools)
    } else {
        &content.tools
    }
}

/// The game's item categories.
pub fn item_categories(content: &GameContent) -> &[ItemCategoryDefinition] {
    if content.item_categories.is_empty() {
        builtin(&ITEM_CATEGORIES, content_builtin::item_categories)
    } else {
        &content.item_categories
    }
}

/// The game's crop quality tiers, ranked.
pub fn crop_qualities(content: &GameContent) -> &[CropQualityDefinition] {
    if content.crop_qualities.is_empty() {
        builtin(&CROP_QUALITIES, content_builtin::crop_qualities)
    } else {
        &content.crop_qualities
    }
}

/// The game's crop mutations, in roll order.
pub fn crop_mutations(content: &GameContent) -> &[CropMutationDefinition] {
    if content.crop_mutations.is_empty() {
        builtin(&CROP_MUTATIONS, content_builtin::crop_mutations)
    } else {
        &content.crop_mutations
    }
}

/// The terrain `id` names among `terrains`.
pub fn find_terrain<'a>(terrains: &'a [TerrainDefinition], id: &str) -> Option<&'a TerrainDefinition> {
    terrains.iter().find(|terrain| terrain.id == id)
}

/// The layer a terrain is painted on: its definition's, else the built-in rule for an id no
/// definition names (the background).
pub fn layer_of<'a>(terrains: &'a [TerrainDefinition], id: &str) -> &'a str {
    match find_terrain(terrains, id) {
        Some(terrain) if tile_layers::ALL.contains(&terrain.layer.as_str()) => &terrain.layer,
        _ => tile_layers::BACKGROUND,
    }
}

/// Whether a terrain can be walked on (an unknown one can).
pub fn walkable(terrains: &[TerrainDefinition], id: &str) -> bool {
    find_terrain(terrains, id).is_none_or(|terrain| terrain.walkable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{tile_types, tool_types};

    #[test]
    fn empty_content_reads_as_the_built_in_vocabulary() {
        let content = GameContent::default();
        assert_eq!(terrains(&content).len(), tile_types::ALL.len());
        assert_eq!(tools(&content).len(), tool_types::ALL.len());
        assert_eq!(item_categories(&content).len(), 8);
        assert_eq!(crop_qualities(&content)[0].id, "normal");
        assert_eq!(crop_mutations(&content)[0].id, "ancient");
    }

    #[test]
    fn layers_and_walking_follow_the_definitions() {
        let builtin = builtin_terrains();
        assert_eq!(layer_of(builtin, tile_types::PATH), tile_layers::OVERLAY);
        assert_eq!(layer_of(builtin, tile_types::DOOR), tile_layers::OBJECT);
        assert_eq!(layer_of(builtin, "lava"), tile_layers::BACKGROUND);
        assert!(!walkable(builtin, tile_types::WALL));
        assert!(walkable(builtin, tile_types::WATER));
        assert!(walkable(builtin, "lava"));
    }
}
