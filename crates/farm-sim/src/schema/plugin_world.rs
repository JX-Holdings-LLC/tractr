//! What plugins keep in the world (docs/PLUGINS.md "Entities" and "Timers"): entities that
//! stand on a tile and timers that run a plugin handler later. Both are saved with the game
//! and change only through plugin mutations and the engine's own clock, so a replay of the
//! command log reaches the same entities and fires the same timers without the plugins.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Most plugin entities a game holds; spawning more is refused.
pub const MAX_PLUGIN_ENTITIES: usize = 512;

/// Most timers one plugin may have running; starting more is refused.
pub const MAX_PLUGIN_TIMERS: usize = 64;

/// Longest entity state or timer data, as compact JSON, in bytes.
pub const MAX_PLUGIN_STATE_BYTES: usize = 4096;

/// The prefix of a project script's plugin id (`script/<id>`, #241). Pack ids cannot hold a
/// `/`, so no pack plugin's id starts with it.
pub const PROJECT_SCRIPT_PREFIX: &str = "script/";

/// Whether plugin `plugin_id` may change entities of type `type_id`: a pack's plugins change
/// their own pack's types (`packId:…`), project scripts any.
pub fn may_change_entity(plugin_id: &str, type_id: &str) -> bool {
    if plugin_id.starts_with(PROJECT_SCRIPT_PREFIX) {
        return true;
    }
    let pack = plugin_id.split_once(':').map_or(plugin_id, |(pack, _)| pack);
    type_id.split_once(':').is_some_and(|(owner, _)| owner == pack)
}

/// The prefix of an item's `useActionId` that names a plugin handler (`plugin:packId:name`)
/// instead of an action.
pub const PLUGIN_USE_PREFIX: &str = "plugin:";

/// A plugin entity (#238): a thing of a type a plugin defines, standing on a tile. The engine
/// saves and draws it; the plugin that defines its type hears `onTick` (each game minute),
/// `onInteract` (the player uses it) and `onDayStart`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PluginEntity {
    /// `entity-N`, unique in the game.
    pub id: String,
    /// The entity type, `packId:name`; its plugin's handlers run for it.
    pub type_id: String,
    pub scene_id: String,
    /// The tile it stands on.
    #[serde(with = "crate::units::int")]
    pub x: i32,
    #[serde(with = "crate::units::int")]
    pub y: i32,
    /// What it looks like: an item id, drawn with the art of the item's kind; absent:
    /// a plain marker.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub art: Option<String>,
    /// The plugin's own state for it (any JSON, at most [`MAX_PLUGIN_STATE_BYTES`]).
    #[serde(skip_serializing_if = "Value::is_null")]
    pub state: Value,
}

/// A plugin timer (#238): runs the `timer` handler of `plugin_id` once the clock reaches
/// `due_at_minute`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PluginTimer {
    /// The plugin that started it (`packId:pluginId`).
    pub plugin_id: String,
    /// The plugin's name for it; starting a timer of the same name restarts it.
    pub id: String,
    /// When it fires: game time since day 1 (the clock of buffs and machines).
    #[serde(with = "crate::units::absolute_micro_minutes")]
    pub due_at_minute: i64,
    /// What the handler gets back (any JSON, at most [`MAX_PLUGIN_STATE_BYTES`]).
    #[serde(skip_serializing_if = "Value::is_null")]
    pub data: Value,
}

/// A project script (#241): JavaScript of the game itself, with the API of a pack plugin but
/// trusted: every hook, mutation and read, the game's ids without namespacing, and budgets
/// from [`ScriptSettings`]. Its plugin id is `script/<id>`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectScript {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The body of `function (api) { … }`.
    pub source: String,
    /// Off: the script does not run.
    #[serde(skip_serializing_if = "super::is_false")]
    pub disabled: bool,
}

/// Budgets of a project's scripts (#241), in fuel (the plugin unit, docs/PLUGINS.md "Limits");
/// absent fields keep the pack plugin defaults.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScriptSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fuel_per_call: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fuel_per_second: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub init_fuel: Option<u64>,
}
