//! M9: the hooks plugins gained (scene entry, time of day, …) and the mutations that run the
//! M3–M7 outcomes or keep a plugin's own data.

mod common;

use common::{flag, make_engine};
use farm_sim::hooks::{HookEvent, SceneEnterHookPayload, TimeOfDayHookPayload};
use farm_sim::schema::{GameState, NodeTypeDefinition, PluginMutation};
use farm_sim::{engine, units, Command, EngineContext};
use serde_json::json;

fn mutate(ctx: &EngineContext, state: &mut GameState, mutation: PluginMutation) {
    engine::apply_command(ctx, state, &Command::PluginMutation { plugin_id: "glow:p".to_owned(), mutation });
}

#[test]
fn the_clock_reaching_a_full_hour_fires_on_time_of_day() {
    let (ctx, mut state) = make_engine(|_| {}, "hours");
    ctx.drain_hook_events();
    // Six o'clock to a little past eight: one in-game minute a second.
    engine::advance_tick(&ctx, &mut state, u64::from(units::TICKS_PER_SECOND) * 125);
    let hours: Vec<TimeOfDayHookPayload> = ctx
        .drain_hook_events()
        .into_iter()
        .filter_map(|event| match event {
            HookEvent::TimeOfDay(payload) => Some(payload),
            _ => None,
        })
        .collect();
    assert_eq!(hours, [TimeOfDayHookPayload { hour: 7, day: 1 }, TimeOfDayHookPayload { hour: 8, day: 1 }]);
}

#[test]
fn entering_another_scene_by_any_means_fires_on_scene_enter() {
    let (ctx, mut state) = make_engine(
        |project| {
            let mut cellar = common::empty_scene("scene-cellar", "Cellar", 4, 4);
            cellar.indoor = Some(true);
            project.scenes.push(cellar);
        },
        "scenes",
    );
    ctx.drain_hook_events();
    let warp = |scene: &str| PluginMutation::WarpPlayer { scene_id: scene.to_owned(), x: 1, y: 1 };
    mutate(&ctx, &mut state, warp("scene-cellar"));
    // Warping within the scene the player is in is no scene entry.
    mutate(&ctx, &mut state, warp("scene-cellar"));
    mutate(&ctx, &mut state, warp("scene-test"));
    let entered: Vec<SceneEnterHookPayload> = ctx
        .drain_hook_events()
        .into_iter()
        .filter_map(|event| match event {
            HookEvent::SceneEnter(payload) => Some(payload),
            _ => None,
        })
        .collect();
    assert_eq!(
        entered,
        [
            SceneEnterHookPayload { scene_id: "scene-cellar".to_owned(), from_scene_id: "scene-test".to_owned() },
            SceneEnterHookPayload { scene_id: "scene-test".to_owned(), from_scene_id: "scene-cellar".to_owned() },
        ]
    );
}

#[test]
fn outcome_mutations_mean_what_the_outcomes_mean() {
    let (ctx, mut state) = make_engine(
        |project| {
            project.node_types.push(NodeTypeDefinition {
                id: "node-crystal".to_owned(),
                name: "Crystal".to_owned(),
                health: 2,
                ..NodeTypeDefinition::default()
            });
        },
        "outcomes",
    );
    mutate(&ctx, &mut state, PluginMutation::SetVariable { variable: "glow:lanterns".to_owned(), value: json!(2) });
    mutate(&ctx, &mut state, PluginMutation::AddVariable { variable: "glow:lanterns".to_owned(), amount: 3 });
    assert_eq!(flag(&state, "glow:lanterns"), Some(&json!(5)));
    mutate(&ctx, &mut state, PluginMutation::SetVariable { variable: "glow:mood".to_owned(), value: json!("glad") });
    assert_eq!(flag(&state, "glow:mood"), Some(&json!("glad")));

    mutate(&ctx, &mut state, PluginMutation::MoveNpc { npc_id: "npc-test".to_owned(), scene_id: None, x: 2, y: 3 });
    let npc = &state.npcs["npc-test"];
    assert_eq!((npc.x, npc.y), (units::tiles(2), units::tiles(3)));

    let place = PluginMutation::PlaceNode { scene_id: None, x: 0, y: 0, node_type_id: "node-crystal".to_owned() };
    mutate(&ctx, &mut state, place);
    assert_eq!(state.world.scenes[0].tiles[0][0].node.as_ref().map(|node| node.type_id.as_str()), Some("node-crystal"));
    mutate(&ctx, &mut state, PluginMutation::RemoveNode { scene_id: None, x: 0, y: 0 });
    assert!(state.world.scenes[0].tiles[0][0].node.is_none());

    // Unknown ids fail soft, like the outcome.
    let before = state.clone();
    mutate(&ctx, &mut state, PluginMutation::StartCutscene { cutscene_id: "glow:missing".to_owned() });
    mutate(&ctx, &mut state, PluginMutation::SendMail { mail_id: "glow:missing".to_owned() });
    assert_eq!(state.cutscene, before.cutscene);
    assert_eq!(state.mail_queue, before.mail_queue);
}

#[test]
fn save_data_keeps_each_plugins_data_in_the_state() {
    let (ctx, mut state) = make_engine(|_| {}, "data");
    mutate(&ctx, &mut state, PluginMutation::SaveData { data: json!({"count": 1.0, "seen": ["a"]}) });
    mutate(&ctx, &mut state, PluginMutation::SaveData { data: json!({"count": 2}) });
    // The latest answer replaces the earlier one; numbers are kept canonical.
    assert_eq!(state.plugin_data.get("glow:p"), Some(&json!({"count": 2})));
    let saved = serde_json::to_value(&state).expect("state");
    assert_eq!(saved["pluginData"], json!({"glow:p": {"count": 2}}));
    let (_, fresh) = make_engine(|_| {}, "data");
    assert!(serde_json::to_value(&fresh).expect("state").get("pluginData").is_none());
}

#[test]
fn keep_changes_carries_what_the_plugins_saved_into_the_next_playtest() {
    let (ctx, mut state) = make_engine(|_| {}, "kept");
    mutate(&ctx, &mut state, PluginMutation::SaveData { data: json!({"visits": 3}) });
    let project = common::make_project();
    let kept = farm_sim::state::apply_state_to_project(&project, &state);
    let next = farm_sim::state::create_game_state(&kept, Some("kept"));
    assert_eq!(next.plugin_data.get("glow:p"), Some(&json!({"visits": 3})));
}
