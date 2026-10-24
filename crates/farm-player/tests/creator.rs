//! The character creator (M4): a game that asks for it opens it on New Game, the game waits
//! behind it, and Start names and colours the player through `setPlayerLook`.

mod common;

use common::{click, idle, press, Stores};
use farm_player::ScreenKind;
use farm_ui::WidgetId;

#[test]
fn new_game_opens_the_creator_and_start_names_the_player() {
    let mut project = common::starter();
    project.settings.character_creator = Some(true);
    project.player.name = Some("Rowan".into());
    let stores = Stores::new();
    let mut player = stores.standalone(&project);
    idle(&mut player, 1);
    press(&mut player, "enter");
    assert_eq!(player.screen(), ScreenKind::Creator);
    idle(&mut player, 10);
    assert_eq!(player.state().unwrap().clock.tick, 0, "the game waits behind the creator");

    // Type a farm name, pick a shirt, start.
    click(&mut player, WidgetId::new("creator-field").with(1usize));
    for key in ["d", "e", "w", " ", "f", "a", "r", "m"] {
        press(&mut player, key);
    }
    press(&mut player, "enter");
    click(&mut player, WidgetId::new("creator-swatch").with(2usize).with(1usize));
    click(&mut player, WidgetId::new("creator-start"));
    assert_eq!(player.screen(), ScreenKind::Playing);
    let state = player.state().unwrap();
    assert_eq!(state.player.name, "Rowan");
    assert_eq!(state.player.farm_name, "Dew Farm");
    assert_eq!(state.player.look.as_ref().and_then(|look| look.shirt.as_deref()), Some("#c23a3a"));
}

#[test]
fn games_without_the_creator_start_at_once() {
    let stores = Stores::new();
    let player = common::new_game(&stores);
    assert_eq!(player.screen(), ScreenKind::Playing);
}
