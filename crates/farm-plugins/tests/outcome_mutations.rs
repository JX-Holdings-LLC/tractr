//! M9: plugins answer with the M3–M7 outcomes, keep their own data in the save and hear it back
//! in `onLoad`.

mod common;

use common::plugin;
use farm_plugins::{
    parse_mutation, plugin_specs_from_packs, PluginHost, PluginHostOptions, PluginRuntime, WasmPluginHost,
    MAX_SAVED_DATA_BYTES,
};
use farm_sim::schema::{ContentPack, PackManifest, PackPermissions, PluginMutation};
use indexmap::IndexMap;
use serde_json::json;

fn scoped_pack(mutations: &[&str], source: &str) -> ContentPack {
    ContentPack {
        manifest: PackManifest {
            id: "glow".to_owned(),
            name: "Glow".to_owned(),
            version: "1.0.0".to_owned(),
            permissions: PackPermissions {
                hooks: vec!["onDayStart".to_owned(), "onLoad".to_owned()],
                mutations: Some(mutations.iter().map(|m| (*m).to_owned()).collect()),
                ..Default::default()
            },
            ..Default::default()
        },
        plugins: vec![plugin("p", source, &["onDayStart", "onLoad"])],
        ..Default::default()
    }
}

fn answer(pack: &ContentPack) -> (Vec<PluginMutation>, Vec<String>) {
    let mut host = WasmPluginHost::new(plugin_specs_from_packs([pack]), PluginHostOptions::default());
    let results = host.dispatch("onDayStart", r#"{"day":1,"season":"spring","year":1}"#);
    let mutations = results.iter().flat_map(|r| r.mutations.clone()).collect();
    let errors = results.iter().flat_map(|r| r.errors.iter().map(|e| e.message.clone())).collect();
    (mutations, errors)
}

#[test]
fn the_outcome_mutations_parse_with_their_fields_and_ranges() {
    let ok = |value: serde_json::Value| parse_mutation(&value).map_err(|e| e.to_string());
    assert_eq!(
        ok(json!({"type": "moveNpc", "npcId": "mira", "x": 3, "y": 4})),
        Ok(PluginMutation::MoveNpc { npc_id: "mira".to_owned(), scene_id: None, x: 3, y: 4 })
    );
    assert_eq!(
        ok(json!({"type": "addShopStock", "shopId": "s", "itemId": "i", "price": 40})),
        Ok(PluginMutation::AddShopStock {
            shop_id: "s".to_owned(),
            item_id: "i".to_owned(),
            price: Some(40),
            daily_limit: None
        })
    );
    assert!(ok(json!({"type": "setVariable", "variable": "v", "value": "text"})).is_ok());
    assert!(ok(json!({"type": "setVariable", "variable": "v", "value": 2.5})).is_ok());
    assert_eq!(
        ok(json!({"type": "setVariable", "variable": "v", "value": true})),
        Err("setVariable: value: expected a number (at most 1e9 either way) or a string".to_owned())
    );
    assert!(ok(json!({"type": "placeMachine", "x": 1, "y": 1, "machineTypeId": "keg"})).is_ok());
    assert_eq!(ok(json!({"type": "removeNode", "x": -1, "y": 1})), Err("removeNode: x: must be >= 0".to_owned()));
    assert_eq!(
        ok(json!({"type": "setRelationship", "npcId": "mira", "status": "best friends"})),
        Err("setRelationship: status: expected one of single, dating, engaged, married".to_owned())
    );
    assert_eq!(ok(json!({"type": "modifyHealth", "delta": -5})), Ok(PluginMutation::ModifyHealth { delta: -5 }));
    assert_eq!(
        ok(json!({"type": "saveData", "data": {"count": 3, "seen": ["a"]}})),
        Ok(PluginMutation::SaveData { data: json!({"count": 3, "seen": ["a"]}) })
    );
    let big = "x".repeat(MAX_SAVED_DATA_BYTES);
    assert!(ok(json!({"type": "saveData", "data": big}))
        .unwrap_err()
        .starts_with("saveData: data: at most 16384 bytes"));
}

#[test]
fn outcome_mutations_name_the_packs_own_content_unless_any_is_granted() {
    let source = "api.on('onDayStart', () => [\
        { type: 'startCutscene', cutsceneId: 'glow-show' },\
        { type: 'sendMail', mailId: 'other:letter' },\
        { type: 'setVariable', variable: 'lanterns', value: 3 },\
        { type: 'setVariable', variable: 'event:x:fired', value: 1 },\
        { type: 'modifyHealth', delta: 2 },\
        { type: 'saveData', data: { day: 1 } }\
    ])";
    let (mutations, errors) =
        answer(&scoped_pack(&["startCutscene", "sendMail", "setVariable", "modifyHealth"], source));
    assert_eq!(
        mutations,
        [
            PluginMutation::StartCutscene { cutscene_id: "glow:glow-show".to_owned() },
            PluginMutation::SetVariable { variable: "glow:lanterns".to_owned(), value: json!(3) },
            PluginMutation::ModifyHealth { delta: 2 },
            // saveData needs no capability: it keeps only the plugin's own data.
            PluginMutation::SaveData { data: json!({"day": 1}) },
        ]
    );
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert!(errors[0].contains("sendMail: mailId 'other:letter' is not this pack's"), "{errors:?}");
    assert!(errors[1].contains("variable 'event:x:fired' is not this pack's"), "{errors:?}");

    // With `:any`, ids are used as written; reserved event flags stay refused.
    let (mutations, errors) = answer(&scoped_pack(&["setVariable:any", "sendMail:any"], source));
    assert!(mutations.contains(&PluginMutation::SendMail { mail_id: "other:letter".to_owned() }));
    assert!(mutations.contains(&PluginMutation::SetVariable { variable: "lanterns".to_owned(), value: json!(3) }));
    assert!(errors.iter().any(|e| e.contains("variables starting with 'event:' belong to events")), "{errors:?}");
    assert!(errors.iter().any(|e| e.contains("startCutscene: not in the pack's permissions.mutations")), "{errors:?}");
}

#[test]
fn each_plugin_hears_onload_alone_with_the_data_it_saved() {
    let source = "var restored = null;\
        api.on('onLoad', p => { restored = p.data; return [{ type: 'message', text: JSON.stringify(p) }]; });\
        api.on('onDayStart', () => [{ type: 'message', text: 'count ' + (restored ? restored.count : 0) }]);";
    let first = scoped_pack(&["message"], source);
    let mut second = scoped_pack(&["message"], source);
    second.manifest.id = "other".to_owned();
    // A plugin not granted onLoad does not hear it.
    let mut deaf = scoped_pack(&["message"], source);
    deaf.manifest.id = "deaf".to_owned();
    deaf.manifest.permissions.hooks = vec!["onDayStart".to_owned()];

    let specs = plugin_specs_from_packs([&first, &second, &deaf]);
    let host = WasmPluginHost::new(specs.clone(), PluginHostOptions::default());
    let mut runtime = PluginRuntime::new(host, &specs);
    let mut saved = IndexMap::new();
    saved.insert("glow:p".to_owned(), json!({"count": 7}));
    assert!(runtime.restore(&saved).is_empty());
    let texts: Vec<String> = runtime
        .drain()
        .into_iter()
        .filter_map(|queued| match queued.mutation {
            PluginMutation::Message { text } => Some(format!("{}={text}", queued.plugin_id)),
            _ => None,
        })
        .collect();
    assert_eq!(texts, [r#"glow:p={"data":{"count":7}}"#, r#"other:p={"data":null}"#]);

    // What onLoad restored, the plugin's handlers use.
    runtime.dispatch("onDayStart", r#"{"day":2,"season":"spring","year":1}"#);
    let texts: Vec<String> = runtime
        .drain()
        .into_iter()
        .filter_map(|queued| match queued.mutation {
            PluginMutation::Message { text } => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(texts, ["count 7", "count 0", "count 0"]);
}

#[test]
fn a_plugin_rebuilt_after_an_overrun_hears_onload_again_with_what_it_saved() {
    // Day 2 overruns (an endless loop), so the instance is rebuilt from source and loses
    // `restored`; it hears onLoad again with the data saved so far and keeps counting from it.
    let source = "var restored = null;\
        api.on('onLoad', p => { restored = p.data; return []; });\
        api.on('onDayStart', p => {\
            if (p.day === 2) { for (;;) {} }\
            var count = (restored ? restored.count : 0) + 1;\
            restored = { count: count };\
            return [{ type: 'message', text: 'count ' + count }, { type: 'saveData', data: restored }];\
        });";
    let pack = scoped_pack(&["message"], source);
    let specs = plugin_specs_from_packs([&pack]);
    let host = WasmPluginHost::new(
        specs.clone(),
        PluginHostOptions { fuel_per_call: 5_000_000, ..PluginHostOptions::default() },
    );
    let mut runtime = PluginRuntime::new(host, &specs);
    let mut saved = IndexMap::new();
    saved.insert("glow:p".to_owned(), json!({"count": 7}));
    runtime.restore(&saved);
    runtime.drain();

    let day = |runtime: &mut PluginRuntime, saved: &mut IndexMap<String, serde_json::Value>, day: u32| {
        runtime.dispatch("onDayStart", &format!(r#"{{"day":{day},"season":"spring","year":1}}"#));
        let mut texts = Vec::new();
        for queued in runtime.drain() {
            match queued.mutation {
                PluginMutation::Message { text } => texts.push(text),
                PluginMutation::SaveData { data } => {
                    saved.insert(queued.plugin_id.clone(), data);
                }
                _ => {}
            }
        }
        texts
    };
    assert_eq!(day(&mut runtime, &mut saved, 1), ["count 8"]);
    assert_eq!(day(&mut runtime, &mut saved, 2), Vec::<String>::new());
    // The rebuilt plugin hears what it saved before the overrun.
    assert!(runtime.restore_rebuilt(&saved).is_empty());
    runtime.drain();
    assert_eq!(day(&mut runtime, &mut saved, 3), ["count 9"]);
    // Each rebuild is told once.
    assert!(runtime.host_mut().take_rebuilt().is_empty());
}
