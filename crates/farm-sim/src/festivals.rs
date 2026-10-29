//! Authored festival places: entry, attendance, a shop and one daily activity.
use crate::{
    effects::message_levels, events, game_time, inventory, messages, schema::*, units, world::world_movement, Effect,
    Effects, EngineContext,
};
use indexmap::IndexMap;

pub fn attending(state: &GameState, npc_id: &str) -> bool {
    state.festival.as_ref().is_some_and(|session| session.npcs.contains_key(npc_id))
}

fn program<'a>(ctx: &'a EngineContext, id: &str) -> Option<&'a FestivalProgram> {
    ctx.content.settings.calendar.festivals.iter().find(|festival| festival.id == id)?.program.as_ref()
}

/// Return an interaction only for an authored festival entrance, exit, shop, activity or guest.
pub fn interact(ctx: &EngineContext, state: &mut GameState, x: i32, y: i32) -> Option<Effects> {
    if let Some(session) = state.festival.clone() {
        let p = program(ctx, &session.festival_id)?;
        if state.player.scene_id != p.scene_id {
            return None;
        }
        if x == p.arrival_x && y == p.arrival_y {
            return Some(leave(ctx, state, true));
        }
        if let (Some(shop), Some(tile)) = (&p.shop_id, &p.shop_tile) {
            if tile.x == x && tile.y == y {
                return Some(crate::economy::handle_open_shop(ctx, state, shop));
            }
        }
        if let Some(activity) = &p.activity {
            if activity.x == x && activity.y == y {
                return Some(play_activity(ctx, state, &session.festival_id, activity));
            }
        }
        for guest in &p.attendance {
            if guest.x == x && guest.y == y {
                if let Some(id) = &guest.dialogue_id {
                    if crate::dialogue_system::find_dialogue(ctx, &guest.npc_id, id).is_some() {
                        state.dialogue =
                            Some(DialogueState { npc_id: guest.npc_id.clone(), dialogue_id: id.clone(), line: None });
                        crate::relationships::note_talk(ctx, state, &guest.npc_id);
                        return Some(crate::quests::progress_quests(ctx, state, "talk", &guest.npc_id, 1));
                    }
                }
            }
        }
        return None;
    }
    let festival = game_time::festival_today(&ctx.content.settings.calendar, &state.clock)?;
    let p = festival.program.as_ref()?;
    if p.entrance_scene_id != state.player.scene_id || p.entrance_x != x || p.entrance_y != y {
        return None;
    }
    let minute = state.clock.time_minutes / units::MINUTE;
    if minute < p.open_minute
        || minute >= p.close_minute
        || !events::conditions_hold(
            ctx,
            state,
            &p.conditions,
            &events::ConditionScope { salt: &festival.id, ..Default::default() },
        )
    {
        return Some(vec![Effect::say(message_levels::INFO, &messages::FESTIVAL_CLOSED)]);
    }
    if !world_movement::scene_exists(ctx, state, &p.scene_id) {
        return Some(Vec::new());
    }
    world_movement::ensure_scene(ctx, state, &p.scene_id);
    let landing = state
        .world
        .scenes
        .iter()
        .find(|scene| scene.id == p.scene_id)
        .and_then(|scene| world_movement::landing_tile(ctx, scene, p.arrival_x, p.arrival_y));
    if landing.is_none() {
        return Some(vec![Effect::say(message_levels::ERROR, &messages::FESTIVAL_UNAVAILABLE)]);
    }
    let mut saved = IndexMap::new();
    for guest in &p.attendance {
        if let Some(npc) = state.npcs.get_mut(&guest.npc_id) {
            saved.insert(guest.npc_id.clone(), npc.clone());
            npc.scene_id.clone_from(&p.scene_id);
            npc.x = units::tiles(guest.x);
            npc.y = units::tiles(guest.y);
            npc.path = None;
        }
    }
    state.festival = Some(FestivalSession {
        festival_id: festival.id.clone(),
        return_scene_id: state.player.scene_id.clone(),
        return_x: state.player.x,
        return_y: state.player.y,
        npcs: saved,
    });
    let visit = state.festival_visits.entry(festival.id.clone()).or_default();
    if visit.day != state.clock.day {
        *visit = FestivalVisit { day: state.clock.day, activity_done: false };
    }
    let (_, effects) = world_movement::land_player(ctx, state, &p.scene_id, p.arrival_x, p.arrival_y);
    Some(effects)
}

fn play_activity(
    ctx: &EngineContext,
    state: &mut GameState,
    festival_id: &str,
    activity: &FestivalActivity,
) -> Effects {
    if state.festival_visits.get(festival_id).is_some_and(|visit| visit.day == state.clock.day && visit.activity_done) {
        return vec![Effect::say(message_levels::INFO, &messages::FESTIVAL_ALREADY_PLAYED)];
    }
    if let Some(contest) = &activity.contest {
        let qualities = ctx.crop_qualities();
        let rank =
            |quality: Option<&str>| qualities.iter().position(|tier| Some(tier.id.as_str()) == quality).unwrap_or(0);
        let minimum = rank(contest.minimum_quality.as_deref());
        let offered = state
            .player
            .inventory
            .iter()
            .filter(|slot| {
                slot.item.id == contest.item_id && slot.quantity > 0 && rank(slot.quality.as_deref()) >= minimum
            })
            .max_by_key(|slot| rank(slot.quality.as_deref()))
            .cloned();
        let Some(slot) = offered else {
            return vec![Effect::say(message_levels::INFO, &messages::FESTIVAL_CONTEST_ITEM)];
        };
        if contest.consume_item {
            state.player.inventory = inventory::remove_item_with_quality(
                &state.player.inventory,
                &contest.item_id,
                slot.quality.as_deref(),
                1,
            );
        }
    }
    state.festival_visits.entry(festival_id.to_owned()).or_default().activity_done = true;
    let mut effects = Vec::new();
    if let Some(contest) = &activity.contest {
        effects.extend(events::apply_outcomes(ctx, state, &contest.rewards, 0));
    }
    if let Some(id) = &activity.action_id {
        let result = events::perform_action(ctx, state, id);
        if !result.ran && activity.contest.is_none() && activity.cutscene_id.is_none() && activity.minigame_id.is_none()
        {
            if let Some(visit) = state.festival_visits.get_mut(festival_id) {
                visit.activity_done = false;
            }
        }
        effects.extend(result.effects);
    }
    if let Some(id) = &activity.cutscene_id {
        effects.extend(events::apply_outcomes(
            ctx,
            state,
            &[EventOutcome { r#type: "startCutscene".into(), cutscene_id: Some(id.clone()), ..Default::default() }],
            0,
        ));
    }
    if let Some(id) = &activity.minigame_id {
        effects.extend(events::start_minigame_session(ctx, state, id, None));
    }
    effects
}

/// Restore attendees and the entrance location. Leaving can advance the clock or end the day.
pub fn leave(ctx: &EngineContext, state: &mut GameState, advance_time: bool) -> Effects {
    let Some(session) = state.festival.take() else { return Vec::new() };
    let p = program(ctx, &session.festival_id);
    for (id, npc) in session.npcs {
        state.npcs.insert(id, npc);
    }
    world_movement::put_player(state, &session.return_scene_id, session.return_x, session.return_y);
    state.dialogue = None;
    state.shop = None;
    state.minigame = None;
    crate::cutscenes::stop(state);
    if advance_time {
        if let Some(p) = p {
            state.clock.time_minutes = state.clock.time_minutes.max(units::minutes(p.close_minute));
            if p.end_day {
                return game_time::perform_sleep(ctx, state, game_time::SleepOptions { collapsed: false });
            }
        }
    }
    Vec::new()
}

/// End a visit at closing time, or restore guests if another event moved the player away.
pub fn advance(ctx: &EngineContext, state: &mut GameState) -> Effects {
    let Some(session) = state.festival.as_ref() else { return Vec::new() };
    let Some(p) = program(ctx, &session.festival_id) else { return leave(ctx, state, false) };
    if state.player.scene_id != p.scene_id {
        // An event or door already chose a destination. Restore attendees without
        // moving the player again or cancelling a cutscene started by that event.
        if let Some(session) = state.festival.take() {
            for (id, npc) in session.npcs {
                state.npcs.insert(id, npc);
            }
        }
        return Vec::new();
    }
    if state.clock.time_minutes / units::MINUTE >= p.close_minute {
        return leave(ctx, state, true);
    }
    Vec::new()
}

/// Missing a festival is evaluated once before the overnight date advances.
pub fn finish_day(ctx: &EngineContext, state: &mut GameState) -> Effects {
    let mut effects = leave(ctx, state, false);
    if let Some(festival) = game_time::festival_today(&ctx.content.settings.calendar, &state.clock) {
        if state.festival_visits.get(&festival.id).is_none_or(|visit| visit.day != state.clock.day) {
            if let Some(p) = &festival.program {
                effects.extend(events::apply_outcomes(ctx, state, &p.on_miss, 0));
            }
        }
    }
    effects
}
