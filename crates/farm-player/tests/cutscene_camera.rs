//! A cutscene's camera (M2) moves the player's view: the world is drawn around where the
//! cutscene looks instead of around the player.

mod common;

use common::{idle, new_game_of, starter, Stores, FRAME, SIZE};
use farm_player::Player;
use farm_render::DrawCmd;
use farm_sim::schema::{CutsceneCamera, CutsceneState};
use farm_sim::units;

/// A small frame, so the camera has room to move in the starter farm.
const VIEW: (u32, u32) = (320, 200);

/// The world's camera offset in the next frame (its first translation).
fn camera_offset(player: &mut Player) -> (f32, f32) {
    let (_, composition) = player.frame_composition(FRAME, &[], VIEW.0, VIEW.1).unwrap();
    let world = composition.world.expect("a world is drawn");
    world
        .list
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCmd::Translate { dx, dy } => Some((*dx, *dy)),
            _ => None,
        })
        .expect("the world is translated by its camera")
}

/// Holds the camera on tile (x, y) under a cutscene that waits on a line.
fn look_at(player: &mut Player, x: i32, y: i32) {
    let mut state = player.state().unwrap().clone();
    let (cx, cy) = (units::tile_center(x), units::tile_center(y));
    state.cutscene = Some(CutsceneState {
        cutscene_id: "look".into(),
        started: true,
        camera: Some(CutsceneCamera { from_x: cx, from_y: cy, to_x: cx, to_y: cy, ..CutsceneCamera::default() }),
        ..CutsceneState::default()
    });
    player.replace_state(state).unwrap();
}

#[test]
fn the_cutscene_camera_is_the_follow_target() {
    let mut project = starter();
    project.cutscenes = Some(vec![serde_json::from_value(
        serde_json::json!({"id": "look", "steps": [{"type": "say", "text": "Look over there."}]}),
    )
    .unwrap()]);
    let stores = Stores::new();
    let mut player = new_game_of(&stores, &project);
    let follow = camera_offset(&mut player);

    let scene = player.session().unwrap().current_scene().unwrap().clone();
    look_at(&mut player, 0, 0);
    let corner = camera_offset(&mut player);
    assert_eq!(corner, (0.0, 0.0), "the top-left corner: the camera stops at the world's edge");
    look_at(&mut player, scene.width - 1, scene.height - 1);
    let far = camera_offset(&mut player);
    assert!(far.0 <= follow.0 && far.1 <= follow.1 && far != follow, "far corner {far:?}, player {follow:?}");

    // When the cutscene ends the camera follows the player again.
    let mut state = player.state().unwrap().clone();
    state.cutscene = None;
    player.replace_state(state).unwrap();
    assert_eq!(camera_offset(&mut player), follow);
}

#[test]
fn a_pan_moves_on_between_ticks() {
    let mut project = starter();
    project.cutscenes = Some(vec![serde_json::from_value(serde_json::json!({"id": "pan", "steps": [
        {"type": "camera", "x": 2, "y": 2, "seconds": 3},
        {"type": "say", "text": "Look over there."},
    ]}))
    .unwrap()]);
    let stores = Stores::new();
    let mut player = new_game_of(&stores, &project);
    let mut state = player.state().unwrap().clone();
    state.cutscene = Some(CutsceneState { cutscene_id: "pan".into(), ..CutsceneState::default() });
    player.replace_state(state).unwrap();
    // The pan starts on the first tick and moves from the second.
    idle(&mut player, 9);

    // Three frames to a tick: the camera moves on every frame, not only when a tick falls in it.
    let mut seen = Vec::new();
    for _ in 0..12 {
        player.step(FRAME, &[], SIZE.0, SIZE.1).unwrap();
        let session = player.session().unwrap();
        let (x, y) = session.interpolated_camera().expect("the cutscene's camera");
        let (tick_x, tick_y) = farm_sim::cutscenes::camera(session.state()).unwrap();
        // Never past the tick's camera (the pan heads up and left, onto tile 2, 2).
        assert!(x >= units::position_to_tiles(tick_x) && y >= units::position_to_tiles(tick_y));
        seen.push((x, y));
    }
    assert!(seen.windows(2).all(|pair| pair[1].0 < pair[0].0 && pair[1].1 < pair[0].1), "{seen:?}");
}
