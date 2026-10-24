//! The state invariants (`farm_fuzz::invariants`) under generated content (#116): a golden
//! project with random settings, player, items, shops and scene sizes, played with a random
//! command log, checked after every command and tick. Content edits go past what the editor
//! allows: hosts take project JSON from anywhere.
//!
//! A failure prints the shrunk project edits and log bytes; `PROPTEST_RNG_SEED` replays a run.

use farm_fuzz::input::{self, Choices, Ids};
use farm_sim::replay::ReplayInput;
use farm_sim::schema::{GameProject, InventorySlot};
use farm_sim::{state, units, Command, CommandRules, EngineContext, HookBus};
use proptest::prelude::*;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::LazyLock;

const BASES: [&str; 6] = ["starter-farm", "cozy-garden", "quest-rpg", "lab-calendar", "blank", "fixture-farm"];

static PROJECTS: LazyLock<Vec<GameProject>> = LazyLock::new(|| {
    BASES
        .iter()
        .map(|name| {
            let path: PathBuf =
                [env!("CARGO_MANIFEST_DIR"), "..", "..", "fixtures", "golden", "content", &format!("{name}.json")]
                    .iter()
                    .collect();
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            let fixture: Value = serde_json::from_str(&text).expect("fixture JSON");
            serde_json::from_value(fixture["project"].clone()).expect("a project")
        })
        .collect()
});

/// One change to a project.
#[derive(Debug, Clone)]
enum ContentEdit {
    PlayerSpeed(i32),
    Energy { enabled: bool, max: i32, start: Option<i32>, player_max: Option<i32> },
    Collapse { penalty: i64, fraction: u32 },
    Time { start: u32, end: u32, rate: u32 },
    Money(i64),
    InventorySize(u32),
    StartItem { index: usize, quantity: u32 },
    ItemStack { index: usize, max_stack: u32, stackable: bool },
    ItemValue { index: usize, value: i64 },
    ShopPrice { index: usize, price: i64, sell_multiplier: u32 },
    SceneSize { index: usize, width: i32, height: i32 },
    PlayerAt { scene: usize, x: i32, y: i32 },
}

fn content_edit() -> impl Strategy<Value = ContentEdit> {
    prop_oneof![
        prop_oneof![Just(0), 1..=units::TILE, Just(units::TILE * 40)].prop_map(ContentEdit::PlayerSpeed),
        (any::<bool>(), -10..=500i32, proptest::option::of(-50..=600i32), proptest::option::of(-10..=500i32)).prop_map(
            |(enabled, max, start, player_max)| ContentEdit::Energy {
                enabled,
                max: units::points(max),
                start: start.map(units::points),
                player_max: player_max.map(units::points),
            }
        ),
        (-100..=100_000i64, 0..=2000u32).prop_map(|(penalty, fraction)| ContentEdit::Collapse { penalty, fraction }),
        (0..=1500u32, 0..=1600u32, prop_oneof![Just(0u32), 1..=units::MINUTE, Just(units::MINUTE * 120)])
            .prop_map(|(start, end, rate)| ContentEdit::Time { start, end, rate }),
        prop_oneof![-1000..=0i64, 0..=100_000i64].prop_map(ContentEdit::Money),
        (0..=40u32).prop_map(ContentEdit::InventorySize),
        (any::<usize>(), 0..=2500u32).prop_map(|(index, quantity)| ContentEdit::StartItem { index, quantity }),
        (any::<usize>(), 0..=5u32, any::<bool>()).prop_map(|(index, max_stack, stackable)| ContentEdit::ItemStack {
            index,
            max_stack,
            stackable
        }),
        (any::<usize>(), -500..=100_000i64).prop_map(|(index, value)| ContentEdit::ItemValue { index, value }),
        (any::<usize>(), -500..=100_000i64, 0..=5000u32)
            .prop_map(|(index, price, sell_multiplier)| ContentEdit::ShopPrice { index, price, sell_multiplier }),
        (any::<usize>(), -2..=48i32, -2..=48i32).prop_map(|(index, width, height)| ContentEdit::SceneSize {
            index,
            width,
            height
        }),
        (any::<usize>(), -4..=60i32, -4..=60i32).prop_map(|(scene, x, y)| ContentEdit::PlayerAt { scene, x, y }),
    ]
}

fn edit(project: &mut GameProject, edit: &ContentEdit) {
    match *edit {
        ContentEdit::PlayerSpeed(speed) => project.settings.movement.player_speed = speed,
        ContentEdit::Energy { enabled, max, start, player_max } => {
            project.settings.energy_enabled = enabled;
            project.settings.max_energy = max;
            project.player.energy = start;
            project.player.max_energy = player_max;
        }
        ContentEdit::Collapse { penalty, fraction } => {
            project.settings.collapse_money_penalty = penalty;
            project.settings.collapse_energy_fraction = fraction;
        }
        ContentEdit::Time { start, end, rate } => {
            project.settings.time.day_start_minute = start;
            project.settings.time.day_end_minute = end;
            project.settings.time.minutes_per_real_second = rate;
        }
        ContentEdit::Money(money) => project.player.money = money,
        ContentEdit::InventorySize(size) => project.player.max_inventory_size = size,
        ContentEdit::StartItem { index, quantity } => {
            if let Some(item) = pick(&project.items, index).cloned() {
                project.player.inventory.push(InventorySlot { item, quantity, quality: None });
            }
        }
        ContentEdit::ItemStack { index, max_stack, stackable } => {
            let len = project.items.len();
            if len > 0 {
                let item = &mut project.items[index % len];
                item.max_stack = max_stack;
                item.stackable = stackable;
            }
        }
        ContentEdit::ItemValue { index, value } => {
            let len = project.items.len();
            if len > 0 {
                project.items[index % len].value = value;
            }
        }
        ContentEdit::ShopPrice { index, price, sell_multiplier } => {
            let len = project.shops.len();
            if len > 0 {
                let shop = &mut project.shops[index % len];
                shop.sell_price_multiplier = sell_multiplier;
                for entry in &mut shop.stock {
                    entry.price = Some(price);
                }
            }
        }
        ContentEdit::SceneSize { index, width, height } => {
            let len = project.scenes.len();
            if len > 0 {
                let scene = &mut project.scenes[index % len];
                scene.width = width;
                scene.height = height;
            }
        }
        ContentEdit::PlayerAt { scene, x, y } => {
            project.player.scene_id =
                pick(&project.scenes, scene).map_or_else(|| "nowhere".to_owned(), |s| s.id.clone());
            project.player.x = x;
            project.player.y = y;
        }
    }
}

fn pick<T>(list: &[T], index: usize) -> Option<&T> {
    if list.is_empty() {
        None
    } else {
        list.get(index % list.len())
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 96, ..ProptestConfig::default() })]

    #[test]
    fn generated_content_keeps_the_state_invariants_after_every_step(
        base in 0..BASES.len(),
        edits in proptest::collection::vec(content_edit(), 0..10),
        scripted in any::<bool>(),
        cutscenes in any::<bool>(),
        farm in any::<bool>(),
        log in proptest::collection::vec(any::<u8>(), 0..400),
    ) {
        let mut project = PROJECTS[base].clone();
        for change in &edits {
            edit(&mut project, change);
        }
        if cutscenes {
            input::add_cutscenes(&mut project);
        }
        if farm {
            input::add_farm(&mut project);
        }
        let content = state::create_content_from_project(&project);
        let ids = Ids::of(&content);
        let log = input::inputs(&ids, &mut Choices::new(&log));
        let rules = if scripted { CommandRules::Scripted } else { CommandRules::Player };
        let ctx = EngineContext::with_hooks(content, HookBus::new()).with_rules(rules);
        let mut game = state::create_game_state(&project, Some("invariants"));
        farm_fuzz::play_checked(&ctx, &mut game, &log);
    }
}

/// The generator reaches every cutscene command, and the cutscenes `add_cutscenes` gives a game
/// start, play, branch, skip and end under the invariants.
#[test]
fn generated_games_start_play_and_end_cutscenes() {
    let mut project = PROJECTS[0].clone();
    input::add_cutscenes(&mut project);
    let content = state::create_content_from_project(&project);
    let ids = Ids::of(&content);
    assert_eq!(ids.cutscenes, ["fuzz-scene", "fuzz-loop"]);
    let decoded: Vec<Command> = (0..=255u8)
        .flat_map(|a| (0..=255u8).map(move |b| [a, b]))
        .map(|bytes| input::command(&ids, &mut Choices::new(&bytes)))
        .collect();
    for wanted in ["advanceCutscene", "chooseCutsceneOption", "skipCutscene"] {
        assert!(decoded.iter().any(|command| command.type_name() == wanted), "no {wanted} decoded");
    }

    let ctx = EngineContext::with_hooks(content, HookBus::new()).with_rules(CommandRules::Player);
    let start =
        |id: &str| ReplayInput::Command { command: Command::PerformAction { action_id: input::start_action(id) } };
    let command = |command: Command| ReplayInput::Command { command };
    let tick = |ticks: u64| ReplayInput::Tick { ticks };

    // Played through: the line, the walks and timed steps, then the choice's "go on".
    let mut game = state::create_game_state(&project, Some("cutscenes"));
    farm_fuzz::play_checked(&ctx, &mut game, &[start("fuzz-scene"), tick(1)]);
    assert_eq!(game.cutscene.as_ref().map(|running| running.step), Some(0), "the first line waits");
    farm_fuzz::play_checked(&ctx, &mut game, &[command(Command::AdvanceCutscene), tick(400)]);
    assert!(game.cutscene.is_some(), "the choice waits");
    farm_fuzz::play_checked(
        &ctx,
        &mut game,
        &[command(Command::ChooseCutsceneOption { index: 1 }), tick(1), command(Command::CloseDialogue), tick(1)],
    );
    // The loop cutscene could not start over the running one; "Back." waits.
    assert_eq!(game.cutscene.as_ref().map(|running| running.cutscene_id.as_str()), Some("fuzz-scene"));
    farm_fuzz::play_checked(&ctx, &mut game, &[command(Command::AdvanceCutscene), tick(1)]);
    assert_eq!(game.cutscene, None, "the end step ends it");

    // Skipped, then the loop ends itself, and a cutscene the content lacks never starts.
    let mut game = state::create_game_state(&project, Some("cutscenes"));
    farm_fuzz::play_checked(&ctx, &mut game, &[start("fuzz-scene"), tick(1), command(Command::SkipCutscene)]);
    assert!(game.cutscene.is_some(), "skipping stops at the choice");
    farm_fuzz::play_checked(
        &ctx,
        &mut game,
        &[command(Command::ChooseCutsceneOption { index: 0 }), command(Command::SkipCutscene)],
    );
    farm_fuzz::play_checked(&ctx, &mut game, &[command(Command::ChooseCutsceneOption { index: 2 }), tick(400)]);
    let mut looping = state::create_game_state(&project, Some("cutscenes"));
    farm_fuzz::play_checked(&ctx, &mut looping, &[start("fuzz-loop"), tick(1)]);
    assert_eq!(looping.cutscene, None, "a cutscene that never waits ends");
    farm_fuzz::play_checked(&ctx, &mut looping, &[start(input::UNKNOWN_ID), tick(1)]);
    assert_eq!(looping.cutscene, None);
}

/// The generator reaches every M5–M7 command, and a game with [`input::add_farm`]'s kit plays
/// them — storage, building, upgrading, animals, layouts, goals, mail and fights — under the
/// invariants.
#[test]
fn generated_games_build_farms_reach_goals_and_fight() {
    let mut project = PROJECTS[0].clone();
    input::add_farm(&mut project);
    let content = state::create_content_from_project(&project);
    let ids = Ids::of(&content);
    let decoded: Vec<Command> = (0..=255u8)
        .flat_map(|a| (0..=255u8).map(move |b| [a, b]))
        .map(|bytes| input::command(&ids, &mut Choices::new(&bytes)))
        .collect();
    for wanted in [
        "storeItem",
        "takeStoredItem",
        "closeStorage",
        "orderBuilding",
        "upgradeBuilding",
        "buyAnimal",
        "chooseFarmLayout",
        "donateItem",
        "handInBundle",
        "readMail",
    ] {
        assert!(decoded.iter().any(|command| command.type_name() == wanted), "no {wanted} decoded");
    }

    let ctx = EngineContext::with_hooks(content, HookBus::new()).with_rules(CommandRules::Scripted);
    let command = |command: Command| ReplayInput::Command { command };
    let tick = |ticks: u64| ReplayInput::Tick { ticks };
    let scene = project.player.scene_id.clone();
    let item = project.items[0].id.clone();
    let mut game = state::create_game_state(&project, Some("farm"));
    let log = [
        command(Command::ChooseFarmLayout { layout_id: "fuzz-other".into() }),
        tick(1),
        command(Command::OpenShop { shop_id: "fuzz-builder".into() }),
        command(Command::OrderBuilding { building_id: "fuzz-coop".into(), scene_id: scene.clone(), x: 4, y: 4 }),
        command(Command::OrderBuilding { building_id: "fuzz-well".into(), scene_id: scene.clone(), x: 1, y: 6 }),
        command(Command::BuyAnimal { species_id: "fuzz-hen".into(), building: "fuzz-house".into(), name: String::new() }),
        command(Command::UpgradeBuilding { id: "fuzz-house".into(), building_id: "fuzz-big-coop".into() }),
        command(Command::CloseShop),
        command(Command::PlaceMachine { machine_type_id: "fuzz-chest".into() }),
        command(Command::Interact),
        command(Command::StoreItem { item_id: item.clone(), quantity: 1, quality: None }),
        command(Command::TakeStoredItem { index: 0, quantity: 5 }),
        command(Command::CloseStorage),
        command(Command::UseTool { tool: input::FUZZ_SWORD.into() }),
        command(Command::PerformAction { action_id: "fuzz-post".into() }),
        command(Command::DonateItem { collection_id: "fuzz-museum".into(), item_id: item.clone() }),
        command(Command::HandInBundle { bundle_id: "fuzz-bundle".into(), line: 1 }),
        tick(400),
        command(Command::Sleep),
        command(Command::ReadMail { index: 0 }),
        command(Command::Sleep),
        tick(600),
        command(Command::Sleep),
    ];
    farm_fuzz::play_checked(&ctx, &mut game, &log);
    assert_eq!(game.farm_layout.as_deref(), Some("fuzz-other"));
    assert!(!game.mailbox.is_empty());
    assert!(game.achievements.contains_key("fuzz-rich"));
}
