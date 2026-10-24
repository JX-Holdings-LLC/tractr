//! Cutscenes (M2): the runner that plays a cutscene's steps over time.
//!
//! The `startCutscene` outcome sets `state.cutscene` at step 0; from the next tick on
//! [`advance`] plays it, whatever the clock does. While it plays it is a modal like dialogue:
//! under [`crate::CommandRules::Player`] only its own commands apply (`advanceCutscene`,
//! `chooseCutsceneOption`, `skipCutscene`, plus `setMoveIntent` and plugin mutations), the
//! player's free movement is frozen, the NPCs it directs keep off their schedules and patterns,
//! and with `time.pauseInModals` the clock stops.
//!
//! One step runs at a time; a step that is done hands over to the next in the same tick:
//!
//! - `say` shows a line and waits for `advanceCutscene`.
//! - `choice` shows a question and waits for `chooseCutsceneOption`, then goes to the option's
//!   `goto` label (empty or unknown: the next step). Without options it is a `say`.
//! - `jump` goes to `goto` when every condition holds (the events' conditions; unknown label:
//!   the next step), else to the next step.
//! - `move` walks the actor (`player`, or an NPC in the player's scene) to tile (x, y) along an
//!   A* path at the player's speed (`movement.playerSpeed`). Actors pass through each other and
//!   the player enters tiles without their doors, pickups or events. It waits for the arrival
//!   unless `wait` is false. A free tile no path reaches is reached at once; a move to a tile
//!   that blocks walking or lies outside the scene is skipped.
//! - `face` turns the actor; `emote` shows an emote over it for `seconds` (1.5 by default) and
//!   waits as long; `wait` waits `seconds` (1 by default).
//! - `camera` pans to tile (x, y), or onto `actor` and then follows it, over `seconds` (1 by
//!   default) and waits for the pan; with neither it pans back to the player.
//! - `fade` fades the screen `out` to `color` (black by default) or back `in` over `seconds` (1
//!   by default) and waits for it. The screen clears when the cutscene ends.
//! - `music` sets `state.music` (empty: the place's music again); `sound` plays a sound.
//! - `outcomes` runs its outcomes in order (the events' runner). One that opens a dialogue, a
//!   shop or a minigame holds the cutscene until it closes (the cutscene's commands are refused
//!   meanwhile); `startCutscene` does nothing while one plays.
//! - `end` ends it, as does running past the last step.
//!
//! Content mistakes never stop a game: an unknown step type, and a step whose actor is missing
//! or not in the player's scene, are skipped; a cutscene the content lacks ends; steps that loop
//! without ever waiting end it after [`MAX_STEPS_PER_TICK`] with an error message.
//!
//! Skipping (`skipCutscene`, when `skippable`) runs the rest at once: `outcomes`, `music` and
//! `face` steps apply, jumps are followed, moves put their actors on their tiles, and the other
//! steps do nothing. It stops at a `choice` with options, which the player still answers.
//!
//! Hosts draw from [`view`]: the line or question, the camera, the fade and the emotes.

use crate::effects::{message_levels, Effect};
use crate::engine_types::{Effects, EngineContext};
use crate::events;
use crate::messages;
use crate::schema::{
    cutscene_step_types as steps, directions, emotes, Cutscene, CutsceneCamera, CutsceneEmote, CutsceneFade,
    CutsceneStep, CutsceneWalk, GameContent, GameState, GridPoint,
};
use crate::substitution;
use crate::units;
use crate::world::pathfinding::{self, PathPoint, Walkability};
use crate::world::world_movement;
use serde::Serialize;

/// The actor id of the player (any other actor is an NPC id).
pub const PLAYER: &str = "player";

/// The most steps one tick or one command runs. Only steps that finish at once (jumps, faces,
/// music, outcomes…) chain, so a cutscene that gets here loops without waiting: it ends.
pub const MAX_STEPS_PER_TICK: u32 = 1000;

/// The most steps a skip runs (jumps may loop).
pub const MAX_SKIP_STEPS: u32 = 10_000;

/// `seconds` of a `wait`, `camera` or `fade` step without one, in thousandths.
pub const DEFAULT_MILLIS: u32 = 1000;

/// `seconds` of an `emote` step without one, in thousandths.
pub const DEFAULT_EMOTE_MILLIS: u32 = 1500;

/// A fade's colour when the step names none.
pub const DEFAULT_FADE_COLOR: &str = "#000000";

/// Opacity of a faded-out screen, in thousandths.
const OPAQUE: u32 = units::MILLI_ONE;

/// What a step does next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    /// Still running.
    Wait,
    Next,
    Goto(usize),
    End,
}

/// The cutscene `id` of the content.
fn find<'a>(content: &'a GameContent, id: &str) -> Option<&'a Cutscene> {
    content.cutscenes.iter().find(|cutscene| cutscene.id == id)
}

/// TS `x || y` on a string: an empty (or missing) string is falsy.
fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|text| !text.is_empty())
}

/// A dialogue, shop or minigame (opened by an `outcomes` step) is open over the cutscene, which
/// waits for it to close.
fn held(state: &GameState) -> bool {
    state.dialogue.is_some() || state.shop.is_some() || state.minigame.is_some()
}

/// Thousandths of a second as ticks (rounded).
fn ticks_of(millis: u32) -> u64 {
    let ticks = units::div_round(i64::from(millis) * i64::from(units::TICKS_PER_SECOND), i64::from(units::MILLI_ONE));
    u64::try_from(ticks).unwrap_or(0)
}

/// How long a timed step lasts, in ticks.
fn duration(step: &CutsceneStep) -> u64 {
    let default = if step.r#type == steps::EMOTE { DEFAULT_EMOTE_MILLIS } else { DEFAULT_MILLIS };
    ticks_of(step.seconds.unwrap_or(default))
}

/// `from` moved toward `to` by `elapsed` of `duration` ticks (rounded).
fn lerp(from: i32, to: i32, elapsed: u64, duration: u64) -> i32 {
    if elapsed >= duration {
        return to;
    }
    let (elapsed, duration) = (i64::try_from(elapsed).unwrap_or(0), i64::try_from(duration).unwrap_or(1));
    let moved = units::div_round((i64::from(to) - i64::from(from)).saturating_mul(elapsed), duration);
    i32::try_from(i64::from(from) + moved).unwrap_or(to)
}

/// `elapsed` of `duration` ticks in thousandths.
fn progress(elapsed: u64, duration: u64) -> u32 {
    if elapsed >= duration {
        return units::MILLI_ONE;
    }
    u32::try_from(elapsed.saturating_mul(u64::from(units::MILLI_ONE)) / duration).unwrap_or(units::MILLI_ONE)
}

/// The NPC `id` when it stands in the player's scene (where cutscenes direct actors).
fn npc_here<'a>(state: &'a GameState, id: &str) -> Option<&'a crate::schema::NpcState> {
    state.npcs.get(id).filter(|npc| npc.scene_id == state.player.scene_id)
}

/// The centre of an actor in position units: the player's box, an NPC's tile. `None` for an
/// NPC that is missing or not in the player's scene.
pub fn actor_point(state: &GameState, actor: &str) -> Option<(i32, i32)> {
    if actor == PLAYER {
        return Some((state.player.x, state.player.y));
    }
    let half = units::TILE / 2;
    npc_here(state, actor).map(|npc| (npc.x.saturating_add(half), npc.y.saturating_add(half)))
}

/// The tile an actor stands on.
fn actor_tile(state: &GameState, actor: &str) -> Option<PathPoint> {
    if actor == PLAYER {
        let tile = world_movement::player_tile(state);
        return Some(PathPoint { x: tile.x, y: tile.y });
    }
    npc_here(state, actor).map(|npc| PathPoint { x: units::tile_of(npc.x), y: units::tile_of(npc.y) })
}

/// Where an actor stands on `tile`: the player's box centred on it, an NPC on its corner (NPC
/// positions are whole tiles).
fn stand_point(actor: &str, tile: &GridPoint) -> (i32, i32) {
    if actor == PLAYER {
        (units::tile_center(tile.x), units::tile_center(tile.y))
    } else {
        (units::tiles(tile.x), units::tiles(tile.y))
    }
}

/// The position of an actor that is still in `scene_id`.
fn position_in(state: &GameState, actor: &str, scene_id: &str) -> Option<(i32, i32)> {
    if actor == PLAYER {
        return (state.player.scene_id == scene_id).then_some((state.player.x, state.player.y));
    }
    state.npcs.get(actor).filter(|npc| npc.scene_id == scene_id).map(|npc| (npc.x, npc.y))
}

fn set_position(state: &mut GameState, actor: &str, (x, y): (i32, i32)) {
    if actor == PLAYER {
        state.player.x = x;
        state.player.y = y;
    } else if let Some(npc) = state.npcs.get_mut(actor) {
        npc.x = x;
        npc.y = y;
    }
}

/// Puts an actor still in `scene_id` on `tile`.
fn place(state: &mut GameState, actor: &str, scene_id: &str, tile: &GridPoint) {
    if position_in(state, actor, scene_id).is_some() {
        set_position(state, actor, stand_point(actor, tile));
    }
}

/// Turns an actor: the player's own direction, an NPC's in the cutscene.
fn set_facing(state: &mut GameState, actor: &str, direction: &str) {
    if actor == PLAYER {
        state.player.direction = direction.to_owned();
    } else if let Some(running) = state.cutscene.as_mut() {
        running.facing.insert(actor.to_owned(), direction.to_owned());
    }
}

/// The step at `index` of the running cutscene, not started.
fn go_to(state: &mut GameState, index: usize) {
    if let Some(running) = state.cutscene.as_mut() {
        running.step = u32::try_from(index).unwrap_or(u32::MAX);
        running.started = false;
        running.elapsed = 0;
    }
}

/// The step labelled `label` (`Next` when empty or unknown).
fn goto(cutscene: &Cutscene, label: Option<&str>) -> Flow {
    let Some(label) = non_empty(label) else { return Flow::Next };
    match cutscene.steps.iter().position(|step| step.label.as_deref() == Some(label)) {
        Some(index) => Flow::Goto(index),
        None => Flow::Next,
    }
}

/// Ends the running cutscene: walking actors arrive at once, the camera and the screen clear.
/// Nothing happens without one.
pub fn stop(state: &mut GameState) {
    let Some(running) = state.cutscene.take() else { return };
    for walk in &running.walks {
        if let Some(target) = walk.path.last() {
            place(state, &walk.actor, &walk.scene_id, target);
        }
    }
}

/// The NPCs the running cutscene directs (named as a step's actor): their schedules and movement
/// patterns wait for it to end. Empty without a cutscene.
pub fn directed_npcs(ctx: &EngineContext, state: &GameState) -> Vec<String> {
    let Some(running) = state.cutscene.as_ref() else { return Vec::new() };
    let mut ids: Vec<String> = Vec::new();
    let named = find(&ctx.content, &running.cutscene_id)
        .into_iter()
        .flat_map(|cutscene| cutscene.steps.iter().filter_map(|step| step.actor.as_deref()));
    for id in named.chain(running.walks.iter().map(|walk| walk.actor.as_str())) {
        if !id.is_empty() && id != PLAYER && !ids.iter().any(|known| known == id) {
            ids.push(id.to_owned());
        }
    }
    ids
}

/// Plays the running cutscene for one tick (from the engine's tick, before the clock). Without
/// a cutscene nothing happens.
pub fn advance(ctx: &EngineContext, state: &mut GameState) -> Effects {
    let Some(running) = state.cutscene.as_ref() else { return Vec::new() };
    if held(state) {
        return Vec::new();
    }
    let Some(cutscene) = find(&ctx.content, &running.cutscene_id) else {
        stop(state);
        return Vec::new();
    };
    pass_tick(ctx, state);
    run_steps(ctx, state, cutscene)
}

/// One tick of the running timers: walks, the step's clock, the camera, the fade and emotes.
fn pass_tick(ctx: &EngineContext, state: &mut GameState) {
    walk(ctx, state);
    let Some(running) = state.cutscene.as_mut() else { return };
    if running.started {
        running.elapsed = running.elapsed.saturating_add(1);
    }
    if let Some(camera) = running.camera.as_mut() {
        camera.elapsed = camera.elapsed.saturating_add(1).min(camera.duration);
    }
    if let Some(fade) = running.fade.as_mut() {
        fade.elapsed = fade.elapsed.saturating_add(1).min(fade.duration);
    }
    for emote in &mut running.emotes {
        emote.elapsed = emote.elapsed.saturating_add(1);
    }
    running.emotes.retain(|emote| emote.elapsed < emote.duration);
    settle(running);
}

/// A pan back onto the player that is over hands the camera back; a fade in that is over
/// clears the screen.
fn settle(running: &mut crate::schema::CutsceneState) {
    if running
        .camera
        .as_ref()
        .is_some_and(|camera| camera.elapsed >= camera.duration && camera.actor.as_deref() == Some(PLAYER))
    {
        running.camera = None;
    }
    if running.fade.as_ref().is_some_and(|fade| fade.elapsed >= fade.duration && fade.to == 0) {
        running.fade = None;
    }
}

/// Moves every walking actor up to the walking speed along its path; a walk ends on its last
/// tile, or when its actor left the scene.
fn walk(ctx: &EngineContext, state: &mut GameState) {
    let Some(running) = state.cutscene.as_mut() else { return };
    if running.walks.is_empty() {
        return;
    }
    let mut walks = std::mem::take(&mut running.walks);
    let speed = i64::from(ctx.content.settings.movement.player_speed.max(1));
    let mut turns = Vec::new();
    walks.retain_mut(|walk| {
        let Some((x, y)) = position_in(state, &walk.actor, &walk.scene_id) else { return false };
        let (mut x, mut y) = (i64::from(x), i64::from(y));
        let mut budget = speed;
        let mut facing = None;
        while budget > 0 {
            let Some(next) = walk.path.first() else { break };
            let (tx, ty) = stand_point(&walk.actor, next);
            let (dx, dy) = (i64::from(tx) - x, i64::from(ty) - y);
            // Along x first, then y: the path's tiles are 4-connected.
            if dx != 0 {
                let step = dx.abs().min(budget);
                x += step * dx.signum();
                budget -= step;
                facing = Some(if dx < 0 { directions::LEFT } else { directions::RIGHT });
            } else if dy != 0 {
                let step = dy.abs().min(budget);
                y += step * dy.signum();
                budget -= step;
                facing = Some(if dy < 0 { directions::UP } else { directions::DOWN });
            } else {
                walk.path.remove(0);
            }
        }
        if walk.path.first().is_some_and(|next| {
            let (tx, ty) = stand_point(&walk.actor, next);
            i64::from(tx) == x && i64::from(ty) == y
        }) {
            walk.path.remove(0);
        }
        let position = (i32::try_from(x).unwrap_or(i32::MAX), i32::try_from(y).unwrap_or(i32::MAX));
        set_position(state, &walk.actor, position);
        if let Some(direction) = facing {
            turns.push((walk.actor.clone(), direction));
        }
        !walk.path.is_empty()
    });
    if let Some(running) = state.cutscene.as_mut() {
        running.walks = walks;
    }
    for (actor, direction) in turns {
        set_facing(state, &actor, direction);
    }
}

/// Runs steps from the current one until one waits (or the cutscene ends).
fn run_steps(ctx: &EngineContext, state: &mut GameState, cutscene: &Cutscene) -> Effects {
    let mut effects = Vec::new();
    for _ in 0..MAX_STEPS_PER_TICK {
        if held(state) {
            return effects;
        }
        let Some(running) = state.cutscene.as_ref() else { return effects };
        let index = running.step as usize;
        let Some(step) = cutscene.steps.get(index) else {
            stop(state);
            return effects;
        };
        if !running.started {
            let (applies, started) = start(ctx, state, step);
            effects.extend(started);
            if !applies {
                go_to(state, index + 1);
                continue;
            }
            if let Some(running) = state.cutscene.as_mut() {
                running.started = true;
                running.elapsed = 0;
            }
        }
        match flow(ctx, state, cutscene, step) {
            Flow::Wait => return effects,
            Flow::Next => go_to(state, index + 1),
            Flow::Goto(target) => go_to(state, target),
            Flow::End => {
                stop(state);
                return effects;
            }
        }
    }
    // Steps that loop without waiting would hold the player forever.
    effects.push(Effect::say(message_levels::ERROR, messages::CUTSCENE_LOOP.with(&[&cutscene.id])));
    stop(state);
    effects
}

/// Begins a step: its instant part. False when the step does nothing here (its actor is missing,
/// a field it needs is absent), so the next step follows.
fn start(ctx: &EngineContext, state: &mut GameState, step: &CutsceneStep) -> (bool, Effects) {
    match step.r#type.as_str() {
        steps::MOVE => (start_move(ctx, state, step), Vec::new()),
        steps::FACE => (face(state, step), Vec::new()),
        steps::EMOTE => {
            let Some(actor) = non_empty(step.actor.as_deref()).filter(|actor| actor_point(state, actor).is_some())
            else {
                return (false, Vec::new());
            };
            let Some(emote) = step.emote.as_deref().filter(|emote| emotes::ALL.contains(emote)) else {
                return (false, Vec::new());
            };
            let emote = CutsceneEmote {
                actor: actor.to_owned(),
                emote: emote.to_owned(),
                duration: duration(step),
                elapsed: 0,
            };
            if let Some(running) = state.cutscene.as_mut() {
                running.emotes.retain(|shown| shown.actor != emote.actor);
                if emote.duration > 0 {
                    running.emotes.push(emote);
                }
            }
            (true, Vec::new())
        }
        steps::CAMERA => (start_camera(state, step), Vec::new()),
        steps::FADE => (start_fade(state, step), Vec::new()),
        steps::MUSIC => {
            state.music = non_empty(step.sound_id.as_deref()).map(str::to_owned);
            (true, Vec::new())
        }
        steps::SOUND => match non_empty(step.sound_id.as_deref()) {
            Some(id) => (true, vec![Effect::Sound { id: id.to_owned() }]),
            None => (false, Vec::new()),
        },
        steps::OUTCOMES => (true, events::apply_outcomes(ctx, state, &step.outcomes, 0)),
        _ => (true, Vec::new()),
    }
}

/// Whether a started step is done, and where the cutscene goes from it.
fn flow(ctx: &EngineContext, state: &GameState, cutscene: &Cutscene, step: &CutsceneStep) -> Flow {
    let Some(running) = state.cutscene.as_ref() else { return Flow::End };
    let timed = || if running.elapsed >= duration(step) { Flow::Next } else { Flow::Wait };
    match step.r#type.as_str() {
        steps::SAY | steps::CHOICE => Flow::Wait,
        steps::JUMP => {
            if step.conditions.iter().all(|condition| events::condition_met(ctx, state, condition, None)) {
                goto(cutscene, step.goto.as_deref())
            } else {
                Flow::Next
            }
        }
        steps::MOVE => {
            let walking = running.walks.iter().any(|walk| Some(walk.actor.as_str()) == step.actor.as_deref());
            if walking && step.wait != Some(false) {
                Flow::Wait
            } else {
                Flow::Next
            }
        }
        steps::EMOTE | steps::WAIT | steps::CAMERA | steps::FADE => timed(),
        steps::END => Flow::End,
        _ => Flow::Next,
    }
}

/// The actor and tile of a `move` step, when the tile lies in the player's scene and does not
/// block walking (no actor is put in a wall).
fn move_target(ctx: &EngineContext, state: &GameState, step: &CutsceneStep) -> Option<(String, PathPoint)> {
    let actor = non_empty(step.actor.as_deref())?;
    let (x, y) = (step.x?, step.y?);
    let scene = world_movement::find_scene(state, &state.player.scene_id)?;
    let tile = scene.tile(x, y)?;
    if pathfinding::tile_blocks(tile, &ctx.content.node_types, &ctx.content.machine_types) {
        return None;
    }
    Some((actor.to_owned(), PathPoint { x, y }))
}

/// Sets an actor walking to the step's tile (or puts it there when no path reaches it).
fn start_move(ctx: &EngineContext, state: &mut GameState, step: &CutsceneStep) -> bool {
    let Some((actor, goal)) = move_target(ctx, state, step) else { return false };
    let Some(from) = actor_tile(state, &actor) else { return false };
    let scene_id = state.player.scene_id.clone();
    let path = world_movement::find_scene(state, &scene_id).and_then(|scene| {
        // Actors pass through each other: cutscenes are choreographed.
        let walkability = Walkability {
            scene,
            node_types: &ctx.content.node_types,
            machine_types: &ctx.content.machine_types,
            blocked: &[],
            index: Some(ctx.index()),
        };
        pathfinding::find_path(&walkability, from, goal)
    });
    let target = GridPoint { x: goal.x, y: goal.y };
    let Some(running) = state.cutscene.as_mut() else { return false };
    running.walks.retain(|walk| walk.actor != actor);
    match path {
        Some(path) => {
            let mut path: Vec<GridPoint> = path.into_iter().map(|point| GridPoint { x: point.x, y: point.y }).collect();
            if path.is_empty() {
                // On the tile already: step onto its centre.
                path.push(target);
            }
            running.walks.push(CutsceneWalk { actor, scene_id, path });
        }
        None => place(state, &actor, &scene_id, &target),
    }
    true
}

/// A `face` step: turns its actor (false when the actor or the direction is unknown).
fn face(state: &mut GameState, step: &CutsceneStep) -> bool {
    let Some(actor) = non_empty(step.actor.as_deref()).filter(|actor| actor_point(state, actor).is_some()) else {
        return false;
    };
    let Some(direction) = step.direction.as_deref().filter(|direction| directions::ALL.contains(direction)) else {
        return false;
    };
    set_facing(state, actor, direction);
    true
}

/// Where the camera centres now: the cutscene's pan, else the player.
fn camera_point(state: &GameState, camera: Option<&CutsceneCamera>) -> (i32, i32) {
    let Some(camera) = camera else { return (state.player.x, state.player.y) };
    let (to_x, to_y) =
        camera.actor.as_deref().and_then(|actor| actor_point(state, actor)).unwrap_or((camera.to_x, camera.to_y));
    (
        lerp(camera.from_x, to_x, camera.elapsed, camera.duration),
        lerp(camera.from_y, to_y, camera.elapsed, camera.duration),
    )
}

/// Where the running cutscene's camera centres now (position units); `None` while the camera
/// follows the player as usual (no cutscene, or no camera step). Hosts that draw between ticks
/// interpolate it from the tick before, as they do the player.
pub fn camera(state: &GameState) -> Option<(i32, i32)> {
    let camera = state.cutscene.as_ref()?.camera.as_ref()?;
    Some(camera_point(state, Some(camera)))
}

/// A `camera` step: a pan from where the camera is to the tile, onto the actor, or back to the
/// player.
fn start_camera(state: &mut GameState, step: &CutsceneStep) -> bool {
    let current = state.cutscene.as_ref().and_then(|running| running.camera.as_ref());
    let (from_x, from_y) = camera_point(state, current);
    let (actor, (to_x, to_y)) = match (non_empty(step.actor.as_deref()), step.x, step.y) {
        (Some(actor), _, _) => {
            let Some(point) = actor_point(state, actor) else { return false };
            (Some(actor.to_owned()), point)
        }
        (None, Some(x), Some(y)) => (None, (units::tile_center(x), units::tile_center(y))),
        _ => (Some(PLAYER.to_owned()), (state.player.x, state.player.y)),
    };
    let camera = CutsceneCamera { from_x, from_y, to_x, to_y, actor, duration: duration(step), elapsed: 0 };
    if let Some(running) = state.cutscene.as_mut() {
        running.camera = Some(camera);
        settle(running);
    }
    true
}

/// The fade's opacity now, in thousandths.
fn fade_alpha(fade: &CutsceneFade) -> u32 {
    let alpha =
        lerp(i32::try_from(fade.from).unwrap_or(0), i32::try_from(fade.to).unwrap_or(0), fade.elapsed, fade.duration);
    u32::try_from(alpha).unwrap_or(0).min(OPAQUE)
}

/// A `fade` step: from the screen's opacity now to opaque (`out`) or clear (`in`).
fn start_fade(state: &mut GameState, step: &CutsceneStep) -> bool {
    let Some(running) = state.cutscene.as_mut() else { return false };
    let current = running.fade.as_ref();
    let from = current.map_or(0, fade_alpha);
    let (to, color) = match step.mode.as_deref() {
        Some("out") => (OPAQUE, non_empty(step.color.as_deref()).unwrap_or(DEFAULT_FADE_COLOR)),
        Some("in") => (
            0,
            non_empty(step.color.as_deref()).or(current.map(|fade| fade.color.as_str())).unwrap_or(DEFAULT_FADE_COLOR),
        ),
        _ => return false,
    };
    running.fade = Some(CutsceneFade { color: color.to_owned(), from, to, duration: duration(step), elapsed: 0 });
    settle(running);
    true
}

// ── Commands ────────────────────────────────────────────────────────────────────────────────

/// The running cutscene and its current step, when it waits for the player (nothing open over
/// it).
fn waiting<'a>(ctx: &'a EngineContext, state: &GameState) -> Option<(&'a Cutscene, &'a CutsceneStep)> {
    let running = state.cutscene.as_ref()?;
    if held(state) {
        return None;
    }
    let cutscene = find(&ctx.content, &running.cutscene_id)?;
    Some((cutscene, cutscene.steps.get(running.step as usize)?))
}

/// A step `advanceCutscene` moves on from: a line, or a question without answers.
fn advances(step: &CutsceneStep) -> bool {
    step.r#type == steps::SAY || (step.r#type == steps::CHOICE && step.options.is_empty())
}

/// The answer `index` of a `choice` step.
fn option_at(step: &CutsceneStep, index: i32) -> Option<&crate::schema::CutsceneChoice> {
    if step.r#type != steps::CHOICE {
        return None;
    }
    usize::try_from(index).ok().and_then(|index| step.options.get(index))
}

/// Why `advanceCutscene` does not apply now (`None` when it does): no line is shown.
pub fn advance_refusal(ctx: &EngineContext, state: &GameState) -> Option<Effects> {
    match waiting(ctx, state) {
        Some((_, step)) if advances(step) => None,
        _ => Some(Vec::new()),
    }
}

/// Why `chooseCutsceneOption` does not apply now: no question is asked, or it has no answer
/// `index`.
pub fn choose_refusal(ctx: &EngineContext, state: &GameState, index: i32) -> Option<Effects> {
    match waiting(ctx, state) {
        Some((_, step)) if option_at(step, index).is_some() => None,
        _ => Some(Vec::new()),
    }
}

/// Why `skipCutscene` does not apply now: no cutscene waits for the player, or it may not be
/// skipped.
pub fn skip_refusal(ctx: &EngineContext, state: &GameState) -> Option<Effects> {
    match waiting(ctx, state) {
        Some((cutscene, _)) if cutscene.skippable => None,
        _ => Some(Vec::new()),
    }
}

/// `advanceCutscene`: past the line shown, on to the next step.
pub fn handle_advance(ctx: &EngineContext, state: &mut GameState) -> Effects {
    let Some((cutscene, step)) = waiting(ctx, state) else { return Vec::new() };
    if !advances(step) {
        return Vec::new();
    }
    let index = state.cutscene.as_ref().map_or(0, |running| running.step as usize);
    go_to(state, index + 1);
    run_steps(ctx, state, cutscene)
}

/// `chooseCutsceneOption`: answers the question with option `index` (an index past the
/// answers does nothing) and goes on from the option's label.
pub fn handle_choose(ctx: &EngineContext, state: &mut GameState, index: i32) -> Effects {
    let Some((cutscene, step)) = waiting(ctx, state) else { return Vec::new() };
    let Some(option) = option_at(step, index) else { return Vec::new() };
    let current = state.cutscene.as_ref().map_or(0, |running| running.step as usize);
    match goto(cutscene, Some(&option.goto)) {
        Flow::Goto(target) => go_to(state, target),
        _ => go_to(state, current + 1),
    }
    run_steps(ctx, state, cutscene)
}

/// `skipCutscene`: runs the rest at once (see the module docs), up to a `choice` with options.
pub fn handle_skip(ctx: &EngineContext, state: &mut GameState) -> Effects {
    let Some((cutscene, _)) = waiting(ctx, state) else { return Vec::new() };
    if !cutscene.skippable {
        return Vec::new();
    }
    // What runs over time is over: walkers arrive, emotes end, the camera and screen clear.
    if let Some(running) = state.cutscene.as_mut() {
        let walks = std::mem::take(&mut running.walks);
        running.emotes.clear();
        running.camera = None;
        running.fade = None;
        for walk in &walks {
            if let Some(target) = walk.path.last() {
                place(state, &walk.actor, &walk.scene_id, target);
            }
        }
    }
    let mut effects = Vec::new();
    for _ in 0..MAX_SKIP_STEPS {
        let Some(running) = state.cutscene.as_ref() else { return effects };
        let index = running.step as usize;
        let Some(step) = cutscene.steps.get(index) else {
            stop(state);
            return effects;
        };
        let flow = match step.r#type.as_str() {
            steps::CHOICE if !step.options.is_empty() => {
                // The player still answers.
                if let Some(running) = state.cutscene.as_mut() {
                    running.started = true;
                    running.elapsed = 0;
                }
                return effects;
            }
            steps::JUMP => flow(ctx, state, cutscene, step),
            steps::MOVE => {
                if let Some((actor, goal)) = move_target(ctx, state, step) {
                    let scene_id = state.player.scene_id.clone();
                    place(state, &actor, &scene_id, &GridPoint { x: goal.x, y: goal.y });
                }
                Flow::Next
            }
            steps::FACE => {
                face(state, step);
                Flow::Next
            }
            steps::MUSIC | steps::OUTCOMES => {
                effects.extend(start(ctx, state, step).1);
                Flow::Next
            }
            steps::END => Flow::End,
            _ => Flow::Next,
        };
        match flow {
            Flow::Goto(target) => go_to(state, target),
            Flow::End => {
                stop(state);
                return effects;
            }
            _ => go_to(state, index + 1),
        }
        // An outcome opened a dialogue, a shop or a minigame: the cutscene goes on after it.
        if held(state) {
            return effects;
        }
    }
    effects.push(Effect::say(message_levels::ERROR, messages::CUTSCENE_LOOP.with(&[&cutscene.id])));
    stop(state);
    effects
}

// ── View ────────────────────────────────────────────────────────────────────────────────────

/// What a host shows of the running cutscene ([`view`]). Positions are in position units
/// (1/8192 tile; tiles in JSON), opacities and progress in thousandths (0–1 in JSON).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CutsceneView {
    pub cutscene_id: String,
    /// Index of the running step (a new line or question has a new index, or a new cutscene).
    pub step: u32,
    /// `skipCutscene` applies now.
    pub skippable: bool,
    /// The line said or the question asked (absent while nothing is shown, or while a dialogue,
    /// shop or minigame is open over the cutscene).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<CutsceneLine>,
    /// Where the camera centres; absent: on the player as usual.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera: Option<CutsceneCameraView>,
    /// The fade over the screen; absent: clear.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fade: Option<CutsceneFadeView>,
    pub emotes: Vec<CutsceneEmoteView>,
    /// Actors the cutscene walks or turns (the player's direction is also in its state).
    pub actors: Vec<CutsceneActorView>,
}

/// A `say` line, or a `choice` question with its answers.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CutsceneLine {
    pub text: String,
    /// `player`, an NPC id, or absent for a narrator.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
    /// The speaking NPC's name (its id when the content lacks it); absent for the player and a
    /// narrator.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_name: Option<String>,
    /// The speaker's portrait expression (M4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,
    /// The answers, in order: `chooseCutsceneOption` indexes this list. Empty: a line that
    /// `advanceCutscene` moves on from.
    pub options: Vec<String>,
}

/// The camera of a cutscene: a pan, then held on a tile or following an actor.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CutsceneCameraView {
    /// The centre of the view now.
    #[serde(with = "crate::units::position")]
    pub x: i32,
    #[serde(with = "crate::units::position")]
    pub y: i32,
    /// Where the pan started and where it ends (the followed actor's centre now).
    #[serde(with = "crate::units::position")]
    pub from_x: i32,
    #[serde(with = "crate::units::position")]
    pub from_y: i32,
    #[serde(with = "crate::units::position")]
    pub to_x: i32,
    #[serde(with = "crate::units::position")]
    pub to_y: i32,
    /// How far the pan is (1: there).
    #[serde(with = "crate::units::milli")]
    pub progress: u32,
    /// The actor it follows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
}

/// A full-screen fade.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CutsceneFadeView {
    /// `#rrggbb` (whatever the step wrote; hosts fall back to black).
    pub color: String,
    /// Opacity now (1: opaque).
    #[serde(with = "crate::units::milli")]
    pub alpha: u32,
}

/// An emote over an actor.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CutsceneEmoteView {
    pub actor: String,
    /// One of `emotes::ALL`.
    pub emote: String,
    /// The actor's centre (draw the emote above it).
    #[serde(with = "crate::units::position")]
    pub x: i32,
    #[serde(with = "crate::units::position")]
    pub y: i32,
    /// How long it has shown (1: gone next tick).
    #[serde(with = "crate::units::milli")]
    pub progress: u32,
}

/// An actor the cutscene walks or turns.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CutsceneActorView {
    pub actor: String,
    /// One of `directions`.
    pub direction: String,
    /// Walking (drives the walk cycle).
    pub moving: bool,
}

/// What hosts and renderers show of the running cutscene; `None` without one. Pure.
pub fn view(content: &GameContent, state: &GameState) -> Option<CutsceneView> {
    let running = state.cutscene.as_ref()?;
    let cutscene = find(content, &running.cutscene_id);
    let open = !held(state);
    let line = cutscene
        .and_then(|cutscene| cutscene.steps.get(running.step as usize))
        .filter(|step| open && (step.r#type == steps::SAY || step.r#type == steps::CHOICE))
        .map(|step| {
            let speaker = non_empty(step.speaker.as_deref());
            CutsceneLine {
                text: substitution::fill(content, state, step.text.as_deref().unwrap_or_default()).into_owned(),
                expression: non_empty(step.expression.as_deref()).map(str::to_owned),
                speaker: speaker.map(str::to_owned),
                speaker_name: speaker.filter(|speaker| *speaker != PLAYER).map(|speaker| {
                    content
                        .npcs
                        .iter()
                        .find(|npc| npc.id == speaker)
                        .map_or(speaker, |npc| npc.name.as_str())
                        .to_owned()
                }),
                options: step
                    .options
                    .iter()
                    .map(|option| substitution::fill(content, state, &option.text).into_owned())
                    .collect(),
            }
        });
    let camera = running.camera.as_ref().map(|camera| {
        let (x, y) = camera_point(state, Some(camera));
        let (to_x, to_y) =
            camera.actor.as_deref().and_then(|actor| actor_point(state, actor)).unwrap_or((camera.to_x, camera.to_y));
        CutsceneCameraView {
            x,
            y,
            from_x: camera.from_x,
            from_y: camera.from_y,
            to_x,
            to_y,
            progress: progress(camera.elapsed, camera.duration),
            actor: camera.actor.clone(),
        }
    });
    let fade =
        running.fade.as_ref().map(|fade| CutsceneFadeView { color: fade.color.clone(), alpha: fade_alpha(fade) });
    let emotes = running
        .emotes
        .iter()
        .filter_map(|emote| {
            let (x, y) = actor_point(state, &emote.actor)?;
            Some(CutsceneEmoteView {
                actor: emote.actor.clone(),
                emote: emote.emote.clone(),
                x,
                y,
                progress: progress(emote.elapsed, emote.duration),
            })
        })
        .collect();
    let direction = |actor: &str| {
        if actor == PLAYER {
            state.player.direction.clone()
        } else {
            running.facing.get(actor).cloned().unwrap_or_else(|| directions::DOWN.to_owned())
        }
    };
    let mut actors: Vec<CutsceneActorView> = running
        .walks
        .iter()
        .map(|walk| CutsceneActorView { actor: walk.actor.clone(), direction: direction(&walk.actor), moving: true })
        .collect();
    for actor in running.facing.keys() {
        if !actors.iter().any(|known| known.actor == *actor) {
            actors.push(CutsceneActorView { actor: actor.clone(), direction: direction(actor), moving: false });
        }
    }
    Some(CutsceneView {
        cutscene_id: running.cutscene_id.clone(),
        step: running.step,
        skippable: open && cutscene.is_some_and(|cutscene| cutscene.skippable),
        line,
        camera,
        fade,
        emotes,
        actors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_become_ticks_and_pans_round() {
        assert_eq!(ticks_of(1500), 30);
        assert_eq!(ticks_of(25), 1);
        assert_eq!(ticks_of(0), 0);
        assert_eq!(lerp(0, 100, 1, 3), 33);
        assert_eq!(lerp(0, -100, 2, 3), -67);
        assert_eq!(lerp(10, 20, 5, 0), 20);
        assert_eq!(progress(1, 4), 250);
        assert_eq!(progress(0, 0), 1000);
    }
}
