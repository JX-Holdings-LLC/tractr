//! Plugin screens (#240) and the read API and entities in a running game (#237, #238): the
//! session lists a pack's screens with their title and hotkey, draws their widget trees, runs
//! their actions as mutations at the next tick, and plugin conditions answer while the engine
//! evaluates them.

use farm_player::session::PlaySession;
use farm_runtime::screens::ScreenWidget;
use farm_sim::schema::{
    ContentPack, GameProject, PackInstallation, PackManifest, PackPermissions, PackPlugin, PluginFunction, PluginScreen,
};
use farm_sim::{state, StartState};

fn read_json(relative: &str) -> serde_json::Value {
    let path = [env!("CARGO_MANIFEST_DIR"), "..", "..", relative].iter().collect::<std::path::PathBuf>();
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn strings(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

fn ledger_pack() -> ContentPack {
    let source = "api.screen('ledger', { title: 'Ledger', hotkey: 'L', \
                  draw: (state) => ({ type: 'column', children: [{ type: 'heading', text: 'Money ' + state.player().money }, { type: 'button', label: 'Tip', action: 'tip' }] }), \
                  action: (action) => action === 'tip' ? [{ type: 'takeMoney', amount: 10 }] : [] }); \
                  api.screen('broken', { draw: () => ({ type: 'canvas' }) }); \
                  api.condition('rich', (args, state) => state.player().money >= args.min);";
    ContentPack {
        manifest: PackManifest {
            id: "ledger".to_owned(),
            name: "Ledger".to_owned(),
            version: "1.0.0".to_owned(),
            permissions: PackPermissions {
                mutations: Some(strings(&["takeMoney"])),
                read: Some(strings(&["player"])),
                screens: Some(strings(&["ledger", "broken"])),
                ..Default::default()
            },
            ..Default::default()
        },
        plugins: vec![PackPlugin {
            id: "p".to_owned(),
            source: source.to_owned(),
            screens: Some(vec![
                PluginScreen { name: "ledger".to_owned(), ..Default::default() },
                PluginScreen { name: "broken".to_owned(), ..Default::default() },
            ]),
            conditions: Some(vec![PluginFunction { name: "rich".to_owned(), ..Default::default() }]),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn session_with(project: &GameProject) -> PlaySession {
    let content = state::create_content_from_project(project);
    let mut session = PlaySession::new_game(content, &StartState::from_project(project), Some("screens"));
    session.set_plugins(farm_player::plugins::for_project(project, Default::default()));
    assert!(session.plugin_errors().is_empty(), "{:?}", session.plugin_errors());
    session
}

#[test]
fn plugin_screens_draw_and_act() {
    let fixture = read_json("fixtures/golden/content/starter-farm.json");
    let mut project: GameProject = serde_json::from_value(fixture["project"].clone()).unwrap();
    project.content_packs.push(PackInstallation { pack: ledger_pack(), enabled: true });
    let mut session = session_with(&project);
    let screens = session.plugin_screens().to_vec();
    assert_eq!(screens.len(), 2);
    assert_eq!((screens[0].hook.as_str(), screens[0].title.as_str()), ("screen:ledger:ledger", "Ledger"));
    assert_eq!(screens[0].hotkey.as_deref(), Some("l"));
    let money = session.state().player.money;
    let tree = session.draw_plugin_screen("screen:ledger:ledger").expect("the ledger draws");
    let ScreenWidget::Column { children } = &tree else { panic!("{tree:?}") };
    assert_eq!(children[0], ScreenWidget::Heading { text: format!("Money {money}") });
    assert_eq!(tree.actions(), ["tip"]);
    session.plugin_screen_action("screen:ledger:ledger", "tip");
    // The answer applies at the next tick.
    for _ in 0..3 {
        session.update(1.0 / 60.0, false);
    }
    assert_eq!(session.state().player.money, money - 10, "{:?}", session.plugin_errors());
    assert!(session.draw_plugin_screen("screen:ledger:broken").unwrap_err().contains("unknown widget"));
    // The plugin's condition answers while the engine evaluates it.
    let rich = |min: i64| {
        farm_sim::events::condition_met(
            session.context(),
            session.state(),
            &farm_sim::schema::EventCondition::Plugin {
                condition: "ledger:rich".to_owned(),
                args: serde_json::json!({ "min": min }),
            },
            None,
        )
    };
    assert!(rich(0));
    assert!(!rich(money + 1_000_000));
}
