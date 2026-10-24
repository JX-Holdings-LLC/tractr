//! Building the farm (M5): what placed objects do, buildings and their interiors, animal
//! housing, farm layouts and the farm settings. Placeable objects are machine types
//! ([`super::MachineTypeDefinition`]) with storage, a gate or area effects.

use super::actors::GridPoint;
use super::content::InventorySlot;
use super::crafting::RecipeIngredient;
use super::graphics::VisualRef;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// What a placed object does to the tiles around it (M5).
pub mod area_effects {
    /// Each morning the soil in the area is watered (a sprinkler).
    pub const WATER: &str = "water";
    /// Overnight, crops in the area escape storm damage and pests (a scarecrow).
    pub const PROTECT: &str = "protect";

    pub const ALL: &[&str] = &[WATER, PROTECT];
}

/// An effect of a placed object on the square of tiles `radius` around it, in its scene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AreaEffect {
    /// One of [`area_effects`].
    pub effect: String,
    /// Tiles in each direction (1: the 3×3 square around the object). int, 0..32.
    #[serde(with = "crate::units::count")]
    pub radius: u32,
}

impl Default for AreaEffect {
    fn default() -> Self {
        Self { effect: area_effects::WATER.to_owned(), radius: 1 }
    }
}

/// The largest area effect radius the engine applies (a larger one is cut down to it).
pub const MAX_AREA_RADIUS: u32 = 32;

/// The slots a shipping bin holds when its type sets no `storage`.
pub const DEFAULT_SHIPPING_SLOTS: u32 = 24;

/// The largest storage an object has (a larger `storage` is cut down to it).
pub const MAX_STORAGE_SLOTS: u32 = 256;

/// An open chest or shipping bin (M5), a modal like a shop: the placed object on tile `(x, y)`
/// of `sceneId`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StorageSession {
    pub scene_id: String,
    #[serde(with = "crate::units::int")]
    pub x: i32,
    #[serde(with = "crate::units::int")]
    pub y: i32,
}

/// A building a builder puts up (M5): a footprint on a farm scene with a door into an interior
/// scene made from a template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BuildingDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visual: Option<VisualRef>,
    /// `#rrggbb`: how the game draws it without art.
    pub color: String,
    /// Footprint in tiles. int, 1..16.
    #[serde(with = "crate::units::count")]
    pub width: u32,
    #[serde(with = "crate::units::count")]
    pub height: u32,
    /// The door's tile within the footprint (from its top-left corner); absent: the middle of
    /// the bottom row.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub door: Option<GridPoint>,
    /// The scene each building of this kind gets a copy of as its interior; absent: no interior
    /// (a silo, a well).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interior_scene_id: Option<String>,
    /// The interior's door: the tile that leads back out; the player arrives on the tile above
    /// it. Absent: the middle of the interior's bottom row.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interior_door: Option<GridPoint>,
    /// What building it costs. nonnegative.
    #[serde(with = "crate::units::money")]
    pub cost: i64,
    pub materials: Vec<RecipeIngredient>,
    /// Nights of building before it stands; 0: done the next morning all the same.
    #[serde(with = "crate::units::count")]
    pub build_days: u32,
    /// Scenes it may be built in; empty: the farm scene (`settings.farm.sceneId`), else the
    /// start scene.
    pub scenes: Vec<String>,
    /// Animals it houses.
    #[serde(with = "crate::units::count")]
    pub capacity: u32,
    /// The building this one is an upgrade of: it is never built new, only by upgrading one of
    /// those (the house upgrades the same way).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upgrade_of: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for BuildingDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            visual: None,
            color: "#b5651d".to_owned(),
            width: 3,
            height: 2,
            door: None,
            interior_scene_id: None,
            interior_door: None,
            cost: 0,
            materials: Vec::new(),
            build_days: 2,
            scenes: Vec::new(),
            capacity: 0,
            upgrade_of: None,
            extra: Map::new(),
        }
    }
}

/// The largest building side the engine places, in tiles.
pub const MAX_BUILDING_SIZE: u32 = 16;

impl BuildingDefinition {
    /// The footprint, cut down to [`MAX_BUILDING_SIZE`] and at least one tile.
    pub fn size(&self) -> (i32, i32) {
        let side = |value: u32| value.clamp(1, MAX_BUILDING_SIZE) as i32;
        (side(self.width), side(self.height))
    }

    /// The door's offset within the footprint (clamped into it).
    pub fn door_offset(&self) -> (i32, i32) {
        let (width, height) = self.size();
        match &self.door {
            Some(door) => (door.x.clamp(0, width - 1), door.y.clamp(0, height - 1)),
            None => (width / 2, height - 1),
        }
    }
}

/// A building on the map (M5), from the project's start (`buildings`) or built in play.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BuildingState {
    /// Unique among the game's buildings; built ones are `building-N`.
    pub id: String,
    /// Its [`BuildingDefinition`].
    pub building_id: String,
    pub scene_id: String,
    /// The footprint's top-left tile.
    #[serde(with = "crate::units::int")]
    pub x: i32,
    #[serde(with = "crate::units::int")]
    pub y: i32,
    /// The scene behind its door (empty: none yet, or no interior).
    pub interior_scene_id: String,
    /// Nights of building left; 0: it stands.
    #[serde(with = "crate::units::count")]
    pub days_left: u32,
    /// The building it becomes when the work under way (`daysLeft`) is done (an upgrade).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upgrade_to: Option<String>,
}

impl BuildingState {
    /// Whether it stands (no construction under way, an upgrade aside).
    pub fn built(&self) -> bool {
        self.days_left == 0 || self.upgrade_to.is_some()
    }
}

/// One more product of an animal species (M5) beside its `productItemId`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AnimalProduct {
    pub item_id: String,
    /// Days between products. int, positive.
    #[serde(with = "crate::units::count")]
    pub interval_days: u32,
    /// The mood (0..100) the animal needs for it.
    #[serde(with = "crate::units::int")]
    pub min_mood: i32,
}

impl Default for AnimalProduct {
    fn default() -> Self {
        Self { item_id: String::new(), interval_days: 1, min_mood: 30 }
    }
}

/// A product quality an animal's mood earns (M5): the best tier whose `minMood` it has.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MoodQuality {
    #[serde(with = "crate::units::int")]
    pub min_mood: i32,
    /// A crop quality id.
    pub quality: String,
}

/// How animals of a species have young (M5): each night an adult in a building with room may.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AnimalBreeding {
    /// Chance per night, 0..1.
    #[serde(with = "crate::units::probability")]
    pub chance: u64,
    #[serde(with = "crate::units::int")]
    pub min_mood: i32,
}

impl Default for AnimalBreeding {
    fn default() -> Self {
        Self { chance: crate::units::from_authoring::<crate::units::Probability>(0.05), min_mood: 70 }
    }
}

/// Progress toward one of [`super::AnimalSpeciesDefinition::products`].
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProductProgress {
    #[serde(with = "crate::units::count")]
    pub days: u32,
    pub ready: bool,
}

/// A farm map a new game can start with (M5).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FarmLayout {
    pub id: String,
    pub name: String,
    pub description: String,
    /// The scene that becomes the farm.
    pub scene_id: String,
}

/// The farm (M5): its scene, the layouts a new game picks from, and pests.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FarmSettings {
    /// The farm scene: buildings go here by default, and a layout replaces it.
    pub scene_id: String,
    /// Farm maps a new game picks from (two or more make the character creator ask).
    pub layouts: Vec<FarmLayout>,
    /// Chance per night that pests eat an outdoor crop no object protects, 0..1.
    #[serde(with = "crate::units::probability")]
    pub pest_chance: u64,
}

/// The contents of a placed object's storage that a pick-up or a layout must not lose.
pub fn storage_is_empty(contents: &[InventorySlot]) -> bool {
    contents.iter().all(|slot| slot.quantity == 0)
}
