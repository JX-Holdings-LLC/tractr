//! M4: dialogue and relationships — daily lines, text substitution, portraits by expression,
//! heart events, friendship decay, dating, marriage, a spouse who moves in and helps, children,
//! and the player character (clothing, the character creator).

mod common;

use common::{flag, make_engine, slot};
use farm_sim::schema::{
    heart_event_seen_flag, relationship_statuses, Cutscene, CutsceneStep, EventCondition, GameProject, GameState,
    HeartEvent, Item, NpcLine, PlayerLook, RelationshipSettings, WearDefinition,
};
use farm_sim::{engine, overlay, relationships, units, Command, EngineContext};
use serde_json::json;

/// Puts the NPC on the tile the player faces (3, 3).
fn face_npc(state: &mut GameState) {
    let npc = state.npcs.get_mut("npc-test").unwrap();
    npc.x = units::tiles(3);
    npc.y = units::tiles(3);
}

fn line(text: &str, conditions: Vec<EventCondition>) -> NpcLine {
    NpcLine { text: text.to_owned(), conditions, ..NpcLine::default() }
}

fn talk(ctx: &EngineContext, state: &mut GameState) {
    state.dialogue = None;
    engine::apply_command(ctx, state, &Command::Interact);
}

fn shown_text(ctx: &EngineContext, state: &GameState) -> Option<String> {
    overlay::overlay_view(ctx, state).dialogue_text
}

fn friendship(state: &GameState) -> i32 {
    state.social.get("npc-test").map_or(0, |social| social.friendship)
}

fn set_friendship(state: &mut GameState, points: i32) {
    state.social.entry("npc-test".into()).or_default().friendship = points;
}

fn status(state: &GameState) -> &str {
    farm_sim::social::relationship_status(state, "npc-test")
}

fn item(id: &str, name: &str) -> Item {
    Item {
        id: id.to_owned(),
        name: name.to_owned(),
        r#type: "crop".to_owned(),
        stackable: true,
        max_stack: 99,
        value: 10,
        ..Item::default()
    }
}

fn romance(project: &mut GameProject, settings: RelationshipSettings) {
    project.npcs[0].romanceable = Some(true);
    project.items.push(item("bouquet", "Bouquet"));
    project.items.push(item("pendant", "Pendant"));
    let bouquet = slot(&project.items, "bouquet", 1);
    let pendant = slot(&project.items, "pendant", 1);
    project.player.inventory.push(bouquet);
    project.player.inventory.push(pendant);
    project.settings.relationships = Some(settings);
}

fn married_settings() -> RelationshipSettings {
    RelationshipSettings {
        dating: true,
        dating_hearts: 2,
        dating_item_id: Some("bouquet".into()),
        marriage: true,
        proposal_hearts: 4,
        proposal_item_id: Some("pendant".into()),
        wedding_days: 2,
        home_scene_id: Some("scene-test".into()),
        home_x: Some(5),
        home_y: Some(5),
        spouse_helps: false,
        children: true,
        child_days: 3,
        max_children: 1,
        child_names: vec!["Pip".into()],
        ..RelationshipSettings::default()
    }
}

#[test]
fn daily_lines_replace_the_greeting_by_day() {
    let (ctx, mut state) = make_engine(
        |project| {
            project.npcs[0].lines = Some(vec![
                line("Lovely spring, {player}!", vec![EventCondition::Season { seasons: vec!["spring".into()] }]),
                line("Brr.", vec![EventCondition::Season { seasons: vec!["winter".into()] }]),
            ]);
        },
        "lines",
    );
    state.player.name = "Ada".into();
    face_npc(&mut state);
    talk(&ctx, &mut state);
    let open = state.dialogue.clone().unwrap();
    assert_eq!((open.dialogue_id.as_str(), open.line), ("dlg-1", Some(0)));
    assert_eq!(shown_text(&ctx, &state).as_deref(), Some("Lovely spring, Ada!"));
    // The dialogue's options are still there.
    assert_eq!(overlay::overlay_view(&ctx, &state).visible_dialogue_options.len(), 2);

    // No line holds: the dialogue's own text.
    state.clock.season = "summer".into();
    talk(&ctx, &mut state);
    assert_eq!(state.dialogue.as_ref().unwrap().line, None);
    assert_eq!(shown_text(&ctx, &state).as_deref(), Some("Hello!"));
}

#[test]
fn lines_vary_by_day_and_stay_all_day() {
    let lines: Vec<NpcLine> = (0..6).map(|index| line(&format!("Line {index}"), vec![])).collect();
    let (ctx, mut state) = make_engine(move |project| project.npcs[0].lines = Some(lines), "variety");
    face_npc(&mut state);
    let mut seen = std::collections::BTreeSet::new();
    for day in 1..=30 {
        state.clock.day = day;
        talk(&ctx, &mut state);
        let first = state.dialogue.as_ref().unwrap().line;
        talk(&ctx, &mut state);
        assert_eq!(state.dialogue.as_ref().unwrap().line, first, "the same line all day");
        seen.insert(first.unwrap());
    }
    assert!(seen.len() >= 4, "{seen:?}");
}

#[test]
fn an_npc_with_lines_and_no_dialogue_says_its_line() {
    let (ctx, mut state) = make_engine(
        |project| {
            project.npcs[0].dialogue.clear();
            project.npcs[0].lines = Some(vec![line("Just a line.", vec![])]);
        },
        "line-only",
    );
    face_npc(&mut state);
    talk(&ctx, &mut state);
    assert_eq!(shown_text(&ctx, &state).as_deref(), Some("Just a line."));
    assert!(overlay::overlay_view(&ctx, &state).visible_dialogue_options.is_empty());
    engine::apply_command(&ctx, &mut state, &Command::CloseDialogue);
    assert!(state.dialogue.is_none());
}

#[test]
fn lines_read_hearts_of_the_npc_speaking() {
    let (ctx, mut state) = make_engine(
        |project| {
            project.npcs[0].lines = Some(vec![
                line("Hi, friend.", vec![EventCondition::Hearts { npc_id: String::new(), min: Some(2), max: None }]),
                line(
                    "Hello, stranger.",
                    vec![EventCondition::Hearts { npc_id: String::new(), min: None, max: Some(1) }],
                ),
            ]);
        },
        "hearts",
    );
    face_npc(&mut state);
    talk(&ctx, &mut state);
    assert_eq!(shown_text(&ctx, &state).as_deref(), Some("Hello, stranger."));
    set_friendship(&mut state, 260);
    talk(&ctx, &mut state);
    assert_eq!(shown_text(&ctx, &state).as_deref(), Some("Hi, friend."));
}

#[test]
fn expressions_come_from_the_line_or_the_dialogue() {
    let (ctx, mut state) = make_engine(
        |project| {
            project.npcs[0].dialogue[0].expression = Some("calm".into());
            project.npcs[0].lines = Some(vec![NpcLine {
                expression: Some("happy".into()),
                ..line("Yay!", vec![EventCondition::Flag { flag: "party".into(), value: true }])
            }]);
        },
        "faces",
    );
    face_npc(&mut state);
    talk(&ctx, &mut state);
    assert_eq!(overlay::overlay_view(&ctx, &state).dialogue_expression.as_deref(), Some("calm"));
    state.flags.insert("party".into(), json!(true));
    talk(&ctx, &mut state);
    assert_eq!(overlay::overlay_view(&ctx, &state).dialogue_expression.as_deref(), Some("happy"));
}

#[test]
fn heart_events_play_once_at_their_hearts() {
    let (ctx, mut state) = make_engine(
        |project| {
            project.cutscenes = Some(vec![Cutscene {
                id: "cut-picnic".into(),
                steps: vec![CutsceneStep {
                    r#type: "say".into(),
                    text: Some("A picnic!".into()),
                    ..CutsceneStep::default()
                }],
                ..Cutscene::default()
            }]);
            project.heart_events = Some(vec![HeartEvent {
                id: "heart-picnic".into(),
                npc_id: "npc-test".into(),
                hearts: 3,
                cutscene_id: "cut-picnic".into(),
                ..HeartEvent::default()
            }]);
        },
        "heart",
    );
    face_npc(&mut state);
    talk(&ctx, &mut state);
    assert!(state.cutscene.is_none() && state.dialogue.is_some());
    set_friendship(&mut state, 3 * 125);
    talk(&ctx, &mut state);
    assert_eq!(state.cutscene.as_ref().map(|c| c.cutscene_id.as_str()), Some("cut-picnic"));
    assert!(state.dialogue.is_none());
    assert_eq!(flag(&state, &heart_event_seen_flag("heart-picnic")), Some(&json!(true)));
    state.cutscene = None;
    talk(&ctx, &mut state);
    assert!(state.cutscene.is_none() && state.dialogue.is_some());
}

#[test]
fn friendship_fades_on_days_without_a_word() {
    let (ctx, mut state) = make_engine(
        |project| {
            project.settings.relationships =
                Some(RelationshipSettings { decay_per_day: 10, ..RelationshipSettings::default() })
        },
        "decay",
    );
    set_friendship(&mut state, 100);
    engine::apply_command(&ctx, &mut state, &Command::Sleep);
    assert_eq!(friendship(&state), 90);
    face_npc(&mut state);
    talk(&ctx, &mut state);
    engine::apply_command(&ctx, &mut state, &Command::Sleep);
    assert_eq!(friendship(&state), 90);
}

#[test]
fn no_decay_leaves_states_as_they_were() {
    let (ctx, mut state) = make_engine(|_| {}, "no-decay");
    face_npc(&mut state);
    talk(&ctx, &mut state);
    assert!(state.social.get("npc-test").is_none_or(|social| social.talked_day.is_none()));
}

#[test]
fn dating_marriage_spouse_and_children() {
    let (ctx, mut state) = make_engine(|project| romance(project, married_settings()), "romance");
    face_npc(&mut state);
    let gift = |state: &mut GameState, item: &str| {
        engine::apply_command(&ctx, state, &Command::GiveGift { item_id: item.into() });
    };
    // Not enough hearts: refused, the bouquet is kept.
    gift(&mut state, "bouquet");
    assert_eq!(status(&state), relationship_statuses::SINGLE);
    assert!(state.player.inventory.iter().any(|slot| slot.item.id == "bouquet"));
    // The pendant before dating does nothing either.
    set_friendship(&mut state, 5 * 125);
    gift(&mut state, "pendant");
    assert_eq!(status(&state), relationship_statuses::SINGLE);
    gift(&mut state, "bouquet");
    assert_eq!(status(&state), relationship_statuses::DATING);
    assert!(!state.player.inventory.iter().any(|slot| slot.item.id == "bouquet"));
    gift(&mut state, "pendant");
    assert_eq!(status(&state), relationship_statuses::ENGAGED);
    let wedding = state.social["npc-test"].status_day.unwrap();
    assert_eq!(wedding, state.clock.day + 2);

    engine::apply_command(&ctx, &mut state, &Command::Sleep);
    assert_eq!(status(&state), relationship_statuses::ENGAGED);
    engine::apply_command(&ctx, &mut state, &Command::Sleep);
    assert_eq!(status(&state), relationship_statuses::MARRIED);
    assert_eq!(relationships::spouse(&state), Some("npc-test"));
    let npc = &state.npcs["npc-test"];
    assert_eq!((npc.x, npc.y), (units::tiles(5), units::tiles(5)));
    assert!(relationships::lives_at_home(&ctx, &state, "npc-test"));

    for _ in 0..3 {
        engine::apply_command(&ctx, &mut state, &Command::Sleep);
    }
    assert_eq!(state.children.len(), 1);
    assert_eq!(state.children[0].name, "Pip");
    for _ in 0..5 {
        engine::apply_command(&ctx, &mut state, &Command::Sleep);
    }
    assert_eq!(state.children.len(), 1, "at most maxChildren");
}

#[test]
fn dating_without_a_gift_starts_at_the_hearts() {
    let (ctx, mut state) = make_engine(
        |project| {
            project.npcs[0].romanceable = Some(true);
            project.settings.relationships =
                Some(RelationshipSettings { dating: true, dating_hearts: 2, ..RelationshipSettings::default() });
        },
        "auto-dating",
    );
    engine::apply_command(&ctx, &mut state, &Command::Sleep);
    assert_eq!(status(&state), relationship_statuses::SINGLE);
    set_friendship(&mut state, 250);
    engine::apply_command(&ctx, &mut state, &Command::Sleep);
    assert_eq!(status(&state), relationship_statuses::DATING);
}

#[test]
fn relationship_conditions_and_outcomes() {
    let (ctx, mut state) = make_engine(|_| {}, "rel");
    let married = EventCondition::Relationship { npc_id: "npc-test".into(), status: "married".into() };
    let single_anyone = EventCondition::Relationship { npc_id: String::new(), status: "single".into() };
    assert!(farm_sim::events::condition_met(&ctx, &state, &single_anyone, None));
    farm_sim::events::apply_outcomes(
        &ctx,
        &mut state,
        &[farm_sim::schema::EventOutcome {
            npc_id: Some("npc-test".into()),
            status: Some("married".into()),
            ..farm_sim::schema::EventOutcome::of(farm_sim::schema::OutcomeKind::SetRelationship)
        }],
        0,
    );
    assert!(farm_sim::events::condition_met(&ctx, &state, &married, None));
    assert!(!farm_sim::events::condition_met(&ctx, &state, &single_anyone, None));
}

#[test]
fn clothes_are_worn_and_taken_off() {
    let (ctx, mut state) = make_engine(
        |project| {
            let wear = |slot: &str, color: &str| {
                Some(WearDefinition { slot: slot.into(), color: Some(color.into()), visual: None })
            };
            project.items.push(Item { wear: wear("hat", "#aa2222"), ..item("hat-red", "Red Hat") });
            project.items.push(Item { wear: wear("hat", "#2222aa"), ..item("hat-blue", "Blue Hat") });
            let red = slot(&project.items, "hat-red", 1);
            let blue = slot(&project.items, "hat-blue", 1);
            project.player.inventory.push(red);
            project.player.inventory.push(blue);
        },
        "clothes",
    );
    let held = |state: &GameState, id: &str| state.player.inventory.iter().any(|slot| slot.item.id == id);
    engine::apply_command(&ctx, &mut state, &Command::WearItem { item_id: "hat-red".into() });
    assert_eq!(state.player.worn.len(), 1);
    assert!(!held(&state, "hat-red"));
    // Another hat swaps with the one worn.
    engine::apply_command(&ctx, &mut state, &Command::WearItem { item_id: "hat-blue".into() });
    assert_eq!(state.player.worn[0].item.id, "hat-blue");
    assert!(held(&state, "hat-red"));
    // Seeds are not clothing.
    engine::apply_command(&ctx, &mut state, &Command::WearItem { item_id: "seed-wheat".into() });
    assert_eq!(state.player.worn.len(), 1);
    engine::apply_command(&ctx, &mut state, &Command::TakeOff { slot: "hat".into() });
    assert!(state.player.worn.is_empty());
    assert!(held(&state, "hat-blue"));
}

#[test]
fn the_character_creator_names_and_colours_the_player_before_play() {
    let (ctx, mut state) = make_engine(|_| {}, "creator");
    let look = PlayerLook { hair: Some("#ffcc00".into()), shirt: Some("red".into()), ..PlayerLook::default() };
    engine::apply_command(
        &ctx,
        &mut state,
        &Command::SetPlayerLook { name: "  Ada ".into(), farm_name: "Sunny Acres".into(), look },
    );
    assert_eq!((state.player.name.as_str(), state.player.farm_name.as_str()), ("Ada", "Sunny Acres"));
    let look = state.player.look.clone().unwrap();
    assert_eq!((look.hair.as_deref(), look.shirt.as_deref()), (Some("#ffcc00"), None));
    // Once the game runs, a player cannot rename themselves this way.
    engine::advance_tick(&ctx, &mut state, 1);
    engine::apply_command(
        &ctx,
        &mut state,
        &Command::SetPlayerLook { name: "Bob".into(), farm_name: String::new(), look: PlayerLook::default() },
    );
    assert_eq!(state.player.name, "Ada");
}
