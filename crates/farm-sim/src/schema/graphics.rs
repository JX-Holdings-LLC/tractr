//! Port of `Graphics.cs` (packages/engine-schemas/src/graphics.ts).

use serde::{Deserialize, Serialize};

/// Rectangles are in source pixels; display size remains independent of art resolution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArtFrame {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<String>,
    /// int, nonnegative.
    #[serde(with = "crate::units::screen")]
    pub x: f64,
    /// int, nonnegative.
    #[serde(with = "crate::units::screen")]
    pub y: f64,
    /// int, positive.
    #[serde(with = "crate::units::screen")]
    pub width: f64,
    /// int, positive.
    #[serde(with = "crate::units::screen")]
    pub height: f64,
    /// int, positive.
    #[serde(with = "crate::units::screen")]
    pub ticks: f64,
}

impl Default for ArtFrame {
    fn default() -> Self {
        Self { asset_id: None, x: 0.0, y: 0.0, width: 0.0, height: 0.0, ticks: 6.0 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AnimationClip {
    /// min length 1.
    pub name: String,
    pub r#loop: bool,
    /// 1..1024 frames.
    pub frames: Vec<ArtFrame>,
    /// An action clip (#235: `use-till-down`, `eat`, …) keeps the player in place until it ends.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lock_movement: Option<bool>,
    /// An action clip's impact frame (0 = the first): the tool's effect applies when it shows;
    /// absent: at once. int, nonnegative.
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub impact_frame: Option<u32>,
    /// A drawing effect while the clip plays: one of [`clip_effects`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect: Option<String>,
    /// The item the action is about (the one eaten or held up) is drawn over the character.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold_item: Option<bool>,
}

impl Default for AnimationClip {
    fn default() -> Self {
        Self {
            name: String::new(),
            r#loop: true,
            frames: Vec::new(),
            lock_movement: None,
            impact_frame: None,
            effect: None,
            hold_item: None,
        }
    }
}

/// Drawing effects of an action clip (#235, F# `ClipEffects`).
pub mod clip_effects {
    /// Blinks lighter every other frame.
    pub const FLASH: &str = "flash";
    /// Shakes a pixel left and right.
    pub const SHAKE: &str = "shake";
    /// Squashes down at the impact frame.
    pub const SQUASH: &str = "squash";

    pub const ALL: &[&str] = &[FLASH, SHAKE, SQUASH];
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VisualRef {
    pub asset_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<ArtFrame>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GraphicsSettings {
    pub pixel_art: bool,
}

impl Default for GraphicsSettings {
    fn default() -> Self {
        Self { pixel_art: true }
    }
}
