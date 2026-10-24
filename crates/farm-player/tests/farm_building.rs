//! Building the farm and long-term goals in the player (M5, M6): a chest opened by interacting,
//! filled from its modal and closed with Escape; a building placed from a builder's shop on its
//! map, with Escape backing out to the shop; the farm layout picked in the creator; the goals
//! and mail panels on their keys.

mod common;

use common::{click, idle, new_game_of, press, Stores};
use farm_player::ScreenKind;
use farm_sim::schema::{
    AchievementDefinition, BuildingDefinition, FarmLayout, FarmSettings, GameProject, MachineTypeDefinition,
    ShopDefinition, ShopSession, TileMachine,
};
use farm_ui::game::Panel;
use farm_ui::WidgetId;

#[test]
fn a_chest_opens_takes_items_and_closes_with_escape() {
    let mut project = common::starter();
    project.machine_types.push(MachineTypeDefinition {
        id: "chest".into(),
        name: "Chest".into(),
        storage: Some(8),
        ..MachineTypeDefinition::default()
    });
    let stores = Stores::new();
    let mut player = new_game_of(&stores, &project);
    // A chest on the tile the player faces.
    let mut state = player.state().unwrap().clone();
    let target = farm_sim::world::world_movement::facing_target(&state);
    let scene = state.world.scenes.iter_mut().find(|scene| scene.id == state.player.scene_id).unwrap();
    scene.tiles[target.y as usize][target.x as usize] = farm_sim::schema::Tile {
        machine: Some(TileMachine { type_id: "chest".into(), ..TileMachine::default() }),
        ..scene.tiles[target.y as usize][target.x as usize].clone()
    };
    player.replace_state(state).unwrap();

    press(&mut player, "e");
    assert!(player.state().unwrap().storage.is_some(), "interacting opens the chest");
    idle(&mut player, 2);
    let wheat = |player: &farm_player::Player| {
        farm_sim::inventory::count_item(&player.state().unwrap().player.inventory, "seed-wheat")
    };
    let before = wheat(&player);
    click(&mut player, WidgetId::new("storage-store").with("seed-wheat").with("all"));
    assert_eq!(wheat(&player), 0, "every wheat seed went in");
    click(&mut player, WidgetId::new("storage-take").with(0usize).with("seed-wheat"));
    assert_eq!(wheat(&player), 1);

    press(&mut player, "escape");
    let state = player.state().unwrap();
    assert!(state.storage.is_none(), "Escape closes the chest");
    assert_eq!(player.screen(), ScreenKind::Playing, "and does not open the pause menu");
    let chest = state
        .world
        .scenes
        .iter()
        .find(|scene| scene.id == state.player.scene_id)
        .and_then(|scene| scene.tile(target.x, target.y))
        .and_then(|tile| tile.machine.as_ref())
        .unwrap();
    assert_eq!(chest.contents.iter().map(|slot| u64::from(slot.quantity)).sum::<u64>(), before - 1);
}

fn builder(project: &mut GameProject) {
    project.building_types = Some(vec![BuildingDefinition {
        id: "shed".into(),
        name: "Shed".into(),
        width: 2,
        height: 2,
        cost: 30,
        ..BuildingDefinition::default()
    }]);
    project.shops.push(ShopDefinition {
        id: "builder".into(),
        name: "Builder".into(),
        buildings: vec!["shed".into()],
        ..ShopDefinition::default()
    });
}

#[test]
fn a_building_is_placed_on_its_map_and_ordered_from_the_open_shop() {
    let mut project = common::starter();
    builder(&mut project);
    let stores = Stores::new();
    let mut player = new_game_of(&stores, &project);
    let mut state = player.state().unwrap().clone();
    state.shop = Some(ShopSession { shop_id: "builder".into() });
    player.replace_state(state).unwrap();
    idle(&mut player, 2);

    click(&mut player, WidgetId::new("shop-build").with("shed"));
    assert!(player.widget_rect(WidgetId::new("placement-map")).is_some(), "the placement map");
    // Escape backs out to the shop, which stays open.
    press(&mut player, "escape");
    assert!(player.widget_rect(WidgetId::new("placement-map")).is_none());
    assert!(player.state().unwrap().shop.is_some());
    assert_eq!(player.screen(), ScreenKind::Playing);

    click(&mut player, WidgetId::new("shop-build").with("shed"));
    press(&mut player, "arrowup");
    press(&mut player, "enter");
    let state = player.state().unwrap();
    assert_eq!(state.buildings.len(), 1, "Enter orders it");
    assert_eq!(state.buildings[0].building_id, "shed");
    assert!(state.shop.is_some(), "back in the shop");
    assert_eq!(state.player.money, 100 - 30);
    assert!(player.widget_rect(WidgetId::new("placement-map")).is_none());
}

fn two_layouts(project: &mut GameProject) {
    let mut river = project.scenes[0].clone();
    river.id = "farm-river".into();
    river.name = "River farm".into();
    let farm = project.scenes[0].id.clone();
    project.scenes.push(river);
    project.settings.farm = Some(FarmSettings {
        scene_id: farm.clone(),
        layouts: vec![
            FarmLayout {
                id: "classic".into(),
                name: "Classic".into(),
                description: "Open fields.".into(),
                scene_id: farm,
            },
            FarmLayout {
                id: "river".into(),
                name: "River".into(),
                description: "Fish by the fence.".into(),
                scene_id: "farm-river".into(),
            },
        ],
        ..FarmSettings::default()
    });
}

#[test]
fn the_creator_asks_for_the_farm_layout_even_without_the_character_creator() {
    let mut project = common::starter();
    two_layouts(&mut project);
    let stores = Stores::new();
    let mut player = stores.standalone(&project);
    idle(&mut player, 1);
    press(&mut player, "enter");
    assert_eq!(player.screen(), ScreenKind::Creator);
    assert!(player.widget_rect(WidgetId::new("creator-field").with(0usize)).is_none(), "only the layouts");
    click(&mut player, WidgetId::new("creator-layout").with(1usize));
    click(&mut player, WidgetId::new("creator-start"));
    assert_eq!(player.screen(), ScreenKind::Playing);
    assert_eq!(player.state().unwrap().farm_layout.as_deref(), Some("river"));
}

#[test]
fn the_goals_key_opens_and_closes_the_panel_in_games_with_goals() {
    let stores = Stores::new();
    let mut plain = common::new_game(&stores);
    press(&mut plain, "g");
    assert_eq!(plain.panel(), None, "no goals: G passes to the game");

    let mut project = common::starter();
    project.achievements =
        Some(vec![AchievementDefinition { id: "a".into(), name: "A".into(), ..AchievementDefinition::default() }]);
    let mut player = new_game_of(&Stores::new(), &project);
    press(&mut player, "g");
    assert_eq!(player.panel(), Some(Panel::Goals));
    press(&mut player, "g");
    assert_eq!(player.panel(), None);
    press(&mut player, "g");
    press(&mut player, "escape");
    assert_eq!(player.panel(), None);
    assert_eq!(player.screen(), ScreenKind::Playing);
}
