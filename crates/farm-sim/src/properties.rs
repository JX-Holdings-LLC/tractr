//! Creator-defined properties (#244): reading, bounded writing, change over game time and
//! threshold effects. The schema is in [`crate::schema::properties`].
//!
//! A property's values are variables (`GameState::flags`), one per instance:
//!
//! | Scope | Variable |
//! |---|---|
//! | player | `oxygen` |
//! | scene | `pressure@scene-greenhouse` |
//! | object (a placed machine) | `charge@scene-farm:4,7` |
//!
//! Conditions and outcomes name a property by its id. For a scene or object property the bare id
//! means the instance in view: the scene the player is in (or the machine they face), or, while
//! one instance's own rates and thresholds are worked out, that instance. `pressure@scene-b`
//! names another scene's, and `pressure@here` spells out the default.
//!
//! Every write goes through [`write`]: the value is kept within the property's bounds, and a
//! threshold whose comparison starts to hold runs its outcomes. Values change with the clock
//! ([`advance`]: each whole in-game minute gets its exact share of the hourly rates, so an hour
//! always moves a property by its rate whatever the tick rate) and with the hours a night's
//! sleep skips ([`advance_skipped`]).

use crate::effects::{message_levels, Effect};
use crate::engine_types::{Effects, EngineContext};
use crate::events;
use crate::schema::{property_scopes, GameContent, GameState, PropertyDefinition, Scene};
use crate::units;
use serde_json::Value;

/// The instance whose own rates or thresholds are being worked out: what a bare scene or object
/// property id means meanwhile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyScope {
    Scene(String),
    /// `scene:x,y` of a placed machine.
    Object(String),
}

/// Threshold outcomes that write properties may cross further thresholds; this deep and no more.
const MAX_THRESHOLD_DEPTH: u32 = 4;

/// The separator between a property id and its instance in a variable name.
pub const INSTANCE_SEPARATOR: char = '@';
/// `pressure@here`: the instance in view, spelled out.
pub const HERE: &str = "here";

/// The property `id` of the game.
pub fn definition<'a>(content: &'a GameContent, id: &str) -> Option<&'a PropertyDefinition> {
    content.properties.iter().find(|definition| definition.id == id)
}

/// The key of a machine instance: `scene:x,y`.
pub fn object_key(scene_id: &str, x: i32, y: i32) -> String {
    format!("{scene_id}:{x},{y}")
}

/// The variable holding `definition`'s value for `instance` (a scene id or an object key; unused
/// for a player property).
pub fn variable_name(definition: &PropertyDefinition, instance: &str) -> String {
    if definition.scope == property_scopes::PLAYER {
        definition.id.clone()
    } else {
        format!("{}{INSTANCE_SEPARATOR}{instance}", definition.id)
    }
}

/// Whether `scene` has the scene property `definition`: an authored scene the property lists (or
/// any authored scene when it lists none). Generated scenes (mine floors) have none.
fn scene_has(definition: &PropertyDefinition, scene: &Scene) -> bool {
    !scene.is_generated() && definition.scenes.as_ref().is_none_or(|scenes| scenes.contains(&scene.id))
}

/// The machine type on the tile an object key names, when there is a machine.
fn machine_at<'a>(state: &'a GameState, key: &str) -> Option<&'a str> {
    let (scene_id, tile) = key.rsplit_once(':')?;
    let (x, y) = tile.split_once(',')?;
    let (x, y) = (x.parse::<i32>().ok()?, y.parse::<i32>().ok()?);
    let scene = state.world.scenes.iter().find(|scene| scene.id == scene_id)?;
    scene.tile(x, y)?.machine.as_ref().map(|machine| machine.type_id.as_str())
}

/// What the scene property `definition` starts at in `scene`: the scene's own start value, else
/// the property's.
fn start_value(definition: &PropertyDefinition, scene: &Scene) -> i64 {
    scene.properties.as_ref().and_then(|values| values.get(&definition.id)).copied().unwrap_or(definition.initial)
}

fn object_has(definition: &PropertyDefinition, machine_type: &str) -> bool {
    definition.objects.as_ref().is_none_or(|types| types.iter().any(|id| id == machine_type))
}

/// The machine the player faces, as an object key.
fn faced_object(state: &GameState) -> Option<String> {
    let facing = crate::world::world_movement::facing_target(state);
    let key = object_key(&state.player.scene_id, facing.x, facing.y);
    machine_at(state, &key).map(|_| key)
}

/// A property instance: its definition, the variable that holds it and what it starts at.
#[derive(Debug, Clone, PartialEq)]
pub struct Instance<'a> {
    pub definition: &'a PropertyDefinition,
    pub variable: String,
    pub initial: i64,
    scope: Option<PropertyScope>,
}

/// The instance the variable `name` means now, when it names a property (and an instance the
/// property has): `id`, `id@here`, `id@<scene id>` or `id@<scene id>:<x>,<y>`.
pub fn resolve<'a>(ctx: &'a EngineContext, state: &GameState, name: &str) -> Option<Instance<'a>> {
    let (id, target) = match name.split_once(INSTANCE_SEPARATOR) {
        Some((id, target)) => (id, Some(target).filter(|target| *target != HERE)),
        None => (name, None),
    };
    let definition = definition(&ctx.content, id)?;
    let in_view = ctx.property_scope.borrow().clone();
    match definition.scope.as_str() {
        property_scopes::SCENE => {
            let scene_id = match (target, in_view) {
                (Some(target), _) => target.to_owned(),
                (None, Some(PropertyScope::Scene(scene_id))) => scene_id,
                // A machine's rates read the pressure of the scene it stands in.
                (None, Some(PropertyScope::Object(key))) => key.rsplit_once(':')?.0.to_owned(),
                (None, None) => state.player.scene_id.clone(),
            };
            let scene = state.world.scenes.iter().find(|scene| scene.id == scene_id)?;
            scene_has(definition, scene).then(|| Instance {
                definition,
                variable: variable_name(definition, &scene_id),
                initial: start_value(definition, scene),
                scope: Some(PropertyScope::Scene(scene_id)),
            })
        }
        property_scopes::OBJECT => {
            let key = match (target, in_view) {
                (Some(target), _) => target.to_owned(),
                (None, Some(PropertyScope::Object(key))) => key,
                (None, _) => faced_object(state)?,
            };
            let machine_type = machine_at(state, &key)?;
            object_has(definition, machine_type).then(|| Instance {
                definition,
                variable: variable_name(definition, &key),
                initial: definition.initial,
                scope: Some(PropertyScope::Object(key)),
            })
        }
        _ => target.is_none().then(|| Instance {
            definition,
            variable: definition.id.clone(),
            initial: definition.initial,
            scope: None,
        }),
    }
}

fn stored(state: &GameState, variable: &str) -> Option<i64> {
    match state.flags.get(variable) {
        Some(Value::Number(number)) => Some(units::milli_of_number(number)),
        _ => None,
    }
}

fn clamp(definition: &PropertyDefinition, value: i64) -> i64 {
    // Bounds the wrong way round (Problems reports them) keep the lower one.
    value.clamp(definition.min, definition.max.max(definition.min))
}

impl Instance<'_> {
    /// Its value now, in thousandths: what is stored, else what it starts at.
    pub fn value(&self, state: &GameState) -> i64 {
        clamp(self.definition, stored(state, &self.variable).unwrap_or(self.initial))
    }
}

/// The value of the variable `name` when it names a property (thousandths); `None` for a plain
/// variable, or a property instance that does not exist (a scene without the property).
pub fn read(ctx: &EngineContext, state: &GameState, name: &str) -> Option<i64> {
    resolve(ctx, state, name).map(|instance| instance.value(state))
}

/// The value of `definition` for the HUD and other readers without an engine context: the
/// player's, or (scene scope) that of the scene the player is in; `None` for an object property
/// or a scene without it.
pub fn value_in_view(content: &GameContent, state: &GameState, definition: &PropertyDefinition) -> Option<i64> {
    let _ = content;
    match definition.scope.as_str() {
        property_scopes::PLAYER => Some(clamp(definition, stored(state, &definition.id).unwrap_or(definition.initial))),
        property_scopes::SCENE => {
            let scene = state.world.scenes.iter().find(|scene| scene.id == state.player.scene_id)?;
            scene_value(state, definition, scene)
        }
        _ => None,
    }
}

/// The value of the scene property `definition` in `scene`; `None` when the scene lacks it.
fn scene_value(state: &GameState, definition: &PropertyDefinition, scene: &Scene) -> Option<i64> {
    scene_has(definition, scene).then(|| {
        let initial = start_value(definition, scene);
        clamp(definition, stored(state, &variable_name(definition, &scene.id)).unwrap_or(initial))
    })
}

/// What each property is in `scene`, for the crops that need one in a range: the player's
/// properties and the scene's own. Empty in a game without properties.
pub fn values_in_scene<'a>(content: &'a GameContent, state: &GameState, scene: &Scene) -> Vec<(&'a str, i64)> {
    content
        .properties
        .iter()
        .filter_map(|definition| {
            let value = match definition.scope.as_str() {
                property_scopes::PLAYER => {
                    Some(clamp(definition, stored(state, &definition.id).unwrap_or(definition.initial)))
                }
                property_scopes::SCENE => scene_value(state, definition, scene),
                _ => None,
            };
            value.map(|value| (definition.id.as_str(), value))
        })
        .collect()
}

fn number_value(milli: i64) -> Value {
    units::canonical_json(units::json::<units::SignedMilli>(milli))
}

/// Sets the variable `name` to `value` (thousandths) when it names a property: kept within the
/// property's bounds, and thresholds it crosses run. `None` when `name` is a plain variable (the
/// caller stores it as before); a property instance that does not exist takes no value.
pub fn write(ctx: &EngineContext, state: &mut GameState, name: &str, value: i64) -> Option<Effects> {
    // A name that is a property's but not an instance it has (a scene without it): nothing to
    // write, and not a plain variable either.
    let id = name.split_once(INSTANCE_SEPARATOR).map_or(name, |(id, _)| id);
    definition(&ctx.content, id)?;
    let Some(instance) = resolve(ctx, state, name) else { return Some(Vec::new()) };
    Some(set(ctx, state, &instance, value))
}

/// [`write`] of the value plus `delta`.
pub fn add(ctx: &EngineContext, state: &mut GameState, name: &str, delta: i64) -> Option<Effects> {
    let id = name.split_once(INSTANCE_SEPARATOR).map_or(name, |(id, _)| id);
    definition(&ctx.content, id)?;
    let Some(instance) = resolve(ctx, state, name) else { return Some(Vec::new()) };
    let value = instance.value(state).saturating_add(delta);
    Some(set(ctx, state, &instance, value))
}

/// Stores an instance's value within its bounds and runs the thresholds it crossed, read for
/// that instance.
fn set(ctx: &EngineContext, state: &mut GameState, instance: &Instance<'_>, value: i64) -> Effects {
    let definition = instance.definition;
    let before = instance.value(state);
    let after = clamp(definition, value);
    state.flags.insert(instance.variable.clone(), number_value(after));
    let mut effects = Vec::new();
    if before == after || definition.thresholds.is_empty() {
        return effects;
    }
    let depth = ctx.property_depth.get();
    if depth >= MAX_THRESHOLD_DEPTH {
        return effects;
    }
    ctx.property_depth.set(depth + 1);
    let outer = ctx.property_scope.replace(instance.scope.clone());
    for threshold in &definition.thresholds {
        if threshold.holds(after) && !threshold.holds(before) {
            if let Some(message) = threshold.message.as_deref().filter(|message| !message.is_empty()) {
                let text = crate::substitution::fill(&ctx.content, state, message).into_owned();
                effects.push(Effect::message(message_levels::INFO, text));
            }
            effects.extend(events::apply_outcomes(ctx, state, &threshold.outcomes, 0));
        }
    }
    ctx.property_scope.replace(outer);
    ctx.property_depth.set(depth);
    effects
}

/// Every instance of the game's properties that exists now, in content order (scenes and tiles
/// in world order): the player's, each scene's, each placed machine's.
fn instances<'a>(ctx: &'a EngineContext, state: &GameState, only_with_rates: bool) -> Vec<Instance<'a>> {
    let mut found = Vec::new();
    for definition in &ctx.content.properties {
        if only_with_rates && definition.rates.is_empty() {
            continue;
        }
        match definition.scope.as_str() {
            property_scopes::SCENE => {
                for scene in state.world.scenes.iter().filter(|scene| scene_has(definition, scene)) {
                    found.push(Instance {
                        definition,
                        variable: variable_name(definition, &scene.id),
                        initial: start_value(definition, scene),
                        scope: Some(PropertyScope::Scene(scene.id.clone())),
                    });
                }
            }
            property_scopes::OBJECT => {
                for scene in &state.world.scenes {
                    for (y, row) in scene.tiles.iter().enumerate() {
                        for (x, tile) in row.iter().enumerate() {
                            let Some(machine) = tile.machine.as_ref() else { continue };
                            if !object_has(definition, &machine.type_id) {
                                continue;
                            }
                            let key = object_key(&scene.id, x as i32, y as i32);
                            found.push(Instance {
                                definition,
                                variable: variable_name(definition, &key),
                                initial: definition.initial,
                                scope: Some(PropertyScope::Object(key)),
                            });
                        }
                    }
                }
            }
            _ => found.push(Instance {
                definition,
                variable: definition.id.clone(),
                initial: definition.initial,
                scope: None,
            }),
        }
    }
    found
}

/// Writes the start value of every player and scene property that has no value yet, so
/// everything that reads variables (plugins, panels, test states, replay expectations) sees
/// them: a new game, a save from before the property existed, a scene added since.
pub fn materialize(ctx: &EngineContext, state: &mut GameState) {
    if ctx.content.properties.is_empty() {
        return;
    }
    for instance in instances(ctx, state, false) {
        if instance.definition.scope != property_scopes::OBJECT && stored(state, &instance.variable).is_none() {
            let value = clamp(instance.definition, instance.initial);
            state.flags.insert(instance.variable, number_value(value));
        }
    }
}

/// The hourly rate of an instance now: its rates whose conditions hold for it, added up.
fn rate_now(ctx: &EngineContext, state: &GameState, instance: &Instance<'_>) -> i64 {
    let outer = ctx.property_scope.replace(instance.scope.clone());
    let rate = instance
        .definition
        .rates
        .iter()
        .filter(|rate| rate.conditions.iter().all(|condition| events::condition_met(ctx, state, condition, None)))
        .fold(0_i64, |sum, rate| sum.saturating_add(rate.per_hour));
    ctx.property_scope.replace(outer);
    rate
}

/// Moves every property with rates by its share of the clock going from `before_minute` to
/// `after_minute` of the day: `rate × minute / 60` rounded down at each end, so the shares of an
/// hour add up to the rate exactly.
pub fn advance(ctx: &EngineContext, state: &mut GameState, before_minute: u32, after_minute: u32) -> Effects {
    if ctx.content.properties.is_empty() {
        return Vec::new();
    }
    materialize(ctx, state);
    forget_gone_objects(ctx, state);
    let mut effects = Vec::new();
    for instance in instances(ctx, state, true) {
        let rate = rate_now(ctx, state, &instance);
        let share = |minute: u32| rate.saturating_mul(i64::from(minute)).div_euclid(60);
        let delta = share(after_minute) - share(before_minute);
        if delta != 0 {
            let value = instance.value(state).saturating_add(delta);
            effects.extend(set(ctx, state, &instance, value));
        }
    }
    effects
}

/// The hours a night skips (bed or collapse to the next morning): every property with rates
/// moves by `minutes` of its rate as it is now.
pub fn advance_skipped(ctx: &EngineContext, state: &mut GameState, minutes: u32) -> Effects {
    if ctx.content.properties.is_empty() || minutes == 0 {
        return Vec::new();
    }
    materialize(ctx, state);
    let mut effects = Vec::new();
    for instance in instances(ctx, state, true) {
        let delta = rate_now(ctx, state, &instance).saturating_mul(i64::from(minutes)).div_euclid(60);
        if delta != 0 {
            let value = instance.value(state).saturating_add(delta);
            effects.extend(set(ctx, state, &instance, value));
        }
    }
    effects
}

/// Drops the values of machines that are gone (picked up, removed), so a machine placed on the
/// same tile later starts fresh.
fn forget_gone_objects(ctx: &EngineContext, state: &mut GameState) {
    let object_ids: Vec<&str> = ctx
        .content
        .properties
        .iter()
        .filter(|definition| definition.scope == property_scopes::OBJECT)
        .map(|definition| definition.id.as_str())
        .collect();
    if object_ids.is_empty() {
        return;
    }
    let gone: Vec<String> = state
        .flags
        .keys()
        .filter(|name| {
            name.split_once(INSTANCE_SEPARATOR)
                .is_some_and(|(id, key)| object_ids.contains(&id) && machine_at(state, key).is_none())
        })
        .cloned()
        .collect();
    for name in gone {
        state.flags.shift_remove(&name);
    }
}
