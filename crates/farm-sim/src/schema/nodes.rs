//! Port of `Nodes.cs` (packages/engine-schemas/src/nodes.ts).

use super::graphics::VisualRef;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Weighted drop-table entry for a gathering node.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NodeDrop {
    pub item_id: String,
    /// int, nonnegative.
    #[serde(with = "crate::units::count")]
    pub min: u32,
    /// int, nonnegative.
    #[serde(with = "crate::units::count")]
    pub max: u32,
    /// positive.
    #[serde(with = "crate::units::count")]
    pub weight: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seasons: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A gathering node type (tree, rock, weeds, …) — content-defined so mods and projects can add
/// their own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NodeTypeDefinition {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visual: Option<VisualRef>,
    /// Number of tool hits required to break the node. int, positive.
    #[serde(with = "crate::units::int")]
    pub health: i32,
    /// One of [`super::tool_types`].
    pub required_tool: String,
    /// Minimum tool tier required (tools default to tier 1). int, positive.
    #[serde(with = "crate::units::int")]
    pub required_tool_tier: i32,
    /// Weighted drop table; each hit that depletes the node rolls once per entry range.
    pub drops: Vec<NodeDrop>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seasons: Option<Vec<String>>,
    /// Days until a depleted node respawns; null/absent = never. int, positive.
    /// TS is `.nullable().optional()` and the two hash differently, so absent (`None`, the
    /// default, as in hand-written packs) and `null` (`Some(None)`, as the built-in content
    /// spells it) stay apart. Read it with [`NodeTypeDefinition::respawn_after`].
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::nullable")]
    pub respawn_days: Option<Option<u32>>,
    /// Renderer hint (hex color).
    pub color: String,
    /// Whether the node blocks movement while present.
    pub blocks_movement: bool,
    /// The light it gives at night while standing (M8: glowing crystals, a lantern tree).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light: Option<super::world::LightSource>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl NodeTypeDefinition {
    /// Days until a depleted node respawns, or `None` for never (absent or null alike).
    pub fn respawn_after(&self) -> Option<u32> {
        self.respawn_days.flatten()
    }
}

impl Default for NodeTypeDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            visual: None,
            health: 0,
            required_tool: String::new(),
            required_tool_tier: 1,
            drops: Vec::new(),
            seasons: None,
            respawn_days: None,
            color: "#7a5a3a".to_owned(),
            blocks_movement: true,
            light: None,
            extra: Map::new(),
        }
    }
}

/// Live node instance state stored on a tile.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TileNode {
    pub type_id: String,
    /// int.
    #[serde(with = "crate::units::int")]
    pub remaining_health: i32,
    /// Set when depleted; used for respawn scheduling. int.
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub depleted_on_day: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub dormant_health: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub spawned_on_day: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spawned_season: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub expires_on_day: Option<u32>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A weighted seasonal source of fresh gathering nodes each morning.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ForageSpawnTable {
    pub id: String,
    pub entries: Vec<super::MineRockWeight>,
    #[serde(with = "crate::units::count")]
    pub maximum: u32,
    pub terrains: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seasons: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<ForageRegion>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub lifetime_days: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ForageRegion {
    #[serde(with = "crate::units::int")]
    pub x: i32,
    #[serde(with = "crate::units::int")]
    pub y: i32,
    #[serde(with = "crate::units::int")]
    pub width: i32,
    #[serde(with = "crate::units::int")]
    pub height: i32,
}

impl Default for ForageRegion {
    fn default() -> Self {
        Self { x: 0, y: 0, width: 1, height: 1 }
    }
}
