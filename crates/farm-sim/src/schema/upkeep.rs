//! Creator tooling around a shipped game (#229, #242): test states a playtest starts from and the
//! save migration rules a game update carries.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A named game state a playtest or a headless run starts from (#229): applied after New Game,
/// on the current content, so it survives content edits. Every field is optional: what a test
/// state leaves out stays as New Game made it. Stored in the project's `testStates` (never
/// compiled into a cartridge) or in a file of its own (`farm-player --test-state`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TestState {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Absolute day (1 is the first day of the first year); the season, its day and the year
    /// follow from it unless they are given too.
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub day: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub day_of_season: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub year: Option<u32>,
    /// Minute of the day (`480` is 8:00), kept inside the game's day.
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub time: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weather: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_id: Option<String>,
    /// The tile the player stands on (the scene's centre when absent).
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub y: Option<i32>,
    /// One of [`super::directions`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facing: Option<String>,
    /// Flags set (`true`) or cleared (`false`).
    pub flags: IndexMap<String, Value>,
    /// Variables (flags holding a number or a text).
    pub variables: IndexMap<String, Value>,
    /// Quest statuses by quest id: one of [`super::quest_statuses`]. Set as they are: rewards
    /// and outcomes do not run (the test state lists what they would give).
    pub quests: IndexMap<String, String>,
    /// Friendship points by NPC id (125 a heart).
    #[serde(with = "crate::units::int::map")]
    pub friendship: IndexMap<String, i32>,
    /// The whole inventory, replacing New Game's; absent: New Game's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inventory: Option<Vec<TestStateItem>>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::money::opt")]
    pub money: Option<i64>,
}

/// An inventory line of a [`TestState`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TestStateItem {
    pub item_id: String,
    #[serde(with = "crate::units::count")]
    pub quantity: u32,
    /// A crop quality other than normal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
}

impl Default for TestStateItem {
    fn default() -> Self {
        Self { item_id: String::new(), quantity: 1, quality: None }
    }
}

/// What a [`SaveMigration`] renames (#242).
pub mod save_migration_kinds {
    pub const ITEM: &str = "item";
    pub const SCENE: &str = "scene";

    pub const ALL: &[&str] = &[ITEM, SCENE];
}

/// A rule a game update carries for saves written before it (#242), applied when such a save
/// loads, before ids the game no longer has go to quarantine. A rule applies only while the game
/// has no content under its `from` id.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SaveMigration {
    /// One of [`save_migration_kinds`].
    pub kind: String,
    /// The id old saves hold.
    pub from: String,
    /// The id that takes its place; absent: removed (an item is then paid for with `money`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// Units of `to` for each unit of `from` (items; 1 when absent).
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub quantity: Option<u32>,
    /// Gold paid for each unit of a removed item (items without `to`).
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::money::opt")]
    pub money: Option<i64>,
    /// Where a player who was in a moved scene lands (scenes; their own tile when absent).
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub y: Option<i32>,
    /// The old name, for the notice a player reads (a save's item carries its own).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
