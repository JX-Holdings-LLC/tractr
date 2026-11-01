//! Plugin systems with a presence in the world (#238): entities, timers and item use.
//!
//! The engine owns all of it: entities and timers are part of the state (saved, hashed,
//! replayed), change only through plugin mutations ([`apply_mutation`]) and the clock, and the
//! engine tells plugins what happens to them through [`HookEvent::Extension`] events:
//!
//! - each whole game minute, every entity type with entities in the world gets one
//!   `entity:<type>` event `{ op: 'tick', minute, entities }`; due timers are taken out of the
//!   state and each fires `timer` to the plugin that started it ([`advance_minute`]);
//! - each morning, `{ op: 'dayStart', day, entities }` per type ([`day_started`]);
//! - interacting with the tile an entity stands on sends `{ op: 'interact', entity }`
//!   ([`interact`]);
//! - using an item whose `useActionId` is `plugin:<handler>` sends `use:<handler>` with the
//!   item and the tile the player faces ([`use_item`]).
//!
//! Plugins answer with mutations through the queue as for any hook, so a replay of the command
//! log (which holds those mutations) needs no plugins.

use crate::effects::{message_levels, Effect};
use crate::engine_types::{Effects, EngineContext};
use crate::hooks::{extension_hooks, ExtensionHookPayload, HookEvent};
use crate::schema::{
    may_change_entity, GameState, PluginEntity, PluginMutation, PluginTimer, MAX_PLUGIN_ENTITIES,
    MAX_PLUGIN_STATE_BYTES, MAX_PLUGIN_TIMERS, PLUGIN_USE_PREFIX,
};
use crate::world::world_movement;
use crate::{crafting, units};
use serde_json::{json, Value};

/// A failed plugin mutation: the error toast every rejected plugin mutation shows.
fn refuse(text: String) -> Effects {
    vec![Effect::message(message_levels::ERROR, text)]
}

fn state_size(value: &Value) -> usize {
    serde_json::to_string(value).map_or(usize::MAX, |text| text.len())
}

/// Whether `(x, y)` is a tile of scene `scene_id` in `state`.
fn on_map(state: &GameState, scene_id: &str, x: i32, y: i32) -> bool {
    world_movement::find_scene(state, scene_id)
        .is_some_and(|scene| x >= 0 && y >= 0 && x < scene.width && y < scene.height)
}

/// Apply a plugin-world mutation of `plugin_id`; `None` when `mutation` is not one.
pub fn apply_mutation(
    ctx: &EngineContext,
    state: &mut GameState,
    plugin_id: &str,
    mutation: &PluginMutation,
) -> Option<Effects> {
    Some(match mutation {
        PluginMutation::SpawnEntity { entity_type, scene_id, x, y, art, state: entity_state } => {
            let scene_id = scene_id.clone().unwrap_or_else(|| state.player.scene_id.clone());
            if state.plugin_entities.len() >= MAX_PLUGIN_ENTITIES {
                return Some(refuse(format!(
                    "spawnEntity: the game already has {MAX_PLUGIN_ENTITIES} plugin entities"
                )));
            }
            if !on_map(state, &scene_id, *x, *y) {
                return Some(refuse(format!("spawnEntity: ({x}, {y}) is not a tile of scene {scene_id}")));
            }
            if state_size(entity_state) > MAX_PLUGIN_STATE_BYTES {
                return Some(refuse(format!("spawnEntity: state is larger than {MAX_PLUGIN_STATE_BYTES} bytes")));
            }
            state.next_plugin_entity = state.next_plugin_entity.saturating_add(1);
            let id = format!("entity-{}", state.next_plugin_entity);
            state.plugin_entities.push(PluginEntity {
                id,
                type_id: entity_type.clone(),
                scene_id,
                x: *x,
                y: *y,
                art: art.clone(),
                state: units::canonical_json(entity_state.clone()),
            });
            Vec::new()
        }
        PluginMutation::UpdateEntity { entity_id, scene_id, x, y, art, state: entity_state } => {
            let Some(index) = state.plugin_entities.iter().position(|entity| entity.id == *entity_id) else {
                return Some(Vec::new());
            };
            let entity = &state.plugin_entities[index];
            if !may_change_entity(plugin_id, &entity.type_id) {
                return Some(refuse(format!("updateEntity: {entity_id} is not an entity of this plugin's pack")));
            }
            let scene = scene_id.clone().unwrap_or_else(|| entity.scene_id.clone());
            let (nx, ny) = (x.unwrap_or(entity.x), y.unwrap_or(entity.y));
            if !on_map(state, &scene, nx, ny) {
                return Some(refuse(format!("updateEntity: ({nx}, {ny}) is not a tile of scene {scene}")));
            }
            if entity_state.as_ref().is_some_and(|value| state_size(value) > MAX_PLUGIN_STATE_BYTES) {
                return Some(refuse(format!("updateEntity: state is larger than {MAX_PLUGIN_STATE_BYTES} bytes")));
            }
            let entity = &mut state.plugin_entities[index];
            entity.scene_id = scene;
            entity.x = nx;
            entity.y = ny;
            if let Some(art) = art {
                entity.art = (!art.is_empty()).then(|| art.clone());
            }
            if let Some(value) = entity_state {
                entity.state = units::canonical_json(value.clone());
            }
            Vec::new()
        }
        PluginMutation::RemoveEntity { entity_id } => {
            if state
                .plugin_entities
                .iter()
                .any(|entity| entity.id == *entity_id && !may_change_entity(plugin_id, &entity.type_id))
            {
                return Some(refuse(format!("removeEntity: {entity_id} is not an entity of this plugin's pack")));
            }
            state.plugin_entities.retain(|entity| entity.id != *entity_id);
            Vec::new()
        }
        PluginMutation::StartTimer { timer, minutes, days, data } => {
            if state_size(data) > MAX_PLUGIN_STATE_BYTES {
                return Some(refuse(format!("startTimer: data is larger than {MAX_PLUGIN_STATE_BYTES} bytes")));
            }
            state.plugin_timers.retain(|running| !(running.plugin_id == plugin_id && running.id == *timer));
            if state.plugin_timers.iter().filter(|running| running.plugin_id == plugin_id).count() >= MAX_PLUGIN_TIMERS
            {
                return Some(refuse(format!("startTimer: a plugin may run at most {MAX_PLUGIN_TIMERS} timers")));
            }
            let day_minutes = i64::from(ctx.content.settings.time.minutes_per_day());
            let delay = (i64::from(*days) * day_minutes + i64::from(*minutes)).max(1) * i64::from(units::MINUTE);
            state.plugin_timers.push(PluginTimer {
                plugin_id: plugin_id.to_owned(),
                id: timer.clone(),
                due_at_minute: crafting::absolute_minute_in(ctx.content.settings.time.minutes_per_day(), state)
                    .saturating_add(delay),
                data: units::canonical_json(data.clone()),
            });
            Vec::new()
        }
        PluginMutation::CancelTimer { timer } => {
            state.plugin_timers.retain(|running| !(running.plugin_id == plugin_id && running.id == *timer));
            Vec::new()
        }
        _ => return None,
    })
}

fn emit(ctx: &EngineContext, hook: String, plugin_id: Option<String>, payload: Value) {
    ctx.emit(HookEvent::Extension(ExtensionHookPayload { hook, plugin_id, payload }));
}

fn entity_json(entity: &PluginEntity) -> Value {
    serde_json::to_value(entity).unwrap_or(Value::Null)
}

/// The entity types with entities in the world, in order of their first entity.
fn types_present(state: &GameState) -> Vec<String> {
    let mut types: Vec<String> = Vec::new();
    for entity in &state.plugin_entities {
        if !types.contains(&entity.type_id) {
            types.push(entity.type_id.clone());
        }
    }
    types
}

/// One `entity:<type>` event per type present, `{ op, …extra, entities }`.
fn emit_per_type(ctx: &EngineContext, state: &GameState, op: &str, extra: &[(&str, Value)]) {
    for type_id in types_present(state) {
        let entities: Vec<Value> =
            state.plugin_entities.iter().filter(|entity| entity.type_id == type_id).map(entity_json).collect();
        let mut payload = serde_json::Map::new();
        payload.insert("op".to_owned(), Value::String(op.to_owned()));
        for (key, value) in extra {
            payload.insert((*key).to_owned(), value.clone());
        }
        payload.insert("entities".to_owned(), Value::Array(entities));
        emit(ctx, format!("{}{type_id}", extension_hooks::ENTITY), None, Value::Object(payload));
    }
}

/// A whole game minute passed: fire the timers now due (taken out of the state first, in the
/// order they were started) and tick every entity type present.
pub fn advance_minute(ctx: &EngineContext, state: &mut GameState) {
    if !state.plugin_timers.is_empty() {
        let now = crafting::absolute_minute_in(ctx.content.settings.time.minutes_per_day(), state);
        let (due, running): (Vec<PluginTimer>, Vec<PluginTimer>) =
            std::mem::take(&mut state.plugin_timers).into_iter().partition(|timer| timer.due_at_minute <= now);
        state.plugin_timers = running;
        for timer in due {
            emit(
                ctx,
                extension_hooks::TIMER.to_owned(),
                Some(timer.plugin_id.clone()),
                json!({ "timer": timer.id, "data": timer.data }),
            );
        }
    }
    if !state.plugin_entities.is_empty() {
        let minute = units::whole_minute(state.clock.time_minutes);
        emit_per_type(ctx, state, "tick", &[("minute", json!(minute))]);
    }
}

/// A new day started: every entity type present hears `dayStart`.
pub fn day_started(ctx: &EngineContext, state: &GameState) {
    if !state.plugin_entities.is_empty() {
        emit_per_type(ctx, state, "dayStart", &[("day", json!(state.clock.day))]);
    }
}

/// The plugin entity on tile `(x, y)` of the player's scene, if any (the last spawned wins).
pub fn entity_at(state: &GameState, x: i32, y: i32) -> Option<&PluginEntity> {
    state
        .plugin_entities
        .iter()
        .rev()
        .find(|entity| entity.scene_id == state.player.scene_id && entity.x == x && entity.y == y)
}

/// The player interacts with tile `(x, y)`: when a plugin entity stands there, its type hears
/// `interact` and the interaction ends there (`true`).
pub fn interact(ctx: &EngineContext, state: &GameState, x: i32, y: i32) -> bool {
    let Some(entity) = entity_at(state, x, y) else { return false };
    emit(
        ctx,
        format!("{}{}", extension_hooks::ENTITY, entity.type_id),
        None,
        json!({ "op": "interact", "entity": entity_json(entity) }),
    );
    true
}

/// The plugin handler an item's `useActionId` names (`plugin:<handler>`), with a plain handler
/// name read as the item's own pack's (`packId:<handler>` for an item `packId:…`).
pub fn use_handler(item_id: &str, use_action_id: &str) -> Option<String> {
    let handler = use_action_id.strip_prefix(PLUGIN_USE_PREFIX)?.trim();
    if handler.is_empty() {
        return None;
    }
    if handler.contains(':') {
        return Some(handler.to_owned());
    }
    Some(match item_id.split_once(':') {
        Some((pack, _)) => format!("{pack}:{handler}"),
        None => handler.to_owned(),
    })
}

/// The player used item `item_id`, whose `useActionId` names the plugin handler `handler`:
/// `use:<handler>` hears `{ itemId, sceneId, x, y }` with the tile the player faces.
pub fn use_item(ctx: &EngineContext, state: &GameState, item_id: &str, handler: &str) {
    let target = world_movement::facing_target(state);
    emit(
        ctx,
        format!("{}{handler}", extension_hooks::ITEM_USE),
        None,
        json!({ "itemId": item_id, "sceneId": state.player.scene_id, "x": target.x, "y": target.y }),
    );
}

/// A plugin outcome (an event outcome of type `plugin`) ran: `outcome:<name>` hears its
/// arguments.
pub fn run_outcome(ctx: &EngineContext, name: &str, args: &Value) {
    emit(ctx, format!("{}{name}", extension_hooks::OUTCOME), None, json!({ "args": args }));
}
