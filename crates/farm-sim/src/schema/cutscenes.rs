//! Cutscenes (M2): steps that run over time. The F# records are in `Schema.fs`.

use super::events::{EventCondition, EventOutcome};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// What a cutscene step does (F# `CutsceneStepTypes`).
pub mod cutscene_step_types {
    pub const SAY: &str = "say";
    pub const CHOICE: &str = "choice";
    pub const JUMP: &str = "jump";
    pub const MOVE: &str = "move";
    pub const FACE: &str = "face";
    pub const EMOTE: &str = "emote";
    pub const WAIT: &str = "wait";
    pub const CAMERA: &str = "camera";
    pub const FADE: &str = "fade";
    pub const MUSIC: &str = "music";
    pub const SOUND: &str = "sound";
    pub const OUTCOMES: &str = "outcomes";
    pub const END: &str = "end";

    pub const ALL: &[&str] = &[SAY, CHOICE, JUMP, MOVE, FACE, EMOTE, WAIT, CAMERA, FADE, MUSIC, SOUND, OUTCOMES, END];
}

/// The emotes a cutscene can show over a character (F# `Emotes`).
pub mod emotes {
    pub const ALL: &[&str] = &["exclaim", "question", "heart", "music", "anger", "sweat", "sleep", "idea"];
}

/// A cutscene: steps that run over time, started by the `startCutscene` outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Cutscene {
    pub id: String,
    pub name: String,
    pub steps: Vec<CutsceneStep>,
    /// The player may skip it; `outcomes` steps still run.
    pub skippable: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Cutscene {
    fn default() -> Self {
        Self { id: String::new(), name: String::new(), steps: Vec::new(), skippable: true, extra: Map::new() }
    }
}

/// One answer of a `choice` step.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CutsceneChoice {
    pub text: String,
    /// The label of the step to go on from; empty: the next step.
    pub goto: String,
}

/// One step of a cutscene: a flat object whose `type` (one of [`cutscene_step_types`]) says
/// which fields apply.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CutsceneStep {
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// `player` or an NPC id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    /// Tile column.
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub x: Option<i32>,
    /// Tile row.
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub y: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emote: Option<String>,
    /// Seconds, in thousandths.
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::milli::opt")]
    pub seconds: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
    pub options: Vec<CutsceneChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goto: Option<String>,
    pub conditions: Vec<EventCondition>,
    /// `out` or `in` (fade).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound_id: Option<String>,
    pub outcomes: Vec<EventOutcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait: Option<bool>,
}

/// A cutscene being played (game state). The engine's cutscene runner owns what it holds
/// beyond the cutscene and the step.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CutsceneState {
    pub cutscene_id: String,
    /// Index of the running step.
    #[serde(with = "crate::units::count")]
    pub step: u32,
}
