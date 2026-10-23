//! The minigame overlay (web MinigameOverlay, C# `MinigameOverlay`): the prompt, then per kind a
//! track (the timing bar's marker and target zone, the moving target over the player's bar, the
//! rhythm notes scrolling to their line) with its meters, the status and log, the choice buttons
//! (a battle's moves, a memory game's symbols) or the primary button. Presses and releases go to
//! `PlaySession::minigame_input`; the score enters the command log there, exactly once. Engine
//! texts show in the player's language (their catalog messages); a def's own prompt as written.
//!
//! A pack-defined kind brings its own frame (`MinigameView::canvas`, validated by
//! `farm_runtime::canvas`): its rectangles, text and item sprites are scaled from canvas units
//! into the window and clipped to it, colors by name follow the theme, and its input comes from
//! the game keys and the pointer over the frame (reported in canvas units as
//! [`GameAction::MinigamePointer`]), so only Give up is a button.

use super::{draw_item_art, GameAction, GameView};
use crate::format::capitalize;
use crate::i18n::Lang;
use crate::icons::Icon;
use crate::input::InputDevice;
use crate::layout::{Align, RectExt};
use crate::theme::fade;
use crate::ui::{HitKind, Ui, WidgetId};
use crate::widgets::{Button, ButtonKind, ModalSpec};
use farm_render::{Color, DrawCmd, FontId, ImageStore, Rect, TextAlign};
use farm_runtime::canvas::{
    CanvasAlign, CanvasColor, CanvasCommand, CanvasPointer, MinigameCanvas, ThemeColor, CANVAS_HEIGHT, CANVAS_WIDTH,
};
use farm_runtime::host::MinigameInput;
use farm_sim::Command;

const BAR_WIDTH: f32 = 320.0;
const BAR_HEIGHT: f32 = 26.0;

pub(crate) fn draw(ui: &mut Ui, view: &GameView<'_>, images: &mut ImageStore, actions: &mut Vec<GameAction>) {
    let (Some(active), Some(game)) = (view.state.minigame.as_ref(), view.minigame) else { return };
    let colors = ui.theme().colors;
    let definition = view.content.minigames.iter().find(|def| def.id == active.minigame_id);
    let title = definition.map_or(active.minigame_id.as_str(), |def| def.name.as_str());
    let footer = ui.button_height(13.0) + 20.0;
    let modal = ui.begin_modal(ModalSpec {
        id: WidgetId::new("minigame"),
        icon: Icon::Star,
        title,
        subtitle: definition.map(|def| def.kind.as_str()),
        width: 440.0,
        max_height: 560.0,
        footer,
        backdrop: true,
    });
    if modal.close {
        actions.push(GameAction::Command(Command::CancelMinigame));
    }
    let area = modal.body;
    let lang = ui.lang();
    let mut y = modal.top;
    if let Some(canvas) = &game.canvas {
        y = draw_canvas(ui, view, images, canvas, area, y, actions);
        let hint = ui.tr("minigame.packKeys");
        let colors = ui.theme().colors;
        ui.label(
            Rect::new(area.x, y, area.width, ui.line_height(12.0)),
            hint,
            12.0,
            FontId::Regular,
            colors.muted,
            Align::Center,
        );
        y += ui.line_height(12.0);
        ui.end_modal_body(y);
        if let Some(mut footer) = modal.footer {
            // Not focusable: Space and Enter belong to the minigame.
            let give_up = Button::new(ui.tr("minigame.giveUp")).kind(ButtonKind::Secondary);
            let width = ui.button_width(&give_up);
            let rect = footer.cut_right(width).centered(width, ui.button_height(13.0));
            if ui.button(WidgetId::new("minigame-give-up"), rect, give_up.focusable(false)) {
                actions.push(GameAction::Command(Command::CancelMinigame));
            }
        }
        ui.close_modal();
        return;
    }
    let prompt = lang.localize(game.prompt_message.as_ref(), &game.prompt);
    y += ui.paragraph(area.x, y, area.width, &prompt, 14.0, FontId::Regular, colors.text, TextAlign::Center) + 14.0;

    let choices = !game.choices.is_empty();
    let track_kind = matches!(game.kind.as_str(), "timing-bar" | "moving-target" | "rhythm-tap");
    if track_kind {
        let track = Rect::new(area.x + (area.width - BAR_WIDTH) / 2.0, y, BAR_WIDTH, BAR_HEIGHT);
        ui.list_mut().fill_round_rect(track, 6.0, colors.track);
        let zone = Rect::new(
            track.x + game.target_left as f32 * BAR_WIDTH,
            track.y,
            game.target_size as f32 * BAR_WIDTH,
            BAR_HEIGHT,
        );
        ui.push_clip(track);
        ui.list_mut().fill_rect(zone, fade(colors.primary, 0.35));
        ui.list_mut().stroke_rect(zone.inset(0.5), colors.primary, 1.0);
        if game.kind == "rhythm-tap" {
            for note in &game.notes {
                let x = track.x + (*note as f32).clamp(0.0, 1.0) * BAR_WIDTH;
                ui.list_mut().fill_round_rect(
                    Rect::new(x - 5.0, track.y + 4.0, 10.0, BAR_HEIGHT - 8.0),
                    4.0,
                    colors.accent,
                );
            }
        } else {
            let marker_x = track.x + (game.position as f32).clamp(0.0, 1.0) * (BAR_WIDTH - 3.0);
            ui.list_mut().fill_rect(Rect::new(marker_x, track.y, 3.0, BAR_HEIGHT), colors.accent);
        }
        ui.pop_clip();
        y += BAR_HEIGHT + 14.0;
        if game.kind != "timing-bar" {
            // The score so far and the time played, as two thin meters under the track.
            for (value, color) in [(game.meter, colors.primary), (game.progress, colors.muted)] {
                let meter = Rect::new(track.x, y, BAR_WIDTH, 5.0);
                ui.list_mut().fill_round_rect(meter, 2.5, colors.track);
                let filled = Rect::new(meter.x, meter.y, (value as f32).clamp(0.0, 1.0) * BAR_WIDTH, meter.height);
                ui.list_mut().fill_round_rect(filled, 2.5, color);
                y += 9.0;
            }
            y += 6.0;
        }
    }
    let status = lang.localize(game.status_message.as_ref(), &game.status);
    if !status.is_empty() {
        y += ui.paragraph(area.x, y, area.width, &status, 13.5, FontId::Bold, colors.text, TextAlign::Center) + 6.0;
    }
    if !game.highlight.is_empty() {
        let symbol = choice_label(lang, &game.highlight);
        y += ui.paragraph(area.x, y, area.width, &symbol, 22.0, FontId::Bold, colors.accent, TextAlign::Center) + 8.0;
    }
    if choices {
        let log = lang.localize(game.log_message.as_ref(), &game.log);
        y += ui.paragraph(area.x, y, area.width, &log, 12.5, FontId::Regular, colors.muted, TextAlign::Center) + 12.0;
        let buttons: Vec<(String, &String)> =
            game.choices.iter().map(|choice| (choice_label(lang, choice), choice)).collect();
        let widths: Vec<f32> =
            buttons.iter().map(|(label, _)| ui.button_width(&Button::new(label)).max(84.0)).collect();
        let total = widths.iter().sum::<f32>() + 8.0 * (widths.len() - 1) as f32;
        let mut x = area.x + (area.width - total) / 2.0;
        let height = ui.button_height(13.0);
        for (index, ((label, choice), width)) in buttons.iter().zip(&widths).enumerate() {
            let mut button = Button::new(label).enabled(!game.is_done);
            if index == 0 {
                button = button.default_focus();
            }
            if ui.button(
                WidgetId::new("minigame-choice").with(choice.as_str()),
                Rect::new(x, y, *width, height),
                button,
            ) {
                actions.push(GameAction::Minigame(MinigameInput::Act { choice: (*choice).clone() }));
            }
            x += width + 8.0;
        }
        y += height + 8.0;
    } else {
        // Press/release semantics (hold-to-catch needs both).
        let label = lang.localize(game.button_message.as_ref(), &game.button_text);
        let button = Button::new(&label).primary().size(15.0).default_focus();
        let width = ui.button_width(&button).max(180.0);
        let height = ui.button_height(15.0) + 8.0;
        let rect = Rect::new(area.x + (area.width - width) / 2.0, y, width, height);
        let state = ui.hold_button(WidgetId::new("minigame-primary"), rect, button);
        if state.pressed {
            actions.push(GameAction::Minigame(MinigameInput::Press));
        }
        if state.released {
            actions.push(GameAction::Minigame(MinigameInput::Release));
        }
        y += height + 6.0;
        // Gamepad A, or the touch controls' A button.
        let hint =
            if matches!(ui.device(), InputDevice::Gamepad | InputDevice::Touch) { "A" } else { ui.tr("minigame.keys") };
        ui.label(
            Rect::new(area.x, y, area.width, ui.line_height(12.0)),
            hint,
            12.0,
            FontId::Regular,
            colors.muted,
            Align::Center,
        );
        y += ui.line_height(12.0);
    }
    ui.end_modal_body(y);
    if let Some(mut footer) = modal.footer {
        let give_up = Button::new(ui.tr("minigame.giveUp")).kind(ButtonKind::Secondary);
        let width = ui.button_width(&give_up);
        let rect = footer.cut_right(width).centered(width, ui.button_height(13.0));
        if ui.button(WidgetId::new("minigame-give-up"), rect, give_up.focusable(choices)) {
            actions.push(GameAction::Command(Command::CancelMinigame));
        }
    }
    ui.close_modal();
}

/// A theme color or a literal one.
fn canvas_color(ui: &Ui, color: CanvasColor) -> Color {
    let colors = ui.theme().colors;
    match color {
        CanvasColor::Rgba([r, g, b, a]) => Color::rgba(r, g, b, a),
        CanvasColor::Theme(theme) => match theme {
            ThemeColor::Text => colors.text,
            ThemeColor::Muted => colors.muted,
            ThemeColor::Primary => colors.primary,
            ThemeColor::Accent => colors.accent,
            ThemeColor::Track => colors.track,
            ThemeColor::Panel => colors.panel,
            ThemeColor::Background => colors.background,
            ThemeColor::Error => colors.error,
        },
    }
}

/// Draws a pack minigame's frame at `top`, as wide as `area` (at most twice the canvas size),
/// clipped to the canvas; returns the y below it. The pointer over the frame, or held after
/// going down on it, is reported in canvas units; a press anywhere else is not.
fn draw_canvas(
    ui: &mut Ui,
    view: &GameView<'_>,
    images: &mut ImageStore,
    canvas: &MinigameCanvas,
    area: Rect,
    top: f32,
    actions: &mut Vec<GameAction>,
) -> f32 {
    let scale = (area.width / CANVAS_WIDTH).min(2.0);
    let frame =
        Rect::new(area.x + (area.width - CANVAS_WIDTH * scale) / 2.0, top, CANVAS_WIDTH * scale, CANVAS_HEIGHT * scale);
    let pointer = ui.interact(WidgetId::new("minigame-canvas"), frame, HitKind::Button, false);
    if let Some((x, y)) = ui.pointer() {
        if pointer.hovered || pointer.active || pointer.pressed || pointer.released {
            let (x, y) = ((x - frame.x) / scale, (y - frame.y) / scale);
            let pointer = CanvasPointer::at(x, y, pointer.active, pointer.pressed, pointer.released);
            actions.push(GameAction::MinigamePointer(pointer));
        }
    }
    let at = |x: f32, y: f32, w: f32, h: f32| Rect::new(frame.x + x * scale, frame.y + y * scale, w * scale, h * scale);
    let track = ui.theme().colors.track;
    ui.list_mut().fill_round_rect(frame, 6.0, track);
    ui.push_clip(frame);
    for command in &canvas.commands {
        match command {
            CanvasCommand::Rect { x, y, w, h, color, radius, outline } => {
                let rect = at(*x, *y, *w, *h);
                let color = canvas_color(ui, *color);
                match outline {
                    Some(width) => ui.list_mut().stroke_rect(rect, color, width * scale),
                    None if *radius > 0.0 => ui.list_mut().fill_round_rect(rect, radius * scale, color),
                    None => ui.list_mut().fill_rect(rect, color),
                }
            }
            CanvasCommand::Text { x, y, text, size, color, align, bold } => {
                let font = ui.face(if *bold { FontId::Bold } else { FontId::Regular }, text);
                let color = canvas_color(ui, *color);
                let align = match align {
                    CanvasAlign::Left => TextAlign::Left,
                    CanvasAlign::Center => TextAlign::Center,
                    CanvasAlign::Right => TextAlign::Right,
                };
                // Canvas units, not the text-size setting: the plugin laid the frame out.
                ui.list_mut().push(DrawCmd::Text {
                    text: text.clone(),
                    x: frame.x + x * scale,
                    y: frame.y + y * scale,
                    font,
                    size: size * scale,
                    color,
                    stroke: None,
                    align,
                });
            }
            CanvasCommand::Sprite { x, y, w, h, item } => {
                let found = canvas
                    .sprite_candidates(item)
                    .find_map(|id| view.content.items.iter().find(|candidate| candidate.id == id));
                if let Some(found) = found {
                    draw_item_art(ui, images, &view.art, view.content, found, at(*x, *y, *w, *h));
                }
            }
        }
    }
    ui.pop_clip();
    frame.bottom() + 10.0
}

/// A choice's label: `minigame.choice.<id>` in the player's language, else the id capitalized.
fn choice_label(lang: Lang, choice: &str) -> String {
    lang.get(&format!("minigame.choice.{choice}")).map_or_else(|| capitalize(choice), str::to_owned)
}
