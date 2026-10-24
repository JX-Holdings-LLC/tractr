//! Buildings (M5), gates (M5) and monsters (M7) in the world: what the shell snapshot carries of
//! them, and how the renderer draws them without art (a block with a roof and a door, a
//! scaffold, a gate's posts, a blob with eyes and a health bar).

use farm_render::tiny_skia::Pixmap;
use farm_render::{
    build_world, css_color, shell_snapshot, Color, DrawCmd, ImageStore, SnapshotBuilding, SnapshotEntity,
    SnapshotMachine, SnapshotMonster, SnapshotOptions, SnapshotTile, WorldRenderer, WorldSnapshot,
};
use farm_sim::schema::{BuildingDefinition, BuildingState, MonsterDefinition, MonsterState};

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
        ..WorldSnapshot::default()
    }
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

fn commands(snapshot: &WorldSnapshot) -> Vec<DrawCmd> {
    build_world(snapshot, None, &mut ImageStore::default()).commands
}

#[test]
fn a_standing_building_is_a_block_with_a_roof_and_a_door() {
    let mut snapshot = grass(4, 3);
    snapshot.buildings.push(SnapshotBuilding {
        x: 0,
        y: 1,
        width: 3,
        height: 2,
        color: "#3366cc".into(),
        door: Some((1, 1)),
        ..SnapshotBuilding::default()
    });
    let bitmap = render(&snapshot);
    // Walls on the footprint's lower part, the roof above it (reaching into the row above).
    assert_color("#3366cc", pixel(&bitmap, 2.5 * TS, TS + 30.0));
    assert_color("#7a2e22", pixel(&bitmap, 1.5 * TS, TS - 8.0));
    // The door in the middle of the bottom row.
    assert_color("#4a2e1a", pixel(&bitmap, 1.5 * TS - 3.0, 3.0 * TS - 4.0));
    // Outside the footprint the grass shows.
    assert_ne!(pixel(&bitmap, 3.5 * TS, 2.5 * TS), pixel(&bitmap, 2.5 * TS, TS + 30.0));
}

#[test]
fn a_building_still_going_up_is_a_scaffold() {
    let mut snapshot = grass(3, 3);
    snapshot.buildings.push(SnapshotBuilding {
        x: 0,
        y: 0,
        width: 2,
        height: 2,
        scaffold: true,
        ..SnapshotBuilding::default()
    });
    let list = commands(&snapshot);
    let wood = css_color::parse(Some("#a0763c"));
    assert!(
        list.iter().filter(|command| matches!(command, DrawCmd::Line { color, .. } if *color == wood)).count() >= 5
    );
    let roof = css_color::parse(Some("#7a2e22"));
    assert!(!list.iter().any(|command| matches!(command, DrawCmd::FillRoundRect { color, .. } if *color == roof)));
}

#[test]
fn an_open_gate_shows_its_posts() {
    let mut snapshot = grass(2, 1);
    let gate = SnapshotMachine { color: "#c08040".into(), ..SnapshotMachine::default() };
    snapshot.tiles[0][0].machine = Some(gate.clone());
    snapshot.tiles[0][1].machine = Some(SnapshotMachine { open: true, ..gate });
    let bitmap = render(&snapshot);
    // Closed: the block in the middle of the tile; open: the middle is clear, the posts aren't.
    assert_color("#c08040", pixel(&bitmap, 16.0, 16.0));
    assert_ne!(pixel(&bitmap, TS + 16.0, 16.0), pixel(&bitmap, 16.0, 16.0));
    assert_color("#c08040", pixel(&bitmap, TS + 3.0, 26.0));
}

#[test]
fn monsters_are_blobs_with_eyes_and_a_health_bar_once_hurt() {
    let mut snapshot = grass(3, 1);
    snapshot.monsters.push(SnapshotMonster { x: 0.5, y: 0.5, color: "#ff00ff".into(), ..SnapshotMonster::default() });
    snapshot.monsters.push(SnapshotMonster {
        x: 2.5,
        y: 0.5,
        color: "#ff00ff".into(),
        health: Some(0.4),
        ..SnapshotMonster::default()
    });
    let bitmap = render(&snapshot);
    assert_color("#ff00ff", pixel(&bitmap, 16.0, 26.0));
    let list = commands(&snapshot);
    let white = css_color::parse(Some("#ffffff"));
    let eyes = list.iter().filter(|command| matches!(command, DrawCmd::FillCircle { color, .. } if *color == white));
    assert_eq!(eyes.count(), 4, "two eyes each");
    // Only the hurt one has a bar, filled to its health.
    let amber = css_color::parse(Some("#e0b040"));
    let bars: Vec<_> = list
        .iter()
        .filter_map(|command| match command {
            DrawCmd::FillRect { rect, color, .. } if *color == amber => Some(*rect),
            _ => None,
        })
        .collect();
    assert_eq!(bars.len(), 1);
    assert!(bars[0].x > 2.0 * TS as f32 && (bars[0].width - 0.4 * (TS * 0.7).floor() as f32).abs() < 0.01);
}

#[test]
fn the_shell_snapshot_carries_the_scenes_buildings_and_monsters() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/golden/content/starter-farm.json")).unwrap();
    let mut project: farm_sim::GameProject = serde_json::from_value(fixture["project"].clone()).unwrap();
    project.building_types = Some(vec![BuildingDefinition {
        id: "barn".into(),
        name: "Barn".into(),
        width: 4,
        height: 3,
        color: "#aa3322".into(),
        interior_scene_id: Some(project.scenes[0].id.clone()),
        ..BuildingDefinition::default()
    }]);
    project.monsters = Some(vec![MonsterDefinition {
        id: "slime".into(),
        color: "#22aa44".into(),
        health: 10,
        ..MonsterDefinition::default()
    }]);
    let content = farm_sim::create_content_from_project(&project);
    let mut state = farm_sim::create_game_state(&project, None);
    let scene_id = state.player.scene_id.clone();
    state.buildings.push(BuildingState {
        id: "building-1".into(),
        building_id: "barn".into(),
        scene_id: scene_id.clone(),
        x: 2,
        y: 1,
        days_left: 1,
        ..BuildingState::default()
    });
    let tile = farm_sim::units::TILE;
    for (scene, health) in [(scene_id.clone(), 4), ("elsewhere".to_owned(), 10)] {
        state.monsters.push(MonsterState {
            monster_id: "slime".into(),
            scene_id: scene,
            x: tile * 3 + tile / 2,
            y: tile * 4 + tile / 2,
            health,
            ..MonsterState::default()
        });
    }
    let scene = state.world.scenes.iter().find(|scene| scene.id == scene_id).unwrap().clone();
    let options = SnapshotOptions { tile_size: TS, ..SnapshotOptions::default() };
    let snapshot = shell_snapshot(&content, &state, &scene, &options);
    assert_eq!(snapshot.buildings.len(), 1);
    let barn = &snapshot.buildings[0];
    assert_eq!((barn.x, barn.y, barn.width, barn.height), (2, 1, 4, 3));
    assert!(barn.scaffold, "still a night of building left");
    assert_eq!(barn.door, Some((2, 2)));
    assert_eq!(snapshot.monsters.len(), 1, "only this scene's");
    let slime = &snapshot.monsters[0];
    assert_eq!((slime.x, slime.y), (3.5, 4.5));
    assert_eq!(slime.color, "#22aa44");
    assert_eq!(slime.health, Some(0.4));
}

#[test]
fn snapshots_without_them_serialize_as_before() {
    let json = serde_json::to_value(grass(1, 1)).unwrap();
    assert!(json.get("monsters").is_none() && json.get("buildings").is_none());
    let machine = serde_json::to_value(SnapshotMachine::default()).unwrap();
    assert!(machine.get("open").is_none());
}
