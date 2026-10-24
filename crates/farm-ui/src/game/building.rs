//! A builder's shop (M5): the Build tab lists the buildings it puts up and the upgrades of the
//! buildings standing; the Animals tab sells animals into the buildings that house them. A new
//! building is placed on a small top-down map of its scene first: the footprint follows the
//! arrow keys, the D-pad or the pointer, green where `farm_sim::overlay::site_status` lets it go
//! and red with the reason where it doesn't; accepting sends `orderBuilding`.
//!
//! The shop stays open the whole time (the placement map is drawn in its place), because the
//! engine takes `orderBuilding` only from an open builder's shop; cancelling goes back to it.

use super::shop::two_lines;
use super::{item_name, right_aligned, row, GameAction, GameView};
use crate::format::num;
use crate::i18n::Lang;
use crate::icons::Icon;
use crate::input::{InputDevice, NavAction};
use crate::layout::{Align, RectExt};
use crate::theme::{fade, mix};
use crate::ui::{HitKind, Ui, WidgetId};
use crate::widgets::{Button, ButtonKind, ModalSpec};
use farm_render::{Color, FontId, Rect, TextAlign};
use farm_sim::overlay::{self, BuildOption};
use farm_sim::schema::BuildingDefinition;
use farm_sim::{units, Command};

const ROW_GAP: f32 = 8.0;
/// The placement map's cells, in interface pixels: at most, and at least (a larger scene shows
/// the part around the footprint).
const MAX_CELL: f32 = 24.0;
const MIN_CELL: f32 = 12.0;
/// How far from its start a new placement looks for a free site, in tiles.
const SEARCH_RADIUS: i32 = 12;

/// A building being placed: its kind, the scene and the footprint's top-left tile.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    pub building_id: String,
    pub scene_id: String,
    pub x: i32,
    pub y: i32,
    /// Where the pointer was last frame (`None` before the first): the footprint follows the
    /// pointer only when it moves, so the keys can move it under a resting pointer and the map
    /// opening under the pointer leaves it where it starts.
    pointer: Option<Option<(f32, f32)>>,
}

impl Placement {
    /// Starts placing `option` in its first scene (the player's, when it may go there), on the
    /// free site nearest the player (or the scene's middle), else right there.
    pub fn new(view: &GameView<'_>, option: &BuildOption) -> Option<Self> {
        let state = view.state;
        let scene_id =
            option.scenes.iter().find(|id| **id == state.player.scene_id).or_else(|| option.scenes.first())?.clone();
        let mut placement = Self { building_id: option.building_id.clone(), scene_id, x: 0, y: 0, pointer: None };
        placement.start_in_scene(view, option);
        Some(placement)
    }

    /// Puts the footprint on the free site nearest the player (or the scene's middle).
    fn start_in_scene(&mut self, view: &GameView<'_>, option: &BuildOption) {
        let state = view.state;
        let (width, height) = scene_size(view, &self.scene_id);
        let (cx, cy) = if state.player.scene_id == self.scene_id {
            (units::tile_of(state.player.x), units::tile_of(state.player.y))
        } else {
            (width / 2, height / 2)
        };
        let (x0, y0) = (cx - option.width / 2, cy - option.height / 2);
        let free =
            |x: i32, y: i32| overlay::site_status(view.ctx, state, &self.building_id, &self.scene_id, x, y).is_none();
        let mut found = None;
        'search: for radius in 0..=SEARCH_RADIUS {
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs().max(dy.abs()) == radius && free(x0 + dx, y0 + dy) {
                        found = Some((x0 + dx, y0 + dy));
                        break 'search;
                    }
                }
            }
        }
        let (x, y) = found.unwrap_or((x0, y0));
        self.x = x.clamp(0, (width - option.width).max(0));
        self.y = y.clamp(0, (height - option.height).max(0));
    }

    /// Moves the footprint by whole tiles, keeping it on the map.
    fn nudge(&mut self, dx: i32, dy: i32, scene: (i32, i32), size: (i32, i32)) {
        self.x = (self.x + dx).clamp(0, (scene.0 - size.0).max(0));
        self.y = (self.y + dy).clamp(0, (scene.1 - size.1).max(0));
    }
}

/// A scene's size in tiles (the live world's, else the content's).
fn scene_size(view: &GameView<'_>, scene_id: &str) -> (i32, i32) {
    farm_sim::world::world_movement::find_scene(view.state, scene_id)
        .or_else(|| view.ctx.content_scene(scene_id))
        .map_or((1, 1), |scene| (scene.width.max(1), scene.height.max(1)))
}

/// A scene's name (its id when unknown).
fn scene_name(view: &GameView<'_>, scene_id: &str) -> String {
    farm_sim::world::world_movement::find_scene(view.state, scene_id)
        .or_else(|| view.ctx.content_scene(scene_id))
        .map_or_else(|| scene_id.to_owned(), |scene| scene.name.clone())
}

/// The building kind `id`.
fn definition<'a>(view: &GameView<'a>, id: &str) -> Option<&'a BuildingDefinition> {
    view.content.building_types.iter().find(|definition| definition.id == id)
}

/// What a building costs: money and materials, and the nights of building.
fn cost_line(view: &GameView<'_>, lang: Lang, option: &BuildOption) -> String {
    let mut parts = vec![view.money(lang, option.cost)];
    for material in &option.materials {
        parts.push(format!("{}\u{00d7} {}", num(material.quantity), item_name(view.content, &material.item_id)));
    }
    parts.push(lang.format("build.days", &[&num(option.build_days)]));
    parts.join(" \u{00b7} ")
}

/// The Build tab's rows from `top`; returns where they end.
pub(crate) fn build_rows(
    ui: &mut Ui,
    view: &GameView<'_>,
    area: Rect,
    top: f32,
    placement: &mut Option<Placement>,
    actions: &mut Vec<GameAction>,
) -> f32 {
    let lang = ui.lang();
    let mut y = top;
    let small = ui.button_height(12.5);
    let row_height = (ui.line_height(14.0) + ui.line_height(12.5) + 16.0).max(small + 16.0);
    for option in &view.overlay.building_options {
        let button = match option.upgrades {
            Some(_) => Button::new(lang.tr("build.upgrade")),
            None => Button::new(lang.tr("build.place")),
        }
        .primary()
        .size(12.5)
        .enabled(option.affordable);
        let width = ui.button_width(&button).max(72.0);
        let (content, buttons) = row(ui, area, y, row_height, width);
        let title = match option.upgrades.as_deref() {
            Some(standing) => {
                let from = view
                    .state
                    .buildings
                    .iter()
                    .find(|building| building.id == standing)
                    .and_then(|building| definition(view, &building.building_id))
                    .map_or(standing, |definition| definition.name.as_str());
                lang.format("build.upgradeOf", &[&option.name, &from])
            }
            None => option.name.clone(),
        };
        let size = format!("{}\u{00d7}{}", num(option.width), num(option.height));
        let mut detail = cost_line(view, lang, option);
        if !option.affordable {
            detail = lang.format("build.cantAfford", &[&detail]);
        }
        two_lines(ui, content, &title, Some(&size), &detail, !option.affordable);
        let rect = right_aligned(buttons, &[width], small, 6.0)[0];
        let mut id = WidgetId::new("shop-build").with(&option.building_id);
        if let Some(standing) = &option.upgrades {
            id = id.with(standing);
        }
        if ui.button(id, rect, button) {
            match &option.upgrades {
                Some(standing) => actions.push(GameAction::Command(Command::UpgradeBuilding {
                    id: standing.clone(),
                    building_id: option.building_id.clone(),
                })),
                None => *placement = Placement::new(view, option),
            }
        }
        y += row_height + ROW_GAP;
    }
    y
}

/// The Animals tab's rows from `top`: one per animal and home with room; returns where they end.
pub(crate) fn animal_rows(
    ui: &mut Ui,
    view: &GameView<'_>,
    area: Rect,
    top: f32,
    actions: &mut Vec<GameAction>,
) -> f32 {
    let lang = ui.lang();
    let money = view.state.player.money;
    let mut y = top;
    let small = ui.button_height(12.5);
    let row_height = (ui.line_height(14.0) + ui.line_height(12.5) + 16.0).max(small + 16.0);
    for option in &view.overlay.animal_options {
        let price = view.money(lang, option.cost);
        if option.homes.is_empty() {
            let button = Button::new(lang.tr("shop.buy")).primary().size(12.5).enabled(false);
            let width = ui.button_width(&button).max(52.0);
            let (content, buttons) = row(ui, area, y, row_height, width);
            two_lines(ui, content, &option.name, Some(&price), lang.tr("animals.noHome"), true);
            let rect = right_aligned(buttons, &[width], small, 6.0)[0];
            ui.button(WidgetId::new("shop-animal").with(&option.species_id), rect, button);
            y += row_height + ROW_GAP;
            continue;
        }
        for (home, home_name, living, capacity) in &option.homes {
            let room = living < capacity;
            let button = Button::new(lang.tr("shop.buy")).primary().size(12.5).enabled(room && money >= option.cost);
            let width = ui.button_width(&button).max(52.0);
            let (content, buttons) = row(ui, area, y, row_height, width);
            let detail = if room {
                lang.format("animals.home", &[home_name, &num(*living), &num(*capacity)])
            } else {
                lang.format("animals.homeFull", &[home_name])
            };
            two_lines(ui, content, &option.name, Some(&price), &detail, !room);
            let rect = right_aligned(buttons, &[width], small, 6.0)[0];
            if ui.button(WidgetId::new("shop-animal").with(&option.species_id).with(home), rect, button) {
                actions.push(GameAction::Command(Command::BuyAnimal {
                    species_id: option.species_id.clone(),
                    building: home.clone(),
                    name: String::new(),
                }));
            }
            y += row_height + ROW_GAP;
        }
    }
    y
}

/// The colour of a terrain on the map: its definition's, else the built-in one.
fn terrain_color(view: &GameView<'_>, id: &str) -> Color {
    farm_sim::vocabulary::terrains(view.content)
        .iter()
        .find(|terrain| terrain.id == id)
        .and_then(|terrain| Color::parse(&terrain.color))
        .unwrap_or_else(|| farm_render::canvas2d::tile_color(Some(id), "grass"))
}

/// The keys of the placement map on the current device.
fn controls_hint(device: InputDevice, lang: Lang) -> &'static str {
    lang.tr(match device {
        InputDevice::Gamepad => "build.hintGamepad",
        InputDevice::Keyboard => "build.hintKeys",
        InputDevice::Mouse | InputDevice::Touch => "build.hintPointer",
    })
}

/// Draws the placement map of `placement` over the builder's shop; false once it is done
/// (ordered or cancelled), when the shop shows again.
pub(crate) fn place(
    ui: &mut Ui,
    view: &GameView<'_>,
    placement: &mut Placement,
    actions: &mut Vec<GameAction>,
) -> bool {
    let Some(option) =
        view.overlay.building_options.iter().find(|o| o.building_id == placement.building_id && o.upgrades.is_none())
    else {
        return false;
    };
    let colors = ui.theme().colors;
    let lang = ui.lang();
    let size = (option.width, option.height);

    // Keys first: the map below draws where they put the footprint.
    let mut order = false;
    let mut cancel = false;
    if view.keys_active {
        let scene = scene_size(view, &placement.scene_id);
        for nav in &ui.input().nav.clone() {
            match nav {
                NavAction::Up => placement.nudge(0, -1, scene, size),
                NavAction::Down => placement.nudge(0, 1, scene, size),
                NavAction::Left => placement.nudge(-1, 0, scene, size),
                NavAction::Right => placement.nudge(1, 0, scene, size),
                NavAction::Accept => order = true,
                NavAction::Back => cancel = true,
                NavAction::TabNext | NavAction::TabPrev => {}
            }
        }
        if ui.tab_delta() != 0 && option.scenes.len() > 1 {
            let current = option.scenes.iter().position(|id| *id == placement.scene_id).unwrap_or(0) as i32;
            let next = (current + ui.tab_delta()).rem_euclid(option.scenes.len() as i32) as usize;
            placement.scene_id.clone_from(&option.scenes[next]);
            placement.start_in_scene(view, option);
        }
    }

    let footer_height = ui.button_height(13.0) + 20.0;
    let title = lang.format("build.placeTitle", &[&option.name]);
    let subtitle = scene_name(view, &placement.scene_id);
    let modal = ui.begin_modal(ModalSpec {
        id: WidgetId::new("placement"),
        icon: Icon::Hammer,
        title: &title,
        subtitle: Some(&subtitle),
        width: 780.0,
        max_height: 720.0,
        footer: footer_height,
        backdrop: true,
    });
    cancel |= modal.close;
    let area = modal.body;
    let mut y = modal.top;

    // Scenes it may go in, as tabs (Tab / RB switch).
    if option.scenes.len() > 1 {
        let tab_height = ui.button_height(13.0);
        let mut x = area.x;
        for scene_id in &option.scenes {
            let name = scene_name(view, scene_id);
            let selected = *scene_id == placement.scene_id;
            let button = Button::new(&name).kind(ButtonKind::Tab { selected }).focusable(false);
            let width = ui.button_width(&button).max(72.0);
            if ui.button(
                WidgetId::new("placement-scene").with(scene_id.as_str()),
                Rect::new(x, y, width, tab_height),
                button,
            ) && !selected
            {
                placement.scene_id.clone_from(scene_id);
                placement.start_in_scene(view, option);
            }
            x += width + 6.0;
        }
        y += tab_height + 10.0;
    }

    // The map: as much of the scene as fits, around the footprint.
    let (scene_width, scene_height) = scene_size(view, &placement.scene_id);
    let screen = ui.screen();
    let map_space = Rect::new(area.x, y, area.width, (screen.height - 330.0).clamp(160.0, 430.0));
    let fit = (map_space.width / scene_width as f32).min(map_space.height / scene_height as f32).floor();
    let cell = fit.clamp(MIN_CELL, MAX_CELL);
    let columns = ((map_space.width / cell).floor() as i32).min(scene_width).max(1);
    let rows = ((map_space.height / cell).floor() as i32).min(scene_height).max(1);
    let x0 = (placement.x + size.0 / 2 - columns / 2).clamp(0, (scene_width - columns).max(0));
    let y0 = (placement.y + size.1 / 2 - rows / 2).clamp(0, (scene_height - rows).max(0));
    let map = Rect::new(
        map_space.x + (map_space.width - columns as f32 * cell) / 2.0,
        map_space.y,
        columns as f32 * cell,
        rows as f32 * cell,
    );
    let cell_rect = |x: i32, y: i32, w: i32, h: i32| {
        Rect::new(map.x + (x - x0) as f32 * cell, map.y + (y - y0) as f32 * cell, w as f32 * cell, h as f32 * cell)
    };
    ui.list_mut().fill_round_rect(map.inset(-4.0), 8.0, colors.row);
    ui.push_clip(map);
    if let Some(site) = overlay::site_map(view.ctx, view.state, &placement.scene_id, (x0, y0, columns, rows)) {
        let mut palette: Vec<(&str, Color)> = Vec::new();
        for (index, tile) in site.tiles.iter().enumerate() {
            let (tx, ty) = (site.x0 + index as i32 % site.columns.max(1), site.y0 + index as i32 / site.columns.max(1));
            let base = match palette.iter().find(|(id, _)| *id == tile.terrain) {
                Some((_, color)) => *color,
                None => {
                    let color = terrain_color(view, &tile.terrain);
                    palette.push((&tile.terrain, color));
                    color
                }
            };
            // Ground a building can't cover is darker.
            let color = if tile.free { base } else { mix(base, Color::rgb(0, 0, 0), 0.5) };
            ui.list_mut().fill_rect(cell_rect(tx, ty, 1, 1), color);
        }
    }
    // The buildings standing (or going up) here.
    for building in view.state.buildings.iter().filter(|building| building.scene_id == placement.scene_id) {
        let Some(kind) = definition(view, &building.building_id) else { continue };
        let (w, h) = kind.size();
        let rect = cell_rect(building.x, building.y, w, h).inset(1.0);
        let color = Color::parse(&kind.color).unwrap_or(colors.secondary);
        ui.list_mut().fill_round_rect(rect, 3.0, fade(color, if building.built() { 0.9 } else { 0.5 }));
        ui.list_mut().stroke_round_rect(rect, 3.0, fade(Color::rgb(0, 0, 0), 0.5), 1.0);
    }
    // The player, where they stand.
    if view.state.player.scene_id == placement.scene_id {
        let (px, py) = (units::tile_of(view.state.player.x), units::tile_of(view.state.player.y));
        let (cx, cy) = cell_rect(px, py, 1, 1).center();
        ui.list_mut().fill_circle(cx, cy, cell * 0.3, colors.accent);
    }
    // The pointer moves the footprint to the tile under it; a click builds there.
    let hit = ui.interact(WidgetId::new("placement-map"), map, HitKind::Button, false);
    let pointer = ui.pointer();
    if let Some((px, py)) = pointer.filter(|_| hit.hovered) {
        let moved = placement.pointer.is_some_and(|last| last != pointer);
        if moved || hit.pressed {
            let tx = x0 + ((px - map.x) / cell).floor() as i32;
            let ty = y0 + ((py - map.y) / cell).floor() as i32;
            placement.x = (tx - size.0 / 2).clamp(0, (scene_width - size.0).max(0));
            placement.y = (ty - size.1 / 2).clamp(0, (scene_height - size.1).max(0));
        }
    }
    placement.pointer = Some(pointer);
    order |= hit.clicked;
    let status = overlay::site_status(
        view.ctx,
        view.state,
        &placement.building_id,
        &placement.scene_id,
        placement.x,
        placement.y,
    );
    let fits = status.is_none();
    let ghost_color = if fits { colors.primary } else { colors.error };
    let ghost = cell_rect(placement.x, placement.y, size.0, size.1);
    ui.list_mut().fill_round_rect(ghost, 3.0, fade(ghost_color, 0.45));
    ui.list_mut().stroke_round_rect(ghost.inset(1.0), 3.0, ghost_color, 2.0);
    if let Some(kind) = definition(view, &placement.building_id) {
        // The door, where it will be.
        let (dx, dy) = kind.door_offset();
        let door = cell_rect(placement.x + dx, placement.y + dy, 1, 1);
        ui.list_mut().fill_rect(door.inset_xy(cell * 0.3, cell * 0.15), fade(Color::rgb(0, 0, 0), 0.55));
    }
    ui.pop_clip();
    y = map.bottom() + 14.0;

    // Whether it may go there, and why not.
    let (text, color) = match &status {
        None if option.affordable => (lang.tr("build.siteFree").to_owned(), colors.primary),
        None => (lang.tr("build.cantAffordShort").to_owned(), colors.error),
        Some(reason) => (lang.message(reason), colors.error),
    };
    y += ui.paragraph(area.x, y, area.width, &text, 13.5, FontId::Bold, color, TextAlign::Left) + 4.0;
    let cost = cost_line(view, lang, option);
    y += ui.paragraph(area.x, y, area.width, &cost, 12.5, FontId::Regular, colors.muted, TextAlign::Left) + 2.0;
    let hint = controls_hint(ui.device(), lang);
    y += ui.paragraph(area.x, y, area.width, hint, 12.0, FontId::Regular, colors.muted, TextAlign::Left);
    ui.end_modal_body(y);

    let ready = fits && option.affordable;
    if let Some(footer) = modal.footer {
        let mut footer = footer;
        let build = Button::new(lang.tr("build.buildHere")).primary().enabled(ready).focusable(false);
        let back = Button::new(lang.tr("common.cancel")).focusable(false);
        let build_width = ui.button_width(&build);
        let back_width = ui.button_width(&back);
        let height = ui.button_height(13.0);
        let build_rect = footer.cut_right(build_width).centered(build_width, height);
        footer.cut_right(8.0);
        let back_rect = footer.cut_right(back_width).centered(back_width, height);
        let coordinates = lang.format("build.at", &[&num(placement.x), &num(placement.y)]);
        ui.label(footer, &coordinates, 12.5, FontId::Regular, colors.muted, Align::Start);
        order |= ui.button(WidgetId::new("placement-build"), build_rect, build);
        cancel |= ui.button(WidgetId::new("placement-cancel"), back_rect, back);
    }
    ui.close_modal();

    if cancel {
        return false;
    }
    if order && ready {
        actions.push(GameAction::Command(Command::OrderBuilding {
            building_id: placement.building_id.clone(),
            scene_id: placement.scene_id.clone(),
            x: placement.x,
            y: placement.y,
        }));
        return false;
    }
    true
}
