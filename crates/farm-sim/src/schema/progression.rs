//! Optional permanent player progression; absent fields retain legacy game hashes.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProgressionLimits {
    #[serde(with = "crate::units::count")]
    pub max_inventory_size: u32,
    #[serde(with = "crate::units::energy")]
    pub max_energy: i32,
    #[serde(with = "crate::units::int")]
    pub max_health: i32,
}

impl Default for ProgressionLimits {
    fn default() -> Self {
        Self { max_inventory_size: 120, max_energy: crate::units::points(1000), max_health: 1000 }
    }
}

/// Training action and XP; an absent action grants no XP for an authored skill.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SkillTraining {
    pub action: String,
    /// Optional item/node/NPC/recipe/monster id for this action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(with = "crate::units::count")]
    pub experience: u32,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SkillReward {
    #[serde(with = "crate::units::count")]
    pub level: u32,
    pub stat: String,
    #[serde(with = "crate::units::int")]
    pub amount: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcomes: Option<Vec<super::EventOutcome>>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SkillDefinition {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub training: Vec<SkillTraining>,
    pub rewards: Vec<SkillReward>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub perks: Option<Vec<SkillPerk>>,
}

/// One timed stat modifier. Amount is whole points, or percent for speed/cost/luck/escape.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BuffDefinition {
    pub id: String,
    pub stat: String,
    #[serde(with = "crate::units::int")]
    pub amount: i32,
    #[serde(with = "crate::units::count")]
    pub duration_minutes: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "super::is_false")]
    pub persists_after_sleep: bool,
    #[serde(skip_serializing_if = "super::is_false")]
    pub additive: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ActiveBuff {
    pub definition: BuffDefinition,
    pub source: String,
    #[serde(with = "crate::units::absolute_micro_minutes")]
    pub expires_at_minute: i64,
}

/// One-time permanent shop reward, identified by a stable id within its shop.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlayerUpgrade {
    pub id: String,
    pub kind: String,
    #[serde(with = "crate::units::int")]
    pub amount: i32,
}

/// A blacksmith's explicit tier-to-tier recipe.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ToolUpgrade {
    pub from_item_id: String,
    pub to_item_id: String,
    #[serde(with = "crate::units::money")]
    pub cost: i64,
    pub materials: Vec<super::RecipeIngredient>,
    #[serde(with = "crate::units::count")]
    pub nights: u32,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PendingToolUpgrade {
    pub shop_id: String,
    pub item: super::Item,
    #[serde(with = "crate::units::count")]
    pub slot: u32,
    #[serde(with = "crate::units::count")]
    pub ready_day: u32,
    #[serde(skip_serializing_if = "super::is_false")]
    pub was_equipped: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SkillPerk {
    pub id: String,
    #[serde(with = "crate::units::count")]
    pub level: u32,
    pub choices: Vec<SkillPerkChoice>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SkillPerkChoice {
    pub id: String,
    pub name: String,
    pub stat: String,
    #[serde(with = "crate::units::int")]
    pub amount: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcomes: Option<Vec<super::EventOutcome>>,
}
