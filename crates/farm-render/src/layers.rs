//! Layered characters (#234): a character drawn as a stack of layer sheets (body, eyes, hair,
//! clothes…) plus the layers of what it wears, each optionally multiplied with a colour.
//!
//! Drawing every layer every frame would cost a draw command per layer per character, so the
//! stack is composed into one image per frame instead and cached in the [`ImageStore`] under a key
//! naming the layer images, their source rectangles and colours. A walk cycle composes each of its
//! frames once; later frames are a cache hit and draw like any single sprite (on the CPU and the
//! GPU alike: both only ever see the composed image). Changing what is worn changes the key, so the
//! old frames simply age out of the cache.
//!
//! Layers of different sizes are stacked standing on the same bottom edge, centred: the composed
//! frame is as wide and as tall as the largest layer.

use crate::css_color::{self, Color};
use crate::images::{Image, ImageId, ImageStore};
use std::fmt::Write as _;

/// One layer of a frame to compose: a rectangle of a decoded image and the colour it is
/// multiplied with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerPart {
    pub image: ImageId,
    /// Source rectangle in whole pixels: x, y, width, height.
    pub src: (u32, u32, u32, u32),
    pub tint: Option<Color>,
}

/// The tint of a layer colour (`#rrggbb`); `None` for none or white (no change).
pub fn tint_of(color: Option<&str>) -> Option<Color> {
    let color = color.filter(|color| !color.is_empty())?;
    let parsed = css_color::parse(Some(color));
    (parsed != Color::WHITE).then_some(parsed)
}

/// The size of the composed frame: the largest layer's width and height.
pub fn composed_size(parts: &[LayerPart]) -> (u32, u32) {
    parts.iter().fold((0, 0), |(w, h), part| (w.max(part.src.2), h.max(part.src.3)))
}

/// The cache key of a composed frame. Image ids are the store's own, so a key never outlives
/// the images it was composed from (a re-decoded layer gets a new id and so a new key).
pub fn composite_key(parts: &[LayerPart]) -> String {
    let mut key = String::from("layers:");
    for part in parts {
        let (x, y, w, h) = part.src;
        let _ = write!(key, "{}@{x},{y},{w},{h}", part.image.0);
        if let Some(tint) = part.tint {
            let _ = write!(key, "*{:08x}", tint.to_u32());
        }
        key.push('|');
    }
    key
}

/// Composes `parts` bottom to top (source-over), each multiplied with its tint; `None` when no
/// part can be read.
pub fn compose(store: &ImageStore, parts: &[LayerPart]) -> Option<Image> {
    let (width, height) = composed_size(parts);
    if width == 0 || height == 0 {
        return None;
    }
    let mut out = vec![0u8; (width as usize) * (height as usize) * 4];
    let mut any = false;
    for part in parts {
        let Some(image) = store.image(part.image) else { continue };
        let (sx, sy, sw, sh) = part.src;
        if sx + sw > image.width() || sy + sh > image.height() {
            continue;
        }
        any = true;
        let ox = (width - sw) / 2;
        let oy = height - sh;
        let pixels = image.pixels();
        for y in 0..sh {
            for x in 0..sw {
                let from = (((sy + y) * image.width() + sx + x) * 4) as usize;
                let mut src = [pixels[from], pixels[from + 1], pixels[from + 2], pixels[from + 3]];
                if src[3] == 0 {
                    continue;
                }
                if let Some(tint) = part.tint {
                    src[0] = mul_255(src[0], tint.r);
                    src[1] = mul_255(src[1], tint.g);
                    src[2] = mul_255(src[2], tint.b);
                }
                let to = ((((oy + y) * width) + ox + x) * 4) as usize;
                let dst = &mut out[to..to + 4];
                // Premultiplied source-over.
                let inverse = 255 - src[3];
                for channel in 0..4 {
                    dst[channel] = src[channel].saturating_add(mul_255(dst[channel], inverse));
                }
            }
        }
    }
    any.then(|| Image::from_premultiplied(width, height, out)).flatten()
}

/// The composed frame of `parts`, from the cache or composed now (and cached).
pub fn composed(store: &mut ImageStore, parts: &[LayerPart]) -> Option<ImageId> {
    let key = composite_key(parts);
    store.get_or_insert_with(&key, |store| compose(store, parts))
}

/// `a × b / 255`, rounded.
fn mul_255(a: u8, b: u8) -> u8 {
    let product = u32::from(a) * u32::from(b) + 128;
    (((product >> 8) + product) >> 8) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_with(images: &[(&str, u32, u32, [u8; 4])]) -> (ImageStore, Vec<ImageId>) {
        let mut store = ImageStore::new(16);
        let ids = images
            .iter()
            .map(|(source, w, h, rgba)| {
                let pixels = rgba.iter().copied().cycle().take((*w * *h * 4) as usize).collect();
                store.insert(source, Image::from_premultiplied(*w, *h, pixels).unwrap())
            })
            .collect();
        (store, ids)
    }

    #[test]
    fn stacks_layers_on_the_bottom_edge_and_tints_them() {
        let (mut store, ids) = store_with(&[("body", 4, 6, [10, 20, 30, 255]), ("hat", 2, 2, [200, 200, 200, 255])]);
        let parts = [
            LayerPart { image: ids[0], src: (0, 0, 4, 6), tint: None },
            LayerPart { image: ids[1], src: (0, 0, 2, 2), tint: tint_of(Some("#ff0000")) },
        ];
        let id = composed(&mut store, &parts).unwrap();
        let image = store.image(id).unwrap();
        assert_eq!((image.width(), image.height()), (4, 6));
        let px = |x: u32, y: u32| &image.pixels()[((y * 4 + x) * 4) as usize..((y * 4 + x) * 4 + 4) as usize];
        assert_eq!(px(0, 0), &[10, 20, 30, 255], "the body shows where the hat is not");
        assert_eq!(px(1, 4), &[200, 0, 0, 255], "the hat stands on the bottom edge, centred, tinted red");
        // The second lookup is a cache hit: same image, nothing composed again.
        let entries = store.len();
        assert_eq!(composed(&mut store, &parts), Some(id));
        assert_eq!(store.len(), entries);
        // Another colour is another frame.
        let blue = [parts[0], LayerPart { tint: tint_of(Some("#0000ff")), ..parts[1] }];
        assert_ne!(composed(&mut store, &blue), Some(id));
    }

    #[test]
    fn white_and_missing_colours_do_not_tint() {
        assert_eq!(tint_of(None), None);
        assert_eq!(tint_of(Some("")), None);
        assert_eq!(tint_of(Some("#ffffff")), None);
        assert_eq!(mul_255(255, 255), 255);
        assert_eq!(mul_255(255, 0), 0);
        assert_eq!(mul_255(128, 255), 128);
    }
}
