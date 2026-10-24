//! Mail (M6): letters arrive in the morning, from a `sendMail` outcome (events, actions,
//! dialogue, cutscenes), a completed quest that sends one, or by themselves on a morning their
//! conditions hold (an NPC at some hearts, a birthday). Reading a letter runs its outcomes once
//! and hands over what it holds.

use crate::effects::{message_levels, Effect};
use crate::engine_types::{Effects, EngineContext};
use crate::events::{self, ConditionScope};
use crate::messages;
use crate::schema::{GameState, MailDefinition, MailState};
use crate::{inventory, substitution};

/// The letter `id`.
pub fn letter<'a>(ctx: &'a EngineContext, id: &str) -> Option<&'a MailDefinition> {
    ctx.content.mail.iter().find(|letter| letter.id == id)
}

/// Sends letter `mail_id`: it arrives tomorrow morning (sent twice before then, it comes once).
pub fn send(ctx: &EngineContext, state: &mut GameState, mail_id: &str) {
    if letter(ctx, mail_id).is_some() && !state.mail_queue.iter().any(|id| id == mail_id) {
        state.mail_queue.push(mail_id.to_owned());
    }
}

/// Whether letter `mail_id` has arrived before.
fn received(state: &GameState, mail_id: &str) -> bool {
    state.mailbox.iter().any(|entry| entry.mail_id == mail_id)
}

/// The morning's post: the letters sent yesterday, then each letter whose conditions hold this
/// morning (once ever, or with `repeat` once a morning).
pub fn deliver_morning(ctx: &EngineContext, state: &mut GameState) -> Effects {
    if ctx.content.mail.is_empty() {
        state.mail_queue.clear();
        return Vec::new();
    }
    let day = state.clock.day;
    let mut arrived = 0_u32;
    for mail_id in std::mem::take(&mut state.mail_queue) {
        let Some(definition) = letter(ctx, &mail_id) else { continue };
        if definition.repeat || !received(state, &mail_id) {
            state.mailbox.push(MailState { mail_id, day, read: false, taken: false });
            arrived += 1;
        }
    }
    for definition in &ctx.content.mail {
        if definition.conditions.is_empty() {
            continue;
        }
        let today = state.mailbox.iter().any(|entry| entry.mail_id == definition.id && entry.day == day);
        if today || (!definition.repeat && received(state, &definition.id)) {
            continue;
        }
        let scope = ConditionScope { salt: &definition.id, ..ConditionScope::default() };
        if events::conditions_hold(ctx, state, &definition.conditions, &scope) {
            state.mailbox.push(MailState { mail_id: definition.id.clone(), day, read: false, taken: false });
            arrived += 1;
        }
    }
    if arrived == 0 {
        return Vec::new();
    }
    vec![Effect::say(message_levels::INFO, messages::YOU_GOT_MAIL.with(&[&arrived]))]
}

/// Letters not read yet.
pub fn unread(state: &GameState) -> usize {
    state.mailbox.iter().filter(|entry| !entry.read).count()
}

/// The letter's text with its placeholders filled in.
pub fn text(ctx: &EngineContext, state: &GameState, definition: &MailDefinition) -> String {
    substitution::fill(&ctx.content, state, &definition.text).into_owned()
}

/// Read letter `index`: the first time its outcomes run; then what it holds is taken, all of it
/// or (no room) none of it until it is read again.
pub fn handle_read(ctx: &EngineContext, state: &mut GameState, index: i32) -> Effects {
    let Some(entry) = usize::try_from(index).ok().and_then(|index| state.mailbox.get(index)).cloned() else {
        return Vec::new();
    };
    let index = index as usize;
    let Some(definition) = letter(ctx, &entry.mail_id) else {
        state.mailbox[index].read = true;
        state.mailbox[index].taken = true;
        return Vec::new();
    };
    let mut effects = Vec::new();
    if !entry.read {
        state.mailbox[index].read = true;
        effects.extend(events::apply_outcomes(ctx, state, &definition.outcomes, 0));
    }
    if entry.taken {
        return effects;
    }
    let mut next = state.player.inventory.clone();
    let mut received = Vec::new();
    for attachment in &definition.attachments {
        let Some(item) = ctx.item(&attachment.item_id) else { continue };
        let added = inventory::add_item(&next, item, attachment.quantity, state.player.max_inventory_size, None);
        if !added.added {
            effects.push(Effect::say(message_levels::ERROR, &messages::INVENTORY_FULL));
            return effects;
        }
        next = added.inventory;
        received.push(Effect::say(
            message_levels::SUCCESS,
            messages::RECEIVED_COUNT.with(&[&attachment.quantity, &item.name]),
        ));
    }
    state.player.inventory = next;
    if definition.money > 0 {
        state.player.money = state.player.money.saturating_add(definition.money);
        received.push(Effect::say(message_levels::SUCCESS, messages::RECEIVED_MONEY.with(&[&definition.money])));
    }
    if let Some(entry) = state.mailbox.get_mut(index) {
        entry.taken = true;
    }
    effects.extend(received);
    effects
}
