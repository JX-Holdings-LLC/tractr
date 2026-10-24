//! Cutscenes (M2): steps that run over time. The F# records are in `Schema.fs`.

use super::actors::GridPoint;
use super::events::{EventCondition, EventOutcome};
use indexmap::IndexMap;
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
    /// The speaker's portrait for a `say` line (one of the NPC's `portraits`, M4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,
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

/// A cutscene being played (game state). The engine's cutscene runner (`crate::cutscenes`) owns
/// what it holds beyond the cutscene and the step. Rust only: the editor never reads it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CutsceneState {
    pub cutscene_id: String,
    /// Index of the running step.
    #[serde(with = "crate::units::count")]
    pub step: u32,
    /// The running step has begun: its instant part ran and its timer counts.
    #[serde(skip_serializing_if = "is_false")]
    pub started: bool,
    /// Ticks since the running step began.
    #[serde(skip_serializing_if = "is_zero", with = "crate::units::ticks")]
    pub elapsed: u64,
    /// Where the camera looks; absent: on the player, as outside cutscenes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera: Option<CutsceneCamera>,
    /// The screen's fade; absent: clear.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fade: Option<CutsceneFade>,
    /// Actors walking to a `move` step's tile, in the order they set off.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub walks: Vec<CutsceneWalk>,
    /// Emotes shown over actors.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub emotes: Vec<CutsceneEmote>,
    /// The way NPCs face, set by `face` steps and walks (the player's is its own direction).
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub facing: IndexMap<String, String>,
}

/// A camera pan: from a point to a tile or onto an actor, then held there.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CutsceneCamera {
    /// Where the pan starts: the centre of the view, in position units.
    #[serde(with = "crate::units::position")]
    pub from_x: i32,
    #[serde(with = "crate::units::position")]
    pub from_y: i32,
    /// Where it goes (a tile's centre), unless it follows `actor`.
    #[serde(with = "crate::units::position")]
    pub to_x: i32,
    #[serde(with = "crate::units::position")]
    pub to_y: i32,
    /// The actor it pans onto and then follows (`player` or an NPC id).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(with = "crate::units::ticks")]
    pub duration: u64,
    #[serde(with = "crate::units::ticks")]
    pub elapsed: u64,
}

/// A fade of the whole screen to a colour (`out`) or back (`in`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CutsceneFade {
    /// `#rrggbb`.
    pub color: String,
    /// Opacity at the start and the end, in thousandths.
    #[serde(with = "crate::units::milli")]
    pub from: u32,
    #[serde(with = "crate::units::milli")]
    pub to: u32,
    #[serde(with = "crate::units::ticks")]
    pub duration: u64,
    #[serde(with = "crate::units::ticks")]
    pub elapsed: u64,
}

/// An actor walking a path in the scene it set off in.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CutsceneWalk {
    pub actor: String,
    pub scene_id: String,
    /// The tiles still to reach, the next first; the last is the target.
    pub path: Vec<GridPoint>,
}

/// An emote over an actor.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CutsceneEmote {
    pub actor: String,
    /// One of [`emotes::ALL`].
    pub emote: String,
    #[serde(with = "crate::units::ticks")]
    pub duration: u64,
    #[serde(with = "crate::units::ticks")]
    pub elapsed: u64,
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_zero(value: &u64) -> bool {
    *value == 0
}
