//! Farm layouts (M5): a new game may pick one of several farm maps. The layout's scene takes
//! the place of the farm scene (`settings.farm.sceneId`, whose id doors and events use), and
//! the other layouts' scenes leave the world.

use crate::effects::Effect;
use crate::engine_types::{Effects, EngineContext};
use crate::schema::GameState;
use crate::units;
use crate::world::{tiles, world_movement};

/// `chooseFarmLayout` under [`crate::CommandRules::Player`]: only before the first tick of a new
/// game, like the character creator.
pub fn refusal(state: &GameState) -> Option<Effects> {
    (state.clock.tick > 0 || state.farm_layout.is_some()).then(Vec::new)
}

/// Make layout `layout_id` the farm.
pub fn handle_choose(ctx: &EngineContext, state: &mut GameState, layout_id: &str) -> Effects {
    let Some(farm) = ctx.content.settings.farm.as_ref() else { return Vec::new() };
    let Some(layout) = farm.layouts.iter().find(|layout| layout.id == layout_id) else { return Vec::new() };
    let target = farm.scene_id.as_str();
    if layout.scene_id != target && !target.is_empty() {
        let Some(mut scene) = world_movement::find_scene(state, &layout.scene_id)
            .or_else(|| ctx.content_scene(&layout.scene_id))
            .cloned()
        else {
            return Vec::new();
        };
        let Some(index) = state.world.scenes.iter().position(|existing| existing.id == target) else {
            return Vec::new();
        };
        scene.id = target.to_owned();
        scene.name.clone_from(&state.world.scenes[index].name);
        tiles::normalize_scene_grid(&mut scene);
        state.world.scenes[index] = scene;
        let others: Vec<&str> =
            farm.layouts.iter().map(|layout| layout.scene_id.as_str()).filter(|id| *id != target).collect();
        state.world.scenes.retain(|scene| !others.contains(&scene.id.as_str()));
        // Whoever stood on the old map stands where the new one lets them.
        if state.player.scene_id == target {
            let (x, y) = (units::tile_of(state.player.x), units::tile_of(state.player.y));
            if let Some(scene) = world_movement::find_scene(state, target) {
                if let Some(tile) = world_movement::landing_tile(ctx, scene, x, y) {
                    if (tile.x, tile.y) != (x, y) {
                        world_movement::put_player(
                            state,
                            target,
                            units::tile_center(tile.x),
                            units::tile_center(tile.y),
                        );
                    }
                }
            }
        }
    }
    state.farm_layout = Some(layout.id.clone());
    vec![Effect::say(crate::effects::message_levels::INFO, crate::messages::FARM_LAYOUT.with(&[&layout.name]))]
}
