//! Text substitution (M4): placeholders a creator writes in game text, filled in when the text
//! is shown.
//!
//! | Placeholder | Becomes |
//! |---|---|
//! | `{player}` | the player's name |
//! | `{farm}` | the farm's name |
//! | `{spouse}` | the spouse's name (empty when single) |
//! | `{npc:ID}` | the name of NPC `ID` |
//! | `{item:ID}` | the name of item `ID` |
//! | `{var:NAME}` | the value of variable (flag) `NAME` |
//! | `{season}` | the season's name |
//! | `{day}` | the day of the season |
//! | `{year}` | the year |
//! | `{date}` | the season and the day (`Spring 3`) |
//! | `{time}` | the time of day as the clock shows it |
//! | `{money}` | the player's money |
//!
//! Anything else in braces stays as it was written, so text with braces of its own is safe.
//! Dialogue (lines, texts and options), cutscene lines and choices, event and action messages,
//! the quest log and game panels are filled in.

use crate::game_time;
use crate::relationships;
use crate::schema::{GameContent, GameState};
use crate::text;
use std::borrow::Cow;

/// `text` with its placeholders filled in from `state` (borrowed when it has none).
pub fn fill<'t>(content: &GameContent, state: &GameState, text: &'t str) -> Cow<'t, str> {
    if !text.contains('{') {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + 16);
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('}').and_then(|end| value(content, state, &after[..end]).map(|value| (value, end))) {
            Some((value, end)) => {
                out.push_str(&value);
                rest = &after[end + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    Cow::Owned(out)
}

/// What a placeholder's name stands for, or `None` when it is not one.
fn value(content: &GameContent, state: &GameState, name: &str) -> Option<String> {
    let calendar = &content.settings.calendar;
    let season_name = || {
        game_time::season_by_id(calendar, &state.clock.season)
            .map_or_else(|| state.clock.season.clone(), |season| season.name.clone())
    };
    let day_of_season = || game_time::clock_date(calendar, &state.clock).day_of_season;
    match name {
        "player" => Some(state.player.name.clone()),
        "farm" => Some(state.player.farm_name.clone()),
        "spouse" => Some(relationships::spouse(state).map(|id| npc_name(content, id)).unwrap_or_default()),
        "season" => Some(season_name()),
        "day" => Some(day_of_season().to_string()),
        "year" => Some(state.clock.year.to_string()),
        "date" => Some(format!("{} {}", season_name(), day_of_season())),
        "time" => {
            Some(game_time::format_time_of_day_in(content.settings.time.minutes_per_day(), state.clock.time_minutes))
        }
        "money" => Some(state.player.money.to_string()),
        _ => {
            let (kind, id) = name.split_once(':')?;
            match kind {
                "npc" => content.npcs.iter().find(|npc| npc.id == id).map(|npc| npc.name.clone()),
                "item" => content.items.iter().find(|item| item.id == id).map(|item| item.name.clone()),
                "var" => Some(state.flags.get(id).map(text::to_js_string).unwrap_or_default()),
                _ => None,
            }
        }
    }
}

/// The name of NPC `id` (its id when the content lacks it).
fn npc_name(content: &GameContent, id: &str) -> String {
    content.npcs.iter().find(|npc| npc.id == id).map_or_else(|| id.to_owned(), |npc| npc.name.clone())
}
