//! Draw lists to GPU instances: one quad per command, with the CPU rasterizer's choices.
//!
//! - Hard-edged rectangles, anti-aliased axis-aligned rectangles, rounded rectangles, nearest and
//!   bilinear images, glyphs and modulated rectangles are drawn by the shader itself.
//! - Glyphs come from farm-render's own coverage masks ([`raster::rasterize_glyph`]), packed into
//!   an R8 atlas once per font, size and quarter-pixel position.
//! - Every other command (strokes, circles, ovals, blurs, lines, outlined or very large text,
//!   anything under a rotating transform) is rasterized once by farm-render's CPU rasterizer into
//!   a premultiplied sprite, packed into an RGBA atlas and drawn as a quad. The sprite is cached by
//!   the command and the fractional part of its transform, so a shape that only moves by whole
//!   pixels is rasterized once.
//!
//! Clips are scissor rectangles of whole device pixels, like the CPU's clip boxes.

use crate::atlas::{Packer, Slot};
use farm_render::raster::{self, GlyphKey, NearestAxis};
use farm_render::tiny_skia::{Pixmap, Transform};
use farm_render::{Color, DrawCmd, DrawList, ImageId, ImageStore, Rasterizer, Rect, Sampling, TextAlign};
use std::collections::BTreeMap;

pub const SOLID: u32 = 0;
pub const AA_RECT: u32 = 1;
pub const ROUND_RECT: u32 = 2;
pub const IMAGE_NEAREST: u32 = 3;
pub const IMAGE_SMOOTH: u32 = 4;
pub const GLYPH: u32 = 5;
pub const SPRITE: u32 = 6;
pub const TINT: u32 = 7;
pub const UPSCALE: u32 = 8;
pub const COPY: u32 = 9;

/// One quad (the vertex buffer's per-instance layout, 64 bytes).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Instance {
    /// Left, top, right, bottom in device pixels.
    pub rect: [f32; 4],
    /// Kind, colour, tint, extra.
    pub data: [u32; 4],
    pub p0: [f32; 4],
    pub p1: [f32; 4],
}

pub const INSTANCE_SIZE: usize = 64;

impl Instance {
    pub fn write(&self, out: &mut Vec<u8>) {
        for value in self.rect {
            out.extend_from_slice(&value.to_le_bytes());
        }
        for value in self.data {
            out.extend_from_slice(&value.to_le_bytes());
        }
        for value in self.p0.iter().chain(&self.p1) {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
}

/// How a batch blends into its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    /// Premultiplied source-over.
    Over,
    /// `ModulateRect`: the target times the source.
    Modulate,
    /// The source replaces the target (the upscale).
    Replace,
}

/// The texture bound as `image` for a batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImageRef {
    /// An image of an [`ImageStore`]: its uid and id.
    Store(u64, ImageId),
    /// The native world target.
    Native,
}

/// A run of instances drawn with one pipeline, image and scissor rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Batch {
    pub blend: Blend,
    pub image: Option<ImageRef>,
    /// `[x, y, width, height]` in device pixels.
    pub scissor: [u32; 4],
    pub start: u32,
    pub end: u32,
}

/// An atlas had no room for what the frame needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Full {
    Glyphs,
    Sprites,
}

/// A glyph's place in the glyph atlas and its offset from the pen pixel.
#[derive(Debug, Clone, Copy)]
pub struct GlyphSlot {
    pub slot: Slot,
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
}

/// A rasterized shape's place in the sprite atlas and its offset from the whole-pixel part of its
/// transform. `None` slot: the shape drew nothing.
#[derive(Debug, Clone, Copy)]
pub struct SpriteSlot {
    pub slot: Option<Slot>,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Pixels to upload into an atlas before the frame is drawn.
#[derive(Debug)]
pub struct Upload {
    pub slot: Slot,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// The atlases' contents (CPU side) and what the frame must upload.
#[derive(Debug)]
pub struct Atlases {
    pub glyph_packer: Packer,
    pub glyphs: BTreeMap<GlyphKey, Option<GlyphSlot>>,
    pub glyph_uploads: Vec<Upload>,
    pub sprite_packer: Packer,
    pub sprites: BTreeMap<Vec<u32>, SpriteSlot>,
    pub sprite_uploads: Vec<Upload>,
    /// Rasterizes the sprites (its own caches, such as blurred shapes, carry over).
    pub rasterizer: Rasterizer,
}

impl Atlases {
    pub fn new(glyph_size: u32, sprite_size: u32) -> Self {
        Self {
            glyph_packer: Packer::new(glyph_size, glyph_size),
            glyphs: BTreeMap::new(),
            glyph_uploads: Vec::new(),
            sprite_packer: Packer::new(sprite_size, sprite_size),
            sprites: BTreeMap::new(),
            sprite_uploads: Vec::new(),
            rasterizer: Rasterizer::new(),
        }
    }

    pub fn clear_glyphs(&mut self) {
        self.glyph_packer.clear();
        self.glyphs.clear();
        self.glyph_uploads.clear();
    }

    pub fn clear_sprites(&mut self) {
        self.sprite_packer.clear();
        self.sprites.clear();
        self.sprite_uploads.clear();
    }

    fn glyph(&mut self, key: GlyphKey) -> Result<Option<GlyphSlot>, Full> {
        if let Some(entry) = self.glyphs.get(&key) {
            return Ok(*entry);
        }
        let entry = match raster::rasterize_glyph(key) {
            None => None,
            Some((left, top, mask)) => {
                let (width, height) = (mask.width(), mask.height());
                let slot = self.glyph_packer.alloc(width, height).ok_or(Full::Glyphs)?;
                self.glyph_uploads.push(Upload { slot, width, height, data: mask.data().to_vec() });
                Some(GlyphSlot { slot, width, height, left, top })
            }
        };
        self.glyphs.insert(key, entry);
        Ok(entry)
    }
}

/// A clip in device pixels, `[x0, x1) × [y0, y1)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClipBox {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

#[derive(Debug, Clone, Copy)]
struct State {
    transform: Transform,
    clip: Option<ClipBox>,
}

/// Device-space bounding box of a rectangle (raster.rs `map_rect`).
fn map_rect(transform: Transform, rect: &Rect) -> (f32, f32, f32, f32) {
    let corners = [(rect.x, rect.y), (rect.right(), rect.y), (rect.x, rect.bottom()), (rect.right(), rect.bottom())];
    let mut bounds = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for (x, y) in corners {
        let (dx, dy) =
            (transform.sx * x + transform.kx * y + transform.tx, transform.ky * x + transform.sy * y + transform.ty);
        bounds.0 = bounds.0.min(dx);
        bounds.1 = bounds.1.min(dy);
        bounds.2 = bounds.2.max(dx);
        bounds.3 = bounds.3.max(dy);
    }
    bounds
}

fn round_to_int(x: f32) -> i32 {
    (x + 0.5).floor() as i32
}

fn finite(bounds: (f32, f32, f32, f32)) -> bool {
    bounds.0.is_finite() && bounds.1.is_finite() && bounds.2.is_finite() && bounds.3.is_finite()
}

/// Edges of a hard-edged rectangle: the pixels whose centres are inside.
fn center_edge(v: f32) -> f32 {
    (v - 0.5).ceil().clamp(-(1 << 30) as f32, (1 << 30) as f32)
}

fn pack(color: Color) -> u32 {
    u32::from(color.r) | (u32::from(color.g) << 8) | (u32::from(color.b) << 16) | (u32::from(color.a) << 24)
}

/// The paint word of an image draw: the opacity byte, bit 8 for a tint.
fn paint(opacity: f32, tint: Option<Color>) -> u32 {
    let alpha = (opacity.clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u32;
    alpha | if tint.is_some() { 256 } else { 0 }
}

/// Encodes draw lists into instances and batches for targets of one size.
#[derive(Debug)]
pub struct Encoder<'a> {
    pub atlases: &'a mut Atlases,
    pub instances: &'a mut Vec<Instance>,
    pub batches: &'a mut Vec<Batch>,
    /// Images the batches use (each needs a texture before drawing).
    pub images_used: &'a mut Vec<(u64, ImageId)>,
    pub width: u32,
    pub height: u32,
    /// Largest texture side: larger images are drawn through the CPU rasterizer.
    pub max_side: u32,
}

impl Encoder<'_> {
    fn full_scissor(&self) -> [u32; 4] {
        [0, 0, self.width, self.height]
    }

    /// Appends one instance to the last batch when it can share it.
    pub fn push(&mut self, instance: Instance, blend: Blend, image: Option<ImageRef>, scissor: [u32; 4]) {
        let index = self.instances.len() as u32;
        self.instances.push(instance);
        if let Some(last) = self.batches.last_mut() {
            let images_agree = image.is_none() || last.image.is_none() || last.image == image;
            if last.blend == blend && last.scissor == scissor && images_agree && last.end == index {
                last.end = index + 1;
                if last.image.is_none() {
                    last.image = image;
                }
                return;
            }
        }
        self.batches.push(Batch { blend, image, scissor, start: index, end: index + 1 });
    }

    /// The scissor of a clip inside the target; `None` when everything is clipped away.
    fn scissor(&self, clip: Option<ClipBox>) -> Option<[u32; 4]> {
        let Some(clip) = clip else { return Some(self.full_scissor()) };
        let (w, h) = (self.width as i32, self.height as i32);
        let (x0, y0, x1, y1) = (clip.x0.clamp(0, w), clip.y0.clamp(0, h), clip.x1.clamp(0, w), clip.y1.clamp(0, h));
        (x0 < x1 && y0 < y1).then_some([x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32])
    }

    /// Encodes `list` (its images in `images`) with `base` as the initial transform.
    pub fn list(&mut self, list: &DrawList, images: &ImageStore, base: Transform) -> Result<(), Full> {
        let mut state = State { transform: base, clip: None };
        let mut stack: Vec<State> = Vec::new();
        for command in &list.commands {
            match command {
                DrawCmd::Save => stack.push(state),
                DrawCmd::Restore => {
                    if let Some(saved) = stack.pop() {
                        state = saved;
                    }
                }
                DrawCmd::Translate { dx, dy } => state.transform = state.transform.pre_translate(*dx, *dy),
                DrawCmd::Scale { sx, sy } => state.transform = state.transform.pre_scale(*sx, *sy),
                DrawCmd::ClipRect { rect } => {
                    let (left, top, right, bottom) = map_rect(state.transform, rect);
                    let mut clip = ClipBox {
                        x0: round_to_int(left),
                        y0: round_to_int(top),
                        x1: round_to_int(right),
                        y1: round_to_int(bottom),
                    };
                    if let Some(outer) = state.clip {
                        clip = ClipBox {
                            x0: clip.x0.max(outer.x0),
                            y0: clip.y0.max(outer.y0),
                            x1: clip.x1.min(outer.x1),
                            y1: clip.y1.min(outer.y1),
                        };
                    }
                    state.clip = Some(clip);
                }
                other => {
                    let Some(scissor) = self.scissor(state.clip) else { continue };
                    self.command(other, images, state.transform, scissor)?;
                }
            }
        }
        Ok(())
    }

    fn command(
        &mut self,
        command: &DrawCmd,
        images: &ImageStore,
        transform: Transform,
        scissor: [u32; 4],
    ) -> Result<(), Full> {
        let axis_aligned = raster::is_scale_translate(transform);
        match command {
            DrawCmd::FillRect { rect, color, anti_alias } if axis_aligned => {
                if color.a == 0 || (*anti_alias && !(rect.width > 0.0 && rect.height > 0.0)) {
                    return Ok(());
                }
                let bounds = map_rect(transform, rect);
                if !finite(bounds) {
                    return Ok(());
                }
                let (left, top, right, bottom) = bounds;
                if *anti_alias {
                    let quad = [left.floor(), top.floor(), right.ceil(), bottom.ceil()];
                    let instance = Instance {
                        rect: quad,
                        data: [AA_RECT, pack(*color), 0, 0],
                        p0: [left, top, right, bottom],
                        ..Instance::default()
                    };
                    self.push(instance, Blend::Over, None, scissor);
                } else {
                    let quad = [center_edge(left), center_edge(top), center_edge(right), center_edge(bottom)];
                    if quad[0] < quad[2] && quad[1] < quad[3] {
                        let instance =
                            Instance { rect: quad, data: [SOLID, pack(*color), 0, 0], ..Instance::default() };
                        self.push(instance, Blend::Over, None, scissor);
                    }
                }
                Ok(())
            }
            DrawCmd::FillRoundRect { rect, radius, color }
                if axis_aligned && (transform.sx - transform.sy).abs() <= 1e-6 * transform.sx =>
            {
                let (left, top, right, bottom) = map_rect(transform, rect);
                let (width, height) = (right - left, bottom - top);
                if color.a == 0 || !(width > 0.0 && height > 0.0 && width.is_finite() && height.is_finite()) {
                    return Ok(());
                }
                let radius = radius * transform.sx;
                let radius = if radius.is_finite() { radius.clamp(0.0, width.min(height) / 2.0) } else { 0.0 };
                let instance = Instance {
                    rect: [left.floor(), top.floor(), right.ceil(), bottom.ceil()],
                    data: [ROUND_RECT, pack(*color), 0, 0],
                    p0: [left + width / 2.0, top + height / 2.0, width / 2.0 - radius, height / 2.0 - radius],
                    p1: [radius, 0.0, 0.0, 0.0],
                };
                self.push(instance, Blend::Over, None, scissor);
                Ok(())
            }
            DrawCmd::Image { image, src, dst, opacity, sampling, tint } if axis_aligned => {
                let Some(source) = images.image(*image) else { return Ok(()) };
                if source.width() > self.max_side || source.height() > self.max_side {
                    return self.sprite(command, images, transform, scissor);
                }
                if !(dst.width > 0.0 && dst.height > 0.0 && src.width > 0.0 && src.height > 0.0 && *opacity > 0.0) {
                    return Ok(());
                }
                let tint = tint.filter(|tint| *tint != Color::WHITE);
                let image_ref = ImageRef::Store(images.uid(), *image);
                let data = [0, 0, tint.map_or(0, pack), paint(*opacity, tint)];
                match sampling {
                    Sampling::Nearest => {
                        let (Some(x), Some(y)) = (
                            NearestAxis::new(
                                src.x,
                                src.width,
                                dst.x,
                                dst.width,
                                source.width(),
                                transform.sx,
                                transform.tx,
                            ),
                            NearestAxis::new(
                                src.y,
                                src.height,
                                dst.y,
                                dst.height,
                                source.height(),
                                transform.sy,
                                transform.ty,
                            ),
                        ) else {
                            return Ok(());
                        };
                        if x.start >= x.end || y.start >= y.end {
                            return Ok(());
                        }
                        let axis = |a: &NearestAxis| {
                            [a.inv_scale, a.inv_translate, a.subset_origin as f32, a.subset_len as f32]
                        };
                        let instance = Instance {
                            rect: [x.start as f32, y.start as f32, x.end as f32, y.end as f32],
                            data: [IMAGE_NEAREST, data[1], data[2], data[3]],
                            p0: axis(&x),
                            p1: axis(&y),
                        };
                        self.images_used.push((images.uid(), *image));
                        self.push(instance, Blend::Over, Some(image_ref), scissor);
                    }
                    Sampling::Smooth => {
                        let clamp = |v: f32, max: u32| v.max(0.0).min(max as f32) as u32;
                        let (x0, y0) = (clamp(src.x.floor(), source.width()), clamp(src.y.floor(), source.height()));
                        let (x1, y1) =
                            (clamp(src.right().ceil(), source.width()), clamp(src.bottom().ceil(), source.height()));
                        if x0 >= x1 || y0 >= y1 {
                            return Ok(());
                        }
                        let bounds = map_rect(transform, dst);
                        if !finite(bounds) {
                            return Ok(());
                        }
                        // Device pixel → image texel: undo the canvas transform, then dst → src.
                        let (scale_x, scale_y) = (dst.width / src.width, dst.height / src.height);
                        let a_x = 1.0 / (transform.sx * scale_x);
                        let a_y = 1.0 / (transform.sy * scale_y);
                        let b_x = src.x - (transform.tx / transform.sx + dst.x) / scale_x;
                        let b_y = src.y - (transform.ty / transform.sy + dst.y) / scale_y;
                        let quad = [
                            center_edge(bounds.0),
                            center_edge(bounds.1),
                            center_edge(bounds.2),
                            center_edge(bounds.3),
                        ];
                        if quad[0] >= quad[2] || quad[1] >= quad[3] {
                            return Ok(());
                        }
                        let instance = Instance {
                            rect: quad,
                            data: [IMAGE_SMOOTH, data[1], data[2], data[3]],
                            p0: [a_x, b_x, a_y, b_y],
                            p1: [x0 as f32, y0 as f32, x1 as f32, y1 as f32],
                        };
                        self.images_used.push((images.uid(), *image));
                        self.push(instance, Blend::Over, Some(image_ref), scissor);
                    }
                }
                Ok(())
            }
            DrawCmd::ModulateRect { rect, color } => {
                if !(rect.width > 0.0 && rect.height > 0.0) {
                    return Ok(());
                }
                let bounds = map_rect(transform, rect);
                if !finite(bounds) {
                    return Ok(());
                }
                let quad = [center_edge(bounds.0), center_edge(bounds.1), center_edge(bounds.2), center_edge(bounds.3)];
                if quad[0] < quad[2] && quad[1] < quad[3] {
                    let instance = Instance { rect: quad, data: [SOLID, pack(*color), 0, 0], ..Instance::default() };
                    self.push(instance, Blend::Modulate, None, scissor);
                }
                Ok(())
            }
            DrawCmd::Text { text, x, y, font, size, color, stroke, align } => {
                if size.is_nan() || *size <= 0.0 || text.is_empty() {
                    return Ok(());
                }
                match raster::sprite_text_layout(text, *x, *y, *font, *size, stroke.is_some(), *align, transform) {
                    Some(glyphs) => {
                        if color.a == 0 {
                            return Ok(());
                        }
                        for placement in glyphs {
                            let Some(glyph) = self.atlases.glyph(placement.key)? else { continue };
                            let (left, top) = (placement.column + glyph.left, placement.row + glyph.top);
                            let instance = Instance {
                                rect: [
                                    left as f32,
                                    top as f32,
                                    (left + glyph.width as i32) as f32,
                                    (top + glyph.height as i32) as f32,
                                ],
                                data: [GLYPH, pack(*color), 0, 0],
                                p0: [glyph.slot.x as f32, glyph.slot.y as f32, 0.0, 0.0],
                                ..Instance::default()
                            };
                            self.push(instance, Blend::Over, None, scissor);
                        }
                        Ok(())
                    }
                    None => self.sprite(command, images, transform, scissor),
                }
            }
            DrawCmd::Save
            | DrawCmd::Restore
            | DrawCmd::Translate { .. }
            | DrawCmd::Scale { .. }
            | DrawCmd::ClipRect { .. } => Ok(()),
            _ => self.sprite(command, images, transform, scissor),
        }
    }

    /// Draws a command through the CPU rasterizer: rasterized once into the sprite atlas (keyed by
    /// the command, the fractional part of the transform and the region drawn), then a quad.
    fn sprite(
        &mut self,
        command: &DrawCmd,
        images: &ImageStore,
        transform: Transform,
        scissor: [u32; 4],
    ) -> Result<(), Full> {
        let Some(local) = local_bounds(command) else { return Ok(()) };
        if !(transform.tx.is_finite() && transform.ty.is_finite())
            || transform.tx.abs() > (1 << 24) as f32
            || transform.ty.abs() > (1 << 24) as f32
        {
            return Ok(());
        }
        let (whole_x, whole_y) = (transform.tx.floor(), transform.ty.floor());
        let fraction = Transform::from_row(
            transform.sx,
            transform.ky,
            transform.kx,
            transform.sy,
            transform.tx - whole_x,
            transform.ty - whole_y,
        );
        let mut bounds = map_rect(fraction, &local);
        if let DrawCmd::BlurOval { sigma, .. } | DrawCmd::BlurRect { sigma, .. } = command {
            let (sx, sy) = fraction.get_scale();
            let pad = (3.0 * sigma.abs() * (sx * sy).abs().sqrt()).ceil();
            bounds = (bounds.0 - pad, bounds.1 - pad, bounds.2 + pad, bounds.3 + pad);
        }
        if !finite(bounds) {
            return Ok(());
        }
        // The region drawn, relative to the whole-pixel translation: the shape's bounds (padded),
        // cut to the scissor.
        let (whole_x, whole_y) = (whole_x as i64, whole_y as i64);
        let clamp = |v: f32| (v as i64).clamp(-(1 << 30), 1 << 30);
        let region_x0 = (clamp(bounds.0.floor()) - 2).max(i64::from(scissor[0]) - whole_x);
        let region_y0 = (clamp(bounds.1.floor()) - 2).max(i64::from(scissor[1]) - whole_y);
        let region_x1 = (clamp(bounds.2.ceil()) + 2).min(i64::from(scissor[0] + scissor[2]) - whole_x);
        let region_y1 = (clamp(bounds.3.ceil()) + 2).min(i64::from(scissor[1] + scissor[3]) - whole_y);
        if region_x0 >= region_x1 || region_y0 >= region_y1 {
            return Ok(());
        }
        let (width, height) = ((region_x1 - region_x0) as u32, (region_y1 - region_y0) as u32);
        let (region_x0, region_y0) = (region_x0 as i32, region_y0 as i32);
        let mut key = command_key(command, images);
        for value in [fraction.sx, fraction.ky, fraction.kx, fraction.sy, fraction.tx, fraction.ty] {
            key.push(value.to_bits());
        }
        key.extend([region_x0 as u32, region_y0 as u32, width, height]);
        let entry = match self.atlases.sprites.get(&key) {
            Some(entry) => *entry,
            None => {
                let (atlas_w, atlas_h) = self.atlases.sprite_packer.size();
                if width > atlas_w || height > atlas_h {
                    return Err(Full::Sprites);
                }
                let mut pixmap = Pixmap::new(width, height).ok_or(Full::Sprites)?;
                let list = DrawList { commands: vec![command.clone()] };
                let base = Transform::from_row(
                    fraction.sx,
                    fraction.ky,
                    fraction.kx,
                    fraction.sy,
                    fraction.tx - region_x0 as f32,
                    fraction.ty - region_y0 as f32,
                );
                self.atlases.rasterizer.render(&list, images, &mut pixmap.as_mut(), base);
                let slot = if pixmap.data().iter().all(|byte| *byte == 0) {
                    None
                } else {
                    let slot = self.atlases.sprite_packer.alloc(width, height).ok_or(Full::Sprites)?;
                    self.atlases.sprite_uploads.push(Upload { slot, width, height, data: pixmap.take() });
                    Some(slot)
                };
                let entry = SpriteSlot { slot, x: region_x0, y: region_y0, width, height };
                self.atlases.sprites.insert(key, entry);
                entry
            }
        };
        let Some(slot) = entry.slot else { return Ok(()) };
        let (left, top) = (whole_x + i64::from(entry.x), whole_y + i64::from(entry.y));
        let instance = Instance {
            rect: [
                left as f32,
                top as f32,
                (left + i64::from(entry.width)) as f32,
                (top + i64::from(entry.height)) as f32,
            ],
            data: [SPRITE, 0, 0, 0],
            p0: [slot.x as f32, slot.y as f32, 0.0, 0.0],
            ..Instance::default()
        };
        self.push(instance, Blend::Over, None, scissor);
        Ok(())
    }
}

/// A conservative local bounding box of a command (before its stroke or blur reaches out).
fn local_bounds(command: &DrawCmd) -> Option<Rect> {
    let grow =
        |rect: &Rect, by: f32| Rect::new(rect.x - by, rect.y - by, rect.width + 2.0 * by, rect.height + 2.0 * by);
    let rect = match command {
        DrawCmd::FillRect { rect, .. }
        | DrawCmd::FillRoundRect { rect, .. }
        | DrawCmd::FillOval { rect, .. }
        | DrawCmd::BlurOval { rect, .. }
        | DrawCmd::BlurRect { rect, .. }
        | DrawCmd::ModulateRect { rect, .. } => *rect,
        DrawCmd::StrokeRect { rect, width, .. } | DrawCmd::StrokeRoundRect { rect, width, .. } => {
            grow(rect, width.abs())
        }
        DrawCmd::FillCircle { cx, cy, radius, .. } => grow(&Rect::new(*cx, *cy, 0.0, 0.0), radius.abs()),
        DrawCmd::StrokeCircle { cx, cy, radius, width, .. } => {
            grow(&Rect::new(*cx, *cy, 0.0, 0.0), radius.abs() + width.abs())
        }
        DrawCmd::Line { x0, y0, x1, y1, width, .. } => {
            grow(&Rect::new(x0.min(*x1), y0.min(*y1), (x1 - x0).abs(), (y1 - y0).abs()), width.abs())
        }
        DrawCmd::Image { dst, .. } => *dst,
        DrawCmd::Text { text, x, y, font, size, stroke, align, .. } => {
            let face = farm_render::text::font(*font);
            let width = face.measure(text, *size);
            let start = match align {
                TextAlign::Left => *x,
                TextAlign::Center => x - width / 2.0,
                TextAlign::Right => x - width,
            };
            let outline = stroke.map_or(0.0, |stroke| stroke.width.abs());
            let reach = size.abs() + outline;
            Rect::new(
                start - reach,
                y - face.ascent(*size) - reach,
                width + 2.0 * reach,
                face.ascent(*size) + face.descent(*size) + 2.0 * reach,
            )
        }
        DrawCmd::Save
        | DrawCmd::Restore
        | DrawCmd::Translate { .. }
        | DrawCmd::Scale { .. }
        | DrawCmd::ClipRect { .. } => return None,
    };
    (rect.width.is_finite() && rect.height.is_finite() && rect.width >= 0.0 && rect.height >= 0.0).then_some(rect)
}

/// Every field of a command, as bits (the sprite cache key without the transform and region).
fn command_key(command: &DrawCmd, images: &ImageStore) -> Vec<u32> {
    let mut key = Vec::with_capacity(24);
    let rect =
        |key: &mut Vec<u32>, rect: &Rect| key.extend([rect.x, rect.y, rect.width, rect.height].map(f32::to_bits));
    let color = |key: &mut Vec<u32>, color: &Color| key.push(pack(*color));
    match command {
        DrawCmd::FillRect { rect: r, color: c, anti_alias } => {
            key.extend([1, u32::from(*anti_alias)]);
            rect(&mut key, r);
            color(&mut key, c);
        }
        DrawCmd::StrokeRect { rect: r, color: c, width } => {
            key.extend([2, width.to_bits()]);
            rect(&mut key, r);
            color(&mut key, c);
        }
        DrawCmd::FillRoundRect { rect: r, radius, color: c } => {
            key.extend([3, radius.to_bits()]);
            rect(&mut key, r);
            color(&mut key, c);
        }
        DrawCmd::StrokeRoundRect { rect: r, radius, color: c, width } => {
            key.extend([4, radius.to_bits(), width.to_bits()]);
            rect(&mut key, r);
            color(&mut key, c);
        }
        DrawCmd::FillCircle { cx, cy, radius, color: c } => {
            key.extend([5, cx.to_bits(), cy.to_bits(), radius.to_bits()]);
            color(&mut key, c);
        }
        DrawCmd::StrokeCircle { cx, cy, radius, color: c, width } => {
            key.extend([6, cx.to_bits(), cy.to_bits(), radius.to_bits(), width.to_bits()]);
            color(&mut key, c);
        }
        DrawCmd::FillOval { rect: r, color: c } => {
            key.push(7);
            rect(&mut key, r);
            color(&mut key, c);
        }
        DrawCmd::BlurOval { rect: r, color: c, sigma } => {
            key.extend([8, sigma.to_bits()]);
            rect(&mut key, r);
            color(&mut key, c);
        }
        DrawCmd::BlurRect { rect: r, color: c, sigma } => {
            key.extend([9, sigma.to_bits()]);
            rect(&mut key, r);
            color(&mut key, c);
        }
        DrawCmd::Line { x0, y0, x1, y1, color: c, width } => {
            key.extend([10, x0.to_bits(), y0.to_bits(), x1.to_bits(), y1.to_bits(), width.to_bits()]);
            color(&mut key, c);
        }
        DrawCmd::Image { image, src, dst, opacity, sampling, tint } => {
            let uid = images.uid();
            key.extend([11, uid as u32, (uid >> 32) as u32, image.0, opacity.to_bits(), *sampling as u32]);
            key.extend([u32::from(tint.is_some()), tint.map_or(0, pack)]);
            rect(&mut key, src);
            rect(&mut key, dst);
        }
        DrawCmd::ModulateRect { rect: r, color: c } => {
            key.push(12);
            rect(&mut key, r);
            color(&mut key, c);
        }
        DrawCmd::Text { text, x, y, font, size, color: c, stroke, align } => {
            key.extend([13, x.to_bits(), y.to_bits(), *font as u32, size.to_bits(), *align as u32]);
            color(&mut key, c);
            match stroke {
                Some(stroke) => key.extend([1, pack(stroke.color), stroke.width.to_bits()]),
                None => key.push(0),
            }
            key.push(text.chars().count() as u32);
            key.extend(text.chars().map(u32::from));
        }
        DrawCmd::Save
        | DrawCmd::Restore
        | DrawCmd::Translate { .. }
        | DrawCmd::Scale { .. }
        | DrawCmd::ClipRect { .. } => key.push(0),
    }
    key
}
