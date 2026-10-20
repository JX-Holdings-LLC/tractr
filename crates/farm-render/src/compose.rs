//! How a player frame is composed from its layers, independent of the renderer that executes it:
//! the world drawn at its native pixel-art size, tinted, scaled up with nearest neighbour into a
//! letterboxed rectangle of the frame, then the UI drawn at full resolution on top.
//!
//! The CPU path (`farm-player`'s `FrameRenderer`) and the GPU backend (`farm-gpu`) both execute a
//! [`Composition`] with the arithmetic here, so they pick the same source pixels and tint colours.

use crate::css_color::Color;
use crate::draw::DrawList;
use crate::images::ImageStore;

/// One frame to draw.
#[derive(Debug, Clone, Copy)]
pub struct Composition<'a> {
    /// Frame size in device pixels.
    pub width: u32,
    pub height: u32,
    /// Fills the frame first (the letterbox around a small scene).
    pub letterbox: Color,
    pub world: Option<WorldLayer<'a>>,
    /// Screen-space commands drawn at full resolution over the world.
    pub ui: &'a DrawList,
    pub ui_images: &'a ImageStore,
}

/// The world: a draw list rendered at `native_width`×`native_height`, then scaled into the frame.
#[derive(Debug, Clone, Copy)]
pub struct WorldLayer<'a> {
    pub list: &'a DrawList,
    pub images: &'a ImageStore,
    pub native_width: u32,
    pub native_height: u32,
    /// Fills the native world first (outside the scene).
    pub background: Color,
    /// Vertical gradients (top, bottom colour) over the frame's height, blended into the native
    /// world before it is scaled up ([`tint_source`], [`Upscale::tint_position`]).
    pub tints: &'a [(Color, Color)],
    pub upscale: Upscale,
}

/// How the native world maps onto the frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Upscale {
    /// Device pixels per world pixel.
    pub zoom: f64,
    /// The camera viewport in world pixels.
    pub view: (f64, f64),
    /// Where the viewport's top-left lands in the frame (device pixels).
    pub origin: (f64, f64),
    /// The camera's position inside its first native pixel (smooth scrolling).
    pub fraction: (f64, f64),
}

impl Upscale {
    /// The frame rectangle the world covers, `(x0, y0, width, height)` in device pixels.
    pub fn destination(&self, frame_width: u32, frame_height: u32) -> (usize, usize, usize, usize) {
        let (frame_w, frame_h) = (frame_width as usize, frame_height as usize);
        let dest_w = ((self.view.0 * self.zoom).round_ties_even() as usize).min(frame_w);
        let dest_h = ((self.view.1 * self.zoom).round_ties_even() as usize).min(frame_h);
        let (x0, y0) = (self.origin.0.max(0.0) as usize, self.origin.1.max(0.0) as usize);
        let dest_w = dest_w.min(frame_w - x0.min(frame_w));
        let dest_h = dest_h.min(frame_h - y0.min(frame_h));
        (x0, y0, dest_w, dest_h)
    }

    /// The native column sampled for device column `device` of the destination (0 = its left
    /// edge), among `native` columns.
    pub fn source_column(&self, device: usize, native: usize) -> usize {
        source_index(device, self.zoom, self.fraction.0, native)
    }

    /// The native row sampled for device row `device` of the destination.
    pub fn source_row(&self, device: usize, native: usize) -> usize {
        source_index(device, self.zoom, self.fraction.1, native)
    }

    /// Where native row `row` lands on screen, as a fraction of the frame's height (its centre;
    /// 0 = top, 1 = bottom): the position of the tints' gradients.
    pub fn tint_position(&self, row: usize, frame_height: u32) -> f32 {
        ((self.origin.1 + (row as f64 + 0.5 - self.fraction.1) * self.zoom) / f64::from(frame_height)).clamp(0.0, 1.0)
            as f32
    }
}

fn source_index(device: usize, zoom: f64, fraction: f64, limit: usize) -> usize {
    (((device as f64 + 0.5) / zoom + fraction).floor() as usize).min(limit.max(1) - 1)
}

/// The premultiplied colour a tint blends source-over at gradient position `t` (0 = `top`,
/// 1 = `bottom`), or `None` when it is transparent there.
pub fn tint_source(top: Color, bottom: Color, t: f32) -> Option<[u32; 4]> {
    let lerp = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t + 0.5) as u32;
    let alpha = lerp(top.a, bottom.a).min(255);
    if alpha == 0 {
        return None;
    }
    let channel = |a: u8, b: u8| (lerp(a, b).min(255) * alpha + 127) / 255;
    Some([channel(top.r, bottom.r), channel(top.g, bottom.g), channel(top.b, bottom.b), alpha])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upscale_maps_device_pixels_to_native_ones() {
        let upscale = Upscale { zoom: 2.0, view: (4.0, 2.0), origin: (0.0, 0.0), fraction: (0.0, 0.0) };
        assert_eq!(upscale.destination(8, 4), (0, 0, 8, 4));
        let columns: Vec<usize> = (0..8).map(|x| upscale.source_column(x, 5)).collect();
        assert_eq!(columns, [0, 0, 1, 1, 2, 2, 3, 3]);
        let shifted = Upscale { fraction: (0.5, 0.0), ..upscale };
        let columns: Vec<usize> = (0..8).map(|x| shifted.source_column(x, 5)).collect();
        assert_eq!(columns, [0, 1, 1, 2, 2, 3, 3, 4]);
        // Letterboxed: the destination is the scene's size, centred by its origin.
        let small = Upscale { zoom: 2.0, view: (344.0, 280.0), origin: (296.0, 120.0), fraction: (0.0, 0.0) };
        assert_eq!(small.destination(1280, 800), (296, 120, 688, 560));
        assert_eq!(small.tint_position(0, 800), ((120.0 + 1.0) / 800.0) as f32);
    }

    #[test]
    fn tints_are_premultiplied_and_skip_transparent_rows() {
        let top = Color::rgba(0, 0, 0, 0);
        let bottom = Color::rgba(200, 100, 50, 255);
        assert_eq!(tint_source(top, bottom, 0.0), None);
        assert_eq!(tint_source(top, bottom, 1.0), Some([200, 100, 50, 255]));
        let [r, g, b, a] = tint_source(top, bottom, 0.5).unwrap();
        assert_eq!(a, 128);
        assert!(r <= a && g <= a && b <= a);
    }
}
