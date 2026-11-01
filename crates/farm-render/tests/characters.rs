//! Character and object art sizes (#215), NPC looks (#216) and layered characters (#234): the
//! draw list's image rectangles, the composed-frame cache, and the pixels it draws. The GPU
//! renderer executes the same draw list (its parity tests cover images), so these hold for both.

use base64::Engine as _;
use farm_render::tiny_skia::Pixmap;
use farm_render::{
    apply_graphics, build_world, encode_png, shell_snapshot, BuiltinArt, Color, DrawCmd, GraphicsSource, ImageStore,
    Rect, SnapshotCrop, SnapshotEntity, SnapshotLayer, SnapshotOptions, SnapshotSprite, SnapshotTile, WorldRenderer,
    WorldSnapshot,
};
use farm_sim::schema::{CharacterLayer, CustomAsset, InventorySlot, Item, PlayerLook, WearDefinition};
use std::sync::Arc;

const TS: f64 = 32.0;

fn scene(width: usize) -> WorldSnapshot {
    WorldSnapshot {
        width: width as i32,
        height: 3,
        tile_size: TS,
        padding: 0.0,
        tile_gap: Some(0.0),
        player: SnapshotEntity { x: -10.0, y: -10.0, ..SnapshotEntity::default() },
        tiles: (0..3)
            .map(|_| {
                (0..width).map(|_| SnapshotTile { background: "grass".into(), ..SnapshotTile::default() }).collect()
            })
            .collect(),
        ..WorldSnapshot::default()
    }
}

fn data_url(width: u32, height: u32, color: Color) -> String {
    let mut pixmap = Pixmap::new(width, height).unwrap();
    for target in pixmap.pixels_mut() {
        *target = farm_render::tiny_skia::ColorU8::from_rgba(color.r, color.g, color.b, color.a).premultiply();
    }
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(encode_png(&pixmap)))
}

fn sprite(url: &str, width: f64, height: f64) -> SnapshotSprite {
    SnapshotSprite { image_url: Arc::from(url), frame_width: width, frame_height: height, ..SnapshotSprite::default() }
}

/// The destination rectangles of the image commands drawing `url`.
fn image_rects(list: &farm_render::DrawList, images: &ImageStore, url: &str) -> Vec<Rect> {
    list.commands
        .iter()
        .filter_map(|command| match command {
            DrawCmd::Image { image, dst, .. } if images.source(*image) == Some(url) => Some(*dst),
            _ => None,
        })
        .collect()
}

fn art() -> Option<&'static BuiltinArt> {
    Some(BuiltinArt::embedded())
}

#[test]
fn creator_characters_taller_than_a_tile_stand_at_the_built_in_scale() {
    let tall = data_url(32, 48, Color::rgb(255, 0, 255));
    let small = data_url(16, 16, Color::rgb(0, 255, 255));
    let mut snapshot = scene(3);
    snapshot.npcs.push(SnapshotEntity {
        x: 0.0,
        y: 1.0,
        sprite: Some(sprite(&tall, 32.0, 48.0)),
        ..Default::default()
    });
    snapshot.npcs.push(SnapshotEntity {
        x: 1.0,
        y: 1.0,
        sprite: Some(sprite(&small, 16.0, 16.0)),
        ..Default::default()
    });
    // The built-in player (32×48 sheet frames) beside them, for comparison.
    snapshot.player = SnapshotEntity { x: 2.0, y: 1.0, ..SnapshotEntity::default() };
    let mut images = ImageStore::default();
    let list = build_world(&snapshot, art(), &mut images);

    let creator = image_rects(&list, &images, &tall);
    assert_eq!(creator, vec![Rect::new(0.0, 16.0, 32.0, 48.0)], "a 32×48 NPC is 1.5 tiles tall");
    let player = image_rects(&list, &images, "builtin://characters.png");
    assert_eq!(player.len(), 1);
    assert_eq!((player[0].width, player[0].height), (32.0, 48.0), "the same size as the built-in player");
    assert_eq!(
        image_rects(&list, &images, &small),
        vec![Rect::new(32.0, 32.0, 32.0, 32.0)],
        "small art still fills the tile"
    );
}

#[test]
fn crop_frames_taller_than_a_tile_are_not_squeezed_and_scale_with_the_tile_size() {
    let frame = data_url(32, 48, Color::rgb(0, 200, 0));
    let mut snapshot = scene(1);
    snapshot.tile_size = 64.0;
    snapshot.tiles[1][0].crop =
        Some(SnapshotCrop { sprite: Some(sprite(&frame, 32.0, 48.0)), ..SnapshotCrop::default() });
    let mut images = ImageStore::default();
    let list = build_world(&snapshot, None, &mut images);
    // 64-pixel tiles: 2 world pixels per sheet pixel, centred, standing on the tile's bottom edge.
    assert_eq!(image_rects(&list, &images, &frame), vec![Rect::new(0.0, 128.0 - 96.0, 64.0, 96.0)]);
}

#[test]
fn pixels_per_tile_sets_the_scale_of_high_resolution_art() {
    let frame = data_url(64, 96, Color::rgb(200, 0, 0));
    let mut snapshot = scene(1);
    let hi_res = SnapshotSprite { pixels_per_tile: Some(64.0), ..sprite(&frame, 64.0, 96.0) };
    snapshot.npcs.push(SnapshotEntity { x: 0.0, y: 1.0, sprite: Some(hi_res), ..Default::default() });
    let mut images = ImageStore::default();
    let list = build_world(&snapshot, None, &mut images);
    assert_eq!(
        image_rects(&list, &images, &frame),
        vec![Rect::new(0.0, 16.0, 32.0, 48.0)],
        "64 px per tile: 1.5 tiles"
    );

    // The asset's `pixelsPerTile` reaches the sprite through the graphics pass.
    let asset = CustomAsset {
        id: "hi".into(),
        name: "hi".into(),
        r#type: "npc".into(),
        data_url: frame.clone(),
        pixels_per_tile: Some(64.0),
        ..CustomAsset::default()
    };
    let assets = farm_render::graphics::art_assets(&[asset]);
    let visual = farm_sim::schema::VisualRef { asset_id: "hi".into(), ..Default::default() };
    let resolved = farm_render::resolve_visual(&assets, Some(&visual), 0.0, "down", false).unwrap();
    assert_eq!(resolved.pixels_per_tile, Some(64.0));
}

#[test]
fn layered_characters_compose_one_cached_frame() {
    let body = data_url(32, 48, Color::rgb(200, 200, 200));
    let hat = data_url(16, 8, Color::rgb(255, 255, 255));
    let mut snapshot = scene(2);
    snapshot.npcs.push(SnapshotEntity {
        x: 0.0,
        y: 1.0,
        layers: Some(vec![
            SnapshotLayer { sprite: sprite(&body, 32.0, 48.0), color: None },
            SnapshotLayer { sprite: sprite(&hat, 16.0, 8.0), color: Some("#ff0000".into()) },
        ]),
        ..Default::default()
    });
    let mut renderer = WorldRenderer { art: None, ..WorldRenderer::new() };
    let first = renderer.draw_list(&snapshot);
    let composed: Vec<_> = first
        .commands
        .iter()
        .filter_map(|command| match command {
            DrawCmd::Image { image, dst, .. } => Some((*image, *dst)),
            _ => None,
        })
        .collect();
    assert_eq!(composed.len(), 1, "one image for the whole stack");
    let (image, dst) = composed[0];
    assert!(renderer.images.source(image).is_some_and(|source| source.starts_with("layers:")));
    assert_eq!(dst, Rect::new(0.0, 16.0, 32.0, 48.0), "drawn like its 32×48 body");
    let cached = renderer.images.len();
    let second = renderer.draw_list(&snapshot);
    assert_eq!(second.commands, first.commands, "the next frame reuses the composed frame");
    assert_eq!(renderer.images.len(), cached, "nothing new is composed");

    // The pixels: the body grey below, the hat on the body's bottom edge tinted red.
    let bitmap = renderer.render(&snapshot, 1.0);
    let at = |x: u32, y: u32| bitmap.pixel(x, y).unwrap().demultiply();
    let body_pixel = at(16, 30);
    assert_eq!((body_pixel.red(), body_pixel.green(), body_pixel.blue()), (200, 200, 200));
    let hat_pixel = at(16, 60);
    assert_eq!((hat_pixel.red(), hat_pixel.green(), hat_pixel.blue()), (255, 0, 0));
}

#[test]
fn worn_layers_draw_over_the_built_in_player() {
    let shirt = data_url(32, 48, Color::rgb(255, 255, 255));
    let mut snapshot = scene(1);
    snapshot.player = SnapshotEntity {
        x: 0.0,
        y: 1.0,
        worn_layers: Some(vec![SnapshotLayer { sprite: sprite(&shirt, 32.0, 48.0), color: Some("#00ff00".into()) }]),
        ..SnapshotEntity::default()
    };
    let mut images = ImageStore::default();
    let list = build_world(&snapshot, art(), &mut images);
    let drawn: Vec<_> = list
        .commands
        .iter()
        .filter_map(|command| match command {
            DrawCmd::Image { image, dst, .. } => images.source(*image).map(|source| (source.to_owned(), *dst)),
            _ => None,
        })
        .filter(|(source, _)| !source.starts_with("builtin://tiles"))
        .collect();
    assert_eq!(drawn.len(), 1, "{drawn:?}");
    assert!(drawn[0].0.starts_with("layers:"));
    assert_eq!((drawn[0].1.width, drawn[0].1.height), (32.0, 48.0), "at the built-in scale");
}

#[test]
fn worn_items_with_a_layer_are_stacked_trousers_shirt_hat() {
    let worn = |slot: &str, layer: &str| InventorySlot {
        item: Item {
            wear: Some(WearDefinition {
                slot: slot.into(),
                color: Some("#123456".into()),
                layer: Some(layer.into()),
                ..WearDefinition::default()
            }),
            ..Item::default()
        },
        quantity: 1,
        quality: None,
    };
    let layers = farm_render::look::worn_layers(&[worn("hat", "cap"), worn("pants", "jeans"), worn("shirt", "tee")]);
    let ids: Vec<_> = layers.unwrap().into_iter().map(|layer| layer.asset_id).collect();
    assert_eq!(ids, ["jeans", "tee", "cap"]);
    assert_eq!(farm_render::look::worn_layers(&[]), None);
}

#[test]
fn npc_looks_recolour_the_base_character() {
    use farm_render::look::{npc_look_key, APPEARANCE_LOOKS};
    assert_eq!(npc_look_key(None, Some("farmer")), None, "hand-drawn appearances stay as they are");
    assert_eq!(npc_look_key(None, Some("wizard")), None);
    let look = PlayerLook { shirt: Some("#ff0000".into()), ..PlayerLook::default() };
    assert_eq!(npc_look_key(Some(&look), Some("farmer")).as_deref(), Some("-,-,ff0000,-,-"));
    // Built-in looks, and a look's colours over them.
    assert!(APPEARANCE_LOOKS.len() >= 8);
    let fisher = npc_look_key(None, Some("Old Fisher")).unwrap();
    assert_eq!(fisher, "e0b088,c8c8c8,2f7f7a,3c4a5c,f2c14e");
    assert_eq!(npc_look_key(Some(&look), Some("fisher")).as_deref(), Some("e0b088,c8c8c8,ff0000,3c4a5c,f2c14e"));
    // Every built-in look is a different character.
    let mut keys: Vec<_> = APPEARANCE_LOOKS.iter().map(|(name, _)| npc_look_key(None, Some(name)).unwrap()).collect();
    keys.dedup();
    assert_eq!(keys.len(), APPEARANCE_LOOKS.len());

    // The map draws an NPC with a look from the recoloured base sheet.
    let mut snapshot = scene(1);
    snapshot.npcs.push(SnapshotEntity { x: 0.0, y: 1.0, appearance: Some("miner".into()), ..Default::default() });
    let mut images = ImageStore::default();
    let list = build_world(&snapshot, art(), &mut images);
    let sources: Vec<_> = list
        .commands
        .iter()
        .filter_map(|command| match command {
            DrawCmd::Image { image, .. } => images.source(*image).map(str::to_owned),
            _ => None,
        })
        .filter(|source| source.contains("#look="))
        .collect();
    assert_eq!(sources.len(), 1, "the NPC (the player off the map has no look)");
    assert!(sources[0].starts_with("builtin://characters.png#look="), "{}", sources[0]);
}

#[test]
fn the_shell_snapshot_carries_npc_looks_and_layers() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/golden/content/starter-farm.json")).unwrap();
    let mut project: farm_sim::GameProject = serde_json::from_value(fixture["project"].clone()).unwrap();
    let body = data_url(32, 48, Color::rgb(90, 90, 90));
    project.custom_assets.push(CustomAsset {
        id: "body".into(),
        name: "Body".into(),
        r#type: "npc".into(),
        data_url: body.clone(),
        ..CustomAsset::default()
    });
    let npc = &mut project.npcs[0];
    npc.look = Some(PlayerLook { hair: Some("#ffffff".into()), ..PlayerLook::default() });
    npc.layers = Some(vec![CharacterLayer { asset_id: "body".into(), color: Some("#808080".into()) }]);
    let scene_id = npc.scene_id.clone();
    let content = farm_sim::create_content_from_project(&project);
    let state = farm_sim::create_game_state(&project, None);
    let scene = state.world.scenes.iter().find(|scene| scene.id == scene_id).unwrap();
    let mut snapshot =
        shell_snapshot(&content, &state, scene, &SnapshotOptions { tile_size: 32.0, ..Default::default() });
    let index =
        content.npcs.iter().filter(|n| n.scene_id == scene_id).position(|n| n.id == project.npcs[0].id).unwrap();
    assert_eq!(snapshot.npcs[index].look.as_deref(), Some("-,ffffff,-,-,-"));
    let presentation = farm_sim::Presentation::from_project(&project);
    let source = GraphicsSource::from_state(&presentation, &content, &state);
    apply_graphics(&mut snapshot, &source, scene, 0.0, false);
    let layers = snapshot.npcs[index].layers.as_ref().expect("the NPC's layers are resolved");
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].sprite.image_url.as_ref(), body);
    assert_eq!(layers[0].color.as_deref(), Some("#808080"));
}

#[test]
fn action_clips_pick_their_frames_and_effects_move_the_character() {
    use farm_sim::schema::{AnimationClip, ArtFrame, VisualRef};
    let frame = |x: f64| ArtFrame { x, width: 16.0, height: 16.0, ticks: 2.0, ..ArtFrame::default() };
    let asset = CustomAsset {
        id: "hero".into(),
        name: "Hero".into(),
        r#type: "player".into(),
        data_url: "data:image/png;base64,x".into(),
        animations: Some(vec![
            AnimationClip { name: "idle".into(), frames: vec![frame(0.0)], ..AnimationClip::default() },
            AnimationClip {
                name: "use-till".into(),
                frames: vec![frame(16.0), frame(32.0)],
                ..AnimationClip::default()
            },
        ]),
        ..CustomAsset::default()
    };
    let assets = farm_render::graphics::art_assets(&[asset]);
    let visual = VisualRef { asset_id: "hero".into(), ..VisualRef::default() };
    let action = farm_render::SnapshotAction { clip: "use-till".into(), elapsed: 2.0, ..Default::default() };
    let acting = farm_render::resolve_acting(&assets, Some(&visual), 0.0, "down", false, Some(&action)).unwrap();
    assert_eq!(acting.source_x, Some(32.0), "the clip's second frame, 2 ticks in");
    let missing = farm_render::SnapshotAction { clip: "eat".into(), ..Default::default() };
    let idle = farm_render::resolve_acting(&assets, Some(&visual), 0.0, "down", false, Some(&missing)).unwrap();
    assert_eq!(idle.source_x, Some(0.0), "no such clip: the usual frame");

    // A shaking character moves a pixel; the same tick always draws the same.
    let art = data_url(32, 48, Color::rgb(10, 10, 200));
    let mut snapshot = scene(1);
    let shaking = farm_render::SnapshotAction { clip: "x".into(), effect: Some("shake".into()), ..Default::default() };
    snapshot.npcs.push(SnapshotEntity {
        x: 0.0,
        y: 1.0,
        sprite: Some(sprite(&art, 32.0, 48.0)),
        action: Some(shaking),
        ..Default::default()
    });
    let mut images = ImageStore::default();
    let list = build_world(&snapshot, None, &mut images);
    assert_eq!(image_rects(&list, &images, &art), vec![Rect::new(1.0, 16.0, 32.0, 48.0)]);
}
