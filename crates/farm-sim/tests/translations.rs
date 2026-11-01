//! A game's own text in other languages (#228): the cartridge's `contentStrings` and the keys
//! `packs::localize_content` reads (the same as the F# `Translations` catalog).

use farm_sim::schema::{
    Cutscene, CutsceneChoice, CutsceneStep, Dialogue, DialogueOption, GameContent, Item, MailDefinition, Npc, NpcLine,
};
use farm_sim::{packs, Presentation};
use indexmap::IndexMap;

fn content() -> GameContent {
    GameContent {
        items: vec![Item { id: "hoe".into(), name: "Hoe".into(), description: "Digs.".into(), ..Item::default() }],
        npcs: vec![Npc {
            id: "bram".into(),
            name: "Bram".into(),
            lines: Some(vec![
                NpcLine { text: "Morning.".into(), id: Some("l-1".into()), ..NpcLine::default() },
                NpcLine { text: "Rain.".into(), ..NpcLine::default() },
            ]),
            dialogue: vec![Dialogue {
                id: "bram.intro".into(),
                npc_id: "bram".into(),
                text: "Hello.".into(),
                options: vec![
                    DialogueOption { text: "Bye.".into(), id: Some("o-1".into()), ..DialogueOption::default() },
                    DialogueOption { text: "Help?".into(), ..DialogueOption::default() },
                ],
                ..Dialogue::default()
            }],
            ..Npc::default()
        }],
        cutscenes: vec![Cutscene {
            id: "mill".into(),
            steps: vec![CutsceneStep {
                r#type: "choice".into(),
                text: Some("Celebrate?".into()),
                id: Some("s-1".into()),
                options: vec![CutsceneChoice { text: "Yes".into(), goto: String::new(), id: None }],
                ..CutsceneStep::default()
            }],
            ..Cutscene::default()
        }],
        mail: vec![MailDefinition { id: "hi".into(), subject: "Hi".into(), text: "Dear you".into(), ..MailDefinition::default() }],
        ..GameContent::default()
    }
}

fn spanish() -> IndexMap<String, String> {
    [
        ("item:hoe:name", "Azada"),
        ("npc:bram:name", "Bramo"),
        ("npc:bram:line:l-1", "Buenos días."),
        ("npc:bram:line:#1", "Lluvia."),
        ("dialogue:bram.intro:text", "Hola."),
        ("dialogue:bram.intro:option:o-1", "Adiós."),
        ("dialogue:bram.intro:option:#1", "¿Ayuda?"),
        ("cutscene:mill:step:s-1", "¿Celebrar?"),
        ("cutscene:mill:step:s-1:choice:#0", "Sí"),
        ("mail:hi:subject", "Hola"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect()
}

#[test]
fn every_text_field_is_localized_by_its_key() {
    let localized = packs::localize_content(content(), &spanish());
    assert_eq!(localized.items[0].name, "Azada");
    assert_eq!(localized.items[0].description, "Digs.", "a key the table lacks keeps the authored text");
    let npc = &localized.npcs[0];
    assert_eq!(npc.name, "Bramo");
    let lines = npc.lines.as_ref().expect("lines");
    assert_eq!((lines[0].text.as_str(), lines[1].text.as_str()), ("Buenos días.", "Lluvia."));
    let dialogue = &npc.dialogue[0];
    assert_eq!(dialogue.text, "Hola.");
    assert_eq!((dialogue.options[0].text.as_str(), dialogue.options[1].text.as_str()), ("Adiós.", "¿Ayuda?"));
    let step = &localized.cutscenes[0].steps[0];
    assert_eq!(step.text.as_deref(), Some("¿Celebrar?"));
    assert_eq!(step.options[0].text, "Sí");
    assert_eq!((localized.mail[0].subject.as_str(), localized.mail[0].text.as_str()), ("Hola", "Dear you"));
    // Only text changes.
    assert_eq!(localized.npcs[0].dialogue[0].options.len(), 2);
    assert_eq!(packs::localize_content(content(), &IndexMap::new()), content());
}

#[test]
fn the_player_language_picks_a_table_by_locale_or_language() {
    let presentation = Presentation {
        content_strings: [("es".to_owned(), spanish()), ("pt-BR".to_owned(), IndexMap::new())].into_iter().collect(),
        ..Presentation::default()
    };
    assert_eq!(presentation.content_locale("es", None), Some("es".to_owned()));
    assert_eq!(presentation.content_locale("", Some("es_MX.UTF-8")), Some("es".to_owned()));
    assert_eq!(presentation.content_locale("", Some("pt-br")), Some("pt-BR".to_owned()));
    assert_eq!(presentation.content_locale("en", Some("es")), None, "a chosen language without a table shows the source");
    assert_eq!(presentation.content_locale("", None), None);
    assert_eq!(presentation.localized(&content(), Some("es")).items[0].name, "Azada");
    assert_eq!(presentation.localized(&content(), None), content());
    // The cartridge section reads with and without the key.
    let json = serde_json::to_value(&presentation).expect("json");
    assert!(json.get("contentStrings").is_some());
    let empty = serde_json::to_value(Presentation::default()).expect("json");
    assert!(empty.get("contentStrings").is_none());
}
