//! The world vocabulary (M1): terrains, tools, item categories, crop qualities and mutations as
//! content. Each game gets the built-in definitions (`content_builtin`) with the project's own
//! over them (`state::over_builtins`); the F# records are in `Schema.fs`.

use super::graphics::VisualRef;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The tile layers a terrain is painted on (F# `TileLayers`).
pub mod tile_layers {
    pub const BACKGROUND: &str = "background";
    pub const OVERLAY: &str = "overlay";
    pub const OBJECT: &str = "object";

    pub const ALL: &[&str] = &[BACKGROUND, OVERLAY, OBJECT];
}

/// What a tool does (F# `ToolActions`).
pub mod tool_actions {
    pub const WATER: &str = "water";
    pub const TILL: &str = "till";
    pub const HARVEST: &str = "harvest";
    pub const FISH: &str = "fish";
    pub const BREAK: &str = "break";
    pub const TRANSFORM: &str = "transform";
    pub const PLACE: &str = "place";
    pub const PLANT: &str = "plant";

    pub const ALL: &[&str] = &[WATER, TILL, HARVEST, FISH, BREAK, TRANSFORM, PLACE, PLANT];
}

/// A terrain change a tool makes.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TerrainTransform {
    /// One of [`tool_actions`]: `till`, `transform` or `break`.
    pub action: String,
    /// Only this tool makes it; absent: every tool with the action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// The terrain it becomes; empty removes an overlay or object.
    pub into: String,
}

/// A kind of ground, path or object a tile can be, and its rules.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TerrainDefinition {
    pub id: String,
    pub name: String,
    /// One of [`tile_layers`].
    pub layer: String,
    /// `#rrggbb`: the editor's swatch, and how the game draws it without art.
    pub color: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visual: Option<VisualRef>,
    pub walkable: bool,
    /// Crops are planted on it (soil).
    pub plantable: bool,
    /// Counts as water for crops that need water nearby.
    pub water_source: bool,
    /// The fishing action works on it.
    pub fishable: bool,
    /// A crop on a tile of it gets no light.
    pub blocks_light: bool,
    /// Energy the player loses on stepping onto it.
    #[serde(with = "crate::units::energy")]
    pub hazard_energy: i32,
    /// Keeps the weather off its tile, as an indoor scene does.
    pub indoor: bool,
    pub transforms: Vec<TerrainTransform>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for TerrainDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            layer: tile_layers::BACKGROUND.to_owned(),
            color: String::new(),
            visual: None,
            walkable: true,
            plantable: false,
            water_source: false,
            fishable: false,
            blocks_light: false,
            hazard_energy: 0,
            indoor: false,
            transforms: Vec::new(),
            extra: Map::new(),
        }
    }
}

/// The area a tool covers at one tier: `width` tiles across the player's facing, `length` ahead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ToolArea {
    #[serde(with = "crate::units::count")]
    pub width: u32,
    #[serde(with = "crate::units::count")]
    pub length: u32,
}

impl Default for ToolArea {
    fn default() -> Self {
        Self { width: 1, length: 1 }
    }
}

/// A tool: what it does, what it costs and its key.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ToolDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    /// One of [`tool_actions`].
    pub action: String,
    /// The play-mode key that uses it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Energy a tier 1 use costs; each tier above costs 15% less.
    #[serde(with = "crate::units::energy")]
    pub energy_cost: i32,
    /// The area by tier (the first entry is tier 1, the last covers the tiers above).
    pub area: Vec<ToolArea>,
    /// The skill that gains experience when it breaks a node; absent: foraging.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
    /// The terrain the `place` action puts down.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub place_terrain: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl ToolDefinition {
    /// The area at `tier` (1-based): the tier's entry, the last entry for higher tiers, one tile
    /// without any.
    pub fn area_at(&self, tier: i32) -> ToolArea {
        let index = usize::try_from(tier.max(1) - 1).unwrap_or(0);
        self.area.get(index).or(self.area.last()).cloned().unwrap_or_default()
    }
}

/// A kind of item and what items of it do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ItemCategoryDefinition {
    pub id: String,
    pub name: String,
    /// Planted on soil (the item's `cropType` says what grows).
    pub plantable: bool,
    /// Planted with a seed to fertilize its soil.
    pub fertilizer: bool,
    /// Held and used as a tool: it wears down and shops repair it.
    pub equippable: bool,
    /// Eaten from the inventory for energy.
    pub edible: bool,
    /// Energy an item of it restores when eaten (the item's `energy` replaces it).
    #[serde(with = "crate::units::energy")]
    pub energy: i32,
    /// Put down in the world (a machine type names the item).
    pub placeable: bool,
    pub giftable: bool,
    pub sellable: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for ItemCategoryDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            plantable: false,
            fertilizer: false,
            equippable: false,
            edible: false,
            energy: 0,
            placeable: false,
            giftable: true,
            sellable: true,
            extra: Map::new(),
        }
    }
}

/// A crop quality tier. Tiers are ranked in list order, the first being the plain one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CropQualityDefinition {
    pub id: String,
    pub name: String,
    /// Sell price multiplier, thousandths.
    #[serde(with = "crate::units::milli")]
    pub price_multiplier: u32,
    /// Harvest yield multiplier, thousandths.
    #[serde(with = "crate::units::milli")]
    pub yield_multiplier: u32,
    /// Mutation chance multiplier, thousandths.
    #[serde(with = "crate::units::milli")]
    pub mutation_multiplier: u32,
    /// Stars shown after an item's name.
    #[serde(with = "crate::units::count")]
    pub stars: u32,
    pub color: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for CropQualityDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            price_multiplier: crate::units::MILLI_ONE,
            yield_multiplier: crate::units::MILLI_ONE,
            mutation_multiplier: crate::units::MILLI_ONE,
            stars: 0,
            color: String::new(),
            extra: Map::new(),
        }
    }
}

/// A crop mutation: a rare harvest that yields more. Mutations are checked in list order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CropMutationDefinition {
    pub id: String,
    pub name: String,
    /// Yield multiplier, thousandths.
    #[serde(with = "crate::units::milli")]
    pub yield_multiplier: u32,
    /// The part of a crop's mutation chance that lands on it, thousandths.
    #[serde(with = "crate::units::milli")]
    pub share: u32,
    pub color: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for CropMutationDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            yield_multiplier: crate::units::MILLI_ONE,
            share: 0,
            color: String::new(),
            extra: Map::new(),
        }
    }
}

/// Neighbours a crop needs around its tile (F# `NeighborKinds`).
pub mod neighbor_kinds {
    pub const CROP: &str = "crop";
    pub const TERRAIN: &str = "terrain";
    pub const WATER: &str = "water";

    pub const ALL: &[&str] = &[CROP, TERRAIN, WATER];
}

/// The light a crop needs (F# `LightNeeds`).
pub mod light_needs {
    pub const LIGHT: &str = "light";
    pub const DARK: &str = "dark";

    pub const ALL: &[&str] = &[LIGHT, DARK];
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NeighborRequirement {
    /// One of [`neighbor_kinds`].
    pub kind: String,
    /// The crop type or terrain id; unused for water.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// How far to look, in tiles.
    #[serde(with = "crate::units::count")]
    pub radius: u32,
    /// How many matching tiles are needed.
    #[serde(with = "crate::units::count")]
    pub min: u32,
}

impl Default for NeighborRequirement {
    fn default() -> Self {
        Self { kind: String::new(), id: None, radius: 1, min: 1 }
    }
}

/// What a crop needs to be planted and to grow, beyond water and its seasons.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GrowthRequirements {
    /// Terrains it can be planted on; empty: any plantable terrain.
    pub terrains: Vec<String>,
    /// One of [`light_needs`]; absent: either.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub min_temperature: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub max_temperature: Option<i32>,
    /// Scene tags it needs, all of them.
    pub scene_tags: Vec<String>,
    /// Fertilizer items it must be planted with (one of them); empty: none needed.
    pub fertilizers: Vec<String>,
    pub neighbors: Vec<NeighborRequirement>,
}

impl GrowthRequirements {
    /// Whether nothing is required.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}
