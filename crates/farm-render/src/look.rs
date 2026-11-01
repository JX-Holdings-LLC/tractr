//! The player character's look (M4): the built-in player art recoloured with the character
//! creator's colours and the clothes worn.
//!
//! The built-in character sheet is drawn with a small palette, so each part of the player has
//! its own shades: hair, skin, shirt, trousers and boots, and the headband (the accent). A look
//! is a key of five colours (`skin,hair,shirt,pants,accent`, `-` for "as drawn"); the sprite's
//! source becomes `builtin://characters.png#look=<key>`, which [`crate::ImageStore`] decodes as
//! the sheet with the player's frames recoloured: every shade of a part is the chosen colour
//! scaled by how light that shade is next to the part's main shade, so shading survives.

use crate::images::Image;
use crate::snapshot::SnapshotSprite;
use farm_sim::schema::{wear_slots, CharacterLayer, InventorySlot, PlayerLook};
use std::sync::Arc;

/// What separates a sheet's source from its look key.
const MARKER: &str = "#look=";

/// The parts of the player, in key order.
const PARTS: usize = 5;
const SKIN: usize = 0;
const HAIR: usize = 1;
const SHIRT: usize = 2;
const PANTS: usize = 3;
const ACCENT: usize = 4;

/// The shades of each part in the built-in player art; the first is the part's main shade.
const SKIN_SHADES: &[u32] = &[0xf2c9a0, 0xe0c091, 0xc49a6a, 0xa07350];
const BROWN_SHADES: &[u32] = &[0x5c3d28, 0x3d2a1e, 0x4e3526, 0x7c563a];
const SHIRT_SHADES: &[u32] = &[0x4f9be0, 0x2f6db5, 0x1e3a6e];
const ACCENT_SHADES: &[u32] = &[0xc23a3a, 0x7a1f2b];

/// Rows of a 48-pixel frame from which brown is trousers and boots rather than hair.
const LEGS_FROM_ROW: u32 = 34;

/// The look key of the player's colours and worn clothes: a worn shirt or trousers colour that
/// part, a worn hat without art colours the headband. `None` when nothing changes.
pub fn look_key(look: Option<&PlayerLook>, worn: &[InventorySlot]) -> Option<String> {
    let mut parts: [Option<String>; PARTS] = Default::default();
    if let Some(look) = look {
        parts[SKIN].clone_from(&look.skin);
        parts[HAIR].clone_from(&look.hair);
        parts[SHIRT].clone_from(&look.shirt);
        parts[PANTS].clone_from(&look.pants);
        parts[ACCENT].clone_from(&look.accent);
    }
    for slot in worn {
        let Some(wear) = &slot.item.wear else { continue };
        let Some(color) = &wear.color else { continue };
        match wear.slot.as_str() {
            wear_slots::SHIRT => parts[SHIRT] = Some(color.clone()),
            wear_slots::PANTS => parts[PANTS] = Some(color.clone()),
            wear_slots::HAT if wear.visual.is_none() => parts[ACCENT] = Some(color.clone()),
            _ => {}
        }
    }
    let colors: Vec<Option<u32>> = parts.iter().map(|part| part.as_deref().and_then(parse_hex)).collect();
    if colors.iter().all(Option::is_none) {
        return None;
    }
    Some(
        colors
            .iter()
            .map(|color| color.map_or_else(|| "-".to_owned(), |c| format!("{c:06x}")))
            .collect::<Vec<_>>()
            .join(","),
    )
}

/// The layers (#234) of the clothes worn, bottom to top: trousers, shirt, hat (then any other
/// slot, in the order worn). Each is tinted with the item's colour. `None` when nothing worn has
/// a layer.
pub fn worn_layers(worn: &[InventorySlot]) -> Option<Vec<CharacterLayer>> {
    let rank = |slot: &str| match slot {
        wear_slots::PANTS => 0,
        wear_slots::SHIRT => 1,
        wear_slots::HAT => 2,
        _ => 3,
    };
    let mut layers: Vec<(i32, CharacterLayer)> = worn
        .iter()
        .filter_map(|slot| {
            let wear = slot.item.wear.as_ref()?;
            let asset_id = wear.layer.clone().filter(|id| !id.is_empty())?;
            Some((rank(&wear.slot), CharacterLayer { asset_id, color: wear.color.clone() }))
        })
        .collect();
    layers.sort_by_key(|(rank, _)| *rank);
    (!layers.is_empty()).then(|| layers.into_iter().map(|(_, layer)| layer).collect())
}

/// Built-in NPC appearances drawn as the base character in fixed colours (#216), on top of the
/// hand-drawn `farmer`, `merchant` and `villager`: `(appearance, skin, hair, shirt, trousers,
/// accent)`. An appearance containing one of these names picks it.
pub const APPEARANCE_LOOKS: &[(&str, [&str; PARTS])] = &[
    ("fisher", ["e0b088", "c8c8c8", "2f7f7a", "3c4a5c", "f2c14e"]),
    ("miner", ["c08a5c", "2b2138", "7a6a5a", "4a3b2a", "f28c3b"]),
    ("baker", ["f2c9a0", "e8d080", "f4f0e8", "8a5a3a", "d95f8e"]),
    ("doctor", ["a07350", "1e1e28", "eef0f5", "3a4a6a", "4f9be0"]),
    ("blacksmith", ["c49a6a", "3d2a1e", "5a5a6b", "2b2b38", "a3a3b1"]),
    ("noble", ["f2c9a0", "f2e0a0", "7a3fa0", "2b2138", "f7b24a"]),
    ("gardener", ["d9a074", "7c563a", "6fae4a", "5c4a32", "f4a2c2"]),
    ("sailor", ["e0b088", "3d2a1e", "eef0f5", "1e3a6e", "c23a3a"]),
];

/// The built-in colours of an appearance (#216), when it names one of [`APPEARANCE_LOOKS`].
pub fn appearance_look(appearance: Option<&str>) -> Option<PlayerLook> {
    let id = appearance?.to_lowercase();
    let (_, colors) = APPEARANCE_LOOKS.iter().find(|(name, _)| id.contains(name))?;
    let color = |part: usize| Some(format!("#{}", colors[part]));
    Some(PlayerLook {
        skin: color(SKIN),
        hair: color(HAIR),
        shirt: color(SHIRT),
        pants: color(PANTS),
        accent: color(ACCENT),
    })
}

/// The look key of an NPC (#216): its `look`'s colours over its appearance's built-in ones
/// ([`appearance_look`]); `None` when it has neither (it keeps its hand-drawn built-in art).
pub fn npc_look_key(look: Option<&PlayerLook>, appearance: Option<&str>) -> Option<String> {
    let preset = appearance_look(appearance);
    let merged = match (look, preset) {
        (None, None) => return None,
        (Some(look), None) => look.clone(),
        (None, Some(preset)) => preset,
        (Some(look), Some(preset)) => PlayerLook {
            skin: look.skin.clone().or(preset.skin),
            hair: look.hair.clone().or(preset.hair),
            shirt: look.shirt.clone().or(preset.shirt),
            pants: look.pants.clone().or(preset.pants),
            accent: look.accent.clone().or(preset.accent),
        },
    };
    // A look of no colours still draws the base character (as drawn).
    Some(look_key(Some(&merged), &[]).unwrap_or_else(|| "-,-,-,-,-".to_owned()))
}

/// An NPC's built-in art (#216): with a look key, the base character (the player's sheet)
/// recoloured; else the hand-drawn art of its appearance.
pub fn npc_sprite(
    art: &crate::BuiltinArt,
    appearance: Option<&str>,
    key: Option<&str>,
    direction: Option<&str>,
    tick: f64,
    moving: bool,
) -> Option<SnapshotSprite> {
    match key {
        Some(key) => art.player(direction, tick, moving).map(|sprite| with_look(sprite, Some(key))),
        None => art.npc(appearance, direction, tick, moving),
    }
}

/// `sprite` drawn from the recoloured sheet of `key` (unchanged without a key).
pub fn with_look(mut sprite: SnapshotSprite, key: Option<&str>) -> SnapshotSprite {
    if let Some(key) = key {
        sprite.image_url = Arc::from(format!("{}{MARKER}{key}", sprite.image_url));
    }
    sprite
}

/// A look source split into the sheet's source and the key.
pub fn split(source: &str) -> Option<(&str, &str)> {
    let at = source.find(MARKER)?;
    Some((&source[..at], &source[at + MARKER.len()..]))
}

/// The sheet `image` with the frames in `region` (x, y, width, height, in pixels; 48-pixel
/// frames) recoloured for `key`.
pub fn recolor(image: Image, region: (u32, u32, u32, u32), key: &str) -> Image {
    let targets: Vec<Option<u32>> = key.split(',').map(parse_hex_bare).collect();
    let target = |part: usize| targets.get(part).copied().flatten();
    let (width, height) = (image.width(), image.height());
    let mut pixels = image.pixels().to_vec();
    let (x0, y0, w, h) = region;
    for y in y0..(y0 + h).min(height) {
        for x in x0..(x0 + w).min(width) {
            let at = ((y * width + x) * 4) as usize;
            let pixel = &mut pixels[at..at + 4];
            if pixel[3] != 255 {
                continue;
            }
            let rgb = (u32::from(pixel[0]) << 16) | (u32::from(pixel[1]) << 8) | u32::from(pixel[2]);
            let legs = (y - y0) % 48 >= LEGS_FROM_ROW;
            let (part, shades) = if SKIN_SHADES.contains(&rgb) {
                (SKIN, SKIN_SHADES)
            } else if BROWN_SHADES.contains(&rgb) {
                (if legs { PANTS } else { HAIR }, BROWN_SHADES)
            } else if SHIRT_SHADES.contains(&rgb) {
                (SHIRT, SHIRT_SHADES)
            } else if ACCENT_SHADES.contains(&rgb) {
                (ACCENT, ACCENT_SHADES)
            } else {
                continue;
            };
            let Some(color) = target(part) else { continue };
            let [r, g, b] = shade(color, rgb, shades[0]);
            pixel[0] = r;
            pixel[1] = g;
            pixel[2] = b;
        }
    }
    Image::from_premultiplied(width, height, pixels).unwrap_or(image)
}

/// `color` as light, next to it, as `shade` is next to `main`.
fn shade(color: u32, shade: u32, main: u32) -> [u8; 3] {
    let ratio = luminance(shade) / luminance(main).max(1.0);
    let channel = |shift: u32| (f64::from((color >> shift) & 0xff) * ratio).round().clamp(0.0, 255.0) as u8;
    [channel(16), channel(8), channel(0)]
}

fn luminance(rgb: u32) -> f64 {
    let channel = |shift: u32| f64::from((rgb >> shift) & 0xff);
    0.299 * channel(16) + 0.587 * channel(8) + 0.114 * channel(0)
}

/// `#rrggbb` as a number.
fn parse_hex(value: &str) -> Option<u32> {
    parse_hex_bare(value.strip_prefix('#')?)
}

/// `rrggbb` as a number (`None` for `-` and anything else).
fn parse_hex_bare(value: &str) -> Option<u32> {
    (value.len() == 6 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| u32::from_str_radix(value, 16).ok())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use farm_sim::schema::{Item, WearDefinition};

    #[test]
    fn keys_combine_the_look_and_the_clothes() {
        assert_eq!(look_key(None, &[]), None);
        let look = PlayerLook { hair: Some("#ffcc00".into()), shirt: Some("#ff0000".into()), ..PlayerLook::default() };
        assert_eq!(look_key(Some(&look), &[]).as_deref(), Some("-,ffcc00,ff0000,-,-"));
        let wear = |slot: &str, color: &str| InventorySlot {
            item: Item {
                wear: Some(WearDefinition {
                    slot: slot.into(),
                    color: Some(color.into()),
                    ..WearDefinition::default()
                }),
                ..Item::default()
            },
            quantity: 1,
            quality: None,
        };
        let worn = [wear("shirt", "#00ff00"), wear("hat", "#0000ff")];
        assert_eq!(look_key(Some(&look), &worn).as_deref(), Some("-,ffcc00,00ff00,-,0000ff"));
    }

    #[test]
    fn recolouring_keeps_the_shading() {
        let source = "builtin://characters.png#look=-,ffffff,-,-,-";
        let (base, key) = split(source).unwrap();
        assert_eq!(base, "builtin://characters.png");
        // One hair pixel of each shade, then the outline.
        let pixels: Vec<u8> = [0x5c3d28u32, 0x3d2a1e, 0x2b2138]
            .iter()
            .flat_map(|rgb| [(rgb >> 16) as u8, (rgb >> 8) as u8, *rgb as u8, 255])
            .collect();
        let image = Image::from_premultiplied(3, 1, pixels).unwrap();
        let out = recolor(image, (0, 0, 3, 1), key);
        let px = |i: usize| &out.pixels()[i * 4..i * 4 + 3];
        assert_eq!(px(0), &[255, 255, 255]);
        assert!(px(1)[0] < 255 && px(1)[0] == px(1)[1], "a darker grey: {:?}", px(1));
        assert_eq!(px(2), &[0x2b, 0x21, 0x38], "the outline stays");
    }
}
