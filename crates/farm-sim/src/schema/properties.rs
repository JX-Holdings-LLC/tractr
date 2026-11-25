//! Creator-defined properties (#244): typed, bounded numbers a game keeps for the player, for
//! each scene or for each placed machine, with drain and recovery and threshold effects.
//!
//! A property is a declared variable. Its values live with the game's variables
//! (`GameState::flags`), so the existing `variable` condition, the `setVariable` and
//! `addVariable` outcomes, plugin reads and writes, test states, replay expectations and saves
//! all work with it; the declaration adds what a bare variable lacks: a start value, bounds a
//! write cannot leave, a unit, a scope (one value for the player, one per scene, one per placed
//! machine), change over time and effects when a threshold is crossed.

use super::{EventCondition, EventOutcome};
use serde::{Deserialize, Serialize};

/// Whose a property is (`PropertyDefinition::scope`).
pub mod property_scopes {
    /// One value: the player's (oxygen, heat, sanity).
    pub const PLAYER: &str = "player";
    /// One value per scene (a greenhouse's pressure, a room's power).
    pub const SCENE: &str = "scene";
    /// One value per placed machine (a tank's charge).
    pub const OBJECT: &str = "object";

    pub const ALL: &[&str] = &[PLAYER, SCENE, OBJECT];
}

/// How the HUD shows a property (`PropertyDefinition::hud`).
pub mod property_hud_styles {
    /// A bar from `min` to `max`, like energy's.
    pub const METER: &str = "meter";
    /// The number with its unit.
    pub const NUMBER: &str = "number";

    pub const ALL: &[&str] = &[METER, NUMBER];
}

/// How a threshold compares the value (`PropertyThreshold::compare`): the ordering words of the
/// `variable` condition ([`super::variable_comparisons`]).
pub mod property_comparisons {
    pub const AT_MOST: &str = "le";
    pub const BELOW: &str = "lt";
    pub const AT_LEAST: &str = "ge";
    pub const ABOVE: &str = "gt";

    pub const ALL: &[&str] = &[AT_MOST, BELOW, AT_LEAST, ABOVE];
}

fn hundred() -> i64 {
    100_000
}

/// A property of the game (#244).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PropertyDefinition {
    /// Also the variable's name in conditions and outcomes.
    pub id: String,
    pub name: String,
    /// One of [`property_scopes`].
    pub scope: String,
    /// Written after the number ("kPa", "%"); absent: none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// The least it can be.
    #[serde(with = "crate::units::signed_milli")]
    pub min: i64,
    /// The most it can be.
    #[serde(with = "crate::units::signed_milli")]
    pub max: i64,
    /// What it starts at (a scene may start at its own, `Scene::properties`).
    #[serde(with = "crate::units::signed_milli")]
    pub initial: i64,
    /// Scene scope: the scenes that have it; absent: every authored scene.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scenes: Option<Vec<String>>,
    /// Object scope: the machine types whose placed machines have it; absent: every machine.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub objects: Option<Vec<String>>,
    /// How it changes as game time passes; the rates whose conditions hold add up.
    pub rates: Vec<PropertyRate>,
    /// What happens when it crosses a value.
    pub thresholds: Vec<PropertyThreshold>,
    /// One of [`property_hud_styles`]; absent: not on the HUD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hud: Option<String>,
    /// `#rrggbb` of its HUD meter; absent: the interface's own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

impl Default for PropertyDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            scope: property_scopes::PLAYER.to_owned(),
            unit: None,
            min: 0,
            max: hundred(),
            initial: hundred(),
            scenes: None,
            objects: None,
            rates: Vec::new(),
            thresholds: Vec::new(),
            hud: None,
            color: None,
        }
    }
}

/// A change per in-game hour while its conditions hold (#244): a drain when negative, a recovery
/// when positive. A scene's or machine's property reads the conditions for that scene or machine:
/// a `variable` naming another property of the same scope means its own.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PropertyRate {
    #[serde(with = "crate::units::signed_milli")]
    pub per_hour: i64,
    /// All of them must hold; empty: always.
    pub conditions: Vec<EventCondition>,
}

/// What a property does when it crosses a value (#244): the outcomes run once each time the
/// comparison starts to hold (it has to stop holding before they run again).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PropertyThreshold {
    /// One of [`property_comparisons`].
    pub compare: String,
    #[serde(with = "crate::units::signed_milli")]
    pub value: i64,
    /// Shown when it is crossed; absent: nothing is said.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub outcomes: Vec<EventOutcome>,
}

impl Default for PropertyThreshold {
    fn default() -> Self {
        Self { compare: property_comparisons::AT_MOST.to_owned(), value: 0, message: None, outcomes: Vec::new() }
    }
}

impl PropertyThreshold {
    /// Whether `value` is on the threshold's side.
    pub fn holds(&self, value: i64) -> bool {
        match self.compare.as_str() {
            property_comparisons::BELOW => value < self.value,
            property_comparisons::AT_LEAST => value >= self.value,
            property_comparisons::ABOVE => value > self.value,
            _ => value <= self.value,
        }
    }
}

/// A range a property must be in for a crop to grow (`GrowthRequirements::properties`, #244): the
/// scene's value of a scene property, the player's of a player property.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PropertyRange {
    pub property: String,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::signed_milli::opt")]
    pub min: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::signed_milli::opt")]
    pub max: Option<i64>,
}

/// serde `with` for an optional map of property values by id (`Scene::properties`).
pub mod start_values {
    use crate::units::{self, SignedMilli};
    use indexmap::IndexMap;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &Option<IndexMap<String, i64>>, serializer: S) -> Result<S::Ok, S::Error> {
        match value {
            Some(map) => units::serialize_map::<SignedMilli, S>(map, serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<IndexMap<String, i64>>, D::Error> {
        units::deserialize_map::<SignedMilli, D>(deserializer).map(Some)
    }
}
