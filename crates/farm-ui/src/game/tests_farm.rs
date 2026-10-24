//! Tests of the farm-building, goals and combat screens (M5–M7): the chest and shipping bin,
//! the builder's Build and Animals tabs with the placement map, the goals and mail panels, the
//! health bar. Every button runs the right engine command.

use super::test_support::Fixture;
use super::{GameAction, GoalsTab, Panel, ShopTab};
use crate::input::NavAction;
use crate::ui::WidgetId;
use farm_sim::schema::{
    AchievementDefinition, AnimalSpeciesDefinition, BuildingDefinition, BuildingState, BundleDefinition, BundleItem,
    CollectionDefinition, CombatSettings, GameProject, MachineTypeDefinition, MailDefinition, MailState,
    RecipeIngredient, ShopDefinition, ShopSession, StorageSession, TileMachine,
};
use farm_sim::{overlay, Command};

fn command(actions: &[GameAction]) -> Option<&Command> {
    actions.iter().find_map(|action| match action {
        GameAction::Command(command) => Some(command),
        _ => None,
    })
}

/// Clicks a widget and opens or closes the panels it asks for, as the player does; lets the
/// modal that opened settle to its size.
fn click_panel(fixture: &mut Fixture, id: WidgetId) {
    for action in fixture.click(id) {
        match action {
            GameAction::TogglePanel(panel) => fixture.game_ui.toggle(panel),
            GameAction::ClosePanel => fixture.game_ui.panel = None,
            _ => {}
        }
    }
    settle(fixture);
}

/// Two frames: a modal that just opened takes its content's size on the second.
fn settle(fixture: &mut Fixture) {
    fixture.idle();
    fixture.idle();
}

/// A chest (or, with `shipping`, a shipping bin) on tile (2, 2) of the farm, open.
fn open_storage(shipping: bool) -> Fixture {
    let mut fixture = Fixture::with(|project| {
        project.machine_types.push(MachineTypeDefinition {
            id: "chest".into(),
            name: if shipping { "Shipping Bin".into() } else { "Chest".into() },
            storage: (!shipping).then_some(4),
            shipping,
            ..MachineTypeDefinition::default()
        });
    });
    let scene = fixture.state.world.scenes[0].id.clone();
    let tile = &mut fixture.state.world.scenes[0].tiles[2][2];
    let mut machine = TileMachine { type_id: "chest".into(), ..TileMachine::default() };
    let item = fixture.state.player.inventory.iter().find(|slot| slot.item.id == "seed-tomato").unwrap().clone();
    machine.contents.push(item);
    tile.machine = Some(machine);
    fixture.state.storage = Some(StorageSession { scene_id: scene, x: 2, y: 2 });
    settle(&mut fixture);
    fixture
}

#[test]
fn a_chest_stores_and_takes_back_through_commands() {
    let mut fixture = open_storage(false);
    let texts = fixture.texts();
    assert!(texts.iter().any(|text| text == "Chest"), "{texts:?}");
    assert!(texts.iter().any(|text| text == "1/4 slots used"), "{texts:?}");
    let store = WidgetId::new("storage-store").with("seed-wheat");
    assert_eq!(
        command(&fixture.click(store)),
        Some(&Command::StoreItem { item_id: "seed-wheat".into(), quantity: 1, quality: Some("normal".into()) })
    );
    assert_eq!(
        command(&fixture.click(store.with("all"))),
        Some(&Command::StoreItem { item_id: "seed-wheat".into(), quantity: 10, quality: Some("normal".into()) })
    );
    let take = WidgetId::new("storage-take").with(0usize).with("seed-tomato");
    assert_eq!(command(&fixture.click(take)), Some(&Command::TakeStoredItem { index: 0, quantity: 1 }));
    assert_eq!(command(&fixture.click(take.with("all"))), Some(&Command::TakeStoredItem { index: 0, quantity: 5 }));
    assert_eq!(command(&fixture.click(WidgetId::new("storage").with("close"))), Some(&Command::CloseStorage));
}

#[test]
fn a_shipping_bin_says_what_it_holds_is_sold_overnight() {
    let fixture = open_storage(true);
    let texts = fixture.texts();
    assert!(texts.iter().any(|text| text == "What you put in is sold overnight."), "{texts:?}");
    assert!(texts.iter().any(|text| text == "SHIPPING TONIGHT"), "{texts:?}");
    assert!(texts.iter().any(|text| text == "Sells for $100"), "{texts:?}");
    assert!(fixture.ui.last_rect(WidgetId::new("storage-store").with("seed-wheat")).is_some());
}

/// A builder that puts up coops (2×2) and their upgrade, and sells chickens.
fn builder(project: &mut GameProject) {
    project.building_types = Some(vec![
        BuildingDefinition {
            id: "coop".into(),
            name: "Coop".into(),
            width: 2,
            height: 2,
            interior_scene_id: Some(project.scenes[0].id.clone()),
            cost: 50,
            materials: vec![RecipeIngredient { item_id: "seed-wheat".into(), quantity: 2 }],
            capacity: 2,
            ..BuildingDefinition::default()
        },
        BuildingDefinition {
            id: "big-coop".into(),
            name: "Big Coop".into(),
            width: 2,
            height: 2,
            cost: 10,
            upgrade_of: Some("coop".into()),
            ..BuildingDefinition::default()
        },
    ]);
    project.animal_species.push(AnimalSpeciesDefinition {
        id: "chicken".into(),
        name: "Chicken".into(),
        purchase_cost: 20,
        housing: vec!["coop".into()],
        ..AnimalSpeciesDefinition::default()
    });
    project.shops.push(ShopDefinition {
        id: "builder".into(),
        name: "Builder".into(),
        buildings: vec!["coop".into(), "big-coop".into()],
        animals: vec!["chicken".into()],
        ..ShopDefinition::default()
    });
}

fn open_builder() -> Fixture {
    let mut fixture = Fixture::with(builder);
    fixture.state.shop = Some(ShopSession { shop_id: "builder".into() });
    settle(&mut fixture);
    fixture
}

#[test]
fn a_builder_opens_on_its_build_tab() {
    let mut fixture = open_builder();
    assert_eq!(fixture.game_ui.shop_tab, ShopTab::Build);
    assert!(fixture.ui.last_rect(WidgetId::new("shop-tab").with("Buy")).is_none(), "no stock, no Buy tab");
    assert!(fixture.ui.last_rect(WidgetId::new("shop-tab").with("Animals")).is_some());
    let texts = fixture.texts();
    assert!(texts.iter().any(|text| text == "Coop"), "{texts:?}");
    // Without money the Place button is disabled.
    fixture.state.player.money = 0;
    fixture.idle();
    assert!(fixture.ui.last_rect(WidgetId::new("shop-build").with("coop")).is_none());
}

#[test]
fn placing_a_building_moves_its_footprint_and_orders_it_with_the_shop_open() {
    let mut fixture = open_builder();
    fixture.click(WidgetId::new("shop-build").with("coop"));
    settle(&mut fixture);
    let placement = fixture.game_ui.placement.clone().expect("placing");
    assert!(fixture.game_ui.host_modal_open());
    let scene = placement.scene_id.clone();
    // It starts on a free site, and the shop is not drawn behind the map.
    assert!(overlay::site_status(&fixture.ctx, &fixture.state, "coop", &scene, placement.x, placement.y).is_none());
    assert!(fixture.ui.last_rect(WidgetId::new("placement-map")).is_some());
    assert!(fixture.ui.last_rect(WidgetId::new("shop-tab").with("Build")).is_none());
    assert!(fixture.texts().iter().any(|text| text == "It can go here."));

    // The arrows move it a tile at a time.
    fixture.nav(&[NavAction::Left]);
    let moved = fixture.game_ui.placement.clone().unwrap();
    assert_eq!((moved.x, moved.y), (placement.x - 1, placement.y));
    fixture.nav(&[NavAction::Right]);

    // Onto the player: red, with the reason, and Accept does nothing.
    let (px, py) = (farm_sim::units::tile_of(fixture.state.player.x), farm_sim::units::tile_of(fixture.state.player.y));
    if let Some(placing) = fixture.game_ui.placement.as_mut() {
        (placing.x, placing.y) = (px, py);
    }
    fixture.idle();
    assert!(fixture.texts().iter().any(|text| text.starts_with("There isn't room")), "{:?}", fixture.texts());
    assert_eq!(command(&fixture.nav(&[NavAction::Accept])), None);
    assert!(fixture.game_ui.placement.is_some());

    // Back to the free site, then Accept orders it there; the shop is still open.
    if let Some(placing) = fixture.game_ui.placement.as_mut() {
        (placing.x, placing.y) = (placement.x, placement.y);
    }
    fixture.idle();
    assert_eq!(
        command(&fixture.nav(&[NavAction::Accept])),
        Some(&Command::OrderBuilding { building_id: "coop".into(), scene_id: scene, x: placement.x, y: placement.y })
    );
    assert!(fixture.game_ui.placement.is_none());
    fixture.idle();
    assert!(fixture.ui.last_rect(WidgetId::new("shop-tab").with("Build")).is_some(), "back in the shop");
}

#[test]
fn placing_follows_the_pointer_and_back_returns_to_the_shop() {
    let mut fixture = open_builder();
    fixture.click(WidgetId::new("shop-build").with("coop"));
    settle(&mut fixture);
    let start = fixture.game_ui.placement.clone().unwrap();
    // Clicking a spot on the map puts the footprint there and builds when it is free.
    let map = fixture.ui.last_rect(WidgetId::new("placement-map")).unwrap();
    let at = Some((map.x + 3.0, map.y + 3.0));
    let input = crate::input::UiInput { pointer: at, ..crate::input::UiInput::default() };
    fixture.frame(input);
    let moved = fixture.game_ui.placement.clone().unwrap();
    assert_ne!((moved.x, moved.y), (start.x, start.y));
    fixture.nav(&[NavAction::Back]);
    assert!(fixture.game_ui.placement.is_none());
    fixture.idle();
    assert!(fixture.ui.last_rect(WidgetId::new("shop-build").with("coop")).is_some());
    // Cancel does the same.
    fixture.click(WidgetId::new("shop-build").with("coop"));
    settle(&mut fixture);
    fixture.click(WidgetId::new("placement-cancel"));
    assert!(fixture.game_ui.placement.is_none());
}

#[test]
fn upgrades_and_animals_run_their_commands() {
    let mut fixture = Fixture::with(builder);
    let scene = fixture.state.world.scenes[0].id.clone();
    fixture.state.buildings.push(BuildingState {
        id: "building-1".into(),
        building_id: "coop".into(),
        scene_id: scene.clone(),
        x: 1,
        y: 1,
        interior_scene_id: scene,
        days_left: 0,
        upgrade_to: None,
    });
    fixture.state.shop = Some(ShopSession { shop_id: "builder".into() });
    settle(&mut fixture);
    let upgrade = WidgetId::new("shop-build").with("big-coop").with("building-1");
    assert_eq!(
        command(&fixture.click(upgrade)),
        Some(&Command::UpgradeBuilding { id: "building-1".into(), building_id: "big-coop".into() })
    );
    fixture.click(WidgetId::new("shop-tab").with("Animals"));
    settle(&mut fixture);
    assert_eq!(fixture.game_ui.shop_tab, ShopTab::Animals);
    assert!(fixture.texts().iter().any(|text| text == "Lives in: Coop (0/2)"), "{:?}", fixture.texts());
    assert_eq!(
        command(&fixture.click(WidgetId::new("shop-animal").with("chicken").with("building-1"))),
        Some(&Command::BuyAnimal { species_id: "chicken".into(), building: "building-1".into(), name: String::new() })
    );
}

fn with_goals(project: &mut GameProject) {
    project.collections = Some(vec![
        CollectionDefinition {
            id: "seeds".into(),
            name: "Seed Catalogue".into(),
            items: vec!["seed-wheat".into(), "seed-corn".into()],
            ..CollectionDefinition::default()
        },
        CollectionDefinition {
            id: "museum".into(),
            name: "Museum".into(),
            kind: "donated".into(),
            items: vec!["seed-tomato".into(), "seed-pumpkin".into()],
            ..CollectionDefinition::default()
        },
    ]);
    project.bundles = Some(vec![BundleDefinition {
        id: "spring".into(),
        name: "Spring Seeds".into(),
        group: "Pantry".into(),
        items: vec![
            BundleItem { item_id: "seed-wheat".into(), quantity: 3, quality: None },
            BundleItem { item_id: "seed-corn".into(), quantity: 1, quality: None },
        ],
        ..BundleDefinition::default()
    }]);
    project.achievements = Some(vec![
        AchievementDefinition { id: "first".into(), name: "First Steps".into(), ..AchievementDefinition::default() },
        AchievementDefinition {
            id: "secret".into(),
            name: "Secret Garden".into(),
            hidden: true,
            ..AchievementDefinition::default()
        },
    ]);
}

#[test]
fn the_goals_panel_donates_hands_in_and_hides_secret_achievements() {
    let mut fixture = Fixture::with(with_goals);
    fixture.state.achievements.insert("first".into(), 3);
    fixture.idle();
    assert!(fixture.ui.last_rect(WidgetId::new("hud").with("goals")).is_some(), "the toolbar has Goals");
    click_panel(&mut fixture, WidgetId::new("hud").with("goals"));
    assert_eq!(fixture.game_ui.panel, Some(Panel::Goals));
    // The tomato seeds are held: they can be donated; the pumpkin seeds are not.
    let donate = WidgetId::new("goals-donate").with("museum");
    assert_eq!(
        command(&fixture.click(donate.with("seed-tomato"))),
        Some(&Command::DonateItem { collection_id: "museum".into(), item_id: "seed-tomato".into() })
    );
    assert!(fixture.ui.last_rect(donate.with("seed-pumpkin")).is_none(), "nothing to donate");

    fixture.click(WidgetId::new("goals-tab").with("Bundles"));
    settle(&mut fixture);
    assert_eq!(fixture.game_ui.goals_tab, GoalsTab::Bundles);
    let texts = fixture.texts();
    assert!(texts.iter().any(|text| text == "PANTRY"), "{texts:?}");
    assert_eq!(
        command(&fixture.click(WidgetId::new("goals-hand-in").with("spring").with(0usize))),
        Some(&Command::HandInBundle { bundle_id: "spring".into(), line: 0 })
    );
    assert!(fixture.ui.last_rect(WidgetId::new("goals-hand-in").with("spring").with(1usize)).is_none());

    fixture.nav(&[NavAction::TabNext]);
    assert_eq!(fixture.game_ui.goals_tab, GoalsTab::Achievements);
    fixture.idle();
    let texts = fixture.texts();
    assert!(texts.iter().any(|text| text == "First Steps") && texts.iter().any(|text| text == "Day 3"));
    assert!(texts.iter().any(|text| text == "???") && !texts.iter().any(|text| text == "Secret Garden"), "{texts:?}");
    // No shipping bin: no Shipped tab.
    assert!(fixture.ui.last_rect(WidgetId::new("goals-tab").with("Shipped")).is_none());
}

#[test]
fn games_without_goals_or_mail_show_neither_button() {
    let mut fixture = Fixture::starter();
    fixture.idle();
    assert!(fixture.ui.last_rect(WidgetId::new("hud").with("goals")).is_none());
    assert!(fixture.ui.last_rect(WidgetId::new("hud").with("mail")).is_none());
    assert!(!fixture.texts().iter().any(|text| text.starts_with("Health")));
}

#[test]
fn the_mailbox_lists_letters_newest_first_and_reading_one_sends_read_mail() {
    let mut fixture = Fixture::with(|project| {
        project.mail = Some(vec![
            MailDefinition {
                id: "hello".into(),
                subject: "Welcome".into(),
                from: "npc-merchant".into(),
                text: "Hello, {player}!".into(),
                attachments: vec![RecipeIngredient { item_id: "seed-corn".into(), quantity: 3 }],
                ..MailDefinition::default()
            },
            MailDefinition { id: "news".into(), subject: "News".into(), ..MailDefinition::default() },
        ]);
    });
    fixture.state.mailbox = vec![
        MailState { mail_id: "hello".into(), day: 1, read: false, taken: false },
        MailState { mail_id: "news".into(), day: 2, read: true, taken: true },
    ];
    fixture.idle();
    // The envelope's badge counts the unread letter.
    assert!(fixture.texts().iter().any(|text| text == "1"), "{:?}", fixture.texts());
    click_panel(&mut fixture, WidgetId::new("hud").with("mail"));
    assert_eq!(fixture.game_ui.panel, Some(Panel::Mail));
    let news = fixture.ui.last_rect(WidgetId::new("mail-letter").with(1usize)).unwrap();
    let hello = fixture.ui.last_rect(WidgetId::new("mail-letter").with(0usize)).unwrap();
    assert!(news.y < hello.y, "newest first");
    assert_eq!(
        command(&fixture.click(WidgetId::new("mail-letter").with(0usize))),
        Some(&Command::ReadMail { index: 0 })
    );
    assert_eq!(fixture.game_ui.letter, Some(0));
    settle(&mut fixture);
    let texts = fixture.texts();
    let filled = farm_sim::substitution::fill(&fixture.ctx.content, &fixture.state, "Hello, {player}!").into_owned();
    assert!(!filled.contains('{') && texts.iter().any(|text| text.contains(&filled)), "{texts:?}");
    assert!(texts.iter().any(|text| text.starts_with("From Merchant Mia")), "{texts:?}");
    assert_eq!(command(&fixture.click(WidgetId::new("mail-take"))), Some(&Command::ReadMail { index: 0 }));
    fixture.click(WidgetId::new("mail-back"));
    assert_eq!(fixture.game_ui.letter, None);
    settle(&mut fixture);
    // A letter read and emptied opens without a command.
    assert_eq!(command(&fixture.click(WidgetId::new("mail-letter").with(1usize))), None);
    assert_eq!(fixture.game_ui.letter, Some(1));
}

#[test]
fn health_shows_beside_energy_with_combat_on() {
    let mut fixture = Fixture::with(|project| {
        project.settings.combat = Some(CombatSettings { enabled: true, max_health: 80, ..CombatSettings::default() });
    });
    // Wide enough for the full HUD, with its numbers.
    fixture.size = (2400.0, 900.0);
    fixture.state.player.health = Some(30);
    fixture.idle();
    let texts = fixture.texts();
    assert!(texts.iter().any(|text| text == "Health:") && texts.iter().any(|text| text == "30 / 80"), "{texts:?}");
    // A hit flashes the world red for a moment.
    fixture.state.player.health = Some(20);
    fixture.idle();
    assert!(!fixture.ui.world_tints().is_empty());
}
