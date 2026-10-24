//! Dialogue and relationships (M4): daily lines, portraits, heart events, dating, marriage,
//! spouses and children. The F# records are in `Schema.fs`.

use super::events::EventCondition;
use super::graphics::VisualRef;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A line an NPC may say when the player talks to it (M4): in place of its first dialogue's
/// text, on days when its conditions hold. Lines whose conditions hold share the day by weight.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NpcLine {
    pub text: String,
    /// The events' conditions; an empty `npcId` in `hearts` and `relationship` is this NPC.
    pub conditions: Vec<EventCondition>,
    /// The portrait the dialogue box shows with the line (one of the NPC's `portraits`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,
    /// How often the line comes up next to the others that hold (absent: 1). int.
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub weight: Option<u32>,
}

/// A portrait an NPC shows in the dialogue box for an expression (M4).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NpcPortrait {
    /// `happy`, `sad`… any name; lines, dialogues and cutscene lines pick it by this name.
    pub expression: String,
    pub visual: VisualRef,
}

/// A cutscene that plays once when the player has enough hearts with an NPC (M4).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HeartEvent {
    pub id: String,
    pub name: String,
    pub npc_id: String,
    /// Whole hearts needed. int.
    #[serde(with = "crate::units::count")]
    pub hearts: u32,
    pub cutscene_id: String,
    /// Where it plays: on entering this scene. Absent or empty: when the player talks to the NPC.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_id: Option<String>,
    /// More conditions (the events'); an empty `npcId` is the event's NPC.
    pub conditions: Vec<EventCondition>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The flag that remembers a heart event has played.
pub fn heart_event_seen_flag(heart_event_id: &str) -> String {
    format!("heart:{heart_event_id}:seen")
}

/// A game's relationship rules (M4). Every part is off until the creator turns it on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RelationshipSettings {
    /// Friendship points an NPC loses on a day the player neither talked to it nor gave it a
    /// gift (0: no decay). int.
    #[serde(with = "crate::units::count")]
    pub decay_per_day: u32,
    /// Romanceable NPCs can be dated.
    pub dating: bool,
    /// Hearts a romanceable NPC needs before it accepts the dating gift. int.
    #[serde(with = "crate::units::count")]
    pub dating_hearts: u32,
    /// The gift that asks to date (absent: dating starts at the hearts by itself).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dating_item_id: Option<String>,
    /// Dated NPCs can be married.
    pub marriage: bool,
    /// Hearts a dated NPC needs before it accepts the proposal. int.
    #[serde(with = "crate::units::count")]
    pub proposal_hearts: u32,
    /// The gift that proposes (needed for marriage).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal_item_id: Option<String>,
    /// Days from the engagement to the wedding. int.
    #[serde(with = "crate::units::count")]
    pub wedding_days: u32,
    /// Plays on the wedding morning.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wedding_cutscene_id: Option<String>,
    /// Where a spouse lives (absent: the spouse keeps its own place).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home_scene_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub home_x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub home_y: Option<i32>,
    /// A spouse helps each morning: waters the crops, feeds the animals or brings a gift.
    pub spouse_helps: bool,
    /// What a spouse may bring in the morning.
    pub spouse_gift_item_ids: Vec<String>,
    /// Married couples have children.
    pub children: bool,
    /// Days married before a child arrives, and between children. int.
    #[serde(with = "crate::units::count")]
    pub child_days: u32,
    /// int.
    #[serde(with = "crate::units::count")]
    pub max_children: u32,
    /// Names the children get, in order.
    pub child_names: Vec<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for RelationshipSettings {
    fn default() -> Self {
        Self {
            decay_per_day: 0,
            dating: false,
            dating_hearts: 8,
            dating_item_id: None,
            marriage: false,
            proposal_hearts: 10,
            proposal_item_id: None,
            wedding_days: 3,
            wedding_cutscene_id: None,
            home_scene_id: None,
            home_x: None,
            home_y: None,
            spouse_helps: true,
            spouse_gift_item_ids: Vec::new(),
            children: false,
            child_days: 14,
            max_children: 2,
            child_names: Vec::new(),
            extra: Map::new(),
        }
    }
}

/// A child of the player and their spouse (M4, game state).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ChildState {
    pub name: String,
    /// The spouse's NPC id.
    pub parent_id: String,
    #[serde(with = "crate::units::count")]
    pub born_day: u32,
}
