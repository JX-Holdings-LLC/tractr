//! The player character (M4) in the play snapshot and the world draw list: the character
//! creator's colours and worn clothes recolour the built-in art, a worn hat's art sits on the
//! head, and the spouse's children stand beside them, smaller. Without any of it nothing
//! changes (the render goldens pin that).

use farm_render::{build_world, shell_snapshot, BuiltinArt, DrawCmd, ImageStore, SnapshotOptions, WorldSnapshot};
use farm_sim::schema::{ChildState, GameState, NpcSocialState, PlayerLook};
use farm_sim::{GameContent, GameProject};

const TS: f64 = 32.0;

fn game() -> (GameContent, GameState) {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/golden/content/starter-farm.json")).unwrap();
    let project: GameProject = serde_json::from_value(fixture["project"].clone()).unwrap();
    let content = farm_sim::create_content_from_project(&project);
    let state = farm_sim::create_game_state(&project, Some("render-look"));
    (content, state)
}

fn snapshot(content: &GameContent, state: &GameState) -> WorldSnapshot {
    let scene = state.world.scenes.iter().find(|scene| scene.id == state.player.scene_id).unwrap();
    shell_snapshot(content, state, scene, &SnapshotOptions { tile_size: TS, padding: 0.0, ..Default::default() })
}

/// The image sources the world draws.
fn sources(snapshot: &WorldSnapshot, images: &mut ImageStore) -> Vec<String> {
    let list = build_world(snapshot, Some(BuiltinArt::embedded()), images);
    list.commands
        .iter()
        .filter_map(|command| match command {
            DrawCmd::Image { image, .. } => images.source(*image).map(str::to_owned),
            _ => None,
        })
        .collect()
}

#[test]
fn colours_recolour_the_built_in_player() {
    let (content, mut state) = game();
    let mut images = ImageStore::default();
    assert!(snapshot(&content, &state).player.look.is_none());
    assert!(!sources(&snapshot(&content, &state), &mut images).iter().any(|source| source.contains("#look=")));

    state.player.look = Some(PlayerLook { shirt: Some("#20c040".into()), ..PlayerLook::default() });
    let world = snapshot(&content, &state);
    assert_eq!(world.player.look.as_deref(), Some("-,-,20c040,-,-"));
    let looked = sources(&world, &mut images).into_iter().find(|source| source.contains("#look=")).unwrap();
    let plain = images.decode("builtin://characters.png").unwrap();
    let recoloured = images.decode(&looked).unwrap();
    assert_eq!((plain.width(), plain.height()), (recoloured.width(), recoloured.height()));
    // The shirt's main shade became the chosen green; the outline did not change.
    let at = |image: &farm_render::Image, x: u32, y: u32| {
        let i = ((y * image.width() + x) * 4) as usize;
        [image.pixels()[i], image.pixels()[i + 1], image.pixels()[i + 2]]
    };
    let shirt =
        (0..48 * 4).flat_map(|y| (0..128).map(move |x| (x, y))).find(|&(x, y)| at(&plain, x, y) == [0x4f, 0x9b, 0xe0]);
    let (x, y) = shirt.expect("the player's shirt");
    assert_eq!(at(&recoloured, x, y), [0x20, 0xc0, 0x40]);
    // NPC art outside the player's frames is untouched.
    assert_eq!(at(&recoloured, 140, 30), at(&plain, 140, 30));
}

#[test]
fn children_stand_beside_the_spouse() {
    let (content, mut state) = game();
    let (spouse, before) = {
        let (id, npc) = state.npcs.iter().find(|(_, npc)| npc.scene_id == state.player.scene_id).unwrap();
        (id.clone(), npc.clone())
    };
    state.social.insert(
        spouse.clone(),
        NpcSocialState { status: Some("married".into()), status_day: Some(1), ..NpcSocialState::default() },
    );
    state.children.push(ChildState { name: "Pip".into(), parent_id: spouse, born_day: 1 });
    let world = snapshot(&content, &state);
    let child = world.npcs.iter().find(|entity| entity.scale.is_some()).expect("a child");
    assert_eq!(child.scale, Some(farm_render::shell::CHILD_SCALE));
    assert_eq!(child.y, farm_sim::units::position_to_tiles(before.y));
}
