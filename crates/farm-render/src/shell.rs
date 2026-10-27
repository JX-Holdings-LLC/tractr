//! Port of `ShellSnapshot.cs`: [`shell_snapshot`] builds a play-mode [`WorldSnapshot`] from
//! engine content and live state (packages/game-shell/src/snapshot.ts), and [`editor_snapshot`]
//! the Edit Mode counterpart from the project (web `GameView.buildSnapshot` outside play).
//! Neither mutates the simulation, reads I/O or OS state. Art bindings and transient pops are
//! added afterwards ([`crate::apply_graphics`], the host's pops).

use crate::builtin_art::BuiltinArt;
use crate::num::to_int;
use crate::snapshot::{
    SnapshotAtmosphere, SnapshotBuilding, SnapshotCamera, SnapshotCrop, SnapshotDecor, SnapshotEmote, SnapshotEntity,
    SnapshotItem, SnapshotLight, SnapshotMachine, SnapshotMonster, SnapshotNode, SnapshotTerrain, SnapshotTile,
    TileWindow, WorldSnapshot, MAX_TALL,
};
use farm_sim::cutscenes::{self, CutsceneView};
use farm_sim::farming::crops;
use farm_sim::schema::{
    directions, soil_states, tile_types, wear_slots, AtmosphereSettings, BuildingDefinition, GameContent, GameProject,
    GameState, LightSource, Scene,
};
use farm_sim::units::{self, MicroMinutes};
use farm_sim::vocabulary;
use farm_sim::world::tiles;
use serde::{Deserialize, Serialize};

/// How big a child is next to a grown-up (M4).
pub const CHILD_SCALE: f64 = 0.7;

/// Options of [`shell_snapshot`] (C# `ShellSnapshotOptions`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SnapshotOptions {
    pub tile_size: f64,
    pub padding: f64,
    /// Interpolated player position in world pixels (play mode).
    pub pixel_x: Option<f64>,
    pub pixel_y: Option<f64>,
    pub camera: Option<SnapshotCamera>,
    /// Build only these tiles (a player builds what its camera shows, not a whole 256×256
    /// scene every frame); `None` builds every tile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tile_window: Option<TileWindow>,
}

/// A cell outside the snapshot's tile window: never drawn, allocates nothing.
fn unbuilt_tile() -> SnapshotTile {
    SnapshotTile {
        background: String::new(),
        overlay: None,
        object: None,
        art_layers: None,
        crop: None,
        node: None,
        machine: None,
        ladder_down: false,
        item: None,
        tilled: false,
        watered: false,
        fertilized: false,
        decor: Vec::new(),
    }
}

fn direction_toward(x: f64, y: f64, target_x: f64, target_y: f64) -> &'static str {
    let (dx, dy) = (target_x - x, target_y - y);
    if dx.abs() > dy.abs() {
        if dx < 0.0 {
            directions::LEFT
        } else {
            directions::RIGHT
        }
    } else if dy < 0.0 {
        directions::UP
    } else {
        directions::DOWN
    }
}

/// Definition lookup by id where later definitions win (`new Map(entries)`).
fn last_by_id<'a, T>(definitions: &'a [T], id: &str, key: impl Fn(&T) -> &str) -> Option<&'a T> {
    definitions.iter().rev().find(|definition| key(definition) == id)
}

/// `(int)scene.Height` rows / columns, looping like C# `for (y = 0; y < scene.Height; y++)`.
fn cells(extent: i32) -> usize {
    usize::try_from(extent).unwrap_or(0)
}

/// A simulation position (1/8192 tile) in tiles, as the draw list uses them.
fn tiles_of(position: i32) -> f64 {
    units::position_to_tiles(position)
}

/// The game's terrains as the renderer reads them (M1): colour and farmable ground.
fn snapshot_terrains(content: &GameContent) -> Vec<SnapshotTerrain> {
    vocabulary::terrains(content)
        .iter()
        .map(|terrain| SnapshotTerrain {
            id: terrain.id.clone(),
            color: terrain.color.clone(),
            plantable: terrain.plantable,
            autotile: terrain.autotile == Some(true),
            tall: terrain.tall.unwrap_or(0).min(MAX_TALL),
        })
        .collect()
}

/// The lights of `scene` (M8) near the tiles of `window` (all of them without one): terrains on
/// any layer, standing nodes, machines, grown crops and standing buildings that give light.
fn scene_lights(
    content: &GameContent,
    scene: &Scene,
    buildings: &[(i32, i32, &BuildingDefinition)],
    window: Option<TileWindow>,
) -> Vec<SnapshotLight> {
    let terrains = vocabulary::terrains(content);
    let mut lights = Vec::new();
    let near = |x: i32, y: i32, radius: u32| {
        window.is_none_or(|w| {
            let r = i32::try_from(radius).unwrap_or(i32::MAX);
            x >= w.x0.saturating_sub(r)
                && x <= w.x1.saturating_add(r)
                && y >= w.y0.saturating_sub(r)
                && y <= w.y1.saturating_add(r)
        })
    };
    let mut push = |x: f64, y: f64, light: &LightSource| {
        lights.push(SnapshotLight {
            x,
            y,
            radius: f64::from(light.radius.clamp(1, 16)),
            color: light.color.clone(),
            brightness: f64::from(light.brightness.clamp(1, 100)) / 100.0,
        });
    };
    // A light reaches 16 tiles at most: rows farther from the window give none.
    let (first_row, last_row) = window.map_or((0, cells(scene.height)), |w| {
        (cells(w.y0.saturating_sub(16)), cells(w.y1.saturating_add(17)).min(cells(scene.height)))
    });
    for (y, row) in scene.tiles.iter().enumerate().take(last_row).skip(first_row) {
        for (x, tile) in row.iter().enumerate() {
            let (tx, ty) = (x as i32, y as i32);
            let centre = (x as f64 + 0.5, y as f64 + 0.5);
            let layers = [Some(tile.background.as_str()), tile.overlay.as_deref(), tile.object.as_deref()];
            let decor = tile.decor.iter().flatten().map(|(_, terrain)| Some(terrain.as_str()));
            for id in layers.into_iter().chain(decor).flatten() {
                if let Some(light) = vocabulary::find_terrain(terrains, id).and_then(|t| t.light.as_ref()) {
                    if near(tx, ty, light.radius) {
                        push(centre.0, centre.1, light);
                    }
                }
            }
            let node = tile.node.as_ref().filter(|node| node.remaining_health > 0);
            if let Some(light) = node
                .and_then(|node| last_by_id(&content.node_types, &node.type_id, |d| &d.id))
                .and_then(|d| d.light.as_ref())
            {
                if near(tx, ty, light.radius) {
                    push(centre.0, centre.1, light);
                }
            }
            let machine = tile
                .machine
                .as_ref()
                .and_then(|machine| last_by_id(&content.machine_types, &machine.type_id, |d| &d.id));
            if let Some(light) = machine.and_then(|d| d.light.as_ref()) {
                if near(tx, ty, light.radius) {
                    push(centre.0, centre.1, light);
                }
            }
            if let Some(crop) = tile.crop.as_ref().filter(|crop| crop.withered != Some(true)) {
                let definition = content.crops.get(&crop.r#type);
                let grown = definition.is_some_and(|d| crops::is_crop_mature_by_days(crop, d));
                if let Some(light) = definition.and_then(|d| d.light.as_ref()).filter(|_| grown) {
                    if near(tx, ty, light.radius) {
                        push(centre.0, centre.1, light);
                    }
                }
            }
        }
    }
    // A building's windows: along its front, a tile above the door row.
    for (bx, by, definition) in buildings {
        if let Some(light) = &definition.light {
            let width = f64::from(definition.width.max(1));
            let height = f64::from(definition.height.max(1));
            if near(*bx, *by, light.radius.saturating_add(definition.width.max(definition.height))) {
                push(f64::from(*bx) + (width / 2.0), f64::from(*by) + height - 0.5, light);
            }
        }
    }
    lights
}

/// The atmosphere of `scene` at the clock (see [`scene_atmosphere`]).
fn snapshot_atmosphere(content: &GameContent, state: &GameState, scene: &Scene) -> SnapshotAtmosphere {
    let clock = &state.clock;
    let time_minutes = units::to_f64::<MicroMinutes>(clock.time_minutes);
    scene_atmosphere(content, scene, time_minutes, &clock.season, Some(&clock.weather_id))
}

/// The atmosphere of `scene` at `time_minutes` of a day in `season` under the weather type
/// `weather_id`: the game's look for the scene and season (M1), its day length, and the weather's
/// overlay resolved from the weather type. An indoor scene keeps the weather out: no particles and
/// no weather tint, only the day/night tint. Play Mode builds its frames with it from the clock,
/// and Edit Mode's sky preview from the time, season and weather the creator picks
/// ([`SkyPreview`]), so the two look the same.
pub fn scene_atmosphere(
    content: &GameContent,
    scene: &Scene,
    time_minutes: f64,
    season: &str,
    weather_id: Option<&str>,
) -> SnapshotAtmosphere {
    let settings = &content.settings;
    let look = AtmosphereSettings::resolve(settings.atmosphere.as_ref(), season, scene.atmosphere.as_ref());
    let day_minutes = settings.time.minutes_per_day();
    let weather_id = weather_id.filter(|_| !scene.is_indoor());
    let overlay_id = weather_id
        .and_then(|id| content.weather.types.iter().find(|weather| weather.id == id))
        .and_then(|weather| weather.overlay.clone());
    SnapshotAtmosphere {
        time_minutes,
        weather_id: weather_id.map(str::to_owned),
        season: Some(season.to_owned()),
        overlay: overlay_id.as_deref().and_then(|id| content.weather.overlay(id)),
        weather_overlay: overlay_id,
        day_minutes: (day_minutes != units::MINUTES_PER_DAY).then_some(f64::from(day_minutes)),
        daylight: (!look.daylight.is_empty()).then_some(look.daylight),
        foliage_tint: look.foliage_tint,
    }
}

/// What Edit Mode's sky preview draws a scene under (editor view state, never saved): a minute of
/// the day, a season id and a weather type id (`None`: clear). The particles stand still, so the
/// preview never moves (and nothing changes with reduced motion).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SkyPreview {
    pub time_minutes: f64,
    /// Empty: the project's starting season.
    pub season: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weather_id: Option<String>,
}

/// [`scene_atmosphere`] for Edit Mode's sky preview of `scene` in `project`.
pub fn preview_atmosphere(
    project: &GameProject,
    content: &GameContent,
    scene: &Scene,
    sky: &SkyPreview,
) -> SnapshotAtmosphere {
    let season = if sky.season.is_empty() { project.current_season.as_str() } else { sky.season.as_str() };
    let time_minutes = if sky.time_minutes.is_finite() { sky.time_minutes } else { 0.0 };
    scene_atmosphere(content, scene, time_minutes, season, sky.weather_id.as_deref())
}

fn build_tiles(
    scene: &Scene,
    content: &GameContent,
    hide_item_at: impl Fn(usize, usize) -> bool,
    window: Option<TileWindow>,
) -> Vec<Vec<SnapshotTile>> {
    let terrains = vocabulary::terrains(content);
    let mut rows = Vec::with_capacity(cells(scene.height));
    for y in 0..cells(scene.height) {
        // Rows outside the window stay empty; inside, cells left of it are stand-ins and the
        // row ends with the window.
        let (first, end) = match window {
            Some(window) if (y as i64) < i64::from(window.y0) || (y as i64) > i64::from(window.y1) => (0, 0),
            Some(window) => (cells(window.x0), cells(scene.width).min(cells(window.x1).saturating_add(1))),
            None => (0, cells(scene.width)),
        };
        let mut row = Vec::with_capacity(end);
        row.extend((0..first.min(end)).map(|_| unbuilt_tile()));
        let source_row = scene.tiles.get(y);
        for x in first..end {
            let empty;
            let tile = match source_row.and_then(|r| r.get(x)) {
                Some(tile) => tile,
                None => {
                    empty = tiles::create_empty_tile(x as i32, y as i32, tile_types::GRASS);
                    &empty
                }
            };
            let background = if tile.background.is_empty() { tile.r#type.clone() } else { tile.background.clone() };
            let soil = vocabulary::find_terrain(terrains, &background).is_some_and(|terrain| terrain.plantable);
            // Soil state (read only): plantable ground is drawn tilled; wet after watering or
            // rain (moisture), speckled once fertilized.
            let watered = soil && (tile.soil_state.as_deref() == Some(soil_states::WATERED) || tile.soil_moisture > 0);
            let fertilized =
                soil && (tile.soil_state.as_deref() == Some(soil_states::FERTILIZED) || tile.soil_fertility > 0);
            let crop = tile.crop.as_ref().and_then(|crop| {
                let definition = content.crops.get(&crop.r#type)?;
                Some(SnapshotCrop {
                    sprite: None,
                    color_index: i32::try_from(crop.stage).unwrap_or(i32::MAX),
                    mature: crops::is_crop_mature_by_days(crop, definition),
                    withered: crop.withered == Some(true),
                    crop_id: Some(crop.r#type.clone()),
                    stages: i32::try_from(definition.stages).unwrap_or(i32::MAX),
                })
            });
            let node = tile.node.as_ref().map(|node| SnapshotNode {
                sprite: None,
                color: last_by_id(&content.node_types, &node.type_id, |d| &d.id)
                    .map_or("#7a5a3a", |d| d.color.as_str())
                    .to_owned(),
                depleted: node.remaining_health <= 0,
                type_id: Some(node.type_id.clone()),
            });
            let machine = tile.machine.as_ref().map(|machine| SnapshotMachine {
                sprite: None,
                color: last_by_id(&content.machine_types, &machine.type_id, |d| &d.id)
                    .map_or("#9a7b4f", |d| d.color.as_str())
                    .to_owned(),
                working: machine.processing.is_some(),
                output_ready: machine.output.as_ref().is_some_and(|output| !output.is_empty()),
                type_id: Some(machine.type_id.clone()),
                open: machine.open == Some(true),
            });
            let item = tile
                .item
                .as_ref()
                .filter(|_| !hide_item_at(x, y))
                .map(|item| SnapshotItem { sprite: None, item_type: Some(item.r#type.clone()) });
            // The scene's extra layers (M8), in its order; a layer the scene lacks is not drawn.
            let decor = match (&tile.decor, &scene.layers) {
                (Some(painted), Some(layers)) => layers
                    .iter()
                    .filter_map(|layer| {
                        painted.get(&layer.id).map(|terrain| SnapshotDecor {
                            terrain: terrain.clone(),
                            above: layer.above == Some(true),
                            sprite: None,
                        })
                    })
                    .collect(),
                _ => Vec::new(),
            };
            row.push(SnapshotTile {
                background,
                overlay: tile.overlay.clone(),
                object: tile.object.clone(),
                art_layers: None,
                crop,
                node,
                machine,
                ladder_down: tile.ladder_down == Some(true),
                item,
                tilled: soil,
                watered,
                fertilized,
                decor,
            });
        }
        rows.push(row);
    }
    rows
}

/// Port of `ShellSnapshot.BuildShellSnapshot`: the play-mode world of `scene` from content and
/// live state, with a contiguous world (no editor seams) and the clock's atmosphere. With
/// [`SnapshotOptions::tile_window`] only those tiles are built.
pub fn shell_snapshot(
    content: &GameContent,
    state: &GameState,
    scene: &Scene,
    options: &SnapshotOptions,
) -> WorldSnapshot {
    // A tall terrain (M8) below the view reaches into it: its rows are built too.
    let tallest = vocabulary::terrains(content).iter().filter_map(|terrain| terrain.tall).max().unwrap_or(0);
    let window = options.tile_window.map(|window| window.with_tall(tallest));
    let tiles = build_tiles(scene, content, |_, _| false, window);
    // A cutscene (M2) turns and walks characters and shows emotes over them.
    let cutscene = (scene.id == state.player.scene_id).then(|| cutscenes::view(content, state)).flatten();
    let directed = |actor: &str| cutscene_actor(cutscene.as_ref(), actor);

    let mut npcs = Vec::new();
    for npc in &content.npcs {
        let live = state.npcs.get(&npc.id);
        if live.map_or(&npc.scene_id, |live| &live.scene_id) != &scene.id {
            continue;
        }
        let (x, y) = live.map_or((npc.x, npc.y), |live| (live.x, live.y));
        let (x, y) = (tiles_of(x), tiles_of(y));
        // A scheduled or patrolling NPC with path steps left is walking toward the next step.
        let next = live.and_then(|live| live.path.as_ref()).and_then(|path| path.first());
        let (direction, moving) = directed(&npc.id).unwrap_or_else(|| {
            let direction =
                next.map_or(directions::DOWN, |next| direction_toward(x, y, f64::from(next.x), f64::from(next.y)));
            (direction.to_owned(), next.is_some())
        });
        // NPCs stand on whole tiles, except while a cutscene walks them: a fractional position
        // is their tile corner, not a centre (`entity_pixel_origin` reads it as one).
        let between_tiles = x.fract() != 0.0 || y.fract() != 0.0;
        let pixel = |coord: f64| between_tiles.then_some(options.padding + (coord * options.tile_size));
        npcs.push(SnapshotEntity {
            x,
            y,
            appearance: Some(npc.appearance.clone()),
            direction,
            moving,
            pixel_x: pixel(x),
            pixel_y: pixel(y),
            ..SnapshotEntity::default()
        });
    }
    for animal in state.animals.iter().filter(|animal| animal.scene_id == scene.id) {
        // Animals face a stable direction chosen from their id so a herd isn't uniform. .NET
        // `string.Length` counts UTF-16 units.
        let length = animal.id.encode_utf16().count() as i32;
        let (x, y) = (tiles_of(animal.x), tiles_of(animal.y));
        let direction = directions::ALL[(BuiltinArt::hash(length, to_int(x + y)) % 4) as usize];
        npcs.push(SnapshotEntity {
            x,
            y,
            color: last_by_id(&content.animal_species, &animal.species_id, |d| &d.id).map(|d| d.color.clone()),
            kind: "animal".to_owned(),
            species_id: Some(animal.species_id.clone()),
            direction: direction.to_owned(),
            ..SnapshotEntity::default()
        });
    }

    // The player's children live with the spouse (M4): small villagers beside them.
    let spouse = farm_sim::relationships::spouse(state).and_then(|id| state.npcs.get(id));
    if let Some(spouse) = spouse.filter(|spouse| spouse.scene_id == scene.id) {
        for (index, _) in state.children.iter().enumerate() {
            let offset = i32::try_from(index).unwrap_or(0).saturating_add(1);
            let x = units::tile_of(spouse.x).saturating_add(offset).min(scene.width - 1).max(0);
            npcs.push(SnapshotEntity {
                x: f64::from(x),
                y: tiles_of(spouse.y),
                appearance: Some("villager".to_owned()),
                scale: Some(CHILD_SCALE),
                ..SnapshotEntity::default()
            });
        }
    }

    // Monsters (M7) and buildings (M5) of the scene.
    let monsters = state
        .monsters
        .iter()
        .filter(|monster| monster.scene_id == scene.id)
        .map(|monster| {
            let definition = last_by_id(&content.monsters, &monster.monster_id, |d| &d.id);
            let max = definition.map_or(monster.health, |d| d.health).max(1);
            SnapshotMonster {
                x: tiles_of(monster.x),
                y: tiles_of(monster.y),
                visual: definition.and_then(|d| d.visual.clone()),
                sprite: None,
                color: definition.map_or_else(|| SnapshotMonster::default().color, |d| d.color.clone()),
                direction: if monster.direction.is_empty() {
                    directions::DOWN.to_owned()
                } else {
                    monster.direction.clone()
                },
                moving: !monster.direction.is_empty() && monster.stunned == 0,
                health: (monster.health < max).then(|| f64::from(monster.health.max(0)) / f64::from(max)),
                stunned: monster.stunned > 0,
            }
        })
        .collect();
    let standing: Vec<(i32, i32, &BuildingDefinition)> = state
        .buildings
        .iter()
        .filter(|building| building.scene_id == scene.id && building.built())
        .filter_map(|building| {
            last_by_id(&content.building_types, &building.building_id, |d| &d.id).map(|d| (building.x, building.y, d))
        })
        .collect();
    let lights = scene_lights(content, scene, &standing, window);
    let buildings = state
        .buildings
        .iter()
        .filter(|building| building.scene_id == scene.id)
        .filter_map(|building| {
            let definition = last_by_id(&content.building_types, &building.building_id, |d| &d.id)?;
            let (width, height) = definition.size();
            Some(SnapshotBuilding {
                x: building.x,
                y: building.y,
                width,
                height,
                visual: definition.visual.clone(),
                sprite: None,
                color: definition.color.clone(),
                scaffold: !building.built(),
                door: definition.interior_scene_id.is_some().then(|| definition.door_offset()),
            })
        })
        .collect();

    let hat_visual = farm_sim::wardrobe::worn_in(state, wear_slots::HAT)
        .and_then(|worn| worn.item.wear.as_ref())
        .and_then(|wear| wear.visual.clone());
    let clock = &state.clock;
    WorldSnapshot {
        width: scene.width,
        height: scene.height,
        tile_size: options.tile_size,
        padding: options.padding,
        tile_gap: Some(0.0),
        camera: options.camera,
        pixel_art: None,
        grid_overlay: false,
        tick: clock.tick as f64,
        atmosphere: Some(snapshot_atmosphere(content, state, scene)),
        terrains: Some(snapshot_terrains(content)),
        tiles,
        npcs,
        monsters,
        buildings,
        lights,
        player: SnapshotEntity {
            x: tiles_of(state.player.x),
            y: tiles_of(state.player.y),
            direction: state.player.direction.clone(),
            pixel_x: options.pixel_x,
            pixel_y: options.pixel_y,
            moving: state.player.move_intent.dx != 0
                || state.player.move_intent.dy != 0
                || directed(cutscenes::PLAYER).is_some_and(|(_, moving)| moving),
            look: crate::look::look_key(state.player.look.as_ref(), &state.player.worn),
            hat_visual,
            ..SnapshotEntity::default()
        },
        pops: None,
        emotes: cutscene.as_ref().filter(|view| !view.emotes.is_empty()).map(|view| {
            view.emotes
                .iter()
                .map(|emote| SnapshotEmote {
                    x: tiles_of(emote.x),
                    y: tiles_of(emote.y),
                    player: emote.actor == cutscenes::PLAYER,
                    emote: emote.emote.clone(),
                    progress: f64::from(emote.progress) / f64::from(units::MILLI_ONE),
                })
                .collect()
        }),
        tile_window: window,
    }
}

/// The direction and walk of an actor the running cutscene turns or walks.
fn cutscene_actor(view: Option<&CutsceneView>, actor: &str) -> Option<(String, bool)> {
    let actor = view?.actors.iter().find(|known| known.actor == actor)?;
    Some((actor.direction.clone(), actor.moving))
}

/// Port of `ShellSnapshot.BuildEditorSnapshot`: the authored `scene` of `project` with 1px grid
/// seams and the grid overlay, NPCs and animals at their authored positions, and items hidden
/// under the player and NPCs. `content` supplies the merged crop, node, machine and species
/// definitions (`farm_sim::state::create_content_from_project`). Edit Mode then calls
/// [`crate::apply_graphics`] with [`crate::GraphicsSource::from_project`], tick 0, not moving.
pub fn editor_snapshot(
    project: &GameProject,
    content: &GameContent,
    scene: &Scene,
    tile_size: f64,
    padding: f64,
) -> WorldSnapshot {
    let scene_npcs: Vec<_> = project.npcs.iter().filter(|npc| npc.scene_id == scene.id).collect();
    let player = &project.player;
    let occupied = |x: usize, y: usize| {
        let (x, y) = (x as i32, y as i32);
        (player.scene_id == scene.id && units::tile_of(player.x) == x && units::tile_of(player.y) == y)
            || scene_npcs.iter().any(|npc| npc.x == units::tiles(x) && npc.y == units::tiles(y))
    };
    let tiles = build_tiles(scene, content, occupied, None);

    let mut npcs: Vec<SnapshotEntity> = scene_npcs
        .iter()
        .map(|npc| SnapshotEntity {
            x: tiles_of(npc.x),
            y: tiles_of(npc.y),
            appearance: Some(npc.appearance.clone()),
            ..SnapshotEntity::default()
        })
        .collect();
    npcs.extend(project.animals.iter().filter(|animal| animal.scene_id == scene.id).map(|animal| SnapshotEntity {
        x: tiles_of(animal.x),
        y: tiles_of(animal.y),
        color: last_by_id(&content.animal_species, &animal.species_id, |d| &d.id).map(|d| d.color.clone()),
        kind: "animal".to_owned(),
        species_id: Some(animal.species_id.clone()),
        ..SnapshotEntity::default()
    }));

    // The player marker only belongs on its own scene.
    let on_scene = player.scene_id == scene.id;
    WorldSnapshot {
        width: scene.width,
        height: scene.height,
        tile_size,
        padding,
        tile_gap: Some(1.0),
        camera: None,
        pixel_art: None,
        grid_overlay: true,
        tick: 0.0,
        atmosphere: None,
        terrains: Some(snapshot_terrains(content)),
        tiles,
        npcs,
        monsters: Vec::new(),
        buildings: Vec::new(),
        // The sky preview shows them at night (M8).
        lights: scene_lights(content, scene, &[], None),
        player: SnapshotEntity {
            x: if on_scene { tiles_of(player.x) } else { -1000.0 },
            y: if on_scene { tiles_of(player.y) } else { -1000.0 },
            direction: if player.direction.is_empty() { directions::DOWN.to_owned() } else { player.direction.clone() },
            ..SnapshotEntity::default()
        },
        pops: None,
        emotes: None,
        tile_window: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn starter() -> GameProject {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../fixtures/golden/content/starter-farm.json")).unwrap();
        serde_json::from_value(fixture["project"].clone()).unwrap()
    }

    #[test]
    fn snapshot_is_read_only_and_tracks_crop_maturity() {
        let project = starter();
        let content = farm_sim::state::create_content_from_project(&project);
        let mut state = farm_sim::state::create_game_state(&project, Some("snapshot"));
        let mut crop = crops::create_planted_crop("wheat", 1, "normal");
        crop.days_grown = Some(99);
        state.world.scenes[0].tiles[0][0].crop = Some(crop);
        let before = farm_sim::hash_state(&state);
        let options = SnapshotOptions { tile_size: 32.0, padding: 12.0, pixel_x: Some(15.5), ..Default::default() };
        let view = shell_snapshot(&content, &state, &state.world.scenes[0], &options);
        assert!(view.tiles[0][0].crop.as_ref().unwrap().mature);
        assert_eq!(view.player.pixel_x, Some(15.5));
        assert_eq!(view.tiles.len(), state.world.scenes[0].height as usize);
        assert_eq!(view.tile_gap, Some(0.0));
        assert!(!view.grid_overlay);
        assert!(view.atmosphere.is_some());
        assert_eq!(farm_sim::hash_state(&state), before);
    }

    #[test]
    fn shell_snapshot_maps_layers_nodes_and_entities() {
        let project = starter();
        let content = farm_sim::state::create_content_from_project(&project);
        let state = farm_sim::state::create_game_state(&project, None);
        let scene = state.world.scenes.iter().find(|s| s.id == state.player.scene_id).unwrap();
        let view = shell_snapshot(&content, &state, scene, &SnapshotOptions { tile_size: 32.0, ..Default::default() });
        assert_eq!((view.width, view.height), (scene.width as i32, scene.height as i32));
        assert!(view.tiles.iter().all(|row| row.len() == scene.width as usize));
        let expected_npcs = content.npcs.iter().filter(|n| n.scene_id == scene.id).count()
            + state.animals.iter().filter(|a| a.scene_id == scene.id).count();
        assert_eq!(view.npcs.len(), expected_npcs);
        let tile = &scene.tiles[0][0];
        let background = if tile.background.is_empty() { &tile.r#type } else { &tile.background };
        assert_eq!(&view.tiles[0][0].background, background);
        let (nx, ny, node_tile) = scene
            .tiles
            .iter()
            .enumerate()
            .find_map(|(y, row)| row.iter().enumerate().find(|(_, t)| t.node.is_some()).map(|(x, t)| (x, y, t)))
            .unwrap();
        let node = view.tiles[ny][nx].node.as_ref().unwrap();
        let type_id = &node_tile.node.as_ref().unwrap().type_id;
        assert_eq!(node.color, content.node_types.iter().rev().find(|n| &n.id == type_id).unwrap().color);
        assert!(!node.depleted);
    }

    #[test]
    fn editor_snapshot_uses_grid_seams_and_hides_items_under_the_player() {
        let mut project = starter();
        let content = farm_sim::state::create_content_from_project(&project);
        let scene_index = project.scenes.iter().position(|s| s.id == project.player.scene_id).unwrap();
        let (px, py) = (units::tile_of(project.player.x) as usize, units::tile_of(project.player.y) as usize);
        let item = project.items[0].clone();
        project.scenes[scene_index].tiles[py][px].item = Some(item.clone());
        project.scenes[scene_index].tiles[0][1].item = Some(item);
        let scene = project.scenes[scene_index].clone();
        let view = editor_snapshot(&project, &content, &scene, 28.0, 12.0);
        assert_eq!(view.tile_gap, Some(1.0));
        assert!(view.grid_overlay);
        assert_eq!(view.tile_size, 28.0);
        assert!(view.atmosphere.is_none());
        assert!(view.tiles[py][px].item.is_none());
        assert!(view.tiles[0][1].item.is_some());
        // Off-scene players are parked far away.
        let mut other = scene.clone();
        other.id = "elsewhere".into();
        let off = editor_snapshot(&project, &content, &other, 28.0, 12.0);
        assert_eq!((off.player.x, off.player.y), (-1000.0, -1000.0));
    }

    #[test]
    fn a_tile_window_builds_only_its_tiles() {
        let project = starter();
        let content = farm_sim::state::create_content_from_project(&project);
        let state = farm_sim::state::create_game_state(&project, None);
        let scene = &state.world.scenes[0];
        let full = shell_snapshot(&content, &state, scene, &SnapshotOptions { tile_size: 32.0, ..Default::default() });
        let window = TileWindow { x0: 2, y0: 1, x1: 4, y1: 3 };
        let options = SnapshotOptions { tile_size: 32.0, tile_window: Some(window), ..Default::default() };
        let part = shell_snapshot(&content, &state, scene, &options);
        assert_eq!(part.tile_window, Some(window));
        assert_eq!(part.tiles.len(), full.tiles.len(), "rows keep their indices");
        assert!(part.tiles[0].is_empty() && part.tiles[4].is_empty());
        assert_eq!(part.tiles[2].len(), 5, "stand-ins left of the window, nothing right of it");
        assert_eq!(part.tiles[2][0].background, "");
        for y in 1..=3 {
            for x in 2..=4 {
                assert_eq!(part.tiles[y][x], full.tiles[y][x], "tile {x},{y}");
            }
        }
        // A window past the scene's edge stops at it.
        let wide = TileWindow { x0: 0, y0: 0, x1: 10_000, y1: 10_000 };
        let options = SnapshotOptions { tile_size: 32.0, tile_window: Some(wide), ..Default::default() };
        let all = shell_snapshot(&content, &state, scene, &options);
        assert_eq!(all.tiles, full.tiles);
    }

    #[test]
    fn camera_windows_cover_the_view_and_a_margin() {
        let camera = SnapshotCamera { x: 100.0, y: 40.0, width: 320.0, height: 200.0 };
        let window = TileWindow::for_camera(&camera, 12.0, 32.0, 2);
        // Columns: (100 - 12) / 32 = 2.75 → 2 and (420 - 12) / 32 = 12.75 → 13, then 2 more
        // each way. Rows: 0 and (240 - 12) / 32 = 7.1 → 8, one more for tall objects, 2 more.
        assert_eq!(window, TileWindow { x0: 0, y0: 0, x1: 15, y1: 11 });
        let far = SnapshotCamera { x: 3200.0, y: 3200.0, width: 64.0, height: 64.0 };
        assert_eq!(TileWindow::for_camera(&far, 0.0, 32.0, 1), TileWindow { x0: 99, y0: 99, x1: 103, y1: 104 });
    }

    #[test]
    fn the_atmosphere_carries_the_games_look_day_length_and_overlay() {
        use farm_sim::schema::{
            AtmosphereLook, AtmosphereSettings, DaylightKey, WeatherOverlayDefinition, WeatherTypeDefinition,
        };
        let project = starter();
        let mut content = farm_sim::state::create_content_from_project(&project);
        let mut state = farm_sim::state::create_game_state(&project, None);
        let mut scene = state.world.scenes[0].clone();
        let key = |minute: f64, color: &str| DaylightKey { minute, color: color.into() };
        let look = |keys: Vec<DaylightKey>, foliage: Option<&str>| AtmosphereLook {
            daylight: keys,
            foliage_tint: foliage.map(str::to_owned),
        };
        let mut settings =
            AtmosphereSettings { all_seasons: look(vec![key(60.0, "#ffffff")], Some("#aa8866")), ..Default::default() };
        settings.seasons.insert("winter".to_owned(), look(vec![key(0.0, "#000000")], None));
        content.settings.atmosphere = Some(settings);
        content.settings.time.day_minutes = Some(900);
        content.weather.overlays = Some(vec![WeatherOverlayDefinition {
            id: "sand".into(),
            style: "dust".into(),
            ..WeatherOverlayDefinition::default()
        }]);
        content.weather.types.push(WeatherTypeDefinition {
            id: "sandstorm".into(),
            overlay: Some("sand".into()),
            ..WeatherTypeDefinition::default()
        });
        state.clock.season = "winter".into();
        state.clock.weather_id = "sandstorm".into();
        let atmosphere = |state: &GameState, scene: &Scene| {
            shell_snapshot(&content, state, scene, &SnapshotOptions::default()).atmosphere.unwrap()
        };
        let winter = atmosphere(&state, &scene);
        assert_eq!(winter.daylight, Some(vec![key(0.0, "#000000")]), "the season's keys");
        assert_eq!(winter.foliage_tint.as_deref(), Some("#aa8866"), "the game's tint");
        assert_eq!(winter.day_minutes, Some(900.0));
        assert_eq!(winter.weather_overlay.as_deref(), Some("sand"));
        assert_eq!(winter.overlay.as_ref().map(|overlay| overlay.style.as_str()), Some("dust"));
        scene.atmosphere = Some(look(vec![key(5.0, "#123456")], None));
        assert_eq!(atmosphere(&state, &scene).daylight, Some(vec![key(5.0, "#123456")]), "the scene's keys");
        state.clock.season = "spring".into();
        scene.atmosphere = None;
        assert_eq!(atmosphere(&state, &scene).daylight, Some(vec![key(60.0, "#ffffff")]), "the game's keys");
        state.clock.weather_id = "sun".into();
        assert_eq!(atmosphere(&state, &scene).overlay, None);
        state.clock.weather_id = "rain".into();
        assert_eq!(atmosphere(&state, &scene).overlay.map(|overlay| overlay.id), Some("rain".to_owned()));
    }

    #[test]
    fn the_sky_preview_draws_the_tint_and_overlay_play_draws() {
        use crate::draw::DrawCmd;
        use farm_sim::schema::{AtmosphereLook, AtmosphereSettings, DaylightKey, WeatherOverlayDefinition};
        let mut project = starter();
        // The game's own look: a winter curve, an autumn foliage tint and an acid rain.
        let mut settings = AtmosphereSettings::default();
        settings.seasons.insert(
            "winter".to_owned(),
            AtmosphereLook { daylight: vec![DaylightKey { minute: 0.0, color: "#405080".into() }], foliage_tint: None },
        );
        settings
            .seasons
            .insert("fall".to_owned(), AtmosphereLook { daylight: Vec::new(), foliage_tint: Some("#c08040".into()) });
        project.settings.atmosphere = Some(settings);
        project.weather.overlays = Some(vec![WeatherOverlayDefinition {
            id: "acid".into(),
            style: "rain".into(),
            color: "#80ff80".into(),
            tint: Some("#c0e0a0".into()),
            ..WeatherOverlayDefinition::default()
        }]);
        project.weather.types[0].overlay = Some("acid".into());
        let acid = project.weather.types[0].id.clone();
        let content = farm_sim::state::create_content_from_project(&project);
        let mut state = farm_sim::state::create_game_state(&project, None);
        let scene_id = state.player.scene_id.clone();
        // The tint of a frame, and whether its weather draws streaks (rain) or flakes (snow):
        // more lines or circles than the same frame without an atmosphere.
        let sky_of = |snapshot: &WorldSnapshot| {
            let count = |snapshot: &WorldSnapshot, circles: bool| {
                let list = crate::build_world(snapshot, None, &mut crate::ImageStore::default());
                list.commands
                    .iter()
                    .filter(|command| match command {
                        DrawCmd::FillCircle { .. } => circles,
                        DrawCmd::Line { .. } => !circles,
                        _ => false,
                    })
                    .count()
            };
            let list = crate::build_world(snapshot, None, &mut crate::ImageStore::default());
            let tint = list.commands.iter().find_map(|command| match command {
                DrawCmd::ModulateRect { color, .. } => Some(*color),
                _ => None,
            });
            let bare = WorldSnapshot { atmosphere: None, ..snapshot.clone() };
            let rain = count(snapshot, false) > count(&bare, false);
            let snow = count(snapshot, true) > count(&bare, true);
            (tint, rain, snow)
        };

        for (minutes, season, weather) in [
            (420.0, "spring", "rain"),
            (720.0, "summer", "sun"),
            (1120.0, "fall", acid.as_str()),
            (1320.0, "winter", "snow"),
        ] {
            state.clock.time_minutes = units::time_of_day(minutes);
            state.clock.season = season.into();
            state.clock.weather_id = weather.into();
            let play_scene = state.world.scenes.iter().find(|scene| scene.id == scene_id).unwrap();
            let play = shell_snapshot(&content, &state, play_scene, &SnapshotOptions::default());
            let sky = SkyPreview { time_minutes: minutes, season: season.into(), weather_id: Some(weather.into()) };
            let scene = project.scenes.iter().find(|scene| scene.id == scene_id).unwrap();
            let mut edit = editor_snapshot(&project, &content, scene, 28.0, 12.0);
            assert_eq!(sky_of(&edit), (None, false, false), "no sky without the preview");
            edit.atmosphere = Some(preview_atmosphere(&project, &content, scene, &sky));
            assert_eq!(edit.atmosphere, play.atmosphere, "{minutes} {season} {weather}");
            let (_, rain, snow) = sky_of(&edit);
            assert_eq!((rain, snow), (weather == "rain" || weather == acid, weather == "snow"), "{weather}");
            assert_eq!(sky_of(&edit), sky_of(&play), "{minutes} {season} {weather}");
        }

        // Indoors, neither draws the weather; the day/night tint stays.
        state.clock.time_minutes = units::time_of_day(1320.0);
        state.clock.weather_id = "rain".into();
        let mut indoor = project.scenes.iter().find(|scene| scene.id == scene_id).unwrap().clone();
        indoor.indoor = Some(true);
        let sky = SkyPreview { time_minutes: 1320.0, season: "winter".into(), weather_id: Some("rain".into()) };
        let preview = preview_atmosphere(&project, &content, &indoor, &sky);
        assert_eq!(preview, shell_snapshot(&content, &state, &indoor, &SnapshotOptions::default()).atmosphere.unwrap());
        assert_eq!((preview.weather_id, preview.overlay), (None, None));
        let mut edit = editor_snapshot(&project, &content, &indoor, 28.0, 12.0);
        edit.atmosphere = Some(preview_atmosphere(&project, &content, &indoor, &sky));
        let (tint, rain, snow) = sky_of(&edit);
        assert!(tint.is_some() && !rain && !snow);

        // A scene's own look wins over the season's, and no season means the starting one.
        indoor.atmosphere = Some(AtmosphereLook {
            daylight: vec![DaylightKey { minute: 0.0, color: "#102030".into() }],
            foliage_tint: None,
        });
        let own = preview_atmosphere(&project, &content, &indoor, &sky);
        assert_eq!(own.daylight.unwrap()[0].color, "#102030");
        let start = preview_atmosphere(&project, &content, &indoor, &SkyPreview::default());
        assert_eq!(start.season.as_deref(), Some(project.current_season.as_str()));
    }

    #[test]
    fn missing_cells_fall_back_to_grass_and_animals_face_a_stable_direction() {
        let project = starter();
        let content = farm_sim::state::create_content_from_project(&project);
        let mut state = farm_sim::state::create_game_state(&project, None);
        let scene_id = state.world.scenes[0].id.clone();
        state.world.scenes[0].tiles.pop();
        state.animals.push(farm_sim::schema::AnimalState {
            id: "cow-\u{1F404}".into(),
            species_id: "missing-species".into(),
            scene_id,
            x: 2,
            y: 3,
            ..Default::default()
        });
        let view = shell_snapshot(&content, &state, &state.world.scenes[0], &SnapshotOptions::default());
        assert_eq!(view.tiles.last().unwrap()[0].background, "grass");
        let cow = view.npcs.last().unwrap();
        assert_eq!(cow.kind, "animal");
        assert_eq!(cow.color, None);
        // Length 6 in UTF-16 (the emoji is a surrogate pair), position sum 5.
        let expected = directions::ALL[(BuiltinArt::hash(6, 5) % 4) as usize];
        assert_eq!(cow.direction, expected);
    }
}
