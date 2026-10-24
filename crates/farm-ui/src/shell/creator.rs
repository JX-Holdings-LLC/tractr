//! The character creator (M4), shown when a new game of a game that asks for it starts: the
//! player's name and the farm's name, the player's skin, hair, shirt and trousers colours with a
//! preview, and Start. The game waits behind it (its clock does not run while a shell screen is
//! open); Start sends the answers as the `setPlayerLook` command, before the first tick.
//!
//! A name field takes typed letters once chosen (click it, or Accept on it): Backspace erases,
//! Enter or Escape is done. Without a keyboard, Random picks a name.

use super::ShellAction;
use crate::game::draw_sprite;
use crate::icons::Icon;
use crate::layout::{Align, RectExt};
use crate::theme::fade;
use crate::ui::{HitKind, Ui, WidgetId};
use crate::widgets::{Button, ModalSpec};
use farm_render::{BuiltinArt, Color, FontId, ImageStore, Rect};
use farm_sim::schema::PlayerLook;

/// The colours offered for each part; the first, `None`, keeps the art as drawn.
pub const SKIN_COLORS: &[Option<&str>] =
    &[None, Some("#f5d3b0"), Some("#e0ab7c"), Some("#c68a5a"), Some("#9a6438"), Some("#6e4526"), Some("#4a2c18")];
pub const HAIR_COLORS: &[Option<&str>] = &[
    None,
    Some("#2b2118"),
    Some("#8a5a2b"),
    Some("#d9a441"),
    Some("#efe0b0"),
    Some("#b33a2a"),
    Some("#7d7d8e"),
    Some("#3a5fc0"),
];
pub const SHIRT_COLORS: &[Option<&str>] =
    &[None, Some("#c23a3a"), Some("#3f8b44"), Some("#f7b24a"), Some("#9b7bd0"), Some("#eef0f5"), Some("#3a3a48")];
pub const PANTS_COLORS: &[Option<&str>] =
    &[None, Some("#3a3a48"), Some("#1e3a6e"), Some("#5a5a6b"), Some("#2e6b3a"), Some("#a07350")];

/// Names Random picks from.
const NAMES: &[&str] = &[
    "Ash", "Bea", "Cass", "Dale", "Ember", "Finn", "Gale", "Hazel", "Ivy", "Juniper", "Kit", "Linden", "Moss", "Nell",
];
const FARMS: &[&str] = &["Sunny Acres", "Willow Hollow", "Bramble End", "Clover Hill", "Fern Gully", "Maple Rise"];

/// The longest name, in characters (farm_sim::wardrobe takes the same).
const MAX_CHARS: usize = farm_sim::wardrobe::MAX_NAME_CHARS;

/// Which name field takes typing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreatorField {
    Name,
    Farm,
}

/// The creator's answers so far (owned by the player while the screen is open).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CreatorScreen {
    pub name: String,
    pub farm: String,
    pub editing: Option<CreatorField>,
    /// Indices into [`SKIN_COLORS`], [`HAIR_COLORS`], [`SHIRT_COLORS`], [`PANTS_COLORS`].
    pub colors: [usize; 4],
    /// How many times Random was pressed (it steps through the names).
    pub randoms: usize,
}

impl CreatorScreen {
    /// A creator starting from the game's own name and farm name.
    pub fn new(name: &str, farm: &str) -> Self {
        Self { name: name.to_owned(), farm: farm.to_owned(), ..Self::default() }
    }

    /// The colours chosen.
    pub fn look(&self) -> PlayerLook {
        let pick = |table: &[Option<&str>], index: usize| table.get(index).copied().flatten().map(str::to_owned);
        PlayerLook {
            skin: pick(SKIN_COLORS, self.colors[0]),
            hair: pick(HAIR_COLORS, self.colors[1]),
            shirt: pick(SHIRT_COLORS, self.colors[2]),
            pants: pick(PANTS_COLORS, self.colors[3]),
            accent: None,
        }
    }

    /// Typed keys into the field being edited (farm-runtime key names: one character, or
    /// `backspace`, `enter`, `escape`). Whether the field closed.
    fn type_keys(&mut self, keys: &[String]) -> bool {
        let Some(field) = self.editing else { return false };
        let text = match field {
            CreatorField::Name => &mut self.name,
            CreatorField::Farm => &mut self.farm,
        };
        for key in keys {
            match key.as_str() {
                "backspace" => {
                    text.pop();
                }
                "enter" | "escape" | "tab" => {
                    self.editing = None;
                    return true;
                }
                _ => {
                    let mut chars = key.chars();
                    let (Some(c), None) = (chars.next(), chars.next()) else { continue };
                    if c.is_control() || text.chars().count() >= MAX_CHARS {
                        continue;
                    }
                    // Keys arrive lower case: words start with a capital.
                    let starts_word = text.is_empty() || text.ends_with(' ');
                    if starts_word {
                        text.extend(c.to_uppercase());
                    } else {
                        text.push(c);
                    }
                }
            }
        }
        false
    }
}

/// Draws the creator; returns [`ShellAction::FinishCreator`] when Start is chosen.
pub fn creator(ui: &mut Ui, screen: &mut CreatorScreen, images: &mut ImageStore) -> Option<ShellAction> {
    let colors = ui.theme().colors;
    let lang = ui.lang();
    let keys = ui.input().keys_pressed.clone();
    let closed = screen.type_keys(&keys);
    let footer_height = ui.button_height(14.0) + 20.0;
    let modal = ui.begin_modal(ModalSpec {
        id: WidgetId::new("creator"),
        icon: Icon::Person,
        title: lang.tr("creator.title"),
        subtitle: Some(lang.tr("creator.subtitle")),
        width: 560.0,
        max_height: 640.0,
        footer: footer_height,
        backdrop: true,
    });
    let area = modal.body;
    let mut y = modal.top;

    // The preview on the right, the fields on the left.
    let preview_width = 120.0f32.min(area.width * 0.3);
    let preview = Rect::new(area.right() - preview_width, y, preview_width, 150.0);
    ui.list_mut().fill_round_rect(preview, 10.0, fade(colors.accent, 0.14));
    if let Some(sprite) = BuiltinArt::embedded().player(Some("down"), 0.0, false) {
        let key = farm_render::look::look_key(Some(&screen.look()), &[]);
        let sprite = farm_render::look::with_look(sprite, key.as_deref());
        draw_sprite(ui, images, &sprite, preview.centered(64.0, 96.0), true);
    }
    let left = Rect::new(area.x, y, area.width - preview_width - 16.0, area.height);

    for (field, label, random) in
        [(CreatorField::Name, lang.tr("creator.name"), NAMES), (CreatorField::Farm, lang.tr("creator.farm"), FARMS)]
    {
        let label_height = ui.line_height(13.0);
        ui.label(Rect::new(left.x, y, left.width, label_height), label, 13.0, FontId::Bold, colors.text, Align::Start);
        y += label_height + 4.0;
        let height = ui.button_height(14.0) + 4.0;
        let mut row = Rect::new(left.x, y, left.width, height);
        let random_button = Button::new(lang.tr("creator.random")).size(12.5);
        let random_width = ui.button_width(&random_button);
        let random_rect = row.cut_right(random_width).centered(random_width, ui.button_height(12.5));
        row.cut_right(8.0);
        let id = WidgetId::new("creator-field").with(field as usize);
        let state = ui.interact(id, row, HitKind::Button, true);
        let editing = screen.editing == Some(field);
        let border = if editing { colors.primary } else { colors.row_border };
        ui.list_mut().fill_round_rect(row, 8.0, colors.row);
        ui.list_mut().stroke_round_rect(row.inset(0.5), 8.0, border, if editing { 2.0 } else { 1.0 });
        if state.focus_visible {
            ui.focus_ring(row, 8.0);
        }
        let text = match field {
            CreatorField::Name => &screen.name,
            CreatorField::Farm => &screen.farm,
        };
        let shown = if editing { format!("{text}\u{258f}") } else { text.clone() };
        let (shown, color) = if shown.is_empty() {
            (lang.tr("creator.typeHint").to_owned(), colors.muted)
        } else {
            (shown, colors.text)
        };
        ui.label(row.inset_xy(12.0, 0.0), &shown, 14.0, FontId::Regular, color, Align::Start);
        if state.clicked && !closed {
            screen.editing = if editing { None } else { Some(field) };
        }
        if ui.button(WidgetId::new("creator-random").with(field as usize), random_rect, random_button) {
            screen.randoms = screen.randoms.wrapping_add(1);
            let pick = random[screen.randoms.wrapping_mul(7) % random.len()].to_owned();
            match field {
                CreatorField::Name => screen.name = pick,
                CreatorField::Farm => screen.farm = pick,
            }
            screen.editing = None;
        }
        y += height + 12.0;
    }

    // Colour swatches, one row per part.
    let parts: [(&str, &[Option<&str>]); 4] = [
        ("creator.skin", SKIN_COLORS),
        ("creator.hair", HAIR_COLORS),
        ("creator.shirt", SHIRT_COLORS),
        ("creator.pants", PANTS_COLORS),
    ];
    y = y.max(preview.bottom() + 12.0).max(y);
    for (part, (key, table)) in parts.iter().enumerate() {
        let label_height = ui.line_height(13.0);
        ui.label(
            Rect::new(area.x, y, area.width, label_height),
            lang.tr(key),
            13.0,
            FontId::Bold,
            colors.text,
            Align::Start,
        );
        y += label_height + 4.0;
        let size = 30.0;
        let mut x = area.x;
        for (index, swatch) in table.iter().enumerate() {
            let rect = Rect::new(x, y, size, size);
            let state =
                ui.interact(WidgetId::new("creator-swatch").with(part).with(index), rect, HitKind::Button, true);
            let fill = swatch.and_then(Color::parse).unwrap_or(colors.row);
            ui.list_mut().fill_round_rect(rect, 6.0, fill);
            let chosen = screen.colors[part] == index;
            ui.list_mut().stroke_round_rect(
                rect.inset(0.5),
                6.0,
                if chosen { colors.primary } else { colors.row_border },
                if chosen { 3.0 } else { 1.0 },
            );
            if swatch.is_none() {
                // "As drawn": a diagonal stroke.
                ui.list_mut().line(
                    (rect.x + 6.0, rect.bottom() - 6.0),
                    (rect.right() - 6.0, rect.y + 6.0),
                    colors.muted,
                    2.0,
                );
            }
            if state.focus_visible {
                ui.focus_ring(rect, 6.0);
            }
            if state.clicked {
                screen.colors[part] = index;
            }
            x += size + 6.0;
        }
        y += size + 12.0;
    }
    ui.end_modal_body(y);

    let mut action = None;
    if let Some(footer) = modal.footer {
        let mut footer = footer;
        let start = Button::new(lang.tr("creator.start")).primary().default_focus();
        let width = ui.button_width(&start);
        let rect = footer.cut_right(width).centered(width, ui.button_height(14.0));
        if ui.button(WidgetId::new("creator-start"), rect, start) {
            action = Some(ShellAction::FinishCreator);
        }
    }
    ui.close_modal();
    action
}
