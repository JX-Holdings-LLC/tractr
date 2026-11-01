//! Action animations (#235): named clips of a character's art that the game picks when the
//! player does something (`use-till-down`, `fish-cast`, `eat`, …), with their timing.
//!
//! Timing is in content ticks (the frames' `ticks`), never the frame rate: a clip of frames
//! lasting 3 + 3 + 4 ticks lasts 10 ticks at any frame rate, and its impact frame starts at a
//! fixed tick. The host (farm-player's session) keeps a started action and sends the tool's
//! command at the impact tick, so replays (which record commands with their ticks) stay exact. A
//! character whose art has no action clips plays none, and the game behaves as without them.

use crate::graphics::ArtAsset;
use farm_sim::schema::{tool_actions, AnimationClip};

/// What the player is doing, to pick an action clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action<'a> {
    /// Using a tool whose action is this one of `tool_actions` (`till`, `water`, …).
    Tool(&'a str),
    /// One of the named actions: `fish-cast`, `fish-reel`, `eat`, `hold-up`, `hurt`, `sleep`.
    Named(&'a str),
}

/// The named actions the engine picks (besides `use-<toolAction>-<direction>`).
pub const NAMED_ACTIONS: &[&str] = &["fish-cast", "fish-reel", "eat", "hold-up", "hurt", "sleep"];

/// The clip names that play `action` facing `direction`, the preferred first: a tool's
/// `use-<toolAction>-<direction>`, `use-<toolAction>` (fishing rods: `fish-cast-<direction>` and
/// `fish-cast` first), a named action's `<name>-<direction>` and `<name>`; then `use-<direction>`
/// and `use` for any action. Without any of these the character keeps its walk frame.
pub fn clip_names(action: Action<'_>, direction: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut named = |name: &str| {
        names.push(format!("{name}-{direction}"));
        names.push(name.to_owned());
    };
    match action {
        Action::Tool(tool) => {
            if tool == tool_actions::FISH {
                named("fish-cast");
            }
            named(&format!("use-{tool}"));
        }
        Action::Named(name) => named(name),
    }
    names.push(format!("use-{direction}"));
    names.push("use".to_owned());
    names
}

/// An action clip and its timing, in ticks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionClip {
    pub name: String,
    /// How long it plays (at least 1).
    pub duration: u32,
    /// When the action's effect applies (the impact frame's first tick; 0: at once).
    pub impact: u32,
    /// The player stays in place while it plays.
    pub lock_movement: bool,
    /// One of `clip_effects`.
    pub effect: Option<String>,
    /// The item the action is about is drawn over the character.
    pub hold_item: bool,
}

impl ActionClip {
    /// The timing of `clip` (`None` without frames).
    pub fn of(clip: &AnimationClip) -> Option<Self> {
        if clip.frames.is_empty() {
            return None;
        }
        // Frame lengths are whole ticks (authoring numbers; at least one tick each).
        let ticks = |frame: &farm_sim::schema::ArtFrame| frame.ticks.round().clamp(1.0, f64::from(u32::MAX)) as u32;
        let duration = clip.frames.iter().map(ticks).fold(0u32, u32::saturating_add).max(1);
        let impact_frame = clip.impact_frame.map_or(0, |frame| frame as usize).min(clip.frames.len() - 1);
        let impact = clip.frames.iter().take(impact_frame).map(ticks).fold(0u32, u32::saturating_add);
        Some(Self {
            name: clip.name.clone(),
            duration,
            impact,
            lock_movement: clip.lock_movement.unwrap_or(false),
            effect: clip.effect.clone(),
            hold_item: clip.hold_item.unwrap_or(false),
        })
    }
}

/// The action clip of `asset` that plays `action` facing `direction` (see [`clip_names`]).
pub fn find(asset: &ArtAsset, action: Action<'_>, direction: &str) -> Option<ActionClip> {
    let clips = asset.animations.as_deref()?;
    clip_names(action, direction)
        .iter()
        .find_map(|name| clips.iter().find(|clip| clip.name == *name))
        .and_then(ActionClip::of)
}

/// The asset whose clips drive the player's actions: the first of its layers (#234), else its
/// art; `None` for the built-in player (the built-in pack has no action clips).
pub fn player_asset(source: &crate::GraphicsSource) -> Option<&ArtAsset> {
    let id = match source.player_layers.as_deref().and_then(<[_]>::first) {
        Some(layer) => layer.asset_id.as_str(),
        None => source.player_visual.as_ref()?.asset_id.as_str(),
    };
    source.assets.iter().find(|asset| asset.id == id)
}

/// The drawing effect of an action `elapsed` ticks in: an x offset (in sheet pixels), a
/// squash (the height kept, in thousandths) and an opacity (thousandths).
pub fn effect_at(effect: Option<&str>, elapsed: u32, impact: u32) -> (i32, u32, u32) {
    match effect {
        Some("shake") => (if (elapsed / 2).is_multiple_of(2) { 1 } else { -1 }, 1000, 1000),
        Some("squash") if elapsed >= impact && elapsed < impact.saturating_add(4) => (0, 850, 1000),
        Some("flash") if !(elapsed / 2).is_multiple_of(2) => (0, 1000, 550),
        _ => (0, 1000, 1000),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use farm_sim::schema::ArtFrame;

    fn clip(name: &str, ticks: &[f64], impact: Option<u32>) -> AnimationClip {
        AnimationClip {
            name: name.into(),
            frames: ticks
                .iter()
                .map(|&ticks| ArtFrame { width: 32.0, height: 48.0, ticks, ..ArtFrame::default() })
                .collect(),
            impact_frame: impact,
            lock_movement: Some(true),
            ..AnimationClip::default()
        }
    }

    #[test]
    fn tools_prefer_their_own_clip_then_a_generic_use_clip() {
        assert_eq!(
            clip_names(Action::Tool("till"), "left"),
            ["use-till-left", "use-till", "use-left", "use"].map(String::from)
        );
        assert_eq!(clip_names(Action::Tool("fish"), "up")[..2], ["fish-cast-up".to_owned(), "fish-cast".to_owned()]);
        assert_eq!(clip_names(Action::Named("eat"), "down")[..3], ["eat-down", "eat", "use-down"].map(String::from));
    }

    #[test]
    fn timing_comes_from_the_frames_ticks() {
        let timing = ActionClip::of(&clip("use-till-down", &[3.0, 3.0, 4.0], Some(2))).unwrap();
        assert_eq!((timing.duration, timing.impact, timing.lock_movement), (10, 6, true));
        // An impact past the end is the last frame; none is at once.
        assert_eq!(ActionClip::of(&clip("x", &[2.0, 2.0], Some(9))).unwrap().impact, 2);
        assert_eq!(ActionClip::of(&clip("x", &[2.0], None)).unwrap().impact, 0);
        assert_eq!(ActionClip::of(&clip("x", &[], None)), None);

        let asset = ArtAsset {
            id: "a".into(),
            asset_type: "player".into(),
            tile_type: None,
            image_url: std::sync::Arc::from("data:"),
            width: None,
            height: None,
            animations: Some(vec![clip("use-down", &[1.0], None), clip("use-water-left", &[5.0], None)]),
            sheet: None,
            pixels_per_tile: None,
        };
        assert_eq!(find(&asset, Action::Tool("water"), "left").unwrap().name, "use-water-left");
        assert_eq!(find(&asset, Action::Tool("water"), "down").unwrap().name, "use-down");
        assert_eq!(find(&asset, Action::Named("eat"), "up"), None);
    }

    #[test]
    fn effects_depend_only_on_the_ticks() {
        assert_eq!(effect_at(None, 5, 0), (0, 1000, 1000));
        assert_eq!(effect_at(Some("shake"), 0, 0).0, 1);
        assert_eq!(effect_at(Some("shake"), 2, 0).0, -1);
        assert_eq!(effect_at(Some("squash"), 6, 6).1, 850);
        assert_eq!(effect_at(Some("squash"), 10, 6).1, 1000);
        assert_eq!(effect_at(Some("flash"), 2, 0).2, 550);
    }
}
