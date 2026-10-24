//! Cutscenes (M2) in the play snapshot and the world draw list: NPCs turned and walked by a
//! cutscene, and emote bubbles over characters. Without a cutscene nothing changes (the render
//! goldens pin that).

use farm_render::{
    apply_graphics, build_world, shell_snapshot, DrawCmd, GraphicsSource, ImageStore, SnapshotOptions, WorldSnapshot,
};
use farm_sim::schema::{
    AnimationClip, ArtFrame, CustomAsset, CutsceneEmote, CutsceneState, CutsceneWalk, GameState, GridPoint, VisualRef,
};
use farm_sim::{units, GameContent, GameProject, Presentation};

const TS: f64 = 32.0;

fn starter() -> GameProject {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/golden/content/starter-farm.json")).unwrap();
    serde_json::from_value(fixture["project"].clone()).unwrap()
}

/// The starter farm with a cutscene `c` (one waiting line) and the id of an NPC in the
/// player's scene.
fn game() -> (GameContent, GameState, String) {
    let project = starter();
    let mut content = farm_sim::create_content_from_project(&project);
    content.cutscenes.push(
        serde_json::from_value(serde_json::json!({"id": "c", "steps": [{"type": "say", "text": "..."}]})).unwrap(),
    );
    let state = farm_sim::create_game_state(&project, Some("render-cutscenes"));
    let npc = state.npcs.iter().find(|(_, npc)| npc.scene_id == state.player.scene_id).unwrap().0.clone();
    (content, state, npc)
}

fn snapshot(content: &GameContent, state: &GameState) -> WorldSnapshot {
    let scene = state.world.scenes.iter().find(|scene| scene.id == state.player.scene_id).unwrap();
    shell_snapshot(content, state, scene, &SnapshotOptions { tile_size: TS, padding: 0.0, ..Default::default() })
}

fn playing(state: &mut GameState) -> &mut CutsceneState {
    state.cutscene.insert(CutsceneState { cutscene_id: "c".into(), started: true, ..CutsceneState::default() })
}

#[test]
fn a_cutscene_turns_and_walks_npcs() {
    let (content, mut state, npc) = game();
    let index = |snapshot: &WorldSnapshot| snapshot.npcs.iter().position(|entity| entity.kind == "npc").unwrap();
    let before = snapshot(&content, &state);
    let at = index(&before);
    assert_eq!((before.npcs[at].direction.as_str(), before.npcs[at].moving), ("down", false));
    assert!(before.emotes.is_none());

    // Turned by a `face` step.
    let first = content.npcs.iter().find(|def| state.npcs.contains_key(&def.id)).unwrap().id.clone();
    assert_eq!(first, npc, "the first NPC of the scene");
    playing(&mut state).facing.insert(npc.clone(), "left".to_owned());
    let turned = snapshot(&content, &state);
    assert_eq!((turned.npcs[at].direction.as_str(), turned.npcs[at].moving), ("left", false));
    assert_eq!(turned.npcs[at].pixel_x, None, "standing on its tile");

    // Walking right, a quarter of the way into the next tile.
    let live = state.npcs.get_mut(&npc).unwrap();
    live.x += units::TILE / 4;
    let (x, y) = (units::position_to_tiles(live.x), units::position_to_tiles(live.y));
    let target = GridPoint { x: units::tile_of(live.x) + 1, y: units::tile_of(live.y) };
    let scene_id = state.player.scene_id.clone();
    let running = playing(&mut state);
    running.facing.insert(npc.clone(), "right".to_owned());
    running.walks = vec![CutsceneWalk { actor: npc.clone(), scene_id, path: vec![target] }];
    let walking = snapshot(&content, &state);
    let entity = &walking.npcs[at];
    assert_eq!((entity.direction.as_str(), entity.moving), ("right", true));
    // Drawn from its tile corner, not read as a centre.
    assert_eq!((entity.pixel_x, entity.pixel_y), (Some(x * TS), Some(y * TS)));
}

#[test]
fn creator_art_npcs_turn_and_walk_like_the_player() {
    // One sheet whose clips sit side by side: idle down at x 0, idle left at 16, walk right at 32.
    let clip = |name: &str, x: f64| AnimationClip {
        name: name.into(),
        r#loop: true,
        frames: vec![ArtFrame { asset_id: None, x, y: 0.0, width: 16.0, height: 16.0, ticks: 1.0 }],
    };
    let mut project = starter();
    project.custom_assets.push(CustomAsset {
        id: "npc-art".into(),
        name: "NPC".into(),
        r#type: "art".into(),
        data_url: "data:image/png;base64,a".into(),
        animations: Some(vec![clip("idle-down", 0.0), clip("idle-left", 16.0), clip("walk-right", 32.0)]),
        ..CustomAsset::default()
    });
    let (mut content, mut state, npc) = game();
    let visual = Some(VisualRef { asset_id: "npc-art".into(), ..VisualRef::default() });
    for definition in project.npcs.iter_mut().filter(|definition| definition.id == npc) {
        definition.visual.clone_from(&visual);
    }
    for definition in content.npcs.iter_mut().filter(|definition| definition.id == npc) {
        definition.visual.clone_from(&visual);
    }
    let source = GraphicsSource::from_state(&Presentation::from_project(&project), &content, &state);
    let scene = state.world.scenes.iter().find(|scene| scene.id == state.player.scene_id).unwrap().clone();
    let drawn_x = |state: &GameState| {
        let mut shown = snapshot(&content, state);
        apply_graphics(&mut shown, &source, &scene, 0.0, false);
        let at = shown.npcs.iter().position(|entity| entity.kind == "npc").unwrap();
        shown.npcs[at].sprite.as_ref().and_then(|sprite| sprite.source_x)
    };
    assert_eq!(drawn_x(&state), Some(0.0), "standing, facing down");
    playing(&mut state).facing.insert(npc.clone(), "left".to_owned());
    assert_eq!(drawn_x(&state), Some(16.0), "turned left by a face step");
    let target = {
        let live = &state.npcs[&npc];
        GridPoint { x: units::tile_of(live.x) + 1, y: units::tile_of(live.y) }
    };
    let scene_id = state.player.scene_id.clone();
    let running = playing(&mut state);
    running.facing.insert(npc.clone(), "right".to_owned());
    running.walks = vec![CutsceneWalk { actor: npc.clone(), scene_id, path: vec![target] }];
    assert_eq!(drawn_x(&state), Some(32.0), "walking right");
}

#[test]
fn emotes_draw_a_bubble_over_the_character() {
    let (content, mut state, npc) = game();
    let plain = build_world(&snapshot(&content, &state), None, &mut ImageStore::default());

    let running = playing(&mut state);
    running.emotes = vec![
        CutsceneEmote { actor: npc.clone(), emote: "heart".into(), duration: 30, elapsed: 15 },
        CutsceneEmote { actor: "player".into(), emote: "question".into(), duration: 30, elapsed: 0 },
    ];
    let shown = snapshot(&content, &state);
    let emotes = shown.emotes.as_ref().expect("emotes in the snapshot");
    assert_eq!(emotes.len(), 2);
    let live = &state.npcs[&npc];
    assert_eq!(
        (emotes[0].x, emotes[0].y, emotes[0].player, emotes[0].progress),
        (units::position_to_tiles(live.x) + 0.5, units::position_to_tiles(live.y) + 0.5, false, 0.5)
    );
    assert!(emotes[1].player && emotes[1].emote == "question");

    let list = build_world(&shown, None, &mut ImageStore::default());
    let texts: Vec<(&str, f32, f32, f32)> = list
        .commands
        .iter()
        .filter_map(|command| match command {
            DrawCmd::Text { text, x, y, size, .. } => Some((text.as_str(), *x, *y, *size)),
            _ => None,
        })
        .collect();
    let heart = texts.iter().find(|(text, ..)| *text == "\u{2665}" || *text == "<3").expect("the heart glyph");
    // Centred over the NPC, above its head.
    assert!((f64::from(heart.1) - emotes[0].x * TS).abs() < 0.01, "{heart:?}");
    assert!(f64::from(heart.2) < (emotes[0].y - 0.5) * TS, "{heart:?}");
    let question = texts.iter().find(|(text, ..)| *text == "?").expect("the question mark");
    assert!(question.3 < heart.3, "the player's bubble is still popping in");
    let bubbles = |list: &farm_render::DrawList| {
        list.commands.iter().filter(|command| matches!(command, DrawCmd::FillRoundRect { .. })).count()
    };
    assert_eq!(bubbles(&list), bubbles(&plain) + 2);
}
