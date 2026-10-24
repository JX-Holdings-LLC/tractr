//! Port of `GameContent.cs` (packages/engine-schemas/src/game-content.ts).
//!
//! Content family — everything that defines a game and is immutable during play. Content comes
//! from the built-in pack + the project's custom content (+ enabled mods from M5), merged and
//! validated at load time.

use super::actors::{Dialogue, Npc};
use super::animals::AnimalSpeciesDefinition;
use super::combat::MonsterDefinition;
use super::content::{CropDefinition, Item};
use super::crafting::{MachineTypeDefinition, RecipeDefinition};
use super::cutscenes::Cutscene;
use super::economy::ShopDefinition;
use super::events::GameEvent;
use super::extensibility::{ActionDef, MinigameDef};
use super::farm::BuildingDefinition;
use super::fishing::FishTable;
use super::goals::{AchievementDefinition, BundleDefinition, CollectionDefinition, MailDefinition};
use super::mining::MineConfig;
use super::nodes::NodeTypeDefinition;
use super::quests::Quest;
use super::relationships::HeartEvent;
use super::settings::ProjectSettings;
use super::vocabulary::{
    CropMutationDefinition, CropQualityDefinition, ItemCategoryDefinition, TerrainDefinition, ToolDefinition,
};
use super::weather::WeatherConfig;
use super::world::Scene;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GameContent {
    /// int.
    #[serde(with = "crate::units::count")]
    pub content_version: u32,
    /// Crop definitions by id (built-in merged with project custom crops).
    pub crops: IndexMap<String, CropDefinition>,
    pub items: Vec<Item>,
    pub npcs: Vec<Npc>,
    pub dialogues: Vec<Dialogue>,
    pub quests: Vec<Quest>,
    pub events: Vec<GameEvent>,
    pub shops: Vec<ShopDefinition>,
    pub node_types: Vec<NodeTypeDefinition>,
    pub settings: ProjectSettings,
    pub recipes: Vec<RecipeDefinition>,
    pub machine_types: Vec<MachineTypeDefinition>,
    pub weather: WeatherConfig,
    pub animal_species: Vec<AnimalSpeciesDefinition>,
    pub fish_tables: Vec<FishTable>,
    pub mine: MineConfig,
    /// Creator-defined actions (extensibility layer).
    pub actions: Vec<ActionDef>,
    /// Declared minigames (extensibility layer).
    pub minigames: Vec<MinigameDef>,
    /// Authored initial scenes — the template the world state is created from.
    pub scenes: Vec<Scene>,
    pub start_scene_id: String,
    /// Terrains (built-in merged with the project's).
    pub terrains: Vec<TerrainDefinition>,
    /// Tools (built-in merged with the project's).
    pub tools: Vec<ToolDefinition>,
    /// Item categories (built-in merged with the project's).
    pub item_categories: Vec<ItemCategoryDefinition>,
    /// Crop quality tiers, ranked (built-in merged with the project's).
    pub crop_qualities: Vec<CropQualityDefinition>,
    /// Crop mutations, in roll order (built-in merged with the project's).
    pub crop_mutations: Vec<CropMutationDefinition>,
    /// Cutscenes (M2).
    pub cutscenes: Vec<Cutscene>,
    /// Heart events (M4).
    pub heart_events: Vec<HeartEvent>,
    /// Buildings a builder puts up (M5).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub building_types: Vec<BuildingDefinition>,
    /// Collections (M6).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub collections: Vec<CollectionDefinition>,
    /// Bundles (M6).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub bundles: Vec<BundleDefinition>,
    /// Achievements (M6).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub achievements: Vec<AchievementDefinition>,
    /// Letters (M6).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mail: Vec<MailDefinition>,
    /// Monsters (M7).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub monsters: Vec<MonsterDefinition>,
}

pub const CURRENT_CONTENT_VERSION: u32 = 1;
