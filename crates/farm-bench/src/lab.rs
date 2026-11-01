//! The balancing lab (#230): simple scripted strategies played on the real engine, over many
//! seeds, for a season or a year, with what they earn per day and per energy point, per crop and
//! per recipe chain, when they reach each tool upgrade, building and quest, and the findings a
//! creator should look at.
//!
//! A strategy farms a plot of `PLOT × PLOT` tiles in a field scene of its own, added to the game's
//! world: every morning it harvests what is ripe, tills, buys seeds and plants empty tiles, and
//! waters, through the engine's own commands (`useTool`, `interactWith`), so tool energy, growth,
//! weather, quality, yields and regrowth are the game's. It stops for the day when energy runs
//! low, sells the day's harvest at the best buying shop's price (`economy::sell_unit_price`),
//! and sleeps through the engine's overnight pass. Seeds cost what the shop that stocks them
//! asks (`commerce::price`).
//!
//! Strategies:
//! - `crop:<id>`: only that crop, while it grows in the season;
//! - `best-crop`: each season, the crop that earned most gold per day in a one-crop trial of that
//!   season with the same seed;
//! - `process`: `best-crop`, with every harvest a machine recipe turns into something worth more
//!   processed (`MACHINES` machines, each busy for the recipe's processing minutes);
//! - `raw`: `best-crop`, everything shipped raw.
//!
//! Recipe chains are compared on their own (raw input value against output value, machine
//! time), whatever the strategies grow. Everything is integer and seeded: the same cartridge and
//! seed set give the same numbers.

use farm_cart::LoadedCartridge;
use farm_sim::commands::Command;
use farm_sim::farming::crops;
use farm_sim::schema::{tool_types, CropDefinition, GameState, InventorySlot, Item, Scene, ShopDefinition, Tile};
use farm_sim::{commerce, economy, game_time, inventory, state, units, EngineContext};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// The plot is `PLOT × PLOT` tiles.
pub const PLOT: i32 = 8;
/// Machines the `process` strategy owns.
pub const MACHINES: u32 = 4;
const FIELD: &str = "lab-field";

/// What a lab run plays.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Strategy {
    Crop(String),
    BestCrop,
    Process,
    Raw,
}

impl Strategy {
    pub fn parse(text: &str) -> Result<Self, String> {
        match text {
            "best-crop" => Ok(Self::BestCrop),
            "process" => Ok(Self::Process),
            "raw" => Ok(Self::Raw),
            _ => match text.strip_prefix("crop:") {
                Some(id) if !id.is_empty() => Ok(Self::Crop(id.to_owned())),
                _ => Err(format!("unknown strategy {text} (crop:<id>, best-crop, process or raw)")),
            },
        }
    }

    pub fn name(&self) -> String {
        match self {
            Self::Crop(id) => format!("crop:{id}"),
            Self::BestCrop => "best-crop".to_owned(),
            Self::Process => "process".to_owned(),
            Self::Raw => "raw".to_owned(),
        }
    }
}

/// One day of a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayRow {
    pub day: u32,
    pub season: String,
    pub money: i64,
    /// Gold from today's sales.
    pub income: i64,
    /// Gold spent on seeds today.
    pub spent: i64,
    /// Energy used today, in thousandths of a point.
    pub energy: i64,
}

/// What a crop did in a run.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CropRow {
    pub planted: u32,
    pub harvested: u32,
    pub income: i64,
    pub seed_cost: i64,
    /// Thousandths of an energy point.
    pub energy: i64,
}

/// A whole run: one strategy, one seed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub strategy: String,
    pub seed: String,
    pub days: Vec<DayRow>,
    pub crops: BTreeMap<String, CropRow>,
    /// Day each milestone was first reached ("tool:<item>", "building:<id>", "quest:<id>").
    pub milestones: BTreeMap<String, u32>,
}

impl Run {
    pub fn profit(&self) -> i64 {
        self.days.iter().map(|d| d.income - d.spent).sum()
    }

    pub fn energy(&self) -> i64 {
        self.days.iter().map(|d| d.energy).sum()
    }
}

/// A recipe chain, valued at sale prices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeRow {
    pub recipe: String,
    pub machine: Option<String>,
    pub input_value: i64,
    pub output_value: i64,
    pub minutes: u32,
}

impl RecipeRow {
    pub fn gain(&self) -> i64 {
        self.output_value - self.input_value
    }

    /// Gain for a machine busy a whole day (1,440 minutes) on it.
    pub fn gain_per_machine_day(&self) -> i64 {
        let runs = i64::from(units::MINUTES_PER_DAY / self.minutes.max(1)).max(1);
        self.gain() * runs
    }
}

/// Everything a lab session found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lab {
    pub content_hash: String,
    pub runs: Vec<Run>,
    pub recipes: Vec<RecipeRow>,
    /// Milestones (`tool:`, `building:`) with their gold cost.
    pub costs: BTreeMap<String, i64>,
    pub flags: Vec<String>,
}

/// The lab's game: the cartridge's content and a fresh state per run.
struct Game<'a> {
    ctx: EngineContext,
    cart: &'a LoadedCartridge,
    /// The shop that pays most for sales (`None`: nothing buys).
    buyer: Option<ShopDefinition>,
}

fn seed_item<'a>(game: &'a Game, crop: &str) -> Option<&'a Item> {
    game.ctx.content.items.iter().find(|item| item.r#type == "seed" && item.crop_type.as_deref() == Some(crop))
}

/// The shop that stocks `item_id` in `season`, and its price.
fn seed_price(game: &Game, state: &GameState, item_id: &str) -> Option<i64> {
    let item = game.ctx.item(item_id)?;
    game.ctx.content.shops.iter().find_map(|shop| {
        let entry = shop.stock.iter().find(|entry| {
            entry.item_id == item_id
                && entry.seasons.as_ref().is_none_or(|s| s.is_empty() || s.contains(&state.clock.season))
        })?;
        Some(commerce::price(&game.ctx, state, shop, item_id, entry.price.unwrap_or(item.value), false))
    })
}

fn sale_value(game: &Game, item: &Item, quality: Option<&str>) -> i64 {
    match &game.buyer {
        Some(shop) => economy::sell_unit_price_with_quality(&game.ctx.content, item, quality, shop),
        None => 0,
    }
}

fn field_scene() -> Scene {
    let size = PLOT + 2;
    let tiles = (0..size)
        .map(|y| {
            (0..size)
                .map(|x| Tile { x, y, r#type: "grass".into(), background: "grass".into(), ..Tile::default() })
                .collect()
        })
        .collect();
    Scene { id: FIELD.into(), name: "Lab field".into(), width: size, height: size, tiles, ..Scene::default() }
}

fn field_tile(state: &GameState, x: i32, y: i32) -> Option<&Tile> {
    state.world.scenes.iter().find(|s| s.id == FIELD)?.tile(x, y)
}

fn field_tile_mut(state: &mut GameState, x: i32, y: i32) -> Option<&mut Tile> {
    state.world.scenes.iter_mut().find(|s| s.id == FIELD)?.tile_mut(x, y)
}

/// A new game standing in the lab field, with the game's hoe and watering can.
fn new_game(game: &Game, seed: &str) -> GameState {
    let mut state = state::create_game_state_from_start(&game.cart.start, Some(seed));
    farm_sim::quests::auto_start_quests(&game.ctx, &mut state);
    state.world.scenes.push(field_scene());
    for tool in [tool_types::HOE, tool_types::WATERING_CAN] {
        if inventory::find_tool_slot(&state.player.inventory, tool).is_none() {
            if let Some(item) = game.ctx.content.items.iter().find(|item| item.tool_type.as_deref() == Some(tool)) {
                state.player.inventory.push(InventorySlot::new(item.clone(), 1));
            }
        }
    }
    state.player.scene_id = FIELD.into();
    state
}

fn days_left_in_season(game: &Game, state: &GameState) -> u32 {
    let date = game_time::clock_date(&game.ctx.content.settings.calendar, &state.clock);
    date.season.days.saturating_sub(date.day_of_season) + 1
}

fn growable<'a>(game: &'a Game, state: &GameState, crop: &str) -> Option<&'a CropDefinition> {
    let definition = game.ctx.content.crops.get(crop)?;
    (crops::can_grow_in_season(Some(definition), &state.clock.season)
        && crops::crop_growth_days(definition) < days_left_in_season(game, state))
    .then_some(definition)
}

/// Faces the plot tile (x, y) from the tile above it and runs `command`; returns the energy used.
fn act(game: &Game, state: &mut GameState, x: i32, y: i32, command: &Command) -> i64 {
    state.player.scene_id = FIELD.into();
    (state.player.x, state.player.y) = (units::tile_center(x), units::tile_center(y - 1));
    state.player.direction = "down".into();
    let before = state.player.energy;
    farm_sim::apply_command(&game.ctx, state, command);
    i64::from(before - state.player.energy).max(0)
}

/// Runs `strategy` for `days` days with `seed`; `plan` names the crop of each season for the
/// best-crop strategies.
fn play(game: &Game, strategy: &Strategy, seed: &str, days: u32, plan: &BTreeMap<String, String>) -> Run {
    let mut state = new_game(game, seed);
    let mut run = Run {
        strategy: strategy.name(),
        seed: seed.to_owned(),
        days: Vec::new(),
        crops: BTreeMap::new(),
        milestones: BTreeMap::new(),
    };
    let low_energy = state.player.max_energy / 10;
    for _ in 0..days {
        let mut day = DayRow {
            day: state.clock.day,
            season: state.clock.season.clone(),
            money: 0,
            income: 0,
            spent: 0,
            energy: 0,
        };
        let crop_today = match strategy {
            Strategy::Crop(id) => Some(id.clone()),
            _ => plan.get(&state.clock.season).cloned(),
        };
        let mut harvest: Vec<(String, InventorySlot)> = Vec::new();
        for y in 1..=PLOT {
            for x in 1..=PLOT {
                let Some(tile) = field_tile(&state, x, y) else { continue };
                // A withered crop is cleared (the scythe's job; not counted).
                if tile.crop.as_ref().is_some_and(|crop| crop.withered == Some(true)) {
                    if let Some(tile) = field_tile_mut(&mut state, x, y) {
                        tile.crop = None;
                    }
                }
                let crop = field_tile(&state, x, y).and_then(|tile| tile.crop.clone());
                if let Some(crop) = &crop {
                    let ripe = game
                        .ctx
                        .content
                        .crops
                        .get(&crop.r#type)
                        .is_some_and(|definition| crops::is_crop_mature_by_days(crop, definition));
                    if ripe {
                        let before = state.player.inventory.clone();
                        let energy = act(game, &mut state, x, y, &Command::Interact);
                        let gained = gained(&before, &state.player.inventory);
                        let row = run.crops.entry(crop.r#type.clone()).or_default();
                        row.energy += energy;
                        day.energy += energy;
                        for slot in gained {
                            row.harvested += slot.quantity;
                            harvest.push((crop.r#type.clone(), slot));
                        }
                    }
                }
                let tile = field_tile(&state, x, y).cloned().unwrap_or_default();
                if tile.crop.is_none() {
                    let Some(crop_id) = crop_today.as_deref() else { continue };
                    if growable(game, &state, crop_id).is_none() || state.player.energy <= low_energy {
                        continue;
                    }
                    let Some(seed) = seed_item(game, crop_id).cloned() else { continue };
                    let held = state.player.inventory.iter().any(|slot| slot.item.id == seed.id);
                    if !held {
                        let Some(price) = seed_price(game, &state, &seed.id) else { continue };
                        if state.player.money < price {
                            continue;
                        }
                        let added = inventory::add_item(
                            &state.player.inventory,
                            &seed,
                            1,
                            state.player.max_inventory_size,
                            None,
                        );
                        if !added.added {
                            continue;
                        }
                        state.player.inventory = added.inventory;
                        state.player.money -= price;
                        day.spent += price;
                        run.crops.entry(crop_id.to_owned()).or_default().seed_cost += price;
                    }
                    let mut energy = 0;
                    if !game.ctx.plantable(&tile.background) {
                        energy += act(game, &mut state, x, y, &Command::UseTool { tool: tool_types::HOE.into() });
                    }
                    energy += act(
                        game,
                        &mut state,
                        x,
                        y,
                        &Command::InteractWith { seed_item_id: Some(seed.id.clone()), fertilizer_item_id: None },
                    );
                    let row = run.crops.entry(crop_id.to_owned()).or_default();
                    if field_tile(&state, x, y).is_some_and(|tile| tile.crop.is_some()) {
                        row.planted += 1;
                    }
                    row.energy += energy;
                    day.energy += energy;
                }
                let crop = field_tile(&state, x, y).and_then(|tile| tile.crop.clone());
                if let Some(crop) = crop.filter(|crop| !crop.watered) {
                    if state.player.energy > low_energy {
                        let energy =
                            act(game, &mut state, x, y, &Command::UseTool { tool: tool_types::WATERING_CAN.into() });
                        run.crops.entry(crop.r#type.clone()).or_default().energy += energy;
                        day.energy += energy;
                    }
                }
            }
        }
        // Sell the day's harvest, raw or processed.
        let mut machine_minutes = MACHINES * units::MINUTES_PER_DAY;
        for (crop, slot) in harvest {
            let mut income = sale_value(game, &slot.item, slot.quality.as_deref()) * i64::from(slot.quantity);
            if *strategy == Strategy::Process {
                if let Some((recipe, output)) = best_chain(game, &slot.item.id) {
                    let runs = slot.quantity / recipe_input(&recipe, &slot.item.id).max(1);
                    let fit = runs.min(machine_minutes / recipe.processing_minutes.max(1));
                    if fit > 0
                        && output > sale_value(game, &slot.item, None) * i64::from(recipe_input(&recipe, &slot.item.id))
                    {
                        machine_minutes -= fit * recipe.processing_minutes.max(1);
                        let used = fit * recipe_input(&recipe, &slot.item.id);
                        let left = slot.quantity - used;
                        income = output * i64::from(fit)
                            + sale_value(game, &slot.item, slot.quality.as_deref()) * i64::from(left);
                    }
                }
            }
            state.player.inventory = inventory::remove_item_with_quality(
                &state.player.inventory,
                &slot.item.id,
                slot.quality.as_deref(),
                slot.quantity,
            );
            state.player.money += income;
            day.income += income;
            run.crops.entry(crop).or_default().income += income;
        }
        day.money = state.player.money;
        record_milestones(game, &state, &mut run);
        run.days.push(day);
        game_time::perform_sleep(&game.ctx, &mut state, game_time::SleepOptions { collapsed: false });
    }
    run
}

fn recipe_input(recipe: &farm_sim::schema::RecipeDefinition, item_id: &str) -> u32 {
    recipe.inputs.iter().filter(|i| i.item_id == item_id).map(|i| i.quantity).sum()
}

/// The machine recipe that takes only `item_id` and pays most, with its output's value.
fn best_chain(game: &Game, item_id: &str) -> Option<(farm_sim::schema::RecipeDefinition, i64)> {
    game.ctx
        .content
        .recipes
        .iter()
        .filter(|r| {
            r.machine_type_id.is_some() && !r.inputs.is_empty() && r.inputs.iter().all(|i| i.item_id == item_id)
        })
        .map(|r| (r.clone(), output_value(game, r)))
        .fold(None, |best: Option<(farm_sim::schema::RecipeDefinition, i64)>, next| match &best {
            Some((_, value)) if *value >= next.1 => best,
            _ => Some(next),
        })
}

fn output_value(game: &Game, recipe: &farm_sim::schema::RecipeDefinition) -> i64 {
    recipe
        .outputs
        .iter()
        .filter_map(|o| game.ctx.item(&o.item_id).map(|item| sale_value(game, item, None) * i64::from(o.quantity)))
        .sum()
}

/// The slots (item, quality) `after` holds more of than `before`.
fn gained(before: &[InventorySlot], after: &[InventorySlot]) -> Vec<InventorySlot> {
    let count = |slots: &[InventorySlot], id: &str, quality: &Option<String>| -> u32 {
        slots.iter().filter(|s| s.item.id == id && &s.quality == quality).map(|s| s.quantity).sum()
    };
    let mut seen: Vec<(String, Option<String>)> = Vec::new();
    let mut out = Vec::new();
    for slot in after {
        let key = (slot.item.id.clone(), slot.quality.clone());
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let more =
            count(after, &slot.item.id, &slot.quality).saturating_sub(count(before, &slot.item.id, &slot.quality));
        if more > 0 {
            out.push(InventorySlot { quantity: more, ..slot.clone() });
        }
    }
    out
}

fn milestone_costs(game: &Game) -> BTreeMap<String, i64> {
    let mut costs = BTreeMap::new();
    for shop in &game.ctx.content.shops {
        for upgrade in shop.tool_upgrades.iter().flatten() {
            costs.entry(format!("tool:{}", upgrade.to_item_id)).or_insert(upgrade.cost);
        }
    }
    for building in &game.ctx.content.building_types {
        costs.entry(format!("building:{}", building.id)).or_insert(building.cost);
    }
    costs
}

fn record_milestones(game: &Game, state: &GameState, run: &mut Run) {
    for (name, cost) in milestone_costs(game) {
        if state.player.money >= cost {
            run.milestones.entry(name).or_insert(state.clock.day);
        }
    }
    for quest in &state.player.completed_quests {
        run.milestones.entry(format!("quest:{quest}")).or_insert(state.clock.day);
    }
}

/// Per season, the crop a one-crop trial of that season earned most with.
fn best_plan(game: &Game, seed: &str, days: u32) -> BTreeMap<String, String> {
    let crops: Vec<String> =
        game.ctx.content.crops.keys().filter(|id| seed_item(game, id).is_some()).cloned().collect();
    let mut best: BTreeMap<String, (i64, String)> = BTreeMap::new();
    for crop in &crops {
        let run = play(game, &Strategy::Crop(crop.clone()), seed, days, &BTreeMap::new());
        let mut by_season: BTreeMap<String, i64> = BTreeMap::new();
        for day in &run.days {
            *by_season.entry(day.season.clone()).or_default() += day.income - day.spent;
        }
        for (season, profit) in by_season {
            match best.get(&season) {
                Some((value, _)) if *value >= profit => {}
                _ => {
                    best.insert(season, (profit, crop.clone()));
                }
            }
        }
    }
    best.into_iter().filter(|(_, (profit, _))| *profit > 0).map(|(season, (_, crop))| (season, crop)).collect()
}

/// Runs every strategy with every seed for `days` days.
pub fn run_lab(cart: &LoadedCartridge, strategies: &[Strategy], seeds: &[String], days: u32) -> Lab {
    let ctx = EngineContext::new(cart.content.clone());
    let buyer =
        ctx.content.shops.iter().filter(|shop| shop.buys_items).max_by_key(|shop| shop.sell_price_multiplier).cloned();
    let game = Game { ctx, cart, buyer };
    let mut runs = Vec::new();
    for seed in seeds {
        let needs_plan = strategies.iter().any(|s| !matches!(s, Strategy::Crop(_)));
        let plan = if needs_plan { best_plan(&game, seed, days) } else { BTreeMap::new() };
        for strategy in strategies {
            runs.push(play(&game, strategy, seed, days, &plan));
        }
    }
    let recipes = game
        .ctx
        .content
        .recipes
        .iter()
        .map(|recipe| RecipeRow {
            recipe: recipe.id.clone(),
            machine: recipe.machine_type_id.clone(),
            input_value: recipe
                .inputs
                .iter()
                .filter_map(|i| {
                    game.ctx.item(&i.item_id).map(|item| sale_value(&game, item, None) * i64::from(i.quantity))
                })
                .sum(),
            output_value: output_value(&game, recipe),
            minutes: recipe.processing_minutes,
        })
        .collect();
    let costs = milestone_costs(&game);
    let mut lab =
        Lab { content_hash: farm_sim::hash_state(&game.ctx.content), runs, recipes, costs, flags: Vec::new() };
    lab.flags = findings(&game, &lab, days);
    lab
}

fn findings(game: &Game, lab: &Lab, days: u32) -> Vec<String> {
    let mut flags = Vec::new();
    // Dominant crops: a crop whose one-crop runs earn more than twice the next best.
    let mut per_crop: BTreeMap<String, i64> = BTreeMap::new();
    for run in lab.runs.iter().filter(|run| run.strategy.starts_with("crop:")) {
        *per_crop.entry(run.strategy["crop:".len()..].to_owned()).or_default() += run.profit();
    }
    let mut ranked: Vec<(&String, &i64)> = per_crop.iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    if let [(best, top), (second, next), ..] = ranked.as_slice() {
        if **top > 0 && **top > 2 * (**next).max(0) {
            flags.push(format!("dominant crop: {best} earns {top} gold, more than twice {second} ({next})"));
        }
    }
    for recipe in &lab.recipes {
        if recipe.input_value > 0 && recipe.gain() < 0 {
            flags.push(format!(
                "recipe loses money: {} sells for {} but its ingredients sell for {}",
                recipe.recipe, recipe.output_value, recipe.input_value
            ));
        }
    }
    let processed: Vec<&RecipeRow> = lab.recipes.iter().filter(|r| r.machine.is_some() && r.gain() > 0).collect();
    if let [first, second, ..] = {
        let mut sorted = processed.clone();
        sorted.sort_by(|a, b| {
            b.gain_per_machine_day().cmp(&a.gain_per_machine_day()).then_with(|| a.recipe.cmp(&b.recipe))
        });
        sorted
    }
    .as_slice()
    {
        if first.gain_per_machine_day() > 2 * second.gain_per_machine_day() {
            flags.push(format!(
                "dominant recipe: {} gains {} gold a machine-day, more than twice {} ({})",
                first.recipe,
                first.gain_per_machine_day(),
                second.recipe,
                second.gain_per_machine_day()
            ));
        }
    }
    if game.buyer.is_none() {
        flags.push("no shop buys items: nothing the player grows can be sold".to_owned());
    } else {
        let mut produced: Vec<String> = game
            .ctx
            .content
            .crops
            .values()
            .filter_map(|crop| crops::harvest_item(&game.ctx.content, crop).ok().map(|item| item.id.clone()))
            .chain(game.ctx.content.recipes.iter().flat_map(|r| r.outputs.iter().map(|o| o.item_id.clone())))
            .collect();
        produced.sort();
        produced.dedup();
        for id in produced {
            if let Some(item) = game.ctx.item(&id) {
                if sale_value(game, item, None) <= 0 {
                    flags.push(format!("no buyer: {id} is grown or made but sells for nothing"));
                }
            }
        }
    }
    let season_days = game_time::calendar_seasons(&game.ctx.content.settings.calendar).first().map_or(28, |s| s.days);
    for name in lab.costs.keys() {
        let reached: Vec<u32> = lab.runs.iter().filter_map(|run| run.milestones.get(name).copied()).collect();
        if reached.is_empty() || reached.iter().all(|day| *day > season_days) {
            flags.push(format!(
                "milestone not reached within a season ({} days played): {name} costs {}",
                days.min(season_days),
                lab.costs[name]
            ));
        }
    }
    flags
}

/// The days of a season, or of a year, of the cartridge's calendar.
pub fn period_days(cart: &LoadedCartridge, year: bool) -> u32 {
    let seasons = game_time::calendar_seasons(&cart.content.settings.calendar);
    if year {
        seasons.iter().map(|s| s.days).sum()
    } else {
        seasons.first().map_or(28, |s| s.days)
    }
}

fn per_mille(numerator: i64, denominator: i64) -> String {
    if denominator == 0 {
        return String::new();
    }
    let value = numerator * 1000 / denominator;
    format!("{}.{:03}", value / 1000, (value % 1000).abs())
}

impl Lab {
    /// The CSV files: `runs.csv`, `days.csv`, `crops.csv`, `recipes.csv`, `milestones.csv`,
    /// `flags.csv`, as (name, text).
    pub fn csv(&self) -> Vec<(&'static str, String)> {
        let mut runs = String::from(
            "content_hash,strategy,seed,days,final_money,profit,energy_points,gold_per_day,gold_per_energy\n",
        );
        let mut days = String::from("strategy,seed,day,season,money,income,spent,energy_points\n");
        let mut crops = String::from(
            "strategy,seed,crop,planted,harvested,income,seed_cost,profit,energy_points,gold_per_energy\n",
        );
        let mut milestones = String::from("strategy,seed,milestone,cost,day\n");
        for run in &self.runs {
            let points = run.energy() / i64::from(units::ENERGY_POINT);
            let _ = writeln!(
                runs,
                "{},{},{},{},{},{},{},{},{}",
                self.content_hash,
                run.strategy,
                run.seed,
                run.days.len(),
                run.days.last().map_or(0, |d| d.money),
                run.profit(),
                points,
                per_mille(run.profit(), run.days.len() as i64),
                per_mille(run.profit() * i64::from(units::ENERGY_POINT), run.energy()),
            );
            for d in &run.days {
                let _ = writeln!(
                    days,
                    "{},{},{},{},{},{},{},{}",
                    run.strategy,
                    run.seed,
                    d.day,
                    d.season,
                    d.money,
                    d.income,
                    d.spent,
                    d.energy / i64::from(units::ENERGY_POINT)
                );
            }
            for (crop, row) in &run.crops {
                let profit = row.income - row.seed_cost;
                let _ = writeln!(
                    crops,
                    "{},{},{crop},{},{},{},{},{profit},{},{}",
                    run.strategy,
                    run.seed,
                    row.planted,
                    row.harvested,
                    row.income,
                    row.seed_cost,
                    row.energy / i64::from(units::ENERGY_POINT),
                    per_mille(profit * i64::from(units::ENERGY_POINT), row.energy)
                );
            }
            for (name, day) in &run.milestones {
                let cost = self.costs.get(name).map_or_else(String::new, i64::to_string);
                let _ = writeln!(milestones, "{},{},{name},{cost},{day}", run.strategy, run.seed);
            }
        }
        let mut recipes = String::from("recipe,machine,input_value,output_value,gain,minutes,gain_per_machine_day\n");
        for r in &self.recipes {
            let _ = writeln!(
                recipes,
                "{},{},{},{},{},{},{}",
                r.recipe,
                r.machine.as_deref().unwrap_or(""),
                r.input_value,
                r.output_value,
                r.gain(),
                r.minutes,
                if r.machine.is_some() { r.gain_per_machine_day().to_string() } else { String::new() }
            );
        }
        let mut flags = String::from("finding\n");
        for flag in &self.flags {
            let _ = writeln!(flags, "\"{}\"", flag.replace('"', "\"\""));
        }
        vec![
            ("runs.csv", runs),
            ("days.csv", days),
            ("crops.csv", crops),
            ("recipes.csv", recipes),
            ("milestones.csv", milestones),
            ("flags.csv", flags),
        ]
    }

    /// A short report: each strategy averaged over the seeds, then the findings.
    pub fn summary(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "content {}", self.content_hash);
        let _ = writeln!(
            out,
            "{:<24} {:>6} {:>10} {:>10} {:>12}",
            "strategy", "seeds", "profit", "gold/day", "gold/energy"
        );
        let mut by: BTreeMap<&str, Vec<&Run>> = BTreeMap::new();
        for run in &self.runs {
            by.entry(&run.strategy).or_default().push(run);
        }
        for (strategy, runs) in by {
            let n = runs.len() as i64;
            let profit: i64 = runs.iter().map(|r| r.profit()).sum::<i64>() / n.max(1);
            let days: i64 = runs.iter().map(|r| r.days.len() as i64).sum::<i64>() / n.max(1);
            let energy: i64 = runs.iter().map(|r| r.energy()).sum::<i64>() / n.max(1);
            let _ = writeln!(
                out,
                "{strategy:<24} {n:>6} {profit:>10} {:>10} {:>12}",
                per_mille(profit, days),
                per_mille(profit * i64::from(units::ENERGY_POINT), energy)
            );
        }
        if !self.flags.is_empty() {
            let _ = writeln!(out, "\nfindings:");
            for flag in &self.flags {
                let _ = writeln!(out, "- {flag}");
            }
        }
        out
    }
}
