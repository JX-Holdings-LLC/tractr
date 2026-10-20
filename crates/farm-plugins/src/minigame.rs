//! Pack-defined minigames (docs/PLUGINS.md "Minigames"): a plugin defines a minigame kind with
//! `api.minigame(name, { start, update, draw })`, and [`PackMinigame`] runs one session of it.
//!
//! - **Its own instance.** A session runs in a fresh sandbox instance of the plugin (the same
//!   QuickJS image every plugin starts from), granted only that minigame. Nothing carries over
//!   from the plugin's hook handlers or from earlier sessions, so a session's frames depend only
//!   on its start (config, seed) and its inputs.
//! - **The host keeps the state.** `start(config, info)` answers the first state, `update(state,
//!   input, dt)` answers `{ state, done }`, and `draw(state)` answers the draw list for the state
//!   just answered. The state is plain JSON, at most [`MinigameLimits::max_state_bytes`] long,
//!   and goes into the sandbox and back once per frame (one guest call runs `update` and `draw`).
//! - **Strict limits.** A start (compiling the plugin, `start` and the first `draw`) has
//!   [`MinigameLimits::start_fuel`]; each frame (`update` and `draw`) has
//!   [`MinigameLimits::fuel_per_frame`]. Draw lists are checked by
//!   [`farm_runtime::canvas::parse_canvas`]. A session that breaks any limit, throws or answers
//!   something malformed fails: [`PackMinigame::frame`] returns the error, and the host ends the
//!   minigame as failed (score 0).
//!
//! The guest is unchanged: the minigame API is a few lines of JavaScript ([`PRELUDE`]) run
//! before every plugin's source, on top of the guest's `api.on`. Each minigame is the handler of
//! the hook `minigame:<name>`, which only a minigame session's instance is granted.

use crate::sandbox::{self, status, GuestFault, GuestInstance, Sandbox, MAX_RESULT_BYTES};
use crate::{PluginError, PluginErrorKind, PluginSpec};
use farm_runtime::canvas::{parse_canvas, CanvasLimits, MinigameCanvas, CANVAS_HEIGHT, CANVAS_WIDTH};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::fmt;

/// Hook name prefix of minigame handlers (`minigame:<name>`).
pub const MINIGAME_HOOK_PREFIX: &str = "minigame:";

/// Longest frame time an `update` is told about, in seconds (a stalled frame is not a jump).
pub const MAX_FRAME_SECONDS: f64 = 0.25;

/// Run before every plugin's source (inside its `function (api) { … }`): replaces `api` by one
/// that also has `api.minigame(name, game)`, which registers `game` as the handler of
/// `minigame:<name>`. The guest's `api.on` ignores hooks the instance is not granted, so in the
/// hook handlers' instance a minigame registers nothing, and in a minigame session's instance
/// only that minigame registers.
///
/// The handler takes `{ op: 'start', config, info }` and answers `[state, drawList]`, or
/// `{ op: 'frame', state, input, dt }` and answers `[updateAnswer, drawList]`; `draw` gets the
/// state `update` answered (the old one when it answered none). Errors name the function they
/// came from (`update: …`). One line, so it moves the plugin's line numbers by one.
pub const PRELUDE: &str = "api = (function (base) { var freeze = Object.freeze; return freeze({ on: base.on, \
minigame: function minigame(name, game) { if (typeof name !== 'string' || game === null || typeof game !== 'object') \
throw new TypeError('api.minigame(name, { start, update, draw })'); \
var run = function (op, a, b, c) { var fn = game[op]; if (typeof fn !== 'function') { if (op === 'start') return null; \
throw new TypeError(op + ': minigame ' + name + ' has no ' + op + ' function'); } \
try { return fn.call(game, a, b, c); } catch (e) { throw new Error(op + ': ' + (e instanceof Error ? e.message : String(e))); } }; \
base.on('minigame:' + name, function (call) { if (call.op === 'start') { var first = run('start', call.config, call.info); \
return [first, run('draw', first)]; } var answer = run('update', call.state, call.input, call.dt); \
var next = answer !== null && typeof answer === 'object' && answer.state !== undefined ? answer.state : call.state; \
return [answer, run('draw', next)]; }); } }); })(api);";

/// The source a plugin's instances run: [`PRELUDE`], then the plugin's own source.
pub(crate) fn wrapped_source(source: &str) -> String {
    format!("{PRELUDE}\n{source}")
}

/// Budgets and bounds of pack minigame sessions. Fuel is the same deterministic unit as the
/// hook budgets ([`crate::PluginHostOptions`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinigameLimits {
    /// Fuel to start a session: compiling the plugin, running its top level, `start` and the
    /// first `draw`. The default, 200 million, is about 0.15–0.4 s in a release build.
    pub start_fuel: u64,
    /// Fuel per frame, `update` and `draw` together. The default, 10 million, is about 8–20 ms
    /// in a release build; a simple game with a small state uses a few hundred thousand.
    pub fuel_per_frame: u64,
    /// Longest state, as JSON text, in bytes. The default is 16 KiB: passing a state into the
    /// sandbox and back costs about 260 fuel per byte, so a full state takes about 4 million.
    pub max_state_bytes: usize,
    /// Bounds on each draw list.
    pub canvas: CanvasLimits,
}

impl Default for MinigameLimits {
    fn default() -> Self {
        Self {
            start_fuel: 200_000_000,
            fuel_per_frame: 10_000_000,
            max_state_bytes: 16 * 1024,
            canvas: CanvasLimits::default(),
        }
    }
}

/// The player's input in one frame, from the game's input bindings (keyboard, gamepad and the
/// touch controls, after rebinding): what an `update` gets as `input`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinigameControls {
    /// Up held (W, arrow up, the left stick and D-pad).
    pub up: bool,
    /// Down held.
    pub down: bool,
    /// Left held.
    pub left: bool,
    /// Right held.
    pub right: bool,
    /// Interact held (E, Space, Enter, gamepad A).
    pub action: bool,
    /// Interact went down this frame.
    pub action_pressed: bool,
    /// Interact went up this frame.
    pub action_released: bool,
}

/// Where a session is after a frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MinigameFrame {
    /// Still playing.
    Running,
    /// `update` answered `done`: the score in [0, 1] that resolves the minigame.
    Finished {
        /// The score.
        score: f64,
    },
}

/// One running session of a pack-defined minigame. `Send`, so a session runs wherever its game
/// does (the editor steps frames on a worker thread).
pub struct PackMinigame {
    plugin_id: String,
    kind: String,
    hook: String,
    instance: Option<Box<dyn GuestInstance>>,
    state: Value,
    canvas: MinigameCanvas,
    limits: MinigameLimits,
    finished: Option<f64>,
    failure: Option<PluginError>,
    last_fuel_used: u64,
}

impl fmt::Debug for PackMinigame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PackMinigame")
            .field("plugin_id", &self.plugin_id)
            .field("kind", &self.kind)
            .field("finished", &self.finished)
            .field("failure", &self.failure)
            .finish_non_exhaustive()
    }
}

/// Which budget a call runs under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Start,
    Frame,
}

impl Phase {
    fn while_doing(self) -> &'static str {
        match self {
            Phase::Start => "while starting",
            Phase::Frame => "in a frame",
        }
    }
}

impl PackMinigame {
    /// Start a session of `kind` (`packId:name`, one of `spec.granted_minigames`) in a fresh
    /// instance from `sandbox`: compile the plugin, then `start(config, info)` and `draw`, all
    /// within [`MinigameLimits::start_fuel`]. `info` is `{ width, height, seed }`.
    pub(crate) fn start(
        sandbox: &Sandbox,
        spec: &PluginSpec,
        kind: &str,
        config: &Value,
        seed: u32,
        limits: MinigameLimits,
    ) -> Result<Self, PluginError> {
        let name = kind.rsplit_once(':').map_or(kind, |(_, name)| name);
        let hook = format!("{MINIGAME_HOOK_PREFIX}{name}");
        let mut session = Self {
            plugin_id: spec.id.clone(),
            kind: kind.to_owned(),
            hook: hook.clone(),
            instance: None,
            state: Value::Null,
            canvas: MinigameCanvas { pack_id: spec.pack_id.clone(), commands: Vec::new() },
            limits,
            finished: None,
            failure: None,
            last_fuel_used: 0,
        };
        let fuel = session.limits.start_fuel;
        let mut instance =
            sandbox.instance().map_err(|fault| session.fail_with(PluginErrorKind::Init, &describe(&fault)))?;
        instance.set_fuel(fuel);
        let granted = serde_json::to_string(&[&hook]).expect("strings serialize");
        let reply = sandbox::init(instance.as_mut(), &granted, &wrapped_source(&spec.source));
        session.last_fuel_used = instance.fuel_used(fuel);
        let reply = match reply {
            Err(fault) => return Err(session.fault(Phase::Start, &fault)),
            Ok(reply) => reply,
        };
        match reply.status {
            status::OK => {}
            status::OUT_OF_MEMORY => {
                return Err(session.fail_with(PluginErrorKind::Budget, "exceeded its memory budget while starting"))
            }
            _ => {
                let message = format!("the plugin failed to start: {}", reply.output);
                return Err(session.fail_with(PluginErrorKind::Init, &message));
            }
        }
        session.instance = Some(instance);
        let info = json!({ "width": CANVAS_WIDTH, "height": CANVAS_HEIGHT, "seed": seed });
        let payload = json!({ "op": "start", "config": config, "info": info });
        let (state, draw) = session.call(&payload, Phase::Start)?;
        session.set_state(state, "start")?;
        session.set_canvas(&draw)?;
        Ok(session)
    }

    /// The full kind (`packId:name`).
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// The plugin that defines the kind (`packId:pluginId`).
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    /// The current state, as the plugin answered it.
    pub fn state(&self) -> &Value {
        &self.state
    }

    /// The latest frame.
    pub fn canvas(&self) -> &MinigameCanvas {
        &self.canvas
    }

    /// The score once `update` answered `done`.
    pub fn score(&self) -> Option<f64> {
        self.finished
    }

    /// Why the session failed, once it has.
    pub fn failure(&self) -> Option<&PluginError> {
        self.failure.as_ref()
    }

    /// Fuel the last start or frame used (a creator tool can show it against the limits).
    pub fn last_fuel_used(&self) -> u64 {
        self.last_fuel_used
    }

    /// One frame: `update(state, input, dt)`, then `draw` of the new state, within
    /// [`MinigameLimits::fuel_per_frame`]. `dt` is the frame time in seconds (non-finite or
    /// negative counts as 0, longer than [`MAX_FRAME_SECONDS`] as that). After the session
    /// finished, frames change nothing; after it failed, they return the same error.
    pub fn frame(&mut self, input: &MinigameControls, dt: f64) -> Result<MinigameFrame, PluginError> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        if let Some(score) = self.finished {
            return Ok(MinigameFrame::Finished { score });
        }
        let dt = if dt.is_finite() { dt.clamp(0.0, MAX_FRAME_SECONDS) } else { 0.0 };
        let payload = json!({ "op": "frame", "state": self.state, "input": input, "dt": dt });
        let (answer, draw) = self.call(&payload, Phase::Frame)?;
        let done = self.apply_update(answer)?;
        self.set_canvas(&draw)?;
        self.finished = done;
        Ok(done.map_or(MinigameFrame::Running, |score| MinigameFrame::Finished { score }))
    }

    /// `{ state?, done? }`: the new state (kept when absent) and the score when done.
    fn apply_update(&mut self, answer: Value) -> Result<Option<f64>, PluginError> {
        let Value::Object(mut answer) = answer else {
            return Err(self.fail("update must return an object { state, done }"));
        };
        if let Some(key) = answer.keys().find(|key| !matches!(key.as_str(), "state" | "done")) {
            let key = crate::clip(key, 40);
            return Err(self.fail(&format!("update returned unknown key \"{key}\" (only state and done)")));
        }
        let done = match answer.remove("done") {
            None | Some(Value::Null) => None,
            Some(Value::Object(done)) => Some(self.score_of(&done)?),
            Some(_) => return Err(self.fail("update's done must be an object { score } or { success }")),
        };
        if let Some(state) = answer.remove("state") {
            self.set_state(state, "update")?;
        }
        Ok(done)
    }

    fn score_of(&mut self, done: &Map<String, Value>) -> Result<f64, PluginError> {
        match (done.get("score"), done.get("success")) {
            (Some(Value::Number(score)), _) => match score.as_f64() {
                Some(score) if (0.0..=1.0).contains(&score) => Ok(score),
                _ => Err(self.fail("done.score must be a number from 0 to 1")),
            },
            (None | Some(Value::Null), Some(Value::Bool(success))) => Ok(if *success { 1.0 } else { 0.0 }),
            _ => Err(self.fail("done must have a score from 0 to 1 or success true/false")),
        }
    }

    fn set_state(&mut self, state: Value, op: &str) -> Result<(), PluginError> {
        let len = serde_json::to_string(&state).map_or(usize::MAX, |text| text.len());
        if len > self.limits.max_state_bytes {
            let max = self.limits.max_state_bytes;
            return Err(self.fail(&format!("{op} returned a state of {len} bytes; at most {max} are allowed")));
        }
        self.state = state;
        Ok(())
    }

    fn set_canvas(&mut self, draw: &Value) -> Result<(), PluginError> {
        match parse_canvas(draw, &self.limits.canvas) {
            Ok(commands) => {
                self.canvas.commands = commands;
                Ok(())
            }
            Err(message) => Err(self.fail(&message)),
        }
    }

    /// One guest call with a fresh budget for `phase` (a start continues the fuel its compile
    /// left): the answer of `start` or `update`, and the draw list.
    fn call(&mut self, payload: &Value, phase: Phase) -> Result<(Value, Value), PluginError> {
        let Some(instance) = self.instance.as_mut() else { return Err(self.fail("the session has ended")) };
        let fuel = match phase {
            Phase::Start => self.limits.start_fuel,
            Phase::Frame => {
                instance.set_fuel(self.limits.fuel_per_frame);
                self.limits.fuel_per_frame
            }
        };
        let payload = serde_json::to_string(payload).expect("JSON values serialize");
        let reply = sandbox::dispatch(instance.as_mut(), &self.hook, &payload);
        self.last_fuel_used = instance.fuel_used(fuel);
        let reply = match reply {
            Err(fault) => return Err(self.fault(phase, &fault)),
            Ok(reply) => reply,
        };
        let op = if phase == Phase::Start { "start" } else { "update" };
        match reply.status {
            status::OK if reply.oversized => {
                Err(self.fail(&format!("{op} and draw returned more than {MAX_RESULT_BYTES} bytes")))
            }
            status::OK => match serde_json::from_str::<Value>(&reply.output) {
                Ok(Value::Array(mut answers)) if answers.len() == 2 => {
                    let draw = answers.pop().unwrap_or(Value::Null);
                    Ok((answers.pop().unwrap_or(Value::Null), draw))
                }
                _ => Err(self.fail(&format!("{op} or draw returned something that is not JSON"))),
            },
            status::NO_HANDLER => {
                let name = self.hook.trim_start_matches(MINIGAME_HOOK_PREFIX).to_owned();
                let message =
                    format!("the plugin does not define it (api.minigame('{name}', {{ start, update, draw }}))");
                Err(self.fail_with(PluginErrorKind::Init, &message))
            }
            status::THREW => {
                // The prelude names the function an error came from (`update: kaput`).
                let message = format!("threw in {}", reply.output);
                Err(self.fail_with(PluginErrorKind::Threw, &message))
            }
            status::NOT_SERIALIZABLE => Err(self.fail(&format!("{op} or draw returned something that is not JSON"))),
            status::OUT_OF_MEMORY => {
                let message = format!("exceeded its memory budget {}", phase.while_doing());
                Err(self.fail_with(PluginErrorKind::Budget, &message))
            }
            other => {
                let message = format!("failed {} (guest status {other}: {})", phase.while_doing(), reply.output);
                Err(self.fail_with(PluginErrorKind::Budget, &message))
            }
        }
    }

    fn fault(&mut self, phase: Phase, fault: &GuestFault) -> PluginError {
        let message = match (fault, phase) {
            (GuestFault::OutOfFuel, Phase::Start) => {
                format!("exceeded its fuel budget for starting ({} fuel)", self.limits.start_fuel)
            }
            (GuestFault::OutOfFuel, Phase::Frame) => {
                format!("exceeded its fuel budget for a frame ({} fuel)", self.limits.fuel_per_frame)
            }
            (other, phase) => format!("{} {}", describe(other), phase.while_doing()),
        };
        let kind = if *fault == GuestFault::OutOfFuel { PluginErrorKind::Timeout } else { PluginErrorKind::Budget };
        self.fail_with(kind, &message)
    }

    fn fail(&mut self, message: &str) -> PluginError {
        self.fail_with(PluginErrorKind::Threw, message)
    }

    /// End the session: drop the instance (it may be mid-way through anything) and keep the
    /// error, which every later frame returns.
    fn fail_with(&mut self, kind: PluginErrorKind, message: &str) -> PluginError {
        self.instance = None;
        let error = PluginError {
            plugin_id: self.plugin_id.clone(),
            kind,
            message: format!("minigame {} failed: {}", self.kind, crate::clip(message, crate::MAX_ERROR_MESSAGE_BYTES)),
            hook: Some(self.hook.clone()),
        };
        self.failure = Some(error.clone());
        error
    }
}

fn describe(fault: &GuestFault) -> String {
    match fault {
        GuestFault::OutOfFuel => "exceeded its fuel budget".to_owned(),
        GuestFault::StackOverflow => "exceeded its recursion budget".to_owned(),
        GuestFault::Aborted => "failed (the sandbox aborted)".to_owned(),
        GuestFault::Other(message) => format!("failed ({message})"),
    }
}
