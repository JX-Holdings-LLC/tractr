//! The engine is a pure reducer over (state, content, command | tick) (port of `Engine.cs` /
//! engine.ts). Same seed + same command/tick log ⇒ same state.

use crate::commands::Command;
use crate::effects::{message_levels, Effect};
use crate::engine_types::{CommandRules, Effects, EngineContext};
use crate::farming::farming_actions;
use crate::game_time::SleepOptions;
use crate::hooks::{CommandHookPayload, HookEvent, SceneEnterHookPayload, TimeOfDayHookPayload};
use crate::messages;
use crate::npcs::npc_movement;
use crate::schema::{event_triggers, EventOutcome, GameState, MoveIntent, OutcomeKind, PluginMutation};
use crate::units;
use crate::world::world_movement;
use crate::{
    buildings, combat, crafting, cutscenes, dialogue_system, economy, energy, events, extensibility, farm_layouts,
    game_time, goals, inventory, mail, mines, objects, skills, social, wardrobe, weather,
};

/// Simulation runs at a fixed rate; rendering interpolates between ticks.
pub const TICKS_PER_SECOND: u32 = units::TICKS_PER_SECOND;

/// Apply one command to the state. Under [`CommandRules::Player`] a command that does not apply
/// now is refused first (see `refusal`); otherwise it emits `onCommand` and runs its handler.
pub fn apply_command(ctx: &EngineContext, state: &mut GameState, command: &Command) -> Effects {
    npc_movement::select_schedules(ctx, state);
    if ctx.rules == CommandRules::Player {
        if let Some(refused) = refusal(ctx, state, command) {
            return refused;
        }
    }
    ctx.emit(HookEvent::Command(CommandHookPayload { command_type: command.type_name().to_owned() }));
    let scene_before = state.player.scene_id.clone();
    let mut effects = run_command(ctx, state, command);
    effects.extend(crate::festivals::advance(ctx, state));
    objects::forget_gone(ctx, state);
    // Items found and achievements reached (M6).
    effects.extend(goals::after_step(ctx, state));
    effects.extend(crate::quests::after_step(ctx, state));
    emit_scene_enter(ctx, state, scene_before);
    effects
}

/// `onSceneEnter` when the step left the player in another scene than it found them in (doors,
/// warps, the mine, fainting, sleeping, outcomes: however they got there, M9).
fn emit_scene_enter(ctx: &EngineContext, state: &GameState, scene_before: String) {
    if state.player.scene_id != scene_before {
        ctx.emit(HookEvent::SceneEnter(SceneEnterHookPayload {
            scene_id: state.player.scene_id.clone(),
            from_scene_id: scene_before,
        }));
    }
}

fn run_command(ctx: &EngineContext, state: &mut GameState, command: &Command) -> Effects {
    match command {
        Command::SetMoveIntent { dx, dy } => {
            // Read with Math.trunc; clamped to −1..1.
            let dx = (*dx).clamp(-1, 1);
            let dy = (*dy).clamp(-1, 1);
            let intent = MoveIntent { dx, dy };
            let direction = world_movement::direction_from_intent(&intent, &state.player.direction);
            state.player.move_intent = intent;
            state.player.direction = direction;
            vec![]
        }
        Command::Move { dir } => world_movement::handle_move(ctx, state, dir),
        Command::UseTool { tool } => farming_actions::handle_use_tool(ctx, state, tool),
        Command::Interact => farming_actions::handle_interact(ctx, state),
        Command::InteractWith { seed_item_id, fertilizer_item_id } => farming_actions::handle_interact_with(
            ctx,
            state,
            farming_actions::PlantChoice {
                seed_item_id: seed_item_id.as_deref(),
                fertilizer_item_id: fertilizer_item_id.as_deref(),
                chosen: true,
            },
        ),
        Command::ChooseDialogueOption { index } => dialogue_system::handle_choose_dialogue_option(ctx, state, *index),
        Command::CloseDialogue => dialogue_system::handle_close_dialogue(state),
        Command::Sleep => game_time::perform_sleep(ctx, state, SleepOptions { collapsed: false }),
        Command::OpenShop { shop_id } => economy::handle_open_shop(ctx, state, shop_id),
        Command::CloseShop => economy::handle_close_shop(state),
        Command::BuyItem { item_id, quantity } => economy::handle_buy_item(ctx, state, item_id, *quantity),
        Command::SellItem { item_id, quantity, quality } => {
            economy::handle_sell_item(ctx, state, item_id, *quantity, quality.as_deref())
        }
        Command::ChoosePerk { skill, perk_id, choice_id } => skills::choose_perk(ctx, state, skill, perk_id, choice_id),
        Command::UpgradeTool { item_id } => crate::progression::order_tool(ctx, state, item_id),
        Command::CollectToolUpgrade { item_id } => crate::progression::collect_tool(ctx, state, item_id),
        Command::RepairTool { item_id } => economy::handle_repair_tool(ctx, state, item_id),
        Command::Craft { recipe_id } => crafting::handle_craft(ctx, state, recipe_id),
        Command::PlaceMachine { machine_type_id } => crafting::handle_place_machine(ctx, state, machine_type_id),
        Command::MachineLoad { recipe_id } => crafting::handle_machine_load(ctx, state, recipe_id),
        Command::PickUpMachine => crafting::handle_pick_up_machine(ctx, state),
        Command::GiveGift { item_id } => social::handle_give_gift(ctx, state, item_id),
        Command::DescendMine { floor } => mines::descend_mine(ctx, state, *floor),
        Command::ExitMine => mines::exit_mine(ctx, state),
        Command::PerformAction { action_id } => events::perform_action(ctx, state, action_id).effects,
        Command::UseItem { item_id } => extensibility::handle_use_item(ctx, state, item_id),
        Command::StartMinigame { minigame_id } => extensibility::handle_start_minigame(ctx, state, minigame_id),
        Command::ResolveMinigame { score } => extensibility::handle_resolve_minigame(ctx, state, *score),
        Command::CancelMinigame => extensibility::handle_cancel_minigame(state),
        Command::AdvanceCutscene => cutscenes::handle_advance(ctx, state),
        Command::ChooseCutsceneOption { index } => cutscenes::handle_choose(ctx, state, *index),
        Command::SkipCutscene => cutscenes::handle_skip(ctx, state),
        Command::WearItem { item_id } => wardrobe::handle_wear(ctx, state, item_id),
        Command::TakeOff { slot } => wardrobe::handle_take_off(state, slot),
        Command::SetPlayerLook { name, farm_name, look } => wardrobe::handle_set_look(state, name, farm_name, look),
        Command::StoreItem { item_id, quantity, quality } => {
            objects::handle_store(ctx, state, item_id, *quantity, quality.as_deref())
        }
        Command::TakeStoredItem { index, quantity } => objects::handle_take(ctx, state, *index, *quantity),
        Command::CloseStorage => objects::handle_close(state),
        Command::OrderBuilding { building_id, scene_id, x, y } => {
            buildings::handle_order(ctx, state, building_id, scene_id, *x, *y)
        }
        Command::UpgradeBuilding { id, building_id } => buildings::handle_upgrade(ctx, state, id, building_id),
        Command::BuyAnimal { species_id, building, name } => {
            buildings::handle_buy_animal(ctx, state, species_id, building, name)
        }
        Command::ChooseFarmLayout { layout_id } => farm_layouts::handle_choose(ctx, state, layout_id),
        Command::DonateItem { collection_id, item_id } => goals::handle_donate(ctx, state, collection_id, item_id),
        Command::HandInBundle { bundle_id, line } => goals::handle_hand_in(ctx, state, bundle_id, *line),
        Command::ReadMail { index } => mail::handle_read(ctx, state, *index),
        Command::PluginMutation { plugin_id, mutation } => apply_plugin_mutation(ctx, state, plugin_id, mutation),
    }
}

/// Why `command` does not apply now under [`CommandRules::Player`] (`None` when it does): the
/// refusal's effects, often none. Nothing else of the command happens, not even its `onCommand`
/// hook.
fn refusal(ctx: &EngineContext, state: &GameState, command: &Command) -> Option<Effects> {
    // While a modal is open only its own commands apply (the player UI sends nothing else); the
    // held intent is stored but movement stays frozen, and plugin mutations are content. A
    // dialogue, shop or minigame an outcome opened over a cutscene comes first.
    let dialogue = state.dialogue.is_some();
    let shop = state.shop.is_some();
    let minigame = state.minigame.is_some();
    let cutscene = state.cutscene.is_some();
    let storage = state.storage.is_some();
    if dialogue || shop || minigame || cutscene || storage {
        let allowed = match command {
            Command::SetMoveIntent { .. } | Command::PluginMutation { .. } => true,
            Command::ChooseDialogueOption { .. } | Command::CloseDialogue => dialogue,
            Command::BuyItem { .. }
            | Command::SellItem { .. }
            | Command::UpgradeTool { .. }
            | Command::CollectToolUpgrade { .. }
            | Command::RepairTool { .. }
            | Command::CloseShop
            | Command::OrderBuilding { .. }
            | Command::UpgradeBuilding { .. }
            | Command::BuyAnimal { .. } => shop,
            Command::StoreItem { .. } | Command::TakeStoredItem { .. } | Command::CloseStorage => storage,
            // The mailbox and the goals list are menus over any open screen but a minigame.
            Command::ReadMail { .. } | Command::DonateItem { .. } | Command::HandInBundle { .. } => {
                !minigame && !cutscene
            }
            Command::ResolveMinigame { .. } | Command::CancelMinigame => minigame,
            Command::AdvanceCutscene | Command::ChooseCutsceneOption { .. } | Command::SkipCutscene => {
                cutscene && !(dialogue || shop || minigame)
            }
            _ => false,
        };
        if !allowed {
            return Some(Vec::new());
        }
    }
    match command {
        Command::DescendMine { floor } => mines::descend_refusal(ctx, state, *floor),
        Command::SetPlayerLook { .. } => wardrobe::set_look_refusal(state),
        Command::ChooseFarmLayout { .. } => farm_layouts::refusal(state),
        Command::OrderBuilding { building_id, .. } => buildings::order_refusal(ctx, state, building_id),
        Command::UpgradeBuilding { building_id, .. } => buildings::order_refusal(ctx, state, building_id),
        Command::BuyAnimal { species_id, .. } => buildings::animal_refusal(ctx, state, species_id),
        Command::ExitMine => mines::exit_refusal(state),
        Command::AdvanceCutscene => cutscenes::advance_refusal(ctx, state),
        Command::ChooseCutsceneOption { index } => cutscenes::choose_refusal(ctx, state, *index),
        Command::SkipCutscene => cutscenes::skip_refusal(ctx, state),
        Command::OpenShop { shop_id } => economy::open_shop_refusal(ctx, state, shop_id),
        // Minigames open from actions, items, tools and plugins, which decide what they are
        // for; opening one on demand would hand out its rewards for any score.
        Command::StartMinigame { .. } => Some(vec![Effect::say(message_levels::INFO, &messages::NOTHING_TO_PLAY)]),
        _ => None,
    }
}

/// A whole plugin amount (gold, a variable's step, health) as an outcome `amount` (thousandths).
fn money_amount(gold: i64) -> i64 {
    gold.saturating_mul(i64::from(units::MILLI_ONE))
}

/// Apply a plugin-declared mutation (M5). Plugins run sandboxed and return these instead of
/// mutating state; each is validated by the schema layer before it becomes a command, and unknown
/// references fail soft here.
fn apply_plugin_mutation(
    ctx: &EngineContext,
    state: &mut GameState,
    plugin_id: &str,
    mutation: &PluginMutation,
) -> Effects {
    let plugin_error = |detail: messages::Message| {
        vec![Effect::say(
            message_levels::ERROR,
            messages::PLUGIN_ERROR.with_args(vec![messages::Arg::text(plugin_id), detail.into()]),
        )]
    };
    // Mutations sharing the event-outcome executor (identical semantics to the equivalent
    // event/action outcome, including soft failure).
    let run_outcome = |state: &mut GameState, outcome: EventOutcome| events::apply_outcomes(ctx, state, &[outcome], 0);

    match mutation {
        PluginMutation::GiveItem { item_id, quantity } => {
            let Some(item) = ctx.item(item_id) else {
                return plugin_error(messages::PLUGIN_UNKNOWN_ITEM.with(&[item_id]));
            };
            let result =
                inventory::add_item(&state.player.inventory, item, *quantity, state.player.max_inventory_size, None);
            if !result.added {
                return vec![Effect::say(message_levels::INFO, &messages::INVENTORY_FULL_SHORT)];
            }
            state.player.inventory = result.inventory;
            vec![Effect::say(message_levels::INFO, messages::RECEIVED_COUNT.with(&[quantity, &item.name]))]
        }
        PluginMutation::SetFlag { flag, value } => {
            // Canonical numbers (`1.0` is `1`) so the flag survives a save/load round trip (#142).
            state.flags.insert(flag.clone(), units::canonical_json(value.clone()));
            vec![]
        }
        PluginMutation::Message { text } => vec![Effect::message(message_levels::INFO, text.clone())],
        PluginMutation::SetWeather { weather_id } => {
            if weather::weather_type_by_id(ctx, weather_id).is_none() {
                return plugin_error(messages::PLUGIN_UNKNOWN_WEATHER.with(&[weather_id]));
            }
            state.clock.weather_id = weather_id.clone();
            vec![]
        }

        PluginMutation::TakeItem { item_id, quantity } => run_outcome(
            state,
            EventOutcome {
                item_id: Some(item_id.clone()),
                item_quantity: Some(*quantity),
                ..EventOutcome::of(OutcomeKind::TakeItem)
            },
        ),
        PluginMutation::GiveMoney { amount } => run_outcome(
            state,
            EventOutcome { amount: Some(money_amount(*amount)), ..EventOutcome::of(OutcomeKind::GiveMoney) },
        ),
        PluginMutation::TakeMoney { amount } => run_outcome(
            state,
            EventOutcome { amount: Some(money_amount(*amount)), ..EventOutcome::of(OutcomeKind::TakeMoney) },
        ),
        PluginMutation::StartQuest { quest_id } => run_outcome(
            state,
            EventOutcome { quest_id: Some(quest_id.clone()), ..EventOutcome::of(OutcomeKind::StartQuest) },
        ),
        PluginMutation::WarpPlayer { scene_id, x, y } => run_outcome(
            state,
            EventOutcome {
                scene_id: Some(scene_id.clone()),
                x: Some(*x),
                y: Some(*y),
                ..EventOutcome::of(OutcomeKind::WarpPlayer)
            },
        ),
        PluginMutation::StartDialogue { npc_id, dialogue_id } => run_outcome(
            state,
            EventOutcome {
                npc_id: Some(npc_id.clone()),
                dialogue_id: dialogue_id.clone(),
                ..EventOutcome::of(OutcomeKind::StartDialogue)
            },
        ),
        PluginMutation::PlaySound { sound_id } => run_outcome(
            state,
            EventOutcome { sound_id: Some(sound_id.clone()), ..EventOutcome::of(OutcomeKind::PlaySound) },
        ),

        PluginMutation::ModifyFriendship { npc_id, delta } => {
            if ctx.npc(npc_id).is_none() {
                return plugin_error(messages::PLUGIN_UNKNOWN_NPC.with(&[npc_id]));
            }
            plugin_modify_friendship(ctx, state, npc_id, *delta)
        }

        PluginMutation::GrantXp { skill, amount } => skills::grant_xp(ctx, state, skill, *amount),

        PluginMutation::ModifyEnergy { delta } => plugin_modify_energy(ctx, state, *delta),

        PluginMutation::PerformAction { action_id } => events::perform_action(ctx, state, action_id).effects,
        PluginMutation::StartMinigame { minigame_id } => extensibility::handle_start_minigame(ctx, state, minigame_id),

        // The M3–M7 outcomes (M9): the outcome executor, so they mean exactly what the outcome
        // means (including failing soft on unknown ids).
        PluginMutation::MoveNpc { npc_id, scene_id, x, y } => run_outcome(
            state,
            EventOutcome {
                npc_id: Some(npc_id.clone()),
                scene_id: scene_id.clone(),
                x: Some(*x),
                y: Some(*y),
                ..EventOutcome::of(OutcomeKind::MoveNpc)
            },
        ),
        PluginMutation::StartCutscene { cutscene_id } => run_outcome(
            state,
            EventOutcome { cutscene_id: Some(cutscene_id.clone()), ..EventOutcome::of(OutcomeKind::StartCutscene) },
        ),
        PluginMutation::SetVariable { variable, value } => {
            let (amount, text) = match value {
                serde_json::Value::String(text) => (None, Some(text.clone())),
                other => {
                    (Some(<units::SignedMilli as units::Unit>::from_authoring(other.as_f64().unwrap_or(0.0))), None)
                }
            };
            run_outcome(
                state,
                EventOutcome {
                    variable: Some(variable.clone()),
                    amount,
                    text,
                    ..EventOutcome::of(OutcomeKind::SetVariable)
                },
            )
        }
        PluginMutation::AddVariable { variable, amount } => run_outcome(
            state,
            EventOutcome {
                variable: Some(variable.clone()),
                amount: Some(money_amount(i64::from(*amount))),
                ..EventOutcome::of(OutcomeKind::AddVariable)
            },
        ),
        PluginMutation::AddShopStock { shop_id, item_id, price, daily_limit } => run_outcome(
            state,
            EventOutcome {
                shop_id: Some(shop_id.clone()),
                item_id: Some(item_id.clone()),
                amount: price.map(money_amount),
                item_quantity: *daily_limit,
                ..EventOutcome::of(OutcomeKind::AddShopStock)
            },
        ),
        PluginMutation::RemoveShopStock { shop_id, item_id } => run_outcome(
            state,
            EventOutcome {
                shop_id: Some(shop_id.clone()),
                item_id: Some(item_id.clone()),
                ..EventOutcome::of(OutcomeKind::RemoveShopStock)
            },
        ),
        PluginMutation::PlaceNode { scene_id, x, y, node_type_id } => run_outcome(
            state,
            EventOutcome {
                node_type_id: Some(node_type_id.clone()),
                ..tile_outcome(OutcomeKind::PlaceNode, scene_id, *x, *y)
            },
        ),
        PluginMutation::RemoveNode { scene_id, x, y } => {
            run_outcome(state, tile_outcome(OutcomeKind::RemoveNode, scene_id, *x, *y))
        }
        PluginMutation::PlaceMachine { scene_id, x, y, machine_type_id } => run_outcome(
            state,
            EventOutcome {
                machine_type_id: Some(machine_type_id.clone()),
                ..tile_outcome(OutcomeKind::PlaceMachine, scene_id, *x, *y)
            },
        ),
        PluginMutation::RemoveMachine { scene_id, x, y } => {
            run_outcome(state, tile_outcome(OutcomeKind::RemoveMachine, scene_id, *x, *y))
        }
        PluginMutation::SetRelationship { npc_id, status } => run_outcome(
            state,
            EventOutcome {
                npc_id: Some(npc_id.clone()),
                status: Some(status.clone()),
                ..EventOutcome::of(OutcomeKind::SetRelationship)
            },
        ),
        PluginMutation::SendMail { mail_id } => run_outcome(
            state,
            EventOutcome { mail_id: Some(mail_id.clone()), ..EventOutcome::of(OutcomeKind::SendMail) },
        ),
        PluginMutation::SetInventorySize { amount } => {
            crate::progression::apply(ctx, state, "setInventorySize", i64::from(*amount));
            vec![]
        }
        PluginMutation::AddInventorySize { amount } => {
            crate::progression::apply(ctx, state, "addInventorySize", i64::from(*amount));
            vec![]
        }
        PluginMutation::AddMaxEnergy { amount } => {
            crate::progression::apply(ctx, state, "addMaxEnergy", i64::from(*amount));
            vec![]
        }
        PluginMutation::AddMaxHealth { amount } => {
            crate::progression::apply(ctx, state, "addMaxHealth", i64::from(*amount));
            vec![]
        }
        PluginMutation::ApplyBuff { buff } => {
            crate::buffs::apply(ctx, state, buff, "plugin");
            vec![]
        }
        PluginMutation::ModifyHealth { delta } => run_outcome(
            state,
            EventOutcome {
                amount: Some(money_amount(i64::from(*delta))),
                ..EventOutcome::of(OutcomeKind::ModifyHealth)
            },
        ),
        PluginMutation::SaveData { data } => {
            state.plugin_data.insert(plugin_id.to_owned(), units::canonical_json(data.clone()));
            vec![]
        }
    }
}

/// An outcome on tile (x, y) of `scene_id` (absent: the player's scene).
fn tile_outcome(kind: OutcomeKind, scene_id: &Option<String>, x: i32, y: i32) -> EventOutcome {
    EventOutcome { scene_id: scene_id.clone(), tile_x: Some(x), tile_y: Some(y), ..EventOutcome::of(kind) }
}

/// `modifyFriendship` from a plugin: whole points, clamped to the friendship range.
fn plugin_modify_friendship(ctx: &EngineContext, state: &mut GameState, npc_id: &str, delta: i32) -> Effects {
    let friendship = social::friendship_with(state, npc_id).saturating_add(delta);
    social::set_friendship(ctx, state, npc_id, friendship)
}

/// `modifyEnergy` from a plugin: a loss spends energy (and can collapse the player), a gain
/// refills up to the maximum. Nothing happens with energy off.
fn plugin_modify_energy(ctx: &EngineContext, state: &mut GameState, delta: i32) -> Effects {
    if !ctx.content.settings.energy_enabled || delta == 0 {
        return vec![];
    }
    if delta < 0 {
        return energy::spend_energy(ctx, state, delta.saturating_neg()).effects;
    }
    state.player.energy = crate::buffs::max_energy(ctx, state).min(state.player.energy.saturating_add(delta));
    vec![]
}

/// Advance simulation time by whole ticks. The game clock accrues in-game minutes; passing the
/// configured day end forces a collapse (the world moves on without you).
pub fn advance_tick(ctx: &EngineContext, state: &mut GameState, ticks: u64) -> Effects {
    if ticks > 0 {
        npc_movement::select_schedules(ctx, state);
    }
    // Ticks are processed one at a time so that advanceTick(N) is exactly equivalent to
    // N × advanceTick(1). The batched fast-path used to evaluate minute-boundary systems once per
    // *call*, which made live simulation frame-rate dependent (a slow frame delivering 3 ticks
    // skipped minute boundaries a fast machine would have hit).
    let mut effects = Vec::new();
    for _ in 0..ticks {
        effects.extend(advance_single_tick(ctx, state));
    }
    effects
}

/// Is a dialogue, shop, minigame or cutscene open? The player's body stays put while one is, and
/// with `time.pauseInModals` the clock does too.
pub fn modal_open(state: &GameState) -> bool {
    state.dialogue.is_some()
        || state.shop.is_some()
        || state.minigame.is_some()
        || state.cutscene.is_some()
        || state.storage.is_some()
}

fn advance_single_tick(ctx: &EngineContext, state: &mut GameState) -> Effects {
    let scene_before = state.player.scene_id.clone();
    let had_buffs = !state.player.buffs.is_empty();
    let effects = advance_single_tick_inner(ctx, state);
    if had_buffs || !state.player.buffs.is_empty() {
        let now = crate::buffs::now(ctx, state);
        state.player.buffs.retain(|buff| buff.expires_at_minute > now);
        state.player.energy = state.player.energy.min(crate::buffs::max_energy(ctx, state));
        if let Some(max) = crate::combat::player_max_health(ctx, state) {
            state.player.health = state.player.health.map(|health| health.min(max));
        }
    }
    emit_scene_enter(ctx, state, scene_before);
    effects
}

fn advance_single_tick_inner(ctx: &EngineContext, state: &mut GameState) -> Effects {
    // An invalid day window would collapse the player every tick (#24).
    let time = game_time::time_config(&ctx.content.settings.time);
    state.clock.tick = state.clock.tick.saturating_add(1);
    // A cutscene plays on whatever the clock does (M2); nothing happens without one.
    let mut effects = cutscenes::advance(ctx, state);
    let modal = modal_open(state);
    // With `pauseInModals` an open dialogue, shop, minigame or cutscene stops the clock: no
    // minutes pass, so no minute-boundary systems run and the day cannot end mid-conversation
    // (#37).
    if modal && time.pauses_in_modals() {
        return effects;
    }
    // The time rate is stored in micro-minutes per tick.
    let before_minute = units::whole_minute(state.clock.time_minutes);
    state.clock.time_minutes = state.clock.time_minutes.saturating_add(time.minutes_per_real_second);

    // The first tick of a game brings its scenes' monsters (M7; later mornings bring new ones).
    if state.clock.tick == 1 {
        combat::spawn_scenes(ctx, state);
        crate::foraging::morning(ctx, state);
    }
    // Free movement integrates every tick from the held intent. Frozen while a dialogue, shop or
    // minigame is open (modal interactions pause the body), and so are the monsters.
    if !modal {
        effects.extend(world_movement::integrate_movement(ctx, state));
        effects.extend(combat::advance_monsters(ctx, state));
    }

    // Minute boundary: NPC movement/schedules step and tick-events evaluate once per whole in-game
    // minute (bounds evaluation cost).
    let after_minute = units::whole_minute(state.clock.time_minutes);
    if after_minute > before_minute {
        advance_npcs_per_minute(ctx, state, before_minute, after_minute);
        // `onTimeOfDay` for each full hour the clock just reached (M9).
        for hour in (before_minute / 60 + 1)..=(after_minute / 60) {
            ctx.emit(HookEvent::TimeOfDay(TimeOfDayHookPayload { hour, day: state.clock.day }));
        }
        crafting::settle_machines(ctx, state);
        effects.extend(events::evaluate_events(ctx, state, "tick", None));
        // `time` events whose minute the clock just reached (M3).
        let span = (before_minute.saturating_mul(units::MINUTE), after_minute.saturating_mul(units::MINUTE));
        effects.extend(events::fire_trigger(
            ctx,
            state,
            &events::Trigger { kind: event_triggers::TIME, span: Some(span), ..events::Trigger::default() },
        ));
    }

    if u64::from(state.clock.time_minutes) >= u64::from(time.day_end_minute) * u64::from(units::MINUTE) {
        effects.extend(game_time::perform_sleep(ctx, state, SleepOptions { collapsed: true }));
    }
    effects.extend(crate::festivals::advance(ctx, state));
    objects::forget_gone(ctx, state);
    // Items found and achievements reached this tick (M6).
    effects.extend(goals::after_step(ctx, state));
    effects.extend(crate::quests::after_step(ctx, state));
    effects
}

/// NPCs take one step per whole minute that passed, each seeing the clock at that minute. At
/// more than one game minute per tick (`minutesPerRealSecond` above 20) one call per tick fell
/// behind and skipped the wander minutes (#37). The last step sees the clock as it is, so at
/// most one minute per tick (every shipped rate) nothing changes.
fn advance_npcs_per_minute(ctx: &EngineContext, state: &mut GameState, before_minute: u32, after_minute: u32) {
    let now = state.clock.time_minutes;
    for minute in before_minute + 1..after_minute {
        state.clock.time_minutes = minute.saturating_mul(units::MINUTE);
        npc_movement::advance_npcs(ctx, state, 1);
    }
    state.clock.time_minutes = now;
    npc_movement::advance_npcs(ctx, state, 1);
}
