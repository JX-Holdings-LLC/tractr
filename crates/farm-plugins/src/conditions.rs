//! Plugin conditions (#239): `api.condition(name, (args, state, context) => boolean)`, asked
//! synchronously whenever the engine evaluates a `plugin` condition (events, dialogue options,
//! NPC lines, quests, mail, schedules, actions…).
//!
//! [`PluginConditionHost`] implements [`farm_sim::PluginConditions`] with instances of their
//! own: every plugin that declares conditions gets one more sandbox instance, granted only its
//! `condition:` hooks, so its hook handlers' JavaScript state never mixes in. Conditions are
//! meant to be pure:
//!
//! - their answer is `[boolean]`; mutations are impossible (there is nowhere for them to go);
//! - each call has [`CONDITION_FUEL`];
//! - they read the game through the read API like any handler, at the state being evaluated;
//! - a condition that throws, runs out of fuel or answers something else is **false**, and the
//!   failure is noted once per message in the plugin errors (the debug drawer);
//! - a condition no running plugin defines (its pack is off or missing) is false.
//!
//! A condition that keeps JavaScript state between calls breaks determinism (the instance
//! starts over when the game loads); docs/PLUGINS.md says so.

use crate::host::{PluginHostOptions, WasmPluginHost};
use crate::payload::to_engine_json;
use crate::reads::GameView;
use crate::spec::PluginSpec;
use crate::{PluginError, PluginErrorKind};
use farm_sim::events::ConditionScope;
use farm_sim::hooks::extension_hooks;
use farm_sim::schema::GameState;
use farm_sim::EngineContext;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::{Arc, Mutex};

/// Fuel per condition call: a fifth of a hook handler's (about 8–20 ms in a release build).
pub const CONDITION_FUEL: u64 = 10_000_000;

/// The engine's answer to plugin conditions; install it as `EngineContext::plugin_conditions`.
pub struct PluginConditionHost {
    host: RefCell<WasmPluginHost>,
    errors: Arc<Mutex<Vec<PluginError>>>,
    seen: RefCell<BTreeSet<String>>,
}

impl fmt::Debug for PluginConditionHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PluginConditionHost").field("host", &self.host).finish_non_exhaustive()
    }
}

impl PluginConditionHost {
    /// A condition host for the plugins of `specs` that define conditions (and project
    /// scripts, which may); `None` when none does.
    pub fn new(specs: &[PluginSpec], options: &PluginHostOptions) -> Option<Self> {
        let specs: Vec<PluginSpec> = specs
            .iter()
            .filter_map(|spec| {
                let hooks: Vec<String> = spec
                    .granted_hooks
                    .iter()
                    .filter(|hook| hook.starts_with(extension_hooks::CONDITION))
                    .cloned()
                    .collect();
                (spec.trusted || !hooks.is_empty()).then(|| PluginSpec {
                    granted_hooks: hooks,
                    granted_minigames: Vec::new(),
                    ..spec.clone()
                })
            })
            .collect();
        if specs.is_empty() {
            return None;
        }
        let options = PluginHostOptions { fuel_per_call: CONDITION_FUEL, ..options.clone() };
        Some(Self {
            host: RefCell::new(WasmPluginHost::new(specs, options)),
            errors: Arc::new(Mutex::new(Vec::new())),
            seen: RefCell::new(BTreeSet::new()),
        })
    }

    /// Where failures go: the runtime drains them into its recent errors
    /// ([`crate::PluginRuntime::attach_condition_errors`]).
    pub fn errors(&self) -> Arc<Mutex<Vec<PluginError>>> {
        Arc::clone(&self.errors)
    }

    /// Note `error` once per message.
    fn note(&self, error: PluginError) {
        if self.seen.borrow_mut().insert(format!("{}|{}", error.plugin_id, error.message)) {
            if let Ok(mut errors) = self.errors.lock() {
                errors.push(error);
            }
        }
    }

    /// Ask condition `name` (`packId:name`, or a project script's plain name) with `args`.
    pub fn ask(&self, ctx: &EngineContext, state: &GameState, name: &str, args: &Value, context: Value) -> bool {
        let hook = format!("{}{name}", extension_hooks::CONDITION);
        let payload = to_engine_json(&json!({ "args": args, "context": context }));
        let game = GameView { ctx, state };
        let answer = self.host.borrow_mut().call_value(&hook, &payload, Some(game), Some(CONDITION_FUEL));
        let failed = |plugin_id: String, message: String| PluginError {
            plugin_id,
            kind: PluginErrorKind::Threw,
            message,
            hook: Some(hook.clone()),
        };
        for error in self.host.borrow().init_errors().iter().cloned() {
            self.note(error);
        }
        match answer {
            None => {
                self.note(failed(
                    name.split_once(':').map_or("(project)", |(pack, _)| pack).to_owned(),
                    format!("condition {name} is false: no running plugin defines it (is its pack turned off?)"),
                ));
                false
            }
            Some(Ok(Value::Array(items))) if items.len() == 1 && items[0].is_boolean() => items[0] == Value::Bool(true),
            Some(Ok(other)) => {
                let id = self.host.borrow().specs().find(|spec| spec.is_granted(&hook)).map(|spec| spec.id.clone());
                self.note(failed(
                    id.unwrap_or_default(),
                    format!("condition {name} is false: it answered {}", crate::clip(&other.to_string(), 120)),
                ));
                false
            }
            Some(Err(error)) => {
                self.note(PluginError { message: format!("condition {name} is false: {}", error.message), ..error });
                false
            }
        }
    }
}

use crate::host::PluginHost as _;

impl farm_sim::PluginConditions for PluginConditionHost {
    fn holds(
        &self,
        ctx: &EngineContext,
        state: &GameState,
        condition: &str,
        args: &Value,
        scope: &ConditionScope<'_>,
    ) -> bool {
        let context = json!({
            "npcId": scope.speaker,
            "x": scope.pos.map(|pos| pos.x),
            "y": scope.pos.map(|pos| pos.y),
        });
        self.ask(ctx, state, condition, args, context)
    }
}
