//! The read API (docs/PLUGINS.md "Reading the game"): read-only queries a plugin makes while a
//! handler runs, answered from the game state of the step its hook fired at.
//!
//! A plugin calls `api.state.player()`, `api.state.tile(x, y)`, … (the [`crate::PRELUDE`]
//! helpers); each is one host call (`api.host({ read: 'player' })`, guest ABI 2) that
//! [`answer_read`] answers here. Answers are plain JSON in authoring units (tiles, whole gold,
//! energy points, game minutes), built from the engine's own serialization so they read like
//! the project and the save.
//!
//! - **Permissions.** The pack manifest's `permissions.read` lists what its plugins may read:
//!   `player`, `world`, `people`, `content`, and `any` (flags and variables beyond the pack's
//!   own). A query outside them throws in the plugin with a "not permitted" message.
//! - **Own ids.** Flags and variables are the pack's own (`flag('met')` reads
//!   `packId:met`) unless `any` is granted, the same rule as mutations. Content ids (items,
//!   crops, NPCs, quests) look for the pack's own definition first (`packId:id`), then the id as
//!   written.
//! - **Determinism.** The answer depends only on the content and the state at the step the hook
//!   fired, never on the frame or the machine; each query costs fuel ([`READ_BASE_FUEL`] plus
//!   [`READ_FUEL_PER_BYTE`] per answer byte), so it counts toward the handler's budget like
//!   any JavaScript it runs.

use crate::payload::to_engine_json;
use crate::sandbox::HostCalls;
use farm_sim::schema::GameState;
use farm_sim::{game_time, social, units, EngineContext};
use serde::Serialize;
use serde_json::{json, Map, Value};

/// The entries `permissions.read` understands: the four kinds of queries, and `any` (flags and
/// variables beyond the pack's own).
pub const READ_SCOPES: &[&str] = &["player", "world", "people", "content", "any"];

/// Fuel every query costs, on top of the bytes of its answer.
pub const READ_BASE_FUEL: u64 = 10_000;

/// Fuel per byte of a query's answer (the guest then parses it, which costs fuel of its own).
pub const READ_FUEL_PER_BYTE: u64 = 16;

/// Most queries one handler call may make; further ones throw.
pub const MAX_READS_PER_CALL: u32 = 2_000;

/// What a plugin may read, from `permissions.read`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReadGrants {
    /// The player: position, money, energy, inventory, skills, buffs, …
    pub player: bool,
    /// The world: date and weather, tiles, crops, nodes, machines, animals, buildings.
    pub world: bool,
    /// People and story: NPCs, friendship, quests, flags and variables.
    pub people: bool,
    /// Content definitions (items, crops, NPCs, quests, …) by id.
    pub content: bool,
    /// Flags and variables of the game and other packs (not only the pack's own).
    pub any_flags: bool,
    /// A project script (trusted tier): everything, and ids are never namespaced.
    pub trusted: bool,
}

impl ReadGrants {
    /// The grants of a `permissions.read` list; unknown entries grant nothing.
    pub fn new(entries: &[String]) -> Self {
        let has = |name: &str| entries.iter().any(|entry| entry.trim() == name);
        Self {
            player: has("player"),
            world: has("world"),
            people: has("people"),
            content: has("content"),
            any_flags: has("any"),
            trusted: false,
        }
    }

    /// Everything, with ids as written: the trusted tier of a creator's project scripts.
    pub fn trusted() -> Self {
        Self { player: true, world: true, people: true, content: true, any_flags: true, trusted: true }
    }

    /// Whether any query is allowed.
    pub fn any(&self) -> bool {
        self.player || self.world || self.people || self.content
    }
}

/// The game a query reads: the content and the state of the step a hook fired at.
#[derive(Debug, Clone, Copy)]
pub struct GameView<'a> {
    /// The context of the running game (content and its id tables).
    pub ctx: &'a EngineContext,
    /// The state after the step.
    pub state: &'a GameState,
}

/// A read refused or malformed: the message the plugin's `Error` carries.
fn refuse(message: impl Into<String>) -> String {
    message.into()
}

fn serialized<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

/// `value` (an object) with only `keys`, in that order; missing keys are left out.
fn pick(value: &Value, keys: &[&str]) -> Value {
    let mut out = Map::new();
    for key in keys {
        if let Some(field) = value.get(*key) {
            out.insert((*key).to_owned(), field.clone());
        }
    }
    Value::Object(out)
}

/// `value` (an object) without `keys` (art and other bulky presentation fields).
fn without(mut value: Value, keys: &[&str]) -> Value {
    if let Value::Object(map) = &mut value {
        for key in keys {
            map.shift_remove(*key);
        }
    }
    value
}

fn text_arg<'r>(request: &'r Value, key: &str) -> Result<&'r str, String> {
    request
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty() && text.len() <= crate::MAX_ID_LENGTH)
        .ok_or_else(|| refuse(format!("{} needs a string \"{key}\"", kind_of(request))))
}

fn optional_text_arg<'r>(request: &'r Value, key: &str) -> Option<&'r str> {
    request.get(key).and_then(Value::as_str).filter(|text| !text.is_empty())
}

fn int_arg(request: &Value, key: &str) -> Result<i32, String> {
    request
        .get(key)
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite() && n.fract() == 0.0 && n.abs() <= 1_000_000.0)
        .map(|n| n as i32)
        .ok_or_else(|| refuse(format!("{} needs a whole number \"{key}\"", kind_of(request))))
}

fn kind_of(request: &Value) -> &str {
    request.get("read").and_then(Value::as_str).unwrap_or("a read")
}

/// A content id as a plugin of `pack_id` means it: its own `packId:id` when that exists, else
/// as written.
fn content_id(pack_id: &str, id: &str, trusted: bool, exists: impl Fn(&str) -> bool) -> String {
    if !trusted && !pack_id.is_empty() && !id.contains(':') {
        let own = format!("{pack_id}:{id}");
        if exists(&own) {
            return own;
        }
    }
    id.to_owned()
}

/// A flag or variable name as a plugin may read it: the pack's own unless `any` is granted.
fn flag_name(pack_id: &str, grants: &ReadGrants, name: &str) -> Result<String, String> {
    if grants.trusted || grants.any_flags || pack_id.is_empty() {
        return Ok(name.to_owned());
    }
    match name.split_once(':') {
        None => Ok(format!("{pack_id}:{name}")),
        Some((pack, _)) if pack == pack_id => Ok(name.to_owned()),
        Some(_) => Err(refuse(format!(
            "reading flag \"{name}\" is not permitted: a plugin reads its own pack's flags unless the manifest's permissions.read lists \"any\""
        ))),
    }
}

/// The scope a read needs, by its `read` kind; `None` for an unknown kind.
pub fn read_scope(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "player" | "inventory" | "itemCount" => "player",
        "date" | "tile" | "animals" | "buildings" | "entities" | "timers" => "world",
        "npc" | "npcs" | "quest" | "quests" | "flag" | "variable" => "people",
        "item" | "crop" | "definition" => "content",
        _ => return None,
    })
}

/// Answer one read `request` (`{ "read": kind, … }`) for a plugin of `pack_id` with `grants`.
/// `Ok` is the value the plugin gets; `Err` the message it throws.
pub fn answer_read(game: &GameView<'_>, pack_id: &str, grants: &ReadGrants, request: &Value) -> Result<Value, String> {
    let kind = request.get("read").and_then(Value::as_str).ok_or_else(|| refuse("a read needs a \"read\" kind"))?;
    let scope = read_scope(kind).ok_or_else(|| refuse(format!("unknown read \"{kind}\"")))?;
    let allowed = grants.trusted
        || match scope {
            "player" => grants.player,
            "world" => grants.world,
            "people" => grants.people,
            _ => grants.content,
        };
    if !allowed {
        return Err(refuse(format!(
            "reading {kind} is not permitted: the pack manifest's permissions.read does not list \"{scope}\""
        )));
    }
    let (ctx, state) = (game.ctx, game.state);
    let trusted = grants.trusted;
    Ok(match kind {
        "player" => player(ctx, state),
        "inventory" => Value::Array(state.player.inventory.iter().map(slot).collect()),
        "itemCount" => {
            let id = text_arg(request, "itemId")?;
            let id = content_id(pack_id, id, trusted, |id| ctx.item(id).is_some());
            let count: u64 = state
                .player
                .inventory
                .iter()
                .filter(|slot| slot.item.id == id)
                .map(|slot| u64::from(slot.quantity))
                .sum();
            json!(count)
        }
        "date" => date(ctx, state),
        "tile" => tile(state, request)?,
        "animals" => Value::Array(
            state
                .animals
                .iter()
                .map(|animal| {
                    pick(
                        &serialized(animal),
                        &[
                            "id",
                            "speciesId",
                            "name",
                            "sceneId",
                            "x",
                            "y",
                            "mood",
                            "fedToday",
                            "pettedToday",
                            "ageDays",
                            "productReady",
                            "buildingId",
                        ],
                    )
                })
                .collect(),
        ),
        "buildings" => Value::Array(
            state
                .buildings
                .iter()
                .map(|building| {
                    let mut value = serialized(building);
                    if let Value::Object(map) = &mut value {
                        map.insert("built".to_owned(), Value::Bool(building.built()));
                    }
                    value
                })
                .collect(),
        ),
        "entities" => Value::Array(
            state
                .plugin_entities
                .iter()
                .filter(|entity| optional_text_arg(request, "sceneId").is_none_or(|scene| entity.scene_id == scene))
                .filter(|entity| {
                    optional_text_arg(request, "typeId").is_none_or(|type_id| {
                        let own = !trusted && !pack_id.is_empty() && !type_id.contains(':');
                        entity.type_id == if own { format!("{pack_id}:{type_id}") } else { type_id.to_owned() }
                    })
                })
                .map(serialized)
                .collect(),
        ),
        "timers" => Value::Array(
            state
                .plugin_timers
                .iter()
                .filter(|timer| trusted || timer.plugin_id.split_once(':').is_some_and(|(pack, _)| pack == pack_id))
                .map(serialized)
                .collect(),
        ),
        "npc" => {
            let id = text_arg(request, "npcId")?;
            let id = content_id(pack_id, id, trusted, |id| ctx.npc(id).is_some() || state.npcs.contains_key(id));
            npc(state, &id)
        }
        "npcs" => Value::Array(state.npcs.keys().map(|id| npc(state, id)).collect()),
        "quest" => {
            let id = text_arg(request, "questId")?;
            let id = content_id(pack_id, id, trusted, |id| ctx.quest(id).is_some() || state.quests.contains_key(id));
            quest(state, &id)
        }
        "quests" => {
            let mut map = Map::new();
            for id in state.quests.keys() {
                map.insert(id.clone(), quest(state, id));
            }
            Value::Object(map)
        }
        "flag" => {
            let name = flag_name(pack_id, grants, text_arg(request, "flag")?)?;
            state.flags.get(&name).cloned().unwrap_or(Value::Null)
        }
        "variable" => {
            let name = flag_name(pack_id, grants, text_arg(request, "variable")?)?;
            match state.flags.get(&name) {
                Some(Value::String(text)) => Value::String(text.clone()),
                Some(Value::Number(number)) => Value::Number(number.clone()),
                Some(Value::Bool(true)) => json!(1),
                _ => json!(0),
            }
        }
        "item" => {
            let id = text_arg(request, "itemId")?;
            let id = content_id(pack_id, id, trusted, |id| ctx.item(id).is_some());
            ctx.item(&id).map_or(Value::Null, |item| without(serialized(item), &["visual", "fishingGear"]))
        }
        "crop" => {
            let id = text_arg(request, "cropId")?;
            let find = |id: &str| ctx.content.crops.get(id);
            let id = content_id(pack_id, id, trusted, |id| find(id).is_some());
            find(&id).map_or(Value::Null, |crop| without(serialized(crop), &["visual", "light"]))
        }
        "definition" => definition(ctx, pack_id, trusted, request)?,
        _ => return Err(refuse(format!("unknown read \"{kind}\""))),
    })
}

fn slot(slot: &farm_sim::schema::InventorySlot) -> Value {
    let mut out = Map::new();
    out.insert("itemId".to_owned(), Value::String(slot.item.id.clone()));
    out.insert("quantity".to_owned(), json!(slot.quantity));
    if let Some(quality) = &slot.quality {
        out.insert("quality".to_owned(), Value::String(quality.clone()));
    }
    Value::Object(out)
}

fn player(ctx: &EngineContext, state: &GameState) -> Value {
    let player = &state.player;
    let raw = serialized(player);
    let mut out = pick(&raw, &["sceneId", "x", "y"]);
    let Value::Object(map) = &mut out else { return out };
    map.insert("tileX".to_owned(), json!(units::tile_of(player.x)));
    map.insert("tileY".to_owned(), json!(units::tile_of(player.y)));
    map.insert("facing".to_owned(), Value::String(player.direction.clone()));
    for key in ["money", "energy", "maxEnergy", "health", "maxHealth", "equippedTool", "name", "farmName"] {
        if let Some(value) = raw.get(key) {
            map.insert(key.to_owned(), value.clone());
        }
    }
    let mut skills = Map::new();
    for (id, skill) in &player.skills {
        skills.insert(
            id.clone(),
            json!({ "level": skill.level, "xp": skill.xp, "effectiveLevel": farm_sim::skills::effective_level(ctx, state, id) }),
        );
    }
    map.insert("skills".to_owned(), Value::Object(skills));
    map.insert(
        "worn".to_owned(),
        Value::Array(
            player
                .worn
                .iter()
                .map(|slot| {
                    json!({ "itemId": slot.item.id, "slot": slot.item.wear.as_ref().map(|wear| wear.slot.clone()) })
                })
                .collect(),
        ),
    );
    map.insert(
        "buffs".to_owned(),
        Value::Array(
            player
                .buffs
                .iter()
                .map(|buff| {
                    let definition = serialized(&buff.definition);
                    let mut value = pick(&definition, &["id", "stat", "amount"]);
                    if let Value::Object(fields) = &mut value {
                        fields.insert("source".to_owned(), Value::String(buff.source.clone()));
                        if let Some(expires) = serialized(buff).get("expiresAtMinute") {
                            fields.insert("expiresAtMinute".to_owned(), expires.clone());
                        }
                    }
                    value
                })
                .collect(),
        ),
    );
    map.insert("inventorySize".to_owned(), json!(player.max_inventory_size));
    out
}

fn date(ctx: &EngineContext, state: &GameState) -> Value {
    let calendar = &ctx.content.settings.calendar;
    let clock = &state.clock;
    let date = game_time::clock_date(calendar, clock);
    json!({
        "day": clock.day,
        "season": clock.season,
        "dayOfSeason": date.day_of_season,
        "year": clock.year,
        "minute": units::whole_minute(clock.time_minutes),
        "weather": clock.weather_id,
        "weekday": game_time::weekday(calendar, clock.day).map(|day| day.id.clone()),
        "festival": game_time::festival_today(calendar, clock).map(|festival| festival.id.clone()),
        "tick": clock.tick,
    })
}

fn tile(state: &GameState, request: &Value) -> Result<Value, String> {
    let x = int_arg(request, "x")?;
    let y = int_arg(request, "y")?;
    let scene_id = optional_text_arg(request, "sceneId").unwrap_or(&state.player.scene_id);
    let Some(scene) = state.world.scenes.iter().find(|scene| scene.id == scene_id) else { return Ok(Value::Null) };
    let Some(tile) = usize::try_from(y)
        .ok()
        .and_then(|y| scene.tiles.get(y))
        .and_then(|row| usize::try_from(x).ok().and_then(|x| row.get(x)))
    else {
        return Ok(Value::Null);
    };
    let crop = tile.crop.as_ref().map(|crop| {
        pick(
            &serialized(crop),
            &["type", "stage", "watered", "daysGrown", "withered", "quality", "mutation", "harvestCount"],
        )
    });
    let entities: Vec<Value> = state
        .plugin_entities
        .iter()
        .filter(|entity| entity.scene_id == scene.id && entity.x == x && entity.y == y)
        .map(|entity| Value::String(entity.id.clone()))
        .collect();
    Ok(json!({
        "sceneId": scene.id,
        "x": x,
        "y": y,
        "terrain": tile.background,
        "overlay": tile.overlay,
        "object": tile.object,
        "collision": tile.collision,
        "soil": tile.soil_state,
        "crop": crop,
        "node": tile.node.as_ref().map(|node| pick(&serialized(node), &["typeId", "remainingHealth"])),
        "machine": tile.machine.as_ref().map(|machine| {
            let raw = serialized(machine);
            json!({
                "typeId": machine.type_id,
                "processing": raw.get("processing").cloned(),
                "ready": machine.output.as_ref().is_some_and(|output| !output.is_empty()),
            })
        }),
        "entities": entities,
    }))
}

fn npc(state: &GameState, id: &str) -> Value {
    let Some(npc) = state.npcs.get(id) else { return Value::Null };
    let friendship = social::friendship_with(state, id);
    json!({
        "id": id,
        "sceneId": npc.scene_id,
        "x": npc.x,
        "y": npc.y,
        "friendship": friendship,
        "hearts": social::hearts(friendship),
        "relationship": social::relationship_status(state, id),
    })
}

fn quest(state: &GameState, id: &str) -> Value {
    match state.quests.get(id) {
        None => json!({ "status": "not-started", "objectives": {} }),
        Some(progress) => {
            let raw = serialized(progress);
            json!({
                "status": progress.status,
                "objectives": raw.get("objectives").cloned().unwrap_or_else(|| json!({})),
                "deadlineDay": raw.get("deadlineDay").cloned(),
            })
        }
    }
}

/// Any content definition by kind and id (`{ read: 'definition', kind: 'npc', id }`).
fn definition(ctx: &EngineContext, pack_id: &str, trusted: bool, request: &Value) -> Result<Value, String> {
    let kind = text_arg(request, "kind")?;
    let id = text_arg(request, "id")?;
    fn find<T: Serialize>(items: &[T], id: &str, key: impl Fn(&T) -> &str) -> Option<Value> {
        items.iter().find(|item| key(item) == id).map(serialized)
    }
    let content = &ctx.content;
    let lookup = |id: &str| -> Option<Value> {
        match kind {
            "item" => find(&content.items, id, |d| &d.id),
            "crop" => content.crops.get(id).map(serialized),
            "npc" => find(&content.npcs, id, |d| &d.id),
            "quest" => find(&content.quests, id, |d| &d.id),
            "shop" => find(&content.shops, id, |d| &d.id),
            "recipe" => find(&content.recipes, id, |d| &d.id),
            "machineType" => find(&content.machine_types, id, |d| &d.id),
            "nodeType" => find(&content.node_types, id, |d| &d.id),
            "animalSpecies" => find(&content.animal_species, id, |d| &d.id),
            "buildingType" => find(&content.building_types, id, |d| &d.id),
            "monster" => find(&content.monsters, id, |d| &d.id),
            _ => None,
        }
    };
    const KINDS: &[&str] = &[
        "item",
        "crop",
        "npc",
        "quest",
        "shop",
        "recipe",
        "machineType",
        "nodeType",
        "animalSpecies",
        "buildingType",
        "monster",
    ];
    if !KINDS.contains(&kind) {
        return Err(refuse(format!("definition: unknown kind \"{kind}\" (one of {})", KINDS.join(", "))));
    }
    let id = content_id(pack_id, id, trusted, |id| lookup(id).is_some());
    Ok(lookup(&id)
        .map_or(Value::Null, |value| without(value, &["visual", "portrait", "sprite", "light", "spriteSheet"])))
}

/// Answers a handler's host calls with reads of `game` (or refuses them when the dispatch has
/// no game, e.g. a minigame frame).
pub(crate) struct ReadHost<'a> {
    pub game: Option<GameView<'a>>,
    pub pack_id: &'a str,
    pub grants: ReadGrants,
    pub reads: u32,
}

impl<'a> ReadHost<'a> {
    pub(crate) fn new(game: Option<GameView<'a>>, pack_id: &'a str, grants: ReadGrants) -> Self {
        Self { game, pack_id, grants, reads: 0 }
    }
}

/// `{"ok": value}` or `{"error": message}` as engine-order JSON text.
pub(crate) fn envelope(result: Result<Value, String>) -> String {
    match result {
        Ok(value) => to_engine_json(&json!({ "ok": value })),
        Err(message) => to_engine_json(&json!({ "error": crate::clip(&message, crate::MAX_ERROR_MESSAGE_BYTES) })),
    }
}

impl HostCalls for ReadHost<'_> {
    fn answer(&mut self, request: &str) -> (String, u64) {
        self.reads += 1;
        let result = if self.reads > MAX_READS_PER_CALL {
            Err(format!("a handler may read at most {MAX_READS_PER_CALL} times per call"))
        } else {
            match (serde_json::from_str::<Value>(request), &self.game) {
                (Err(_), _) => Err("a read must be a JSON object".to_owned()),
                (Ok(_), None) => Err("the game cannot be read here".to_owned()),
                (Ok(request), Some(game)) if request.get("read").is_some() => {
                    answer_read(game, self.pack_id, &self.grants, &request)
                }
                (Ok(_), Some(_)) => Err("unknown host call (expected { read: … })".to_owned()),
            }
        };
        let text = envelope(result);
        let cost = READ_BASE_FUEL.saturating_add(READ_FUEL_PER_BYTE.saturating_mul(text.len() as u64));
        (text, cost)
    }
}
