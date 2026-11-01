//! A game's interface skin (`UiSkin` in the project): its palette over the theme, 9-slice panel
//! art, replacement icons, fonts, layout options and title screen.
//!
//! [`Skin::resolve`] turns the authored skin into what the widgets draw with: image ids of the
//! frame's [`ImageStore`] (resolved again each frame, so an evicted image is fetched again) and
//! registered font ids. Panel art is drawn as nine [`DrawCmd::Image`] commands
//! ([`draw_nine_slice`]), so the CPU rasterizer and the GPU renderer draw a skin the same way
//! they draw any sprite. The player's text size and readable-font settings apply on top: sizes
//! scale as always, and the readable font replaces the skin's fonts ([`crate::Ui::face`]).

use crate::theme::Theme;
use farm_render::graphics::ArtAsset;
use farm_render::{css_color, resolve_visual, Color, DrawCmd, DrawList, FontId, ImageId, ImageStore, Rect, Sampling};
use farm_sim::schema::{UiNineSlice, UiSkin, VisualRef};
use std::collections::BTreeMap;

/// A part of the interface that panel art can replace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Part {
    /// Modal cards (inventory, shop, quests, menus) and creator panels.
    Panel,
    /// The HUD bar and the controls hint.
    Hud,
    Button,
    ButtonHover,
    ButtonPressed,
    ButtonDisabled,
    /// Inventory slots and list rows.
    Slot,
    /// Toasts and tooltips.
    Tooltip,
    /// The dialogue and cutscene text box.
    Dialogue,
    /// Drawn over the speaker's portrait in the dialogue box.
    PortraitFrame,
}

impl Part {
    /// The part named by a `UiSkinParts` key.
    pub fn from_key(key: &str) -> Option<Part> {
        Some(match key {
            "panel" => Part::Panel,
            "hud" => Part::Hud,
            "button" => Part::Button,
            "buttonHover" => Part::ButtonHover,
            "buttonPressed" => Part::ButtonPressed,
            "buttonDisabled" => Part::ButtonDisabled,
            "slot" => Part::Slot,
            "tooltip" => Part::Tooltip,
            "dialogue" => Part::Dialogue,
            "portraitFrame" => Part::PortraitFrame,
            _ => return None,
        })
    }
}

/// A piece of an image: the frame of a sprite to draw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkinImage {
    pub image: ImageId,
    /// Source rectangle in image pixels.
    pub src: Rect,
    pub sampling: Sampling,
}

/// Panel art drawn in nine parts (see [`draw_nine_slice`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NineSlice {
    pub image: SkinImage,
    /// Source pixels of each edge that do not stretch, clamped to the image.
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    /// UI pixels per source pixel of the edges.
    pub scale: f32,
}

/// How the HUD shows the time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClockStyle {
    #[default]
    TwelveHour,
    TwentyFourHour,
    Hidden,
}

/// Where the title screen's menu sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuPosition {
    #[default]
    Center,
    Left,
    Right,
    Bottom,
}

/// Where things go (`UiLayout`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkinLayout {
    pub hud_bottom: bool,
    pub toolbar: bool,
    pub clock: ClockStyle,
    pub dialogue_top: bool,
    /// The portrait's side in the dialogue box (logical pixels).
    pub portrait_size: f32,
}

impl Default for SkinLayout {
    fn default() -> Self {
        Self {
            hud_bottom: false,
            toolbar: true,
            clock: ClockStyle::TwelveHour,
            dialogue_top: false,
            portrait_size: 48.0,
        }
    }
}

/// The skin's fonts, registered with `farm_render::text`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SkinFonts {
    pub heading: Option<FontId>,
    pub body: Option<FontId>,
    pub number: Option<FontId>,
}

/// The title screen's art and menu.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TitleSkin {
    pub background: Option<SkinImage>,
    pub logo: Option<SkinImage>,
    pub menu: MenuPosition,
    /// A scene drawn behind the menu instead of the start scene.
    pub scene: Option<String>,
}

/// A resolved skin. The default is the built-in look.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Skin {
    pub parts: BTreeMap<Part, NineSlice>,
    /// Icons by `UiSkinIcons` name.
    pub icons: BTreeMap<String, SkinImage>,
    pub layout: SkinLayout,
    pub fonts: SkinFonts,
    pub title: TitleSkin,
}

/// The image piece a visual draws (its first frame), from the frame's image store.
pub fn sprite_image(
    assets: &[ArtAsset],
    visual: &VisualRef,
    images: &mut ImageStore,
    pixel_art: bool,
) -> Option<SkinImage> {
    let sprite = resolve_visual(assets, Some(visual), 0.0, "down", true)?;
    let image = images.get_shared(&sprite.image_url)?;
    let (width, height) = images.size(image)?;
    let (width, height) = (f64::from(width), f64::from(height));
    let sw = if sprite.frame_width > 0.0 { sprite.frame_width } else { width };
    let sh = if sprite.frame_height > 0.0 { sprite.frame_height } else { height };
    let sx = sprite.source_x.unwrap_or(sprite.frame * sw);
    let sy = sprite.source_y.unwrap_or(sprite.row * sh);
    if sw <= 0.0 || sh <= 0.0 || sx < 0.0 || sy < 0.0 || sx + sw > width || sy + sh > height {
        return None;
    }
    Some(SkinImage {
        image,
        src: Rect::new(sx as f32, sy as f32, sw as f32, sh as f32),
        sampling: if pixel_art { Sampling::Nearest } else { Sampling::Smooth },
    })
}

fn nine_slice(slice: &UiNineSlice, assets: &[ArtAsset], images: &mut ImageStore, pixel_art: bool) -> Option<NineSlice> {
    let image = sprite_image(assets, &slice.visual, images, pixel_art)?;
    let edge = |v: f64, room: f32| (v.max(0.0) as f32).min(room);
    let (w, h) = (image.src.width, image.src.height);
    let left = edge(slice.left, w);
    let right = edge(slice.right, w - left);
    let top = edge(slice.top, h);
    let bottom = edge(slice.bottom, h - top);
    let scale = slice.scale.map_or(1.0, |s| s.clamp(1.0, 8.0) as f32);
    Some(NineSlice { image, left, top, right, bottom, scale })
}

impl Skin {
    /// What the widgets draw with for `skin` this frame. `font` gives the registered font of a
    /// `font` asset id (the host registers them once, when the game loads). Missing art and
    /// unknown names are left out: that part keeps the built-in look (Problems reports them).
    pub fn resolve(
        skin: &UiSkin,
        assets: &[ArtAsset],
        images: &mut ImageStore,
        pixel_art: bool,
        font: impl Fn(&str) -> Option<FontId>,
    ) -> Skin {
        let parts = skin
            .panels
            .iter()
            .flatten()
            .filter_map(|(key, slice)| Some((Part::from_key(key)?, nine_slice(slice, assets, images, pixel_art)?)))
            .collect();
        let icons = skin
            .icons
            .iter()
            .flatten()
            .filter_map(|(key, visual)| Some((key.clone(), sprite_image(assets, visual, images, pixel_art)?)))
            .collect();
        let layout = skin.layout.as_ref().map_or_else(SkinLayout::default, |layout| SkinLayout {
            hud_bottom: layout.hud_position.as_deref() == Some("bottom"),
            toolbar: layout.toolbar.unwrap_or(true),
            clock: match layout.clock_style.as_deref() {
                Some("24h") => ClockStyle::TwentyFourHour,
                Some("hidden") => ClockStyle::Hidden,
                _ => ClockStyle::TwelveHour,
            },
            dialogue_top: layout.dialogue_position.as_deref() == Some("top"),
            portrait_size: layout.portrait_size.map_or(48.0, |size| size.clamp(32.0, 160.0) as f32),
        });
        let fonts = SkinFonts {
            heading: skin.heading_font.as_deref().and_then(&font),
            body: skin.body_font.as_deref().and_then(&font),
            number: skin.number_font.as_deref().and_then(&font),
        };
        let title = skin.title.as_ref().map_or_else(TitleSkin::default, |title| TitleSkin {
            background: title.background.as_ref().and_then(|v| sprite_image(assets, v, images, pixel_art)),
            logo: title.logo.as_ref().and_then(|v| sprite_image(assets, v, images, pixel_art)),
            menu: match title.menu_position.as_deref() {
                Some("left") => MenuPosition::Left,
                Some("right") => MenuPosition::Right,
                Some("bottom") => MenuPosition::Bottom,
                _ => MenuPosition::Center,
            },
            scene: title.scene.clone().filter(|scene| !scene.is_empty()),
        });
        Skin { parts, icons, layout, fonts, title }
    }

    /// The panel art of `part`, if the skin has it.
    pub fn part(&self, part: Part) -> Option<&NineSlice> {
        self.parts.get(&part)
    }

    /// The icon named `name` (`UiSkinIcons`), if the skin has it.
    pub fn icon(&self, name: &str) -> Option<&SkinImage> {
        self.icons.get(name)
    }
}

/// `theme` with the skin's colours over its palette. Unknown roles and colours that do not
/// parse are left out; `warning` sets the error colour (the theme's warning role).
pub fn apply_palette(theme: Theme, skin: &UiSkin) -> Theme {
    let mut theme = theme;
    for (role, value) in skin.colors.iter().flatten() {
        let Some(color) = css_color::try_parse(value) else { continue };
        let c = &mut theme.colors;
        let slot: &mut Color = match role.as_str() {
            "backdrop" => &mut c.backdrop,
            "background" => &mut c.background,
            "panel" => &mut c.panel,
            "panelBorder" => &mut c.panel_border,
            "panelHeader" => &mut c.panel_header,
            "hud" => &mut c.hud,
            "hudBorder" => &mut c.hud_border,
            "row" => &mut c.row,
            "rowBorder" => &mut c.row_border,
            "text" => &mut c.text,
            "muted" => &mut c.muted,
            "faint" => &mut c.faint,
            "accent" => &mut c.accent,
            "onAccent" => &mut c.on_accent,
            "accentText" => &mut c.accent_text,
            "primary" => &mut c.primary,
            "secondary" => &mut c.secondary,
            "warning" | "error" => &mut c.error,
            "successTint" => &mut c.success_tint,
            "errorTint" => &mut c.error_tint,
            "button" => &mut c.button,
            "buttonHover" => &mut c.button_hover,
            "buttonPressed" => &mut c.button_pressed,
            "buttonBorder" => &mut c.button_border,
            "focus" => &mut c.focus,
            "track" => &mut c.track,
            "shadow" => &mut c.shadow,
            _ => continue,
        };
        *slot = color;
    }
    theme
}

/// Draws `slice` over `dst`: the four corners at their size (times the slice's scale), the
/// edges stretched along their side and the middle stretched both ways. Edges larger than
/// `dst` shrink in proportion, so a small button never draws corners over each other.
pub fn draw_nine_slice(list: &mut DrawList, slice: &NineSlice, dst: Rect) {
    if dst.width <= 0.0 || dst.height <= 0.0 {
        return;
    }
    let src = slice.image.src;
    let (mut l, mut r) = (slice.left * slice.scale, slice.right * slice.scale);
    let (mut t, mut b) = (slice.top * slice.scale, slice.bottom * slice.scale);
    if l + r > dst.width {
        let k = dst.width / (l + r);
        l *= k;
        r *= k;
    }
    if t + b > dst.height {
        let k = dst.height / (t + b);
        t *= k;
        b *= k;
    }
    let sx = [src.x, src.x + slice.left, src.right() - slice.right, src.right()];
    let sy = [src.y, src.y + slice.top, src.bottom() - slice.bottom, src.bottom()];
    let dx = [dst.x, dst.x + l, dst.right() - r, dst.right()];
    let dy = [dst.y, dst.y + t, dst.bottom() - b, dst.bottom()];
    for row in 0..3 {
        for column in 0..3 {
            let s = Rect::new(sx[column], sy[row], sx[column + 1] - sx[column], sy[row + 1] - sy[row]);
            let d = Rect::new(dx[column], dy[row], dx[column + 1] - dx[column], dy[row + 1] - dy[row]);
            if s.width <= 0.0 || s.height <= 0.0 || d.width <= 0.0 || d.height <= 0.0 {
                continue;
            }
            list.push(DrawCmd::Image {
                image: slice.image.image,
                src: s,
                dst: d,
                opacity: 1.0,
                sampling: slice.image.sampling,
                tint: None,
            });
        }
    }
}

/// Draws an icon's image fitted into the square `rect` (whole-number scales for pixel art).
pub fn draw_icon(list: &mut DrawList, icon: &SkinImage, rect: Rect) {
    let src = icon.src;
    if src.width <= 0.0 || src.height <= 0.0 {
        return;
    }
    let scale = (rect.width / src.width).min(rect.height / src.height);
    let scale = if icon.sampling == Sampling::Nearest && scale >= 1.0 { scale.floor() } else { scale };
    let (w, h) = (src.width * scale, src.height * scale);
    list.push(DrawCmd::Image {
        image: icon.image,
        src,
        dst: Rect::new(rect.x + (rect.width - w) / 2.0, rect.y + (rect.height - h) / 2.0, w, h),
        opacity: 1.0,
        sampling: icon.sampling,
        tint: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;

    #[test]
    fn the_palette_sets_known_roles_and_skips_the_rest() {
        let mut colors = IndexMap::new();
        colors.insert("text".to_owned(), "#102030".to_owned());
        colors.insert("warning".to_owned(), "#ff000080".to_owned());
        colors.insert("nope".to_owned(), "#ffffff".to_owned());
        colors.insert("accent".to_owned(), "not a colour".to_owned());
        let skin = UiSkin { colors: Some(colors), ..UiSkin::default() };
        let theme = apply_palette(Theme::cozy(), &skin);
        assert_eq!(theme.colors.text, Color::rgb(0x10, 0x20, 0x30));
        assert_eq!(theme.colors.error, Color::rgba(0xff, 0, 0, 0x80));
        assert_eq!(theme.colors.accent, Theme::cozy().colors.accent);
    }

    fn slice() -> NineSlice {
        NineSlice {
            image: SkinImage { image: ImageId(3), src: Rect::new(10.0, 20.0, 12.0, 12.0), sampling: Sampling::Nearest },
            left: 4.0,
            top: 4.0,
            right: 4.0,
            bottom: 4.0,
            scale: 2.0,
        }
    }

    fn images(list: &DrawList) -> Vec<(Rect, Rect)> {
        list.commands
            .iter()
            .filter_map(|c| match c {
                DrawCmd::Image { src, dst, .. } => Some((*src, *dst)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn nine_slices_keep_corners_and_stretch_the_middle() {
        let mut list = DrawList::new();
        draw_nine_slice(&mut list, &slice(), Rect::new(0.0, 0.0, 100.0, 40.0));
        let parts = images(&list);
        assert_eq!(parts.len(), 9);
        // Top-left corner: 4 source pixels at scale 2.
        assert_eq!(parts[0], (Rect::new(10.0, 20.0, 4.0, 4.0), Rect::new(0.0, 0.0, 8.0, 8.0)));
        // The middle stretches over what is left.
        assert_eq!(parts[4], (Rect::new(14.0, 24.0, 4.0, 4.0), Rect::new(8.0, 8.0, 84.0, 24.0)));
        // Bottom-right corner.
        assert_eq!(parts[8], (Rect::new(18.0, 28.0, 4.0, 4.0), Rect::new(92.0, 32.0, 8.0, 8.0)));
    }

    #[test]
    fn small_targets_shrink_the_edges() {
        let mut list = DrawList::new();
        draw_nine_slice(&mut list, &slice(), Rect::new(0.0, 0.0, 10.0, 10.0));
        let parts = images(&list);
        // No middle row or column is left; the corners share the space.
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0].1, Rect::new(0.0, 0.0, 5.0, 5.0));
    }

    #[test]
    fn the_layout_reads_its_options() {
        let skin = UiSkin {
            layout: Some(farm_sim::schema::UiLayout {
                hud_position: Some("bottom".to_owned()),
                toolbar: Some(false),
                clock_style: Some("24h".to_owned()),
                dialogue_position: Some("top".to_owned()),
                portrait_size: Some(500.0),
            }),
            ..UiSkin::default()
        };
        let resolved = Skin::resolve(&skin, &[], &mut ImageStore::default(), true, |_| None);
        assert!(resolved.layout.hud_bottom && !resolved.layout.toolbar && resolved.layout.dialogue_top);
        assert_eq!(resolved.layout.clock, ClockStyle::TwentyFourHour);
        assert_eq!(resolved.layout.portrait_size, 160.0);
        assert!(resolved.parts.is_empty());
    }
}
