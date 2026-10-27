//! Port of `World.cs` (packages/engine-schemas/src/world.ts).

use super::combat::MonsterSpawn;
use super::content::{Crop, Item};
use super::crafting::TileMachine;
use super::graphics::VisualRef;
use super::nodes::TileNode;
use super::settings::AtmosphereLook;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SceneTransition {
    #[serde(with = "crate::units::int")]
    pub from_x: i32,
    #[serde(with = "crate::units::int")]
    pub from_y: i32,
    pub to_scene_id: String,
    #[serde(with = "crate::units::int")]
    pub to_x: i32,
    #[serde(with = "crate::units::int")]
    pub to_y: i32,
    /// Locked transitions don't fire; events can lock/unlock them (M3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A light that glows at night (M8): lamps, glowing crops, lit windows. Drawn by the game, never
/// read by the simulation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LightSource {
    /// How far the light reaches, in tiles. int, 1..=16.
    #[serde(with = "crate::units::count")]
    pub radius: u32,
    /// `#rrggbb`; empty: a warm lamp light.
    pub color: String,
    /// How strong it is at its centre, in percent. int, 1..=100.
    #[serde(with = "crate::units::count")]
    pub brightness: u32,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for LightSource {
    fn default() -> Self {
        Self { radius: 3, color: String::new(), brightness: 100, extra: Map::new() }
    }
}

/// An extra layer of a scene (M8), painted with terrains like the three fixed layers. It is
/// drawn over the object layer, or (`above`) over the player and everything standing: roofs,
/// treetops, the railings of a bridge. It is scenery: it never blocks and is never farmed.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SceneLayer {
    pub id: String,
    pub name: String,
    /// Drawn over the player and everything standing; absent: over the object layer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub above: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// TS `TileSchema.visuals` (inline object): per-layer art overrides.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TileVisuals {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<VisualRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay: Option<VisualRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<VisualRef>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Tile {
    #[serde(with = "crate::units::int")]
    pub x: i32,
    #[serde(with = "crate::units::int")]
    pub y: i32,
    /// One of [`super::tile_types`].
    pub r#type: String,
    /// Background layer: grass, soil, water, floor
    pub background: String,
    /// Overlay layer: path, rug, … renders on top of background (present-as-null).
    pub overlay: Option<String>,
    /// Object layer: wall, door, … renders on top of overlay (present-as-null).
    pub object: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crop: Option<Crop>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<Item>,
    /// Gathering node instance (tree/rock/…) occupying this tile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<TileNode>,
    /// Placed machine instance (M4 crafting).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine: Option<TileMachine>,
    /// Mine ladder going down (M4 mining, generated floors).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ladder_down: Option<bool>,
    pub collision: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visuals: Option<TileVisuals>,
    /// One of [`super::soil_states`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soil_state: Option<String>,
    #[serde(with = "crate::units::int")]
    pub soil_moisture: i32,
    #[serde(with = "crate::units::int")]
    pub soil_fertility: i32,
    /// The terrain painted on each of the scene's extra layers ([`Scene::layers`], M8), by
    /// layer id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decor: Option<IndexMap<String, String>>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Scene {
    pub id: String,
    pub name: String,
    /// int, positive.
    #[serde(with = "crate::units::int")]
    pub width: i32,
    /// int, positive.
    #[serde(with = "crate::units::int")]
    pub height: i32,
    /// Row-major: `tiles[y][x]`.
    pub tiles: Vec<Vec<Tile>>,
    pub transitions: Vec<SceneTransition>,
    pub npcs: Vec<String>,
    pub events: Vec<String>,
    /// Indoor scenes (greenhouses, interiors, generated mine floors) keep the weather out: rain
    /// does not water their soil and storms do not damage their crops. Absent means outdoor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indoor: Option<bool>,
    /// Set on scenes the engine made during play (mine floors): they are not authored, never
    /// sync back into the editor's scene lists, and `exitMine` drops them.
    // Written as a plain boolean, as when it was an untyped key: the state hash encodes an
    // `Option` with a tag byte.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "plain_bool")]
    pub generated: Option<bool>,
    /// No daylight reaches its crops (caves), for crops that need light or dark (M1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dark: Option<bool>,
    /// How warm it is, for crops with a temperature range (M1); absent: the season's.
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub temperature: Option<i32>,
    /// Free-form tags crops can require.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Its own day/night and foliage tints (M1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub atmosphere: Option<AtmosphereLook>,
    /// A music asset that plays in the scene (M2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub music: Option<String>,
    /// Crops are planted and grow here in every season (a greenhouse, M5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all_seasons: Option<bool>,
    /// The building whose interior this is (M5: a [`super::BuildingState`] id).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub building: Option<String>,
    /// Monsters it has each morning (M7).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monsters: Option<Vec<MonsterSpawn>>,
    /// Its extra layers beyond background, overlay and object, in drawing order (M8).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layers: Option<Vec<SceneLayer>>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// An optional flag that is only written when present, as its bare value.
fn plain_bool<S: serde::Serializer>(value: &Option<bool>, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_bool(value.unwrap_or(false))
}

/// The largest scene side, in tiles: the editor's size limit, and what the engine accepts when a
/// game or save loads (larger scenes are cut down to it).
pub const MAX_SCENE_SIZE: i32 = 512;

impl Scene {
    /// Whether `(x, y)` lies inside the scene's declared size.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= 0 && x < self.width && y >= 0 && y < self.height
    }

    /// The tile at `(x, y)`: `None` outside the declared size or where the grid has no tile (a
    /// ragged grid that was never normalized). Simulation code reads tiles through this and
    /// [`Scene::tile_mut`] instead of indexing `tiles[y][x]`, so a malformed grid cannot panic.
    pub fn tile(&self, x: i32, y: i32) -> Option<&Tile> {
        if !self.contains(x, y) {
            return None;
        }
        self.tiles.get(y as usize)?.get(x as usize)
    }

    /// [`Scene::tile`], mutable.
    pub fn tile_mut(&mut self, x: i32, y: i32) -> Option<&mut Tile> {
        if !self.contains(x, y) {
            return None;
        }
        self.tiles.get_mut(y as usize)?.get_mut(x as usize)
    }

    /// Whether this scene was generated during play (see [`Scene::generated`]).
    pub fn is_generated(&self) -> bool {
        self.generated == Some(true)
    }

    /// Is the scene sheltered from the weather ([`Scene::indoor`])?
    pub fn is_indoor(&self) -> bool {
        self.indoor == Some(true)
    }

    /// Does no daylight reach the scene ([`Scene::dark`])?
    pub fn is_dark(&self) -> bool {
        self.dark == Some(true)
    }

    /// Do crops grow here in every season ([`Scene::all_seasons`])?
    pub fn is_all_seasons(&self) -> bool {
        self.all_seasons == Some(true)
    }

    /// Whether the scene carries `tag` ([`Scene::tags`]).
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.as_deref().is_some_and(|tags| tags.iter().any(|t| t == tag))
    }
}
