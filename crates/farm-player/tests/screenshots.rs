//! Screenshot goldens of the graphical player at 1280×800 (Steam Deck, 16:10) and 1920×1080
//! (16:9): the title screen, settings, the gameplay HUD, a dialogue and a shop; and the start of
//! the Cozy Garden and Quest RPG samples and a pack-defined minigame at 1280×800.
//!
//! Frames are rendered at full size, then halved (2×2 box filter) and compared with the PNGs in
//! `fixtures/player/` within a small tolerance, so the checked-in images stay small.
//!
//! - Bless after an intended change: `FARM_PLAYER_BLESS=1 cargo test -p farm-player --test screenshots`,
//!   then look at the images before committing. A blessing run always fails; rerun without the switch.
//! - Full-size frames for a closer look: `FARM_PLAYER_SCREENSHOTS=<dir>`.

mod common;
#[path = "../../farm-sim/tests/recording/mod.rs"]
mod recording;

static RECORDER: recording::Recorder = recording::Recorder::new("FARM_PLAYER_BLESS", "player screenshots");

use common::{new_game, press, scene, Stores, FRAME};
use farm_player::{Player, PlayerMode};
use farm_render::images::decode_image;
use farm_render::tiny_skia::Pixmap;
use std::path::PathBuf;

const SIZES: [(u32, u32); 2] = [(1280, 800), (1920, 1080)];
/// Channel difference that counts as different, and how many pixels may differ.
const CHANNEL_TOLERANCE: i32 = 6;
const MAX_DIFFERENT_FRACTION: f64 = 0.004;

fn render(mut player: Player, width: u32, height: u32) -> Pixmap {
    // Past the toasts' fade-in, before they leave.
    for _ in 0..20 {
        player.step(FRAME, &[], width, height).unwrap();
    }
    player.frame(FRAME, &[], width, height).unwrap().pixels.clone()
}

/// Halves a frame with a 2×2 box filter (RGBA).
fn half(pixmap: &Pixmap) -> (u32, u32, Vec<u8>) {
    let (w, h) = (pixmap.width() / 2, pixmap.height() / 2);
    let stride = pixmap.width() as usize * 4;
    let data = pixmap.data();
    let mut out = Vec::with_capacity(w as usize * h as usize * 4);
    for y in 0..h as usize {
        for x in 0..w as usize {
            for channel in 0..4 {
                let at = |dx: usize, dy: usize| u32::from(data[(y * 2 + dy) * stride + (x * 2 + dx) * 4 + channel]);
                out.push(((at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1) + 2) / 4) as u8);
            }
        }
    }
    (w, h, out)
}

fn golden_path(name: &str, width: u32, height: u32) -> PathBuf {
    common::root().join("fixtures").join("player").join(format!("{name}-{width}x{height}.png"))
}

fn check(name: &str, width: u32, height: u32) -> Result<(), String> {
    let pixmap = render(scene(name), width, height);
    if let Some(folder) = std::env::var_os("FARM_PLAYER_SCREENSHOTS") {
        let folder = PathBuf::from(folder);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join(format!("{name}-{width}x{height}.png")), farm_render::encode_png(&pixmap)).unwrap();
    }
    let (w, h, pixels) = half(&pixmap);
    let small = Pixmap::from_vec(pixels.clone(), farm_render::tiny_skia::IntSize::from_wh(w, h).unwrap()).unwrap();
    let path = golden_path(name, width, height);
    if RECORDER.enabled() {
        RECORDER.write(&path, &farm_render::encode_png(&small))?;
        return Err(format!("{name} {width}×{height}: blessed. {}", RECORDER.summary()));
    }
    let bytes = std::fs::read(&path)
        .map_err(|e| format!("{name}: {e}; run with FARM_PLAYER_BLESS=1 to create {}", path.display()))?;
    let golden = decode_image(&bytes).map_err(|e| format!("{name}: {e}"))?;
    if (golden.width(), golden.height()) != (w, h) {
        return Err(format!("{name}: {w}×{h} differs from the golden {}×{}", golden.width(), golden.height()));
    }
    let different = pixels
        .chunks_exact(4)
        .zip(golden.pixels().chunks_exact(4))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| (i32::from(*x) - i32::from(*y)).abs() > CHANNEL_TOLERANCE))
        .count();
    let allowed = (f64::from(w * h) * MAX_DIFFERENT_FRACTION) as usize;
    if different > allowed {
        return Err(format!("{name} {width}×{height}: {different} pixels differ (at most {allowed})"));
    }
    Ok(())
}

fn check_all(name: &str) {
    let failures: Vec<String> = SIZES.iter().filter_map(|(w, h)| check(name, *w, *h).err()).collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn title_screen() {
    check_all("title");
}

#[test]
fn settings_screen() {
    check_all("settings");
}

#[test]
fn gameplay_hud() {
    check_all("hud");
}

#[test]
fn dialogue() {
    check_all("dialogue");
}

#[test]
fn shop() {
    check_all("shop");
}

/// A pack-defined minigame's frame, drawn from its plugin's draw list (one size).
#[test]
fn pack_minigame() {
    check("pack-minigame", 1280, 800).unwrap();
}

/// The samples look different from the starter farm and from each other (one size each).
#[test]
fn sample_games() {
    let failures: Vec<String> =
        ["sample-cozy", "sample-quest"].iter().filter_map(|name| check(name, 1280, 800).err()).collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn frames_are_identical_across_runs() {
    let first = render(scene("hud"), 640, 400);
    let second = render(scene("hud"), 640, 400);
    assert_eq!(first.data(), second.data());
    let embedded = Player::from_project(common::starter(), Stores::new().options(PlayerMode::Embedded)).unwrap();
    let frame = render(embedded, 320, 200);
    assert_eq!((frame.width(), frame.height()), (320, 200));
}

/// Spanish and the readable font render every golden screen (not compared with an image: the
/// goldens stay English in Inter). The frames differ from the defaults and stay deterministic;
/// `FARM_PLAYER_SCREENSHOTS` writes them for a look.
#[test]
fn spanish_and_the_readable_font_render() {
    let localized = |name: &str| {
        let mut player = scene(name);
        let mut settings = player.settings().clone();
        settings.language = "es".into();
        settings.accessibility.readable_font = true;
        player.set_settings(settings);
        player
    };
    for name in ["title", "settings", "hud", "dialogue", "shop"] {
        let (width, height) = (1280, 800);
        let default = render(scene(name), width, height);
        let spanish = render(localized(name), width, height);
        assert_ne!(default.data(), spanish.data(), "{name}");
        assert_eq!(spanish.data(), render(localized(name), width, height).data(), "{name} is deterministic");
        if let Some(folder) = std::env::var_os("FARM_PLAYER_SCREENSHOTS") {
            let folder = PathBuf::from(folder);
            std::fs::create_dir_all(&folder).unwrap();
            let path = folder.join(format!("{name}-es-readable-{width}x{height}.png"));
            std::fs::write(path, farm_render::encode_png(&spanish)).unwrap();
        }
    }
}

/// Every other screen, for a look (not compared): `FARM_PLAYER_SCREENSHOTS=<dir> cargo test -p
/// farm-player --test screenshots -- --ignored`.
#[test]
#[ignore = "writes review screenshots; set FARM_PLAYER_SCREENSHOTS"]
fn review_screens() {
    let Some(folder) = std::env::var_os("FARM_PLAYER_SCREENSHOTS") else { return };
    let folder = PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    let write = |name: &str, player: Player, width: u32, height: u32| {
        let pixmap = render(player, width, height);
        std::fs::write(folder.join(format!("{name}-{width}x{height}.png")), farm_render::encode_png(&pixmap)).unwrap();
    };
    let stores = Stores::new();
    let with_panel = |key: &str| {
        let mut player = new_game(&Stores::new());
        press(&mut player, key);
        player
    };
    for (width, height) in [(1280, 800), (1920, 1080), (800, 600)] {
        write("inventory", with_panel("i"), width, height);
        write("quests", with_panel("j"), width, height);
        write("crafting", with_panel("x"), width, height);
        write("pause", with_panel("escape"), width, height);
        let mut sleep = new_game(&Stores::new());
        press(&mut sleep, "z");
        write("toasts", sleep, width, height);
        let mut minigame = new_game(&Stores::new());
        // Opened as a plugin can (the `startMinigame` command is refused under the player's rules).
        let open = farm_sim::schema::PluginMutation::StartMinigame { minigame_id: "fishing".into() };
        minigame.run_command(&farm_sim::Command::PluginMutation { plugin_id: "shots".into(), mutation: open }).unwrap();
        write("minigame", minigame, width, height);
    }
    // Slots and credits with a save on disk.
    let mut saved = new_game(&stores);
    press(&mut saved, "z");
    press(&mut saved, "escape");
    common::click(&mut saved, farm_ui::WidgetId::new("pause").with("Save"));
    write("save-slots", saved, 1280, 800);
    let mut title = stores.standalone(&common::starter());
    common::idle(&mut title, 1);
    write("title-with-save", title, 1280, 800);
    let mut credits = stores.standalone(&common::starter());
    common::idle(&mut credits, 1);
    common::click(&mut credits, farm_ui::WidgetId::new("title").with("Credits"));
    write("credits", credits, 1280, 800);
    for tab in ["Audio", "Controls", "Accessibility"] {
        let mut settings = stores.standalone(&common::starter());
        common::idle(&mut settings, 1);
        common::click(&mut settings, farm_ui::WidgetId::new("title").with("Settings"));
        common::click(&mut settings, farm_ui::WidgetId::new("settings-tab").with(tab));
        write(&format!("settings-{}", tab.to_lowercase()), settings, 1280, 800);
    }
    let mut large = new_game(&Stores::new());
    let mut settings = large.settings().clone();
    settings.accessibility.text_size = 1.4;
    large.set_settings(settings);
    write("hud-large-text", large, 1280, 800);
    let mut pad = new_game(&Stores::new());
    let button =
        |pressed| farm_player::InputEvent::GamepadButton { button: farm_player::GamepadButton::Select, pressed };
    pad.step(FRAME, &[button(true)], 1280, 800).unwrap();
    pad.step(FRAME, &[button(false)], 1280, 800).unwrap();
    write("gamepad-inventory", pad, 1280, 800);
    // Building the farm, goals and combat (M5–M7).
    for name in ["farm-world", "storage", "shipping-bin", "builder", "placement", "goals", "mail", "creator-layouts"] {
        write(name, farm_scene(name), 1280, 800);
    }
}

/// The starter farm with a chest, a shipping bin, a gate, a builder (coop, shed), chickens,
/// collections, a bundle, achievements, letters, combat and a slime (M5–M7).
fn farm_project() -> farm_sim::GameProject {
    use farm_sim::schema::*;
    let mut project = common::starter();
    let object = |id: &str, name: &str| MachineTypeDefinition {
        id: id.into(),
        name: name.into(),
        color: "#8a5a2b".into(),
        ..MachineTypeDefinition::default()
    };
    project.machine_types.push(MachineTypeDefinition { storage: Some(12), ..object("chest", "Chest") });
    project.machine_types.push(MachineTypeDefinition { shipping: true, ..object("bin", "Shipping Bin") });
    project.machine_types.push(MachineTypeDefinition { gate: true, ..object("gate", "Gate") });
    let mut inside = project.scenes[0].clone();
    inside.id = "coop-inside".into();
    inside.name = "Coop".into();
    project.scenes.push(inside);
    project.building_types = Some(vec![
        BuildingDefinition {
            id: "coop".into(),
            name: "Coop".into(),
            description: "Houses chickens.".into(),
            width: 3,
            height: 2,
            color: "#c9a26b".into(),
            interior_scene_id: Some("coop-inside".into()),
            cost: 60,
            materials: vec![RecipeIngredient { item_id: "seed-wheat".into(), quantity: 2 }],
            capacity: 4,
            ..BuildingDefinition::default()
        },
        BuildingDefinition {
            id: "shed".into(),
            name: "Shed".into(),
            width: 2,
            height: 2,
            cost: 40,
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
        name: "Robin's Carpentry".into(),
        buildings: vec!["coop".into(), "shed".into()],
        animals: vec!["chicken".into()],
        ..ShopDefinition::default()
    });
    project.collections = Some(vec![CollectionDefinition {
        id: "museum".into(),
        name: "Seed Museum".into(),
        description: "Every seed of the valley.".into(),
        kind: "donated".into(),
        items: vec!["seed-wheat".into(), "seed-tomato".into(), "seed-corn".into(), "seed-pumpkin".into()],
        rewards: vec![CollectionReward {
            count: 2,
            message: "A golden watering can!".into(),
            ..CollectionReward::default()
        }],
        ..CollectionDefinition::default()
    }]);
    project.bundles = Some(vec![BundleDefinition {
        id: "spring".into(),
        name: "Spring Crops".into(),
        group: "Pantry".into(),
        items: vec![
            BundleItem { item_id: "seed-wheat".into(), quantity: 5, quality: None },
            BundleItem { item_id: "crop-corn".into(), quantity: 2, quality: None },
        ],
        ..BundleDefinition::default()
    }]);
    project.achievements = Some(vec![
        AchievementDefinition {
            id: "first".into(),
            name: "Green Thumb".into(),
            description: "Plant a seed.".into(),
            ..AchievementDefinition::default()
        },
        AchievementDefinition {
            id: "secret".into(),
            name: "Night Owl".into(),
            hidden: true,
            ..AchievementDefinition::default()
        },
    ]);
    project.mail = Some(vec![MailDefinition {
        id: "welcome".into(),
        subject: "Welcome to the valley".into(),
        from: "npc-merchant".into(),
        text: "Dear {player},\n\nHere are a few seeds to get you started. Come by the shop any time!".into(),
        attachments: vec![RecipeIngredient { item_id: "seed-corn".into(), quantity: 5 }],
        money: 50,
        ..MailDefinition::default()
    }]);
    project.settings.combat = Some(CombatSettings { enabled: true, ..CombatSettings::default() });
    project.monsters =
        Some(vec![MonsterDefinition { id: "slime".into(), name: "Slime".into(), ..MonsterDefinition::default() }]);
    project
}

/// A screen of [`farm_project`] for a look.
fn farm_scene(name: &str) -> Player {
    use farm_sim::schema::*;
    let stores = Stores::new();
    let project = farm_project();
    if name == "creator-layouts" {
        let mut project = project;
        let mut river = project.scenes[0].clone();
        river.id = "farm-river".into();
        project.scenes.push(river);
        let layout = |id: &str, name: &str, description: &str, scene: &str| FarmLayout {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            scene_id: scene.into(),
        };
        project.settings.farm = Some(FarmSettings {
            scene_id: project.scenes[0].id.clone(),
            layouts: vec![
                layout("classic", "Standard Farm", "Wide open fields for crops.", &project.scenes[0].id),
                layout("river", "Riverland", "Less room to plant, but great fishing.", "farm-river"),
            ],
            ..FarmSettings::default()
        });
        let mut player = stores.standalone(&project);
        common::idle(&mut player, 1);
        press(&mut player, "enter");
        return player;
    }
    let mut player = common::new_game_of(&stores, &project);
    let mut state = player.state().unwrap().clone();
    let scene = state.player.scene_id.clone();
    let (px, py) = (farm_sim::units::tile_of(state.player.x), farm_sim::units::tile_of(state.player.y));
    let farm = state.world.scenes.iter_mut().find(|candidate| candidate.id == scene).unwrap();
    let mut place = |x: i32, y: i32, type_id: &str, open: bool| {
        if let Some(tile) = farm.tile_mut(x, y) {
            tile.machine =
                Some(TileMachine { type_id: type_id.into(), open: open.then_some(true), ..TileMachine::default() });
        }
    };
    place(px - 1, py - 1, "chest", false);
    place(px + 1, py - 1, "bin", false);
    place(px - 3, py, "gate", true);
    place(px - 4, py, "gate", false);
    state.player.health = Some(64);
    state.mailbox.push(MailState { mail_id: "welcome".into(), day: 1, read: false, taken: false });
    state.achievements.insert("first".into(), 1);
    let item = |id: &str| state.player.inventory.iter().find(|slot| slot.item.id == id).cloned();
    match name {
        "farm-world" => {
            state.buildings.push(BuildingState {
                id: "building-1".into(),
                building_id: "coop".into(),
                scene_id: scene.clone(),
                x: px - 5,
                y: py - 5,
                interior_scene_id: "coop-inside".into(),
                ..BuildingState::default()
            });
            state.buildings.push(BuildingState {
                id: "building-2".into(),
                building_id: "shed".into(),
                scene_id: scene.clone(),
                x: px + 2,
                y: py - 5,
                days_left: 1,
                ..BuildingState::default()
            });
            let tile = farm_sim::units::TILE;
            for (dx, health, direction) in [(2, 10, "left"), (3, 4, "")] {
                state.monsters.push(MonsterState {
                    id: dx as u32,
                    monster_id: "slime".into(),
                    scene_id: scene.clone(),
                    x: (px + dx) * tile + tile / 2,
                    y: (py + 1) * tile + tile / 2,
                    health,
                    direction: direction.into(),
                    ..MonsterState::default()
                });
            }
        }
        "storage" | "shipping-bin" => {
            let (x, y) = if name == "storage" { (px - 1, py - 1) } else { (px + 1, py - 1) };
            if let Some(machine) = state
                .world
                .scenes
                .iter_mut()
                .find(|s| s.id == scene)
                .and_then(|s| s.tile_mut(x, y))
                .and_then(|t| t.machine.as_mut())
            {
                machine.contents.extend(item("seed-tomato"));
                machine.contents.extend(item("snack-trail-mix"));
            }
            state.storage = Some(StorageSession { scene_id: scene.clone(), x, y });
        }
        "builder" | "placement" => {
            state.buildings.push(BuildingState {
                id: "building-1".into(),
                building_id: "coop".into(),
                scene_id: scene.clone(),
                x: px - 5,
                y: py - 5,
                interior_scene_id: "coop-inside".into(),
                ..BuildingState::default()
            });
            state.shop = Some(ShopSession { shop_id: "builder".into() });
        }
        _ => {}
    }
    player.replace_state(state).unwrap();
    common::idle(&mut player, 2);
    match name {
        "placement" => common::click(&mut player, farm_ui::WidgetId::new("shop-build").with("shed")),
        "goals" => press(&mut player, "g"),
        "mail" => {
            press(&mut player, "m");
            common::click(&mut player, farm_ui::WidgetId::new("mail-letter").with(0usize));
        }
        _ => {}
    }
    player
}
