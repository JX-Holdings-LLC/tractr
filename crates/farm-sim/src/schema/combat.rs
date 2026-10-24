//! Combat and adventure (M7, optional per game): player health, monsters, and what the mine's
//! depth bands spawn besides rocks.

use super::graphics::VisualRef;
use super::nodes::NodeDrop;
use crate::units;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Player health and fainting (M7). Absent or not `enabled`: no health, and monsters are not
/// spawned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CombatSettings {
    pub enabled: bool,
    /// Whole points. int, positive.
    #[serde(with = "crate::units::int")]
    pub max_health: i32,
    /// Money lost on fainting. nonnegative.
    #[serde(with = "crate::units::money")]
    pub faint_money_penalty: i64,
    /// Where the player wakes after fainting; absent: the start scene.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub faint_scene_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub faint_x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub faint_y: Option<i32>,
    /// Ticks after a hit during which the player takes no more damage. int.
    #[serde(with = "crate::units::count")]
    pub hurt_ticks: u32,
}

impl Default for CombatSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            max_health: 100,
            faint_money_penalty: 0,
            faint_scene_id: None,
            faint_x: None,
            faint_y: None,
            hurt_ticks: 30,
        }
    }
}

/// How a monster moves (M7).
pub mod monster_movements {
    /// Ambles about.
    pub const WANDER: &str = "wander";
    /// Wanders until the player comes within `sight`, then comes at them.
    pub const CHASE: &str = "chase";
    /// Stays where it is (a turret, a slime that waits).
    pub const STILL: &str = "still";

    pub const ALL: &[&str] = &[WANDER, CHASE, STILL];
}

/// A monster (M7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MonsterDefinition {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visual: Option<VisualRef>,
    /// `#rrggbb`: how the game draws it without art.
    pub color: String,
    /// Hit points. int, positive.
    #[serde(with = "crate::units::int")]
    pub health: i32,
    /// Health the player loses when it touches them. int, nonnegative.
    #[serde(with = "crate::units::int")]
    pub damage: i32,
    /// Taken off every hit it takes (a hit always does at least 1). int, nonnegative.
    #[serde(with = "crate::units::int")]
    pub defense: i32,
    /// Tiles per second.
    #[serde(with = "crate::units::speed")]
    pub speed: i32,
    /// One of [`monster_movements`].
    pub movement: String,
    /// How near (tiles) the player must come for a chaser to notice them. int.
    #[serde(with = "crate::units::count")]
    pub sight: u32,
    /// Rolled when it is slain, like a node's drops.
    pub drops: Vec<NodeDrop>,
    /// Combat experience for slaying it.
    #[serde(with = "crate::units::count")]
    pub experience: u32,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for MonsterDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            visual: None,
            color: "#6a8f3a".to_owned(),
            health: 10,
            damage: 5,
            defense: 0,
            speed: units::from_authoring::<units::Speed>(2.0),
            movement: monster_movements::CHASE.to_owned(),
            sight: 5,
            drops: Vec::new(),
            experience: 5,
            extra: Map::new(),
        }
    }
}

/// Monsters a scene has each morning (M7): `count` of a monster on free tiles of the area
/// (`x..=x2`, `y..=y2`; absent ends: the whole scene).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MonsterSpawn {
    pub monster_id: String,
    #[serde(with = "crate::units::count")]
    pub count: u32,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub y: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub x2: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub y2: Option<i32>,
}

impl Default for MonsterSpawn {
    fn default() -> Self {
        Self { monster_id: String::new(), count: 1, x: None, y: None, x2: None, y2: None }
    }
}

/// A weighted monster of a mine depth band (M7).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MineMonsterWeight {
    pub monster_id: String,
    #[serde(with = "crate::units::count")]
    pub weight: u32,
}

/// A hazard terrain a mine depth band scatters over its floors (M7), on the overlay layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MineHazard {
    pub terrain_id: String,
    /// Fraction of free floor tiles, 0..1.
    #[serde(with = "crate::units::milli")]
    pub density: u32,
}

impl Default for MineHazard {
    fn default() -> Self {
        Self { terrain_id: String::new(), density: 50 }
    }
}

/// A weighted treasure of a mine depth band (M7): an item left on a floor tile.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MineTreasure {
    pub item_id: String,
    #[serde(with = "crate::units::count")]
    pub weight: u32,
}

/// A monster in the world (M7).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MonsterState {
    /// Unique among the live monsters.
    #[serde(with = "crate::units::count")]
    pub id: u32,
    pub monster_id: String,
    pub scene_id: String,
    /// Its centre, in position units.
    #[serde(with = "crate::units::position")]
    pub x: i32,
    #[serde(with = "crate::units::position")]
    pub y: i32,
    #[serde(with = "crate::units::int")]
    pub health: i32,
    /// Ticks until it changes its wandering direction.
    #[serde(with = "crate::units::count")]
    pub wander: u32,
    /// One of [`super::directions`], or empty while it stands.
    pub direction: String,
    /// Ticks it reels from a hit (it does not move).
    #[serde(with = "crate::units::count")]
    pub stunned: u32,
}
