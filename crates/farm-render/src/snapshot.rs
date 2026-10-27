//! Port of `WorldSnapshot.cs` (the snapshot types of packages/renderer-canvas2d).
//!
//! A snapshot is plain data: no engine state and no callbacks. Hosts build a fresh one per
//! frame ([`crate::shell_snapshot`] in play, [`crate::editor_snapshot`] in Edit Mode),
//! [`crate::apply_graphics`] decorates it with creator art, and [`crate::build_world`] turns
//! it into a draw list. Content ids (crop, node, machine, species, appearance) ride along so
//! the renderer can pick built-in art when a sprite is missing; they are never interpreted
//! beyond that.
//!
//! The JSON form (camelCase) is what the C# host deserializes into its `WorldSnapshot`
//! classes, so field names must stay as they are. `None` fields are omitted.

use farm_sim::schema::{DaylightKey, VisualRef, WeatherOverlayDefinition};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Crop drawn on a tile.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SnapshotCrop {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sprite: Option<SnapshotSprite>,
    /// Stage color index; the renderer clamps it to the palette.
    pub color_index: i32,
    pub mature: bool,
    pub withered: bool,
    /// Crop definition id (built-in art lookup).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crop_id: Option<String>,
    /// Growth stages of the definition (0 = unknown).
    pub stages: i32,
}

/// Gathering node occupying a tile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SnapshotNode {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sprite: Option<SnapshotSprite>,
    pub color: String,
    /// Depleted nodes render faded while awaiting respawn.
    pub depleted: bool,
    /// Node type id (built-in art lookup).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_id: Option<String>,
}

impl Default for SnapshotNode {
    fn default() -> Self {
        Self { sprite: None, color: "#7a5a3a".to_owned(), depleted: false, type_id: None }
    }
}

/// Placed machine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SnapshotMachine {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sprite: Option<SnapshotSprite>,
    pub color: String,
    pub working: bool,
    pub output_ready: bool,
    /// Machine type id (built-in art lookup).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_id: Option<String>,
    /// A gate standing open (M5): drawn as its two posts.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub open: bool,
}

impl Default for SnapshotMachine {
    fn default() -> Self {
        Self {
            sprite: None,
            color: "#9a7b4f".to_owned(),
            working: false,
            output_ready: false,
            type_id: None,
            open: false,
        }
    }
}

/// Item lying on a tile.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SnapshotItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sprite: Option<SnapshotSprite>,
    /// Item type (`seed`, `crop`, `tool`, …) for the built-in art.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_type: Option<String>,
}

/// One map cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SnapshotTile {
    pub background: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    /// Per-layer artwork: `[background, overlay, object]` (entries may be null).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub art_layers: Option<Vec<Option<SnapshotSprite>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crop: Option<SnapshotCrop>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<SnapshotNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine: Option<SnapshotMachine>,
    /// Revealed mine ladder going down.
    pub ladder_down: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<SnapshotItem>,
    /// Soil that has been worked (hoed or planted): drawn with furrows.
    pub tilled: bool,
    /// Soil is wet (watered today or by rain).
    pub watered: bool,
    /// Fertilizer applied: a speckle marker is drawn over the soil.
    pub fertilized: bool,
    /// The scene's extra layers on this tile (M8), in the scene's layer order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub decor: Vec<SnapshotDecor>,
}

impl Default for SnapshotTile {
    fn default() -> Self {
        Self {
            background: "grass".to_owned(),
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
}

impl SnapshotTile {
    /// Art for layer `index` (0 background, 1 overlay, 2 object), when bound.
    pub fn art_layer(&self, index: usize) -> Option<&SnapshotSprite> {
        self.art_layers.as_ref().and_then(|layers| layers.get(index)).and_then(Option::as_ref)
    }
}

/// Sprite-sheet reference: the renderer draws frame `frame` from row `row` of a grid-sliced
/// sheet, or the explicit source rectangle when `source_x`/`source_y` are set. Zero frame
/// sizes mean the full image.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SnapshotSprite {
    /// A `data:` URL or a `builtin://` sheet. Shared, so sprites of the same sheet cost no copy.
    pub image_url: Arc<str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_y: Option<f64>,
    pub frame_width: f64,
    pub frame_height: f64,
    pub frame: f64,
    pub row: f64,
}

/// An NPC, an animal or the player (C# `SnapshotEntity` and `SnapshotPlayer`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SnapshotEntity {
    pub x: f64,
    pub y: f64,
    /// Creator art (an animated sprite sheet or one frame).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sprite: Option<SnapshotSprite>,
    /// Fallback fill color (defaults to the NPC gold).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// `npc` (default) or `animal`.
    pub kind: String,
    /// NPC appearance id (`farmer`, `merchant`, …) for the built-in art.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appearance: Option<String>,
    /// Animal species id for the built-in art.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub species_id: Option<String>,
    /// Facing direction (down/left/right/up).
    pub direction: String,
    /// True while walking (drives the walk cycle; idle frame otherwise).
    pub moving: bool,
    /// Pixel-space override of the drawn top-left corner: the player's interpolated movement
    /// (play mode), an NPC a cutscene walks between tiles.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pixel_x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pixel_y: Option<f64>,
    /// The player's colours and clothes as a look key (M4, [`crate::look`]): the built-in art
    /// is recoloured with it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub look: Option<String>,
    /// The art of the hat the player wears (M4), resolved by [`crate::apply_graphics`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hat_visual: Option<VisualRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hat: Option<SnapshotSprite>,
    /// Drawn smaller or larger than a tile's actor (children, M4); absent: 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
}

impl Default for SnapshotEntity {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            sprite: None,
            color: None,
            kind: "npc".to_owned(),
            appearance: None,
            species_id: None,
            direction: "down".to_owned(),
            moving: false,
            pixel_x: None,
            pixel_y: None,
            look: None,
            hat_visual: None,
            hat: None,
            scale: None,
        }
    }
}

/// The player is an entity with an optional pixel position.
pub type SnapshotPlayer = SnapshotEntity;

/// A monster (M7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SnapshotMonster {
    /// Its centre, in tiles.
    pub x: f64,
    pub y: f64,
    /// Its kind's art, resolved from `visual` by [`crate::apply_graphics`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visual: Option<VisualRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sprite: Option<SnapshotSprite>,
    /// `#rrggbb`: the blob drawn without art.
    pub color: String,
    /// Facing direction (down/left/right/up): where its eyes look.
    pub direction: String,
    pub moving: bool,
    /// The health it has left, 0..1, once it has been hurt (a small bar over it); `None` at
    /// full health.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<f64>,
    /// Reeling from a hit: drawn lighter.
    pub stunned: bool,
}

impl Default for SnapshotMonster {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            visual: None,
            sprite: None,
            color: "#6a8f3a".to_owned(),
            direction: "down".to_owned(),
            moving: false,
            health: None,
            stunned: false,
        }
    }
}

/// A building on the map (M5): its footprint in tiles, standing or still being built.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SnapshotBuilding {
    /// The footprint's top-left tile and size.
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    /// Its kind's art, resolved from `visual` by [`crate::apply_graphics`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visual: Option<VisualRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sprite: Option<SnapshotSprite>,
    /// `#rrggbb`: the walls drawn without art.
    pub color: String,
    /// Still being built: a scaffold on its site.
    pub scaffold: bool,
    /// The door's tile within the footprint; `None` without an interior.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub door: Option<(i32, i32)>,
}

impl Default for SnapshotBuilding {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            visual: None,
            sprite: None,
            color: "#b5651d".to_owned(),
            scaffold: false,
            door: None,
        }
    }
}

/// Floating feedback text ("juice"): a renderer reaction, never simulation state.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SnapshotPop {
    pub x: f64,
    pub y: f64,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 0 (just spawned) → 1 (expired); drives rise and fade.
    pub age: f64,
}

/// An emote over a character in a cutscene (M2): a bubble with the emote's glyph.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SnapshotEmote {
    /// The character's centre, in tiles.
    pub x: f64,
    pub y: f64,
    /// Over the player: the bubble follows the player as drawn ([`SnapshotEntity::pixel_x`]).
    pub player: bool,
    /// One of `farm_sim::schema::emotes::ALL`.
    pub emote: String,
    /// 0 (just shown) → 1 (gone); drives the pop-in and the fade-out.
    pub progress: f64,
}

/// Camera viewport in world-pixel space. When present the renderer translates the world by
/// (−x, −y) and culls tiles outside the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SnapshotCamera {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Time of day, weather and season for the atmosphere pass (day/night tint, seasonal foliage,
/// rain and snow). `None` on a snapshot means no atmosphere (Edit Mode without its sky preview).
///
/// The game's own look (M1) rides along already resolved, so the renderer never reads content:
/// the daylight keyframes and foliage tint of the scene and season, the day length and today's
/// overlay definition. Each is `None` for the built-in look.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SnapshotAtmosphere {
    /// Minute of day of the game clock (360 = 6:00).
    pub time_minutes: f64,
    /// Today's weather id (`sun`, `rain`, `storm`, `snow`, …); `None` indoors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weather_id: Option<String>,
    /// Season id (`spring`, `summer`, `fall`, `winter`, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season: Option<String>,
    /// Overlay hint from the weather definition (`rain` | `snow`), when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weather_overlay: Option<String>,
    /// Minutes in the game's day: the tint wraps there. `None`: 1440.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub day_minutes: Option<f64>,
    /// Day/night keyframes (minute of day, `#rrggbb` multiply colour). `None` or empty: the
    /// built-in curve, stretched over the day.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub daylight: Option<Vec<DaylightKey>>,
    /// Foliage multiply tint (`#rrggbb`). `None`: the built-in tint of the season.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub foliage_tint: Option<String>,
    /// Today's overlay, resolved from the weather's `overlay` id. `None`: the overlay is guessed
    /// from [`SnapshotAtmosphere::weather_overlay`] and the weather id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay: Option<WeatherOverlayDefinition>,
}

/// A terrain as the renderer reads it (M1): the colour of a tile of it without art (and under
/// transparent art), and whether it is farmable ground, drawn tilled, wet and fertilized.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SnapshotTerrain {
    pub id: String,
    /// `#rrggbb`; empty or unparseable: the built-in colour of the id.
    pub color: String,
    pub plantable: bool,
    /// Joins up with neighbours of the same terrain (M8): without art its edges are drawn where
    /// it meets another terrain (with art, [`crate::apply_graphics`] picks the frame).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub autotile: bool,
    /// On the object layer, the tiles it rises above its own (M8): drawn standing, y-sorted with
    /// the player.
    #[serde(skip_serializing_if = "is_zero")]
    pub tall: u32,
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}

/// What one of a scene's extra layers (M8) shows on a tile.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SnapshotDecor {
    /// The terrain painted on it.
    pub terrain: String,
    /// Drawn over the player and everything standing.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub above: bool,
    /// The terrain's art ([`crate::apply_graphics`]); without it, the terrain's colour.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sprite: Option<SnapshotSprite>,
}

/// A light that glows at night (M8), at a point of the scene.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SnapshotLight {
    /// The centre, in tiles.
    pub x: f64,
    pub y: f64,
    /// How far it reaches, in tiles.
    pub radius: f64,
    /// `#rrggbb`; empty: a warm lamp light.
    pub color: String,
    /// How strong it is at its centre, 0 to 1.
    pub brightness: f64,
}

/// The most tiles a terrain rises above its own ([`SnapshotTerrain::tall`]).
pub const MAX_TALL: u32 = 8;

/// The tiles a snapshot holds: columns `x0..=x1` of rows `y0..=y1`. Rows outside are empty, and
/// cells left of `x0` are blank stand-ins (indices stay `tiles[y][x]`); the renderer only reads
/// tiles in its camera's range, which the window must cover ([`TileWindow::for_camera`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TileWindow {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl TileWindow {
    /// The tiles [`crate::build_world`] reads for `camera` (world pixels): the visible range,
    /// rounded out, and the row below it (tall objects there poke into view), plus `margin`
    /// tiles around (a host that draws with a slightly larger camera than it built for). A game
    /// with tall terrains adds the rows they rise from ([`TileWindow::with_tall`]).
    pub fn for_camera(camera: &SnapshotCamera, padding: f64, pitch: f64, margin: i32) -> Self {
        let pitch = if pitch > 0.0 { pitch } else { 1.0 };
        let first = |pixel: f64| crate::num::to_int(((pixel - padding) / pitch).floor());
        let last = |pixel: f64| crate::num::to_int(((pixel - padding) / pitch).ceil());
        TileWindow {
            x0: first(camera.x).saturating_sub(margin).max(0),
            y0: first(camera.y).saturating_sub(margin).max(0),
            x1: last(camera.x + camera.width).saturating_add(margin),
            y1: last(camera.y + camera.height).saturating_add(1).saturating_add(margin),
        }
    }

    /// The window with `tall` more rows below (M8): a terrain that rises `tall` tiles stands
    /// that far below the view and still reaches into it.
    pub fn with_tall(self, tall: u32) -> Self {
        let tall = i32::try_from(tall.min(MAX_TALL)).unwrap_or(0);
        Self { y1: self.y1.saturating_add(tall), ..self }
    }

    pub fn contains(&self, x: usize, y: usize) -> bool {
        let (x, y) = (x as i64, y as i64);
        x >= i64::from(self.x0) && x <= i64::from(self.x1) && y >= i64::from(self.y0) && y <= i64::from(self.y1)
    }
}

/// Everything the world renderer draws in one frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorldSnapshot {
    pub width: i32,
    pub height: i32,
    pub tile_size: f64,
    pub padding: f64,
    /// Pixel gap between tiles. Edit Mode uses 1 (visible grid seams); play mode passes 0 for a
    /// contiguous world. `None` means 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tile_gap: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera: Option<SnapshotCamera>,
    /// `None` means "not set" and draws as pixel art (only `false` smooths).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pixel_art: Option<bool>,
    pub grid_overlay: bool,
    /// Engine tick the frame shows (drives water, walk cycles and weather; 0 in Edit Mode).
    pub tick: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub atmosphere: Option<SnapshotAtmosphere>,
    /// The game's terrains (M1). `None`: the built-in palette, with `soil` the only farmable
    /// ground. Terrain art is resolved into [`SnapshotTile::art_layers`] by
    /// [`crate::apply_graphics`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terrains: Option<Vec<SnapshotTerrain>>,
    /// Rows of tiles: `tiles[y][x]`.
    pub tiles: Vec<Vec<SnapshotTile>>,
    pub npcs: Vec<SnapshotEntity>,
    /// Monsters in the scene (M7).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub monsters: Vec<SnapshotMonster>,
    /// Buildings in the scene (M5).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub buildings: Vec<SnapshotBuilding>,
    /// Lights near the tiles it holds (M8), drawn at night.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lights: Vec<SnapshotLight>,
    pub player: SnapshotPlayer,
    /// Transient overlay effects; omitted under reduced motion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pops: Option<Vec<SnapshotPop>>,
    /// Cutscene emotes over characters (M2); `None` outside cutscenes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emotes: Option<Vec<SnapshotEmote>>,
    /// The tiles actually built, when the snapshot holds only part of the scene
    /// ([`crate::SnapshotOptions::tile_window`]); `None`: every tile. Never serialized.
    #[serde(skip)]
    pub tile_window: Option<TileWindow>,
}

impl Default for WorldSnapshot {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            tile_size: 32.0,
            padding: 0.0,
            tile_gap: None,
            camera: None,
            pixel_art: None,
            grid_overlay: false,
            tick: 0.0,
            atmosphere: None,
            terrains: None,
            tiles: Vec::new(),
            npcs: Vec::new(),
            monsters: Vec::new(),
            buildings: Vec::new(),
            lights: Vec::new(),
            player: SnapshotEntity::default(),
            pops: None,
            emotes: None,
            tile_window: None,
        }
    }
}

impl WorldSnapshot {
    /// The gap between tiles (`None` means 1).
    pub fn gap(&self) -> f64 {
        self.tile_gap.unwrap_or(1.0)
    }

    /// Distance between tile origins (tile size + gap).
    pub fn pitch(&self) -> f64 {
        self.tile_size + self.gap()
    }

    /// Full world size in pixels (what the canvas shows without a camera).
    pub fn world_pixel_size(&self) -> (f64, f64) {
        let pitch = self.pitch();
        let gap = self.gap();
        (
            self.padding * 2.0 + (f64::from(self.width) * pitch) - gap,
            self.padding * 2.0 + (f64::from(self.height) * pitch) - gap,
        )
    }

    /// Viewport size in world pixels: the camera size when present, else the whole world. This is
    /// the canvas size a host allocates (times its zoom).
    pub fn viewport_size(&self) -> (f64, f64) {
        match self.camera {
            Some(camera) => (camera.width, camera.height),
            None => self.world_pixel_size(),
        }
    }

    /// The viewport in whole device pixels at `scale` (at least 1×1), like `RenderToBitmap`.
    pub fn pixel_size(&self, scale: f64) -> (u32, u32) {
        let (width, height) = self.viewport_size();
        let side = |v: f64| {
            let pixels = (v * scale).ceil();
            if pixels.is_nan() || pixels < 1.0 {
                1
            } else if pixels > f64::from(u32::MAX) {
                u32::MAX
            } else {
                pixels as u32
            }
        };
        (side(width), side(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_size_counts_seams_between_tiles_only() {
        let snapshot = WorldSnapshot { width: 2, height: 1, tile_size: 32.0, padding: 0.0, ..Default::default() };
        assert_eq!(snapshot.pitch(), 33.0);
        assert_eq!(snapshot.world_pixel_size(), (65.0, 32.0));
        let play = WorldSnapshot { tile_gap: Some(0.0), padding: 12.0, ..snapshot.clone() };
        assert_eq!(play.world_pixel_size(), (88.0, 56.0));
        let camera = WorldSnapshot {
            camera: Some(SnapshotCamera { x: 1.0, y: 2.0, width: 20.5, height: 10.0 }),
            ..play.clone()
        };
        assert_eq!(camera.viewport_size(), (20.5, 10.0));
        assert_eq!(camera.pixel_size(2.0), (41, 20));
        assert_eq!(WorldSnapshot::default().pixel_size(1.0), (1, 1));
    }

    #[test]
    fn json_uses_the_csharp_names_and_defaults() {
        let json = r##"{"width":1,"height":1,"tileSize":28,"padding":12,"tileGap":1,"gridOverlay":true,"pitch":29,
            "tiles":[[{"background":"soil","artLayers":[null,{"imageUrl":"data:x","frameWidth":1,"frameHeight":1,"frame":0,"row":0},null],
            "node":{"typeId":"node-tree"},"machine":{},"ladderDown":true,"tilled":true}]],
            "npcs":[{"x":1.5,"y":2,"kind":"animal","speciesId":"cow"}],"player":{"x":0,"y":0,"pixelX":3}}"##;
        let snapshot: WorldSnapshot = serde_json::from_str(json).unwrap();
        let tile = &snapshot.tiles[0][0];
        assert_eq!(tile.node.as_ref().unwrap().color, "#7a5a3a");
        assert_eq!(tile.machine.as_ref().unwrap().color, "#9a7b4f");
        assert_eq!(tile.art_layer(1).unwrap().image_url.as_ref(), "data:x");
        assert!(tile.art_layer(0).is_none() && tile.art_layer(5).is_none());
        assert_eq!(snapshot.npcs[0].direction, "down");
        assert_eq!(snapshot.player.kind, "npc");
        assert_eq!(snapshot.player.pixel_x, Some(3.0));
        let back = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(back["tiles"][0][0]["ladderDown"], true);
        assert_eq!(back["tiles"][0][0]["artLayers"][0], serde_json::Value::Null);
        assert!(back.get("camera").is_none() && back.get("pops").is_none());
    }
}
