//! Test states and debug operations (#222, #229): what a playtest, a screenshot run or a headless
//! replay starts from, and the creator shortcuts a replay may take to reach a late state quickly.
//!
//! Both are creator tooling, not gameplay: they change the state directly, outside the command
//! log, like the editor's debug drawer. They are deterministic (no randomness, no clock), so a
//! replay that uses them still reproduces its hash.
//!
//! A [`TestState`] is applied to a new game of the *current* content: it names content by id,
//! and what the content no longer has is skipped with a note (the editor's Problems panel lists
//! the same names before a playtest).

use crate::engine_types::{Effects, EngineContext};
use crate::game_time::{self, SleepOptions};
use crate::inventory;
use crate::schema::{
    directions, quest_statuses, GameState, MoveIntent, QuestProgress, TestState, TestStateItem, MAX_FRIENDSHIP,
};
use crate::units;
use crate::world::world_movement;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Applies `test` to `state` (a new game of `ctx`'s content, or a running one): date and time,
/// place, quest statuses, flags and variables, friendship, inventory and money, in that order.
/// Returns a note for each part naming content the game does not have (that part is skipped).
pub fn apply_test_state(ctx: &EngineContext, state: &mut GameState, test: &TestState) -> Vec<String> {
    let mut notes = Vec::new();
    apply_date(ctx, state, test, &mut notes);
    if let Some(weather) = test.weather.as_deref().filter(|w| !w.is_empty()) {
        weather.clone_into(&mut state.clock.weather_id);
    }
    apply_place(state, test, &mut notes);
    for (quest_id, status) in &test.quests {
        if !ctx.content.quests.iter().any(|quest| &quest.id == quest_id) {
            notes.push(format!("No quest '{quest_id}'."));
            continue;
        }
        if !quest_statuses::ALL.contains(&status.as_str()) {
            notes.push(format!("Quest '{quest_id}': unknown status '{status}'."));
            continue;
        }
        set_quest_status(state, quest_id, status);
    }
    for (flag, value) in test.flags.iter().chain(&test.variables) {
        let flag = flag.trim();
        if flag.is_empty() {
            continue;
        }
        match value {
            Value::Bool(false) | Value::Null => {
                state.flags.shift_remove(flag);
            }
            Value::Bool(_) | Value::String(_) => {
                state.flags.insert(flag.to_owned(), value.clone());
            }
            Value::Number(_) => {
                state.flags.insert(flag.to_owned(), units::canonical_json(value.clone()));
            }
            _ => notes.push(format!("Flag '{flag}': a value must be true, false, a number or a text.")),
        }
    }
    for (npc_id, points) in &test.friendship {
        if !ctx.content.npcs.iter().any(|npc| &npc.id == npc_id) {
            notes.push(format!("No NPC '{npc_id}'."));
            continue;
        }
        state.social.entry(npc_id.clone()).or_default().friendship = (*points).clamp(-MAX_FRIENDSHIP, MAX_FRIENDSHIP);
    }
    if let Some(lines) = &test.inventory {
        state.player.inventory = Vec::new();
        for line in lines {
            give_item(ctx, state, line, &mut notes);
        }
    }
    if let Some(money) = test.money {
        state.player.money = money.max(0);
    }
    notes
}

fn apply_date(ctx: &EngineContext, state: &mut GameState, test: &TestState, notes: &mut Vec<String>) {
    let calendar = &ctx.content.settings.calendar;
    let seasons = game_time::calendar_seasons(calendar);
    let year_length: u32 = seasons.iter().map(|season| season.days).sum::<u32>().max(1);
    let clock = &mut state.clock;
    if let Some(day) = test.day {
        let day = day.max(1);
        let date = game_time::natural_date(calendar, day);
        clock.day = day;
        date.season.id.clone_into(&mut clock.season);
        clock.day_of_season = date.day_of_season;
        clock.year = (day - 1) / year_length + 1;
    }
    let season = match test.season.as_deref().filter(|id| !id.is_empty()) {
        Some(id) if seasons.iter().any(|season| season.id == id) => Some(id),
        Some(id) => {
            notes.push(format!("No season '{id}' in the calendar."));
            None
        }
        None => None,
    };
    if season.is_some() || test.day_of_season.is_some() || test.year.is_some() {
        let season_id = season.map_or_else(|| clock.season.clone(), str::to_owned);
        let index = seasons.iter().position(|s| s.id == season_id).unwrap_or(0);
        let days = seasons.get(index).map_or(1, |s| s.days.max(1));
        let day_of_season = match test.day_of_season {
            Some(day) => day,
            // A season of its own starts on its first day.
            None if season.is_some() && test.day.is_none() => 1,
            None => clock.day_of_season,
        }
        .clamp(1, days);
        let year = test.year.unwrap_or(clock.year).max(1);
        let before: u32 = seasons.iter().take(index).map(|s| s.days).sum();
        clock.season = season_id;
        clock.day_of_season = day_of_season;
        clock.year = year;
        clock.day = (year - 1).saturating_mul(year_length).saturating_add(before).saturating_add(day_of_season);
    }
    game_time::reconcile_clock(calendar, clock);
    if let Some(minute) = test.time {
        let time = &ctx.content.settings.time;
        let (start, end) = (time.day_start_minute, time.day_end_minute.max(time.day_start_minute));
        clock.time_minutes = units::minutes(minute.clamp(start, end));
    }
}

fn apply_place(state: &mut GameState, test: &TestState, notes: &mut Vec<String>) {
    let scene_id = match test.scene_id.as_deref().filter(|id| !id.is_empty()) {
        Some(id) => id.to_owned(),
        None if test.x.is_some() || test.y.is_some() => state.player.scene_id.clone(),
        None => {
            set_facing(state, test, notes);
            return;
        }
    };
    let Some(scene) = state.world.scenes.iter().find(|scene| scene.id == scene_id) else {
        notes.push(format!("No scene '{scene_id}'."));
        set_facing(state, test, notes);
        return;
    };
    let (mut x, mut y) = (test.x.unwrap_or(scene.width.div_euclid(2)), test.y.unwrap_or(scene.height.div_euclid(2)));
    if !(0..scene.width).contains(&x) || !(0..scene.height).contains(&y) {
        notes.push(format!("Tile ({x}, {y}) is outside scene '{scene_id}' ({}×{}).", scene.width, scene.height));
        x = x.clamp(0, (scene.width - 1).max(0));
        y = y.clamp(0, (scene.height - 1).max(0));
    }
    // The same move to another scene as a door's (an event's music ends), standing still.
    world_movement::put_player(state, &scene_id, units::tile_center(x), units::tile_center(y));
    state.player.move_intent = MoveIntent::default();
    set_facing(state, test, notes);
}

fn set_facing(state: &mut GameState, test: &TestState, notes: &mut Vec<String>) {
    if let Some(facing) = test.facing.as_deref() {
        if directions::ALL.contains(&facing) {
            facing.clone_into(&mut state.player.direction);
        } else {
            notes.push(format!("Facing '{facing}' is not up, down, left or right."));
        }
    }
}

/// Puts `quest_id` in `status` as it is, without rewards or outcomes.
fn set_quest_status(state: &mut GameState, quest_id: &str, status: &str) {
    let player = &mut state.player;
    player.active_quests.retain(|id| id != quest_id);
    player.completed_quests.retain(|id| id != quest_id);
    if status == quest_statuses::NOT_STARTED {
        state.quests.shift_remove(quest_id);
        return;
    }
    match status {
        quest_statuses::ACTIVE => player.active_quests.push(quest_id.to_owned()),
        quest_statuses::COMPLETED => player.completed_quests.push(quest_id.to_owned()),
        _ => {}
    }
    let progress = state
        .quests
        .entry(quest_id.to_owned())
        .or_insert_with(|| QuestProgress { objectives: Some(IndexMap::new()), ..QuestProgress::default() });
    status.clone_into(&mut progress.status);
}

fn give_item(ctx: &EngineContext, state: &mut GameState, line: &TestStateItem, notes: &mut Vec<String>) {
    let Some(item) = ctx.item(&line.item_id) else {
        notes.push(format!("No item '{}'.", line.item_id));
        return;
    };
    let player = &state.player;
    let added = inventory::add_item_with_quality(
        &player.inventory,
        item,
        line.quality.as_deref(),
        line.quantity,
        player.max_inventory_size,
        None,
    );
    let rejected = added.rejected(line.quantity);
    state.player.inventory = added.inventory;
    if rejected > 0 {
        notes.push(format!("The inventory is full: {rejected} of '{}' did not fit.", line.item_id));
    }
}

/// The status a state gives `quest_id` (one of [`quest_statuses`]).
pub fn quest_status(state: &GameState, quest_id: &str) -> &'static str {
    if state.player.active_quests.iter().any(|id| id == quest_id) {
        return quest_statuses::ACTIVE;
    }
    if state.player.completed_quests.iter().any(|id| id == quest_id) {
        return quest_statuses::COMPLETED;
    }
    match state.quests.get(quest_id).map(|progress| progress.status.as_str()) {
        Some(quest_statuses::FAILED) => quest_statuses::FAILED,
        Some(quest_statuses::ACTIVE) => quest_statuses::ACTIVE,
        Some(quest_statuses::COMPLETED) => quest_statuses::COMPLETED,
        _ => quest_statuses::NOT_STARTED,
    }
}

/// A test state that puts a new game where `state` is (#229, "save the current playtest"):
/// date, time, weather, place and facing, flags and variables, the statuses of the quests that
/// started, friendship, inventory and money.
pub fn capture_test_state(ctx: &EngineContext, state: &GameState, id: &str, name: &str) -> TestState {
    let clock = &state.clock;
    let player = &state.player;
    let (mut flags, mut variables) = (IndexMap::new(), IndexMap::new());
    for (flag, value) in &state.flags {
        match value {
            Value::Bool(_) => flags.insert(flag.clone(), value.clone()),
            _ => variables.insert(flag.clone(), value.clone()),
        };
    }
    let quests = ctx
        .content
        .quests
        .iter()
        .map(|quest| (quest.id.clone(), quest_status(state, &quest.id)))
        .filter(|(_, status)| *status != quest_statuses::NOT_STARTED)
        .map(|(id, status)| (id, status.to_owned()))
        .collect();
    let friendship = state
        .social
        .iter()
        .filter(|(_, social)| social.friendship != 0)
        .map(|(npc, social)| (npc.clone(), social.friendship))
        .collect();
    TestState {
        id: id.to_owned(),
        name: name.to_owned(),
        description: None,
        day: Some(clock.day),
        season: Some(clock.season.clone()).filter(|season| !season.is_empty()),
        day_of_season: Some(clock.day_of_season),
        year: Some(clock.year),
        time: Some(units::whole_minute(clock.time_minutes)),
        weather: Some(clock.weather_id.clone()),
        scene_id: Some(player.scene_id.clone()),
        x: Some(units::tile_of(player.x)),
        y: Some(units::tile_of(player.y)),
        facing: Some(player.direction.clone()),
        flags,
        variables,
        quests,
        friendship,
        inventory: Some(
            player
                .inventory
                .iter()
                .map(|slot| TestStateItem {
                    item_id: slot.item.id.clone(),
                    quantity: slot.quantity,
                    quality: slot.quality.clone(),
                })
                .collect(),
        ),
        money: Some(player.money),
    }
}

/// A creator shortcut a headless replay may take (`{"kind":"debug","action":{…}}` in
/// farm-player's replay files, #222).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum DebugOp {
    /// Adds items to the inventory (stack caps and the slot limit hold).
    #[serde(rename_all = "camelCase")]
    GiveItem {
        item_id: String,
        #[serde(default = "one", with = "crate::units::count")]
        quantity: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        quality: Option<String>,
    },
    /// Sleeps through `days` nights (1 when absent), wherever the player is.
    SkipDay {
        #[serde(default = "one", with = "crate::units::count")]
        days: u32,
    },
    SetMoney {
        #[serde(with = "crate::units::money")]
        amount: i64,
    },
    AddMoney {
        #[serde(with = "crate::units::money")]
        amount: i64,
    },
    /// Sets a flag or a variable (`true` when `value` is absent; `false` clears it).
    SetFlag {
        flag: String,
        #[serde(default = "yes")]
        value: Value,
    },
    #[serde(rename_all = "camelCase")]
    SetQuest { quest_id: String, status: String },
    #[serde(rename_all = "camelCase")]
    SetFriendship {
        npc_id: String,
        #[serde(with = "crate::units::int")]
        points: i32,
    },
    /// Moves the player to a tile (the scene's centre when `x` and `y` are absent).
    #[serde(rename_all = "camelCase")]
    Teleport {
        scene_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
        x: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
        y: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        facing: Option<String>,
    },
    /// Applies a whole test state.
    ApplyTestState { state: Box<TestState> },
}

fn one() -> u32 {
    1
}

fn yes() -> Value {
    Value::Bool(true)
}

/// Applies a debug operation. Returns the effects of the nights slept and a note for each part
/// naming content the game does not have.
pub fn apply_debug(ctx: &EngineContext, state: &mut GameState, op: &DebugOp) -> (Effects, Vec<String>) {
    let mut notes = Vec::new();
    let mut effects = Vec::new();
    let test = match op {
        DebugOp::GiveItem { item_id, quantity, quality } => {
            let line = TestStateItem { item_id: item_id.clone(), quantity: *quantity, quality: quality.clone() };
            give_item(ctx, state, &line, &mut notes);
            return (effects, notes);
        }
        DebugOp::SkipDay { days } => {
            for _ in 0..*days {
                effects.extend(game_time::perform_sleep(ctx, state, SleepOptions { collapsed: false }));
            }
            return (effects, notes);
        }
        DebugOp::AddMoney { amount } => {
            state.player.money = state.player.money.saturating_add(*amount).max(0);
            return (effects, notes);
        }
        DebugOp::SetMoney { amount } => TestState { money: Some(*amount), ..TestState::default() },
        DebugOp::SetFlag { flag, value } => {
            TestState { flags: IndexMap::from([(flag.clone(), value.clone())]), ..TestState::default() }
        }
        DebugOp::SetQuest { quest_id, status } => {
            TestState { quests: IndexMap::from([(quest_id.clone(), status.clone())]), ..TestState::default() }
        }
        DebugOp::SetFriendship { npc_id, points } => {
            TestState { friendship: IndexMap::from([(npc_id.clone(), *points)]), ..TestState::default() }
        }
        DebugOp::Teleport { scene_id, x, y, facing } => {
            TestState { scene_id: Some(scene_id.clone()), x: *x, y: *y, facing: facing.clone(), ..TestState::default() }
        }
        DebugOp::ApplyTestState { state: test } => (**test).clone(),
    };
    notes.extend(apply_test_state(ctx, state, &test));
    (effects, notes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{GameContent, Scene};

    fn game() -> (EngineContext, GameState) {
        let mut content = GameContent::default();
        content.scenes.push(Scene { id: "farm".into(), name: "Farm".into(), width: 10, height: 8, ..Scene::default() });
        let mut state = GameState::default();
        state.world.scenes = content.scenes.clone();
        crate::state::normalize_world(&mut state);
        state.clock.day = 1;
        state.clock.year = 1;
        (EngineContext::new(content), state)
    }

    #[test]
    fn dates_follow_the_calendar() {
        let (ctx, mut state) = game();
        let seasons = game_time::calendar_seasons(&ctx.content.settings.calendar);
        let first = seasons[0].days;
        let test = TestState { day: Some(first + 2), time: Some(600), ..TestState::default() };
        let notes = apply_test_state(&ctx, &mut state, &test);
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(state.clock.season, seasons[1].id);
        assert_eq!((state.clock.day_of_season, state.clock.year), (2, 1));
        assert_eq!(state.clock.time_minutes, units::minutes(600));

        let year: u32 = seasons.iter().map(|s| s.days).sum();
        let test = TestState { season: Some(seasons[1].id.clone()), year: Some(2), ..TestState::default() };
        apply_test_state(&ctx, &mut state, &test);
        assert_eq!(state.clock.day, year + first + 1);
        assert_eq!(state.clock.day_of_season, 1);
    }

    #[test]
    fn places_quests_and_missing_content() {
        let (ctx, mut state) = game();
        let test = TestState {
            scene_id: Some("farm".into()),
            x: Some(3),
            y: Some(4),
            facing: Some("left".into()),
            flags: IndexMap::from([("door-open".to_owned(), Value::Bool(true))]),
            variables: IndexMap::from([("bridges".to_owned(), Value::from(2))]),
            quests: IndexMap::from([("quest-gone".to_owned(), "completed".to_owned())]),
            inventory: Some(vec![TestStateItem { item_id: "item-gone".into(), ..TestStateItem::default() }]),
            money: Some(42),
            ..TestState::default()
        };
        let notes = apply_test_state(&ctx, &mut state, &test);
        assert_eq!(notes, ["No quest 'quest-gone'.", "No item 'item-gone'."]);
        assert_eq!((state.player.x, state.player.y), (units::tile_center(3), units::tile_center(4)));
        assert_eq!(state.player.direction, "left");
        assert_eq!(state.flags["bridges"], Value::from(2));
        assert_eq!(state.player.money, 42);

        let captured = capture_test_state(&ctx, &state, "now", "Now");
        assert_eq!((captured.x, captured.y), (Some(3), Some(4)));
        assert_eq!(captured.flags["door-open"], Value::Bool(true));
        assert_eq!(captured.variables["bridges"], Value::from(2));
        let mut again = game().1;
        assert!(apply_test_state(&ctx, &mut again, &captured).is_empty());
        assert_eq!(again.flags, state.flags);
        assert_eq!((again.player.x, again.clock.day), (state.player.x, state.clock.day));

        let outside =
            apply_test_state(&ctx, &mut state, &TestState { x: Some(20), y: Some(1), ..TestState::default() });
        assert_eq!(outside.len(), 1);
        assert_eq!(state.player.x, units::tile_center(9));
    }
}
