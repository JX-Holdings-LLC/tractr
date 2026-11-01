//! The title screen: the game's name over its world, and New Game, Continue, Load, Settings,
//! Credits and Quit.

use super::ShellAction;
use crate::icons::Icon;
use crate::layout::Align;
use crate::theme::fade;
use crate::ui::{Ui, WidgetId};
use crate::widgets::{Button, ButtonKind};
use farm_render::{Color, DrawCmd, FontId, Rect, TextAlign};

/// What the title screen shows.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TitleView {
    pub title: String,
    /// "Version 1.0 · by Someone".
    pub subtitle: String,
    /// There is a save to continue.
    pub can_continue: bool,
    /// What Continue loads ("Day 3 of Spring, Year 1 · slot 2").
    pub continue_detail: Option<String>,
    /// Any save exists (Load).
    pub has_saves: bool,
    pub show_made_with: bool,
    /// A Quit button (desktop games).
    pub can_quit: bool,
}

pub fn title(ui: &mut Ui, view: &TitleView) -> Option<ShellAction> {
    let colors = ui.theme().colors;
    let screen = ui.screen();
    // Darken the world behind, more at the bottom where the menu sits.
    ui.tint_world(fade(Color::BLACK, 0.4), fade(Color::BLACK, 0.8));
    let skin = ui.skin().title.clone();
    // The skin's background art covers the screen (cropped to keep its shape).
    if let Some(background) = skin.background {
        let src = background.src;
        let scale = (screen.width / src.width).max(screen.height / src.height);
        let (w, h) = (src.width * scale, src.height * scale);
        ui.list_mut().push(DrawCmd::Image {
            image: background.image,
            src,
            dst: Rect::new((screen.width - w) / 2.0, (screen.height - h) / 2.0, w, h),
            opacity: 1.0,
            sampling: background.sampling,
            tint: None,
        });
    }

    // The skin's logo, else the game's title as text.
    let mut y = 0.0;
    if let Some(logo) = skin.logo {
        let room = Rect::new(48.0, screen.height * 0.06, screen.width - 96.0, screen.height * 0.32);
        let src = logo.src;
        let scale = (room.width / src.width).min(room.height / src.height);
        let scale = if logo.sampling == farm_render::Sampling::Nearest && scale >= 1.0 { scale.floor() } else { scale };
        let (w, h) = (src.width * scale, src.height * scale);
        let dst = Rect::new(room.x + (room.width - w) / 2.0, room.y + (room.height - h) / 2.0, w, h);
        ui.list_mut().push(DrawCmd::Image {
            image: logo.image,
            src,
            dst,
            opacity: 1.0,
            sampling: logo.sampling,
            tint: None,
        });
        y = dst.bottom();
    }
    let lines = if skin.logo.is_some() {
        Vec::new()
    } else {
        ui.wrap(&view.title, (screen.width / 16.0).clamp(34.0, 64.0), FontId::Bold, screen.width - 96.0)
    };

    // Logo text: the game's title.
    let title_size = (screen.width / 16.0).clamp(34.0, 64.0);
    let title_px = ui.font_size(title_size);
    let title_font = ui.face(FontId::Bold, &view.title);
    let line_height = ui.line_height(title_size) * 0.92;
    let block = lines.len() as f32 * line_height;
    if !lines.is_empty() {
        y = (screen.height * 0.26 - block / 2.0).max(24.0);
    }
    for line in &lines {
        let baseline = y + (line_height + title_px * 0.727) / 2.0;
        ui.list_mut().push(DrawCmd::Text {
            text: line.clone(),
            x: screen.width / 2.0,
            y: baseline + 3.0,
            font: title_font,
            size: title_px,
            color: fade(Color::BLACK, 0.6),
            stroke: None,
            align: TextAlign::Center,
        });
        ui.list_mut().push(DrawCmd::Text {
            text: line.clone(),
            x: screen.width / 2.0,
            y: baseline,
            font: title_font,
            size: title_px,
            color: colors.accent_text,
            stroke: None,
            align: TextAlign::Center,
        });
        y += line_height;
    }
    if !view.subtitle.is_empty() {
        let rect = Rect::new(0.0, y + 4.0, screen.width, ui.line_height(15.0));
        ui.label(rect, &view.subtitle, 15.0, FontId::Regular, fade(colors.text, 0.85), Align::Center);
        y = rect.bottom();
    }

    // Menu: (widget name, label key, icon, action, enabled).
    let mut entries: Vec<(&str, &'static str, Icon, ShellAction, bool)> = vec![
        ("New Game", "title.newGame", Icon::Play, ShellAction::NewGame, true),
        ("Continue", "title.continue", Icon::Book, ShellAction::Continue, view.can_continue),
        ("Load", "title.load", Icon::Folder, ShellAction::OpenLoad, view.has_saves),
        ("Settings", "title.settings", Icon::Gear, ShellAction::OpenSettings, true),
        ("Credits", "title.credits", Icon::Heart, ShellAction::OpenCredits, true),
    ];
    if view.can_quit {
        entries.push(("Quit", "title.quit", Icon::Exit, ShellAction::Quit, true));
    }
    let button_height = ui.button_height(16.0).max(42.0);
    let gap = 10.0;
    let width = 300.0f32.min(screen.width - 48.0);
    let menu_height = entries.len() as f32 * (button_height + gap) - gap;
    let detail_height = if view.continue_detail.is_some() { ui.line_height(12.5) + 10.0 } else { 0.0 };
    let footer_room = 48.0;
    // Above the host's on-screen controls.
    let safe = ui.safe_area();
    let top = match skin.menu {
        crate::skin::MenuPosition::Bottom => (y + 28.0).max(safe.bottom() - menu_height - detail_height - footer_room),
        _ => (y + 28.0).max((safe.height - menu_height - detail_height - footer_room) * 0.62),
    };
    let menu_x = match skin.menu {
        crate::skin::MenuPosition::Left => 48.0f32.min((screen.width - width) / 2.0),
        crate::skin::MenuPosition::Right => (screen.width - width - 48.0).max((screen.width - width) / 2.0),
        _ => (screen.width - width) / 2.0,
    };
    let mut action = None;
    let default = if view.can_continue { "Continue" } else { "New Game" };
    for (index, (name, key, icon, entry, enabled)) in entries.into_iter().enumerate() {
        let rect = Rect::new(menu_x, top + index as f32 * (button_height + gap), width, button_height);
        let mut button = Button::new(ui.tr(key)).kind(ButtonKind::Menu).icon(icon).size(16.0).enabled(enabled);
        if name == default {
            button = button.default_focus();
        }
        if ui.button(WidgetId::new("title").with(name), rect, button) {
            action = Some(entry);
        }
    }
    if let Some(detail) = &view.continue_detail {
        let rect = Rect::new(0.0, top + menu_height + 10.0, screen.width, ui.line_height(12.5));
        let text = ui.tr_format("title.continueDetail", &[detail]);
        ui.label(rect, &text, 12.5, FontId::Regular, fade(colors.text, 0.75), Align::Center);
    }

    if view.show_made_with {
        let rect = Rect::new(0.0, safe.bottom() - 30.0, screen.width, 20.0);
        let text = ui.tr("hud.madeWith");
        ui.label(rect, text, 12.0, FontId::Regular, fade(colors.text, 0.6), Align::Center);
    }
    action
}
