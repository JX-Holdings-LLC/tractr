//! M5: building the farm — placed objects (chests, gates, sprinklers, scarecrows), buildings
//! with interiors and upgrades, animals with housing, greenhouses and farm layouts.

mod common;

use common::{empty_scene, has_message, make_engine, set_tile_layer, slot};
use farm_sim::schema::{
    AnimalBreeding, AnimalProduct, AnimalSpeciesDefinition, AreaEffect, BuildingDefinition, CropMutationDefinition,
    FarmLayout, FarmSettings, GameState, GridPoint, Item, MachineTypeDefinition, MoodQuality, RecipeIngredient,
    ShopDefinition, ShopSession, TileMachine,
};
use farm_sim::world::pathfinding;
use farm_sim::{engine, overlay, units, Command, CommandRules, Effect, EngineContext};

fn place(state: &mut GameState, x: i32, y: i32, type_id: &str) {
    state.world.scenes[0].tiles[y as usize][x as usize].machine =
        Some(TileMachine { type_id: type_id.to_owned(), ..TileMachine::default() });
}

fn machine(state: &GameState, x: i32, y: i32) -> &TileMachine {
    state.world.scenes[0].tiles[y as usize][x as usize].machine.as_ref().unwrap()
}

fn object(id: &str) -> MachineTypeDefinition {
    MachineTypeDefinition { id: id.to_owned(), name: id.to_owned(), ..MachineTypeDefinition::default() }
}

fn run(ctx: &EngineContext, state: &mut GameState, command: Command) -> Vec<Effect> {
    engine::apply_command(ctx, state, &command)
}

fn sleep(ctx: &EngineContext, state: &mut GameState) -> Vec<Effect> {
    run(ctx, state, Command::Sleep)
}

#[test]
fn a_chest_stores_and_returns_items_and_holds_its_place_until_emptied() {
    let (ctx, mut state) = make_engine(
        |project| project.machine_types.push(MachineTypeDefinition { storage: Some(2), ..object("chest") }),
        "chest",
    );
    place(&mut state, 3, 3, "chest");
    run(&ctx, &mut state, Command::Interact);
    assert!(state.storage.is_some(), "interacting opens the chest");
    assert!(engine::modal_open(&state));
    // Only the chest's own commands apply while it is open.
    run(&ctx, &mut state, Command::Sleep);
    assert_eq!(state.clock.day, 1);

    run(&ctx, &mut state, Command::StoreItem { item_id: "seed-wheat".into(), quantity: 3, quality: None });
    assert_eq!(machine(&state, 3, 3).contents.len(), 1);
    assert_eq!(machine(&state, 3, 3).contents[0].quantity, 3);
    assert_eq!(farm_sim::inventory::count_item(&state.player.inventory, "seed-wheat"), 2);
    let view = overlay::overlay_view(&ctx, &state);
    assert_eq!(view.storage.as_ref().map(|storage| storage.slots), Some(2));

    run(&ctx, &mut state, Command::TakeStoredItem { index: 0, quantity: 1 });
    assert_eq!(machine(&state, 3, 3).contents[0].quantity, 2);
    run(&ctx, &mut state, Command::CloseStorage);
    assert!(state.storage.is_none());

    let refused = run(&ctx, &mut state, Command::PickUpMachine);
    assert!(has_message(&refused, |text| text == "Empty it first."));
    run(&ctx, &mut state, Command::Interact);
    run(&ctx, &mut state, Command::TakeStoredItem { index: 0, quantity: 9 });
    run(&ctx, &mut state, Command::CloseStorage);
    assert!(machine(&state, 3, 3).contents.is_empty());
    run(&ctx, &mut state, Command::PickUpMachine);
    assert!(state.world.scenes[0].tiles[3][3].machine.is_none());
    assert_eq!(farm_sim::inventory::count_item(&state.player.inventory, "seed-wheat"), 5);
}

#[test]
fn a_gate_blocks_until_it_is_opened() {
    let (ctx, mut state) = make_engine(
        |project| project.machine_types.push(MachineTypeDefinition { gate: true, ..object("gate") }),
        "gate",
    );
    place(&mut state, 3, 3, "gate");
    let blocks = |state: &GameState| {
        pathfinding::tile_blocks(
            &state.world.scenes[0].tiles[3][3],
            &ctx.content.node_types,
            &ctx.content.machine_types,
        )
    };
    assert!(blocks(&state));
    let opened = run(&ctx, &mut state, Command::Interact);
    assert!(has_message(&opened, |text| text == "Opened the gate."));
    assert_eq!(machine(&state, 3, 3).open, Some(true));
    assert!(!blocks(&state));
    run(&ctx, &mut state, Command::Move { dir: "up".into() });
    assert_eq!(units::tile_of(state.player.y), 3, "the player walks through the open gate");
    // Standing in it, it won't close; stepping back out, it does.
    state.player.direction = "down".into();
    run(&ctx, &mut state, Command::Move { dir: "down".into() });
    state.player.direction = "up".into();
    run(&ctx, &mut state, Command::Interact);
    assert_eq!(machine(&state, 3, 3).open, None);
    assert!(blocks(&state));
}

#[test]
fn sprinklers_water_their_area_each_morning_and_scarecrows_keep_pests_off() {
    let (ctx, mut state) = make_engine(
        |project| {
            project.machine_types.push(MachineTypeDefinition {
                area_effects: vec![AreaEffect { effect: "water".into(), radius: 1 }],
                ..object("sprinkler")
            });
            project.machine_types.push(MachineTypeDefinition {
                area_effects: vec![AreaEffect { effect: "protect".into(), radius: 1 }],
                ..object("scarecrow")
            });
            project.settings.farm = Some(FarmSettings {
                pest_chance: units::from_authoring::<units::Probability>(1.0),
                ..FarmSettings::default()
            });
            // A wider field of soil.
            let scene = &mut project.scenes[0];
            for (x, y) in [(0, 0), (1, 0), (4, 0), (5, 0)] {
                scene.tiles[y][x] = set_tile_layer(&scene.tiles[y][x], "soil");
            }
        },
        "sprinkler",
    );
    let crop = |state: &mut GameState, x: usize, y: usize| {
        let crop = farm_sim::schema::Crop {
            r#type: "wheat".into(),
            planted_on_day: Some(1),
            quality: "normal".into(),
            ..farm_sim::schema::Crop::default()
        };
        state.world.scenes[0].tiles[y][x].crop = Some(crop);
    };
    crop(&mut state, 0, 0);
    crop(&mut state, 5, 0);
    place(&mut state, 1, 1, "sprinkler");
    place(&mut state, 4, 1, "scarecrow");
    sleep(&ctx, &mut state);
    let tiles = &state.world.scenes[0].tiles;
    assert!(tiles[0][0].crop.is_none(), "pests eat the crop the scarecrow does not cover");
    assert!(tiles[0][5].crop.is_some(), "the scarecrow keeps pests off");
    assert_eq!(tiles[0][1].soil_moisture, 100, "the sprinkler waters the soil around it");
    assert_eq!(tiles[0][4].soil_moisture, 0, "soil outside its area stays dry");
    let _ = CropMutationDefinition::default();
}

/// A project with a builder shop, a coop (2×2, interior "coop-inside"), its upgrade and chickens.
fn builder_project(project: &mut farm_sim::schema::GameProject) {
    let mut inside = empty_scene("coop-inside", "Coop inside", 4, 4);
    inside.indoor = Some(true);
    project.scenes.push(inside);
    let mut big = empty_scene("big-coop-inside", "Big coop inside", 6, 5);
    big.indoor = Some(true);
    project.scenes.push(big);
    project.building_types = Some(vec![
        BuildingDefinition {
            id: "coop".into(),
            name: "Coop".into(),
            width: 2,
            height: 2,
            interior_scene_id: Some("coop-inside".into()),
            cost: 50,
            materials: vec![RecipeIngredient { item_id: "seed-wheat".into(), quantity: 2 }],
            build_days: 2,
            capacity: 1,
            ..BuildingDefinition::default()
        },
        BuildingDefinition {
            id: "big-coop".into(),
            name: "Big Coop".into(),
            width: 2,
            height: 2,
            interior_scene_id: Some("big-coop-inside".into()),
            interior_door: Some(GridPoint { x: 1, y: 4 }),
            cost: 10,
            build_days: 1,
            capacity: 3,
            upgrade_of: Some("coop".into()),
            ..BuildingDefinition::default()
        },
    ]);
    project.items.push(Item {
        id: "egg".into(),
        name: "Egg".into(),
        r#type: "crop".into(),
        stackable: true,
        max_stack: 99,
        value: 5,
        ..Item::default()
    });
    project.items.push(Item {
        id: "feather".into(),
        name: "Feather".into(),
        r#type: "crop".into(),
        stackable: true,
        max_stack: 99,
        value: 2,
        ..Item::default()
    });
    project.animal_species.push(AnimalSpeciesDefinition {
        id: "chicken".into(),
        name: "Chicken".into(),
        purchase_cost: 20,
        product_item_id: "egg".into(),
        product_interval_days: 1,
        days_to_adult: 1,
        housing: vec!["coop".into()],
        products: vec![AnimalProduct { item_id: "feather".into(), interval_days: 2, min_mood: 0 }],
        mood_qualities: vec![MoodQuality { min_mood: 60, quality: "silver".into() }],
        breeding: Some(AnimalBreeding { chance: units::from_authoring::<units::Probability>(1.0), min_mood: 0 }),
        ..AnimalSpeciesDefinition::default()
    });
    project.shops.push(ShopDefinition {
        id: "builder".into(),
        name: "Builder".into(),
        buildings: vec!["coop".into(), "big-coop".into()],
        animals: vec!["chicken".into()],
        ..ShopDefinition::default()
    });
    project.player.money = 500;
    // The NPC stands clear of the site.
    project.npcs[0].x = units::tiles(5);
    project.npcs[0].y = units::tiles(5);
}

fn open_builder(state: &mut GameState) {
    state.shop = Some(ShopSession { shop_id: "builder".into() });
}

#[test]
fn a_builder_puts_up_a_building_with_an_interior_and_upgrades_it() {
    let (ctx, mut state) = make_engine(builder_project, "builder");
    open_builder(&mut state);
    let view = overlay::overlay_view(&ctx, &state);
    assert_eq!(view.building_options.len(), 1, "the upgrade needs a coop first");
    assert!(view.building_options[0].affordable);

    // Not on the player, not off the map.
    assert!(overlay::site_status(&ctx, &state, "coop", "scene-test", 3, 3).is_some());
    assert!(overlay::site_status(&ctx, &state, "coop", "scene-test", 5, 0).is_some());
    assert!(overlay::site_status(&ctx, &state, "coop", "scene-test", 0, 0).is_none());
    let ordered = run(
        &ctx,
        &mut state,
        Command::OrderBuilding { building_id: "coop".into(), scene_id: "scene-test".into(), x: 0, y: 0 },
    );
    assert!(has_message(&ordered, |text| text.starts_with("Construction of the Coop begins")));
    assert_eq!(state.player.money, 450);
    assert_eq!(farm_sim::inventory::count_item(&state.player.inventory, "seed-wheat"), 3);
    assert!(state.world.scenes[0].tiles[0][0].collision, "the site blocks at once");
    state.shop = None;

    sleep(&ctx, &mut state);
    assert_eq!(state.buildings[0].days_left, 1);
    assert!(farm_sim::world::world_movement::find_scene(&state, "building-1").is_none());
    let done = sleep(&ctx, &mut state);
    assert!(has_message(&done, |text| text == "The Coop is finished!"));
    let interior = farm_sim::world::world_movement::find_scene(&state, "building-1").expect("its interior");
    assert_eq!(interior.building.as_deref(), Some("building-1"));
    assert!(interior.is_generated());
    // In through the door (the middle of the bottom row), out through the interior's door.
    let farm = &state.world.scenes[0];
    let door = farm.transitions.iter().find(|t| t.to_scene_id == "building-1").expect("a door");
    assert_eq!((door.from_x, door.from_y), (1, 1));
    assert!(!farm.tiles[1][1].collision);
    assert!(farm.tiles[1][0].collision);
    let out = interior.transitions.iter().find(|t| t.to_scene_id == "scene-test").expect("a way out");
    assert_eq!((out.from_x, out.from_y, out.to_x, out.to_y), (2, 3, 1, 2));

    // A chest left inside stays through the upgrade.
    let interior_index = state.world.scenes.iter().position(|scene| scene.id == "building-1").unwrap();
    state.world.scenes[interior_index].tiles[1][1].machine =
        Some(TileMachine { type_id: "box".into(), ..TileMachine::default() });
    open_builder(&mut state);
    let view = overlay::overlay_view(&ctx, &state);
    let upgrade = view.building_options.iter().find(|option| option.building_id == "big-coop").expect("the upgrade");
    assert_eq!(upgrade.upgrades.as_deref(), Some("building-1"));
    run(&ctx, &mut state, Command::UpgradeBuilding { id: "building-1".into(), building_id: "big-coop".into() });
    assert_eq!(state.buildings[0].upgrade_to.as_deref(), Some("big-coop"));
    state.shop = None;
    let done = sleep(&ctx, &mut state);
    assert!(has_message(&done, |text| text == "The Big Coop is finished!"));
    assert_eq!(state.buildings[0].building_id, "big-coop");
    let interior = farm_sim::world::world_movement::find_scene(&state, "building-1").unwrap();
    assert_eq!((interior.width, interior.height), (6, 5));
    assert!(interior.tiles[1][1].machine.is_some());
    assert!(interior.transitions.iter().any(|t| (t.from_x, t.from_y) == (1, 4)));
    assert!(state.world.scenes[0].transitions.iter().filter(|t| t.to_scene_id == "building-1").count() == 1);
}

#[test]
fn builders_refuse_what_they_do_not_offer_and_sites_that_are_not_free() {
    let (ctx, mut state) = make_engine(builder_project, "refuse");
    let order = Command::OrderBuilding { building_id: "coop".into(), scene_id: "scene-test".into(), x: 0, y: 0 };
    run(&ctx, &mut state, order.clone());
    assert!(state.buildings.is_empty(), "no builder is open");
    open_builder(&mut state);
    let upgrade = Command::OrderBuilding { building_id: "big-coop".into(), scene_id: "scene-test".into(), x: 0, y: 0 };
    assert!(has_message(&run(&ctx, &mut state, upgrade), |text| text.starts_with("That is built by upgrading")));
    let on_wall = Command::OrderBuilding { building_id: "coop".into(), scene_id: "scene-test".into(), x: 3, y: 4 };
    assert!(has_message(&run(&ctx, &mut state, on_wall), |text| text.starts_with("There isn't room")));
    state.player.money = 10;
    assert!(has_message(&run(&ctx, &mut state, order), |text| text == "Not enough money!"));
    assert!(state.buildings.is_empty());
}

#[test]
fn animals_live_in_buildings_give_several_products_by_mood_and_have_young() {
    let (ctx, mut state) = make_engine(builder_project, "animals");
    let ctx = ctx.with_rules(CommandRules::Scripted);
    open_builder(&mut state);
    run(
        &ctx,
        &mut state,
        Command::OrderBuilding { building_id: "coop".into(), scene_id: "scene-test".into(), x: 0, y: 0 },
    );
    state.shop = None;
    sleep(&ctx, &mut state);
    sleep(&ctx, &mut state);
    open_builder(&mut state);
    let view = overlay::overlay_view(&ctx, &state);
    assert_eq!(view.animal_options[0].homes, vec![("building-1".to_owned(), "Coop".to_owned(), 0, 1)]);
    let bought = run(
        &ctx,
        &mut state,
        Command::BuyAnimal { species_id: "chicken".into(), building: "building-1".into(), name: "Henrietta".into() },
    );
    assert!(has_message(&bought, |text| text == "Henrietta moves into the Coop!"));
    assert_eq!(state.animals.len(), 1);
    assert_eq!(state.animals[0].scene_id, "building-1");
    assert_eq!(state.animals[0].building_id.as_deref(), Some("building-1"));
    let full = run(
        &ctx,
        &mut state,
        Command::BuyAnimal { species_id: "chicken".into(), building: "building-1".into(), name: String::new() },
    );
    assert!(has_message(&full, |text| text.starts_with("There's no room left")));
    state.shop = None;

    // A night and a day of care: an egg at the mood's quality, then a feather on its interval.
    sleep(&ctx, &mut state);
    assert!(state.animals[0].product_ready);
    let animal = state.animals[0].clone();
    farm_sim::animals::handle_animal_interaction(&ctx, &mut state, &animal);
    let egg = state.player.inventory.iter().find(|slot| slot.item.id == "egg").expect("an egg");
    assert_eq!(egg.quality.as_deref(), Some("silver"), "mood 70+ earns silver");
    sleep(&ctx, &mut state);
    assert!(state.animals[0].more_products[0].ready, "the feather comes every second day");
    // No young without room.
    assert_eq!(state.animals.len(), 1);
}

#[test]
fn young_are_born_where_there_is_room() {
    let (ctx, mut state) = make_engine(
        |project| {
            builder_project(project);
            project.building_types.as_mut().unwrap()[0].capacity = 2;
        },
        "young",
    );
    open_builder(&mut state);
    run(
        &ctx,
        &mut state,
        Command::OrderBuilding { building_id: "coop".into(), scene_id: "scene-test".into(), x: 0, y: 0 },
    );
    state.shop = None;
    sleep(&ctx, &mut state);
    sleep(&ctx, &mut state);
    open_builder(&mut state);
    run(
        &ctx,
        &mut state,
        Command::BuyAnimal { species_id: "chicken".into(), building: "building-1".into(), name: String::new() },
    );
    state.shop = None;
    // Grown up overnight, it has young the same night.
    let born = sleep(&ctx, &mut state);
    assert!(has_message(&born, |text| text == "A baby Chicken was born overnight!"));
    assert_eq!(state.animals.len(), 2);
    assert_eq!(state.animals[1].name, "Baby Chicken");
    assert_eq!(state.animals[1].building_id.as_deref(), Some("building-1"));
}

#[test]
fn a_greenhouse_grows_crops_in_every_season() {
    let (ctx, mut state) = make_engine(
        |project| {
            project.scenes[0].all_seasons = Some(true);
            project.current_season = "summer".into();
        },
        "greenhouse",
    );
    // Wheat grows in spring and fall only. Face the soil.
    state.player.y = units::tile_center(3);
    let planted = run(&ctx, &mut state, Command::Interact);
    assert!(has_message(&planted, |text| text.starts_with("Planted Wheat")), "{planted:?}");
    let crop = state.world.scenes[0].tiles[2][3].crop.clone();
    assert!(crop.is_some());
    sleep(&ctx, &mut state);
    assert_ne!(state.world.scenes[0].tiles[2][3].crop.as_ref().unwrap().withered, Some(true));
}

#[test]
fn a_new_game_picks_its_farm_layout_before_it_starts() {
    let (ctx, mut state) = make_engine(
        |project| {
            let mut river = empty_scene("farm-river", "River farm", 8, 7);
            river.tiles[0][0] = set_tile_layer(&river.tiles[0][0], "water");
            project.scenes.push(river);
            project.settings.farm = Some(FarmSettings {
                scene_id: "scene-test".into(),
                layouts: vec![
                    FarmLayout {
                        id: "classic".into(),
                        name: "Classic".into(),
                        scene_id: "scene-test".into(),
                        ..FarmLayout::default()
                    },
                    FarmLayout {
                        id: "river".into(),
                        name: "River".into(),
                        scene_id: "farm-river".into(),
                        ..FarmLayout::default()
                    },
                ],
                ..FarmSettings::default()
            });
        },
        "layout",
    );
    let chosen = run(&ctx, &mut state, Command::ChooseFarmLayout { layout_id: "river".into() });
    assert!(has_message(&chosen, |text| text == "Your farm: River"));
    let farm = farm_sim::world::world_movement::find_scene(&state, "scene-test").unwrap();
    assert_eq!((farm.width, farm.height), (8, 7));
    assert_eq!(farm.name, "Test Farm");
    assert!(farm_sim::world::world_movement::find_scene(&state, "farm-river").is_none());
    assert_eq!(state.farm_layout.as_deref(), Some("river"));
    // Once chosen, or once the game runs, it stays.
    run(&ctx, &mut state, Command::ChooseFarmLayout { layout_id: "classic".into() });
    assert_eq!(state.farm_layout.as_deref(), Some("river"));
    let _ = slot;
}
