//! Combat and adventure (M7, optional per game: `settings.combat.enabled`).
//!
//! The player has health beside energy: monsters that touch them and hazard terrain take it,
//! eating restores it (an item's `health`), and a night's sleep fills it. At 0 the player faints
//! and comes to at the faint spot with half their health, paying the faint penalty.
//!
//! Monsters live in the world state: scenes spawn theirs each morning (`scene.monsters`), mine
//! floors when they are made (their depth band's `monsters`). Only the monsters of the player's
//! scene move, one step a tick: chasers come at a player within their sight, wanderers amble,
//! still ones wait. A weapon (a tool whose action is `attack`) hits the monsters on the tiles it
//! covers for its item's `toolPower`, plus the combat level, less the monster's defense; a slain
//! monster rolls its drops and gives combat experience. Their moves and wandering use rolls
//! derived from the game's seed, never its random stream, so a game without monsters is the
//! same game as before.

use crate::effects::{message_levels, Effect};
use crate::engine_types::{Effects, EngineContext};
use crate::messages;
use crate::rng::{self, Rng};
use crate::schema::{
    directions, event_triggers, monster_movements, GameState, MonsterDefinition, MonsterState, ProjectSettings, Scene,
};
use crate::units;
use crate::world::{pathfinding, world_movement};
use crate::{events, gathering, inventory, mines, skills};

/// Half the side of a monster's body, in position units (0.35 tile).
pub const MONSTER_HALF: i32 = units::TILE * 35 / 100;

/// Ticks a monster reels after a hit.
pub const STUN_TICKS: u32 = 8;

/// The player's most health, when the game has combat on.
pub fn max_health(settings: &ProjectSettings) -> Option<i32> {
    settings.combat.as_ref().filter(|combat| combat.enabled).map(|combat| combat.max_health.max(1))
}

/// Whether the game has combat on.
pub fn enabled(ctx: &EngineContext) -> bool {
    max_health(&ctx.content.settings).is_some()
}

/// The monster kind `id`.
pub fn monster<'a>(ctx: &'a EngineContext, id: &str) -> Option<&'a MonsterDefinition> {
    ctx.content.monsters.iter().find(|definition| definition.id == id)
}

/// Heal `amount` health (up to the most), or with a negative `amount` take it (no hurt time).
pub fn modify_health(ctx: &EngineContext, state: &mut GameState, amount: i32) -> Effects {
    let (Some(max), Some(health)) = (max_health(&ctx.content.settings), state.player.health) else {
        return Vec::new();
    };
    if amount >= 0 {
        state.player.health = Some(health.saturating_add(amount).min(max));
        return Vec::new();
    }
    take_health(ctx, state, amount.saturating_neg())
}

/// A hit of `amount`: nothing while the player is still reeling from the last one.
pub fn hurt(ctx: &EngineContext, state: &mut GameState, amount: i32) -> Effects {
    if state.player.health.is_none() || state.player.hurt > 0 || amount <= 0 {
        return Vec::new();
    }
    state.player.hurt = ctx.content.settings.combat.as_ref().map_or(0, |combat| combat.hurt_ticks);
    take_health(ctx, state, amount)
}

fn take_health(ctx: &EngineContext, state: &mut GameState, amount: i32) -> Effects {
    let Some(health) = state.player.health else { return Vec::new() };
    let left = health.saturating_sub(amount).max(0);
    state.player.health = Some(left);
    let mut effects = vec![Effect::PlayerHurt { amount }];
    if left == 0 {
        effects.extend(faint(ctx, state));
    }
    effects
}

/// The player faints: out of the mine, to the faint spot, at half health, paying the penalty.
pub fn faint(ctx: &EngineContext, state: &mut GameState) -> Effects {
    let Some(combat) = ctx.content.settings.combat.as_ref() else { return Vec::new() };
    let max = combat.max_health.max(1);
    let penalty = state.player.money.min(combat.faint_money_penalty.max(0)).max(0);
    state.player.money = state.player.money.saturating_sub(penalty);
    state.player.health = Some((max / 2).max(1));
    state.player.hurt = 0;
    state.player.move_intent = Default::default();
    state.dialogue = None;
    state.shop = None;
    state.storage = None;
    if mines::is_mine_floor(state, &state.player.scene_id) {
        state.world.scenes.retain(|scene| !(scene.is_generated() && mines::is_mine_scene(&scene.id)));
        state.monsters.retain(|monster| !mines::is_mine_scene(&monster.scene_id));
        state.mine.current_floor = 0;
    }
    let scene_id = combat.faint_scene_id.clone().unwrap_or_else(|| ctx.content.start_scene_id.clone());
    let mut effects = Vec::new();
    if world_movement::ensure_scene(ctx, state, &scene_id).is_some() {
        let scene = world_movement::find_scene(state, &scene_id).cloned().unwrap_or_default();
        let x = combat.faint_x.unwrap_or(scene.width / 2);
        let y = combat.faint_y.unwrap_or(scene.height / 2);
        let (landed, landing) = world_movement::land_player(ctx, state, &scene_id, x, y);
        effects.push(Effect::SceneChanged { scene_id, x: landed.x, y: landed.y });
        effects.extend(landing);
    }
    effects.push(Effect::say(message_levels::ERROR, messages::FAINTED.with(&[&penalty])));
    effects
}

/// The night: health fills again.
pub fn restore_overnight(ctx: &EngineContext, state: &mut GameState) {
    if let Some(max) = max_health(&ctx.content.settings) {
        state.player.health = Some(max);
        state.player.hurt = 0;
    }
}

/// A random stream for monsters, from the game's seed and what it is for (never the game's own
/// stream).
fn stream(state: &GameState, purpose: &str) -> Rng {
    let seed = rng::hash_string_to_u32(&format!("{}:{purpose}", state.meta.engine_seed));
    Rng::new(rng::create_rng_state_from_u32(seed))
}

/// Whether a monster body centred at `(x, y)` fits on open tiles of `scene`.
fn body_fits(ctx: &EngineContext, scene: &Scene, x: i32, y: i32) -> bool {
    let corners = [
        (x - MONSTER_HALF, y - MONSTER_HALF),
        (x + MONSTER_HALF - 1, y - MONSTER_HALF),
        (x - MONSTER_HALF, y + MONSTER_HALF - 1),
        (x + MONSTER_HALF - 1, y + MONSTER_HALF - 1),
    ];
    corners.iter().all(|&(cx, cy)| {
        scene
            .tile(units::tile_of(cx), units::tile_of(cy))
            .is_some_and(|tile| !pathfinding::tile_blocks(tile, &ctx.content.node_types, &ctx.content.machine_types))
    })
}

/// Places `count` monsters of `definition` on open tiles of `scene` inside the area, picked with
/// `rng`; returns the new monsters.
fn place(
    ctx: &EngineContext,
    state: &mut GameState,
    scene: &Scene,
    definition: &MonsterDefinition,
    count: u32,
    area: (i32, i32, i32, i32),
    rng: &mut Rng,
) {
    let (x1, y1, x2, y2) = area;
    let mut open: Vec<(i32, i32)> = Vec::new();
    for y in y1.max(0)..=y2.min(scene.height - 1) {
        for x in x1.max(0)..=x2.min(scene.width - 1) {
            let centre = (units::tile_center(x), units::tile_center(y));
            let busy = state
                .monsters
                .iter()
                .any(|m| m.scene_id == scene.id && units::tile_of(m.x) == x && units::tile_of(m.y) == y)
                || scene.transitions.iter().any(|t| (t.from_x, t.from_y) == (x, y))
                || (state.player.scene_id == scene.id
                    && (units::tile_of(state.player.x) - x).abs() <= 1
                    && (units::tile_of(state.player.y) - y).abs() <= 1);
            if !busy && body_fits(ctx, scene, centre.0, centre.1) {
                open.push((x, y));
            }
        }
    }
    for _ in 0..count.min(64) {
        if open.is_empty() {
            break;
        }
        let pick = rng.int(0, open.len() as i64 - 1) as usize;
        let (x, y) = open.remove(pick);
        let id = state.next_monster;
        state.next_monster = state.next_monster.saturating_add(1);
        state.monsters.push(MonsterState {
            id,
            monster_id: definition.id.clone(),
            scene_id: scene.id.clone(),
            x: units::tile_center(x),
            y: units::tile_center(y),
            health: definition.health.max(1),
            ..MonsterState::default()
        });
    }
}

/// The morning's monsters (and the first tick's): every authored scene's monsters are replaced
/// by a fresh set from its `monsters` spawns. Mine floors keep theirs.
pub fn spawn_scenes(ctx: &EngineContext, state: &mut GameState) {
    if !enabled(ctx) || ctx.content.monsters.is_empty() {
        return;
    }
    let scenes: Vec<Scene> = state
        .world
        .scenes
        .iter()
        .filter(|scene| scene.monsters.as_ref().is_some_and(|m| !m.is_empty()))
        .cloned()
        .collect();
    if scenes.is_empty() && state.monsters.is_empty() {
        return;
    }
    state.monsters.retain(|monster| mines::is_mine_scene(&monster.scene_id));
    for scene in &scenes {
        let mut rng = stream(state, &format!("monsters:{}:{}", scene.id, state.clock.day));
        for spawn in scene.monsters.as_deref().unwrap_or_default() {
            let Some(definition) = monster(ctx, &spawn.monster_id) else { continue };
            let x1 = spawn.x.unwrap_or(0);
            let y1 = spawn.y.unwrap_or(0);
            let area = (
                x1,
                y1,
                spawn.x2.unwrap_or(if spawn.x.is_some() { x1 } else { scene.width - 1 }),
                spawn.y2.unwrap_or(if spawn.y.is_some() { y1 } else { scene.height - 1 }),
            );
            place(ctx, state, scene, definition, spawn.count, area, &mut rng);
        }
    }
}

/// A new mine floor's monsters (M7): `monsterCount` of its band's weighted monsters.
pub fn spawn_floor(ctx: &EngineContext, state: &mut GameState, scene_id: &str, floor: u32) {
    if !enabled(ctx) || state.monsters.iter().any(|monster| monster.scene_id == scene_id) {
        return;
    }
    let Some(band) = mines::band_for_floor(&ctx.content.mine, floor) else { return };
    if band.monster_count == 0 || band.monsters.is_empty() {
        return;
    }
    let Some(scene) = world_movement::find_scene(state, scene_id).cloned() else { return };
    let mut rng = stream(state, &format!("mine:{floor}:monsters"));
    let weights: Vec<u32> = band.monsters.iter().map(|entry| entry.weight).collect();
    for _ in 0..band.monster_count.min(64) {
        let Some(definition) = rng.weighted(&weights).and_then(|index| monster(ctx, &band.monsters[index].monster_id))
        else {
            continue;
        };
        let (ex, ey) = mines::FLOOR_ENTRY;
        let area = (1, 1, scene.width - 2, scene.height - 2);
        // Not on the way in: the entry's neighbours stay clear.
        let before = state.monsters.len();
        place(ctx, state, &scene, definition, 1, area, &mut rng);
        if let Some(added) = state.monsters.get(before) {
            if (units::tile_of(added.x) - ex).abs() <= 1 && (units::tile_of(added.y) - ey).abs() <= 1 {
                state.monsters.truncate(before);
            }
        }
    }
}

/// The direction (−1, 0, 1 per axis) from `from` toward `to`, ignoring less than a quarter tile.
fn toward(from: (i32, i32), to: (i32, i32)) -> (i32, i32) {
    let axis = |delta: i32| {
        if delta > units::TILE / 4 {
            1
        } else if delta < -units::TILE / 4 {
            -1
        } else {
            0
        }
    };
    (axis(to.0 - from.0), axis(to.1 - from.1))
}

fn direction_name(dx: i32, dy: i32) -> &'static str {
    match (dx, dy) {
        (_, -1) if dx == 0 => directions::UP,
        (_, 1) if dx == 0 => directions::DOWN,
        (-1, _) => directions::LEFT,
        (1, _) => directions::RIGHT,
        _ => "",
    }
}

fn vector(direction: &str) -> (i32, i32) {
    match direction {
        directions::UP => (0, -1),
        directions::DOWN => (0, 1),
        directions::LEFT => (-1, 0),
        directions::RIGHT => (1, 0),
        _ => (0, 0),
    }
}

/// One tick of the player's scene's monsters: they move, and those touching the player hit them.
pub fn advance_monsters(ctx: &EngineContext, state: &mut GameState) -> Effects {
    if state.player.hurt > 0 {
        state.player.hurt -= 1;
    }
    if state.monsters.is_empty() || !enabled(ctx) {
        return Vec::new();
    }
    let scene_id = state.player.scene_id.clone();
    let Some(scene) = world_movement::find_scene(state, &scene_id).cloned() else { return Vec::new() };
    let player = (state.player.x, state.player.y);
    let tick = state.clock.tick;
    let mut contact = 0;
    for index in 0..state.monsters.len() {
        if state.monsters[index].scene_id != scene_id {
            continue;
        }
        let Some(definition) = monster(ctx, &state.monsters[index].monster_id) else { continue };
        let monster = &mut state.monsters[index];
        if monster.stunned > 0 {
            monster.stunned -= 1;
        } else if definition.movement != monster_movements::STILL {
            let near = {
                let (dx, dy) = ((player.0 - monster.x) as i64, (player.1 - monster.y) as i64);
                let sight = i64::from(definition.sight) * i64::from(units::TILE);
                dx * dx + dy * dy <= sight * sight
            };
            let (dx, dy, speed) = if definition.movement == monster_movements::CHASE && near {
                let (dx, dy) = toward((monster.x, monster.y), player);
                (dx, dy, definition.speed)
            } else {
                if monster.wander == 0 {
                    let mut roll = Rng::new(rng::create_rng_state_from_u32(rng::hash_string_to_u32(&format!(
                        "{}:wander:{}:{tick}",
                        state.meta.engine_seed, monster.id
                    ))));
                    let choice = roll.int(0, 4);
                    monster.direction = [directions::UP, directions::DOWN, directions::LEFT, directions::RIGHT, ""]
                        [choice as usize]
                        .to_owned();
                    monster.wander = roll.int(30, 90) as u32;
                } else {
                    monster.wander -= 1;
                }
                let (dx, dy) = vector(&monster.direction);
                (dx, dy, definition.speed / 2)
            };
            if dx != 0 || dy != 0 {
                let named = direction_name(dx, if dx == 0 { dy } else { 0 });
                if !named.is_empty() {
                    named.clone_into(&mut monster.direction);
                }
            }
            let nx = monster.x.saturating_add(dx.saturating_mul(speed));
            if dx != 0 && body_fits(ctx, &scene, nx, monster.y) {
                monster.x = nx;
            } else if dx != 0 {
                monster.wander = 0;
            }
            let ny = monster.y.saturating_add(dy.saturating_mul(speed));
            if dy != 0 && body_fits(ctx, &scene, monster.x, ny) {
                monster.y = ny;
            } else if dy != 0 {
                monster.wander = 0;
            }
        }
        let reach_x = MONSTER_HALF + world_movement::PLAYER_HALF_WIDTH;
        let reach_y = MONSTER_HALF + world_movement::PLAYER_HALF_HEIGHT;
        if (monster.x - player.0).abs() < reach_x && (monster.y - player.1).abs() < reach_y {
            contact = contact.max(definition.damage);
        }
    }
    if contact > 0 {
        return hurt(ctx, state, contact);
    }
    Vec::new()
}

/// A weapon swing at `tiles` (the tool's area ahead of the player) for `power`: every monster
/// standing on one of them is hit. Returns the effects, or `None` when the swing hit nothing.
pub fn attack(ctx: &EngineContext, state: &mut GameState, tiles: &[(i32, i32)], power: i32) -> Option<Effects> {
    let scene_id = state.player.scene_id.clone();
    let targets: Vec<usize> = state
        .monsters
        .iter()
        .enumerate()
        .filter(|(_, monster)| {
            monster.scene_id == scene_id && tiles.contains(&(units::tile_of(monster.x), units::tile_of(monster.y)))
        })
        .map(|(index, _)| index)
        .collect();
    if targets.is_empty() {
        return None;
    }
    let level = i32::try_from(skills::skill_level(state, "combat")).unwrap_or(i32::MAX);
    let mut effects = Vec::new();
    let mut slain = Vec::new();
    let (px, py) = (state.player.x, state.player.y);
    let scene = world_movement::find_scene(state, &scene_id).cloned().unwrap_or_default();
    for index in targets {
        let Some(definition) = monster(ctx, &state.monsters[index].monster_id) else { continue };
        let damage = power.saturating_add(level).saturating_sub(definition.defense).max(1);
        let target = &mut state.monsters[index];
        target.health = target.health.saturating_sub(damage);
        target.stunned = STUN_TICKS;
        // Knocked back half a tile, where there is room.
        let (dx, dy) = toward((px, py), (target.x, target.y));
        let (kx, ky) = (target.x + dx * units::TILE / 2, target.y + dy * units::TILE / 2);
        if body_fits(ctx, &scene, kx, ky) {
            (target.x, target.y) = (kx, ky);
        }
        if target.health <= 0 {
            slain.push(index);
        }
    }
    slain.sort_by(|a, b| b.cmp(a));
    for index in slain {
        let dead = state.monsters.remove(index);
        let Some(definition) = monster(ctx, &dead.monster_id) else { continue };
        let count = state.slain.entry(dead.monster_id.clone()).or_insert(0);
        *count = count.saturating_add(1);
        let mut rng = Rng::new(state.rng.clone());
        let drops = gathering::roll_drop_table(&definition.drops, &mut rng);
        state.rng = rng.state;
        let mut received = Vec::new();
        for drop in drops {
            let Some(item) = ctx.item(&drop.item_id) else { continue };
            let added = inventory::add_item(
                &state.player.inventory,
                item,
                drop.quantity,
                state.player.max_inventory_size,
                None,
            );
            if added.added {
                state.player.inventory = added.inventory;
                received.push(format!("{}x {}", drop.quantity, item.name));
            } else {
                effects.push(Effect::say(message_levels::ERROR, &messages::INVENTORY_FULL));
            }
        }
        effects.push(Effect::say(
            message_levels::SUCCESS,
            if received.is_empty() {
                messages::MONSTER_SLAIN.with(&[&definition.name])
            } else {
                messages::MONSTER_SLAIN_GOT.with(&[&definition.name, &received.join(", ")])
            },
        ));
        effects.extend(skills::grant_xp(ctx, state, "combat", definition.experience));
        effects.extend(events::fire_trigger(
            ctx,
            state,
            &events::Trigger::about(event_triggers::SLAY, &[definition.id.as_str()]),
        ));
    }
    Some(effects)
}

/// Monsters slain of `monster_id` (empty: of any kind).
pub fn slain(state: &GameState, monster_id: &str) -> u64 {
    if monster_id.is_empty() {
        state.slain.values().map(|count| u64::from(*count)).sum()
    } else {
        state.slain.get(monster_id).map_or(0, |count| u64::from(*count))
    }
}
