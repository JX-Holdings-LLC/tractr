//! Port of `Fishing.cs` (packages/engine-schemas/src/fishing.ts).
//! Fishing (M4e).

use crate::units;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FishTableEntry {
    pub item_id: String,
    /// positive.
    #[serde(with = "crate::units::count")]
    pub weight: u32,
    /// 0..1 — harder fish escape low-tier rods more often.
    #[serde(with = "crate::units::probability")]
    pub difficulty: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<FishingRules>,
}

impl Default for FishTableEntry {
    fn default() -> Self {
        Self {
            item_id: String::new(),
            weight: 0,
            difficulty: units::from_authoring::<units::Probability>(0.3),
            rules: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FishTable {
    pub id: String,
    pub name: String,
    /// Restrict to seasons (absent = all).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seasons: Option<Vec<String>>,
    /// Restrict to scenes (absent = any water).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_ids: Option<Vec<String>>,
    pub entries: Vec<FishTableEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<FishingFilter>,
    /// Opt in to combining every matching table. Absent keeps the legacy first match.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layered: Option<bool>,
    /// Chance (0..1) a cast catches junk instead of rolling the table.
    #[serde(with = "crate::units::probability")]
    pub junk_chance: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub junk_item_id: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for FishTable {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            seasons: None,
            scene_ids: None,
            entries: Vec::new(),
            filter: None,
            layered: None,
            junk_chance: units::from_authoring::<units::Probability>(0.15),
            junk_item_id: None,
            extra: Map::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FishingFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seasons: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weather_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub start_minute: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub end_minute: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<super::EventCondition>,
}
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FishingRules {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<FishingFilter>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub xp: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub min_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub max_size: Option<u32>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FishingGear {
    /// bait or tackle; the first inventory stack of each kind is used per landed catch.
    pub kind: String,
    pub fish_ids: Vec<String>,
    #[serde(with = "crate::units::count")]
    pub weight_bonus: u32,
    #[serde(with = "crate::units::count")]
    pub escape_reduction: u32,
    #[serde(with = "crate::units::count")]
    pub quality_bonus: u32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FishProduction {
    /// A trap uses a table; a pond produces its one species.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub species_item_id: Option<String>,
    #[serde(with = "crate::units::count")]
    pub every_days: u32,
    #[serde(with = "crate::units::count")]
    pub quantity: u32,
}
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FishCatchRecord {
    pub item_id: String,
    pub size: u32,
    pub day: u32,
}

impl Default for FishingGear {
    fn default() -> Self {
        Self { kind: "bait".into(), fish_ids: Vec::new(), weight_bonus: 0, escape_reduction: 0, quality_bonus: 0 }
    }
}
impl Default for FishProduction {
    fn default() -> Self {
        Self { table_id: None, species_item_id: None, every_days: 1, quantity: 1 }
    }
}
