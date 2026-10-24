//! Port of `Animals.cs` (packages/engine-schemas/src/animals.ts).
//! Animals & ranching (M4c).

use super::farm::{AnimalBreeding, AnimalProduct, MoodQuality, ProductProgress};
use super::graphics::VisualRef;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AnimalSpeciesDefinition {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visual: Option<VisualRef>,
    /// nonnegative.
    #[serde(with = "crate::units::money")]
    pub purchase_cost: i64,
    /// Item consumed daily; absent = grazes for free.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_item_id: Option<String>,
    pub product_item_id: String,
    /// Days between products (when fed and adult). int, positive.
    #[serde(with = "crate::units::count")]
    pub product_interval_days: u32,
    /// int, nonnegative.
    #[serde(with = "crate::units::count")]
    pub days_to_adult: u32,
    /// Renderer hint.
    pub color: String,
    /// Buildings its animals live in (M5); a building that upgrades one of them counts too.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub housing: Vec<String>,
    /// Products beside `productItemId` (M5), each on its own interval.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub products: Vec<AnimalProduct>,
    /// The product quality each mood earns (M5); empty: normal quality.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mood_qualities: Vec<MoodQuality>,
    /// Young born in buildings with room (M5); absent: none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub breeding: Option<AnimalBreeding>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for AnimalSpeciesDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            visual: None,
            purchase_cost: 0,
            feed_item_id: None,
            product_item_id: String::new(),
            product_interval_days: 1,
            days_to_adult: 3,
            color: "#e8d8c3".to_owned(),
            housing: Vec::new(),
            products: Vec::new(),
            mood_qualities: Vec::new(),
            breeding: None,
            extra: Map::new(),
        }
    }
}

/// A live animal (persisted per project/save).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AnimalState {
    pub id: String,
    pub species_id: String,
    pub name: String,
    pub scene_id: String,
    /// int.
    #[serde(with = "crate::units::position")]
    pub x: i32,
    /// int.
    #[serde(with = "crate::units::position")]
    pub y: i32,
    /// 0..100; fed & petted raise it, neglect lowers it.
    #[serde(with = "crate::units::int")]
    pub mood: i32,
    pub fed_today: bool,
    pub petted_today: bool,
    /// int.
    #[serde(with = "crate::units::count")]
    pub age_days: u32,
    /// int.
    #[serde(with = "crate::units::count")]
    pub days_since_product: u32,
    /// Product waiting to be collected.
    pub product_ready: bool,
    /// The building it lives in (M5: a [`super::BuildingState`] id).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub building_id: Option<String>,
    /// Progress toward the species' `products` (M5), by index.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub more_products: Vec<ProductProgress>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for AnimalState {
    fn default() -> Self {
        Self {
            id: String::new(),
            species_id: String::new(),
            name: String::new(),
            scene_id: String::new(),
            x: 0,
            y: 0,
            mood: 70,
            fed_today: false,
            petted_today: false,
            age_days: 0,
            days_since_product: 0,
            product_ready: false,
            building_id: None,
            more_products: Vec::new(),
            extra: Map::new(),
        }
    }
}
