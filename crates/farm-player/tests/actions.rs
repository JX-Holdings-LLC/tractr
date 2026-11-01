//! Action clips in the play loop (#235): a tool's clip defers its command to the impact tick
//! and holds the player in place; without clips the command runs at once (the game as before).

use farm_player::session::PlaySession;
use farm_sim::schema::{AnimationClip, ArtFrame, CustomAsset, GameProject};
use farm_sim::{state, StartState};

const TICK: f64 = 1.0 / farm_runtime::timestep::TICKS_PER_SECOND;

fn starter() -> GameProject {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/golden/content/starter-farm.json")).unwrap();
    serde_json::from_value(fixture["project"].clone()).unwrap()
}

fn session_for(project: &GameProject) -> PlaySession {
    let content = state::create_content_from_project(project);
    PlaySession::new_game(content, &StartState::from_project(project), Some("actions-test"))
}

fn tool_uses(session: &PlaySession) -> Vec<String> {
    session.recent_commands().into_iter().filter(|command| command.contains("useTool")).collect()
}

#[test]
fn action_clips_send_the_tool_command_at_the_impact_frame() {
    let project = starter();
    let content = state::create_content_from_project(&project);
    let (key, action) = farm_sim::vocabulary::tools(&content)
        .iter()
        .find_map(|tool| Some((tool.key.clone()?.to_lowercase(), tool.action.clone())))
        .expect("a tool with a key");
    let frame = |ticks: f64| ArtFrame { width: 32.0, height: 48.0, ticks, ..ArtFrame::default() };
    let asset = CustomAsset {
        id: "hero".into(),
        name: "Hero".into(),
        r#type: "player".into(),
        data_url: "data:image/png;base64,".into(),
        animations: Some(vec![AnimationClip {
            name: format!("use-{action}"),
            frames: vec![frame(3.0), frame(3.0), frame(4.0)],
            impact_frame: Some(2),
            lock_movement: Some(true),
            ..AnimationClip::default()
        }]),
        ..CustomAsset::default()
    };

    // Without action art: at once.
    let mut plain = session_for(&project);
    plain.key_down(&key);
    plain.update(0.0, false);
    assert_eq!(tool_uses(&plain).len(), 1);

    let mut session = session_for(&project);
    session.set_action_art(farm_render::graphics::art_assets(&[asset]).pop());
    let start = session.state().clock.tick;
    session.key_down(&key);
    session.update(0.0, false);
    session.key_up(&key);
    assert!(tool_uses(&session).is_empty(), "waits for the impact frame");
    assert_eq!(session.action_view().map(|view| view.clip), Some(format!("use-{action}")));
    // Moving is held while the clip plays.
    session.key_down("d");
    for _ in 0..6 {
        session.update(TICK, false);
    }
    assert_eq!(session.state().clock.tick, start + 6);
    let uses = tool_uses(&session);
    assert_eq!(uses.len(), 1, "sent at the impact tick");
    assert!(uses[0].starts_with(&format!("tick {}: ", start + 6)), "{uses:?}");
    assert_eq!(session.state().player.move_intent.dx, 0, "locked in place");
    for _ in 0..5 {
        session.update(TICK, false);
    }
    assert!(session.action_view().is_none(), "over after 10 ticks");
    session.update(TICK, false);
    assert_eq!(session.state().player.move_intent.dx, 1, "free again");
}
