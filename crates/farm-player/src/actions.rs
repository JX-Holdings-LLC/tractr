//! Action animations in play (#235): the session's side of [`farm_render::actions`].
//!
//! When the player uses a tool and the player's art has an action clip for it, the session starts
//! the clip at the current tick and sends the tool's command when the clip reaches its impact
//! frame (at once when it has none); a clip that locks movement holds the player in place until
//! it ends, and further tool presses wait for it. Everything counts simulation ticks, so the
//! command reaches the simulation at the same tick at any frame rate, and a replay of the
//! commands is exact. Without action clips nothing is deferred: the game plays as before.

use farm_render::actions::{self, Action, ActionClip};
use farm_render::graphics::ArtAsset;
use farm_render::SnapshotAction;
use farm_sim::schema::VisualRef;
use farm_sim::Command;

/// An action clip playing on the player.
#[derive(Debug, Clone, PartialEq)]
pub struct Playback {
    pub clip: ActionClip,
    /// The simulation tick it started at.
    pub started: u64,
    /// The command sent at the impact tick (`None` once sent, or for display-only actions).
    pub pending: Option<Command>,
    /// The item drawn over the player (`holdItem` clips): its art.
    pub held: Option<VisualRef>,
}

impl Playback {
    /// Ticks since it started at simulation tick `now`.
    pub fn elapsed(&self, now: u64) -> u64 {
        now.saturating_sub(self.started)
    }

    /// Its command is due at tick `now`.
    pub fn due(&self, now: u64) -> bool {
        self.pending.is_some() && self.elapsed(now) >= u64::from(self.clip.impact)
    }

    /// It is over at tick `now` (and its command was sent).
    pub fn over(&self, now: u64) -> bool {
        self.pending.is_none() && self.elapsed(now) >= u64::from(self.clip.duration)
    }

    /// It holds the player in place at tick `now`.
    pub fn locks(&self, now: u64) -> bool {
        self.clip.lock_movement && !self.over(now)
    }

    /// What the renderer draws (`alpha`: how far the display is between this tick and the next).
    pub fn view(&self, now: u64, alpha: f64) -> SnapshotAction {
        let elapsed = (self.elapsed(now) as f64 + alpha.clamp(0.0, 1.0)).min(f64::from(self.clip.duration) - 1.0);
        SnapshotAction {
            clip: self.clip.name.clone(),
            elapsed: elapsed.max(0.0),
            impact: f64::from(self.clip.impact),
            effect: self.clip.effect.clone(),
            held_visual: self.held.clone().filter(|_| self.clip.hold_item),
            held: None,
        }
    }
}

/// The action clip of the player's art for `action` facing `direction`.
pub fn find(art: Option<&ArtAsset>, action: Action<'_>, direction: &str) -> Option<ActionClip> {
    actions::find(art?, action, direction)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playback(impact: u32, duration: u32, lock: bool) -> Playback {
        Playback {
            clip: ActionClip {
                name: "use-till-down".into(),
                duration,
                impact,
                lock_movement: lock,
                effect: None,
                hold_item: false,
            },
            started: 100,
            pending: Some(Command::UseTool { tool: "hoe".into() }),
            held: None,
        }
    }

    #[test]
    fn the_command_is_due_at_the_impact_tick_and_the_lock_lasts_the_clip() {
        let mut playing = playback(6, 10, true);
        assert!(!playing.due(105));
        assert!(playing.due(106));
        assert!(playing.locks(106));
        playing.pending = None;
        assert!(playing.locks(109));
        assert!(playing.over(110));
        assert!(!playing.locks(110));
        assert_eq!(playing.view(103, 0.5).elapsed, 3.5);
        assert_eq!(playing.view(130, 0.0).elapsed, 9.0, "the last frame holds");
    }
}
