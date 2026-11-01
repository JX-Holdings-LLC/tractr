//! The plugin systems beyond hooks and mutations: the read API (#237), plugin conditions and
//! outcomes (#239), entities, timers and item use (#238) and project scripts (#241), in a real
//! `farm-sim` session.

mod common;

use common::starter_farm_project;
use farm_plugins::{
    GameView, PluginConditionHost, PluginError, PluginHostOptions, PluginRuntime, ReadGrants, CONDITION_FUEL,
};
use farm_sim::schema::{
    ContentPack, EventCondition, EventOutcome, GameProject, GameState, PackInstallation, PackManifest, PackPermissions,
    PackPlugin, PluginArgument, PluginFunction, PluginMutation, ProjectScript,
};
use farm_sim::{apply_command, state, Command, EngineContext, HookBus};
use serde_json::{json, Value};

/// A session the way the player runs one, reads included.
struct Session {
    ctx: EngineContext,
    state: GameState,
    plugins: PluginRuntime,
    errors: Vec<PluginError>,
}

impl Session {
    fn new(project: &GameProject) -> Self {
        let mut ctx = EngineContext::with_hooks(state::create_content_from_project(project), HookBus::new());
        let state = state::create_game_state(project, Some("systems"));
        let mut plugins =
            PluginRuntime::from_project(project, PluginHostOptions::default()).expect("the project has plugins");
        ctx.plugin_conditions =
            plugins.condition_host().map(|host| Box::new(host) as Box<dyn farm_sim::PluginConditions>);
        let mut session = Self { ctx, state, plugins, errors: Vec::new() };
        let game = GameView { ctx: &session.ctx, state: &session.state };
        let errors = session.plugins.restore_in(&session.state.plugin_data.clone(), Some(game));
        session.errors.extend(errors);
        session
    }

    fn run_at(&mut self, command: &Command, depth: u32) {
        let effects = apply_command(&self.ctx, &mut self.state, command);
        let events = self.ctx.drain_hook_events();
        let game = GameView { ctx: &self.ctx, state: &self.state };
        let errors = self.plugins.handle_step_in(&events, &effects, depth, Some(game));
        self.errors.extend(errors);
    }

    /// Run `command`, then everything the plugins answered (and what that set off).
    fn run(&mut self, command: &Command) {
        self.run_at(command, 0);
        self.settle();
    }

    fn settle(&mut self) {
        for _ in 0..4 {
            let queued = self.plugins.drain();
            if queued.is_empty() {
                break;
            }
            for queued in queued {
                let depth = queued.depth + 1;
                self.run_at(&queued.into_command(), depth);
            }
        }
    }

    /// Advance the clock `ticks` ticks, one at a time, with the plugins answering in between.
    fn ticks(&mut self, ticks: u64) {
        for _ in 0..ticks {
            self.plugins.begin_tick(self.state.clock.tick);
            self.settle();
            let effects = farm_sim::advance_tick(&self.ctx, &mut self.state, 1);
            let events = self.ctx.drain_hook_events();
            let game = GameView { ctx: &self.ctx, state: &self.state };
            let errors = self.plugins.handle_step_in(&events, &effects, 0, Some(game));
            self.errors.extend(errors);
        }
        self.settle();
    }

    fn flag(&self, name: &str) -> Option<&Value> {
        self.state.flags.get(name)
    }
}

fn manifest(id: &str, permissions: PackPermissions) -> PackManifest {
    PackManifest {
        id: id.to_owned(),
        name: id.to_owned(),
        version: "1.0.0".to_owned(),
        permissions,
        ..Default::default()
    }
}

fn project_with(packs: Vec<ContentPack>) -> GameProject {
    let mut project = starter_farm_project();
    project.content_packs = packs.into_iter().map(|pack| PackInstallation { pack, enabled: true }).collect();
    project
}

fn strings(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn handlers_read_the_state_of_the_step_their_hook_fired_at() {
    let source = "api.on('onCommand', p => { if (p.commandType !== 'sleep') return []; \
                  var me = api.state.player(); var d = api.state.date(); var t = api.state.tile(me.tileX, me.tileY); \
                  var w; try { api.state.npc('npc-farmer'); w = 'npc ok'; } catch (e) { w = e.message; } \
                  return [{ type: 'setFlag', flag: 'seen', value: me.money + ':' + me.sceneId + ':' + d.day + ':' + (t && t.terrain) + ':' + api.state.flag('seen') + ':' + w }]; });";
    let pack = ContentPack {
        manifest: manifest(
            "reader",
            PackPermissions {
                hooks: strings(&["onCommand"]),
                mutations: Some(strings(&["setFlag"])),
                read: Some(strings(&["player", "world"])),
                ..Default::default()
            },
        ),
        plugins: vec![PackPlugin {
            id: "p".to_owned(),
            hooks: strings(&["onCommand"]),
            source: source.to_owned(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut session = Session::new(&project_with(vec![pack]));
    session.state.player.money = 1234;
    session.run(&Command::Sleep);
    let seen = session.flag("reader:seen").and_then(Value::as_str).unwrap_or_default().to_owned();
    assert!(!seen.is_empty(), "{:?}", session.errors);
    // The read sees the state after the step: the next day, the money as it was.
    let parts: Vec<&str> = seen.split(':').collect();
    assert_eq!(parts[0], "1234", "{seen}");
    assert_eq!(parts[2], "2", "sleeping started day 2: {seen}");
    assert_eq!(parts[4], "null", "its own flag was not set yet: {seen}");
    assert!(parts[5].contains("not permitted") && parts[5].contains("people"), "{seen}");
    assert!(session.errors.is_empty(), "{:?}", session.errors);
}

#[test]
fn read_grants_and_namespacing() {
    let project = starter_farm_project();
    let ctx = EngineContext::new(state::create_content_from_project(&project));
    let mut game_state = state::create_game_state(&project, Some("reads"));
    game_state.flags.insert("pack:own".to_owned(), json!(3));
    game_state.flags.insert("global".to_owned(), json!("g"));
    let game = GameView { ctx: &ctx, state: &game_state };
    let people = ReadGrants::new(&strings(&["people"]));
    let read = |grants: &ReadGrants, request: Value| farm_plugins::answer_read(&game, "pack", grants, &request);
    assert_eq!(read(&people, json!({ "read": "flag", "flag": "own" })), Ok(json!(3)));
    assert_eq!(read(&people, json!({ "read": "flag", "flag": "pack:own" })), Ok(json!(3)));
    assert!(read(&people, json!({ "read": "flag", "flag": "other:x" })).unwrap_err().contains("not permitted"));
    // A plain name is the pack's own: the game's `global` needs `any`.
    assert_eq!(read(&people, json!({ "read": "flag", "flag": "global" })), Ok(Value::Null));
    let any = ReadGrants::new(&strings(&["people", "any"]));
    assert_eq!(read(&any, json!({ "read": "flag", "flag": "global" })), Ok(json!("g")));
    assert!(read(&people, json!({ "read": "player" })).unwrap_err().contains("\"player\""));
    assert!(read(&people, json!({ "read": "nope" })).unwrap_err().contains("unknown read"));
    let content = ReadGrants::new(&strings(&["content"]));
    let item = read(&content, json!({ "read": "item", "itemId": "seed-wheat" })).unwrap();
    assert_eq!(item["id"], "seed-wheat");
    assert!(item.get("visual").is_none());
    // The same request gives the same answer every time.
    assert_eq!(read(&any, json!({ "read": "quests" })), read(&any, json!({ "read": "quests" })));
}

fn condition_pack(source: &str) -> ContentPack {
    ContentPack {
        manifest: manifest(
            "pets",
            PackPermissions {
                read: Some(strings(&["player"])),
                mutations: Some(strings(&["message", "giveMoney", "setFlag"])),
                ..Default::default()
            },
        ),
        plugins: vec![PackPlugin {
            id: "logic".to_owned(),
            source: source.to_owned(),
            conditions: Some(vec![PluginFunction {
                name: "rich".to_owned(),
                args: vec![PluginArgument {
                    name: "min".to_owned(),
                    r#type: "integer".to_owned(),
                    ..Default::default()
                }],
                ..Default::default()
            }]),
            outcomes: Some(vec![PluginFunction { name: "tip".to_owned(), ..Default::default() }]),
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[test]
fn plugin_conditions_answer_synchronously_and_fail_closed() {
    let source = "api.condition('rich', (args, state) => state.player().money >= args.min); \
                  api.condition('broken', () => { throw new Error('kaput'); }); \
                  api.condition('spin', () => { for (;;) {} });";
    let mut pack = condition_pack(source);
    pack.plugins[0]
        .conditions
        .as_mut()
        .unwrap()
        .extend(["broken", "spin"].map(|name| PluginFunction { name: name.to_owned(), ..Default::default() }));
    let project = project_with(vec![pack]);
    let mut session = Session::new(&project);
    session.state.player.money = 500;
    let holds = |session: &Session, condition: &str, args: Value| {
        farm_sim::events::condition_met(
            &session.ctx,
            &session.state,
            &EventCondition::Plugin { condition: condition.to_owned(), args },
            None,
        )
    };
    assert!(holds(&session, "pets:rich", json!({ "min": 100 })));
    assert!(!holds(&session, "pets:rich", json!({ "min": 1000 })));
    assert!(!holds(&session, "pets:broken", Value::Null));
    assert!(!holds(&session, "pets:spin", Value::Null), "out of fuel is false");
    assert!(!holds(&session, "gone:missing", Value::Null), "no plugin defines it");
    let notes = session.plugins.collect_condition_errors();
    let text: Vec<&str> = notes.iter().map(|error| error.message.as_str()).collect();
    assert!(text.iter().any(|m| m.contains("broken") && m.contains("kaput")), "{text:?}");
    assert!(text.iter().any(|m| m.contains("spin") && m.contains("fuel")), "{text:?}");
    assert!(text.iter().any(|m| m.contains("gone:missing") && m.contains("no running plugin")), "{text:?}");
    // Each failure is noted once.
    assert!(!holds(&session, "pets:broken", Value::Null));
    assert!(session.plugins.collect_condition_errors().is_empty());
    // Without the condition host every plugin condition is false.
    session.ctx.plugin_conditions = None;
    assert!(!holds(&session, "pets:rich", json!({ "min": 0 })));
    assert!(CONDITION_FUEL < PluginHostOptions::default().fuel_per_call);
    let _ = PluginConditionHost::new(&[], &PluginHostOptions::default()).is_none();
}

#[test]
fn plugin_outcomes_answer_through_the_queue() {
    let source =
        "api.outcome('tip', (args, state) => [{ type: 'giveMoney', amount: args.amount + state.player().money }]);";
    let mut project = project_with(vec![condition_pack(source)]);
    let mut outcome = EventOutcome { r#type: "plugin".to_owned(), ..EventOutcome::default() };
    outcome.extra.insert("pluginOutcome".to_owned(), json!("pets:tip"));
    outcome.extra.insert("args".to_owned(), json!({ "amount": 5 }));
    project.actions.push(farm_sim::schema::ActionDef {
        id: "tip-action".to_owned(),
        outcomes: vec![outcome],
        ..Default::default()
    });
    let mut session = Session::new(&project);
    session.state.player.money = 10;
    session.run(&Command::PerformAction { action_id: "tip-action".to_owned() });
    assert_eq!(session.state.player.money, 25, "10 + (5 + 10): {:?}", session.errors);
}

#[test]
fn entities_timers_and_item_use() {
    let source = "api.on('onCommand', p => p.commandType === 'sleep' ? [{ type: 'spawnEntity', entityType: 'lamp', x: 2, y: 3, art: 'seed-wheat', state: { lit: 0 } }, { type: 'startTimer', timer: 'ring', minutes: 2, data: { n: 7 } }] : []); \
                  api.entity('lamp', { onTick: (e) => e.state.lit < 3 ? [{ type: 'updateEntity', entityId: e.id, state: { lit: e.state.lit + 1 } }] : [], \
                                        onInteract: (e) => [{ type: 'setFlag', flag: 'touched', value: e.id }], \
                                        onDayStart: (e) => [{ type: 'setFlag', flag: 'morning', value: e.id }] }); \
                  api.timer('ring', (t, state) => [{ type: 'setFlag', flag: 'rang', value: t.data.n }]); \
                  api.itemUse('wand', (use) => [{ type: 'setFlag', flag: 'wand', value: use.itemId + '@' + use.x + ',' + use.y }]);";
    let pack = ContentPack {
        manifest: manifest(
            "glow",
            PackPermissions {
                hooks: strings(&["onCommand"]),
                mutations: Some(strings(&["spawnEntity", "updateEntity", "startTimer", "setFlag"])),
                entities: Some(strings(&["lamp"])),
                item_uses: Some(strings(&["wand"])),
                ..Default::default()
            },
        ),
        plugins: vec![PackPlugin {
            id: "p".to_owned(),
            hooks: strings(&["onCommand"]),
            entities: Some(strings(&["lamp"])),
            item_uses: Some(strings(&["wand"])),
            source: source.to_owned(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut project = project_with(vec![pack]);
    let mut wand = project.items.iter().find(|item| item.id == "seed-wheat").cloned().expect("wheat seeds");
    wand.id = "wand".to_owned();
    wand.use_action_id = Some("plugin:glow:wand".to_owned());
    project.items.push(wand.clone());
    let mut session = Session::new(&project);
    session.run(&Command::Sleep);
    assert_eq!(session.state.plugin_entities.len(), 1, "{:?}", session.errors);
    let entity = session.state.plugin_entities[0].clone();
    assert_eq!((entity.type_id.as_str(), entity.x, entity.y), ("glow:lamp", 2, 3));
    assert_eq!(session.state.plugin_timers.len(), 1);
    // A few game minutes: the lamp ticks up to 3, the timer fires once and is gone.
    session.ticks(farm_sim::engine::TICKS_PER_SECOND as u64 * 10);
    assert_eq!(session.state.plugin_entities[0].state, json!({ "lit": 3 }), "{:?}", session.errors);
    assert_eq!(session.flag("glow:rang"), Some(&json!(7)));
    assert!(session.state.plugin_timers.is_empty());
    // Interacting with the lamp's tile.
    session.state.player.scene_id = entity.scene_id.clone();
    session.state.player.x = farm_sim::units::tile_center(2);
    session.state.player.y = farm_sim::units::tile_center(4);
    session.state.player.direction = "up".to_owned();
    session.run(&Command::Interact);
    assert_eq!(session.flag("glow:touched"), Some(&json!(entity.id)), "{:?}", session.errors);
    // Using the wand: the faced tile.
    session.state.player.inventory.push(farm_sim::schema::InventorySlot::new(wand, 1));
    session.run(&Command::UseItem { item_id: "wand".to_owned() });
    assert_eq!(session.flag("glow:wand"), Some(&json!("wand@2,3")), "{:?}", session.errors);
    // The next morning.
    session.run(&Command::Sleep);
    assert_eq!(session.flag("glow:morning"), Some(&json!(entity.id)));
    // A replay of the mutations reaches the same entities without the plugins (the command
    // log holds them): spawning the same entity by mutation alone.
    let mut plain = state::create_game_state(&project, Some("systems"));
    let ctx = EngineContext::new(state::create_content_from_project(&project));
    apply_command(
        &ctx,
        &mut plain,
        &Command::PluginMutation {
            plugin_id: "glow:p".to_owned(),
            mutation: PluginMutation::SpawnEntity {
                entity_type: "glow:lamp".to_owned(),
                scene_id: None,
                x: 2,
                y: 3,
                art: None,
                state: Value::Null,
            },
        },
    );
    assert_eq!(plain.plugin_entities.len(), 1);
    // Another pack's plugin cannot change it.
    let entity_id = plain.plugin_entities[0].id.clone();
    apply_command(
        &ctx,
        &mut plain,
        &Command::PluginMutation {
            plugin_id: "other:p".to_owned(),
            mutation: PluginMutation::RemoveEntity { entity_id },
        },
    );
    assert_eq!(plain.plugin_entities.len(), 1);
}

#[test]
fn project_scripts_are_trusted() {
    let mut project = starter_farm_project();
    project.scripts = Some(vec![ProjectScript {
        id: "economy".to_owned(),
        source: "api.on('onCommand', p => p.commandType === 'sleep' ? [{ type: 'setFlag', flag: 'slept', value: api.state.player().money }, { type: 'giveItem', itemId: 'seed-wheat', quantity: 1 }] : []); \
                 api.condition('always', () => true);"
            .to_owned(),
        ..Default::default()
    }]);
    let mut session = Session::new(&project);
    let before: u32 = session
        .state
        .player
        .inventory
        .iter()
        .filter(|slot| slot.item.id == "seed-wheat")
        .map(|slot| slot.quantity)
        .sum();
    session.run(&Command::Sleep);
    // The game's own ids, as written.
    assert!(session.flag("slept").is_some(), "{:?}", session.errors);
    let after: u32 = session
        .state
        .player
        .inventory
        .iter()
        .filter(|slot| slot.item.id == "seed-wheat")
        .map(|slot| slot.quantity)
        .sum();
    assert_eq!(after, before + 1);
    assert!(farm_sim::events::condition_met(
        &session.ctx,
        &session.state,
        &EventCondition::Plugin { condition: "always".to_owned(), args: Value::Null },
        None
    ));
}

#[test]
fn project_script_overruns_are_errors_not_strikes() {
    let mut project = starter_farm_project();
    project.scripts = Some(vec![ProjectScript {
        id: "slow".to_owned(),
        source: "api.on('onCommand', () => { for (;;) {} });".to_owned(),
        ..Default::default()
    }]);
    project.settings.scripts =
        Some(farm_sim::schema::ScriptSettings { fuel_per_call: Some(2_000_000), ..Default::default() });
    let mut session = Session::new(&project);
    for _ in 0..6 {
        session.run(&Command::Sleep);
    }
    let timeouts = session.errors.iter().filter(|error| error.kind == farm_plugins::PluginErrorKind::Timeout).count();
    assert_eq!(timeouts, 6, "every overrun is reported, none disables: {:?}", session.errors);
    assert!(session.errors.iter().all(|error| error.kind != farm_plugins::PluginErrorKind::Disabled));
}
