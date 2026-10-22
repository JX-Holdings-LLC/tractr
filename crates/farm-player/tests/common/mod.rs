//! Shared helpers of the player tests: fixtures, players with in-memory stores, input shortcuts.
#![allow(dead_code)]

use farm_player::{InputEvent, MemorySaveStore, MemorySettingsStore, Player, PlayerMode, PlayerOptions};
use farm_sim::schema::{DialogueState, ShopSession};
use farm_sim::GameProject;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

pub const FRAME: f64 = 1.0 / 60.0;
/// Layout size of stepped (unrendered) frames.
pub const SIZE: (u32, u32) = (1280, 800);

pub fn root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

pub fn project(name: &str) -> GameProject {
    let path = root().join("fixtures").join("golden").join("content").join(format!("{name}.json"));
    let fixture: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    serde_json::from_value(fixture["project"].clone()).unwrap()
}

pub fn starter() -> GameProject {
    project("starter-farm")
}

pub fn cartridge() -> Vec<u8> {
    std::fs::read(root().join("fixtures").join("golden").join("cartridges").join("fixture-project.cart")).unwrap()
}

/// Stores a test keeps handles to.
#[derive(Clone, Default)]
pub struct Stores {
    pub saves: MemorySaveStore,
    pub settings: MemorySettingsStore,
    pub clock: Arc<AtomicI64>,
}

impl Stores {
    pub fn new() -> Self {
        let stores = Self::default();
        stores.clock.store(1_790_000_000, Ordering::SeqCst);
        stores
    }

    pub fn options(&self, mode: PlayerMode) -> PlayerOptions {
        let clock = Arc::clone(&self.clock);
        PlayerOptions {
            mode,
            seed: Some("player-test".into()),
            saves: Box::new(self.saves.clone()),
            settings: Box::new(self.settings.clone()),
            // Every read moves a minute on, so saves are ordered.
            clock: Box::new(move || clock.fetch_add(60, Ordering::SeqCst)),
            can_quit: mode == PlayerMode::Standalone,
            ..PlayerOptions::standalone()
        }
    }

    pub fn standalone(&self, project: &GameProject) -> Player {
        Player::from_project(project.clone(), self.options(PlayerMode::Standalone)).unwrap()
    }
}

pub fn key_down(key: &str) -> InputEvent {
    InputEvent::KeyDown { key: key.into(), repeat: false }
}

pub fn key_up(key: &str) -> InputEvent {
    InputEvent::KeyUp { key: key.into() }
}

/// Steps `count` frames without input.
pub fn idle(player: &mut Player, count: usize) {
    for _ in 0..count {
        player.step(FRAME, &[], SIZE.0, SIZE.1).unwrap();
    }
}

/// Presses and releases a key over two frames, then lets two frames pass.
pub fn press(player: &mut Player, key: &str) {
    player.step(FRAME, &[key_down(key)], SIZE.0, SIZE.1).unwrap();
    player.step(FRAME, &[key_up(key)], SIZE.0, SIZE.1).unwrap();
    idle(player, 2);
}

/// Holds a key for `frames` frames.
pub fn hold(player: &mut Player, key: &str, frames: usize) {
    player.step(FRAME, &[key_down(key)], SIZE.0, SIZE.1).unwrap();
    idle(player, frames);
    player.step(FRAME, &[key_up(key)], SIZE.0, SIZE.1).unwrap();
}

/// Clicks the center of a widget drawn last frame, then lets two frames pass (a modal that
/// just opened settles to its content's size on its second frame).
pub fn click(player: &mut Player, id: farm_ui::WidgetId) {
    let rect = player.widget_rect(id).unwrap_or_else(|| panic!("widget {id:?} was not drawn"));
    let (x, y) = (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
    player
        .step(
            FRAME,
            &[InputEvent::PointerMove { x, y }, InputEvent::PointerDown { x, y, button: Default::default() }],
            SIZE.0,
            SIZE.1,
        )
        .unwrap();
    player.step(FRAME, &[InputEvent::PointerUp { x, y, button: Default::default() }], SIZE.0, SIZE.1).unwrap();
    idle(player, 2);
}

pub fn new_game(stores: &Stores) -> Player {
    new_game_of(stores, &starter())
}

pub fn new_game_of(stores: &Stores, project: &farm_sim::GameProject) -> Player {
    let mut player = stores.standalone(project);
    idle(&mut player, 1);
    press(&mut player, "enter");
    assert_eq!(player.screen(), farm_player::ScreenKind::Playing);
    player
}

/// Sets up one of the screenshot screens (`tests/screenshots.rs`); returns the player to render.
pub fn scene(name: &str) -> Player {
    let stores = Stores::new();
    match name {
        "title" => stores.standalone(&starter()),
        "settings" => {
            let mut player = stores.standalone(&starter());
            // Focus starts on New Game; Continue and Load are disabled without saves, so the
            // next entry down is Settings.
            idle(&mut player, 1);
            press(&mut player, "arrowdown");
            press(&mut player, "enter");
            assert_eq!(player.screen(), farm_player::ScreenKind::Settings);
            player
        }
        "hud" => new_game(&stores),
        "dialogue" => {
            let mut player = new_game(&stores);
            let mut state = player.state().unwrap().clone();
            state.dialogue =
                Some(DialogueState { npc_id: "npc-merchant".into(), dialogue_id: "dialogue-merchant-greeting".into() });
            player.replace_state(state).unwrap();
            player
        }
        "shop" => {
            let mut player = new_game(&stores);
            let mut state = player.state().unwrap().clone();
            state.shop = Some(ShopSession { shop_id: "shop-general".into() });
            player.replace_state(state).unwrap();
            player
        }
        // The sample games (compiled by the authoring core): each starts on its own map.
        "sample-cozy" | "sample-quest" => {
            let file = if name == "sample-cozy" { "cozy-garden.cart" } else { "quest-rpg.cart" };
            let path = root().join("fixtures").join("golden").join("cartridges").join(file);
            let bytes = std::fs::read(path).unwrap();
            let mut player = Player::from_cartridge_bytes(&bytes, stores.options(PlayerMode::Standalone)).unwrap();
            idle(&mut player, 1);
            press(&mut player, "enter");
            if name == "sample-quest" {
                // Through the farm's south gate to the village square.
                let mut state = player.state().unwrap().clone();
                state.player.scene_id = "scene-square".into();
                state.player.x = farm_sim::units::tile_center(8);
                state.player.y = farm_sim::units::tile_center(5);
                player.replace_state(state).unwrap();
            }
            player
        }
        // A pack-defined minigame (the example pack Lantern Night) a few frames in.
        "pack-minigame" => {
            let mut project = starter();
            let pack = root().join("fixtures").join("projects").join("lantern-night-mod.json");
            let pack = serde_json::from_str(&std::fs::read_to_string(pack).unwrap()).unwrap();
            project.content_packs.push(farm_sim::schema::PackInstallation { pack, enabled: true });
            let mut player = new_game_of(&stores, &project);
            let open = farm_sim::schema::PluginMutation::StartMinigame { minigame_id: "lantern-night:catch".into() };
            player
                .run_command(&farm_sim::Command::PluginMutation { plugin_id: "shots".into(), mutation: open })
                .unwrap();
            assert!(player.plugin_errors().is_empty(), "{:?}", player.plugin_errors());
            player
        }
        other => panic!("unknown scene {other}"),
    }
}
