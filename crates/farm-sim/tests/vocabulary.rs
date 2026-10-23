//! The world vocabulary as content (M1): a game's own terrains, tools, item kinds, crop tiers
//! and growth requirements, hazards and the length of a day.

mod common;

use common::{has_message, make_engine, set_tile_layer};
use farm_sim::schema::{
    tile_layers, tool_actions, CropQualityDefinition, CustomCropDefinition, GameProject, GrowthRequirements,
    InventorySlot, Item, ItemCategoryDefinition, TerrainDefinition, TerrainTransform, ToolArea, ToolDefinition,
};
use farm_sim::{apply_command, content_builtin, events, units, Command, CommandRules, EngineContext, GameState};

const FACED: (usize, usize) = (3, 3);

fn terrain(id: &str, layer: &str) -> TerrainDefinition {
    TerrainDefinition {
        id: id.to_owned(),
        name: id.to_owned(),
        layer: layer.to_owned(),
        color: "#808080".to_owned(),
        ..TerrainDefinition::default()
    }
}

fn tool(id: &str, action: &str) -> ToolDefinition {
    ToolDefinition {
        id: id.to_owned(),
        name: id.to_owned(),
        action: action.to_owned(),
        energy_cost: units::points(2),
        area: vec![ToolArea::default()],
        ..ToolDefinition::default()
    }
}

fn tool_item(tool_type: &str) -> Item {
    Item {
        id: format!("item-{tool_type}"),
        name: tool_type.to_owned(),
        r#type: "tool".to_owned(),
        max_stack: 1,
        tool_type: Some(tool_type.to_owned()),
        ..Item::default()
    }
}

fn give(project: &mut GameProject, item: Item, quantity: u32) {
    project.items.push(item.clone());
    project.player.inventory.push(InventorySlot::new(item, quantity));
}

fn paint(project: &mut GameProject, (x, y): (usize, usize), terrain: &str) {
    let tile = &project.scenes[0].tiles[y][x];
    project.scenes[0].tiles[y][x] = set_tile_layer(tile, terrain);
}

fn engine(mutate: impl FnOnce(&mut GameProject)) -> (EngineContext, GameState) {
    let (ctx, state) = make_engine(mutate, "vocabulary");
    (ctx.with_rules(CommandRules::Scripted), state)
}

fn faced(state: &GameState) -> &farm_sim::schema::Tile {
    &state.world.scenes[0].tiles[FACED.1][FACED.0]
}

#[test]
fn a_game_tool_transforms_the_object_its_terrain_names() {
    let (ctx, mut state) = engine(|project| {
        let mut rock = terrain("rock-wall", tile_layers::OBJECT);
        rock.walkable = false;
        rock.transforms = vec![TerrainTransform {
            action: tool_actions::TRANSFORM.to_owned(),
            tool: Some("plasma-cutter".to_owned()),
            into: String::new(),
        }];
        project.terrains = Some(vec![rock]);
        project.tools = Some(vec![tool("plasma-cutter", tool_actions::TRANSFORM)]);
        give(project, tool_item("plasma-cutter"), 1);
        let tile = &mut project.scenes[0].tiles[FACED.1][FACED.0];
        tile.object = Some("rock-wall".to_owned());
        tile.r#type = "rock-wall".to_owned();
        tile.collision = true;
    });
    let energy = state.player.energy;
    let effects = apply_command(&ctx, &mut state, &Command::UseTool { tool: "plasma-cutter".to_owned() });
    assert!(has_message(&effects, |text| text.contains("Cleared")), "{effects:?}");
    assert_eq!(faced(&state).object, None);
    assert!(!faced(&state).collision);
    assert_eq!(state.player.energy, energy - units::points(2));
}

#[test]
fn tilling_follows_the_terrain_transforms_and_any_till_tool() {
    let (ctx, mut state) = engine(|project| {
        let mut regolith = terrain("regolith", tile_layers::BACKGROUND);
        regolith.transforms = vec![TerrainTransform {
            action: tool_actions::TILL.to_owned(),
            tool: None,
            into: "tilled-regolith".to_owned(),
        }];
        let tilled = TerrainDefinition { plantable: true, ..terrain("tilled-regolith", tile_layers::BACKGROUND) };
        project.terrains = Some(vec![regolith, tilled]);
        paint(project, FACED, "regolith");
    });
    let effects = apply_command(&ctx, &mut state, &Command::UseTool { tool: "hoe".to_owned() });
    assert!(has_message(&effects, |text| text == "Tilled soil!"), "{effects:?}");
    assert_eq!(faced(&state).background, "tilled-regolith");
    assert_eq!(faced(&state).soil_state.as_deref(), Some("dry"));
    // Planting works on any plantable terrain.
    let effects = apply_command(&ctx, &mut state, &Command::Interact);
    assert!(has_message(&effects, |text| text.starts_with("Planted Wheat")), "{effects:?}");
}

#[test]
fn a_place_tool_puts_its_terrain_down_and_a_wider_area_reaches_the_neighbours() {
    let (ctx, mut state) = engine(|project| {
        let mut terraformer = tool("terraformer", tool_actions::PLACE);
        terraformer.place_terrain = Some("water".to_owned());
        terraformer.area = vec![ToolArea { width: 3, length: 1 }];
        project.tools = Some(vec![terraformer]);
        give(project, tool_item("terraformer"), 1);
    });
    apply_command(&ctx, &mut state, &Command::UseTool { tool: "terraformer".to_owned() });
    for x in 2..=4 {
        assert_eq!(state.world.scenes[0].tiles[3][x].background, "water", "x={x}");
    }
    // The tile beside the row stays.
    assert_eq!(state.world.scenes[0].tiles[3][1].background, "grass");
}

#[test]
fn edible_kinds_are_eaten_for_energy() {
    let (ctx, mut state) = engine(|project| {
        project.item_categories = Some(vec![ItemCategoryDefinition {
            id: "food".to_owned(),
            name: "Food".to_owned(),
            edible: true,
            energy: units::points(10),
            ..ItemCategoryDefinition::default()
        }]);
        let pie = Item {
            id: "pie".to_owned(),
            name: "Pie".to_owned(),
            r#type: "food".to_owned(),
            stackable: true,
            max_stack: 9,
            ..Item::default()
        };
        let cake =
            Item { energy: Some(units::points(30)), id: "cake".to_owned(), name: "Cake".to_owned(), ..pie.clone() };
        give(project, pie, 2);
        give(project, cake, 1);
    });
    state.player.energy = units::points(50);
    let effects = apply_command(&ctx, &mut state, &Command::UseItem { item_id: "pie".to_owned() });
    assert!(has_message(&effects, |text| text == "Ate Pie."), "{effects:?}");
    assert_eq!(state.player.energy, units::points(60));
    apply_command(&ctx, &mut state, &Command::UseItem { item_id: "cake".to_owned() });
    assert_eq!(state.player.energy, units::points(90));
    let held = |id: &str| state.player.inventory.iter().filter(|s| s.item.id == id).map(|s| s.quantity).sum::<u32>();
    assert_eq!((held("pie"), held("cake")), (1, 0));
}

#[test]
fn tools_are_not_gifts() {
    let (ctx, mut state) = engine(|_| {});
    // Face the NPC at (1,1) from (1,2).
    state.player.x = units::tile_center(1);
    state.player.y = units::tile_center(2);
    state.player.direction = "up".to_owned();
    let effects = apply_command(&ctx, &mut state, &Command::GiveGift { item_id: "tool-hoe".to_owned() });
    assert!(has_message(&effects, |text| text == "Hoe can't be given as a gift."), "{effects:?}");
    assert!(state.player.inventory.iter().any(|s| s.item.id == "tool-hoe"));
}

fn crop_with(needs: GrowthRequirements) -> CustomCropDefinition {
    let wheat = &content_builtin::crop_definitions()["wheat"];
    CustomCropDefinition {
        id: "moonbean".to_owned(),
        name: "Moonbean".to_owned(),
        seed_cost: 1,
        base_harvest_value: 1,
        growth_days: Some(2),
        stages: 3,
        seasons: wheat.seasons.clone(),
        yield_min: 1,
        yield_max: 1,
        requirements: Some(needs),
        ..CustomCropDefinition::default()
    }
}

/// The test field's soil tile at (3,2), faced from (3,3).
fn plant_moonbean(dark: bool) -> (EngineContext, GameState) {
    let (ctx, mut state) = engine(|project| {
        project.custom_crops = Some(vec![crop_with(GrowthRequirements {
            light: Some("dark".to_owned()),
            ..GrowthRequirements::default()
        })]);
        let seed = Item {
            id: "seed-moonbean".to_owned(),
            name: "Moonbean Seeds".to_owned(),
            r#type: "seed".to_owned(),
            stackable: true,
            max_stack: 9,
            crop_type: Some("moonbean".to_owned()),
            ..Item::default()
        };
        project.player.inventory.retain(|slot| slot.item.r#type != "seed");
        give(project, seed, 1);
        project.scenes[0].dark = Some(dark);
    });
    state.player.y = units::tile_center(3);
    apply_command(&ctx, &mut state, &Command::Interact);
    (ctx, state)
}

fn grow_one_night(ctx: &EngineContext, state: &mut GameState) -> u32 {
    apply_command(ctx, state, &Command::UseTool { tool: "watering-can".to_owned() });
    apply_command(ctx, state, &Command::Sleep);
    state.world.scenes[0].tiles[2][3].crop.as_ref().and_then(|crop| crop.days_grown).unwrap_or(99)
}

#[test]
fn a_crop_that_needs_the_dark_waits_in_the_light() {
    let (ctx, mut lit) = plant_moonbean(false);
    assert!(lit.world.scenes[0].tiles[2][3].crop.is_some());
    assert_eq!(grow_one_night(&ctx, &mut lit), 0);
    let (ctx, mut dark) = plant_moonbean(true);
    assert_eq!(grow_one_night(&ctx, &mut dark), 1);
}

#[test]
fn a_crop_is_planted_only_on_its_terrains_and_with_its_fertilizer() {
    let (ctx, mut state) = engine(|project| {
        project.custom_crops = Some(vec![crop_with(GrowthRequirements {
            terrains: vec!["soil".to_owned()],
            fertilizers: vec!["fertilizer-quality".to_owned()],
            ..GrowthRequirements::default()
        })]);
        let seed = Item {
            id: "seed-moonbean".to_owned(),
            name: "Moonbean Seeds".to_owned(),
            r#type: "seed".to_owned(),
            stackable: true,
            max_stack: 9,
            crop_type: Some("moonbean".to_owned()),
            ..Item::default()
        };
        project.player.inventory.retain(|slot| slot.item.r#type != "seed");
        give(project, seed, 2);
    });
    state.player.y = units::tile_center(3);
    let effects = apply_command(&ctx, &mut state, &Command::Interact);
    assert!(has_message(&effects, |text| text == "Moonbean needs Quality Fertilizer to be planted."), "{effects:?}");
    let fertilizer = ctx.item("fertilizer-quality").unwrap().clone();
    state.player.inventory.push(InventorySlot::new(fertilizer, 1));
    let effects = apply_command(&ctx, &mut state, &Command::Interact);
    assert!(has_message(&effects, |text| text.contains("(Fertilized)")), "{effects:?}");
}

#[test]
fn hazardous_terrain_saps_energy_on_entering() {
    let (ctx, mut state) = engine(|project| {
        let lava = TerrainDefinition { hazard_energy: units::points(5), ..terrain("lava", tile_layers::BACKGROUND) };
        project.terrains = Some(vec![lava]);
        paint(project, FACED, "lava");
    });
    let energy = state.player.energy;
    let effects = apply_command(&ctx, &mut state, &Command::Move { dir: "up".to_owned() });
    assert!(has_message(&effects, |text| text == "lava saps your energy!"), "{effects:?}");
    assert_eq!(state.player.energy, energy - units::points(5));
}

#[test]
fn a_game_tool_takes_its_key_and_a_renamed_tool_its_name() {
    let (ctx, _) = engine(|project| {
        let mut hoe = tool("hoe", tool_actions::TILL);
        hoe.name = "Rake".to_owned();
        project.tools = Some(vec![hoe]);
    });
    assert_eq!(ctx.tool("hoe").map(|tool| tool.name.as_str()), Some("Rake"));
    let noun = farm_sim::messages::tool_noun_in(&ctx, "hoe");
    assert_eq!(farm_sim::messages::NEED_TOOL.with_args(vec![noun]).english(), "You need a rake!");
}

#[test]
fn quality_tiers_rank_harvests_in_list_order() {
    let tiers: Vec<CropQualityDefinition> = ["dull", "fine", "superb"]
        .iter()
        .map(|id| CropQualityDefinition { id: (*id).to_owned(), ..CropQualityDefinition::default() })
        .collect();
    use farm_sim::farming::crops;
    assert_eq!(crops::harvest_quality(&tiers, "dull", 3), "fine");
    assert_eq!(crops::harvest_quality(&tiers, "fine", 9), "superb");
    assert_eq!(crops::planted_quality(&tiers, true), "fine");
    assert_eq!(crops::planted_quality(&tiers, false), "dull");
}

#[test]
fn a_day_of_other_length_wraps_times_of_day_at_its_end() {
    let at = |minutes: u32| units::minutes(minutes);
    // A 30-hour day: 26:00 is not yet past midnight.
    assert!(!events::time_of_day_matches_in(30 * 60, at(26 * 60), at(0), at(2 * 60)));
    assert!(events::time_of_day_matches_in(30 * 60, at(31 * 60), at(0), at(2 * 60)));
    assert_eq!(farm_sim::game_time::format_time_of_day_in(30 * 60, at(27 * 60 + 15)), "27:15");
}
