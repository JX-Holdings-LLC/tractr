//! The mailbox (M6): the letters received, newest first, with their sender and whether they
//! were read; opening one shows its text (placeholders filled) and what it holds, and sends
//! `readMail`, which runs its outcomes the first time and hands over the attachments (again
//! while the inventory had no room for them).

use super::{draw_item_art, GameAction, GameView};
use crate::format::num;
use crate::icons::Icon;
use crate::layout::{Align, RectExt};
use crate::theme::fade;
use crate::ui::{HitKind, Ui, WidgetId};
use crate::widgets::{Button, ModalSpec};
use farm_render::{FontId, ImageStore, Rect, TextAlign};
use farm_sim::schema::{MailDefinition, MailState};
use farm_sim::Command;

/// The letter `id` of the game.
fn letter<'a>(view: &GameView<'a>, id: &str) -> Option<&'a MailDefinition> {
    view.content.mail.iter().find(|letter| letter.id == id)
}

/// Who wrote it: the NPC's name, else nothing.
fn sender(view: &GameView<'_>, definition: &MailDefinition) -> Option<String> {
    let from = definition.from.trim();
    (!from.is_empty()).then(|| {
        view.content.npcs.iter().find(|npc| npc.id == from).map_or_else(|| from.to_owned(), |npc| npc.name.clone())
    })
}

fn fill(view: &GameView<'_>, text: &str) -> String {
    farm_sim::substitution::fill(view.content, view.state, text).into_owned()
}

/// Opens letter `index`: shows it, and has the engine read it unless it was read and emptied.
fn open(index: usize, entry: &MailState, letter: &mut Option<usize>, actions: &mut Vec<GameAction>) {
    *letter = Some(index);
    if !entry.read || !entry.taken {
        actions.push(GameAction::Command(Command::ReadMail { index: index as i32 }));
    }
}

pub(crate) fn draw(
    ui: &mut Ui,
    view: &GameView<'_>,
    images: &mut ImageStore,
    letter: &mut Option<usize>,
    actions: &mut Vec<GameAction>,
) {
    let lang = ui.lang();
    let colors = ui.theme().colors;
    let mailbox = &view.state.mailbox;
    if letter.is_some_and(|index| index >= mailbox.len()) {
        *letter = None;
    }
    let unread = view.overlay.unread_mail;
    let subtitle = match unread {
        0 => lang.format("mail.subtitle", &[&num(mailbox.len())]),
        _ => lang.format("mail.subtitleUnread", &[&num(mailbox.len()), &num(unread)]),
    };
    let reading =
        letter.and_then(|index| Some((index, mailbox.get(index)?, self::letter(view, &mailbox[index].mail_id)?)));
    let footer_height = if reading.is_some() { ui.button_height(13.0) + 20.0 } else { 0.0 };
    let modal = ui.begin_modal(ModalSpec {
        id: WidgetId::new("mail"),
        icon: Icon::Mail,
        title: lang.tr("mail.title"),
        subtitle: Some(&subtitle),
        width: 600.0,
        max_height: 640.0,
        footer: footer_height,
        backdrop: true,
    });
    if modal.close {
        actions.push(GameAction::ClosePanel);
    }
    let area = modal.body;
    let mut y = modal.top;

    if let Some((index, entry, definition)) = reading {
        // The letter.
        let title = ui.line_height(17.0);
        ui.label(
            Rect::new(area.x, y, area.width, title),
            &fill(view, &definition.subject),
            17.0,
            FontId::Bold,
            colors.text,
            Align::Start,
        );
        y += title;
        let from = match sender(view, definition) {
            Some(name) => lang.format("mail.fromOn", &[&name, &num(entry.day)]),
            None => lang.format("mail.on", &[&num(entry.day)]),
        };
        ui.label(
            Rect::new(area.x, y, area.width, ui.line_height(12.5)),
            &from,
            12.5,
            FontId::Regular,
            colors.muted,
            Align::Start,
        );
        y += ui.line_height(12.5) + 10.0;
        let text = fill(view, &definition.text);
        let paper = ui.paragraph_height(&text, 14.0, FontId::Regular, area.width - 32.0) + 28.0;
        let sheet = Rect::new(area.x, y, area.width, paper);
        ui.list_mut().fill_round_rect(sheet, 8.0, fade(colors.accent, 0.1));
        ui.list_mut().stroke_round_rect(sheet.inset(0.5), 8.0, colors.row_border, 1.0);
        ui.paragraph(
            area.x + 16.0,
            y + 14.0,
            area.width - 32.0,
            &text,
            14.0,
            FontId::Regular,
            colors.text,
            TextAlign::Left,
        );
        y += paper + 12.0;
        // What the envelope holds.
        if !definition.attachments.is_empty() || definition.money > 0 {
            let key = if entry.taken { "mail.attachmentsTaken" } else { "mail.attachments" };
            y += ui.section(area, y, lang.tr(key), None);
            let height = 44.0;
            for attachment in &definition.attachments {
                let rect = Rect::new(area.x, y, area.width, height);
                ui.row_background(rect);
                let mut inner = rect.inset_xy(10.0, 6.0);
                let tile = inner.cut_left(32.0).centered(32.0, 32.0);
                if let Some(item) = view.content.items.iter().find(|item| item.id == attachment.item_id) {
                    draw_item_art(ui, images, &view.art, view.content, item, tile);
                }
                inner.cut_left(10.0);
                let name = super::item_name(view.content, &attachment.item_id);
                let text = format!("{name} \u{00d7}{}", num(attachment.quantity));
                let color = if entry.taken { colors.muted } else { colors.text };
                ui.label(inner, &text, 13.5, FontId::Regular, color, Align::Start);
                y += height + 4.0;
            }
            if definition.money > 0 {
                let rect = Rect::new(area.x, y, area.width, ui.line_height(13.5) + 14.0);
                ui.row_background(rect);
                let text = view.money(lang, definition.money);
                ui.label(rect.inset_xy(12.0, 0.0), &text, 13.5, FontId::Bold, colors.text, Align::Start);
                y += rect.height + 4.0;
            }
        }
        ui.end_modal_body(y);
        if let Some(footer) = modal.footer {
            let mut footer = footer;
            let back = Button::new(lang.tr("mail.back")).default_focus();
            let width = ui.button_width(&back);
            let height = ui.button_height(13.0);
            let back_rect = footer.cut_left(width).centered(width, height);
            if ui.button(WidgetId::new("mail-back"), back_rect, back) {
                *letter = None;
            }
            if !entry.taken {
                let take = Button::new(lang.tr("mail.take")).primary();
                let width = ui.button_width(&take);
                let take_rect = footer.cut_right(width).centered(width, height);
                if ui.button(WidgetId::new("mail-take"), take_rect, take) {
                    actions.push(GameAction::Command(Command::ReadMail { index: index as i32 }));
                }
            }
        }
        ui.close_modal();
        return;
    }

    // The letters, newest first.
    if mailbox.is_empty() {
        y += ui.empty_state(area, y, Icon::Mail, lang.tr("mail.empty"), lang.tr("mail.emptyDetail"));
    }
    let title = ui.line_height(14.0);
    let detail = ui.line_height(12.5);
    let height = title + detail + 16.0;
    for (index, entry) in mailbox.iter().enumerate().rev() {
        let Some(definition) = self::letter(view, &entry.mail_id) else { continue };
        let rect = Rect::new(area.x, y, area.width, height);
        let id = WidgetId::new("mail-letter").with(index);
        let state = ui.interact(id, rect, HitKind::Button, true);
        ui.list_mut().fill_round_rect(rect, 8.0, if state.hovered { colors.button_hover } else { colors.row });
        ui.list_mut().stroke_round_rect(rect.inset(0.5), 8.0, colors.row_border, 1.0);
        if state.focus_visible {
            ui.focus_ring(rect, 8.0);
        }
        let mut inner = rect.inset_xy(12.0, 8.0);
        // Unread: a dot and a bold subject.
        let dot = inner.cut_left(16.0);
        if !entry.read {
            let (cx, cy) = (dot.x + 5.0, inner.y + title / 2.0);
            ui.list_mut().fill_circle(cx, cy, 4.5, colors.accent);
        }
        let day = lang.format("mail.day", &[&num(entry.day)]);
        let day_width = ui.measure(&day, 12.5, FontId::Regular).ceil();
        let right = inner.cut_right(day_width);
        ui.label(
            Rect::new(right.x, inner.y, right.width, title),
            &day,
            12.5,
            FontId::Regular,
            colors.muted,
            Align::End,
        );
        if !entry.taken && (!definition.attachments.is_empty() || definition.money > 0) {
            let chip = lang.tr("mail.hasAttachments");
            let width = ui.measure(chip, 11.0, FontId::Bold) + 14.0;
            let cy = inner.y + title + detail / 2.0;
            ui.chip(right.right() - width, cy, chip, 11.0, fade(colors.primary, 0.18), colors.primary);
        }
        inner.cut_right(8.0);
        let font = if entry.read { FontId::Regular } else { FontId::Bold };
        let color = if entry.read { colors.muted } else { colors.text };
        ui.label(
            Rect::new(inner.x, inner.y, inner.width, title),
            &fill(view, &definition.subject),
            14.0,
            font,
            color,
            Align::Start,
        );
        let from = sender(view, definition).map_or_else(String::new, |name| lang.format("mail.from", &[&name]));
        ui.label(
            Rect::new(inner.x, inner.y + title, inner.width, detail),
            &from,
            12.5,
            FontId::Regular,
            colors.muted,
            Align::Start,
        );
        if state.clicked {
            open(index, entry, letter, actions);
        }
        y += height + 8.0;
    }
    ui.end_modal(y);
}
