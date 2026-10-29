//! Skills (port of `Skills.cs` / skills.ts).
//!
//! Player skills (M4g): XP per action category; levels unlock recipes and grant small passive
//! modifiers. Toggleable per project.

use crate::effects::{message_levels, Effect};
use crate::engine_types::{Effects, EngineContext};
use crate::messages;
use crate::schema::{GameState, SkillState};

/// The level `xp` reaches on a level curve (the last threshold it meets; 0 below the first).
pub fn skill_level_for_xp(curve: &[u32], xp: u32) -> u32 {
    let mut level = 0;
    for (i, threshold) in curve.iter().enumerate() {
        if xp >= *threshold {
            level = u32::try_from(i).unwrap_or(u32::MAX);
        }
    }
    level
}

/// Grant `xp` in `skill`, announcing a level-up. Nothing happens with skills off.
pub fn grant_xp(ctx: &EngineContext, state: &mut GameState, skill: &str, xp: u32) -> Effects {
    if !ctx.content.settings.skills_enabled || xp == 0 {
        return Vec::new();
    }

    let current = state.player.skills.get(skill).cloned().unwrap_or(SkillState { xp: 0, level: 0 });
    let new_xp = current.xp.saturating_add(xp);
    let new_level = skill_level_for_xp(&ctx.content.settings.skill_level_curve, new_xp);
    let mut effects = Vec::new();
    if new_level > current.level {
        // TS `skill.charAt(0).toUpperCase() + skill.slice(1)`, in English.
        let label = ctx
            .content
            .settings
            .skill_definitions()
            .iter()
            .find(|definition| definition.id == skill)
            .map_or_else(|| messages::skill_noun(skill), |definition| messages::Arg::text(&definition.name));
        effects.push(Effect::say(
            message_levels::SUCCESS,
            messages::LEVEL_UP.with_args(vec![label, messages::Arg::text(new_level)]),
        ));
    }

    // `dict[skill] = …`: an existing key keeps its position, a new one is appended.
    state.player.skills.insert(skill.to_owned(), SkillState { xp: new_xp, level: new_level });
    // Write the level before dispatching rewards, so a reward granting XP cannot repeat it.
    if new_level > current.level {
        let outcomes: Vec<_> = ctx
            .content
            .settings
            .skill_definitions()
            .iter()
            .filter(|definition| definition.id == skill)
            .flat_map(|definition| definition.rewards.iter())
            .filter(|reward| reward.level > current.level && reward.level <= new_level)
            .flat_map(|reward| reward.outcomes.iter().flatten().cloned())
            .collect();
        effects.extend(run_rewards(ctx, state, &outcomes));
    }
    effects
}

/// The player's level in `skill` (0 untrained).
pub fn skill_level(state: &GameState, skill: &str) -> u32 {
    state.player.skills.get(skill).map_or(0, |s| s.level)
}

/// Base skill level with the active temporary skill bonus, clamped to zero.
pub fn effective_level(ctx: &EngineContext, state: &GameState, skill: &str) -> u32 {
    let base = i64::from(skill_level(state, skill));
    u32::try_from(base.saturating_add(i64::from(stat_bonus(ctx, state, "skillLevel"))).clamp(0, i64::from(u32::MAX)))
        .unwrap_or(u32::MAX)
}

/// Farming levels add bonus yield: +1 per 4 levels.
pub fn farming_yield_bonus(state: &GameState) -> u32 {
    skill_level(state, "farming") / 4
}

/// Train every authored skill listening to an action. Empty definitions retain legacy XP.
pub fn grant_action_xp(
    ctx: &EngineContext,
    state: &mut GameState,
    action: &str,
    legacy_skill: &str,
    legacy_xp: u32,
) -> Effects {
    grant_action_xp_subject(ctx, state, action, "", legacy_skill, legacy_xp)
}

/// As `grant_action_xp`, restricted to training rows naming this action's subject.
pub fn grant_action_xp_subject(
    ctx: &EngineContext,
    state: &mut GameState,
    action: &str,
    subject: &str,
    legacy_skill: &str,
    legacy_xp: u32,
) -> Effects {
    if ctx.content.settings.skill_definitions().is_empty() {
        return grant_xp(ctx, state, legacy_skill, legacy_xp);
    }
    let awards: Vec<_> = ctx
        .content
        .settings
        .skill_definitions()
        .iter()
        .flat_map(|definition| {
            definition
                .training
                .iter()
                .filter(|entry| {
                    entry.action == action && entry.subject.as_deref().is_none_or(|id| id.is_empty() || id == subject)
                })
                .map(|entry| (definition.id.clone(), entry.experience))
        })
        .collect();
    let mut effects = Vec::new();
    for (skill, xp) in awards {
        effects.extend(grant_xp(ctx, state, &skill, xp));
    }
    effects
}

/// Sum unlocked stat rewards; values are integer authoring units (percent for escape/cost).
pub fn stat_bonus(ctx: &EngineContext, state: &GameState, stat: &str) -> i32 {
    stat_bonus_for(&ctx.content, state, stat)
}

/// Read-only presentation accessor with the same rules as the engine.
pub fn stat_bonus_for(content: &crate::schema::GameContent, state: &GameState, stat: &str) -> i32 {
    let now = crate::crafting::absolute_minute_in(content.settings.time.minutes_per_day(), state);
    let buffs = crate::buffs::stat_bonus_at(state, stat, now);
    if !content.settings.skills_enabled {
        return buffs;
    }
    content
        .settings
        .skill_definitions()
        .iter()
        .flat_map(|definition| {
            let level = skill_level(state, &definition.id);
            definition.rewards.iter().filter(move |reward| reward.level <= level && reward.stat == stat)
        })
        .fold(buffs.saturating_add(perk_bonus(content, state, stat)), |sum, reward| sum.saturating_add(reward.amount))
}

pub fn perk_key(skill: &str, perk: &str) -> String {
    format!("{}:{skill}{perk}", skill.len())
}

/// Choose an unlocked profession exactly once. Save before running outcomes to prevent repeats.
pub fn choose_perk(ctx: &EngineContext, state: &mut GameState, skill: &str, perk_id: &str, choice_id: &str) -> Effects {
    if !ctx.content.settings.skills_enabled {
        return vec![];
    }
    let Some(definition) = ctx.content.settings.skill_definitions().iter().find(|definition| definition.id == skill)
    else {
        return vec![];
    };
    let Some(perk) = definition.perks.iter().flatten().find(|perk| perk.id == perk_id) else { return vec![] };
    let key = perk_key(skill, perk_id);
    if skill_level(state, skill) < perk.level || state.player.perks.contains_key(&key) {
        return vec![];
    }
    let Some(choice) = perk.choices.iter().find(|choice| choice.id == choice_id) else { return vec![] };
    state.player.perks.insert(key, choice.id.clone());
    run_rewards(ctx, state, choice.outcomes.as_deref().unwrap_or(&[]))
}

fn perk_bonus(content: &crate::schema::GameContent, state: &GameState, stat: &str) -> i32 {
    content
        .settings
        .skill_definitions()
        .iter()
        .flat_map(|definition| {
            definition.perks.iter().flatten().filter_map(move |perk| {
                let selected = state.player.perks.get(&perk_key(&definition.id, &perk.id))?;
                perk.choices.iter().find(|choice| choice.id == *selected && choice.stat == stat)
            })
        })
        .fold(0_i32, |sum, choice| sum.saturating_add(choice.amount))
}

/// Rewards may grant XP or invoke actions that grant XP. Bound the whole nested reward chain.
fn run_rewards(ctx: &EngineContext, state: &mut GameState, outcomes: &[crate::schema::EventOutcome]) -> Effects {
    const MAX_DEPTH: u32 = 8;
    const MAX_RUNS: u32 = 256;
    if outcomes.is_empty() {
        return vec![];
    }
    let depth = ctx.skill_reward_depth.get();
    if depth == 0 {
        ctx.skill_reward_runs.set(0);
    }
    let runs = ctx.skill_reward_runs.get();
    if depth >= MAX_DEPTH || runs >= MAX_RUNS {
        return vec![Effect::say(
            message_levels::ERROR,
            messages::ACTION_CHAIN_LIMIT.with(&[&"skill rewards", &MAX_DEPTH, &MAX_RUNS]),
        )];
    }
    struct Guard<'a> {
        depth: &'a std::cell::Cell<u32>,
        previous: u32,
    }
    impl Drop for Guard<'_> {
        fn drop(&mut self) {
            self.depth.set(self.previous);
        }
    }
    ctx.skill_reward_depth.set(depth + 1);
    let _guard = Guard { depth: &ctx.skill_reward_depth, previous: depth };
    let mut effects = Vec::new();
    for outcome in outcomes {
        let runs = ctx.skill_reward_runs.get();
        if runs >= MAX_RUNS {
            break;
        }
        ctx.skill_reward_runs.set(runs + 1);
        effects.extend(crate::events::apply_outcomes(ctx, state, std::slice::from_ref(outcome), depth));
    }
    effects
}
