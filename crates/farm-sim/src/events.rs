//! Events, conditions, outcomes and creator actions (port of `Events.cs` / events.ts).
//!
//! Event & trigger runtime (M3). Deterministic: events evaluate in content order; fire-once is
//! tracked via an auto-managed flag; repeatable events opt in explicitly.

use crate::effects::{message_levels, Effect};
use crate::engine_types::{Effects, EngineContext};
use crate::farming::farming_actions;
use crate::hooks::{ActionHookPayload, HookEvent};
use crate::messages;
use crate::schema::{
    event_fired_flag, event_triggers, relationship_statuses, variable_comparisons, CutsceneState, DialogueState,
    EventCondition, EventOutcome, GameEvent, GameState, MinigameSession, NpcState, OutcomeKind, ShopStockEntry,
    TileMachine, TileNode, MAX_FRIENDSHIP,
};
use crate::text;
use crate::units::{self, SignedMilli};
use crate::world::{tiles, world_movement};
use crate::{energy, game_time, inventory, quests, skills, social, substitution, weather};
use indexmap::IndexMap;
use serde_json::Value;

/// TS `EventPosition`: the tile entered or interacted with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EventPosition {
    pub x: i32,
    pub y: i32,
}

/// TS `PerformActionResult` minus the state (updated in place).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PerformActionResult {
    pub effects: Effects,
    pub ran: bool,
}

/// Actions may perform actions; cap the chain so cycles terminate.
pub const MAX_ACTION_DEPTH: u32 = 4;

/// The most actions one top-level call (a command, a dialogue option, an event, a minigame tier,
/// a plugin mutation) may run in all. The depth cap alone bounds a chain, not its breadth: an
/// action that performs itself twelve times would run 22,621 times within four levels.
pub const MAX_ACTION_RUNS: u32 = 256;

/// What the action chain of one top-level call has used up.
#[derive(Debug, Default)]
struct ActionBudget {
    runs: u32,
    /// The "limit reached" warning was given (once per call).
    warned: bool,
}

impl ActionBudget {
    /// The warning creators see when a chain is cut short (once per top-level call).
    fn limit_reached(&mut self, action_id: &str) -> Effects {
        if self.warned {
            return Vec::new();
        }
        self.warned = true;
        vec![Effect::say(
            message_levels::ERROR,
            messages::ACTION_CHAIN_LIMIT.with(&[&action_id, &MAX_ACTION_DEPTH, &MAX_ACTION_RUNS]),
        )]
    }
}

const QUEST_STATUS_NOT_STARTED: &str = "not-started";

/// An outcome `amount` (thousandths) as a whole number: gold or friendship points.
fn whole(amount: i64) -> i64 {
    units::div_round(amount, i64::from(units::MILLI_ONE))
}

/// TS `x || y` on a string: an empty (or missing) string is falsy.
fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|text| !text.is_empty())
}

fn position_matches(x: i32, y: i32, x2: Option<i32>, y2: Option<i32>, pos: EventPosition) -> bool {
    let min_x = x.min(x2.unwrap_or(x));
    let max_x = x.max(x2.unwrap_or(x));
    let min_y = y.min(y2.unwrap_or(y));
    let max_y = y.max(y2.unwrap_or(y));
    pos.x >= min_x && pos.x <= max_x && pos.y >= min_y && pos.y <= max_y
}

/// What conditions are checked against besides the state.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConditionScope<'a> {
    /// The tile entered or interacted with (the tile conditions).
    pub pos: Option<EventPosition>,
    /// The NPC the player is talking to (daily lines, heart events): `hearts` and
    /// `relationship` with an empty `npcId` mean this NPC (M4).
    pub speaker: Option<&'a str>,
    /// What a `chance` roll is about (an event or line id), so different events roll apart.
    pub salt: &'a str,
}

/// Whether `condition` holds now; `pos` is the tile entered or interacted with (for the tile
/// conditions).
pub fn condition_met(
    ctx: &EngineContext,
    state: &GameState,
    condition: &EventCondition,
    pos: Option<EventPosition>,
) -> bool {
    condition_holds(ctx, state, condition, &ConditionScope { pos, ..ConditionScope::default() })
}

/// Whether every one of `conditions` holds in `scope`.
pub fn conditions_hold(
    ctx: &EngineContext,
    state: &GameState,
    conditions: &[EventCondition],
    scope: &ConditionScope<'_>,
) -> bool {
    conditions.iter().all(|condition| condition_holds(ctx, state, condition, scope))
}

/// Whether `condition` holds now in `scope`.
pub fn condition_holds(
    ctx: &EngineContext,
    state: &GameState,
    condition: &EventCondition,
    scope: &ConditionScope<'_>,
) -> bool {
    let pos = scope.pos;
    match condition {
        EventCondition::EnterTile { x, y, x2, y2 } => pos.is_some_and(|pos| position_matches(*x, *y, *x2, *y2, pos)),
        EventCondition::InteractTile { x, y, x2, y2 } => pos.is_some_and(|pos| position_matches(*x, *y, *x2, *y2, pos)),
        EventCondition::HasItem { item_id, quantity } => {
            let held = state
                .player
                .inventory
                .iter()
                .filter(|slot| slot.item.id == *item_id)
                .fold(0_u64, |sum, slot| sum + u64::from(slot.quantity));
            held >= u64::from(*quantity)
        }
        EventCondition::InventorySpace { item_id, quantity } => {
            let item = ctx.item(item_id);
            item.is_some_and(|item| {
                inventory::add_item(&state.player.inventory, item, *quantity, state.player.max_inventory_size, None)
                    .added
            })
        }
        EventCondition::Flag { flag, value } => text::truthy(flag_value(state, flag)) == *value,
        EventCondition::DayRange { min_day, max_day } => {
            if min_day.is_some_and(|min_day| state.clock.day < min_day) {
                return false;
            }
            if max_day.is_some_and(|max_day| state.clock.day > max_day) {
                return false;
            }
            true
        }
        EventCondition::Season { seasons } => seasons.contains(&state.clock.season),
        EventCondition::YearRange { min_year, max_year } => {
            if min_year.is_some_and(|min_year| state.clock.year < min_year) {
                return false;
            }
            if max_year.is_some_and(|max_year| state.clock.year > max_year) {
                return false;
            }
            true
        }
        EventCondition::TimeOfDay { min_minute, max_minute } => time_of_day_matches_in(
            ctx.content.settings.time.minutes_per_day(),
            state.clock.time_minutes,
            *min_minute,
            *max_minute,
        ),
        EventCondition::QuestStatus { quest_id, status } => {
            state.quests.get(quest_id).map_or(QUEST_STATUS_NOT_STARTED, |progress| progress.status.as_str()) == status
        }
        EventCondition::Friendship { npc_id, min } => {
            state.social.get(npc_id).map_or(0, |social| social.friendship) >= *min
        }
        EventCondition::Weather { weather_ids } => weather_ids.contains(&state.clock.weather_id),
        EventCondition::FestivalId { festival_id } => {
            game_time::festival_today(&ctx.content.settings.calendar, &state.clock)
                .is_some_and(|festival| festival.id == *festival_id)
        }
        EventCondition::Variable { name, compare, value, text } => match text {
            Some(text) => compares(compare, text::compare_strings(&variable_text(state, name), text)),
            None => compares(compare, variable_number(state, name).cmp(&value.unwrap_or(0))),
        },
        EventCondition::All { conditions } => conditions_hold(ctx, state, conditions, scope),
        EventCondition::Any { conditions } => {
            conditions.iter().any(|condition| condition_holds(ctx, state, condition, scope))
        }
        EventCondition::Not { conditions } => {
            !conditions.iter().any(|condition| condition_holds(ctx, state, condition, scope))
        }
        EventCondition::Chance { chance } => u64::from(chance_roll(state, scope.salt, *chance)) < *chance,
        EventCondition::Scene { scene_ids } => scene_ids.contains(&state.player.scene_id),
        EventCondition::Hearts { npc_id, min, max } => {
            let Some(npc_id) = non_empty(Some(npc_id)).or(scope.speaker) else { return false };
            let hearts = social::hearts(social::friendship_with(state, npc_id));
            min.is_none_or(|min| hearts >= min) && max.is_none_or(|max| hearts <= max)
        }
        EventCondition::DayOfSeason { min_day, max_day } => {
            let day = game_time::clock_date(&ctx.content.settings.calendar, &state.clock).day_of_season;
            min_day.is_none_or(|min| day >= min) && max_day.is_none_or(|max| day <= max)
        }
        EventCondition::Relationship { npc_id, status } => match non_empty(Some(npc_id)).or(scope.speaker) {
            Some(npc_id) => social::relationship_status(state, npc_id) == status,
            // Anyone: single means with no one.
            None if status == relationship_statuses::SINGLE => {
                state.social.values().all(|social| social.status.as_deref().is_none_or(str::is_empty))
            }
            None => state.social.values().any(|social| social.status.as_deref() == Some(status.as_str())),
        },
        EventCondition::Money { min } => state.player.money >= *min,
        EventCondition::SkillLevel { skill, min } => skills::skill_level(state, skill) >= *min,
        EventCondition::MineFloor { min } => state.mine.deepest_floor >= *min,
        EventCondition::Collection { collection_id, min } => match min {
            Some(min) => crate::goals::collected(state, collection_id) as u64 >= u64::from(*min),
            None => crate::goals::collection_complete(ctx, state, collection_id),
        },
        EventCondition::Bundle { bundle_id } => crate::goals::bundle_complete(state, bundle_id),
        EventCondition::Achievement { achievement_id } => state.achievements.contains_key(achievement_id),
        EventCondition::Shipped { item_id, min } => crate::goals::shipped(state, item_id) >= u64::from(*min),
        EventCondition::Building { building_id, min } => {
            crate::buildings::count_standing(ctx, state, building_id) >= *min
        }
        EventCondition::Slain { monster_id, min } => crate::combat::slain(state, monster_id) >= u64::from(*min),
        EventCondition::Birthday { npc_id } => {
            let date = game_time::clock_date(&ctx.content.settings.calendar, &state.clock);
            let today = |npc: &crate::schema::Npc| {
                npc.birthday
                    .as_ref()
                    .is_some_and(|birthday| birthday.season == state.clock.season && birthday.day == date.day_of_season)
            };
            match non_empty(Some(npc_id)) {
                Some(npc_id) => ctx.npc(npc_id).is_some_and(today),
                None => ctx.content.npcs.iter().any(today),
            }
        }
    }
}

/// Whether an ordering satisfies a `variable` comparison (an unknown one never does).
fn compares(compare: &str, ordering: std::cmp::Ordering) -> bool {
    use std::cmp::Ordering::{Equal, Greater, Less};
    match compare {
        variable_comparisons::EQ => ordering == Equal,
        variable_comparisons::NE => ordering != Equal,
        variable_comparisons::LT => ordering == Less,
        variable_comparisons::LE => ordering != Greater,
        variable_comparisons::GT => ordering == Greater,
        variable_comparisons::GE => ordering != Less,
        _ => false,
    }
}

/// A variable as a number, in thousandths (M3): a number as it is, `true` as 1, anything else
/// (unset, false, a text) as 0.
pub fn variable_number(state: &GameState, name: &str) -> i64 {
    match state.flags.get(name) {
        Some(Value::Number(number)) => units::milli_of_number(number),
        Some(Value::Bool(true)) => i64::from(units::MILLI_ONE),
        _ => 0,
    }
}

/// A variable as text (M3): JavaScript `String(value)`, empty when unset.
pub fn variable_text(state: &GameState, name: &str) -> String {
    state.flags.get(name).map(text::to_js_string).unwrap_or_default()
}

/// A random draw for a `chance` condition. Conditions are read without changing the state (the
/// game's screens ask them too), so the roll does not advance the game's RNG: it hashes the RNG
/// state, the tick, `salt` and the chance. It is the same for a whole tick, and new each tick.
fn chance_roll(state: &GameState, salt: &str, chance: u64) -> u32 {
    let mut bytes = Vec::with_capacity(48 + salt.len());
    for word in state.rng.s {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.extend_from_slice(&state.clock.tick.to_le_bytes());
    bytes.extend_from_slice(&state.clock.day.to_le_bytes());
    bytes.extend_from_slice(&chance.to_le_bytes());
    bytes.extend_from_slice(salt.as_bytes());
    twox_hash::XxHash3_64::oneshot(&bytes) as u32
}

/// A `timeOfDay` range against the clock (all in micro-minutes). The clock counts on past
/// midnight until the day ends (26:00 by default) while the game shows 0:00–2:00, so a range
/// matches the clock as it counts or as it is shown: "0:00–2:00" matches 24:00–26:00. A range
/// whose start is after its end wraps past midnight: "22:00–2:00" matches 22:00 to 2:00 (#37).
pub fn time_of_day_matches(time: u32, min: u32, max: u32) -> bool {
    time_of_day_matches_in(units::MINUTES_PER_DAY, time, min, max)
}

/// [`time_of_day_matches`] in a day of `day_minutes` minutes (M1: games choose their day's
/// length; 1440 is 24 hours).
pub fn time_of_day_matches_in(day_minutes: u32, time: u32, min: u32, max: u32) -> bool {
    let day = day_minutes.max(1).saturating_mul(units::MINUTE);
    let shown = time % day;
    if min <= max {
        (min..=max).contains(&time) || (min..=max).contains(&shown)
    } else {
        shown >= min % day || shown <= max % day
    }
}

/// TS `state.flags[name]` (undefined → `None`).
pub fn flag_value<'a>(state: &'a GameState, name: &str) -> Option<&'a Value> {
    state.flags.get(name)
}

/// Run one outcome. Outcomes fail soft: a missing field, an unknown reference or an unknown
/// `type` does nothing.
fn apply_outcome(
    ctx: &EngineContext,
    state: &mut GameState,
    outcome: &EventOutcome,
    depth: u32,
    budget: &mut ActionBudget,
) -> Effects {
    let Some(kind) = outcome.kind() else { return vec![] };
    match kind {
        OutcomeKind::Message => match non_empty(outcome.message.as_deref()) {
            Some(message) => {
                vec![Effect::message(message_levels::INFO, substitution::fill(&ctx.content, state, message))]
            }
            None => vec![],
        },
        OutcomeKind::ModifyFriendship => outcome_modify_friendship(ctx, state, outcome),
        OutcomeKind::ModifyEnergy => outcome_modify_energy(ctx, state, outcome),
        OutcomeKind::WaterArea => outcome_water_area(ctx, state, outcome),
        OutcomeKind::GiveItem => outcome_give_item(ctx, state, outcome),
        OutcomeKind::TakeItem => {
            let Some(item_id) = non_empty(outcome.item_id.as_deref()) else { return vec![] };
            state.player.inventory =
                inventory::remove_item(&state.player.inventory, item_id, outcome.item_quantity.unwrap_or(1));
            vec![]
        }
        OutcomeKind::GiveMoney => outcome_give_money(state, outcome),
        OutcomeKind::TakeMoney => outcome_take_money(state, outcome),
        OutcomeKind::SetFlag | OutcomeKind::ClearFlag => {
            let Some(flag_name) = non_empty(outcome.flag_name.as_deref()) else { return vec![] };
            state.flags.insert(flag_name.to_owned(), Value::Bool(kind == OutcomeKind::SetFlag));
            vec![]
        }
        OutcomeKind::StartQuest => {
            let Some(quest_id) = non_empty(outcome.quest_id.as_deref()) else { return vec![] };
            quests::start_quest_by_id(ctx, state, quest_id)
        }
        OutcomeKind::CompleteQuest => {
            let Some(quest_id) = non_empty(outcome.quest_id.as_deref()) else { return vec![] };
            quests::complete_quest_by_id(ctx, state, quest_id)
        }
        OutcomeKind::SpawnNpc => outcome_spawn_npc(ctx, state, outcome),
        OutcomeKind::RemoveNpc => {
            let Some(npc_id) = non_empty(outcome.npc_id.as_deref()) else { return vec![] };
            state.npcs.shift_remove(npc_id);
            vec![]
        }
        OutcomeKind::ChangeTile => outcome_change_tile(ctx, state, outcome),
        OutcomeKind::WarpPlayer => outcome_warp_player(ctx, state, outcome),
        OutcomeKind::StartDialogue => outcome_start_dialogue(ctx, state, outcome),
        OutcomeKind::LockTransition | OutcomeKind::UnlockTransition => {
            outcome_lock_transition(state, outcome, kind == OutcomeKind::LockTransition)
        }
        OutcomeKind::PlaySound => match non_empty(outcome.sound_id.as_deref()) {
            Some(sound_id) => vec![Effect::Sound { id: sound_id.to_owned() }],
            None => vec![],
        },
        OutcomeKind::PlayMusic => {
            // M2: a music asset until the scene changes; empty: back to the music the place picks.
            state.music = non_empty(outcome.sound_id.as_deref()).map(str::to_owned);
            vec![]
        }
        OutcomeKind::StartCutscene => {
            // M2: the cutscene runner plays it from the next tick; one at a time.
            let Some(cutscene_id) = non_empty(outcome.cutscene_id.as_deref()) else { return vec![] };
            if state.cutscene.is_none() && ctx.content.cutscenes.iter().any(|cutscene| cutscene.id == cutscene_id) {
                state.cutscene =
                    Some(CutsceneState { cutscene_id: cutscene_id.to_owned(), ..CutsceneState::default() });
            }
            vec![]
        }
        OutcomeKind::PerformAction => {
            let Some(action_id) = non_empty(outcome.action_id.as_deref()) else { return vec![] };
            perform_action_detailed(ctx, state, action_id, depth + 1, budget).effects
        }
        OutcomeKind::StartMinigame => {
            let Some(minigame_id) = non_empty(outcome.minigame_id.as_deref()) else { return vec![] };
            start_minigame_session(ctx, state, minigame_id, None)
        }
        OutcomeKind::SetVariable => {
            let Some(variable) = non_empty(outcome.variable.as_deref()) else { return vec![] };
            let value = match &outcome.text {
                Some(text) => Value::String(text.clone()),
                None => number_value(outcome.amount.unwrap_or(0)),
            };
            state.flags.insert(variable.to_owned(), value);
            vec![]
        }
        OutcomeKind::AddVariable => {
            let Some(variable) = non_empty(outcome.variable.as_deref()) else { return vec![] };
            let sum = variable_number(state, variable).saturating_add(outcome.amount.unwrap_or(0));
            state.flags.insert(variable.to_owned(), number_value(sum));
            vec![]
        }
        OutcomeKind::MoveNpc => outcome_move_npc(ctx, state, outcome),
        OutcomeKind::SetWeather => {
            let Some(weather_id) = non_empty(outcome.weather_id.as_deref()) else { return vec![] };
            if weather::weather_type_by_id(ctx, weather_id).is_some() {
                state.clock.weather_id = weather_id.to_owned();
            }
            vec![]
        }
        OutcomeKind::GiveExperience => {
            let Some(skill) = non_empty(outcome.skill.as_deref()) else { return vec![] };
            let amount = u32::try_from(whole(outcome.amount.unwrap_or(0)).max(0)).unwrap_or(u32::MAX);
            skills::grant_xp(ctx, state, skill, amount)
        }
        OutcomeKind::AddShopStock | OutcomeKind::RemoveShopStock => {
            outcome_shop_stock(ctx, state, outcome, kind == OutcomeKind::AddShopStock)
        }
        OutcomeKind::PlaceNode | OutcomeKind::RemoveNode | OutcomeKind::PlaceMachine | OutcomeKind::RemoveMachine => {
            outcome_tile_object(ctx, state, outcome, kind)
        }
        OutcomeKind::SetRelationship => {
            let (Some(npc_id), Some(status)) = (non_empty(outcome.npc_id.as_deref()), outcome.status.as_deref()) else {
                return vec![];
            };
            if ctx.npc(npc_id).is_none() || !relationship_statuses::ALL.contains(&status) {
                return vec![];
            }
            social::set_relationship(ctx, state, npc_id, status)
        }
        OutcomeKind::SendMail => {
            if let Some(mail_id) = non_empty(outcome.mail_id.as_deref()) {
                crate::mail::send(ctx, state, mail_id);
            }
            vec![]
        }
        OutcomeKind::ModifyHealth => {
            let amount = i32::try_from(whole(outcome.amount.unwrap_or(0))).unwrap_or(0);
            crate::combat::modify_health(ctx, state, amount)
        }
    }
}

/// A number variable's stored value: a whole number as an integer, else the double.
fn number_value(milli: i64) -> Value {
    units::canonical_json(units::json::<SignedMilli>(milli))
}

/// `moveNpc`: put the NPC on the tile (x, y) of `sceneId` (absent: the scene it is in). Its
/// schedule or pattern takes it on from there.
fn outcome_move_npc(ctx: &EngineContext, state: &mut GameState, outcome: &EventOutcome) -> Effects {
    let Some(npc_id) = non_empty(outcome.npc_id.as_deref()) else { return vec![] };
    let (Some(x), Some(y)) = (outcome.x, outcome.y) else { return vec![] };
    let Some(npc_def) = ctx.npc(npc_id) else { return vec![] };
    let current = state.npcs.get(npc_id).cloned();
    let scene_id = non_empty(outcome.scene_id.as_deref())
        .map(str::to_owned)
        .or_else(|| current.as_ref().map(|npc| npc.scene_id.clone()))
        .unwrap_or_else(|| npc_def.scene_id.clone());
    if world_movement::ensure_scene(ctx, state, &scene_id).is_none() {
        return vec![];
    }
    let npc = NpcState { x: units::tiles(x), y: units::tiles(y), scene_id, path: None, ..current.unwrap_or_default() };
    state.npcs.insert(npc_id.to_owned(), npc);
    vec![]
}

/// `addShopStock` / `removeShopStock`: the game's own changes to a shop's stock, kept in the
/// state (the shop's definition is content).
fn outcome_shop_stock(ctx: &EngineContext, state: &mut GameState, outcome: &EventOutcome, add: bool) -> Effects {
    let (Some(shop_id), Some(item_id)) = (non_empty(outcome.shop_id.as_deref()), non_empty(outcome.item_id.as_deref()))
    else {
        return vec![];
    };
    if ctx.shop(shop_id).is_none() || ctx.item(item_id).is_none() {
        return vec![];
    }
    let changes = state.shop_changes.entry(shop_id.to_owned()).or_default();
    changes.added.retain(|entry| entry.item_id != item_id);
    changes.removed.retain(|removed| removed != item_id);
    if add {
        changes.added.push(ShopStockEntry {
            item_id: item_id.to_owned(),
            price: outcome.amount.map(whole).filter(|price| *price >= 0),
            daily_limit: outcome.item_quantity.filter(|limit| *limit > 0),
            ..ShopStockEntry::default()
        });
    } else {
        changes.removed.push(item_id.to_owned());
    }
    vec![]
}

/// `placeNode` / `removeNode` / `placeMachine` / `removeMachine` on tile (tileX, tileY) of
/// `sceneId` (absent: the player's). A node or machine goes only where there is nothing in the
/// way (a crop, a node, a machine, an item, a wall); removing one takes it away with whatever it
/// held.
fn outcome_tile_object(
    ctx: &EngineContext,
    state: &mut GameState,
    outcome: &EventOutcome,
    kind: OutcomeKind,
) -> Effects {
    let (Some(x), Some(y)) = (outcome.tile_x, outcome.tile_y) else { return vec![] };
    let scene_id = non_empty(outcome.scene_id.as_deref()).unwrap_or(&state.player.scene_id).to_owned();
    let Some(scene_index) = world_movement::ensure_scene(ctx, state, &scene_id) else { return vec![] };
    let npc_there =
        state.npcs.values().any(|npc| npc.scene_id == scene_id && npc.x == units::tiles(x) && npc.y == units::tiles(y));
    let player_there =
        state.player.scene_id == scene_id && world_movement::player_tile(state) == world_movement::TilePoint { x, y };
    let Some(tile) = state.world.scenes[scene_index].tile_mut(x, y) else { return vec![] };
    let occupied =
        tile.collision || tile.crop.is_some() || tile.node.is_some() || tile.machine.is_some() || tile.item.is_some();
    match kind {
        OutcomeKind::PlaceNode => {
            let Some(node_type) = outcome.node_type_id.as_deref().and_then(|id| ctx.node_type(id)) else {
                return vec![];
            };
            if !occupied && !npc_there && !player_there {
                tile.node = Some(TileNode {
                    type_id: node_type.id.clone(),
                    remaining_health: node_type.health,
                    ..TileNode::default()
                });
            }
        }
        OutcomeKind::PlaceMachine => {
            let Some(machine_type) = outcome.machine_type_id.as_deref().and_then(|id| ctx.machine_type(id)) else {
                return vec![];
            };
            if !occupied && !npc_there && !player_there {
                tile.machine = Some(TileMachine { type_id: machine_type.id.clone(), ..TileMachine::default() });
            }
        }
        OutcomeKind::RemoveNode => tile.node = None,
        OutcomeKind::RemoveMachine => tile.machine = None,
        _ => {}
    }
    vec![]
}

fn outcome_modify_friendship(ctx: &EngineContext, state: &mut GameState, outcome: &EventOutcome) -> Effects {
    let Some(npc_id) = non_empty(outcome.npc_id.as_deref()) else { return vec![] };
    if ctx.npc(npc_id).is_none() {
        return vec![];
    }
    // Friendship is whole points: the amount rounds.
    let delta = whole(outcome.amount.unwrap_or(0));
    let current = i64::from(social::friendship_with(state, npc_id));
    let friendship = (current + delta).clamp(0, i64::from(MAX_FRIENDSHIP)) as i32;
    social::set_friendship(ctx, state, npc_id, friendship)
}

fn outcome_modify_energy(ctx: &EngineContext, state: &mut GameState, outcome: &EventOutcome) -> Effects {
    // Thousandths of a point: the energy unit. Out-of-range amounts saturate.
    let amount = outcome.amount.unwrap_or(0).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    if !ctx.content.settings.energy_enabled {
        return vec![];
    }
    if amount < 0 {
        return energy::spend_energy(ctx, state, amount.saturating_neg()).effects;
    }
    state.player.energy = state.player.max_energy.min(state.player.energy.saturating_add(amount));
    vec![]
}

/// The watering can's rule on every tile of the square around the player (grid positions,
/// bounded to the area), so fertilized soil stays fertilized, withered crops stay dry and a
/// multi-tile crop is watered whole.
fn outcome_water_area(ctx: &EngineContext, state: &mut GameState, outcome: &EventOutcome) -> Effects {
    let radius = i64::from(outcome.radius.unwrap_or(1).min(10));
    let center = world_movement::player_tile(state);
    let (cx, cy) = (i64::from(center.x), i64::from(center.y));
    let scene_id = state.player.scene_id.clone();
    if let Some(index) = state.world.scenes.iter().position(|scene| scene.id == scene_id) {
        for y in (cy - radius).max(0)..=cy + radius {
            for x in (cx - radius).max(0)..=cx + radius {
                farming_actions::water_at(ctx, state, index, x as usize, y as usize);
            }
        }
    }
    vec![Effect::say(message_levels::SUCCESS, &messages::SOIL_WATERED)]
}

fn outcome_give_item(ctx: &EngineContext, state: &mut GameState, outcome: &EventOutcome) -> Effects {
    let Some(item) = outcome.item_id.as_deref().and_then(|id| ctx.item(id)) else { return vec![] };
    let quantity = outcome.item_quantity.unwrap_or(1);
    let result = inventory::add_item(&state.player.inventory, item, quantity, state.player.max_inventory_size, None);
    if !result.added {
        return vec![Effect::say(message_levels::ERROR, &messages::INVENTORY_FULL)];
    }
    state.player.inventory = result.inventory;
    let received = if quantity > 1 {
        messages::RECEIVED_ITEMS.with(&[&item.name, &quantity])
    } else {
        messages::RECEIVED_ITEM.with(&[&item.name])
    };
    vec![Effect::say(message_levels::SUCCESS, received)]
}

fn outcome_give_money(state: &mut GameState, outcome: &EventOutcome) -> Effects {
    // Money is whole gold: the amount rounds.
    let amount = whole(outcome.amount.unwrap_or(0));
    if amount <= 0 {
        return vec![];
    }
    state.player.money = state.player.money.saturating_add(amount);
    vec![Effect::say(message_levels::SUCCESS, messages::RECEIVED_MONEY.with(&[&amount]))]
}

fn outcome_take_money(state: &mut GameState, outcome: &EventOutcome) -> Effects {
    let amount = whole(outcome.amount.unwrap_or(0)).min(state.player.money);
    if amount <= 0 {
        return vec![];
    }
    state.player.money -= amount;
    vec![Effect::say(message_levels::INFO, messages::PAID_MONEY.with(&[&amount]))]
}

fn outcome_spawn_npc(ctx: &EngineContext, state: &mut GameState, outcome: &EventOutcome) -> Effects {
    let Some(npc_id) = non_empty(outcome.npc_id.as_deref()) else { return vec![] };
    let Some(npc_def) = ctx.npc(npc_id) else { return vec![] };
    let npc_state = NpcState {
        x: outcome.x.map_or(npc_def.x, units::tiles),
        y: outcome.y.map_or(npc_def.y, units::tiles),
        scene_id: outcome.scene_id.clone().unwrap_or_else(|| npc_def.scene_id.clone()),
        path: None,
        patrol_index: None,
    };
    state.npcs.insert(npc_id.to_owned(), npc_state);
    vec![]
}

fn outcome_change_tile(ctx: &EngineContext, state: &mut GameState, outcome: &EventOutcome) -> Effects {
    let (Some(tile_x), Some(tile_y)) = (outcome.tile_x, outcome.tile_y) else { return vec![] };
    let Some(new_tile_type) = non_empty(outcome.new_tile_type.as_deref()) else { return vec![] };
    let scene_id = non_empty(outcome.scene_id.as_deref()).unwrap_or(&state.player.scene_id).to_owned();
    let Some(scene_index) = state.world.scenes.iter().position(|s| s.id == scene_id) else { return vec![] };
    let scene = &mut state.world.scenes[scene_index];
    if tile_y < 0 || tile_y >= scene.height || tile_x < 0 || tile_x >= scene.width {
        return vec![];
    }
    if let Some(tile) = scene.tiles.get_mut(tile_y as usize).and_then(|row| row.get_mut(tile_x as usize)) {
        *tile = tiles::set_tile_layer_in(ctx.terrains(), tile, new_tile_type, None);
    }
    vec![]
}

fn outcome_warp_player(ctx: &EngineContext, state: &mut GameState, outcome: &EventOutcome) -> Effects {
    let Some(scene_id) = non_empty(outcome.scene_id.as_deref()) else { return vec![] };
    let (Some(x), Some(y)) = (outcome.x, outcome.y) else { return vec![] };
    // A scene of an enabled content pack joins the world on first visit.
    if world_movement::ensure_scene(ctx, state, scene_id).is_none() {
        return vec![];
    }
    // Warp targets are authored as tile coordinates; land on the center, or on the nearest
    // walkable tile when the target is outside the scene or blocked. Music an event chose ends
    // when the player leaves the scene (`land_player`).
    let changed_scene = state.player.scene_id != scene_id;
    let (landed, landing_effects) = world_movement::land_player(ctx, state, scene_id, x, y);
    let mut effects = vec![Effect::SceneChanged { scene_id: scene_id.to_owned(), x: landed.x, y: landed.y }];
    effects.extend(landing_effects);
    if changed_scene {
        // A heart event of the scene entered (M4).
        crate::relationships::start_heart_event(ctx, state, crate::relationships::Place::Scene(scene_id));
    }
    effects
}

fn outcome_start_dialogue(ctx: &EngineContext, state: &mut GameState, outcome: &EventOutcome) -> Effects {
    let Some(npc_id) = non_empty(outcome.npc_id.as_deref()) else { return vec![] };
    let dialogue_id = outcome
        .dialogue_id
        .clone()
        .or_else(|| ctx.npc(npc_id).and_then(|npc| npc.dialogue.first()).map(|dialogue| dialogue.id.clone()));
    let Some(dialogue_id) = dialogue_id.filter(|id| !id.is_empty()) else { return vec![] };
    state.dialogue = Some(DialogueState { npc_id: npc_id.to_owned(), dialogue_id, line: None });
    vec![]
}

fn outcome_lock_transition(state: &mut GameState, outcome: &EventOutcome, locked: bool) -> Effects {
    let scene_id = non_empty(outcome.scene_id.as_deref()).unwrap_or(&state.player.scene_id).to_owned();
    let Some(scene_index) = state.world.scenes.iter().position(|s| s.id == scene_id) else { return vec![] };
    let (Some(x), Some(y)) = (outcome.x, outcome.y) else { return vec![] };
    for transition in &mut state.world.scenes[scene_index].transitions {
        if transition.from_x == x && transition.from_y == y {
            transition.locked = Some(locked);
        }
    }
    vec![]
}

/// Apply a sequence of outcomes (the shared executor behind events, custom actions, minigame
/// result tiers and plugin mutations). A top-level call: the actions its outcomes perform share
/// one [`MAX_ACTION_RUNS`] budget.
pub fn apply_outcomes(ctx: &EngineContext, state: &mut GameState, outcomes: &[EventOutcome], depth: u32) -> Effects {
    apply_outcomes_within(ctx, state, outcomes, depth, &mut ActionBudget::default())
}

fn apply_outcomes_within(
    ctx: &EngineContext,
    state: &mut GameState,
    outcomes: &[EventOutcome],
    depth: u32,
    budget: &mut ActionBudget,
) -> Effects {
    let mut effects = Vec::new();
    for outcome in outcomes {
        effects.extend(apply_outcome(ctx, state, outcome, depth, budget));
    }
    effects
}

/// Run a creator-defined action (extensibility layer): check its conditions, spend energy, apply
/// its outcomes, and notify plugins via `onAction`. The actions it performs in turn run at most
/// [`MAX_ACTION_DEPTH`] levels deep and [`MAX_ACTION_RUNS`] times in all; a chain cut short says
/// so with an error message.
pub fn perform_action(ctx: &EngineContext, state: &mut GameState, action_id: &str) -> PerformActionResult {
    perform_action_detailed(ctx, state, action_id, 0, &mut ActionBudget::default())
}

fn perform_action_detailed(
    ctx: &EngineContext,
    state: &mut GameState,
    action_id: &str,
    depth: u32,
    budget: &mut ActionBudget,
) -> PerformActionResult {
    if depth > MAX_ACTION_DEPTH || budget.runs >= MAX_ACTION_RUNS {
        return PerformActionResult { effects: budget.limit_reached(action_id), ran: false };
    }
    let Some(action) = ctx.action(action_id) else {
        return PerformActionResult {
            effects: vec![Effect::say(message_levels::ERROR, messages::UNKNOWN_ACTION.with(&[&action_id]))],
            ran: false,
        };
    };

    for condition in &action.conditions {
        if !condition_met(ctx, state, condition, None) {
            return PerformActionResult {
                effects: match non_empty(Some(&action.fail_message)) {
                    Some(fail_message) => {
                        vec![Effect::message(
                            message_levels::INFO,
                            substitution::fill(&ctx.content, state, fail_message),
                        )]
                    }
                    None => vec![],
                },
                ran: false,
            };
        }
    }

    budget.runs += 1;
    let mut effects = Vec::new();

    if action.energy_cost > 0 {
        let spend = energy::spend_energy(ctx, state, action.energy_cost);
        effects.extend(spend.effects);
        if spend.collapsed {
            return PerformActionResult { effects, ran: false };
        }
    }

    effects.extend(apply_outcomes_within(ctx, state, &action.outcomes, depth, budget));

    ctx.emit(HookEvent::Action(ActionHookPayload { action_id: action.id.clone() }));
    PerformActionResult { effects, ran: true }
}

/// Open a declared minigame session (modal, resolves via the command log).
pub fn start_minigame_session(
    ctx: &EngineContext,
    state: &mut GameState,
    minigame_id: &str,
    context: Option<&IndexMap<String, Value>>,
) -> Effects {
    if ctx.minigame(minigame_id).is_none() {
        return vec![Effect::say(message_levels::ERROR, messages::UNKNOWN_MINIGAME.with(&[&minigame_id]))];
    }
    if state.minigame.is_some() {
        return vec![];
    }
    state.minigame =
        Some(MinigameSession { minigame_id: minigame_id.to_owned(), context: context.cloned().unwrap_or_default() });
    vec![]
}

/// Run an event's outcomes and mark a non-repeatable event as fired.
pub fn fire_event(ctx: &EngineContext, state: &mut GameState, event: &GameEvent) -> Effects {
    let effects = apply_outcomes(ctx, state, &event.outcomes, 0);
    if !event.repeatable {
        state.flags.insert(event_fired_flag(&event.id), Value::Bool(true));
    }
    effects
}

/// How deep events set off by triggers may nest (an event whose outcomes set off a trigger whose
/// events set off another…, M3). Deeper triggers are ignored, so an event that sets off its own
/// trigger runs a few times and stops.
pub const MAX_TRIGGER_DEPTH: u32 = 4;

/// What set off a trigger (M3).
#[derive(Debug, Clone, Copy, Default)]
pub struct Trigger<'a> {
    /// One of [`event_triggers`].
    pub kind: &'a str,
    /// The tile entered or interacted with.
    pub pos: Option<EventPosition>,
    /// What it is about (an item, an NPC, a quest; a harvest names the crop type and the item):
    /// an event with a `subject` fires only when it is one of these.
    pub subjects: &'a [&'a str],
    /// For `time`: the clock went from the first time (excluded) to the second (included), in
    /// micro-minutes.
    pub span: Option<(u32, u32)>,
}

impl<'a> Trigger<'a> {
    /// A trigger of `kind` about `subjects`.
    pub fn about(kind: &'a str, subjects: &'a [&'a str]) -> Self {
        Self { kind, subjects, ..Self::default() }
    }
}

/// Evaluate all events for a trigger kind. `pos` is the tile entered or interacted with (for
/// position conditions).
pub fn evaluate_events(
    ctx: &EngineContext,
    state: &mut GameState,
    trigger: &str,
    pos: Option<EventPosition>,
) -> Effects {
    fire_trigger_detailed(ctx, state, &Trigger { kind: trigger, pos, ..Trigger::default() }).effects
}

/// Fire the events of `trigger` (M3 triggers; see [`Trigger`]).
pub fn fire_trigger(ctx: &EngineContext, state: &mut GameState, trigger: &Trigger<'_>) -> Effects {
    fire_trigger_detailed(ctx, state, trigger).effects
}

/// What [`evaluate_events_detailed`] did.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EvaluateEventsResult {
    pub effects: Effects,
    /// At least one event fired (TS: `evaluateEvents` returned a new state).
    pub fired: bool,
}

/// [`evaluate_events`], also saying whether any event fired.
pub fn evaluate_events_detailed(
    ctx: &EngineContext,
    state: &mut GameState,
    trigger: &str,
    pos: Option<EventPosition>,
) -> EvaluateEventsResult {
    fire_trigger_detailed(ctx, state, &Trigger { kind: trigger, pos, ..Trigger::default() })
}

/// Whether a `time` event at `minute` falls in the clock's step `span`. The clock counts past
/// midnight until the day ends, so a time is also matched a day later (0:30 at 24:30).
fn time_reached(ctx: &EngineContext, minute: u32, (from, to): (u32, u32)) -> bool {
    let day = ctx.content.settings.time.minutes_per_day().max(1).saturating_mul(units::MINUTE);
    let crossed = |at: u32| from < at && at <= to;
    crossed(minute) || crossed(minute.saturating_add(day))
}

/// [`fire_trigger`], also saying whether any event fired.
pub fn fire_trigger_detailed(
    ctx: &EngineContext,
    state: &mut GameState,
    trigger: &Trigger<'_>,
) -> EvaluateEventsResult {
    let depth = ctx.trigger_depth.get();
    if depth >= MAX_TRIGGER_DEPTH {
        return EvaluateEventsResult::default();
    }
    ctx.trigger_depth.set(depth + 1);
    let mut effects = Vec::new();
    let mut fired = false;

    for event in &ctx.content.events {
        if !event.active || event.trigger != trigger.kind {
            continue;
        }
        if !event.scene_id.is_empty() && event.scene_id != state.player.scene_id {
            continue;
        }
        if let Some(subject) = non_empty(event.subject.as_deref()) {
            if !trigger.subjects.contains(&subject) {
                continue;
            }
        }
        if trigger.kind == event_triggers::TIME {
            let (Some(minute), Some(span)) = (event.minute, trigger.span) else { continue };
            if !time_reached(ctx, minute, span) {
                continue;
            }
        }
        if !event.repeatable && text::truthy(flag_value(state, &event_fired_flag(&event.id))) {
            continue;
        }
        let scope = ConditionScope { pos: trigger.pos, speaker: None, salt: &event.id };
        if !conditions_hold(ctx, state, &event.conditions, &scope) {
            continue;
        }

        effects.extend(fire_event(ctx, state, event));
        fired = true;
    }

    ctx.trigger_depth.set(depth);
    EvaluateEventsResult { effects, fired }
}
