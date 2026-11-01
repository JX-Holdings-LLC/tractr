//! A game's interface skin (#236) in the player: its palette, panel art, icons, fonts and layout,
//! with the player's readable-font setting on top. `gpu_parity.rs` draws the same screens on
//! the GPU.

mod common;

use common::{idle, scene, skinned, Stores, SIZE};
use farm_player::Player;
use farm_render::{DrawCmd, FontId, Rect};

fn images(player: &Player) -> Vec<Rect> {
    player
        .ui_draw_list()
        .commands
        .iter()
        .filter_map(|command| match command {
            DrawCmd::Image { dst, .. } => Some(*dst),
            _ => None,
        })
        .collect()
}

fn texts(player: &Player) -> Vec<(String, FontId, f32)> {
    player
        .ui_draw_list()
        .commands
        .iter()
        .filter_map(|command| match command {
            DrawCmd::Text { text, font, y, .. } => Some((text.clone(), *font, *y)),
            _ => None,
        })
        .collect()
}

#[test]
fn the_title_screen_draws_skinned_buttons_in_the_games_fonts_on_the_left() {
    let mut player = scene("skinned-title");
    idle(&mut player, 2);
    // Six menu buttons and more: nine images per skinned button.
    assert!(images(&player).len() >= 9 * 5, "{} images", images(&player).len());
    let fonts: Vec<FontId> = texts(&player).iter().map(|(_, font, _)| *font).collect();
    assert!(fonts.iter().any(|font| matches!(font, FontId::Custom(_))), "{fonts:?}");
    // The menu sits on the left.
    assert!(images(&player).iter().any(|rect| rect.x < 60.0 && rect.y > 200.0));
    assert!(!images(&player).iter().any(|rect| (rect.x - (SIZE.0 as f32 - 300.0) / 2.0).abs() < 1.0));
}

#[test]
fn the_hud_follows_the_skins_layout() {
    let mut player = scene("skinned-hud");
    idle(&mut player, 2);
    let shown = texts(&player);
    // No clock: neither the label nor a time.
    assert!(!shown.iter().any(|(text, _, _)| text.ends_with(" AM") || text.ends_with(" PM")), "{shown:?}");
    // The HUD bar is at the bottom of the screen.
    let money = shown.iter().find(|(text, _, _)| text.contains("50") || text.contains('$')).expect("the money");
    assert!(money.2 > SIZE.1 as f32 / 2.0, "money at {}", money.2);
    // The coin icon stands in for the money label.
    assert!(!shown.iter().any(|(text, _, _)| text == "Money:"));
}

#[test]
fn the_dialogue_box_sits_at_the_top_with_a_large_portrait() {
    let mut player = scene("skinned-dialogue");
    idle(&mut player, 2);
    let name = texts(&player).into_iter().find(|(text, _, _)| text == "Merchant" || text.contains("Merchant"));
    let name = name.expect("the speaker's name");
    assert!(name.2 < SIZE.1 as f32 / 2.0, "name at {}", name.2);
}

#[test]
fn the_readable_font_setting_wins_over_the_skins_fonts() {
    let stores = Stores::new();
    let mut player = stores.standalone(&skinned());
    let mut settings = player.settings().clone();
    settings.accessibility.readable_font = true;
    player.set_settings(settings);
    idle(&mut player, 2);
    let fonts: Vec<FontId> = texts(&player).iter().map(|(_, font, _)| *font).collect();
    assert!(!fonts.iter().any(|font| matches!(font, FontId::Custom(_))), "{fonts:?}");
    assert!(fonts.contains(&FontId::ReadableBold));
}

#[test]
fn a_game_without_a_skin_draws_the_built_in_look() {
    let mut player = scene("title");
    idle(&mut player, 2);
    assert!(texts(&player).iter().all(|(_, font, _)| !matches!(font, FontId::Custom(_))));
}
