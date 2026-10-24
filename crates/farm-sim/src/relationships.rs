//! Relationships (M4): daily lines, heart events, friendship decay, dating, marriage, a spouse
//! who moves in and helps, and children. Every part is optional: friendship decay, dating,
//! marriage and children each wait for the game's [`RelationshipSettings`] to turn them on, and
//! daily lines and heart events for the creator to write some.
//!
//! - **Daily lines.** Talking to an NPC opens its first dialogue as before, but on a day when
//!   some of its `lines` hold (their conditions are the events'; `hearts` and `relationship`
//!   with an empty NPC mean the NPC itself) one of them is shown in place of the dialogue's
//!   text, with the dialogue's options. An NPC with lines and no dialogue just says the line.
//!   The line is picked by weight, the same one all day and a new pick each day.
//! - **Heart events.** A heart event plays its cutscene once, when the player has the hearts
//!   with its NPC and its conditions hold: on entering its scene, or (without a scene) on
//!   talking to the NPC, in place of the conversation.
//! - **Decay.** Overnight, an NPC the player neither talked to nor gave a gift that day loses
//!   `decayPerDay` friendship points.
//! - **Dating and marriage.** Giving a romanceable NPC the dating gift at `datingHearts` or more
//!   starts dating (without a dating gift, dating starts by itself the morning after the hearts
//!   are reached); giving a dated NPC the proposal gift at `proposalHearts` or more is an
//!   engagement, and `weddingDays` later the two marry that morning (the wedding cutscene plays).
//!   One partner at a time. The `setRelationship` outcome sets any stage directly.
//! - **The spouse** lives at the game's home tile (when it has one) every morning, off its
//!   schedule, and with `spouseHelps` does one chore each morning: waters the crops of the
//!   player's farm (the start scene), feeds the animals, or brings one of `spouseGiftItemIds`.
//! - **Children** arrive every `childDays` of marriage, up to `maxChildren`, named from
//!   `childNames`. They live with the spouse.

use crate::effects::{message_levels, Effect};
use crate::engine_types::{Effects, EngineContext};
use crate::events::{self, ConditionScope};
use crate::farming::farming_actions;
use crate::inventory;
use crate::messages;
use crate::rng::Rng;
use crate::schema::{
    heart_event_seen_flag, relationship_statuses, ChildState, CutsceneState, GameState, Npc, RelationshipSettings,
};
use crate::social;
use crate::text;
use crate::units;
use serde_json::Value;

/// The game's relationship rules (the defaults, everything off, when it has none).
fn settings(ctx: &EngineContext) -> RelationshipSettings {
    ctx.content.settings.relationships.clone().unwrap_or_default()
}

/// The daily line `npc` says today, as an index into its `lines` (see the module docs).
pub fn pick_line(ctx: &EngineContext, state: &GameState, npc: &Npc) -> Option<u32> {
    let lines = npc.lines.as_ref()?;
    let mut candidates: Vec<(usize, u32)> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let salt = format!("line:{}:{index}", npc.id);
        let scope = ConditionScope { pos: None, speaker: Some(&npc.id), salt: &salt };
        if events::conditions_hold(ctx, state, &line.conditions, &scope) {
            candidates.push((index, line.weight.unwrap_or(1)));
        }
    }
    let total: u64 = candidates.iter().map(|(_, weight)| u64::from(*weight)).sum();
    if total == 0 {
        return None;
    }
    // A draw from the game's seed, the NPC and the day: the same all day, without touching the
    // game's random stream.
    let mut bytes = Vec::new();
    bytes.extend_from_slice(state.meta.engine_seed.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(npc.id.as_bytes());
    bytes.extend_from_slice(&state.clock.day.to_le_bytes());
    let draw = twox_hash::XxHash3_64::oneshot(&bytes) as u32;
    let mut roll = ((u128::from(draw) * u128::from(total)) >> 32) as u64;
    for (index, weight) in &candidates {
        let weight = u64::from(*weight);
        if roll < weight {
            return u32::try_from(*index).ok();
        }
        roll -= weight;
    }
    candidates.last().and_then(|(index, _)| u32::try_from(*index).ok())
}

/// The player talked to `npc_id` today (friendship decay counts it; only kept when the game has
/// decay, so other games' states do not change).
pub fn note_talk(ctx: &EngineContext, state: &mut GameState, npc_id: &str) {
    if settings(ctx).decay_per_day == 0 {
        return;
    }
    let today = state.clock.day;
    state.social.entry(npc_id.to_owned()).or_default().talked_day = Some(today);
}

/// Where a heart event may start.
#[derive(Debug, Clone, Copy)]
pub enum Place<'a> {
    /// The player entered this scene.
    Scene(&'a str),
    /// The player is talking to this NPC.
    Talk(&'a str),
}

/// Starts the first heart event that is due at `place` (see the module docs); whether one
/// started.
pub fn start_heart_event(ctx: &EngineContext, state: &mut GameState, place: Place<'_>) -> bool {
    if state.cutscene.is_some() {
        return false;
    }
    for heart_event in &ctx.content.heart_events {
        let scene = heart_event.scene_id.as_deref().filter(|scene| !scene.is_empty());
        let here = match (place, scene) {
            (Place::Scene(entered), Some(scene)) => entered == scene,
            (Place::Talk(npc_id), None) => npc_id == heart_event.npc_id,
            _ => false,
        };
        if !here || ctx.npc(&heart_event.npc_id).is_none() {
            continue;
        }
        if text::truthy(state.flags.get(&heart_event_seen_flag(&heart_event.id))) {
            continue;
        }
        let hearts = social::hearts(social::friendship_with(state, &heart_event.npc_id));
        if hearts < i32::try_from(heart_event.hearts).unwrap_or(i32::MAX) {
            continue;
        }
        let scope = ConditionScope { pos: None, speaker: Some(&heart_event.npc_id), salt: &heart_event.id };
        if !events::conditions_hold(ctx, state, &heart_event.conditions, &scope) {
            continue;
        }
        if !ctx.content.cutscenes.iter().any(|cutscene| cutscene.id == heart_event.cutscene_id) {
            continue;
        }
        state.flags.insert(heart_event_seen_flag(&heart_event.id), Value::Bool(true));
        state.cutscene =
            Some(CutsceneState { cutscene_id: heart_event.cutscene_id.clone(), ..CutsceneState::default() });
        return true;
    }
    false
}

/// The partner the player is dating, engaged to or married to.
pub fn partner(state: &GameState) -> Option<&str> {
    state
        .social
        .iter()
        .find(|(_, social)| social.status.as_deref().is_some_and(|status| !status.is_empty()))
        .map(|(id, _)| id.as_str())
}

/// The NPC the player is married to.
pub fn spouse(state: &GameState) -> Option<&str> {
    state
        .social
        .iter()
        .find(|(_, social)| social.status.as_deref() == Some(relationship_statuses::MARRIED))
        .map(|(id, _)| id.as_str())
}

/// Whether `npc_id` is a spouse who lives at the game's home (its schedule waits).
pub fn lives_at_home(ctx: &EngineContext, state: &GameState, npc_id: &str) -> bool {
    settings(ctx).home_scene_id.is_some_and(|scene| !scene.is_empty())
        && social::relationship_status(state, npc_id) == relationship_statuses::MARRIED
}

/// Puts a spouse on the game's home tile (nothing when the game has no home).
pub fn move_spouse_home(ctx: &EngineContext, state: &mut GameState, npc_id: &str) {
    let settings = settings(ctx);
    let Some(scene_id) = settings.home_scene_id.filter(|scene| !scene.is_empty()) else { return };
    if crate::world::world_movement::ensure_scene(ctx, state, &scene_id).is_none() {
        return;
    }
    let npc = state.npcs.entry(npc_id.to_owned()).or_default();
    npc.scene_id = scene_id;
    npc.x = units::tiles(settings.home_x.unwrap_or(0));
    npc.y = units::tiles(settings.home_y.unwrap_or(0));
    npc.path = None;
    npc.patrol_index = None;
}

/// The dating gift or the proposal given to `npc` (M4): its effects, or `None` when `item_id`
/// is an ordinary gift here.
pub fn romantic_gift(ctx: &EngineContext, state: &mut GameState, npc: &Npc, item_id: &str) -> Option<Effects> {
    let settings = settings(ctx);
    let romanceable = npc.romanceable == Some(true);
    let is = |id: &Option<String>| id.as_deref().is_some_and(|id| !id.is_empty() && id == item_id);
    let dating_gift = settings.dating && is(&settings.dating_item_id);
    let proposal = settings.marriage && is(&settings.proposal_item_id);
    if !romanceable || !(dating_gift || proposal) {
        return None;
    }
    let hearts = social::hearts(social::friendship_with(state, &npc.id));
    let status = social::relationship_status(state, &npc.id).to_owned();
    let taken = partner(state).is_some_and(|partner| partner != npc.id);
    let (wanted_status, wanted_hearts) = if proposal {
        (relationship_statuses::DATING, settings.proposal_hearts)
    } else {
        (relationship_statuses::SINGLE, settings.dating_hearts)
    };
    if taken {
        return Some(vec![Effect::say(message_levels::INFO, &messages::ALREADY_TAKEN)]);
    }
    if status != wanted_status || hearts < i32::try_from(wanted_hearts).unwrap_or(i32::MAX) {
        return Some(vec![Effect::say(message_levels::INFO, messages::NOT_READY_TO_DATE.with(&[&npc.name]))]);
    }
    state.player.inventory = inventory::remove_item(&state.player.inventory, item_id, 1);
    let next = if proposal { relationship_statuses::ENGAGED } else { relationship_statuses::DATING };
    let mut effects = social::set_relationship(ctx, state, &npc.id, next);
    effects.push(if proposal {
        Effect::say(message_levels::SUCCESS, messages::NOW_ENGAGED.with(&[&npc.name, &settings.wedding_days.max(1)]))
    } else {
        Effect::say(message_levels::SUCCESS, messages::NOW_DATING.with(&[&npc.name]))
    });
    Some(effects)
}

/// The relationship part of the night (from the overnight pass, once the new day has begun):
/// decay for `previous_day`, dating by hearts, weddings, the spouse's move and chore, children.
pub fn morning(ctx: &EngineContext, state: &mut GameState, previous_day: u32) -> Effects {
    let settings = settings(ctx);
    let mut effects = Vec::new();

    if settings.decay_per_day > 0 {
        let decay = i32::try_from(settings.decay_per_day).unwrap_or(i32::MAX);
        for social in state.social.values_mut() {
            let tended = social.talked_day == Some(previous_day) || social.last_gift_day == Some(previous_day);
            if !tended && social.friendship > 0 {
                social.friendship = social.friendship.saturating_sub(decay).max(0);
            }
        }
    }

    // Dating without a dating gift starts by itself at the hearts.
    let no_dating_gift = settings.dating_item_id.as_deref().is_none_or(str::is_empty);
    if settings.dating && no_dating_gift && partner(state).is_none() {
        let ready = ctx.content.npcs.iter().find(|npc| {
            npc.romanceable == Some(true)
                && social::hearts(social::friendship_with(state, &npc.id))
                    >= i32::try_from(settings.dating_hearts).unwrap_or(i32::MAX)
        });
        if let Some(npc) = ready {
            effects.extend(social::set_relationship(ctx, state, &npc.id, relationship_statuses::DATING));
            effects.push(Effect::say(message_levels::SUCCESS, messages::NOW_DATING.with(&[&npc.name])));
        }
    }

    // The wedding.
    let today = state.clock.day;
    let wedding = state.social.iter().find_map(|(id, social)| {
        (social.status.as_deref() == Some(relationship_statuses::ENGAGED)
            && social.status_day.is_some_and(|day| day <= today))
        .then(|| id.clone())
    });
    if let Some(npc_id) = wedding {
        effects.extend(social::set_relationship(ctx, state, &npc_id, relationship_statuses::MARRIED));
        let name = ctx.npc(&npc_id).map_or(npc_id.as_str(), |npc| npc.name.as_str()).to_owned();
        effects.push(Effect::say(message_levels::SUCCESS, messages::WEDDING_DAY.with(&[&name])));
        let cutscene = settings.wedding_cutscene_id.as_deref().filter(|id| !id.is_empty());
        if let Some(cutscene_id) = cutscene.filter(|id| ctx.content.cutscenes.iter().any(|c| c.id == *id)) {
            if state.cutscene.is_none() {
                state.cutscene =
                    Some(CutsceneState { cutscene_id: cutscene_id.to_owned(), ..CutsceneState::default() });
            }
        }
    }

    let Some(spouse_id) = spouse(state).map(str::to_owned) else { return effects };
    move_spouse_home(ctx, state, &spouse_id);
    let spouse_name = ctx.npc(&spouse_id).map_or(spouse_id.as_str(), |npc| npc.name.as_str()).to_owned();
    if settings.spouse_helps {
        effects.extend(spouse_chore(ctx, state, &settings, &spouse_name));
    }

    if settings.children && settings.child_days > 0 {
        let married_on = state.social.get(&spouse_id).and_then(|social| social.status_day).unwrap_or(today);
        let born = u32::try_from(state.children.len()).unwrap_or(u32::MAX);
        let due = married_on.saturating_add(settings.child_days.saturating_mul(born.saturating_add(1)));
        if born < settings.max_children && today >= due {
            let name = settings
                .child_names
                .get(state.children.len())
                .filter(|name| !name.is_empty())
                .cloned()
                .unwrap_or_else(|| format!("Baby {}", born + 1));
            effects.push(Effect::say(message_levels::SUCCESS, messages::CHILD_BORN.with(&[&name])));
            state.children.push(ChildState { name, parent_id: spouse_id.clone(), born_day: today });
        }
    }
    effects
}

/// One morning chore of the spouse, picked with the game's RNG: water every crop of the farm,
/// feed every animal, or bring a gift. A chore with nothing to do (no crops, no animals, no gift
/// items) is skipped quietly.
fn spouse_chore(ctx: &EngineContext, state: &mut GameState, settings: &RelationshipSettings, name: &str) -> Effects {
    let mut rng = Rng::new(state.rng.clone());
    let chore = rng.int(0, 2);
    state.rng = rng.state.clone();
    match chore {
        0 => {
            let farm = ctx.content.start_scene_id.clone();
            let Some(index) = state.world.scenes.iter().position(|scene| scene.id == farm) else { return vec![] };
            let mut watered = false;
            let (width, height) = (state.world.scenes[index].width, state.world.scenes[index].height);
            for y in 0..height.max(0) {
                for x in 0..width.max(0) {
                    let has_crop = state.world.scenes[index].tile(x, y).is_some_and(|tile| tile.crop.is_some());
                    if has_crop {
                        farming_actions::water_at(ctx, state, index, x as usize, y as usize);
                        watered = true;
                    }
                }
            }
            if watered {
                vec![Effect::say(message_levels::INFO, messages::SPOUSE_WATERED.with(&[&name]))]
            } else {
                vec![]
            }
        }
        1 => {
            if state.animals.is_empty() {
                return vec![];
            }
            for animal in &mut state.animals {
                animal.fed_today = true;
            }
            vec![Effect::say(message_levels::INFO, messages::SPOUSE_FED.with(&[&name]))]
        }
        _ => {
            let gifts: Vec<&str> = settings.spouse_gift_item_ids.iter().map(String::as_str).collect();
            if gifts.is_empty() {
                return vec![];
            }
            let pick = usize::try_from(rng.int(0, gifts.len() as i64 - 1)).unwrap_or(0);
            state.rng = rng.state;
            let Some(item) = gifts.get(pick).and_then(|id| ctx.item(id)) else { return vec![] };
            let result = inventory::add_item(&state.player.inventory, item, 1, state.player.max_inventory_size, None);
            if !result.added {
                return vec![];
            }
            state.player.inventory = result.inventory;
            vec![Effect::say(message_levels::INFO, messages::SPOUSE_GIFT.with(&[&name, &item.name]))]
        }
    }
}
