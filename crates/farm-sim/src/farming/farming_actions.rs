//! Farming & tool interactions (M2): energy costs, day-based growth, gathering-node strikes,
//! tiered area-of-effect, explicit selling (harvest no longer auto-pays). (Port of
//! `Farming/FarmingActions.cs` / engine-core/src/farming/actions.ts.)

use crate::effects::{message_levels, Effect};
use crate::engine_types::{Effects, EngineContext};
use crate::events::EventPosition;
use crate::farming::{crops, multi_tile};
use crate::hooks::{CropHarvestHookPayload, HookEvent, NpcInteractHookPayload};
use crate::messages;
use crate::rng::Rng;
use crate::schema::{
    event_triggers, soil_states, tile_layers, tool_actions, Crop, CropDefinition, DialogueState, GameState, Item,
    TerrainTransform, Tile, ToolArea, ToolDefinition, FISHING_MINIGAME_ID,
};
use crate::world::tiles;
use crate::world::world_movement;
use crate::{
    animals, crafting, economy, energy, events, fishing, gathering, inventory, mines, quests, relationships, skills,
    social, tools, units,
};
use indexmap::IndexMap;
use serde_json::Value;

/// TS `facingTile` result. The C# record holds the `Scene`; here the scene's identity and size
/// are copied out (and the tile cloned) so the caller can go on to mutate the state.
#[derive(Clone)]
struct FacingTileResult {
    scene_id: String,
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    tile: Tile,
}

fn facing_tile(state: &GameState) -> Option<FacingTileResult> {
    let scene = world_movement::find_scene(state, &state.player.scene_id)?;
    let vector = world_movement::get_direction_vector(&state.player.direction);
    let origin = world_movement::player_tile(state);
    let x = origin.x + vector.dx;
    let y = origin.y + vector.dy;
    if x < 0 || x >= scene.width || y < 0 || y >= scene.height {
        return None;
    }
    Some(FacingTileResult {
        scene_id: scene.id.clone(),
        width: scene.width,
        height: scene.height,
        x,
        y,
        tile: scene.tile(x, y)?.clone(),
    })
}

/// The tiles a tool covers: rows of `area.width` tiles across the facing direction, centred on
/// the faced tile (then alternating left and right), `area.length` rows ahead. Tiles outside the
/// scene are left out. The built-in tier 2 area (3 × 1) is the faced tile, then its two
/// perpendicular neighbours.
fn area_targets(target: &FacingTileResult, direction: &str, area: &ToolArea) -> Vec<(i32, i32)> {
    let ahead = world_movement::get_direction_vector(direction);
    let (across_x, across_y) = if direction == "up" || direction == "down" { (1, 0) } else { (0, 1) };
    let mut offsets = vec![0_i32];
    let width = i32::try_from(area.width.clamp(1, 15)).unwrap_or(1);
    let mut step = 1;
    while (offsets.len() as i32) < width {
        offsets.push(-step);
        if (offsets.len() as i32) < width {
            offsets.push(step);
        }
        step += 1;
    }
    let mut targets = Vec::new();
    for row in 0..i32::try_from(area.length.clamp(1, 15)).unwrap_or(1) {
        for offset in &offsets {
            let tx = target.x + ahead.dx * row + across_x * offset;
            let ty = target.y + ahead.dy * row + across_y * offset;
            if tx >= 0 && tx < target.width && ty >= 0 && ty < target.height {
                targets.push((tx, ty));
            }
        }
    }
    targets
}

/// TS `waterTileInPlace`: only soil (a plantable terrain) takes water.
fn water_tile(ctx: &EngineContext, tile: &mut Tile, day: u32) {
    if !ctx.plantable(&tile.background) {
        return;
    }
    tile.soil_moisture = 100;
    tile.soil_state = Some(
        if tile.soil_state.as_deref() == Some(soil_states::FERTILIZED) {
            soil_states::FERTILIZED
        } else {
            soil_states::WATERED
        }
        .to_owned(),
    );
    if let Some(crop) = &mut tile.crop {
        if crop.withered != Some(true) {
            crop.watered = true;
            crop.last_watered_day = Some(day);
            crop.days_without_water = 0;
        }
    }
}

/// Water the tile at (x, y) of scene `scene_index` the way the watering can does (the event
/// outcome `waterArea` shares it). A multi-tile crop is watered on all of its tiles. Positions
/// outside the grid are ignored.
pub(crate) fn water_at(ctx: &EngineContext, state: &mut GameState, scene_index: usize, x: usize, y: usize) {
    let Some(scene) = state.world.scenes.get(scene_index) else { return };
    if scene.tiles.get(y).and_then(|row| row.get(x)).is_none() {
        return;
    }
    let positions = multi_tile::group_positions(ctx, scene, x, y);
    let day = state.clock.day;
    let tiles = &mut state.world.scenes[scene_index].tiles;
    water_tile(ctx, &mut tiles[y][x], day);
    for (tx, ty) in positions {
        if (tx, ty) != (x, y) {
            if let Some(tile) = tiles.get_mut(ty).and_then(|row| row.get_mut(tx)) {
                water_tile(ctx, tile, day);
            }
        }
    }
}

fn scene_index(state: &GameState, scene_id: &str) -> Option<usize> {
    state.world.scenes.iter().position(|scene| scene.id == scene_id)
}

/// Durability + energy apply after a successful action (the C# local `Finish`).
fn finish(ctx: &EngineContext, state: &mut GameState, tool: &Item, energy_cost: i32, mut effects: Effects) -> Effects {
    state.player.inventory =
        inventory::replace_item(&state.player.inventory, &tool.id, &tools::damage_tool_durability(tool, 1));
    let spent = energy::spend_energy(ctx, state, energy_cost);
    effects.extend(spent.effects);
    effects
}

/// The `useTool` command: use the held tool of `tool_type` on the faced tile, spending energy and
/// durability when it does something. Any tool strikes the gathering node it faces; otherwise the
/// tool's action decides (M1: tools are content, [`crate::schema::ToolDefinition`]).
pub fn handle_use_tool(ctx: &EngineContext, state: &mut GameState, tool_type: &str) -> Effects {
    let Some(tool_slot) = inventory::find_tool_slot(&state.player.inventory, tool_type).cloned() else {
        // "watering can", or the tool's name.
        return vec![Effect::say(
            message_levels::ERROR,
            messages::NEED_TOOL.with_args(vec![messages::tool_noun_in(ctx, tool_type)]),
        )];
    };
    if tools::is_tool_broken(&tool_slot.item) {
        return vec![Effect::say(message_levels::ERROR, messages::TOOL_BROKEN.with(&[&tool_slot.item.name]))];
    }

    let Some(target) = facing_tile(state) else {
        return Vec::new();
    };

    let tier = tool_slot.item.tool_tier.unwrap_or(1);
    let power = tool_slot.item.tool_power.unwrap_or(tier);
    // An unknown tool type does nothing but strike nodes (which refuse it), for no energy.
    let definition = ctx.tool(tool_type).cloned().unwrap_or_default();
    let energy_cost = energy::effective_energy_cost(&definition, tier);
    let area = definition.area_at(tier);
    let tool = &tool_slot.item;
    let background = ctx.terrain(&target.tile.background);

    let used = if target.tile.node.is_some() && gathering::is_node_active(&target.tile) {
        // Gathering node strike takes priority on node tiles.
        let outcome = gathering::strike_node(ctx, state, &target.scene_id, target.x, target.y, tool_type, tier, power);
        if outcome.struck {
            ToolUse::Used(outcome.effects)
        } else {
            ToolUse::Refused(outcome.effects)
        }
    } else {
        match definition.action.as_str() {
            tool_actions::WATER if background.is_some_and(|terrain| terrain.plantable) => {
                water_with_can(ctx, state, &target, &area)
            }
            tool_actions::TILL if till_into(ctx, &definition, &target.tile).is_some() => {
                till_with_hoe(ctx, state, &target, &definition, &area)
            }
            tool_actions::HARVEST if target.tile.crop.is_some() => harvest_area(ctx, state, &target, &area),
            tool_actions::FISH if background.is_some_and(|terrain| terrain.fishable) => {
                cast_fishing_rod(ctx, state, tier)
            }
            tool_actions::BREAK | tool_actions::TRANSFORM
                if layer_transform(ctx, &definition, &target.tile).is_some() =>
            {
                transform_area(ctx, state, &target, &definition, &area)
            }
            tool_actions::PLACE => place_area(ctx, state, &target, &definition, &area),
            tool_actions::PLANT if background.is_some_and(|terrain| terrain.plantable) => {
                plant_area(ctx, state, &target, &area)
            }
            _ => ToolUse::NotHere,
        }
    };

    match used {
        ToolUse::Used(effects) => finish(ctx, state, tool, energy_cost, effects),
        ToolUse::Refused(effects) => effects,
        ToolUse::NotHere => vec![Effect::say(message_levels::INFO, messages::CANT_USE_HERE.with(&[&tool.name]))],
    }
}

/// What a tool did to its target.
enum ToolUse {
    /// The tool was used: it costs energy and durability ([`finish`]), then these effects.
    Used(Effects),
    /// Nothing happened (or the target refused it): just these effects.
    Refused(Effects),
    /// The tool does nothing to this target.
    NotHere,
}

/// Whether a terrain transform applies to `tool` doing `action`.
fn transform_matches(transform: &TerrainTransform, tool: &ToolDefinition, action: &str) -> bool {
    transform.action == action && transform.tool.as_deref().is_none_or(|only| only == tool.id)
}

/// The terrain the till action turns `tile`'s background into, if it tills.
fn till_into(ctx: &EngineContext, tool: &ToolDefinition, tile: &Tile) -> Option<String> {
    let terrain = ctx.terrain(&tile.background)?;
    terrain
        .transforms
        .iter()
        .find(|transform| transform_matches(transform, tool, tool_actions::TILL) && !transform.into.is_empty())
        .map(|transform| transform.into.clone())
}

/// The layer and the terrain (empty: none) the tool's break or transform action turns `tile`
/// into: the object first, then the overlay, then the background.
fn layer_transform(ctx: &EngineContext, tool: &ToolDefinition, tile: &Tile) -> Option<(&'static str, String)> {
    let layers = [
        (tile_layers::OBJECT, tile.object.as_deref()),
        (tile_layers::OVERLAY, tile.overlay.as_deref()),
        (tile_layers::BACKGROUND, Some(tile.background.as_str())),
    ];
    layers.into_iter().find_map(|(layer, id)| {
        let terrain = ctx.terrain(id?)?;
        let transform = terrain.transforms.iter().find(|transform| transform_matches(transform, tool, &tool.action))?;
        // A background can't be removed, only changed.
        (layer != tile_layers::BACKGROUND || !transform.into.is_empty()).then(|| (layer, transform.into.clone()))
    })
}

/// A tile turned into soil (a plantable terrain): dry and unfertilized.
fn reset_soil(tile: &mut Tile) {
    tile.soil_moisture = 0;
    tile.soil_fertility = 0;
    tile.soil_state = Some(soil_states::DRY.to_owned());
}

/// The watering can on soil: water the tool tier's area in front of the player.
fn water_with_can(ctx: &EngineContext, state: &mut GameState, target: &FacingTileResult, area: &ToolArea) -> ToolUse {
    let Some(index) = scene_index(state, &target.scene_id) else {
        return ToolUse::Refused(Vec::new());
    };
    let direction = state.player.direction.clone();
    for (spot_x, spot_y) in area_targets(target, &direction, area) {
        water_at(ctx, state, index, spot_x as usize, spot_y as usize);
    }
    ToolUse::Used(vec![Effect::say(message_levels::SUCCESS, &messages::WATERED)])
}

/// The hoe on tillable ground (grass or floor in the built-in terrains): till the tool tier's
/// area in front of the player into dry soil (tiles with a gathering node stay).
fn till_with_hoe(
    ctx: &EngineContext,
    state: &mut GameState,
    target: &FacingTileResult,
    tool: &ToolDefinition,
    area: &ToolArea,
) -> ToolUse {
    let Some(index) = scene_index(state, &target.scene_id) else {
        return ToolUse::Refused(Vec::new());
    };
    let direction = state.player.direction.clone();
    for (spot_x, spot_y) in area_targets(target, &direction, area) {
        let Some(spot_tile) = state.world.scenes[index].tile(spot_x, spot_y) else {
            continue;
        };
        if spot_tile.node.is_some() {
            continue;
        }
        let Some(into) = till_into(ctx, tool, spot_tile) else { continue };
        let plantable = ctx.plantable(&into);
        let Some(spot_tile) = state.world.scenes[index].tile_mut(spot_x, spot_y) else { continue };
        spot_tile.background.clone_from(&into);
        spot_tile.r#type = into;
        if plantable {
            reset_soil(spot_tile);
        }
    }
    ToolUse::Used(vec![Effect::say(message_levels::SUCCESS, &messages::TILLED)])
}

/// A tool's break or transform action: each tile of its area with a transform for it changes
/// (its object, overlay or background, the first that has one).
fn transform_area(
    ctx: &EngineContext,
    state: &mut GameState,
    target: &FacingTileResult,
    tool: &ToolDefinition,
    area: &ToolArea,
) -> ToolUse {
    let Some(index) = scene_index(state, &target.scene_id) else {
        return ToolUse::Refused(Vec::new());
    };
    let direction = state.player.direction.clone();
    let mut changed: Option<(String, String)> = None;
    for (spot_x, spot_y) in area_targets(target, &direction, area) {
        let Some(spot_tile) = state.world.scenes[index].tile(spot_x, spot_y) else { continue };
        if gathering::is_node_active(spot_tile) {
            continue;
        }
        let Some((layer, into)) = layer_transform(ctx, tool, spot_tile) else { continue };
        let from = match layer {
            tile_layers::OBJECT => spot_tile.object.clone(),
            tile_layers::OVERLAY => spot_tile.overlay.clone(),
            _ => Some(spot_tile.background.clone()),
        }
        .unwrap_or_default();
        let mut updated = if into.is_empty() {
            tiles::clear_tile_layer_in(ctx.terrains(), spot_tile, layer)
        } else {
            tiles::set_tile_layer_in(ctx.terrains(), spot_tile, &into, None)
        };
        if layer == tile_layers::BACKGROUND && ctx.plantable(&into) && !ctx.plantable(&from) {
            reset_soil(&mut updated);
        }
        if layer == tile_layers::BACKGROUND && !ctx.plantable(&into) {
            updated.crop = None;
        }
        if changed.is_none() {
            changed = Some((from, into));
        }
        if let Some(tile) = state.world.scenes[index].tile_mut(spot_x, spot_y) {
            *tile = updated;
        }
    }
    match changed {
        Some((from, into)) => {
            let name = |id: &str| ctx.terrain(id).map_or_else(|| id.to_owned(), |terrain| terrain.name.clone());
            let message = if into.is_empty() {
                messages::CLEARED_TERRAIN.with(&[&name(&from)])
            } else {
                messages::TRANSFORMED_TERRAIN.with(&[&name(&from), &name(&into)])
            };
            ToolUse::Used(vec![Effect::say(message_levels::SUCCESS, message)])
        }
        None => ToolUse::NotHere,
    }
}

/// A tool's place action: its terrain goes down on each tile of its area that holds nothing
/// (no crop, node, machine, dropped item or NPC) and does not have to stay clear.
fn place_area(
    ctx: &EngineContext,
    state: &mut GameState,
    target: &FacingTileResult,
    tool: &ToolDefinition,
    area: &ToolArea,
) -> ToolUse {
    let Some(terrain) = tool.place_terrain.as_deref().and_then(|id| ctx.terrain(id)) else {
        return ToolUse::NotHere;
    };
    let Some(index) = scene_index(state, &target.scene_id) else {
        return ToolUse::Refused(Vec::new());
    };
    let direction = state.player.direction.clone();
    let mut placed = false;
    for (spot_x, spot_y) in area_targets(target, &direction, area) {
        let Some(spot_tile) = state.world.scenes[index].tile(spot_x, spot_y) else { continue };
        let occupied = spot_tile.crop.is_some()
            || spot_tile.node.is_some()
            || spot_tile.machine.is_some()
            || spot_tile.item.is_some()
            || spot_tile.collision;
        let current = match terrain.layer.as_str() {
            tile_layers::OBJECT => spot_tile.object.as_deref(),
            tile_layers::OVERLAY => spot_tile.overlay.as_deref(),
            _ => Some(spot_tile.background.as_str()),
        };
        let npc_there = state.npcs.values().any(|npc| {
            npc.scene_id == target.scene_id && npc.x == units::tiles(spot_x) && npc.y == units::tiles(spot_y)
        });
        if occupied || npc_there || current == Some(terrain.id.as_str()) {
            continue;
        }
        if !terrain.walkable && crafting::keeps_clear(ctx, state, &target.scene_id, spot_x, spot_y) {
            continue;
        }
        let mut updated = tiles::set_tile_layer_in(ctx.terrains(), spot_tile, &terrain.id, terrain.visual.as_ref());
        if terrain.plantable && terrain.layer == tile_layers::BACKGROUND {
            reset_soil(&mut updated);
        }
        if let Some(tile) = state.world.scenes[index].tile_mut(spot_x, spot_y) {
            *tile = updated;
        }
        placed = true;
    }
    if placed {
        ToolUse::Used(vec![Effect::say(message_levels::SUCCESS, messages::PLACED.with(&[&terrain.name]))])
    } else {
        ToolUse::Refused(vec![Effect::say(message_levels::ERROR, &messages::NO_ROOM_TO_PLACE)])
    }
}

/// A tool's plant action (a seed drone): the first seed held goes into each open soil tile of
/// its area, as planting by hand would (with its checks and messages, the first shown).
fn plant_area(ctx: &EngineContext, state: &mut GameState, target: &FacingTileResult, area: &ToolArea) -> ToolUse {
    let direction = state.player.direction.clone();
    let mut first: Option<Effects> = None;
    let mut planted = false;
    for (spot_x, spot_y) in area_targets(target, &direction, area) {
        let Some(scene) = world_movement::find_scene(state, &target.scene_id) else { break };
        let Some(spot_tile) = scene.tile(spot_x, spot_y) else { continue };
        if spot_tile.crop.is_some() || !ctx.plantable(&spot_tile.background) || gathering::is_node_active(spot_tile) {
            continue;
        }
        let before = state.player.inventory.clone();
        let effects = plant_seed(ctx, state, &target.scene_id, spot_x, spot_y, PlantChoice::default());
        let worked = state.player.inventory != before;
        planted |= worked;
        if first.is_none() {
            first = Some(effects);
        }
        if !worked {
            break;
        }
    }
    let effects = first.unwrap_or_default();
    if planted {
        ToolUse::Used(effects)
    } else if effects.is_empty() {
        ToolUse::NotHere
    } else {
        ToolUse::Refused(effects)
    }
}

/// The scythe (a harvest tool) over its area: each crop is cleared when withered or harvested
/// when mature ([`harvest_with_scythe`]); a one-tile area is the faced crop alone.
fn harvest_area(ctx: &EngineContext, state: &mut GameState, target: &FacingTileResult, area: &ToolArea) -> ToolUse {
    if area.width <= 1 && area.length <= 1 {
        return harvest_with_scythe(ctx, state, target);
    }
    let direction = state.player.direction.clone();
    let mut used: Option<Effects> = None;
    let mut refused: Option<Effects> = None;
    for (spot_x, spot_y) in area_targets(target, &direction, area) {
        let Some(scene) = world_movement::find_scene(state, &target.scene_id) else { break };
        let Some(spot_tile) = scene.tile(spot_x, spot_y).filter(|tile| tile.crop.is_some()).cloned() else { continue };
        let spot = FacingTileResult { x: spot_x, y: spot_y, tile: spot_tile, ..target.clone() };
        match harvest_with_scythe(ctx, state, &spot) {
            ToolUse::Used(effects) => used.get_or_insert_with(Vec::new).extend(effects),
            ToolUse::Refused(effects) => {
                refused.get_or_insert(effects);
            }
            ToolUse::NotHere => {}
        }
    }
    match (used, refused) {
        (Some(effects), _) => ToolUse::Used(effects),
        (None, Some(effects)) => ToolUse::Refused(effects),
        (None, None) => ToolUse::NotHere,
    }
}

/// The scythe on a crop: clear a withered crop (a multi-tile one whole) or harvest a mature
/// one.
fn harvest_with_scythe(ctx: &EngineContext, state: &mut GameState, target: &FacingTileResult) -> ToolUse {
    let Some(crop) = &target.tile.crop else {
        return ToolUse::NotHere;
    };
    if crop.withered == Some(true) {
        let Some(index) = scene_index(state, &target.scene_id) else {
            return ToolUse::Refused(Vec::new());
        };
        // A withered multi-tile crop is cleared whole.
        let scene = &state.world.scenes[index];
        for (x, y) in multi_tile::group_positions(ctx, scene, target.x as usize, target.y as usize) {
            if let Some(tile) = state.world.scenes[index].tile_mut(x as i32, y as i32) {
                tile.crop = None;
            }
        }
        return ToolUse::Used(vec![Effect::say(message_levels::SUCCESS, &messages::CLEARED_WITHERED)]);
    }
    if let Some(crop_def) = ctx.content.crops.get(&crop.r#type) {
        if crops::is_crop_mature_by_days(crop, crop_def) {
            return ToolUse::Used(harvest_crop(ctx, state, &target.scene_id, target.x, target.y));
        }
    }
    ToolUse::Refused(vec![Effect::say(message_levels::INFO, &messages::CROP_NOT_READY)])
}

/// The fishing rod on water. A declared 'fishing' minigame gates the catch on player skill; the
/// score re-enters through the resolveMinigame command. Without one (or while another minigame
/// is open) the cast resolves instantly (original behavior).
fn cast_fishing_rod(ctx: &EngineContext, state: &mut GameState, tier: i32) -> ToolUse {
    if let Some(minigame) = ctx.minigame(FISHING_MINIGAME_ID) {
        if state.minigame.is_none() {
            let mut context: IndexMap<String, Value> = IndexMap::new();
            context.insert("builtin".to_owned(), Value::String("fishing".to_owned()));
            context.insert("rodTier".to_owned(), Value::from(tier));
            return ToolUse::Used(events::start_minigame_session(ctx, state, &minigame.id, Some(&context)));
        }
    }
    ToolUse::Used(fishing::resolve_fishing(ctx, state, tier, None).effects)
}

/// What the player chose to plant with (the `interactWith` command). The default is plain
/// `interact`: the first seed held, with the first fertilizer held.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlantChoice<'a> {
    /// Plant this seed item (`None`: the first seed held that grows this season; for plain
    /// `interact`, the first seed held).
    pub seed_item_id: Option<&'a str>,
    /// Fertilize with this item (`None`: no fertilizer, or the legacy automatic pick).
    pub fertilizer_item_id: Option<&'a str>,
    /// The player chose: fertilizer is used only when named, and an unnamed seed is the first
    /// that grows this season. False for plain `interact`, which keeps the recorded games'
    /// rules (the first seed held, the first fertilizer held).
    pub chosen: bool,
}

/// The `interact` command: [`handle_interact_with`] with the default planting choice.
pub fn handle_interact(ctx: &EngineContext, state: &mut GameState) -> Effects {
    handle_interact_with(ctx, state, PlantChoice::default())
}

/// Interact with the faced tile (events, NPCs, animals, machines, the mine, crops), planting on
/// open soil according to `choice`.
pub fn handle_interact_with(ctx: &EngineContext, state: &mut GameState, choice: PlantChoice<'_>) -> Effects {
    let target = facing_tile(state);

    let facing = world_movement::facing_target(state);
    let target_x = facing.x;
    let target_y = facing.y;

    // Authored interact-events take priority over built-in interactions (M3). JS compared the
    // state before and after (`eventResult.state !== state`), which differs exactly when an
    // event fired.
    let events =
        events::evaluate_events_detailed(ctx, state, "interact", Some(EventPosition { x: target_x, y: target_y }));
    if events.fired || !events.effects.is_empty() {
        return events.effects;
    }

    // NPC dialogue next (uses live NPC positions from state)
    if let Some(npc_entry_id) = social::npc_on_tile(ctx, state, target_x, target_y) {
        let npc_def = ctx.npc(&npc_entry_id);
        if let Some(npc_def) = npc_def {
            // A heart event due here plays in place of the conversation (M4).
            if relationships::start_heart_event(ctx, state, relationships::Place::Talk(&npc_def.id)) {
                relationships::note_talk(ctx, state, &npc_def.id);
                return Vec::new();
            }
            // Today's line (M4) is said in place of the first dialogue's text.
            let line = relationships::pick_line(ctx, state, npc_def);
            if !npc_def.dialogue.is_empty() || line.is_some() {
                ctx.emit(HookEvent::NpcInteract(NpcInteractHookPayload { npc_id: npc_def.id.clone() }));
                let dialogue_id = npc_def.dialogue.first().map(|dialogue| dialogue.id.clone()).unwrap_or_default();
                state.dialogue = Some(DialogueState { npc_id: npc_def.id.clone(), dialogue_id, line });
                relationships::note_talk(ctx, state, &npc_def.id);
                return quests::progress_quests(ctx, state, "talk", &npc_def.id, 1);
            }
        }
        return Vec::new();
    }

    let Some(target) = target else {
        return Vec::new();
    };

    // Animals (M4c): feed → collect → pet
    let animal = animals::animal_at(state, &state.player.scene_id, target.x, target.y).cloned();
    if let Some(animal) = animal {
        return animals::handle_animal_interaction(ctx, state, &animal);
    }

    // Machines (M4a): collect finished output (loading goes through machineLoad)
    if let Some(machine) = &target.tile.machine {
        if machine.output.as_ref().is_some_and(|output| !output.is_empty()) {
            return crafting::collect_machine_output(ctx, state, &target.scene_id, target.x, target.y);
        }
        if machine.processing.is_some() {
            return vec![Effect::say(message_levels::INFO, &messages::MACHINE_STILL_WORKING_SHORT)];
        }
        let machine_def = ctx.machine_type(&machine.type_id);
        let name = machine_def.map_or("Machine", |def| def.name.as_str());
        return vec![Effect::say(message_levels::INFO, messages::MACHINE_IDLE.with(&[&name]))];
    }

    // Mine (M4f): entrance descends (elevator checkpoint when unlocked);
    // interacting near the entry tile of a floor climbs out.
    let mine_config = &ctx.content.mine;
    if mine_config.enabled
        && state.player.scene_id == mine_config.entrance_scene_id.as_deref().unwrap_or("")
        && target.x == mine_config.entrance_x.unwrap_or(-1)
        && target.y == mine_config.entrance_y.unwrap_or(-1)
    {
        let floor = mines::elevator_floor(mine_config, state);
        return mines::descend_mine(ctx, state, floor);
    }
    if mines::is_mine_floor(state, &state.player.scene_id) && (target.x, target.y) == mines::FLOOR_ENTRY {
        return mines::exit_mine(ctx, state);
    }

    if let Some(crop) = &target.tile.crop {
        if crop.withered == Some(true) {
            return vec![Effect::say(message_levels::INFO, &messages::CROP_WITHERED)];
        }
        if let Some(definition) = ctx.content.crops.get(&crop.r#type) {
            if !crops::is_crop_mature_by_days(crop, definition) {
                return vec![Effect::say(message_levels::INFO, &messages::CROP_NOT_READY)];
            }
        }
        return harvest_crop(ctx, state, &target.scene_id, target.x, target.y);
    }

    if ctx.plantable(&target.tile.background) && !gathering::is_node_active(&target.tile) {
        return plant_seed(ctx, state, &target.scene_id, target.x, target.y, choice);
    }

    Vec::new()
}

fn harvest_crop(ctx: &EngineContext, state: &mut GameState, scene_id: &str, x: i32, y: i32) -> Effects {
    let Some(scene) = world_movement::find_scene(state, scene_id) else {
        return Vec::new();
    };
    let Some(crop) = scene.tile(x, y).and_then(|tile| tile.crop.clone()) else {
        return Vec::new();
    };
    if crop.withered == Some(true) {
        return Vec::new();
    }

    let Some(definition) = ctx.content.crops.get(&crop.r#type) else {
        return Vec::new();
    };
    if !crops::is_crop_mature_by_days(&crop, definition) {
        return vec![Effect::say(message_levels::INFO, &messages::CROP_NOT_READY)];
    }

    let crop_item = match crops::harvest_item(&ctx.content, definition) {
        Ok(item) => item,
        // A content mistake (Problems reports it as crop.noHarvestItem): say so instead of
        // silently doing nothing.
        Err(crop_item_id) => {
            return vec![Effect::say(
                message_levels::ERROR,
                messages::HARVEST_ITEM_MISSING.with(&[&definition.name, &crop_item_id]),
            )]
        }
    };
    // Every tile of a multi-tile crop is harvested together, once.
    let positions = multi_tile::group_positions(ctx, scene, x as usize, y as usize);

    // Quality is decided at harvest: the planted quality, raised by farming skill.
    let tiers = ctx.crop_qualities();
    let quality = crops::harvest_quality(tiers, &crop.quality, skills::skill_level(state, "farming"));
    let tier = tiers.iter().find(|tier| tier.id == quality);
    let mut rng = Rng::new(state.rng.clone());
    let mutation = crops::roll_mutation(Some(definition), tier, ctx.crop_mutations(), &mut rng);
    // Farming skill: +1 yield per 4 levels (M4g)
    let quantity = crops::roll_yield(Some(definition), tier, mutation, &mut rng)
        .saturating_add(skills::farming_yield_bonus(state));
    let mutation = mutation.map(|mutation| mutation.name.clone());
    // What the harvest is worth at a shop's base price: the quality value of the units added
    // (a mutation pays through its larger yield).
    let estimated_value =
        economy::quality_value(&ctx.content, crop_item, Some(&quality)).saturating_mul(i64::from(quantity));
    let slot_quality = ctx.slot_quality(Some(&quality));

    let add_result = inventory::add_item_with_quality(
        &state.player.inventory,
        crop_item,
        slot_quality.as_deref(),
        quantity,
        state.player.max_inventory_size,
        None,
    );
    if !add_result.added {
        // Full inventory aborts the harvest; the rng draws are discarded.
        return vec![Effect::say(message_levels::ERROR, &messages::INVENTORY_FULL)];
    }

    let Some(index) = scene_index(state, scene_id) else {
        return Vec::new();
    };
    for (tile_x, tile_y) in positions {
        let Some(tile) = state.world.scenes[index].tile_mut(tile_x as i32, tile_y as i32) else { continue };
        let Some(planted) = tile.crop.take() else { continue };
        if definition.can_regrow {
            let growth_days = crops::crop_growth_days(definition);
            let regrowth = crops::crop_regrowth_days(definition);
            let mut regrown = Crop {
                days_grown: Some(growth_days.saturating_sub(regrowth)),
                harvest_count: planted.harvest_count.saturating_add(1),
                watered: false,
                ..planted
            };
            regrown.stage = crops::compute_crop_stage(&regrown, definition);
            tile.crop = Some(regrown);
        }
    }

    // One star per the tier's stars; a mutation by its name in capitals.
    let stars = tier.map_or(0, |tier| tier.stars).min(10);
    let quality_text = if stars == 0 { String::new() } else { format!(" {}", "⭐".repeat(stars as usize)) };
    let mutation_text = match &mutation {
        Some(mutation) if !mutation.is_empty() => format!(" ({}!)", mutation.to_uppercase()),
        _ => String::new(),
    };

    state.rng = rng.state;
    state.player.inventory = add_result.inventory;

    let mut effects: Effects = vec![
        Effect::say(
            message_levels::SUCCESS,
            messages::HARVESTED.with(&[&quantity, &definition.name, &quality_text, &mutation_text, &estimated_value]),
        ),
        Effect::CropHarvested { crop_type: crop.r#type.clone(), quantity },
    ];
    ctx.emit(HookEvent::CropHarvest(CropHarvestHookPayload {
        crop_type: crop.r#type.clone(),
        quantity,
        quality: quality.clone(),
    }));

    effects.extend(skills::grant_xp(ctx, state, "farming", 8));
    effects.extend(quests::progress_quests(ctx, state, "harvest", &crop.r#type, quantity));
    effects.extend(events::fire_trigger(
        ctx,
        state,
        &events::Trigger::about(event_triggers::HARVEST, &[crop.r#type.as_str(), crop_item.id.as_str()]),
    ));
    effects
}

/// A held slot of `item_id` whose item is of the kind `kind` accepts.
fn held_of_kind<'a>(state: &'a GameState, item_id: &str, kind: impl Fn(&Item) -> bool) -> Option<&'a Item> {
    state
        .player
        .inventory
        .iter()
        .find(|slot| slot.item.id == item_id && kind(&slot.item) && slot.quantity > 0)
        .map(|slot| &slot.item)
}

/// Whether `item` is a seed: its category is plantable.
pub(crate) fn is_seed(ctx: &EngineContext, item: &Item) -> bool {
    ctx.item_category(&item.r#type).is_some_and(|category| category.plantable)
}

/// Whether `item` is a fertilizer: its category fertilizes.
pub(crate) fn is_fertilizer(ctx: &EngineContext, item: &Item) -> bool {
    ctx.item_category(&item.r#type).is_some_and(|category| category.fertilizer)
}

/// Whether the crop may be planted with the fertilizer `item_id` (any, unless its requirements
/// name some).
fn allowed_fertilizer(definition: &CropDefinition, item_id: &str) -> bool {
    definition
        .requirements
        .as_ref()
        .is_none_or(|needs| needs.fertilizers.is_empty() || needs.fertilizers.iter().any(|id| id == item_id))
}

/// An item's name for a message: its content name, else the id.
fn item_name(ctx: &EngineContext, item_id: &str) -> String {
    ctx.item(item_id).map_or_else(|| item_id.to_owned(), |item| item.name.clone())
}

fn plant_seed(
    ctx: &EngineContext,
    state: &mut GameState,
    scene_id: &str,
    x: i32,
    y: i32,
    choice: PlantChoice<'_>,
) -> Effects {
    let Some(scene) = world_movement::find_scene(state, scene_id) else {
        return Vec::new();
    };

    let seed_item = match choose_seed(ctx, state, choice) {
        Ok(item) => item,
        Err(refused) => return refused,
    };
    let Some(crop_type) = seed_item.crop_type.as_deref().filter(|crop| !crop.is_empty()) else {
        return vec![Effect::say(message_levels::INFO, &messages::NO_SEEDS)];
    };
    let seed_item_id = seed_item.id.clone();

    let Some(definition) = ctx.content.crops.get(crop_type) else {
        return vec![Effect::say(message_levels::ERROR, &messages::INVALID_CROP)];
    };

    if !crops::can_grow_in_season(Some(definition), &state.clock.season) {
        let name = messages::Arg::text(&definition.name);
        let season = messages::season_noun(&state.clock.season);
        let text = if choice.seed_item_id.is_some() {
            messages::CANT_GROW_IN_SEASON.with_args(vec![
                name,
                season,
                messages::Arg::text(definition.seasons.join(", ")),
            ])
        } else {
            messages::CANNOT_GROW_IN_SEASON.with_args(vec![name, season])
        };
        return vec![Effect::say(message_levels::ERROR, text)];
    }

    // What the crop needs where it is planted (M1): its terrains and scene tags.
    if let Some(needs) = definition.requirements.as_ref() {
        let on_terrain =
            scene.tile(x, y).is_some_and(|tile| needs.terrains.is_empty() || needs.terrains.contains(&tile.background));
        if !on_terrain || !needs.scene_tags.iter().all(|tag| scene.has_tag(tag)) {
            return vec![Effect::say(message_levels::ERROR, messages::CANT_PLANT_HERE.with(&[&definition.name]))];
        }
    }

    let fertilizer_item_id = match choose_fertilizer(ctx, state, choice) {
        Ok(fertilizer_item_id) => fertilizer_item_id,
        Err(refused) => return refused,
    };
    if let Some(needs) = definition.requirements.as_ref().filter(|needs| !needs.fertilizers.is_empty()) {
        let fits = match (choice.chosen, &fertilizer_item_id) {
            (true, Some(chosen)) => allowed_fertilizer(definition, chosen),
            (true, None) => false,
            // Plain interact picks an allowed one below.
            (false, _) => state
                .player
                .inventory
                .iter()
                .any(|slot| is_fertilizer(ctx, &slot.item) && allowed_fertilizer(definition, &slot.item.id)),
        };
        if !fits {
            let names: Vec<String> = needs.fertilizers.iter().map(|id| item_name(ctx, id)).collect();
            return vec![Effect::say(
                message_levels::ERROR,
                messages::NEEDS_FERTILIZER.with(&[&definition.name, &names.join(" / ")]),
            )];
        }
    }

    if let Some(multi_tile) = &definition.multi_tile {
        let can_place =
            crops::can_place_multi_tile_crop(&scene.tiles, x, y, multi_tile.width, multi_tile.height, &|terrain| {
                ctx.plantable(terrain)
            });
        if !can_place {
            return vec![Effect::say(message_levels::ERROR, &messages::NO_SPACE_FOR_CROP)];
        }
    }

    let crop_type = crop_type.to_owned();
    let mut inventory = inventory::remove_item(&state.player.inventory, &seed_item_id, 1);
    // Plain interact keeps the legacy automatic pick: the first fertilizer held.
    let fertilizer_item_id = if choice.chosen {
        fertilizer_item_id
    } else {
        inventory
            .iter()
            .find(|slot| is_fertilizer(ctx, &slot.item) && allowed_fertilizer(definition, &slot.item.id))
            .map(|slot| slot.item.id.clone())
    };
    let used_fertilizer = fertilizer_item_id.is_some();
    if let Some(fertilizer_item_id) = fertilizer_item_id {
        inventory = inventory::remove_item(&inventory, &fertilizer_item_id, 1);
    }

    let Some(index) = scene_index(state, scene_id) else {
        return Vec::new();
    };
    let quality = crops::planted_quality(ctx.crop_qualities(), used_fertilizer);
    place_crop(state, index, (x, y), definition, &crop_type, (used_fertilizer, quality));
    state.player.inventory = inventory;

    let planted = if used_fertilizer { &messages::PLANTED_FERTILIZED } else { &messages::PLANTED };
    vec![Effect::say(message_levels::SUCCESS, planted.with(&[&definition.name]))]
}

/// The seed to plant: the chosen one (refused when not held), or for `interactWith` the first
/// seed held that grows this season; else (and for plain `interact`, as recorded games expect)
/// the first seed, whose season message explains why nothing is planted.
fn choose_seed<'a>(ctx: &EngineContext, state: &'a GameState, choice: PlantChoice<'_>) -> Result<&'a Item, Effects> {
    if let Some(seed_item_id) = choice.seed_item_id {
        return held_of_kind(state, seed_item_id, |item| is_seed(ctx, item)).ok_or_else(|| {
            vec![Effect::say(message_levels::INFO, messages::NO_SEED_TO_PLANT.with(&[&item_name(ctx, seed_item_id)]))]
        });
    }
    let seeds: Vec<&Item> =
        state.player.inventory.iter().filter(|slot| is_seed(ctx, &slot.item)).map(|slot| &slot.item).collect();
    let grows_now = |item: &&&Item| {
        let definition = item.crop_type.as_deref().and_then(|crop| ctx.content.crops.get(crop));
        definition.is_some_and(|definition| crops::can_grow_in_season(Some(definition), &state.clock.season))
    };
    let in_season = if choice.chosen { seeds.iter().find(grows_now) } else { None };
    in_season
        .or_else(|| seeds.first())
        .copied()
        .ok_or_else(|| vec![Effect::say(message_levels::INFO, &messages::NO_SEEDS)])
}

/// The fertilizer the player chose (refused when not held); `None` when none was chosen or for
/// plain `interact`, which picks its own after the seed is taken.
fn choose_fertilizer(
    ctx: &EngineContext,
    state: &GameState,
    choice: PlantChoice<'_>,
) -> Result<Option<String>, Effects> {
    let (true, Some(fertilizer_item_id)) = (choice.chosen, choice.fertilizer_item_id) else {
        return Ok(None);
    };
    match held_of_kind(state, fertilizer_item_id, |item| is_fertilizer(ctx, item)) {
        Some(item) => Ok(Some(item.id.clone())),
        None => Err(vec![Effect::say(
            message_levels::INFO,
            messages::NO_FERTILIZER.with(&[&item_name(ctx, fertilizer_item_id)]),
        )]),
    }
}

/// Put a new crop of `crop_type` on (x, y) and, for a multi-tile crop, on every tile it covers
/// (the placement check kept them inside the scene); fertilizer feeds the soil under the whole
/// crop.
fn place_crop(
    state: &mut GameState,
    index: usize,
    (x, y): (i32, i32),
    definition: &CropDefinition,
    crop_type: &str,
    (used_fertilizer, quality): (bool, &str),
) {
    let new_crop = crops::create_planted_crop(crop_type, state.clock.day, quality);
    let multi_tile_id = format!("{}-{}-{}-{}", crop_type, state.clock.tick, x, y);
    let scene = &mut state.world.scenes[index];

    let (width, height) = definition.multi_tile.as_ref().map_or((1, 1), |size| (size.width.max(1), size.height.max(1)));
    for crop_dy in 0..height {
        for crop_dx in 0..width {
            let tx = x.saturating_add(i32::try_from(crop_dx).unwrap_or(i32::MAX));
            let ty = y.saturating_add(i32::try_from(crop_dy).unwrap_or(i32::MAX));
            let Some(tile) = scene.tile_mut(tx, ty) else { continue };
            tile.crop = Some(if definition.multi_tile.is_some() {
                Crop {
                    is_multi_tile_root: Some(crop_dy == 0 && crop_dx == 0),
                    multi_tile_id: Some(multi_tile_id.clone()),
                    ..new_crop.clone()
                }
            } else {
                new_crop.clone()
            });
            if used_fertilizer {
                tile.soil_fertility = 100;
                tile.soil_state = Some(soil_states::FERTILIZED.to_owned());
            }
        }
    }
}

/// Shared fixtures for the ported engine-level tests (`EngineTests.MakeProject` and the
/// `CoreTestHelpers` used by `M2SystemsTests`).
#[cfg(test)]
pub(crate) mod test_support {
    use crate::content_builtin;
    use crate::effects::Effect;
    use crate::engine_types::EngineContext;
    use crate::schema::{
        default_project_settings, Dialogue, DialogueOption, GameProject, GameState, InventorySlot, Item, MineConfig,
        Npc, Player, Quest, QuestObjective, QuestRewards, ShopDefinition, ShopStockEntry, WeatherConfig,
        WeatherTableEntry, WeatherTypeDefinition,
    };
    use crate::state;
    use crate::units;
    use crate::world::tiles;
    use indexmap::IndexMap;

    /// TS `effects.some(e => e.type === 'message' && predicate(e.text))`.
    pub(crate) fn has_message(effects: &[Effect], predicate: impl Fn(&str) -> bool) -> bool {
        effects.iter().any(|effect| matches!(effect, Effect::Message { text, .. } if predicate(text)))
    }

    pub(crate) fn slot(items: &[Item], id: &str, quantity: u32) -> InventorySlot {
        let item = items.iter().find(|item| item.id == id).unwrap_or_else(|| panic!("item {id}")).clone();
        InventorySlot::new(item, quantity)
    }

    /// Minimal test project: 6x6 open field with soil at (3,2), npc at (1,1).
    pub(crate) fn make_project() -> GameProject {
        let mut scene = tiles::create_empty_scene("scene-test", "Test Farm", 6, 6);
        scene.tiles[2][3] = tiles::set_tile_layer(&scene.tiles[2][3], "soil", None);
        scene.tiles[4][4] = tiles::set_tile_layer(&scene.tiles[4][4], "wall", None);

        let items = content_builtin::create_default_items();
        let npc = Npc {
            id: "npc-test".to_owned(),
            name: "Testy".to_owned(),
            x: units::tiles(1),
            y: units::tiles(1),
            scene_id: "scene-test".to_owned(),
            dialogue: vec![
                Dialogue {
                    id: "dlg-1".to_owned(),
                    npc_id: "npc-test".to_owned(),
                    text: "Hello!".to_owned(),
                    options: vec![
                        DialogueOption { text: "Bye".to_owned(), ..DialogueOption::default() },
                        DialogueOption {
                            text: "Gift me".to_owned(),
                            give_money: Some(25),
                            next_dialogue_id: Some("dlg-2".to_owned()),
                            ..DialogueOption::default()
                        },
                    ],
                    ..Dialogue::default()
                },
                Dialogue {
                    id: "dlg-2".to_owned(),
                    npc_id: "npc-test".to_owned(),
                    text: "More?".to_owned(),
                    options: vec![DialogueOption { text: "No".to_owned(), ..DialogueOption::default() }],
                    ..Dialogue::default()
                },
            ],
            can_move: false,
            appearance: "farmer".to_owned(),
            ..Npc::default()
        };

        let quest = Quest {
            id: "quest-wheat".to_owned(),
            name: "Wheat!".to_owned(),
            description: "Harvest 1 wheat".to_owned(),
            status: "active".to_owned(),
            objectives: vec![QuestObjective {
                id: "obj-1".to_owned(),
                r#type: "harvest".to_owned(),
                description: "Harvest wheat".to_owned(),
                target_crop_type: Some("wheat".to_owned()),
                target_crop_quantity: Some(1),
                completed: false,
                progress: 0,
                ..QuestObjective::default()
            }],
            rewards: QuestRewards { money: Some(100), ..QuestRewards::default() },
            ..Quest::default()
        };

        let sun_only = || vec![WeatherTableEntry { weather_id: "sun".to_owned(), weight: 1 }];
        let mut weather_table = IndexMap::new();
        for season in ["spring", "summer", "fall", "winter"] {
            weather_table.insert(season.to_owned(), sun_only());
        }

        GameProject {
            schema_version: 4,
            id: "proj-test".to_owned(),
            name: "Test".to_owned(),
            version: "2".to_owned(),
            scenes: vec![scene],
            npcs: vec![npc.clone()],
            items: items.clone(),
            events: Vec::new(),
            // Same list as the NPC's dialogue (TS `dialogues: npc.dialogue`).
            dialogues: npc.dialogue.clone(),
            quests: vec![quest],
            player: Player {
                x: units::tiles(3),
                y: units::tiles(4),
                direction: "up".to_owned(),
                scene_id: "scene-test".to_owned(),
                inventory: vec![
                    slot(&items, "seed-wheat", 5),
                    slot(&items, "tool-hoe", 1),
                    slot(&items, "tool-watering-can", 1),
                ],
                max_inventory_size: 10,
                money: 100,
                active_quests: vec!["quest-wheat".to_owned()],
                completed_quests: Vec::new(),
                ..Player::default()
            },
            event_flags: IndexMap::new(),
            start_scene_id: "scene-test".to_owned(),
            mode: "play".to_owned(),
            selected_tile_type: "grass".to_owned(),
            selected_npc_id: None,
            selected_item_id: None,
            custom_assets: Vec::new(),
            current_season: "spring".to_owned(),
            current_day: 1,
            current_time_minutes: 6 * 60,
            current_year: 1,
            shops: Vec::new(),
            node_types: Vec::new(),
            settings: default_project_settings(),
            recipes: Vec::new(),
            actions: Vec::new(),
            minigames: Vec::new(),
            machine_types: Vec::new(),
            // Sun-only table keeps sim tests weather-independent (weather has its
            // own dedicated tests).
            weather: WeatherConfig {
                types: vec![WeatherTypeDefinition {
                    id: "sun".to_owned(),
                    name: "Sunny".to_owned(),
                    ..WeatherTypeDefinition::default()
                }],
                table: weather_table,
                ..WeatherConfig::default()
            },
            animal_species: Vec::new(),
            animals: Vec::new(),
            fish_tables: Vec::new(),
            mine: MineConfig { enabled: false, ..MineConfig::default() },
            content_packs: Vec::new(),
            game_start_time: 1_000_000.0,
            ..GameProject::default()
        }
    }

    pub(crate) fn make_engine_with(seed: &str, project: GameProject) -> (EngineContext, GameState) {
        let ctx = EngineContext::new(state::create_content_from_project(&project));
        let state = state::create_game_state(&project, Some(seed));
        (ctx, state)
    }

    /// `M2SystemsTests.MakeEngine`: the test project plus a test shop, seed "m2".
    pub(crate) fn make_m2_engine(mutate: impl FnOnce(&mut GameProject)) -> (EngineContext, GameState) {
        let mut project = make_project();
        project.shops = vec![ShopDefinition {
            id: "shop-test".to_owned(),
            name: "Test Shop".to_owned(),
            stock: vec![
                ShopStockEntry { item_id: "seed-wheat".to_owned(), ..ShopStockEntry::default() },
                ShopStockEntry {
                    item_id: "seed-tomato".to_owned(),
                    seasons: Some(vec!["summer".to_owned()]),
                    ..ShopStockEntry::default()
                },
                ShopStockEntry {
                    item_id: "fertilizer-quality".to_owned(),
                    daily_limit: Some(2),
                    ..ShopStockEntry::default()
                },
                ShopStockEntry { item_id: "seed-carrot".to_owned(), price: Some(3), ..ShopStockEntry::default() },
            ],
            sell_price_multiplier: 1,
            buys_items: true,
            repairs_tools: true,
            repair_cost_per_point: 500,
            ..ShopDefinition::default()
        }];
        mutate(&mut project);
        make_engine_with("m2", project)
    }

    /// `state with { Player = state.Player with { X = 3, Y = 3, Direction = "up" } }`.
    pub(crate) fn facing_soil(state: &mut GameState) {
        state.player.x = units::tiles(3);
        state.player.y = units::tiles(3);
        state.player.direction = "up".to_owned();
    }
}

/// Port of engine.test.ts (`EngineTests.cs`): the tools and planting/harvesting cases.
#[cfg(test)]
mod engine_tests {
    use super::test_support::{facing_soil, has_message, make_engine_with, make_project};
    use crate::commands::Command;
    use crate::effects::Effect;
    use crate::engine::apply_command;
    use crate::engine_types::EngineContext;
    use crate::schema::GameState;
    use crate::units;

    const NEEDS_MODULES: &str = "needs Tiles/WorldMovement/Engine/Economy/GameTime/… ports — enable at integration";

    fn make_engine() -> (EngineContext, GameState) {
        make_engine_with("engine-test", make_project())
    }

    fn use_tool(tool: &str) -> Command {
        Command::UseTool { tool: tool.to_owned() }
    }

    /// Water the facing tile then sleep — one full watered day for a crop.
    fn water_and_sleep(ctx: &EngineContext, state: &mut GameState, days: usize) {
        for _ in 0..days {
            apply_command(ctx, state, &use_tool("watering-can"));
            apply_command(ctx, state, &Command::Sleep);
        }
    }

    // --- tools ---

    #[test]
    fn tills_grass_into_soil_consuming_durability_and_energy() {
        let _ = NEEDS_MODULES;
        let (ctx, mut state) = make_engine();
        // player at (3,4) facing up → target (3,3) is grass
        let effects = apply_command(&ctx, &mut state, &use_tool("hoe"));
        let tile = &state.world.scenes[0].tiles[3][3];
        assert_eq!(tile.background, "soil");
        assert_eq!(tile.soil_state.as_deref(), Some("dry"));
        assert!(effects.contains(&Effect::message("success", "Tilled soil!")));
        let hoe = state.player.inventory.iter().find(|s| s.item.tool_type.as_deref() == Some("hoe")).unwrap();
        assert_eq!(hoe.item.durability, Some(99));
        assert_eq!(state.player.energy, units::points(96)); // hoe costs 4
    }

    #[test]
    fn waters_soil_and_marks_crops_watered() {
        let (ctx, mut state) = make_engine();
        facing_soil(&mut state);
        apply_command(&ctx, &mut state, &use_tool("watering-can"));
        let tile = &state.world.scenes[0].tiles[2][3];
        assert_eq!(tile.soil_moisture, 100);
        assert_eq!(tile.soil_state.as_deref(), Some("watered"));
        assert_eq!(state.player.energy, units::points(98)); // watering can costs 2
    }

    #[test]
    fn reports_missing_tools() {
        let (ctx, mut state) = make_engine();
        state.player.inventory = Vec::new();
        let before = state.clone();
        let effects = apply_command(&ctx, &mut state, &use_tool("watering-can"));
        assert!(effects.contains(&Effect::message("error", "You need a watering can!")));
        assert_eq!(state, before);
    }

    // --- planting and harvesting (day-based) ---

    #[test]
    fn plants_a_seed_on_soil_consuming_it_crop_starts_unwatered() {
        let (ctx, mut state) = make_engine();
        facing_soil(&mut state);
        apply_command(&ctx, &mut state, &Command::Interact);
        let tile = &state.world.scenes[0].tiles[2][3];
        let crop = tile.crop.as_ref().unwrap();
        assert_eq!(crop.r#type, "wheat");
        assert!(!crop.watered);
        assert_eq!(crop.days_grown, Some(0));
        assert_eq!(crop.planted_on_day, Some(1));
        let seeds = state.player.inventory.iter().find(|s| s.item.id == "seed-wheat").unwrap();
        assert_eq!(seeds.quantity, 4);
    }

    #[test]
    fn refuses_out_of_season_crops() {
        let (ctx, mut state) = make_engine();
        state.clock.season = "summer".to_owned();
        facing_soil(&mut state);
        let effects = apply_command(&ctx, &mut state, &Command::Interact);
        assert!(state.world.scenes[0].tiles[2][3].crop.is_none());
        assert!(has_message(&effects, |t| t.contains("cannot grow in summer")));
    }

    #[test]
    fn crops_mature_after_growth_days_watered_days_harvest_yields_items_but_no_auto_money() {
        let (ctx, mut state) = make_engine();
        facing_soil(&mut state);
        apply_command(&ctx, &mut state, &Command::Interact); // plant wheat (3 growth days)
        water_and_sleep(&ctx, &mut state, 3);

        assert_eq!(state.world.scenes[0].tiles[2][3].crop.as_ref().and_then(|c| c.days_grown), Some(3));
        assert_eq!(state.clock.day, 4);

        let money_before_harvest = state.player.money;
        let effects = apply_command(&ctx, &mut state, &Command::Interact);
        assert!(state.world.scenes[0].tiles[2][3].crop.is_none());
        let wheat = state.player.inventory.iter().find(|s| s.item.id == "crop-wheat").expect("harvested wheat");
        assert!(wheat.quantity >= 1);
        // No auto-sell: money only moves via the quest reward (100).
        assert_eq!(state.player.money, money_before_harvest + 100);
        assert!(state.player.completed_quests.iter().any(|q| q == "quest-wheat"));
        assert_eq!(state.quests["quest-wheat"].status, "completed");
        assert!(effects.iter().any(|e| matches!(e, Effect::QuestCompleted { .. })));
    }

    #[test]
    fn unwatered_crops_do_not_grow_overnight() {
        let (ctx, mut state) = make_engine();
        facing_soil(&mut state);
        apply_command(&ctx, &mut state, &Command::Interact);
        apply_command(&ctx, &mut state, &Command::Sleep); // no watering
        let crop = state.world.scenes[0].tiles[2][3].crop.as_ref().unwrap();
        assert_eq!(crop.days_grown, Some(0));
        assert_eq!(crop.days_without_water, 1);
    }

    #[test]
    fn is_not_harvestable_before_maturity() {
        let (ctx, mut state) = make_engine();
        facing_soil(&mut state);
        apply_command(&ctx, &mut state, &Command::Interact);
        let effects = apply_command(&ctx, &mut state, &Command::Interact);
        assert!(effects.contains(&Effect::message("info", "Crop is not ready to harvest yet")));
        assert!(state.world.scenes[0].tiles[2][3].crop.is_some());
    }
}

/// Port of m2-systems.test.ts (`M2SystemsTests.cs`): the nightly regrowth case.
#[cfg(test)]
mod m2_systems_tests {
    use super::test_support::{make_m2_engine, slot};
    use crate::commands::Command;
    use crate::engine::apply_command;
    use crate::units;

    #[test]
    fn regrowing_crops_reset_to_a_partial_growth_state_on_harvest() {
        let (ctx, mut state) = make_m2_engine(|project| {
            let items = project.items.clone();
            project.player.inventory = vec![slot(&items, "seed-tomato", 1), slot(&items, "tool-watering-can", 1)];
        });
        // Summer for tomatoes (4 growth days, 2 regrowth days). Day 29 keeps
        // the season stable across the sleeps below.
        state.clock.season = "summer".to_owned();
        state.clock.day = 29;
        state.player.x = units::tiles(3);
        state.player.y = units::tiles(3);
        state.player.direction = "up".to_owned();
        apply_command(&ctx, &mut state, &Command::Interact);
        for _ in 0..4 {
            apply_command(&ctx, &mut state, &Command::UseTool { tool: "watering-can".to_owned() });
            apply_command(&ctx, &mut state, &Command::Sleep);
        }
        apply_command(&ctx, &mut state, &Command::Interact);
        let crop = state.world.scenes[0].tiles[2][3].crop.as_ref().expect("regrown crop");
        assert_eq!(crop.harvest_count, 1);
        assert_eq!(crop.days_grown, Some(2)); // growthDays 4 - regrowthDays 2
    }
}
