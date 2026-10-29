//! M7: combat and adventure — health, fainting, monsters, weapons, and mine floors with
//! hazards and treasure.

mod common;

use common::{has_message, make_engine};
use farm_sim::schema::{
    CombatSettings, GameProject, GameState, Item, MineBand, MineConfig, MineHazard, MineMonsterWeight, MineTreasure,
    MonsterDefinition, MonsterSpawn, MonsterState, NodeDrop, TerrainDefinition, ToolArea, ToolDefinition,
};
use farm_sim::{engine, mines, units, Command, CommandRules, Effect, EngineContext};

fn run(ctx: &EngineContext, state: &mut GameState, command: Command) -> Vec<Effect> {
    engine::apply_command(ctx, state, &command)
}

fn combat(project: &mut GameProject) {
    project.settings.combat = Some(CombatSettings {
        enabled: true,
        max_health: 20,
        faint_money_penalty: 30,
        faint_scene_id: Some("scene-test".into()),
        faint_x: Some(0),
        faint_y: Some(5),
        ..CombatSettings::default()
    });
    project.items.push(Item {
        id: "slime-goo".into(),
        name: "Slime goo".into(),
        r#type: "crop".into(),
        stackable: true,
        max_stack: 99,
        value: 4,
        ..Item::default()
    });
    project.items.push(Item {
        id: "sword".into(),
        name: "Sword".into(),
        r#type: "tool".into(),
        tool_type: Some("sword".into()),
        tool_power: Some(4),
        ..Item::default()
    });
    project.items.push(Item {
        id: "berry".into(),
        name: "Berry".into(),
        r#type: "food".into(),
        stackable: true,
        max_stack: 99,
        health: Some(5),
        ..Item::default()
    });
    project.tools = Some(vec![ToolDefinition {
        id: "sword".into(),
        name: "Sword".into(),
        action: "attack".into(),
        area: vec![ToolArea { width: 1, length: 1 }],
        ..ToolDefinition::default()
    }]);
    project.monsters = Some(vec![MonsterDefinition {
        id: "slime".into(),
        name: "Slime".into(),
        health: 6,
        damage: 4,
        defense: 1,
        sight: 8,
        drops: vec![NodeDrop { item_id: "slime-goo".into(), min: 1, max: 1, weight: 1, ..NodeDrop::default() }],
        experience: 7,
        ..MonsterDefinition::default()
    }]);
    let sword = project.items.iter().find(|item| item.id == "sword").unwrap().clone();
    project.player.inventory.push(farm_sim::schema::InventorySlot::new(sword, 1));
    project.player.money = 100;
    project.settings.skills_enabled = true;
}

fn slime(state: &mut GameState, x: i32, y: i32, health: i32) {
    let id = state.next_monster;
    state.next_monster += 1;
    state.monsters.push(MonsterState {
        id,
        monster_id: "slime".into(),
        scene_id: "scene-test".into(),
        x: units::tile_center(x),
        y: units::tile_center(y),
        health,
        ..MonsterState::default()
    });
}

#[test]
fn without_combat_there_is_no_health() {
    let (ctx, mut state) = make_engine(|_| {}, "peace");
    assert_eq!(state.player.health, None);
    engine::advance_tick(&ctx, &mut state, 5);
    assert!(state.monsters.is_empty());
}

#[test]
fn monsters_spawn_chase_and_hurt_and_the_player_faints_at_zero() {
    let (ctx, mut state) = make_engine(
        |project| {
            combat(project);
            project.scenes[0].monsters = Some(vec![MonsterSpawn {
                monster_id: "slime".into(),
                count: 1,
                x: Some(0),
                y: Some(0),
                ..MonsterSpawn::default()
            }]);
        },
        "chase",
    );
    assert_eq!(state.player.health, Some(20));
    engine::advance_tick(&ctx, &mut state, 1);
    assert_eq!(state.monsters.len(), 1, "the scene's monster comes with the first tick");
    let start = (state.monsters[0].x, state.monsters[0].y);
    assert_eq!((units::tile_of(start.0), units::tile_of(start.1)), (0, 0));
    let mut hurt = Vec::new();
    for _ in 0..200 {
        hurt.extend(engine::advance_tick(&ctx, &mut state, 1));
        if state.player.health != Some(20) {
            break;
        }
    }
    assert!(hurt.iter().any(|effect| matches!(effect, Effect::PlayerHurt { amount: 4 })), "it reached the player");
    assert_eq!(state.player.health, Some(16));
    // The player reels: the next touches do no harm until the hurt time is over.
    engine::advance_tick(&ctx, &mut state, 5);
    assert_eq!(state.player.health, Some(16));

    state.player.health = Some(3);
    state.player.hurt = 0;
    let fainted = engine::advance_tick(&ctx, &mut state, 1);
    assert!(has_message(&fainted, |text| text.starts_with("You fainted!")), "{fainted:?}");
    assert_eq!(state.player.health, Some(10));
    assert_eq!(state.player.money, 70);
    assert_eq!((units::tile_of(state.player.x), units::tile_of(state.player.y)), (0, 5));
    // Sleep fills health and brings a fresh monster.
    run(&ctx, &mut state, Command::Sleep);
    assert_eq!(state.player.health, Some(20));
    assert_eq!(state.monsters.len(), 1);
}

#[test]
fn a_weapon_hits_slays_and_gives_drops_and_experience() {
    let (ctx, mut state) = make_engine(combat, "sword");
    // The player at (3,4) faces up: (3,3).
    slime(&mut state, 3, 3, 6);
    let hit = run(&ctx, &mut state, Command::UseTool { tool: "sword".into() });
    assert_eq!(state.monsters[0].health, 3, "4 power less 1 defense: {hit:?}");
    assert!(state.monsters[0].stunned > 0);
    // Knocked back a little: put it in front again.
    state.monsters[0].x = units::tile_center(3);
    state.monsters[0].y = units::tile_center(3);
    let slain = run(&ctx, &mut state, Command::UseTool { tool: "sword".into() });
    assert!(has_message(&slain, |text| text == "Defeated the Slime! Got 1x Slime goo"), "{slain:?}");
    assert!(state.monsters.is_empty());
    assert_eq!(state.slain.get("slime"), Some(&1));
    assert_eq!(farm_sim::inventory::count_item(&state.player.inventory, "slime-goo"), 1);
    assert_eq!(state.player.skills.get("combat").map(|skill| skill.xp), Some(7));
    let condition = farm_sim::schema::EventCondition::Slain { monster_id: String::new(), min: 1 };
    assert!(farm_sim::events::condition_met(&ctx, &state, &condition, None));
}

#[test]
fn a_luck_buff_changes_monster_loot_without_advancing_extra_randomness() {
    let (ctx, mut state) = make_engine(
        |project| {
            combat(project);
            project.monsters.as_mut().unwrap()[0].drops = vec![
                NodeDrop { item_id: "slime-goo".into(), min: 1, max: 1, weight: 100, ..Default::default() },
                NodeDrop { item_id: "berry".into(), min: 1, max: 1, weight: 10, ..Default::default() },
            ];
        },
        "lucky-sword",
    );
    slime(&mut state, 3, 3, 1);
    state.rng = (0..1024)
        .map(farm_sim::rng::create_rng_state_from_u32)
        .find(|seed| {
            let mut plain = farm_sim::rng::Rng::new(seed.clone());
            let mut lucky = plain.clone();
            plain.weighted(&[100, 10]) == Some(0) && lucky.weighted(&[200, 110]) == Some(1)
        })
        .unwrap();
    let mut lucky = state.clone();
    farm_sim::buffs::apply(
        &ctx,
        &mut lucky,
        &farm_sim::schema::BuffDefinition {
            id: "luck".into(),
            stat: "luck".into(),
            amount: 100,
            duration_minutes: 10,
            ..Default::default()
        },
        "test",
    );
    run(&ctx, &mut state, Command::UseTool { tool: "sword".into() });
    run(&ctx, &mut lucky, Command::UseTool { tool: "sword".into() });
    assert_eq!(farm_sim::inventory::count_item(&state.player.inventory, "slime-goo"), 1);
    assert_eq!(farm_sim::inventory::count_item(&lucky.player.inventory, "berry"), 1);
    assert_eq!(lucky.rng, state.rng);
}

#[test]
fn food_heals_and_hazard_terrain_hurts() {
    let (ctx, mut state) = make_engine(
        |project| {
            combat(project);
            let berry = project.items.iter().find(|item| item.id == "berry").unwrap().clone();
            project.player.inventory.push(farm_sim::schema::InventorySlot::new(berry, 2));
            project.item_categories = Some(vec![farm_sim::schema::ItemCategoryDefinition {
                id: "food".into(),
                name: "Food".into(),
                edible: true,
                ..farm_sim::schema::ItemCategoryDefinition::default()
            }]);
            project.terrains = Some(vec![TerrainDefinition {
                id: "lava".into(),
                name: "Lava".into(),
                layer: "overlay".into(),
                damage: 6,
                ..TerrainDefinition::default()
            }]);
            project.scenes[0].tiles[3][3].overlay = Some("lava".into());
        },
        "lava",
    );
    let ctx = ctx.with_rules(CommandRules::Scripted);
    let stepped = run(&ctx, &mut state, Command::Move { dir: "up".into() });
    assert!(has_message(&stepped, |text| text == "Lava hurts!"));
    assert_eq!(state.player.health, Some(14));
    run(&ctx, &mut state, Command::UseItem { item_id: "berry".into() });
    assert_eq!(state.player.health, Some(19));
    run(&ctx, &mut state, Command::UseItem { item_id: "berry".into() });
    assert_eq!(state.player.health, Some(20), "no more than the most");
}

#[test]
fn mine_floors_have_hazards_treasure_and_monsters() {
    let (ctx, mut state) = make_engine(
        |project| {
            combat(project);
            project.terrains = Some(vec![TerrainDefinition {
                id: "spikes".into(),
                name: "Spikes".into(),
                layer: "overlay".into(),
                damage: 2,
                ..TerrainDefinition::default()
            }]);
            project.mine = MineConfig {
                enabled: true,
                entrance_scene_id: Some("scene-test".into()),
                entrance_x: Some(3),
                entrance_y: Some(3),
                floor_width: 12,
                floor_height: 12,
                bands: vec![MineBand {
                    from_floor: 1,
                    to_floor: 20,
                    density: 0,
                    monsters: vec![MineMonsterWeight { monster_id: "slime".into(), weight: 1 }],
                    monster_count: 3,
                    hazards: vec![MineHazard { terrain_id: "spikes".into(), density: 200 }],
                    treasure: vec![MineTreasure { item_id: "slime-goo".into(), weight: 1 }],
                    treasure_count: 2,
                    ..MineBand::default()
                }],
                ..MineConfig::default()
            };
        },
        "mine",
    );
    let ctx = ctx.with_rules(CommandRules::Scripted);
    run(&ctx, &mut state, Command::DescendMine { floor: 1 });
    let floor = farm_sim::world::world_movement::find_scene(&state, &mines::mine_floor_scene_id(1)).unwrap();
    let tiles = floor.tiles.iter().flatten();
    let spikes = tiles.clone().filter(|tile| tile.overlay.as_deref() == Some("spikes")).count();
    let treasure = tiles.filter(|tile| tile.item.is_some()).count();
    assert!(spikes > 0);
    assert_eq!(treasure, 2);
    assert_eq!(state.monsters.len(), 3);
    assert!(state.monsters.iter().all(|monster| monster.scene_id == "mine-floor-1"));
    // The same floor every time.
    let first = state.world.scenes.iter().find(|scene| scene.id == "mine-floor-1").cloned();
    run(&ctx, &mut state, Command::ExitMine);
    assert!(state.monsters.is_empty(), "leaving the mine leaves its monsters");
    run(&ctx, &mut state, Command::DescendMine { floor: 1 });
    assert_eq!(state.world.scenes.iter().find(|scene| scene.id == "mine-floor-1").cloned(), first);
}
