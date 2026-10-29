mod common;

use farm_sim::schema::{GameProject, GameState, KeptState, MineConfig, MineDefinition};
use farm_sim::{mines, state, EngineContext};

fn engine() -> (EngineContext, GameState) {
    let primary = MineConfig {
        enabled: true,
        entrance_scene_id: Some("farm".into()),
        entrance_x: Some(2),
        entrance_y: Some(2),
        ..Default::default()
    };
    let project = GameProject {
        id: "mines".into(),
        start_scene_id: "farm".into(),
        scenes: vec![common::empty_scene("farm", "Farm", 12, 12)],
        mine: primary.clone(),
        mines: Some(vec![MineDefinition {
            id: "volcano".into(),
            name: "Volcano".into(),
            config: MineConfig { entrance_x: Some(8), entrance_y: Some(8), elevator_every: 3, ..primary },
            music: Some("lava".into()),
        }]),
        ..Default::default()
    };
    (
        EngineContext::new(state::create_content_from_project(&project)),
        state::create_game_state(&project, Some("mines")),
    )
}

#[test]
fn mines_keep_independent_depths_and_return_to_their_own_entrances() {
    let (ctx, mut current) = engine();
    mines::descend_mine(&ctx, &mut current, 10);
    assert_eq!(current.player.scene_id, "mine-floor-10");
    mines::exit_mine(&ctx, &mut current);
    current.player.x = farm_sim::units::tile_center(8);
    current.player.y = farm_sim::units::tile_center(8);
    mines::enter_mine(&ctx, &mut current, "volcano");
    assert_eq!(current.current_mine_id.as_deref(), Some("volcano"));
    assert_eq!(current.music.as_deref(), Some("lava"));
    mines::descend_mine(&ctx, &mut current, 6);
    assert_eq!(mines::progress(&current, "mine").deepest_floor, 10);
    assert_eq!(mines::progress(&current, "volcano").deepest_floor, 6);
    assert_ne!(current.player.scene_id, "mine-floor-6");
    let bytes = serde_json::to_vec(&current).unwrap();
    let mut restored: GameState = serde_json::from_slice(&bytes).unwrap();
    mines::exit_mine(&ctx, &mut restored);
    assert_eq!(restored.player.scene_id, "farm");
    assert_eq!(restored.player.x, farm_sim::units::tile_center(8));
    assert_eq!(restored.mine.deepest_floor, 10);
    mines::enter_mine(&ctx, &mut restored, "volcano");
    assert_eq!(restored.mine.current_floor, 6);
    let kept = KeptState::of_state(&restored).unwrap();
    let mut fresh = engine().1;
    fresh.mine = restored.mine.clone();
    kept.apply_to(&mut fresh);
    assert_eq!(mines::progress(&fresh, "mine").deepest_floor, 10);
    assert_eq!(mines::progress(&fresh, "volcano").deepest_floor, 6);
}

#[test]
fn legacy_single_mine_serialization_stays_unchanged() {
    let project = GameProject::default();
    let content = state::create_content_from_project(&project);
    let current = state::create_game_state(&project, Some("legacy"));
    let content = serde_json::to_value(content).unwrap();
    let current = serde_json::to_value(current).unwrap();
    assert!(content.get("mines").is_none());
    assert!(current.get("mines").is_none());
    assert!(current.get("currentMineId").is_none());
}

#[test]
fn explicit_empty_authoring_lists_roundtrip_and_normalize_in_content() {
    let project: GameProject = serde_json::from_str(r#"{"mines":[],"requestBoards":[]}"#).unwrap();
    let project_json = serde_json::to_value(&project).unwrap();
    assert_eq!(project_json["mines"], serde_json::json!([]));
    assert_eq!(project_json["requestBoards"], serde_json::json!([]));
    let content = serde_json::to_value(state::create_content_from_project(&project)).unwrap();
    assert!(content.get("mines").is_none());
    assert!(content.get("requestBoards").is_none());
}
