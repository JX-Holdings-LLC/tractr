//! Port of `Weather.cs` (packages/engine-schemas/src/weather.ts).
//! Weather (M4b) — content-defined weather types + per-season roll tables.

use crate::units;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WeatherTypeDefinition {
    pub id: String,
    pub name: String,
    /// Rain-like: outdoor soil is watered automatically at day start.
    pub waters_outdoor_soil: bool,
    /// Chance (0..1) each outdoor crop is destroyed overnight (storms).
    #[serde(with = "crate::units::probability")]
    pub crop_damage_chance: u64,
    /// NPCs with schedules stay home (skip schedule walking).
    pub npcs_stay_inside: bool,
    /// The overlay it shows: a [`WeatherOverlayDefinition`] id (`rain`, `snow` or the game's
    /// own) or null (present-as-null).
    pub overlay: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WeatherTableEntry {
    pub weather_id: String,
    /// positive.
    #[serde(with = "crate::units::count")]
    pub weight: u32,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WeatherConfig {
    pub types: Vec<WeatherTypeDefinition>,
    /// Per-season weighted roll tables (rolled at each day start).
    pub table: IndexMap<String, Vec<WeatherTableEntry>>,
    /// The game's own overlays (M1), beside and over the built-in rain and snow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlays: Option<Vec<WeatherOverlayDefinition>>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// How a weather overlay draws (F# `OverlayStyles`).
pub mod overlay_styles {
    pub const RAIN: &str = "rain";
    pub const SNOW: &str = "snow";
    pub const DUST: &str = "dust";
    pub const NONE: &str = "none";

    pub const ALL: &[&str] = &[RAIN, SNOW, DUST, NONE];
}

/// How a weather looks on screen (M1): particles and a tint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WeatherOverlayDefinition {
    pub id: String,
    /// One of [`overlay_styles`].
    pub style: String,
    /// `#rrggbb` (or `#rrggbbaa`) of the particles.
    pub color: String,
    /// Particles relative to the built-in amount.
    #[serde(with = "crate::units::screen")]
    pub density: f64,
    /// Speed relative to the built-in style.
    #[serde(with = "crate::units::screen")]
    pub speed: f64,
    /// `#rrggbb` multiplied over the scene while it falls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tint: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for WeatherOverlayDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            style: overlay_styles::RAIN.to_owned(),
            color: String::new(),
            density: 1.0,
            speed: 1.0,
            tint: None,
            extra: Map::new(),
        }
    }
}

/// The built-in overlays: rain and snow.
pub fn builtin_weather_overlays() -> Vec<WeatherOverlayDefinition> {
    vec![
        WeatherOverlayDefinition {
            id: overlay_styles::RAIN.to_owned(),
            style: overlay_styles::RAIN.to_owned(),
            color: "#d8ecff9a".to_owned(),
            tint: Some("#c8d2e6".to_owned()),
            ..WeatherOverlayDefinition::default()
        },
        WeatherOverlayDefinition {
            id: overlay_styles::SNOW.to_owned(),
            style: overlay_styles::SNOW.to_owned(),
            color: "#ffffffdc".to_owned(),
            ..WeatherOverlayDefinition::default()
        },
    ]
}

impl WeatherConfig {
    /// The overlay `id` names: the game's own, else a built-in one.
    pub fn overlay(&self, id: &str) -> Option<WeatherOverlayDefinition> {
        self.overlays
            .as_deref()
            .and_then(|own| own.iter().find(|overlay| overlay.id == id).cloned())
            .or_else(|| builtin_weather_overlays().into_iter().find(|overlay| overlay.id == id))
    }
}

fn weather_type(
    id: &str,
    name: &str,
    waters: bool,
    damage: f64,
    inside: bool,
    overlay: Option<&str>,
) -> WeatherTypeDefinition {
    WeatherTypeDefinition {
        id: id.to_owned(),
        name: name.to_owned(),
        waters_outdoor_soil: waters,
        crop_damage_chance: units::from_authoring::<units::Probability>(damage),
        npcs_stay_inside: inside,
        overlay: overlay.map(str::to_owned),
        extra: Map::new(),
    }
}

fn entry(weather_id: &str, weight: u32) -> WeatherTableEntry {
    WeatherTableEntry { weather_id: weather_id.to_owned(), weight }
}

/// The weather config projects get when they have none (TS `defaultWeatherConfig()` in
/// migrations.ts; F# `MigrationsSchema.DefaultWeatherConfig`). Content assembly uses it.
pub fn default_weather_config() -> WeatherConfig {
    let mut table = IndexMap::new();
    table.insert("spring".to_owned(), vec![entry("sun", 6), entry("rain", 3), entry("storm", 1)]);
    table.insert("summer".to_owned(), vec![entry("sun", 7), entry("rain", 1), entry("storm", 2)]);
    table.insert("fall".to_owned(), vec![entry("sun", 6), entry("rain", 3), entry("storm", 1)]);
    table.insert("winter".to_owned(), vec![entry("sun", 5), entry("snow", 5)]);
    WeatherConfig {
        types: vec![
            weather_type("sun", "Sunny", false, 0.0, false, None),
            weather_type("rain", "Rain", true, 0.0, false, Some("rain")),
            weather_type("storm", "Storm", true, 0.03, true, Some("rain")),
            weather_type("snow", "Snow", false, 0.0, false, Some("snow")),
        ],
        table,
        overlays: None,
        extra: Map::new(),
    }
}
