//! Long-term goals (M6): the shipping log, collections, bundles, achievements and mail.

use super::crafting::RecipeIngredient;
use super::events::{EventCondition, EventOutcome};
use super::graphics::VisualRef;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// What a shipping bin sold one night (M6).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShippedLine {
    pub item_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    #[serde(with = "crate::units::count")]
    pub quantity: u32,
    #[serde(with = "crate::units::money")]
    pub money: i64,
}

/// How an item joins a collection (M6).
pub mod collection_kinds {
    /// The first time the player holds it.
    pub const FOUND: &str = "found";
    /// The first time a shipping bin sells it.
    pub const SHIPPED: &str = "shipped";
    /// When the player hands one in (`donateItem`; it is taken).
    pub const DONATED: &str = "donated";

    pub const ALL: &[&str] = &[FOUND, SHIPPED, DONATED];
}

/// A reward a collection gives once it holds `count` items (M6).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CollectionReward {
    #[serde(with = "crate::units::count")]
    pub count: u32,
    /// Shown when it is given; empty: none.
    pub message: String,
    pub outcomes: Vec<EventOutcome>,
}

/// A museum, catalogue or other list of items to find (M6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CollectionDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub items: Vec<String>,
    /// One of [`collection_kinds`].
    pub kind: String,
    /// Donations are taken only in this scene (the museum); absent: anywhere.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_id: Option<String>,
    /// In order of `count`; each is given once.
    pub rewards: Vec<CollectionReward>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for CollectionDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            items: Vec::new(),
            kind: collection_kinds::FOUND.to_owned(),
            scene_id: None,
            rewards: Vec::new(),
            extra: Map::new(),
        }
    }
}

/// A collection's progress.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CollectionProgress {
    /// The items it holds, in the order they came.
    pub items: Vec<String>,
    /// How many of its rewards were given.
    #[serde(with = "crate::units::count")]
    pub rewarded: u32,
}

/// A line of a bundle (M6): `quantity` of an item, of `quality` or better.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BundleItem {
    pub item_id: String,
    #[serde(with = "crate::units::count")]
    pub quantity: u32,
    /// The lowest crop quality it takes; absent: any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
}

impl Default for BundleItem {
    fn default() -> Self {
        Self { item_id: String::new(), quantity: 1, quality: None }
    }
}

/// A Community Center-style list of items to hand in (M6); done, it runs its outcomes (rewards,
/// flags, an unlocked door: the world fixed).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BundleDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    /// The room or board it belongs to (the game lists bundles by group).
    pub group: String,
    /// Items are handed in only in this scene; absent: anywhere.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_id: Option<String>,
    pub items: Vec<BundleItem>,
    /// Lines that complete it; absent: all of them.
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub required: Option<u32>,
    /// Shown when it is complete; empty: a built-in message.
    pub message: String,
    pub outcomes: Vec<EventOutcome>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl BundleDefinition {
    /// The lines that complete it (at least one, at most every line).
    pub fn lines_needed(&self) -> usize {
        let all = self.items.len();
        self.required.map_or(all, |required| (required as usize).max(1)).min(all)
    }
}

/// A bundle's progress.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BundleProgress {
    /// Units handed in, by line.
    #[serde(with = "crate::units::count::vec")]
    pub handed: Vec<u32>,
    pub complete: bool,
}

/// Something to achieve (M6): unlocked the moment its conditions hold.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AchievementDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub conditions: Vec<EventCondition>,
    /// Shown as "???" until unlocked.
    pub hidden: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visual: Option<VisualRef>,
    /// Its id on Steam, for an export with Steamworks (later).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steam_id: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A letter (M6). It arrives in the morning: after a `sendMail` outcome or a quest that sends
/// it, or by itself on a morning its `conditions` hold (an NPC at some hearts, a birthday).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MailDefinition {
    pub id: String,
    pub subject: String,
    /// The NPC who writes it; empty: no sender (or the text says).
    pub from: String,
    pub text: String,
    /// Items in the envelope, taken when it is read.
    pub attachments: Vec<RecipeIngredient>,
    #[serde(with = "crate::units::money")]
    pub money: i64,
    /// Run when it is first read.
    pub outcomes: Vec<EventOutcome>,
    /// It arrives by itself on a morning these hold; empty: only when sent.
    pub conditions: Vec<EventCondition>,
    /// It may come again (each morning its conditions hold; a birthday letter every year).
    pub repeat: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A letter in the mailbox.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MailState {
    pub mail_id: String,
    /// The day it arrived.
    #[serde(with = "crate::units::count")]
    pub day: u32,
    pub read: bool,
    /// Its attachments were taken (or it had none).
    pub taken: bool,
}
