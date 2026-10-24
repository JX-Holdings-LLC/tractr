//! The rule-derived data the play overlays read, in one batched query: the open dialogue and its
//! visible options, the open shop with today's stock remainders, recipe availability and
//! ingredients, and the tile the player faces.
//!
//! Every in-game UI (the Rust `farm-ui`, the editor's C# overlays through `farm-ffi`) asks this
//! module instead of re-deriving a rule. The UI never decides whether an option is visible, a
//! purchase allowed or a recipe craftable; it shows what this says and runs engine commands.
//! Nothing here changes state.

use crate::crafting::{self, CraftableStatus};
use crate::engine_types::EngineContext;
use crate::schema::{Dialogue, DialogueOption, GameState, InventorySlot, RecipeIngredient, ShopDefinition, Tile};
use crate::world::world_movement;
use crate::{buildings, dialogue_system, economy, social, substitution};
use serde::Serialize;
use std::borrow::Cow;
use std::collections::BTreeMap;

/// A tile coordinate (the facing tile). May lie outside the scene.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct FacingTile {
    pub x: i32,
    pub y: i32,
}

/// One read of everything the overlays need. The JSON form (camelCase) is what the editor's
/// overlays deserialize (`fe_session_overlay_json`). A `null` stock remainder means unlimited:
/// JSON cannot carry positive infinity.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayView<'a> {
    pub dialogue: Option<&'a Dialogue>,
    /// What the open conversation says: today's line of the NPC (M4) or the dialogue's text,
    /// with its placeholders filled in (see [`crate::substitution`]).
    pub dialogue_text: Option<String>,
    /// The portrait expression of the line or dialogue (M4).
    pub dialogue_expression: Option<String>,
    /// The options the player may pick, in order: `chooseDialogueOption` indexes this list.
    /// Their texts have their placeholders filled in.
    pub visible_dialogue_options: Vec<DialogueOption>,
    /// The open shop as it is today (stock events applied, M3).
    pub shop: Option<Cow<'a, ShopDefinition>>,
    /// Stock left today per item id of the open shop (`None`: unlimited).
    pub stock_remaining: BTreeMap<String, Option<u32>>,
    pub facing: FacingTile,
    /// Hand and machine recipes by id.
    pub craftable: BTreeMap<&'a str, CraftableStatus>,
    /// Whether the inventory holds every input of a recipe (machine loading).
    pub has_ingredients: BTreeMap<&'a str, bool>,
    /// The open chest or shipping bin (M5).
    pub storage: Option<StorageView>,
    /// What the open shop builds (M5): new buildings, and upgrades of standing ones.
    pub building_options: Vec<BuildOption>,
    /// The animals the open shop sells and the buildings they may go to (M5).
    pub animal_options: Vec<AnimalOption>,
    /// Letters not read yet (M6).
    pub unread_mail: usize,
}

/// The open chest or shipping bin (M5).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageView {
    pub name: String,
    pub slots: u32,
    /// A shipping bin: what it holds is sold overnight.
    pub shipping: bool,
    pub contents: Vec<InventorySlot>,
}

/// A building the open shop builds, or an upgrade of a standing building (M5).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildOption {
    pub building_id: String,
    pub name: String,
    pub description: String,
    pub cost: i64,
    pub materials: Vec<RecipeIngredient>,
    pub width: i32,
    pub height: i32,
    /// The player has the money and the materials.
    pub affordable: bool,
    /// The building on the map it upgrades (`upgradeBuilding`); absent: a new building
    /// (`orderBuilding`) placed in one of `scenes`.
    pub upgrades: Option<String>,
    /// The scenes it may be built in.
    pub scenes: Vec<String>,
    pub build_days: u32,
}

/// An animal the open shop sells (M5).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimalOption {
    pub species_id: String,
    pub name: String,
    pub cost: i64,
    /// Standing buildings that house it with room: (building on the map, its name, animals in
    /// it, its capacity).
    pub homes: Vec<(String, String, u32, u32)>,
}

/// Builds the overlay view for `state`.
pub fn overlay_view<'a>(ctx: &'a EngineContext, state: &GameState) -> OverlayView<'a> {
    let dialogue = state
        .dialogue
        .as_ref()
        .and_then(|active| dialogue_system::find_dialogue(ctx, &active.npc_id, &active.dialogue_id));
    let mut visible_dialogue_options =
        dialogue.map(|dialogue| social::visible_dialogue_options(ctx, state, dialogue)).unwrap_or_default();
    for option in &mut visible_dialogue_options {
        option.text = substitution::fill(&ctx.content, state, &option.text).into_owned();
    }
    let line = state.dialogue.as_ref().and_then(|active| {
        let index = usize::try_from(active.line?).ok()?;
        ctx.npc(&active.npc_id)?.lines.as_ref()?.get(index)
    });
    let (dialogue_text, dialogue_expression) = match (line, dialogue) {
        (Some(line), _) => (Some(line.text.as_str()), line.expression.clone()),
        (None, Some(dialogue)) => (Some(dialogue.text.as_str()), dialogue.expression.clone()),
        (None, None) => (None, None),
    };
    let dialogue_text = dialogue_text.map(|text| substitution::fill(&ctx.content, state, text).into_owned());
    let shop = state.shop.as_ref().and_then(|active| economy::find_shop(ctx, state, &active.shop_id));
    let mut stock_remaining = BTreeMap::new();
    if let Some(shop) = &shop {
        for entry in &shop.stock {
            let remaining = economy::remaining_daily_stock(state, &shop.id, &entry.item_id, entry.daily_limit);
            stock_remaining.insert(entry.item_id.clone(), remaining);
        }
    }
    let facing = world_movement::facing_target(state);
    let mut craftable = BTreeMap::new();
    let mut has_ingredients = BTreeMap::new();
    for recipe in &ctx.content.recipes {
        craftable.insert(recipe.id.as_str(), crafting::craftable_status(ctx, state, recipe));
        has_ingredients.insert(recipe.id.as_str(), crafting::has_ingredients(state, recipe));
    }
    let (building_options, animal_options) = match &shop {
        Some(shop) => (build_options(ctx, state, shop), animal_options(ctx, state, shop)),
        None => (Vec::new(), Vec::new()),
    };
    let storage = crate::objects::open_storage(ctx, state).map(|(machine, definition)| StorageView {
        name: definition.name.clone(),
        slots: definition.storage_slots(),
        shipping: definition.shipping,
        contents: machine.contents.clone(),
    });
    OverlayView {
        dialogue,
        dialogue_text,
        dialogue_expression,
        visible_dialogue_options,
        shop,
        stock_remaining,
        facing: FacingTile { x: facing.x, y: facing.y },
        craftable,
        has_ingredients,
        storage,
        building_options,
        animal_options,
        unread_mail: crate::mail::unread(state),
    }
}

fn build_options(ctx: &EngineContext, state: &GameState, shop: &ShopDefinition) -> Vec<BuildOption> {
    let mut options = Vec::new();
    for id in &shop.buildings {
        let Some(definition) = buildings::definition(ctx, id) else { continue };
        let affordable = state.player.money >= definition.cost
            && definition
                .materials
                .iter()
                .all(|m| crate::inventory::count_item(&state.player.inventory, &m.item_id) >= u64::from(m.quantity));
        let (width, height) = definition.size();
        let option = |upgrades: Option<String>| BuildOption {
            building_id: definition.id.clone(),
            name: definition.name.clone(),
            description: definition.description.clone(),
            cost: definition.cost,
            materials: definition.materials.clone(),
            width,
            height,
            affordable,
            upgrades,
            scenes: buildings::allowed_scenes(ctx, definition).into_iter().map(str::to_owned).collect(),
            build_days: definition.build_days.max(1),
        };
        match &definition.upgrade_of {
            None => options.push(option(None)),
            Some(from) => {
                for building in state.buildings.iter().filter(|b| &b.building_id == from && b.days_left == 0) {
                    options.push(option(Some(building.id.clone())));
                }
            }
        }
    }
    options
}

fn animal_options(ctx: &EngineContext, state: &GameState, shop: &ShopDefinition) -> Vec<AnimalOption> {
    shop.animals
        .iter()
        .filter_map(|id| ctx.animal_species(id))
        .map(|species| AnimalOption {
            species_id: species.id.clone(),
            name: species.name.clone(),
            cost: species.purchase_cost,
            homes: state
                .buildings
                .iter()
                .filter(|building| buildings::houses(ctx, species, building))
                .map(|building| {
                    let name = buildings::definition(ctx, &building.building_id)
                        .map_or(building.id.clone(), |definition| definition.name.clone());
                    (
                        building.id.clone(),
                        name,
                        buildings::occupants(state, &building.id),
                        buildings::capacity(ctx, building),
                    )
                })
                .collect(),
        })
        .collect()
}

/// Why building `building_id` can't go with its top-left tile on `(x, y)` of `scene_id` (the
/// build menu's placement preview), or `None` when it can.
pub fn site_status(
    ctx: &EngineContext,
    state: &GameState,
    building_id: &str,
    scene_id: &str,
    x: i32,
    y: i32,
) -> Option<crate::messages::Message> {
    let definition = buildings::definition(ctx, building_id)?;
    buildings::site_refusal(ctx, state, definition, scene_id, x, y)
}

impl OverlayView<'_> {
    /// Units of `item_id` the open shop still sells today (`None` when unlimited or unknown).
    pub fn remaining(&self, item_id: &str) -> Option<u32> {
        self.stock_remaining.get(item_id).copied().flatten()
    }

    /// Whether a recipe can be crafted now, with the reason when not (unknown recipes are not).
    pub fn status(&self, recipe_id: &str) -> CraftableStatus {
        self.craftable.get(recipe_id).cloned().unwrap_or_default()
    }

    /// Whether the inventory holds every input of a recipe.
    pub fn ingredients(&self, recipe_id: &str) -> bool {
        self.has_ingredients.get(recipe_id).copied().unwrap_or(false)
    }

    /// The tile the player faces in the current scene, when it lies inside it.
    pub fn facing_tile<'s>(&self, state: &'s GameState) -> Option<&'s Tile> {
        let scene = state.world.scenes.iter().find(|scene| scene.id == state.player.scene_id)?;
        let (x, y) = (self.facing.x, self.facing.y);
        let (x, y) = (usize::try_from(x).ok()?, usize::try_from(y).ok()?);
        scene.tiles.get(y)?.get(x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{DialogueState, GameProject, ShopSession};

    fn starter() -> GameProject {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/golden/content/starter-farm.json")).unwrap();
        serde_json::from_value(fixture["project"].clone()).unwrap()
    }

    #[test]
    fn reads_dialogue_shop_recipes_and_the_facing_tile() {
        let project = starter();
        let ctx = EngineContext::new(crate::create_content_from_project(&project));
        let mut state = crate::create_game_state(&project, Some("overlay"));
        let npc = ctx.content.npcs.iter().find(|npc| !npc.dialogue.is_empty()).unwrap();
        state.dialogue =
            Some(DialogueState { npc_id: npc.id.clone(), dialogue_id: npc.dialogue[0].id.clone(), line: None });
        let shop = ctx.content.shops[0].clone();
        state.shop = Some(ShopSession { shop_id: shop.id.clone() });
        let view = overlay_view(&ctx, &state);
        assert_eq!(view.dialogue.map(|d| d.id.as_str()), Some(npc.dialogue[0].id.as_str()));
        assert_eq!(view.visible_dialogue_options, social::visible_dialogue_options(&ctx, &state, &npc.dialogue[0]));
        assert_eq!(view.shop.as_ref().map(|s| s.id.as_str()), Some(shop.id.as_str()));
        assert_eq!(view.stock_remaining.len(), shop.stock.len());
        assert_eq!(view.craftable.len(), ctx.content.recipes.len());
        assert_eq!(view.remaining("no-such-item"), None);
        assert!(!view.status("no-such-recipe").craftable);
        assert!(!view.ingredients("no-such-recipe"));
        let target = world_movement::facing_target(&state);
        assert_eq!((view.facing.x, view.facing.y), (target.x, target.y));
        let tile = view.facing_tile(&state).unwrap();
        assert_eq!((tile.x, tile.y), (target.x, target.y));
        let json = serde_json::to_value(&view).unwrap();
        assert!(json.get("visibleDialogueOptions").is_some() && json.get("hasIngredients").is_some());
    }
}
