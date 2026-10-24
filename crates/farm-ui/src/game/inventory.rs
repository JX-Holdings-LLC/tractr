//! The inventory (web PlayerInventory, C# `PlayOverlays.Inventory`): every slot with its art,
//! quantity, value and durability; Hold picks the seed and fertilizer to plant with, Use runs the
//! item's bound action, Wear puts on clothing (M4; what is worn is listed first, with Take off),
//! Gift gives it to the NPC the player faces; the footer totals the inventory's value.

use super::{draw_item_art, right_aligned, GameAction, GameView};
use crate::format::num;
use crate::icons::Icon;
use crate::layout::{Align, RectExt};
use crate::theme::fade;
use crate::ui::{Ui, WidgetId};
use crate::widgets::{Button, ModalSpec};
use farm_render::{FontId, ImageStore, Rect};
use farm_sim::Command;

pub(crate) fn draw(ui: &mut Ui, view: &GameView<'_>, images: &mut ImageStore, actions: &mut Vec<GameAction>) {
    let colors = ui.theme().colors;
    let player = &view.state.player;
    let lang = ui.lang();
    let subtitle = lang.format("inventory.slotsUsed", &[&player.inventory.len(), &num(player.max_inventory_size)]);
    let footer_height = ui.button_height(13.0) + 20.0;
    let modal = ui.begin_modal(ModalSpec {
        id: WidgetId::new("inventory"),
        icon: Icon::Package,
        title: lang.tr("inventory.title"),
        subtitle: Some(&subtitle),
        width: 680.0,
        max_height: 640.0,
        footer: footer_height,
        backdrop: true,
    });
    if modal.close {
        actions.push(GameAction::ClosePanel);
    }
    let area = modal.body;
    let mut y = modal.top;
    // What the player wears (M4), each with Take off.
    for worn in &player.worn {
        let Some(slot) = worn.item.wear.as_ref().map(|wear| wear.slot.as_str()) else { continue };
        let row = Rect::new(area.x, y, area.width, ui.button_height(12.0) + 8.0);
        let take_off = Button::new(lang.tr("inventory.takeOff")).size(12.0);
        let mut inner = row;
        let width = ui.button_width(&take_off);
        let button = inner.cut_right(width).centered(width, ui.button_height(12.0));
        if ui.button(WidgetId::new("inventory-take-off").with(slot), button, take_off) {
            actions.push(GameAction::Command(Command::TakeOff { slot: slot.to_owned() }));
        }
        let text = lang.format("inventory.wearing", &[&worn.item.name]);
        ui.label(inner, &text, 13.0, FontId::Regular, colors.text, Align::Start);
        y += row.height + 4.0;
    }
    if player.inventory.is_empty() {
        y += ui.empty_state(area, y, Icon::Package, lang.tr("inventory.empty"), lang.tr("inventory.emptyDetail"));
    } else {
        let columns = if area.width >= 560.0 { 2 } else { 1 };
        let gap = 8.0;
        let column_width = (area.width - gap * (columns - 1) as f32) / columns as f32;
        let small = ui.button_height(12.0);
        let height = (ui.line_height(14.0) + ui.line_height(12.5) + ui.line_height(12.0) + 18.0).max(62.0);
        for (index, slot) in player.inventory.iter().enumerate() {
            let item = &slot.item;
            let column = index % columns;
            let rect = Rect::new(area.x + column as f32 * (column_width + gap), y, column_width, height);
            ui.row_background(rect);
            let mut inner = rect.inset_xy(10.0, 8.0);

            // Buttons: Hold (seeds and fertilizer, for planting), Use (bound action), Gift (to
            // the faced NPC; not tools).
            let held = view.planting.holds(&item.id);
            let hold_button = if held {
                Button::new(lang.tr("inventory.held")).primary().size(12.0)
            } else {
                Button::new(lang.tr("inventory.hold")).size(12.0)
            };
            let use_label = if item.use_action_id.as_deref().is_some_and(|id| !id.is_empty()) {
                lang.tr("inventory.use")
            } else {
                lang.tr("inventory.eat")
            };
            let use_button = Button::new(use_label).size(12.0);
            let gift_button = Button::new(lang.tr("inventory.gift")).size(12.0);
            let wear_button = Button::new(lang.tr("inventory.wear")).size(12.0);
            // Clothing (M4) is put on with Wear.
            let can_wear = item.wear.is_some();
            let kind = super::category(view.content, item);
            let can_hold = kind.is_some_and(|kind| kind.plantable || kind.fertilizer);
            let has_action = item.use_action_id.as_deref().is_some_and(|id| !id.is_empty());
            // An edible item (M1) is eaten with Use.
            let can_use = has_action || kind.is_some_and(|kind| kind.edible);
            let can_gift = kind.is_none_or(|kind| kind.giftable);
            let mut widths = Vec::new();
            if can_hold {
                widths.push(ui.button_width(&hold_button));
            }
            if can_use {
                widths.push(ui.button_width(&use_button));
            }
            if can_wear {
                widths.push(ui.button_width(&wear_button));
            }
            if can_gift {
                widths.push(ui.button_width(&gift_button));
            }
            let buttons_width = widths.iter().sum::<f32>() + 4.0 * widths.len().saturating_sub(1) as f32;
            let buttons = inner.cut_right(buttons_width);
            let rects = right_aligned(buttons, &widths, small, 4.0);
            let id = WidgetId::new("inventory").with(index).with(&item.id);
            let mut next = 0;
            if can_hold {
                if ui.button(id.with("hold"), rects[next], hold_button) {
                    actions.push(GameAction::Hold(item.id.clone()));
                }
                next += 1;
            }
            if can_use {
                if ui.button(id.with("use"), rects[next], use_button) {
                    actions.push(GameAction::ClosePanel);
                    actions.push(GameAction::Command(Command::UseItem { item_id: item.id.clone() }));
                }
                next += 1;
            }
            if can_wear {
                if ui.button(id.with("wear"), rects[next], wear_button) {
                    actions.push(GameAction::Command(Command::WearItem { item_id: item.id.clone() }));
                }
                next += 1;
            }
            if can_gift && ui.button(id.with("gift"), rects[next], gift_button) {
                actions.push(GameAction::Command(Command::GiveGift { item_id: item.id.clone() }));
            }

            // Art tile.
            let tile = inner.cut_left(44.0).centered(44.0, 44.0);
            ui.list_mut().fill_round_rect(tile, 8.0, fade(colors.accent, 0.18));
            ui.list_mut().stroke_round_rect(tile.inset(0.5), 8.0, colors.row_border, 1.0);
            draw_item_art(ui, images, &view.art, view.content, item, tile.centered(32.0, 32.0));
            inner.cut_left(10.0);
            inner.cut_right(6.0);

            // Name, description, meta.
            let (title, description, meta) = (ui.line_height(14.0), ui.line_height(12.5), ui.line_height(12.0));
            let top = inner.y + (inner.height - title - description - meta) / 2.0;
            ui.label(
                Rect::new(inner.x, top, inner.width, title),
                &super::slot_name(view.content, &item.name, slot.quality.as_deref()),
                14.0,
                FontId::Bold,
                colors.text,
                Align::Start,
            );
            ui.label(
                Rect::new(inner.x, top + title, inner.width, description),
                &item.description,
                12.5,
                FontId::Regular,
                colors.muted,
                Align::Start,
            );
            let mut meta_row = Rect::new(inner.x, top + title + description, inner.width, meta);
            let cy = meta_row.y + meta_row.height / 2.0;
            if item.stackable {
                let width = ui.chip(
                    meta_row.x,
                    cy,
                    &format!("\u{00d7}{}", num(slot.quantity)),
                    11.0,
                    fade(colors.primary, 0.18),
                    colors.primary,
                );
                meta_row.cut_left(width + 8.0);
            }
            let mut meta_text =
                view.money(lang, farm_sim::economy::quality_value(view.content, item, slot.quality.as_deref()));
            if kind.is_some_and(|kind| kind.equippable) {
                if let (Some(durability), Some(max)) = (item.durability, item.max_durability) {
                    meta_text.push_str(&format!("  {}/{}", num(durability), num(max)));
                }
            }
            ui.label(meta_row, &meta_text, 12.0, FontId::Regular, colors.muted, Align::Start);
            if column == columns - 1 || index == player.inventory.len() - 1 {
                y += height + gap;
            }
        }
    }
    ui.end_modal_body(y);
    if let Some(footer) = modal.footer {
        let total: i64 = player
            .inventory
            .iter()
            .map(|slot| {
                farm_sim::economy::quality_value(view.content, &slot.item, slot.quality.as_deref())
                    .saturating_mul(i64::from(slot.quantity))
            })
            .sum();
        let mut footer = footer;
        let close = Button::new(lang.tr("common.close")).primary();
        let width = ui.button_width(&close);
        let close_rect = footer.cut_right(width).centered(width, ui.button_height(13.0));
        let text = lang.format("inventory.totalValue", &[&view.money(lang, total)]);
        ui.label(footer, &text, 13.5, FontId::Regular, colors.muted, Align::Start);
        if ui.button(WidgetId::new("inventory-close"), close_rect, close) {
            actions.push(GameAction::ClosePanel);
        }
    }
    ui.close_modal();
}
