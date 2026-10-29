//! Port of `Economy.cs` (packages/engine-schemas/src/economy.ts).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A single line in a shop's stock list.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShopStockEntry {
    pub item_id: String,
    /// Override price; defaults to the item's base value.
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::money::opt")]
    pub price: Option<i64>,
    /// Restrict availability to these seasons (empty/absent = always).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seasons: Option<Vec<String>>,
    /// Max units purchasable per in-game day (absent = unlimited). int, positive.
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub daily_limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upgrade: Option<super::PlayerUpgrade>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShopHours {
    #[serde(with = "crate::units::count")]
    pub open: u32,
    #[serde(with = "crate::units::count")]
    pub close: u32,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShopClosedOn {
    pub weekdays: Vec<String>,
    pub seasons: Vec<String>,
    pub festivals: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShopPriceModifier {
    pub seasons: Vec<String>,
    pub weekdays: Vec<String>,
    pub conditions: Vec<super::EventCondition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::milli::opt")]
    pub buy_multiplier: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::milli::opt")]
    pub sell_multiplier: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShopSaturation {
    #[serde(with = "crate::units::count")]
    pub quantity_per_step: u32,
    #[serde(with = "crate::units::milli")]
    pub reduction_per_step: u32,
    #[serde(with = "crate::units::milli")]
    pub minimum_multiplier: u32,
    #[serde(with = "crate::units::count")]
    pub recovery_days: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShopDefinition {
    pub id: String,
    pub name: String,
    pub stock: Vec<ShopStockEntry>,
    /// Multiplier applied to item base value when the player sells here.
    #[serde(with = "crate::units::milli")]
    pub sell_price_multiplier: u32,
    /// Whether this shop buys player items at all.
    pub buys_items: bool,
    /// Whether this shop repairs broken tools (for durability * repairCostPerPoint).
    pub repairs_tools: bool,
    #[serde(with = "crate::units::milli")]
    pub repair_cost_per_point: u32,
    /// Buildings it builds and upgrades (M5): a builder.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub buildings: Vec<String>,
    /// Animal species it sells (M5).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub animals: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_upgrades: Option<Vec<super::ToolUpgrade>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hours: Option<ShopHours>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed_on: Option<ShopClosedOn>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keeper_npc_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_modifiers: Option<Vec<ShopPriceModifier>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saturation: Option<ShopSaturation>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for ShopDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            stock: Vec::new(),
            sell_price_multiplier: 1000,
            buys_items: true,
            repairs_tools: false,
            repair_cost_per_point: 500,
            buildings: Vec::new(),
            animals: Vec::new(),
            tool_upgrades: None,
            hours: None,
            closed_on: None,
            keeper_npc_id: None,
            price_modifiers: None,
            saturation: None,
            extra: Map::new(),
        }
    }
}

impl Default for ShopHours {
    fn default() -> Self {
        Self { open: 540, close: 1020 }
    }
}

impl Default for ShopSaturation {
    fn default() -> Self {
        Self { quantity_per_step: 10, reduction_per_step: 100, minimum_multiplier: 250, recovery_days: 3 }
    }
}
