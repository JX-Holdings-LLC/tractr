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
        "skinned-title" => stores.standalone(&skinned()),
        "skinned-hud" => new_game_of(&stores, &skinned()),
        "skinned-dialogue" => {
            let mut player = new_game_of(&stores, &skinned());
            let mut state = player.state().unwrap().clone();
            state.dialogue = Some(DialogueState {
                npc_id: "npc-merchant".into(),
                dialogue_id: "dialogue-merchant-greeting".into(),
                line: None,
            });
            player.replace_state(state).unwrap();
            player
        }
        "dialogue" => {
            let mut player = new_game(&stores);
            let mut state = player.state().unwrap().clone();
            state.dialogue = Some(DialogueState {
                npc_id: "npc-merchant".into(),
                dialogue_id: "dialogue-merchant-greeting".into(),
                line: None,
            });
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

/// Standard base64 of `bytes` (data URLs of the skin fixtures).
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// A 12×12 PNG: a `border` frame 3 pixels wide around a `fill` middle (9-slice panel art).
pub fn panel_png(border: [u8; 4], fill: [u8; 4]) -> String {
    let mut pixmap = farm_render::tiny_skia::Pixmap::new(12, 12).unwrap();
    for (i, pixel) in pixmap.data_mut().chunks_exact_mut(4).enumerate() {
        let (x, y) = (i % 12, i / 12);
        let edge = !(3..9).contains(&x) || !(3..9).contains(&y);
        pixel.copy_from_slice(if edge { &border } else { &fill });
    }
    format!("data:image/png;base64,{}", base64(&farm_render::encode_png(&pixmap)))
}

/// The starter farm with an interface skin (#236): a palette, 9-slice panels and buttons, a
/// money icon, the Atkinson Hyperlegible files as the game's fonts, the HUD at the bottom
/// without a clock, the dialogue box at the top with a large portrait, the menu on the left.
pub fn skinned() -> GameProject {
    use farm_sim::schema::{CustomAsset, UiLayout, UiNineSlice, UiSkin, UiTitleScreen, VisualRef};
    let mut project = starter();
    let asset = |id: &str, kind: &str, url: String| CustomAsset {
        id: id.into(),
        name: id.into(),
        r#type: kind.into(),
        data_url: url,
        ..CustomAsset::default()
    };
    let font = |file: &str| {
        let bytes = std::fs::read(root().join("assets").join("fonts").join(file)).unwrap();
        format!("data:font/ttf;base64,{}", base64(&bytes))
    };
    project.custom_assets.push(asset("skin-panel", "art", panel_png([90, 50, 20, 255], [240, 220, 180, 255])));
    project.custom_assets.push(asset("skin-button", "art", panel_png([40, 90, 40, 255], [120, 200, 120, 255])));
    project.custom_assets.push(asset("skin-coin", "art", panel_png([200, 160, 0, 255], [255, 220, 0, 255])));
    project.custom_assets.push(asset("font-body", "font", font("AtkinsonHyperlegible-Regular.ttf")));
    project.custom_assets.push(asset("font-heading", "font", font("AtkinsonHyperlegible-Bold.ttf")));
    let slice = |id: &str| UiNineSlice {
        visual: VisualRef { asset_id: id.into(), ..VisualRef::default() },
        left: 3.0,
        top: 3.0,
        right: 3.0,
        bottom: 3.0,
        scale: Some(2.0),
    };
    let mut panels = indexmap::IndexMap::new();
    for part in ["panel", "hud", "dialogue", "slot", "tooltip"] {
        panels.insert(part.to_owned(), slice("skin-panel"));
    }
    panels.insert("button".to_owned(), slice("skin-button"));
    let mut icons = indexmap::IndexMap::new();
    icons.insert("money".to_owned(), VisualRef { asset_id: "skin-coin".into(), ..VisualRef::default() });
    let mut colors = indexmap::IndexMap::new();
    colors.insert("text".to_owned(), "#2a1a0a".to_owned());
    colors.insert("muted".to_owned(), "#5a4020".to_owned());
    project.ui_skin = Some(UiSkin {
        heading_font: Some("font-heading".into()),
        body_font: Some("font-body".into()),
        colors: Some(colors),
        panels: Some(panels),
        icons: Some(icons),
        layout: Some(UiLayout {
            hud_position: Some("bottom".into()),
            toolbar: Some(true),
            clock_style: Some("hidden".into()),
            dialogue_position: Some("top".into()),
            portrait_size: Some(72.0),
        }),
        title: Some(UiTitleScreen { menu_position: Some("left".into()), ..UiTitleScreen::default() }),
        ..UiSkin::default()
    });
    project
}
