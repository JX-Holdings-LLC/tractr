//! Bigger and richer maps (M8): tall objects that stand among the player, the scene's extra
//! layers under and over the player, autotiled terrains and lights at night; what the shell
//! snapshot carries of them, and how the renderer draws them without art.

use farm_render::tiny_skia::Pixmap;
use farm_render::{
    css_color, shell_snapshot, Color, SnapshotAtmosphere, SnapshotDecor, SnapshotEntity, SnapshotLight,
    SnapshotOptions, SnapshotTerrain, SnapshotTile, WorldRenderer, WorldSnapshot,
};
use farm_sim::schema::{LightSource, NodeTypeDefinition, SceneLayer, TerrainDefinition, TileNode};

const TS: f64 = 32.0;

fn grass(width: usize, height: usize) -> WorldSnapshot {
    WorldSnapshot {
        width: width as i32,
        height: height as i32,
        tile_size: TS,
        padding: 0.0,
        tile_gap: Some(0.0),
        player: SnapshotEntity { x: -10.0, y: -10.0, ..SnapshotEntity::default() },
        tiles: vec![vec![SnapshotTile { background: "grass".into(), ..SnapshotTile::default() }; width]; height],
        terrains: Some(vec![terrain("grass", "#5b9a4a")]),
        ..WorldSnapshot::default()
    }
}

fn terrain(id: &str, color: &str) -> SnapshotTerrain {
    SnapshotTerrain { id: id.into(), color: color.into(), ..SnapshotTerrain::default() }
}

fn render(snapshot: &WorldSnapshot) -> Pixmap {
    WorldRenderer { art: None, ..WorldRenderer::new() }.render(snapshot, 1.0)
}

fn pixel(pixmap: &Pixmap, x: f64, y: f64) -> Color {
    let p = pixmap.pixel(x as u32, y as u32).unwrap().demultiply();
    Color::rgba(p.red(), p.green(), p.blue(), p.alpha())
}

fn assert_color(expected_css: &str, actual: Color) {
    let expected = css_color::parse(Some(expected_css));
    let close = |a: u8, b: u8| (i32::from(a) - i32::from(b)).abs() <= 3;
    assert!(
        close(expected.r, actual.r) && close(expected.g, actual.g) && close(expected.b, actual.b),
        "expected {expected} got {actual}"
    );
}

fn brightness(color: Color) -> i32 {
    i32::from(color.r) + i32::from(color.g) + i32::from(color.b)
}

#[test]
fn a_tall_object_stands_over_what_is_behind_it() {
    let mut snapshot = grass(3, 4);
    snapshot.terrains.as_mut().unwrap().push(SnapshotTerrain { tall: 2, ..terrain("pine", "#1f6b2f") });
    snapshot.tiles[3][1].object = Some("pine".into());
    // The player stands two rows up, behind the tree: hidden by it.
    snapshot.player = SnapshotEntity { x: 1.0, y: 1.0, ..SnapshotEntity::default() };
    let bitmap = render(&snapshot);
    // The tree rises from its tile to two tiles above it, the player's tile included.
    assert_color("#1f6b2f", pixel(&bitmap, 1.5 * TS, 3.5 * TS));
    assert_color("#1f6b2f", pixel(&bitmap, 1.5 * TS, 1.6 * TS));
    // Beside it, grass.
    assert_color("#5b9a4a", pixel(&bitmap, 0.5 * TS, 1.5 * TS));

    // Flat, the same terrain stays on its own tile, under the player.
    snapshot.terrains.as_mut().unwrap()[1].tall = 0;
    let flat = render(&snapshot);
    assert_color("#1f6b2f", pixel(&flat, 1.5 * TS, 3.5 * TS));
    assert_ne!(pixel(&flat, 1.5 * TS, 1.6 * TS), pixel(&bitmap, 1.5 * TS, 1.6 * TS));
}

#[test]
fn extra_layers_draw_under_or_over_the_player() {
    let mut snapshot = grass(3, 3);
    snapshot.terrains.as_mut().unwrap().extend([terrain("rug", "#aa3355"), terrain("leaves", "#2e7d32")]);
    snapshot.tiles[1][1].decor = vec![SnapshotDecor { terrain: "rug".into(), ..SnapshotDecor::default() }];
    snapshot.tiles[1][2].decor =
        vec![SnapshotDecor { terrain: "leaves".into(), above: true, ..SnapshotDecor::default() }];
    let bitmap = render(&snapshot);
    assert_color("#aa3355", pixel(&bitmap, 1.5 * TS, 1.5 * TS));
    assert_color("#2e7d32", pixel(&bitmap, 2.5 * TS, 1.5 * TS));

    // The player on the rug hides it; under the leaves, the leaves hide the player.
    let mut on_rug = snapshot.clone();
    on_rug.player = SnapshotEntity { x: 1.0, y: 1.0, ..SnapshotEntity::default() };
    let mut under_leaves = snapshot.clone();
    under_leaves.player = SnapshotEntity { x: 2.0, y: 1.0, ..SnapshotEntity::default() };
    assert_ne!(pixel(&render(&on_rug), 1.5 * TS, 1.5 * TS), pixel(&bitmap, 1.5 * TS, 1.5 * TS));
    assert_color("#2e7d32", pixel(&render(&under_leaves), 2.5 * TS, 1.5 * TS));
}

#[test]
fn an_autotiled_terrain_without_art_shows_where_it_meets_another() {
    let mut snapshot = grass(3, 3);
    snapshot.terrains.as_mut().unwrap().push(SnapshotTerrain { autotile: true, ..terrain("water", "#3075b0") });
    for x in 0..3 {
        snapshot.tiles[1][x].background = "water".into();
    }
    let bitmap = render(&snapshot);
    let centre = pixel(&bitmap, 1.5 * TS, 1.5 * TS);
    assert_color("#3075b0", centre);
    // Grass above and below: a darker band along those edges; water left and right: none.
    assert!(brightness(pixel(&bitmap, 1.5 * TS, TS + 1.0)) < brightness(centre) - 40);
    assert!(brightness(pixel(&bitmap, 1.5 * TS, 2.0 * TS - 1.0)) < brightness(centre) - 40);
    assert_color("#3075b0", pixel(&bitmap, 1.0 * TS + 1.0, 1.5 * TS));
    assert_color("#3075b0", pixel(&bitmap, 2.0 * TS - 1.0, 1.5 * TS));
}

#[test]
fn lights_show_the_scene_at_night_only() {
    let mut snapshot = grass(5, 5);
    snapshot.lights = vec![SnapshotLight { x: 2.5, y: 2.5, radius: 2.0, color: "#ffcc66".into(), brightness: 1.0 }];
    let at = |time_minutes: f64| {
        let mut lit = snapshot.clone();
        lit.atmosphere = Some(SnapshotAtmosphere { time_minutes, ..SnapshotAtmosphere::default() });
        let mut dark = lit.clone();
        dark.lights.clear();
        (pixel(&render(&lit), 2.5 * TS, 2.5 * TS), pixel(&render(&dark), 2.5 * TS, 2.5 * TS))
    };
    let (lit, dark) = at(23.0 * 60.0);
    assert!(brightness(lit) > brightness(dark) + 60, "{lit} vs {dark}");
    let (noon_lit, noon) = at(12.0 * 60.0);
    assert_eq!(noon_lit, noon);
    // A light map, not a glow: at a full light's centre the grass shows its own colour (the
    // daytime one), and the night around it stays dark.
    assert_color("#5b9a4a", lit);
    let mut far = snapshot.clone();
    far.atmosphere = Some(SnapshotAtmosphere { time_minutes: 23.0 * 60.0, ..SnapshotAtmosphere::default() });
    let bitmap = render(&far);
    let mut unlit = far.clone();
    unlit.lights.clear();
    assert_eq!(pixel(&bitmap, 0.2 * TS, 0.2 * TS), pixel(&render(&unlit), 0.2 * TS, 0.2 * TS));
    // Halfway out, between the two.
    let half = brightness(pixel(&bitmap, 3.5 * TS, 2.5 * TS));
    assert!(half > brightness(dark) && half < brightness(lit), "{half}");
}

#[test]
fn the_world_draws_its_lights_as_one_light_map_over_the_tint() {
    use farm_render::{build_world, DrawCmd, ImageStore};
    let mut snapshot = grass(4, 4);
    snapshot.atmosphere = Some(SnapshotAtmosphere { time_minutes: 23.0 * 60.0, ..SnapshotAtmosphere::default() });
    let commands = |snapshot: &WorldSnapshot| build_world(snapshot, None, &mut ImageStore::default()).commands;
    // No lights: the plain day/night multiply.
    assert!(commands(&snapshot).iter().any(|c| matches!(c, DrawCmd::ModulateRect { .. })));
    snapshot.lights = vec![
        SnapshotLight { x: 1.5, y: 1.5, radius: 2.0, color: String::new(), brightness: 0.5 },
        // Far off the camera: left out.
        SnapshotLight { x: 300.0, y: 300.0, radius: 1.0, color: String::new(), brightness: 1.0 },
    ];
    snapshot.camera = Some(farm_render::SnapshotCamera { x: 0.0, y: 0.0, width: 4.0 * TS, height: 4.0 * TS });
    let list = commands(&snapshot);
    assert!(!list.iter().any(|c| matches!(c, DrawCmd::ModulateRect { .. })));
    let lights = list.iter().find_map(|c| match c {
        DrawCmd::LightRect { lights, .. } => Some(lights.clone()),
        _ => None,
    });
    let lights = lights.expect("a light map");
    assert_eq!(lights.len(), 1);
    assert_eq!((lights[0].x, lights[0].y, lights[0].radius, lights[0].strength), (48.0, 48.0, 64.0, 0.5));
    assert_eq!(lights[0].color, css_color::parse(Some("#ffd27a")));
}

#[test]
fn the_shell_snapshot_carries_layers_tall_terrains_and_lights() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/golden/content/starter-farm.json")).unwrap();
    let mut project: farm_sim::GameProject = serde_json::from_value(fixture["project"].clone()).unwrap();
    let lamp = LightSource { radius: 4, color: "#ffaa44".into(), brightness: 80, ..LightSource::default() };
    project.terrains = Some(vec![
        TerrainDefinition {
            id: "pine".into(),
            name: "Pine".into(),
            layer: "object".into(),
            walkable: false,
            tall: Some(3),
            ..TerrainDefinition::default()
        },
        TerrainDefinition {
            id: "moss".into(),
            name: "Glow moss".into(),
            layer: "overlay".into(),
            autotile: Some(true),
            light: Some(LightSource { radius: 1, ..LightSource::default() }),
            ..TerrainDefinition::default()
        },
    ]);
    project.node_types.push(NodeTypeDefinition {
        id: "crystal".into(),
        name: "Crystal".into(),
        health: 3,
        light: Some(lamp.clone()),
        ..NodeTypeDefinition::default()
    });
    let scene = &mut project.scenes[0];
    scene.layers = Some(vec![
        SceneLayer { id: "rugs".into(), name: "Rugs".into(), ..SceneLayer::default() },
        SceneLayer { id: "canopy".into(), name: "Canopy".into(), above: Some(true), ..SceneLayer::default() },
    ]);
    let tile = &mut scene.tiles[2][2];
    tile.decor = Some([("canopy".to_owned(), "moss".to_owned()), ("rugs".to_owned(), "pine".to_owned())].into());
    scene.tiles[3][3].node = Some(TileNode { type_id: "crystal".into(), remaining_health: 3, ..TileNode::default() });
    let content = farm_sim::create_content_from_project(&project);
    let state = farm_sim::create_game_state(&project, None);
    let scene = state.world.scenes[0].clone();
    let snapshot =
        shell_snapshot(&content, &state, &scene, &SnapshotOptions { tile_size: TS, ..SnapshotOptions::default() });

    // The decor is in the scene's layer order, whatever order the tile lists it in.
    let decor = &snapshot.tiles[2][2].decor;
    assert_eq!(
        decor.iter().map(|d| (d.terrain.as_str(), d.above)).collect::<Vec<_>>(),
        [("pine", false), ("moss", true)]
    );
    let terrains = snapshot.terrains.as_ref().unwrap();
    assert_eq!(terrains.iter().find(|t| t.id == "pine").map(|t| t.tall), Some(3));
    assert!(terrains.iter().find(|t| t.id == "moss").is_some_and(|t| t.autotile));
    // The crystal's light and the moss's (on an extra layer) are in the snapshot.
    assert!(snapshot
        .lights
        .iter()
        .any(|l| (l.x, l.y, l.radius, l.brightness) == (3.5, 3.5, 4.0, 0.8) && l.color == "#ffaa44"));
    assert!(snapshot.lights.iter().any(|l| (l.x, l.y, l.radius) == (2.5, 2.5, 1.0)));

    // A broken crystal gives no light.
    let mut state = state;
    state.world.scenes[0].tiles[3][3].node.as_mut().unwrap().remaining_health = 0;
    let scene = state.world.scenes[0].clone();
    let snapshot =
        shell_snapshot(&content, &state, &scene, &SnapshotOptions { tile_size: TS, ..SnapshotOptions::default() });
    assert!(!snapshot.lights.iter().any(|l| l.radius == 4.0));
}

#[test]
fn an_autotiled_terrain_with_art_picks_the_frame_for_its_neighbours() {
    use farm_sim::schema::{AnimationClip, ArtFrame, CustomAsset, VisualRef};
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/golden/content/starter-farm.json")).unwrap();
    let mut project: farm_sim::GameProject = serde_json::from_value(fixture["project"].clone()).unwrap();
    // A 16-frame sheet: frame n at x = 16 n.
    let frames = (0..16).map(|n| ArtFrame {
        x: f64::from(n) * 16.0,
        y: 0.0,
        width: 16.0,
        height: 16.0,
        ticks: 1.0,
        ..ArtFrame::default()
    });
    project.custom_assets.push(CustomAsset {
        id: "water-sheet".into(),
        name: "Water".into(),
        r#type: "tile".into(),
        data_url: "data:image/png;base64,AAAA".into(),
        animations: Some(vec![AnimationClip { name: "autotile".into(), r#loop: false, frames: frames.collect() }]),
        ..CustomAsset::default()
    });
    project.terrains = Some(vec![TerrainDefinition {
        id: "pond".into(),
        name: "Pond".into(),
        autotile: Some(true),
        visual: Some(VisualRef { asset_id: "water-sheet".into(), ..VisualRef::default() }),
        ..TerrainDefinition::default()
    }]);
    // A pond tile with pond above and to its right: 1 + 2 = frame 3.
    for (x, y) in [(2, 2), (2, 1), (3, 2)] {
        project.scenes[0].tiles[y][x].background = "pond".into();
    }
    let content = farm_sim::create_content_from_project(&project);
    let state = farm_sim::create_game_state(&project, None);
    let scene = state.world.scenes[0].clone();
    let mut snapshot =
        shell_snapshot(&content, &state, &scene, &SnapshotOptions { tile_size: TS, ..SnapshotOptions::default() });
    let source =
        farm_render::GraphicsSource::from_state(&farm_sim::Presentation::from_project(&project), &content, &state);
    farm_render::apply_graphics(&mut snapshot, &source, &scene, 0.0, false);
    let sprite = snapshot.tiles[2][2].art_layer(0).expect("the pond's art");
    assert_eq!(sprite.source_x, Some(48.0));
    // With pond only below it: frame 4.
    let lone = snapshot.tiles[1][2].art_layer(0).expect("the pond's art");
    assert_eq!(lone.source_x, Some(4.0 * 16.0), "pond below it only: frame 4");
}
