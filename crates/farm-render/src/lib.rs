//! `farm-render`: what the game world looks like, as data, plus a CPU rasterizer.
//!
//! This crate is the single implementation of *what to draw* for a game world (a port of the retired
//! C# renderer). It never writes simulation state; rendering may use floats.
//!
//! The pipeline, per frame:
//!
//! 1. **Snapshot**: [`shell_snapshot`] (play: content + live state) or [`editor_snapshot`] (Edit
//!    Mode: the project) builds a plain-data [`WorldSnapshot`].
//! 2. **Art**: [`apply_graphics`] decorates it with the creator's bound art ([`GraphicsSource`],
//!    [`resolve_visual`] for animation frames).
//! 3. **Draw list**: [`build_world`] turns it into a [`DrawList`] of [`DrawCmd`]s (ground,
//!    shadows, y-sorted objects and entities, atmosphere, weather, pops, grid), using the embedded
//!    [`BuiltinArt`] pack for anything without art and an [`ImageStore`] for decoded images. A UI
//!    layer appends its own screen-space commands after the world.
//! 4. **Pixels** (feature `raster`, on by default): a [`Rasterizer`] executes the list into a
//!    [`tiny_skia::Pixmap`]. [`WorldRenderer`] bundles the caches for a host that renders every
//!    frame; [`render_to_pixmap`] and [`encode_png`] are one-shot helpers.
//!
//! [`text`] has the embedded Inter fonts and the measurement a UI layout needs.
//!
//! Supporting modules: [`atmosphere`] (day/night tint, seasons, weather overlays), [`canvas2d`]
//! (tile colors, the follow camera, entity positions) and [`css_color`] (CSS color parsing).
#![forbid(unsafe_code)]

pub mod atmosphere;
pub mod builtin_art;
pub mod canvas2d;
pub mod compose;
pub mod css_color;
pub mod draw;
pub mod graphics;
pub mod images;
pub mod look;
mod num;
#[cfg(feature = "raster")]
pub mod raster;
pub mod shell;
pub mod snapshot;
pub mod text;
pub mod world;

pub use builtin_art::{BuiltinArt, BuiltinArtEntry, BuiltinArtManifest};
pub use canvas2d::{compute_camera, entity_pixel_origin};
pub use compose::{Composition, Upscale, WorldLayer};
pub use css_color::Color;
pub use draw::{
    light_add, light_factor, light_round, DrawCmd, DrawList, FontId, LightSpot, Rect, Sampling, TextAlign, TextStroke,
    MAX_LIGHTS,
};
pub use graphics::{apply_graphics, resolve_visual, ArtAsset, GraphicsSource};
pub use images::{Image, ImageError, ImageId, ImageStore};
#[cfg(feature = "raster")]
pub use raster::{device_lights, encode_png, render_to_pixmap, Rasterizer, WorldRenderer};
pub use shell::{editor_snapshot, preview_atmosphere, scene_atmosphere, shell_snapshot, SkyPreview, SnapshotOptions};
pub use snapshot::{
    SnapshotAtmosphere, SnapshotBuilding, SnapshotCamera, SnapshotCrop, SnapshotDecor, SnapshotEmote, SnapshotEntity,
    SnapshotItem, SnapshotLight, SnapshotMachine, SnapshotMonster, SnapshotNode, SnapshotPlayer, SnapshotPop,
    SnapshotSprite, SnapshotTerrain, SnapshotTile, TileWindow, WorldSnapshot, MAX_TALL,
};
#[cfg(feature = "raster")]
pub use tiny_skia;
pub use world::{build_world, build_world_into};
