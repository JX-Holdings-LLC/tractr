//! The play screens (ports of the editor's Play Mode overlays, `PlayModeView.cs`,
//! `PlayOverlays.cs`, `MinigameOverlay.cs`, `ToastHost.cs`): the HUD with its toolbar and
//! controls hints, creator game panels, dialogue, shop, crafting, inventory, quest log,
//! minigames and toasts; and the farm-building, goals and combat screens (M5–M7): chests and
//! shipping bins, a builder's shop with its placement map, the goals panel, the mailbox and
//! the health bar.
//!
//! [`GameUi::draw`] reads a [`GameView`] (the live state plus rule queries from
//! `farm_sim::overlay`) and returns [`GameAction`]s; every gameplay action is an engine command.

mod building;
mod crafting;
mod cutscene;
mod dialogue;
mod goals;
mod hud;
mod inventory;
mod mail;
mod minigame;
mod quests;
mod shop;
mod storage;
mod toasts;

pub use building::Placement;
pub use goals::GoalsTab;

pub use toasts::{
    toast_lifetime, Toast, ToastKind, Toasts, MAX_TOAST_LIFETIME, MAX_VISIBLE as MAX_TOASTS, TOAST_LIFETIME,
};

use crate::i18n::Lang;
use crate::icons::{self, Icon};
use crate::layout::RectExt;
use crate::settings::Bindings;
use crate::theme;
use crate::ui::Ui;
use farm_render::graphics::ArtAsset;
use farm_render::{resolve_visual, BuiltinArt, DrawCmd, ImageStore, Rect, Sampling, SnapshotSprite};
use farm_runtime::host::{CalendarView, MinigameInput, MinigameView};
use farm_runtime::panels::PanelView;
use farm_sim::overlay::OverlayView;
use farm_sim::schema::{GameContent, GameState, GameWords, Item, ItemCategoryDefinition};
use farm_sim::{Command, EngineContext};

/// A host panel over the game (web `showInventory` / `showQuests` / `showCrafting`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Inventory,
    Quests,
    Crafting,
    /// Collections, bundles, achievements and the shipping log (M6).
    Goals,
    /// The mailbox (M6).
    Mail,
}

/// The shop dialog's tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShopTab {
    #[default]
    Buy,
    Sell,
    Repair,
    /// A builder's buildings and upgrades (M5).
    Build,
    /// The animals a shop sells (M5).
    Animals,
}

/// What the player asked for this frame.
#[derive(Debug, Clone, PartialEq)]
pub enum GameAction {
    /// Run an engine command.
    Command(Command),
    /// Open the panel, or close it when it is open.
    TogglePanel(Panel),
    ClosePanel,
    /// The toolbar's Menu button (the pause menu).
    OpenMenu,
    /// Input for the running minigame.
    Minigame(MinigameInput),
    /// The pointer over a pack minigame's canvas (`PlaySession::minigame_pointer`).
    MinigamePointer(farm_runtime::canvas::CanvasPointer),
    /// Hold this seed or fertilizer for planting, or put it away when it is held.
    Hold(String),
}

/// What the player holds for planting (picked in the inventory): interacting with open soil
/// plants the held seed (else the first seed that grows this season) and uses the held
/// fertilizer (else none).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Planting {
    pub seed: Option<String>,
    pub fertilizer: Option<String>,
}

impl Planting {
    /// Holds `item_id` (a seed or a fertilizer), or puts it away when it is already held.
    pub fn toggle(&mut self, content: &GameContent, item_id: &str) {
        let Some(item) = content.items.iter().find(|item| item.id == item_id) else { return };
        let Some(kind) = category(content, item) else { return };
        let hand = if kind.plantable {
            &mut self.seed
        } else if kind.fertilizer {
            &mut self.fertilizer
        } else {
            return;
        };
        *hand = if hand.as_deref() == Some(item_id) { None } else { Some(item_id.to_owned()) };
    }

    /// Whether `item_id` is held.
    pub fn holds(&self, item_id: &str) -> bool {
        self.seed.as_deref() == Some(item_id) || self.fertilizer.as_deref() == Some(item_id)
    }

    /// Puts away what the player no longer has.
    pub fn retain_held(&mut self, state: &GameState) {
        for hand in [&mut self.seed, &mut self.fertilizer] {
            if hand.as_deref().is_some_and(|id| held(state, id) == 0) {
                *hand = None;
            }
        }
    }
}

/// What item art is drawn from: the creator's assets, else the built-in pack.
#[derive(Debug, Clone, Copy)]
pub struct ItemArt<'a> {
    pub assets: &'a [ArtAsset],
    /// `graphics.pixelArt`: nearest-neighbour scaling.
    pub pixel_art: bool,
    pub builtin: Option<&'a BuiltinArt>,
}

/// Everything the play screens read in one frame.
#[derive(Debug, Clone, Copy)]
pub struct GameView<'a> {
    pub state: &'a GameState,
    pub content: &'a GameContent,
    /// The engine's context (rule queries such as `farm_sim::overlay::site_status`); its
    /// content is [`GameView::content`].
    pub ctx: &'a EngineContext,
    pub overlay: &'a OverlayView<'a>,
    pub calendar: &'a CalendarView,
    /// Creator game panels (`farm_runtime::panels::render`).
    pub panels: &'a [PanelView],
    pub minigame: Option<&'a MinigameView>,
    pub art: ItemArt<'a>,
    pub bindings: &'a Bindings,
    /// Show "Made with Tractr".
    pub show_made_with: bool,
    /// The game UI owns keyboard and gamepad this frame (digits pick dialogue options, tabs
    /// switch shop tabs). False while a shell screen (pause, settings) is on top.
    pub keys_active: bool,
    /// What the player holds for planting.
    pub planting: &'a Planting,
}

/// The play screens' own state: which panel is open, the shop tab, toasts.
#[derive(Debug, Clone, Default)]
pub struct GameUi {
    pub panel: Option<Panel>,
    pub shop_tab: ShopTab,
    pub toasts: Toasts,
    /// What the player holds for planting (kept here, sent with each interact by the host).
    pub planting: Planting,
    /// A building being placed from the open builder's shop (M5): the shop stays open behind
    /// the placement map, so `orderBuilding` is sent while it is.
    pub placement: Option<Placement>,
    /// The goals panel's tab (M6).
    pub goals_tab: GoalsTab,
    /// The letter open in the mailbox (M6): its index in `state.mailbox`.
    pub letter: Option<usize>,
    /// The player's health last frame (M7): a drop flashes the world red until `hurt_until`
    /// (UI seconds).
    pub last_health: Option<i32>,
    pub hurt_until: f64,
}

/// How long the screen flashes red after the player is hurt, in seconds.
const HURT_FLASH: f64 = 0.35;

/// Whether the game has anything for the goals panel (M6): collections, bundles, achievements
/// or a shipping bin.
pub fn has_goals(content: &GameContent) -> bool {
    !goals::tabs(content).is_empty()
}

/// Whether the game sends letters (M6).
pub fn has_mail(content: &GameContent) -> bool {
    !content.mail.is_empty()
}

impl GameUi {
    pub fn new() -> Self {
        Self::default()
    }

    /// Draws the HUD and every open overlay; returns what the player did.
    pub fn draw(&mut self, ui: &mut Ui, view: &GameView<'_>, images: &mut ImageStore) -> Vec<GameAction> {
        let mut actions = Vec::new();
        if view.state.shop.is_none() {
            self.shop_tab = ShopTab::Buy;
            self.placement = None;
        }
        if self.panel != Some(Panel::Mail) {
            self.letter = None;
        }
        self.planting.retain_held(view.state);
        // A hit (M7) flashes the world red for a moment.
        let health = view.state.player.health;
        if let (Some(now), Some(before)) = (health, self.last_health) {
            if now < before {
                self.hurt_until = ui.time() + HURT_FLASH;
            }
        }
        self.last_health = health;
        let flash = (self.hurt_until - ui.time()) / HURT_FLASH;
        if flash > 0.0 && !ui.reduced_motion() {
            let red = theme::fade(farm_render::Color::rgb(0xd0, 0x30, 0x30), 0.35 * flash.min(1.0) as f32);
            ui.tint_world(red, red);
        }
        let hud_bottom = hud::draw(ui, view, images, &mut actions);
        self.toasts.top = hud_bottom + 12.0;
        match self.panel {
            Some(Panel::Inventory) => inventory::draw(ui, view, images, &mut actions),
            Some(Panel::Quests) => quests::draw(ui, view, &mut actions),
            Some(Panel::Crafting) => crafting::draw(ui, view, &mut actions),
            Some(Panel::Goals) => goals::draw(ui, view, &mut self.goals_tab, &mut actions),
            Some(Panel::Mail) => mail::draw(ui, view, images, &mut self.letter, &mut actions),
            None => {}
        }
        // Under the dialogue, shop or minigame a cutscene step opened.
        cutscene::draw(ui, view, images, &mut actions);
        match &mut self.placement {
            Some(placement) if view.state.shop.is_some() => {
                if !building::place(ui, view, placement, &mut actions) {
                    self.placement = None;
                }
            }
            _ => shop::draw(ui, view, &mut self.shop_tab, &mut self.placement, &mut actions),
        }
        storage::draw(ui, view, images, &mut actions);
        dialogue::draw(ui, view, images, &mut actions);
        minigame::draw(ui, view, images, &mut actions);
        actions
    }

    /// Toggles a host panel (the same panel closes it).
    pub fn toggle(&mut self, panel: Panel) {
        self.panel = if self.panel == Some(panel) { None } else { Some(panel) };
    }

    /// Whether the play screens hold the keys themselves this frame: a host panel, or the
    /// placement map over a builder's shop (Escape cancels it instead of closing the shop).
    pub fn host_modal_open(&self) -> bool {
        self.panel.is_some() || self.placement.is_some()
    }

    /// Draws the toasts (call last, above every other layer).
    pub fn draw_toasts(&mut self, ui: &mut Ui) {
        self.toasts.draw(ui);
    }
}

/// A list row with content on the left and buttons on the right (web `Ui.Row`): draws the row
/// background and returns (content area, actions area).
pub(crate) fn row(ui: &mut Ui, area: Rect, top: f32, height: f32, actions_width: f32) -> (Rect, Rect) {
    let rect = Rect::new(area.x, top, area.width, height);
    ui.row_background(rect);
    let mut inner = rect.inset_xy(10.0, 8.0);
    let actions = inner.cut_right(actions_width);
    inner.cut_right(10.0);
    (inner, actions)
}

/// Lays buttons of `widths` right to left in `area`, vertically centered at `height`; returns
/// their rectangles left to right.
pub(crate) fn right_aligned(area: Rect, widths: &[f32], height: f32, gap: f32) -> Vec<Rect> {
    let total: f32 = widths.iter().sum::<f32>() + gap * widths.len().saturating_sub(1) as f32;
    let mut x = area.right() - total;
    let y = area.y + (area.height - height) / 2.0;
    widths
        .iter()
        .map(|width| {
            let rect = Rect::new(x, y, *width, height);
            x += width + gap;
            rect
        })
        .collect()
}

/// Draws an item's art into `rect`: the creator's visual (first frame), its custom image, the
/// built-in art for its type, or a package glyph.
pub(crate) fn draw_item_art(
    ui: &mut Ui,
    images: &mut ImageStore,
    art: &ItemArt<'_>,
    content: &GameContent,
    item: &Item,
    rect: Rect,
) {
    let definition = content.items.iter().find(|candidate| candidate.id == item.id).unwrap_or(item);
    let visual = definition.visual.as_ref().or(item.visual.as_ref());
    let sprite = resolve_visual(art.assets, visual, 0.0, "down", true)
        .or_else(|| art.builtin.and_then(|builtin| builtin.item(Some(definition.r#type.as_str()))));
    if let Some(sprite) = sprite {
        if draw_sprite(ui, images, &sprite, rect, art.pixel_art) {
            return;
        }
    }
    let colors = ui.theme().colors;
    icons::draw(ui.list_mut(), Icon::Package, rect.inset(rect.width * 0.18), colors.accent_text, colors.row);
}

/// Draws the first frame of a sprite fitted into `rect`; false when its image is unavailable.
pub(crate) fn draw_sprite(
    ui: &mut Ui,
    images: &mut ImageStore,
    sprite: &SnapshotSprite,
    rect: Rect,
    pixel_art: bool,
) -> bool {
    let Some(id) = images.get_shared(&sprite.image_url) else { return false };
    let Some((width, height)) = images.size(id) else { return false };
    let (width, height) = (f64::from(width), f64::from(height));
    let sw = if sprite.frame_width > 0.0 { sprite.frame_width } else { width };
    let sh = if sprite.frame_height > 0.0 { sprite.frame_height } else { height };
    let sx = sprite.source_x.unwrap_or(sprite.frame * sw);
    let sy = sprite.source_y.unwrap_or(sprite.row * sh);
    if sw <= 0.0 || sh <= 0.0 || sx < 0.0 || sy < 0.0 || sx + sw > width || sy + sh > height {
        return false;
    }
    let scale = (f64::from(rect.width) / sw).min(f64::from(rect.height) / sh);
    // Whole-number scales keep pixel art even.
    let scale = if pixel_art && scale >= 1.0 { scale.floor() } else { scale };
    let (dw, dh) = ((sw * scale) as f32, (sh * scale) as f32);
    let dst = Rect::new(rect.x + (rect.width - dw) / 2.0, rect.y + (rect.height - dh) / 2.0, dw, dh);
    ui.list_mut().push(DrawCmd::Image {
        image: id,
        src: Rect::new(sx as f32, sy as f32, sw as f32, sh as f32),
        dst,
        opacity: 1.0,
        sampling: if pixel_art { Sampling::Nearest } else { Sampling::Smooth },
        tint: None,
    });
    true
}

/// The quantity of an item held across all slots.
pub(crate) fn held(state: &GameState, item_id: &str) -> u64 {
    state.player.inventory.iter().filter(|slot| slot.item.id == item_id).map(|slot| u64::from(slot.quantity)).sum()
}

/// A slot's display name: the item name with its quality tier's stars (in the built-in tiers
/// silver ★, gold ★★, iridium ★★★).
pub(crate) fn slot_name(content: &GameContent, name: &str, quality: Option<&str>) -> String {
    let stars = quality
        .and_then(|quality| farm_sim::vocabulary::crop_qualities(content).iter().find(|tier| tier.id == quality))
        .map_or(0, |tier| tier.stars.min(10) as usize);
    if stars == 0 {
        name.to_owned()
    } else {
        format!("{name} {}", "\u{2605}".repeat(stars))
    }
}

/// The category (kind) of `item` in the game (M1).
pub(crate) fn category<'a>(content: &'a GameContent, item: &Item) -> Option<&'a ItemCategoryDefinition> {
    farm_sim::vocabulary::item_categories(content).iter().find(|category| category.id == item.r#type)
}

impl GameView<'_> {
    /// The game's words (M1) in `lang`.
    pub(crate) fn words(&self, lang: Lang) -> GameWords {
        self.content.settings.words.as_ref().map(|words| words.for_locale(lang.code())).unwrap_or_default()
    }

    /// An amount of money in the game's words.
    pub(crate) fn money(&self, lang: Lang, amount: i64) -> String {
        crate::format::money_in(&self.words(lang), amount)
    }
}

/// An item's display name (its id when unknown).
pub(crate) fn item_name<'a>(content: &'a GameContent, item_id: &'a str) -> &'a str {
    content.items.iter().find(|item| item.id == item_id).map_or(item_id, |item| item.name.as_str())
}

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_farm;
