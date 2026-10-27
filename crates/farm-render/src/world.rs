//! Port of `SkiaWorldRenderer.RenderCore`: turns a [`WorldSnapshot`] into a [`DrawList`] with the
//! same passes, ordering and geometry as the Skia reference:
//!
//! 1. ground: tile background, overlay and object layers, soil states, ladders;
//! 2. soft drop shadows under entities and objects (built-in art only);
//! 3. y-sorted objects and entities (nodes, machines, crops, items, buildings, NPCs, animals,
//!    monsters, the player) so tall things overlap whoever stands behind them; ties keep
//!    insertion order;
//! 4. atmosphere: day/night tint (modulate), seasonal foliage, the weather overlay;
//! 5. floating pops and the Edit Mode grid.
//!
//! Art per layer or entity: creator-bound sprite or image → built-in art → the original colored
//! shapes. Tile colours and which ground is farmable come from the snapshot's terrains (M1), the
//! built-in palette for anything they don't name. Coordinates are world pixels at 1×; hosts scale when rasterizing. Geometry is computed
//! in `f64` and converted to `f32` exactly where the C# renderer converts to `float`.

use crate::atmosphere::{self, WeatherLook, WeatherOverlay};
use crate::builtin_art::{BuiltinArt, BuiltinArtEntry};
use crate::canvas2d::{self, entity_pixel_origin};
use crate::css_color::{self, Color};
use crate::draw::{DrawCmd, DrawList, FontId, Rect, Sampling, TextAlign, TextStroke};
use crate::images::{ImageId, ImageStore};
use crate::num::{cs_clamp, cs_compare, cs_max, cs_min, round_half_even, to_byte, to_int};
use crate::snapshot::{
    SnapshotEmote, SnapshotEntity, SnapshotSprite, SnapshotTerrain, SnapshotTile, WorldSnapshot, MAX_TALL,
};
use std::sync::Arc;

const BACKGROUND_TINT: Color = Color::rgba(0x1a, 0x1a, 0x2e, 0x10);
const SHADOW_COLOR: Color = Color::rgba(0, 0, 0, 0x48);

/// Builds the draw list of a world frame. `art` is the built-in pack for anything the project
/// binds no art to (`None` draws the original colored shapes only); `images` decodes and caches
/// every image the frame uses (their sizes decide which sprites are drawable).
pub fn build_world(snapshot: &WorldSnapshot, art: Option<&BuiltinArt>, images: &mut ImageStore) -> DrawList {
    let mut list = DrawList::new();
    build_world_into(&mut list, snapshot, art, images);
    list
}

/// [`build_world`] appending to an existing list. The world is wrapped in one save/restore pair,
/// so whatever follows draws in the list's original coordinate space.
pub fn build_world_into(
    list: &mut DrawList,
    snapshot: &WorldSnapshot,
    art: Option<&BuiltinArt>,
    images: &mut ImageStore,
) {
    images.begin_frame();
    let mut builder = Builder::new(list, snapshot, art, images);
    builder.world();
}

/// The glyph and colour of a cutscene emote (`farm_sim::schema::emotes::ALL`); a symbol the
/// font lacks falls back to plain text, an unknown emote to an ellipsis.
fn emote_glyph(emote: &str) -> (&'static str, &'static str) {
    let (glyph, fallback, color) = match emote {
        "exclaim" => ("!", "!", "#d64545"),
        "question" => ("?", "?", "#3b6fd6"),
        "heart" => ("\u{2665}", "<3", "#e0457b"),
        "music" => ("\u{266a}", "~", "#7a4fd6"),
        "anger" => ("\u{203c}", "!!", "#b02020"),
        "sweat" => ("\u{2026}", "...", "#3f9fd0"),
        "sleep" => ("Zz", "Zz", "#5a6b8c"),
        "idea" => ("\u{2605}", "*", "#e0a400"),
        _ => ("\u{2026}", "...", "#555555"),
    };
    let glyph = if crate::text::font(FontId::Bold).covers(glyph) { glyph } else { fallback };
    (glyph, color)
}

/// Trees and weeds take the seasonal foliage tint; rocks and ores don't.
fn is_foliage(entry: Option<&BuiltinArtEntry>) -> bool {
    entry.is_some_and(|entry| entry.name.starts_with("node-tree") || entry.name.starts_with("node-weeds"))
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect::new(x as f32, y as f32, width as f32, height as f32)
}

fn parse(css: &str) -> Color {
    css_color::parse(Some(css))
}

/// A snapshot terrain with its colour parsed once per frame (`None`: the built-in colour).
struct Terrain<'a> {
    id: &'a str,
    color: Option<Color>,
    plantable: bool,
    /// Joins up with its neighbours (M8).
    autotile: bool,
    /// Tiles it rises above its own on the object layer (M8).
    tall: u32,
}

impl<'a> Terrain<'a> {
    fn new(terrain: &'a SnapshotTerrain) -> Self {
        Self {
            id: &terrain.id,
            color: css_color::try_parse(&terrain.color),
            plantable: terrain.plantable,
            autotile: terrain.autotile,
            tall: terrain.tall.min(MAX_TALL),
        }
    }
}

/// Which layer of a tile an autotile edge is drawn for.
#[derive(Debug, Clone, Copy)]
enum EdgeLayer<'a> {
    Background,
    Overlay,
    Object,
    Decor(&'a str),
}

impl EdgeLayer<'_> {
    fn of(self, tile: &SnapshotTile) -> Option<&str> {
        match self {
            Self::Background => Some(&tile.background),
            Self::Overlay => tile.overlay.as_deref(),
            Self::Object => tile.object.as_deref(),
            Self::Decor(terrain) => tile.decor.iter().find(|d| d.terrain == terrain).map(|d| d.terrain.as_str()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DrawKind {
    Node,
    Machine,
    Crop,
    Item,
    Entity,
    Player,
    /// A building (M5): sorted by its footprint's bottom edge.
    Building,
    /// A monster (M7).
    Monster,
    /// An object-layer terrain that rises above its tile (M8), y-sorted like a node.
    Tall,
}

/// One thing to draw in the y-sorted pass.
struct Drawable<'a> {
    sort_y: f64,
    seq: usize,
    kind: DrawKind,
    tile: Option<&'a SnapshotTile>,
    entity: Option<&'a SnapshotEntity>,
    tile_x: i32,
    tile_y: i32,
    px: f64,
    py: f64,
    /// The building's or monster's index in the snapshot.
    index: usize,
}

struct Builder<'a, 'l> {
    list: &'l mut DrawList,
    snapshot: &'a WorldSnapshot,
    art: Option<&'a BuiltinArt>,
    images: &'l mut ImageStore,
    sampling: Sampling,
    ts: f64,
    padding: f64,
    pitch: f64,
    tick: f64,
    season: Option<&'a str>,
    foliage_tint: Option<Color>,
    art_scale: f64,
    /// The snapshot's terrains; `None`: the built-in palette.
    terrains: Option<Vec<Terrain<'a>>>,
}

impl<'a, 'l> Builder<'a, 'l> {
    fn new(
        list: &'l mut DrawList,
        snapshot: &'a WorldSnapshot,
        art: Option<&'a BuiltinArt>,
        images: &'l mut ImageStore,
    ) -> Self {
        let season = snapshot.atmosphere.as_ref().and_then(|a| a.season.as_deref());
        let foliage = snapshot.atmosphere.as_ref().and_then(|a| a.foliage_tint.as_deref());
        Self {
            list,
            snapshot,
            art,
            images,
            // Pixel art: nearest-neighbour sampling (canvas imageSmoothingEnabled = false).
            sampling: if snapshot.pixel_art == Some(false) { Sampling::Smooth } else { Sampling::Nearest },
            ts: snapshot.tile_size,
            padding: snapshot.padding,
            pitch: snapshot.pitch(),
            tick: snapshot.tick,
            season,
            foliage_tint: atmosphere::foliage_tint_in(season, foliage),
            art_scale: art.map_or(1.0, |art| snapshot.tile_size / f64::from(art.tile_size())),
            terrains: snapshot.terrains.as_ref().map(|terrains| terrains.iter().map(Terrain::new).collect()),
        }
    }

    fn terrain(&self, id: &str) -> Option<&Terrain<'a>> {
        self.terrains.as_ref()?.iter().find(|terrain| terrain.id == id)
    }

    /// The colour of a terrain: its definition's, else the built-in one (`fallback`'s for an id
    /// the built-in palette doesn't know either).
    fn terrain_color(&self, id: &str, fallback: &str) -> Color {
        match self.terrain(id).and_then(|terrain| terrain.color) {
            Some(color) => color,
            None => canvas2d::tile_color(Some(id), fallback),
        }
    }

    /// How many tiles a terrain rises above its own (M8); 0 lies flat.
    fn tall(&self, id: &str) -> u32 {
        self.terrain(id).map_or(0, |terrain| terrain.tall)
    }

    fn autotile(&self, id: &str) -> bool {
        self.terrain(id).is_some_and(|terrain| terrain.autotile)
    }

    /// Without art, an autotiled terrain (M8) shows where it meets another: a darker band on
    /// each side whose neighbour has something else on the layer.
    fn autotile_edges(&mut self, x: i32, y: i32, id: &str, layer: EdgeLayer<'_>, color: Color, tile_rect: Rect) {
        if !self.autotile(id) {
            return;
        }
        let snapshot = self.snapshot;
        let same = |dx: i32, dy: i32| {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 {
                return true;
            }
            let Some(row) = snapshot.tiles.get(ny as usize) else { return true };
            // A neighbour the snapshot did not build (outside its window) counts as the same.
            match row.get(nx as usize) {
                Some(tile) if !tile.background.is_empty() => layer.of(tile) == Some(id),
                _ => true,
            }
        };
        let band = cs_max(2.0, (self.ts / 10.0).floor()) as f32;
        let edge = Color::rgba(
            (f32::from(color.r) * 0.62) as u8,
            (f32::from(color.g) * 0.62) as u8,
            (f32::from(color.b) * 0.62) as u8,
            color.a,
        );
        let (left, top, width, height) = (tile_rect.x, tile_rect.y, tile_rect.width, tile_rect.height);
        for (dx, dy, rect) in [
            (0, -1, Rect::new(left, top, width, band)),
            (1, 0, Rect::new(left + width - band, top, band, height)),
            (0, 1, Rect::new(left, top + height - band, width, band)),
            (-1, 0, Rect::new(left, top, band, height)),
        ] {
            if !same(dx, dy) {
                self.fill_rect(rect, edge);
            }
        }
    }

    /// Whether a terrain is farmable ground: its definition says so; without terrains, `soil`.
    fn plantable(&self, id: &str) -> bool {
        match &self.terrains {
            Some(_) => self.terrain(id).is_some_and(|terrain| terrain.plantable),
            None => id == "soil",
        }
    }

    fn push(&mut self, command: DrawCmd) {
        self.list.push(command);
    }

    fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.push(DrawCmd::FillRect { rect, color, anti_alias: false });
    }

    /// The image for a source with its size, decoding it on first use.
    fn image(&mut self, source: &Arc<str>) -> Option<(ImageId, f64, f64)> {
        let id = self.images.get_shared(source)?;
        let (width, height) = self.images.size(id)?;
        Some((id, f64::from(width), f64::from(height)))
    }

    fn push_image(&mut self, id: ImageId, src: Rect, dst: Rect, opacity: f64, tint: Option<Color>) {
        // The paint alpha is a byte: (byte)Math.Round(255 * opacity).
        let alpha = to_byte(cs_clamp(round_half_even(255.0 * opacity), 0.0, 255.0));
        self.push(DrawCmd::Image {
            image: id,
            src,
            dst,
            opacity: f32::from(alpha) / 255.0,
            sampling: self.sampling,
            tint,
        });
    }

    /// Creator sprites: scaled to fit the box, bottom-aligned (TS `drawSprite`).
    fn draw_sprite(
        &mut self,
        sprite: Option<&SnapshotSprite>,
        dx: f64,
        dy: f64,
        dw: f64,
        dh: f64,
        opacity: f64,
    ) -> bool {
        let Some(sprite) = sprite else { return false };
        let Some((id, width, height)) = self.image(&sprite.image_url) else { return false };
        if width == 0.0 || height == 0.0 {
            return false;
        }
        let sw = if sprite.frame_width != 0.0 { sprite.frame_width } else { width };
        let sh = if sprite.frame_height != 0.0 { sprite.frame_height } else { height };
        let sx = sprite.source_x.unwrap_or(sprite.frame * sw);
        let sy = sprite.source_y.unwrap_or(sprite.row * sh);
        if sx < 0.0 || sy < 0.0 || sx + sw > width || sy + sh > height {
            return false;
        }
        let scale = cs_min(dw / sw, dh / sh);
        let dst = rect(dx + ((dw - (sw * scale)) / 2.0), dy + dh - (sh * scale), sw * scale, sh * scale);
        self.push_image(id, rect(sx, sy, sw, sh), dst, opacity, None);
        true
    }

    /// Built-in sprites: drawn at the pack's native scale (one sheet pixel = ts/32 world pixels),
    /// centred on the tile, standing on its bottom edge (or filling it for ground tiles).
    fn draw_builtin(
        &mut self,
        sprite: Option<SnapshotSprite>,
        px: f64,
        py: f64,
        opacity: f64,
        foliage: bool,
        top: bool,
    ) -> bool {
        let Some(sprite) = sprite else { return false };
        let Some((id, width, height)) = self.image(&sprite.image_url) else { return false };
        let (sw, sh) = (sprite.frame_width, sprite.frame_height);
        let sx = sprite.source_x.unwrap_or(0.0);
        let sy = sprite.source_y.unwrap_or(0.0);
        if sw <= 0.0 || sh <= 0.0 || sx + sw > width || sy + sh > height {
            return false;
        }
        let dw = sw * self.art_scale;
        let dh = sh * self.art_scale;
        let ts = self.ts;
        let dst = rect(px + ((ts - dw) / 2.0), if top { py } else { py + ts - dh }, dw, dh);
        let tint = if foliage { self.foliage_tint } else { None };
        self.push_image(id, rect(sx, sy, sw, sh), dst, opacity, tint);
        true
    }

    fn draw_shadow(&mut self, px: f64, py: f64, width_factor: f64) {
        let ts = self.ts;
        let rx = (ts * width_factor / 2.0) as f32;
        let ry = cs_max(1.5, ts * 0.11) as f32;
        let cx = (px + (ts / 2.0)) as f32;
        let cy = (py + ts - f64::from(ry) - 1.0) as f32;
        let sigma = cs_max(0.8, ts / 14.0) as f32;
        let bounds = Rect::new(cx - rx, cy - ry, (cx + rx) - (cx - rx), (cy + ry) - (cy - ry));
        self.push(DrawCmd::BlurOval { rect: bounds, color: SHADOW_COLOR, sigma });
    }

    fn world(&mut self) {
        let snapshot = self.snapshot;
        let art = self.art;
        let ts = self.ts;
        let padding = self.padding;
        let pitch = self.pitch;
        let tick = self.tick;
        let camera = snapshot.camera;
        let (view_width, view_height) = snapshot.viewport_size();
        let (world_width, world_height) = snapshot.world_pixel_size();

        // clearRect + faint tint over the whole canvas.
        self.push(DrawCmd::Save);
        let view = Rect::new(0.0, 0.0, view_width as f32, view_height as f32);
        self.push(DrawCmd::ClipRect { rect: view });
        self.fill_rect(view, BACKGROUND_TINT);
        if let Some(camera) = camera {
            self.push(DrawCmd::Translate { dx: -camera.x as f32, dy: -camera.y as f32 });
        }

        // Visible tile range (everything when there is no camera).
        let (x_start, x_end, y_start, y_end) = match camera {
            Some(camera) => (
                to_int(((camera.x - padding) / pitch).floor()).max(0),
                (snapshot.width - 1).min(to_int(((camera.x + camera.width - padding) / pitch).ceil())),
                to_int(((camera.y - padding) / pitch).floor()).max(0),
                (snapshot.height - 1).min(to_int(((camera.y + camera.height - padding) / pitch).ceil())),
            ),
            None => (0, snapshot.width - 1, 0, snapshot.height - 1),
        };

        self.ground(x_start, x_end, y_start, y_end);

        // ---- Collect everything that stands on the ground ------------------------------------
        // Rows below the viewport: tall objects there poke into view (one row for nodes and
        // machines, as many as the tallest terrain rises for tall objects, M8).
        let mut drawables: Vec<Drawable<'a>> = Vec::new();
        let tallest = self.terrains.as_ref().map_or(0, |terrains| terrains.iter().map(|t| t.tall).max().unwrap_or(0));
        let objects_end = (snapshot.height - 1).min(y_end.saturating_add(1 + tallest as i32));
        let mut y = y_start;
        while y <= objects_end && (y as usize) < snapshot.tiles.len() {
            let row = &snapshot.tiles[y as usize];
            let mut x = x_start;
            while x <= x_end && (x as usize) < row.len() {
                let tile = &row[x as usize];
                let px = padding + (f64::from(x) * pitch);
                let py = padding + (f64::from(y) * pitch);
                let tall = tile.object.as_deref().is_some_and(|object| self.tall(object) > 0);
                for (present, kind) in [
                    (tall, DrawKind::Tall),
                    (tile.node.is_some(), DrawKind::Node),
                    (tile.machine.is_some(), DrawKind::Machine),
                    (tile.crop.is_some(), DrawKind::Crop),
                    (tile.item.is_some(), DrawKind::Item),
                ] {
                    if present {
                        let seq = drawables.len();
                        drawables.push(Drawable {
                            sort_y: py + ts,
                            seq,
                            kind,
                            tile: Some(tile),
                            entity: None,
                            tile_x: x,
                            tile_y: y,
                            px,
                            py,
                            index: 0,
                        });
                    }
                }
                x += 1;
            }
            y += 1;
        }
        for npc in &snapshot.npcs {
            // A cutscene walks NPCs between tiles (pixel override); otherwise they stand on one.
            let px = npc.pixel_x.unwrap_or_else(|| entity_pixel_origin(npc.x, padding, pitch));
            let py = npc.pixel_y.unwrap_or_else(|| entity_pixel_origin(npc.y, padding, pitch));
            let seq = drawables.len();
            drawables.push(Drawable {
                sort_y: py + ts,
                seq,
                kind: DrawKind::Entity,
                tile: None,
                entity: Some(npc),
                tile_x: 0,
                tile_y: 0,
                px,
                py,
                index: 0,
            });
        }
        for (index, building) in snapshot.buildings.iter().enumerate() {
            let px = padding + (f64::from(building.x) * pitch);
            let py = padding + (f64::from(building.y) * pitch);
            let seq = drawables.len();
            drawables.push(Drawable {
                sort_y: py + (f64::from(building.height) * pitch) - self.snapshot.gap(),
                seq,
                kind: DrawKind::Building,
                tile: None,
                entity: None,
                tile_x: building.x,
                tile_y: building.y,
                px,
                py,
                index,
            });
        }
        for (index, monster) in snapshot.monsters.iter().enumerate() {
            // Monsters stand by their centre, wherever it is (never a tile index).
            let px = padding + ((monster.x - 0.5) * pitch);
            let py = padding + ((monster.y - 0.5) * pitch);
            let seq = drawables.len();
            drawables.push(Drawable {
                sort_y: py + ts,
                seq,
                kind: DrawKind::Monster,
                tile: None,
                entity: None,
                tile_x: 0,
                tile_y: 0,
                px,
                py,
                index,
            });
        }
        let player = &snapshot.player;
        let player_x = player.pixel_x.unwrap_or_else(|| entity_pixel_origin(player.x, padding, pitch));
        let player_y = player.pixel_y.unwrap_or_else(|| entity_pixel_origin(player.y, padding, pitch));
        let seq = drawables.len();
        drawables.push(Drawable {
            sort_y: player_y + ts,
            seq,
            kind: DrawKind::Player,
            tile: None,
            entity: Some(player),
            tile_x: 0,
            tile_y: 0,
            px: player_x,
            py: player_y,
            index: 0,
        });

        // Y-sort: lower on screen draws later (in front); ties keep insertion order.
        drawables.sort_by(|a, b| cs_compare(a.sort_y, b.sort_y).then(a.seq.cmp(&b.seq)));

        // ---- Soft drop shadows (built-in art only; the color-block look stays as it was) ----
        if let Some(art) = art {
            for d in &drawables {
                let factor = match (d.kind, d.tile, d.entity) {
                    (DrawKind::Node, Some(tile), _) => tile
                        .node
                        .as_ref()
                        .filter(|node| node.sprite.is_none() && !node.depleted)
                        .map(|node| if is_foliage(art.node_entry(node.type_id.as_deref())) { 0.7 } else { 0.6 }),
                    (DrawKind::Machine, Some(tile), _) => {
                        tile.machine.as_ref().filter(|machine| machine.sprite.is_none()).map(|_| 0.7)
                    }
                    (DrawKind::Item, Some(tile), _) => {
                        tile.item.as_ref().filter(|item| item.sprite.is_none()).map(|_| 0.4)
                    }
                    (DrawKind::Entity | DrawKind::Player, _, Some(entity)) if entity.sprite.is_none() => Some(0.55),
                    (DrawKind::Monster, _, _) => snapshot.monsters[d.index].sprite.is_none().then_some(0.55),
                    _ => None,
                };
                if let Some(factor) = factor {
                    self.draw_shadow(d.px, d.py, factor);
                }
            }
        }

        // ---- Y-sorted objects and entities ----------------------------------------------------
        for d in &drawables {
            match d.kind {
                DrawKind::Node => self.node(d),
                DrawKind::Machine => self.machine(d),
                DrawKind::Crop => self.crop(d),
                DrawKind::Item => self.item(d),
                DrawKind::Entity => self.entity(d),
                DrawKind::Player => self.player(d),
                DrawKind::Building => self.building(d),
                DrawKind::Monster => self.monster(d),
                DrawKind::Tall => self.tall_object(d),
            }
        }

        // ---- The scene's extra layers over the player (M8): roofs, treetops ------------------
        for y in y_start..=y_end {
            let Some(row) = snapshot.tiles.get(y as usize) else { break };
            for x in x_start..=x_end.min(i32::try_from(row.len()).unwrap_or(i32::MAX) - 1) {
                self.decor(&row[x as usize], x, y, true);
            }
        }

        // ---- Atmosphere -------------------------------------------------------------------------
        if let Some(atmosphere) = &snapshot.atmosphere {
            let world_rect = Rect::new(0.0, 0.0, world_width as f32, world_height as f32);
            let overlay = atmosphere::weather_look(
                atmosphere.weather_id.as_deref(),
                atmosphere.weather_overlay.as_deref(),
                atmosphere.overlay.as_ref(),
            );
            let mut tint = atmosphere::daylight_tint_in(
                atmosphere.time_minutes,
                atmosphere.day_minutes.unwrap_or(atmosphere::DAY_MINUTES),
                atmosphere.daylight.as_deref().unwrap_or_default(),
            );
            if let Some(weather) = overlay.and_then(|overlay| overlay.tint) {
                // Integer math, like the C# byte arithmetic.
                let mix = |a: u8, b: u8| (u32::from(a) * u32::from(b) / 255) as u8;
                tint = Color::rgb(mix(tint.r, weather.r), mix(tint.g, weather.g), mix(tint.b, weather.b));
            }
            if !atmosphere::is_neutral(tint) {
                // Modulate multiplies color (and alpha) so the padding and letterbox stay untouched.
                self.push(DrawCmd::ModulateRect { rect: world_rect, color: tint });
            }
            self.lights(tint, camera.map(|camera| (camera.x, camera.y, camera.width, camera.height)));
            if let Some(overlay) = overlay {
                let (origin_x, origin_y) = camera.map_or((0.0, 0.0), |camera| (camera.x, camera.y));
                self.weather(overlay, tick, origin_x, origin_y, view_width, view_height, world_rect);
            }
        }

        // Cutscene emotes (M2), above the atmosphere like the pops: a bubble over the head.
        if let Some(emotes) = &snapshot.emotes {
            for emote in emotes {
                let (cx, top) = if emote.player {
                    (player_x + (ts / 2.0), player_y)
                } else {
                    (padding + (emote.x * pitch), padding + ((emote.y - 0.5) * pitch))
                };
                self.emote_bubble(emote, cx, top);
            }
        }

        // Juice: floating feedback text rises and fades with age.
        if let Some(pops) = snapshot.pops.as_ref().filter(|pops| !pops.is_empty()) {
            let size = cs_max(11.0, (ts * 0.42).floor()) as f32;
            for pop in pops {
                let age = cs_clamp(pop.age, 0.0, 1.0);
                let px = entity_pixel_origin(pop.x, padding, pitch) + (ts / 2.0);
                let py = entity_pixel_origin(pop.y, padding, pitch) - (age * ts * 0.8);
                let opacity = 1.0 - (age * age);
                self.push(DrawCmd::Text {
                    text: pop.text.clone(),
                    x: px as f32,
                    y: py as f32,
                    font: FontId::Bold,
                    size,
                    color: parse(pop.color.as_deref().unwrap_or("#ffd94a")).with_opacity(opacity),
                    stroke: Some(TextStroke { color: parse("#00000090").with_opacity(opacity), width: 3.0 }),
                    align: TextAlign::Center,
                });
            }
        }

        // Edit Mode grid overlay.
        if snapshot.grid_overlay {
            let color = parse("#ffffff10");
            // Only over tiles the snapshot has: `width`/`height` alone must not set the work.
            let rows = i32::try_from(snapshot.tiles.len()).unwrap_or(i32::MAX);
            let columns = i32::try_from(snapshot.tiles.iter().map(Vec::len).max().unwrap_or(0)).unwrap_or(i32::MAX);
            for y in y_start..=y_end.min(rows - 1) {
                for x in x_start..=x_end.min(columns - 1) {
                    let rect = rect(padding + (f64::from(x) * pitch), padding + (f64::from(y) * pitch), ts, ts);
                    self.push(DrawCmd::StrokeRect { rect, color, width: 0.5 });
                }
            }
        }

        self.push(DrawCmd::Restore);
    }

    fn ground(&mut self, x_start: i32, x_end: i32, y_start: i32, y_end: i32) {
        let snapshot = self.snapshot;
        let (ts, padding, pitch, tick, season) = (self.ts, self.padding, self.pitch, self.tick, self.season);
        let art = self.art;
        for y in y_start..=y_end {
            let Some(row) = snapshot.tiles.get(y as usize) else { break };
            let mut x = x_start;
            while x <= x_end && (x as usize) < row.len() {
                let tile = &row[x as usize];
                let px = padding + (f64::from(x) * pitch);
                let py = padding + (f64::from(y) * pitch);
                let tile_rect = rect(px, py, ts, ts);

                let ground_color = self.terrain_color(&tile.background, "grass");
                self.fill_rect(tile_rect, ground_color);
                if !self.draw_sprite(tile.art_layer(0), px, py, ts, ts, 1.0) {
                    if let Some(art) = art {
                        if self.plantable(&tile.background) {
                            // Farmable soil is drawn tilled; wet after watering, speckled when fertilized.
                            self.draw_builtin(art.soil(tile.watered, x, y), px, py, 1.0, false, true);
                            if tile.fertilized {
                                self.draw_builtin(art.fertilized_marker(), px, py, 1.0, false, true);
                            }
                        } else {
                            self.draw_builtin(
                                art.tile(Some(&tile.background), season, x, y, tick),
                                px,
                                py,
                                1.0,
                                false,
                                true,
                            );
                        }
                    }
                    self.autotile_edges(x, y, &tile.background, EdgeLayer::Background, ground_color, tile_rect);
                }

                if let Some(overlay) = &tile.overlay {
                    if !self.draw_sprite(tile.art_layer(1), px, py, ts, ts, 1.0) {
                        let color = self.terrain_color(overlay, "path");
                        if !art.is_some_and(|art| {
                            self.draw_builtin(art.tile(Some(overlay), None, x, y, tick), px, py, 1.0, false, true)
                        }) {
                            self.fill_rect(tile_rect, color.with_opacity(0.7));
                        }
                        self.autotile_edges(x, y, overlay, EdgeLayer::Overlay, color, tile_rect);
                    }
                }

                // A tall object (M8) stands among the player and NPCs instead (see `tall_object`).
                if let Some(object) = tile.object.as_ref().filter(|object| self.tall(object) == 0) {
                    if !self.draw_sprite(tile.art_layer(2), px, py, ts, ts, 1.0) {
                        let color = self.terrain_color(object, "wall");
                        if !art.is_some_and(|art| {
                            self.draw_builtin(art.tile(Some(object), None, x, y, tick), px, py, 1.0, false, true)
                        }) {
                            self.fill_rect(tile_rect, color);
                        }
                        self.autotile_edges(x, y, object, EdgeLayer::Object, color, tile_rect);
                    }
                }

                if tile.ladder_down && !art.is_some_and(|art| self.draw_builtin(art.ladder(), px, py, 1.0, false, true))
                {
                    let hole = (ts * 0.6).floor();
                    self.fill_rect(
                        rect(px + ((ts - hole) / 2.0), py + ((ts - hole) / 2.0), hole, hole),
                        parse("#241a10"),
                    );
                    let color = parse("#c9a95e");
                    let lx = px + (ts / 2.0);
                    let top = (py + ((ts - hole) / 2.0) + 2.0) as f32;
                    let bottom = (py + ((ts + hole) / 2.0) - 2.0) as f32;
                    for side in [-0.2, 0.2] {
                        let x = (lx + (hole * side)) as f32;
                        self.push(DrawCmd::Line { x0: x, y0: top, x1: x, y1: bottom, color, width: 2.0 });
                    }
                }

                // The scene's extra layers under the player (M8).
                self.decor(tile, x, y, false);
                x += 1;
            }
        }
    }

    /// The scene's lights (M8) glowing over the darkened scene: as strong as the day/night tint
    /// is dark (none at noon), each a soft disc of its colour with a brighter core. Lights whose
    /// glow misses the camera are left out.
    fn lights(&mut self, tint: Color, camera: Option<(f64, f64, f64, f64)>) {
        let snapshot = self.snapshot;
        if snapshot.lights.is_empty() {
            return;
        }
        let luminance = (0.299 * f64::from(tint.r)) + (0.587 * f64::from(tint.g)) + (0.114 * f64::from(tint.b));
        let darkness = 1.0 - (luminance / 255.0);
        if darkness < 0.08 {
            return;
        }
        let (padding, pitch) = (self.padding, self.pitch);
        for light in &snapshot.lights {
            let cx = padding + (light.x * pitch);
            let cy = padding + (light.y * pitch);
            let reach = cs_max(0.5, light.radius) * pitch;
            if let Some((x, y, width, height)) = camera {
                if cx + reach < x || cy + reach < y || cx - reach > x + width || cy - reach > y + height {
                    continue;
                }
            }
            let color = if light.color.is_empty() { parse("#ffd27a") } else { parse(&light.color) };
            let strength = cs_clamp(light.brightness, 0.0, 1.0) * darkness;
            for (scale, alpha) in [(1.0, 0.5), (0.4, 0.45)] {
                let r = reach * scale;
                self.push(DrawCmd::BlurOval {
                    rect: rect(cx - r, cy - r, r * 2.0, r * 2.0),
                    color: color.with_opacity(strength * alpha),
                    sigma: (r / 2.5) as f32,
                });
            }
        }
    }

    /// One tile's extra layers (M8): those `above` the player or those under it, in the scene's
    /// layer order, each its terrain's art or colour.
    fn decor(&mut self, tile: &SnapshotTile, x: i32, y: i32, above: bool) {
        if tile.decor.is_empty() {
            return;
        }
        let (ts, padding, pitch) = (self.ts, self.padding, self.pitch);
        let px = padding + (f64::from(x) * pitch);
        let py = padding + (f64::from(y) * pitch);
        let tile_rect = rect(px, py, ts, ts);
        for decor in tile.decor.iter().filter(|d| d.above == above) {
            if !self.draw_sprite(decor.sprite.as_ref(), px, py, ts, ts, 1.0) {
                let color = self.terrain_color(&decor.terrain, "path");
                self.fill_rect(tile_rect, color);
                self.autotile_edges(x, y, &decor.terrain, EdgeLayer::Decor(&decor.terrain), color, tile_rect);
            }
        }
    }

    /// An object-layer terrain that rises `tall` tiles above its own (M8): its art or colour
    /// standing on the tile's bottom edge, drawn in the y-sorted pass.
    fn tall_object(&mut self, d: &Drawable<'_>) {
        let Some(tile) = d.tile else { return };
        let Some(object) = tile.object.as_deref() else { return };
        let tall = f64::from(self.tall(object));
        let (ts, pitch) = (self.ts, self.pitch);
        let height = ts + (tall * pitch);
        let top = d.py + ts - height;
        if !self.draw_sprite(tile.art_layer(2), d.px, top, ts, height, 1.0) {
            let color = self.terrain_color(object, "wall");
            self.fill_rect(rect(d.px, top, ts, height), color);
            // A lighter top face, so it reads as standing rather than painted on.
            let lighter = Color::rgba(
                color.r.saturating_add((255 - color.r) / 4),
                color.g.saturating_add((255 - color.g) / 4),
                color.b.saturating_add((255 - color.b) / 4),
                color.a,
            );
            self.fill_rect(rect(d.px, top, ts, cs_min(ts * 0.3, height)), lighter);
        }
    }

    fn node(&mut self, d: &Drawable<'_>) {
        let Some(node) = d.tile.and_then(|tile| tile.node.as_ref()) else { return };
        let (px, py, ts) = (d.px, d.py, self.ts);
        let opacity = if node.depleted { 0.35 } else { 1.0 };
        if self.draw_sprite(node.sprite.as_ref(), px, py, ts, ts, opacity) {
            return;
        }
        if let Some(art) = self.art {
            let foliage = is_foliage(art.node_entry(node.type_id.as_deref()));
            if self.draw_builtin(art.node(node.type_id.as_deref(), d.tile_x, d.tile_y), px, py, opacity, foliage, false)
            {
                return;
            }
        }
        // Gathering node: filled circle in the node color; faded while depleted.
        let radius = (ts * 0.32).floor() as f32;
        let cx = (px + (ts / 2.0)) as f32;
        let cy = (py + (ts / 2.0)) as f32;
        self.push(DrawCmd::FillCircle { cx, cy, radius, color: parse(&node.color).with_opacity(opacity) });
        self.push(DrawCmd::StrokeCircle {
            cx,
            cy,
            radius,
            color: parse("#00000040").with_opacity(opacity),
            width: 1.0,
        });
    }

    fn machine(&mut self, d: &Drawable<'_>) {
        let Some(machine) = d.tile.and_then(|tile| tile.machine.as_ref()) else { return };
        let (px, py, ts) = (d.px, d.py, self.ts);
        // An open gate (M5) stands aside: its art faded between its two posts.
        if machine.open {
            self.draw_sprite(machine.sprite.as_ref(), px, py, ts, ts, 0.3);
            let color = parse(&machine.color);
            let post_width = cs_max(2.0, (ts * 0.16).floor());
            let post_height = (ts * 0.8).floor();
            for x in [px + 1.0, px + ts - post_width - 1.0] {
                let post = rect(x, py + ts - post_height, post_width, post_height);
                self.push(DrawCmd::FillRoundRect { rect: post, radius: 1.5, color });
                self.push(DrawCmd::StrokeRoundRect { rect: post, radius: 1.5, color: parse("#00000060"), width: 1.0 });
            }
            return;
        }
        if self.draw_sprite(machine.sprite.as_ref(), px, py, ts, ts, 1.0) {
            return;
        }
        if let Some(art) = self.art {
            if self.draw_builtin(art.machine(machine.type_id.as_deref(), machine.working), px, py, 1.0, false, false) {
                if machine.output_ready {
                    self.draw_builtin(art.ready_marker(), px, py - (ts * 0.45), 1.0, false, true);
                }
                return;
            }
        }
        let size = (ts * 0.7).floor();
        let mx = px + ((ts - size) / 2.0);
        let my = py + ((ts - size) / 2.0);
        let body = rect(mx, my, size, size);
        self.push(DrawCmd::FillRoundRect { rect: body, radius: 3.0, color: parse(&machine.color) });
        self.push(DrawCmd::StrokeRoundRect { rect: body, radius: 3.0, color: parse("#00000060"), width: 1.5 });
        if machine.output_ready {
            self.push(DrawCmd::FillCircle {
                cx: (px + (ts * 0.72)) as f32,
                cy: (py + (ts * 0.3)) as f32,
                radius: (ts * 0.12).floor() as f32,
                color: parse("#ffd94a"),
            });
        } else if machine.working {
            let light = rect(mx + (size * 0.35), my + (size * 0.35), size * 0.3, size * 0.3);
            self.fill_rect(light, parse("#ffffff70"));
        }
    }

    fn crop(&mut self, d: &Drawable<'_>) {
        let Some(crop) = d.tile.and_then(|tile| tile.crop.as_ref()) else { return };
        let (px, py, ts) = (d.px, d.py, self.ts);
        let crop_size = (ts * 0.35).floor();
        let crop_rect = rect(px + ((ts - crop_size) / 2.0), py + ((ts - crop_size) / 2.0), crop_size, crop_size);
        if !crop.withered && crop.mature {
            // Canvas shadowBlur 8 ≈ Gaussian sigma 4: a golden glow behind ripe crops.
            self.push(DrawCmd::BlurRect { rect: crop_rect, color: parse("#d6bd2499"), sigma: 4.0 });
        }
        if self.draw_sprite(crop.sprite.as_ref(), px, py, ts, ts, 1.0) {
            return;
        }
        if let Some(art) = self.art {
            let sprite = art.crop(crop.crop_id.as_deref(), crop.color_index, crop.stages, crop.withered, crop.mature);
            if self.draw_builtin(sprite, px, py, 1.0, false, false) {
                return;
            }
        }
        let index = crop.color_index.clamp(0, canvas2d::CROP_STAGE_COLORS.len() as i32 - 1) as usize;
        let css = if crop.withered { canvas2d::WITHERED_CROP_COLOR } else { canvas2d::CROP_STAGE_COLORS[index] };
        self.fill_rect(crop_rect, parse(css));
    }

    fn item(&mut self, d: &Drawable<'_>) {
        let Some(item) = d.tile.and_then(|tile| tile.item.as_ref()) else { return };
        let (px, py, ts) = (d.px, d.py, self.ts);
        if self.draw_sprite(item.sprite.as_ref(), px, py, ts, ts, 1.0) {
            return;
        }
        if let Some(art) = self.art {
            if self.draw_builtin(art.item(item.item_type.as_deref()), px, py, 1.0, false, false) {
                return;
            }
        }
        self.push(DrawCmd::FillCircle {
            cx: (px + (ts / 2.0)) as f32,
            cy: (py + (ts / 2.0)) as f32,
            radius: (ts * 0.15).floor() as f32,
            color: parse(canvas2d::ITEM_COLOR),
        });
    }

    /// A building (M5): its art over the footprint (a tile taller, standing on its bottom edge);
    /// without art a block with a roof, windows and a door. A site still being built is a
    /// scaffold of posts and planks over its foundation.
    fn building(&mut self, d: &Drawable<'_>) {
        let building = &self.snapshot.buildings[d.index];
        let (px, py, ts, pitch) = (d.px, d.py, self.ts, self.pitch);
        let width = (f64::from(building.width) * pitch) - self.snapshot.gap();
        let height = (f64::from(building.height) * pitch) - self.snapshot.gap();
        let color = parse(&building.color);
        let outline = parse("#00000070");
        if building.scaffold {
            self.push(DrawCmd::FillRect {
                rect: rect(px, py, width, height),
                color: parse("#8a7350a0"),
                anti_alias: false,
            });
            let wood = parse("#a0763c");
            let top = py - (ts * 0.3);
            let bottom = py + height;
            let line = |x0: f64, y0: f64, x1: f64, y1: f64| DrawCmd::Line {
                x0: x0 as f32,
                y0: y0 as f32,
                x1: x1 as f32,
                y1: y1 as f32,
                color: wood,
                width: cs_max(1.5, (ts * 0.08).floor()) as f32,
            };
            for column in 0..=building.width {
                let x = (px + (f64::from(column) * pitch)).clamp(px + 1.0, px + width - 1.0);
                self.push(line(x, top, x, bottom));
            }
            for level in [0.3, 0.7] {
                let y = top + ((bottom - top) * level);
                self.push(line(px, y, px + width, y));
            }
            self.push(line(px, bottom, px + width, top));
            return;
        }
        if self.draw_sprite(building.sprite.as_ref(), px, py - ts, width, height + ts, 1.0) {
            return;
        }
        // Walls on the footprint, the roof over its upper part and half a tile above it.
        let roof_height = (height * 0.35) + (ts * 0.5);
        let walls = rect(px, py + (height * 0.35), width, height * 0.65);
        self.push(DrawCmd::FillRect { rect: walls, color, anti_alias: false });
        self.push(DrawCmd::StrokeRect { rect: walls, color: outline, width: 1.0 });
        let roof = rect(px - 2.0, py + (height * 0.35) - roof_height, width + 4.0, roof_height + 2.0);
        let roof_color = parse("#7a2e22");
        self.push(DrawCmd::FillRoundRect { rect: roof, radius: 3.0, color: roof_color });
        self.push(DrawCmd::StrokeRoundRect { rect: roof, radius: 3.0, color: outline, width: 1.0 });
        let ridge_y = (roof.y + (roof.height * 0.3)) as f32;
        self.push(DrawCmd::Line {
            x0: (px + 3.0) as f32,
            y0: ridge_y,
            x1: (px + width - 3.0) as f32,
            y1: ridge_y,
            color: parse("#00000040"),
            width: 1.0,
        });
        // Windows either side of the door on a building three or more tiles wide.
        let window = (ts * 0.32).floor();
        let window_y = walls.y as f64 + ((f64::from(walls.height) - window) * 0.35);
        if building.width >= 3 {
            for x in [px + (ts * 0.34), px + width - (ts * 0.34) - window] {
                let pane = rect(x, window_y, window, window);
                self.push(DrawCmd::FillRect { rect: pane, color: parse("#f4e3a1"), anti_alias: false });
                self.push(DrawCmd::StrokeRect { rect: pane, color: outline, width: 1.0 });
            }
        }
        if let Some((dx, dy)) = building.door {
            let door_width = (ts * 0.5).floor();
            let door_height = (ts * 0.72).floor();
            let dx = px + (f64::from(dx) * pitch) + ((ts - door_width) / 2.0);
            let dy = py + (f64::from(dy) * pitch) + ts - door_height;
            let door = rect(dx, dy, door_width, door_height);
            self.push(DrawCmd::FillRoundRect { rect: door, radius: 2.0, color: parse("#4a2e1a") });
            self.push(DrawCmd::FillCircle {
                cx: (dx + (door_width * 0.75)) as f32,
                cy: (dy + (door_height * 0.55)) as f32,
                radius: cs_max(1.0, ts * 0.04) as f32,
                color: parse("#e0c060"),
            });
        }
    }

    /// A monster (M7): its art, else a blob of its colour with eyes that look where it goes,
    /// lighter while it reels; a small health bar over it once hurt.
    fn monster(&mut self, d: &Drawable<'_>) {
        let monster = &self.snapshot.monsters[d.index];
        let (px, py, ts, tick) = (d.px, d.py, self.ts, self.tick);
        let opacity = if monster.stunned { 0.6 } else { 1.0 };
        if !self.draw_sprite(monster.sprite.as_ref(), px, py, ts, ts, opacity) {
            // A gentle bob while it moves.
            let bob = if monster.moving { ((tick / 4.0).floor() % 2.0) * (ts * 0.04) } else { 0.0 };
            let body_width = (ts * 0.72).floor();
            let body_height = (ts * 0.58).floor();
            let body = rect(px + ((ts - body_width) / 2.0), py + ts - body_height - 2.0 - bob, body_width, body_height);
            let color = parse(&monster.color).with_opacity(opacity);
            self.push(DrawCmd::FillRoundRect { rect: body, radius: (body_height * 0.45) as f32, color });
            self.push(DrawCmd::StrokeRoundRect {
                rect: body,
                radius: (body_height * 0.45) as f32,
                color: parse("#00000070").with_opacity(opacity),
                width: 1.0,
            });
            let (dx, dy) = canvas2d::direction_offset(Some(&monster.direction));
            let eye = cs_max(1.5, (ts * 0.09).floor());
            let pupil = cs_max(1.0, (eye * 0.55).floor());
            let eye_y = f64::from(body.y) + (body_height * 0.4);
            let center = f64::from(body.x) + (body_width / 2.0);
            for side in [-1.0, 1.0] {
                let ex = center + (side * body_width * 0.18);
                self.push(DrawCmd::FillCircle {
                    cx: ex as f32,
                    cy: eye_y as f32,
                    radius: eye as f32,
                    color: parse("#ffffff"),
                });
                self.push(DrawCmd::FillCircle {
                    cx: (ex + (f64::from(dx) * (eye - pupil))) as f32,
                    cy: (eye_y + (f64::from(dy) * (eye - pupil))) as f32,
                    radius: pupil as f32,
                    color: parse("#1a1a2e"),
                });
            }
        }
        if let Some(health) = monster.health {
            let bar_width = (ts * 0.7).floor();
            let bar = rect(px + ((ts - bar_width) / 2.0), py - 2.0, bar_width, cs_max(2.0, (ts * 0.09).floor()));
            self.push(DrawCmd::FillRect { rect: bar, color: parse("#000000a0"), anti_alias: false });
            let left =
                rect(f64::from(bar.x), f64::from(bar.y), bar_width * cs_clamp(health, 0.0, 1.0), f64::from(bar.height));
            let color = if health > 0.5 {
                parse("#5ec25e")
            } else if health > 0.25 {
                parse("#e0b040")
            } else {
                parse("#e04f4f")
            };
            self.push(DrawCmd::FillRect { rect: left, color, anti_alias: false });
        }
    }

    /// Creator sprite, then built-in art (`builtin` picks it).
    fn entity_art(
        &mut self,
        entity: &SnapshotEntity,
        px: f64,
        py: f64,
        builtin: impl FnOnce(&BuiltinArt) -> Option<SnapshotSprite>,
    ) -> bool {
        let ts = self.ts;
        if entity.sprite.is_some() && self.draw_sprite(entity.sprite.as_ref(), px, py, ts, ts, 1.0) {
            return true;
        }
        match self.art {
            Some(art) => self.draw_builtin(builtin(art), px, py, 1.0, false, false),
            None => false,
        }
    }

    fn entity(&mut self, d: &Drawable<'_>) {
        let Some(npc) = d.entity else { return };
        let (px, py, ts, tick) = (d.px, d.py, self.ts, self.tick);
        let npc_w = (ts * 0.65).floor();
        let npc_h = (ts * 0.75).floor();
        // Children (M4) draw smaller.
        let art_scale = self.art_scale;
        self.art_scale *= npc.scale.unwrap_or(1.0);
        let drawn = self.entity_art(npc, px, py, |art| {
            if npc.kind == "animal" {
                art.animal(npc.species_id.as_deref(), Some(&npc.direction), tick, npc.moving)
            } else {
                art.npc(npc.appearance.as_deref(), Some(&npc.direction), tick, npc.moving)
            }
        });
        self.art_scale = art_scale;
        if drawn {
            return;
        }
        let body = rect(px + ((ts - npc_w) / 2.0), py + ((ts - npc_h) / 2.0), npc_w, npc_h);
        let color = parse(npc.color.as_deref().unwrap_or(canvas2d::NPC_COLOR));
        self.push(DrawCmd::FillRoundRect { rect: body, radius: 2.0, color });
        self.push(DrawCmd::StrokeRoundRect {
            rect: body,
            radius: 2.0,
            color: parse(canvas2d::NPC_BORDER_COLOR),
            width: 1.0,
        });
    }

    /// A cutscene emote: a rounded bubble with the emote's glyph, centred on `cx` above a
    /// character whose tile starts at `top` (built-in characters stand half a tile taller). It
    /// pops in over the first eighth of its time and fades over the last.
    fn emote_bubble(&mut self, emote: &SnapshotEmote, cx: f64, top: f64) {
        let ts = self.ts;
        let progress = cs_clamp(emote.progress, 0.0, 1.0);
        let scale = 0.6 + (0.4 * cs_min(1.0, progress * 8.0));
        let opacity = cs_min(1.0, (1.0 - progress) * 8.0);
        let (glyph, color) = emote_glyph(&emote.emote);
        let size = (ts * 0.62).floor() * scale;
        let bottom = top - (ts * 0.55);
        let bubble = rect(cx - (size / 2.0), bottom - size, size, size);
        let radius = (size * 0.3) as f32;
        self.push(DrawCmd::FillCircle {
            cx: cx as f32,
            cy: (bottom + (ts * 0.04)) as f32,
            radius: (ts * 0.06 * scale) as f32,
            color: parse("#ffffff").with_opacity(opacity),
        });
        self.push(DrawCmd::FillRoundRect { rect: bubble, radius, color: parse("#ffffff").with_opacity(opacity) });
        self.push(DrawCmd::StrokeRoundRect {
            rect: bubble,
            radius,
            color: parse("#00000070").with_opacity(opacity),
            width: 1.0,
        });
        let text_size = (size * 0.7) as f32;
        self.push(DrawCmd::Text {
            text: glyph.to_owned(),
            x: cx as f32,
            // Baseline: the glyph's cap height centred in the bubble.
            y: ((bottom - (size / 2.0)) + (f64::from(text_size) * 0.36)) as f32,
            font: FontId::Bold,
            size: text_size,
            color: parse(color).with_opacity(opacity),
            stroke: None,
            align: TextAlign::Center,
        });
    }

    fn player(&mut self, d: &Drawable<'_>) {
        let Some(player) = d.entity else { return };
        let (px, py, ts, tick) = (d.px, d.py, self.ts, self.tick);
        let player_w = (ts * 0.65).floor();
        let player_h = (ts * 0.75).floor();
        let look = player.look.as_deref();
        let drawn = self.entity_art(player, px, py, |art| {
            art.player(Some(&player.direction), tick, player.moving).map(|sprite| crate::look::with_look(sprite, look))
        });
        // A worn hat (M4) sits on the head: the built-in characters stand a tile and a half tall.
        if player.hat.is_some() {
            let size = (ts * 0.6).floor();
            self.draw_sprite(player.hat.as_ref(), px + ((ts - size) / 2.0), py - (ts * 0.62), size, size, 1.0);
        }
        if drawn {
            return;
        }
        let body = rect(px + ((ts - player_w) / 2.0), py + ((ts - player_h) / 2.0), player_w, player_h);
        self.push(DrawCmd::FillRoundRect { rect: body, radius: 2.0, color: parse(canvas2d::PLAYER_COLOR) });
        self.push(DrawCmd::StrokeRoundRect {
            rect: body,
            radius: 2.0,
            color: parse(canvas2d::PLAYER_BORDER_COLOR),
            width: 2.0,
        });

        let (dx, dy) = canvas2d::direction_offset(Some(&player.direction));
        let dot = (ts * 0.1).floor();
        let center_x = px + (ts / 2.0);
        let center_y = py + (ts / 2.0);
        let dot_x = center_x + (f64::from(dx) * ((player_w / 2.0) - (dot / 2.0))) - (dot / 2.0);
        let dot_y = center_y + (f64::from(dy) * ((player_h / 2.0) - (dot / 2.0))) - (dot / 2.0);
        self.push(DrawCmd::FillCircle {
            cx: (dot_x + (dot / 2.0)) as f32,
            cy: (dot_y + (dot / 2.0)) as f32,
            radius: (dot / 2.0) as f32,
            color: parse("#ffffffcc"),
        });
    }

    /// Rain streaks, snow flakes or dust specks: every particle's position is a pure function of
    /// the tick and its index (hash), so frames are reproducible and nothing is stored. The
    /// look's density scales the particle count and its speed the clock they move by.
    #[allow(clippy::too_many_arguments)]
    fn weather(
        &mut self,
        overlay: WeatherLook,
        tick: f64,
        origin_x: f64,
        origin_y: f64,
        view_width: f64,
        view_height: f64,
        world_rect: Rect,
    ) {
        if overlay.style == WeatherOverlay::TintOnly {
            return;
        }
        let count = atmosphere::particle_count(view_width, view_height);
        // Built-in density and speed (1) leave the count and the clock exactly as they were.
        let count = if overlay.density == 1.0 {
            count
        } else {
            to_int(cs_clamp(round_half_even(f64::from(count) * overlay.density), 0.0, 4800.0))
        };
        let t = cs_max(0.0, tick) * overlay.speed;
        let color = overlay.color;
        self.push(DrawCmd::Save);
        self.push(DrawCmd::ClipRect { rect: world_rect });
        for i in 0..count.max(0) as u32 {
            let unit = |salt: u32| atmosphere::unit(i, salt);
            match overlay.style {
                WeatherOverlay::Rain => {
                    let speed = 9.0 + (unit(3) * 4.0);
                    let x = origin_x + ((((unit(1) * view_width) - (t * 2.5)) % view_width + view_width) % view_width);
                    let y = origin_y + (((unit(2) * view_height) + (t * speed)) % view_height);
                    self.push(DrawCmd::Line {
                        x0: x as f32,
                        y0: y as f32,
                        x1: (x - 2.0) as f32,
                        y1: (y - 9.0) as f32,
                        color,
                        width: 1.0,
                    });
                }
                WeatherOverlay::Snow => {
                    let speed = 0.9 + (unit(3) * 0.8);
                    // The reference renderer uses the literal 6.283, not TAU.
                    #[allow(clippy::approx_constant)]
                    let sway = ((t / 12.0) + (unit(4) * 6.283)).sin() * 5.0;
                    let x = origin_x + ((((unit(1) * view_width) + sway) % view_width + view_width) % view_width);
                    let y = origin_y + (((unit(2) * view_height) + (t * speed)) % view_height);
                    self.push(DrawCmd::FillCircle {
                        cx: x as f32,
                        cy: y as f32,
                        radius: (1.0 + (unit(5) * 1.2)) as f32,
                        color,
                    });
                }
                WeatherOverlay::Dust => {
                    // Blown sideways, bobbing a little, sinking slowly.
                    let drift = 1.2 + (unit(3) * 1.6);
                    #[allow(clippy::approx_constant)]
                    let bob = ((t / 18.0) + (unit(4) * 6.283)).sin() * 3.0;
                    let x =
                        origin_x + ((((unit(1) * view_width) + (t * drift)) % view_width + view_width) % view_width);
                    let y = origin_y
                        + ((((unit(2) * view_height) + (t * 0.25) + bob) % view_height + view_height) % view_height);
                    let size = (1.0 + (unit(5) * 1.5)).floor();
                    self.fill_rect(rect(x, y, size, size), color);
                }
                WeatherOverlay::TintOnly => {}
            }
        }
        self.push(DrawCmd::Restore);
    }
}
