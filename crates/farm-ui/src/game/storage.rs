//! The open chest or shipping bin (M5, M6): what it holds, with Take 1 and Take all, over the
//! inventory, with Store 1 and Store all. A shipping bin says that what it holds is sold
//! overnight and takes only what can be sold. Every move is an engine command (`storeItem`,
//! `takeStoredItem`), which checks room and quantities again; closing sends `closeStorage`.

use super::shop::two_lines;
use super::{draw_item_art, right_aligned, row, GameAction, GameView};
use crate::format::num;
use crate::icons::Icon;
use crate::layout::RectExt;
use crate::theme::fade;
use crate::ui::{Ui, WidgetId};
use crate::widgets::{Button, ModalSpec};
use farm_render::{ImageStore, Rect};
use farm_sim::schema::{crop_qualities, InventorySlot};
use farm_sim::Command;

const ROW_GAP: f32 = 8.0;

/// One row per item and quality, totalled across the slots holding it.
pub(crate) fn grouped(slots: &[InventorySlot]) -> Vec<(&InventorySlot, u32)> {
    let mut rows: Vec<(&InventorySlot, u32)> = Vec::new();
    for slot in slots.iter().filter(|slot| slot.quantity > 0) {
        match rows.iter_mut().find(|(first, _)| first.item.id == slot.item.id && first.quality == slot.quality) {
            Some((_, total)) => *total = total.saturating_add(slot.quantity),
            None => rows.push((slot, slot.quantity)),
        }
    }
    rows
}

/// The art tile at the left of a row's content; returns the content right of it.
fn art_tile(ui: &mut Ui, view: &GameView<'_>, images: &mut ImageStore, content: Rect, slot: &InventorySlot) -> Rect {
    let colors = ui.theme().colors;
    let mut content = content;
    let side = content.height.min(40.0);
    let tile = content.cut_left(side).centered(side, side);
    ui.list_mut().fill_round_rect(tile, 8.0, fade(colors.accent, 0.18));
    draw_item_art(ui, images, &view.art, view.content, &slot.item, tile.inset(side * 0.15));
    content.cut_left(10.0);
    content
}

pub(crate) fn draw(ui: &mut Ui, view: &GameView<'_>, images: &mut ImageStore, actions: &mut Vec<GameAction>) {
    let Some(storage) = view.overlay.storage.as_ref() else { return };
    let lang = ui.lang();
    let state = view.state;
    let used = storage.contents.iter().filter(|slot| slot.quantity > 0).count();
    let subtitle = if storage.shipping {
        lang.tr("storage.shippingSubtitle").to_owned()
    } else {
        lang.format("storage.slotsUsed", &[&used, &num(storage.slots)])
    };
    let modal = ui.begin_modal(ModalSpec {
        id: WidgetId::new("storage"),
        icon: if storage.shipping { Icon::Coin } else { Icon::Package },
        title: &storage.name,
        subtitle: Some(&subtitle),
        width: 640.0,
        max_height: 660.0,
        footer: 0.0,
        backdrop: true,
    });
    if modal.close {
        actions.push(GameAction::Command(Command::CloseStorage));
    }
    let area = modal.body;
    let mut y = modal.top;
    let small = ui.button_height(12.5);
    let row_height = (ui.line_height(14.0) + ui.line_height(12.5) + 16.0).max(small + 16.0).max(52.0);

    // What it holds, slot by slot (`takeStoredItem` takes from a slot).
    let heading = if storage.shipping { "storage.toShip" } else { "storage.contents" };
    y += ui.section(area, y, lang.tr(heading), None);
    let held: Vec<(usize, &InventorySlot)> =
        storage.contents.iter().enumerate().filter(|(_, slot)| slot.quantity > 0).collect();
    if held.is_empty() {
        let (title, detail) = if storage.shipping {
            ("storage.binEmpty", "storage.binEmptyDetail")
        } else {
            ("storage.empty", "storage.emptyDetail")
        };
        y += ui.empty_state(area, y, Icon::Package, lang.tr(title), lang.tr(detail));
    }
    for (index, slot) in held {
        let take_one = Button::new(lang.tr("storage.takeOne")).size(12.5);
        let take_all = Button::new(lang.tr("storage.takeAll")).primary().size(12.5);
        let mut widths = vec![ui.button_width(&take_one)];
        if slot.quantity > 1 {
            widths.push(ui.button_width(&take_all));
        }
        let actions_width = widths.iter().sum::<f32>() + 6.0 * (widths.len() - 1) as f32;
        let (content, buttons) = row(ui, area, y, row_height, actions_width);
        let content = art_tile(ui, view, images, content, slot);
        let name = super::slot_name(view.content, &slot.item.name, slot.quality.as_deref());
        let quantity = format!("\u{00d7}{}", num(slot.quantity));
        let value = farm_sim::economy::quality_value(view.content, &slot.item, slot.quality.as_deref());
        let detail = if storage.shipping {
            lang.format("storage.shipsFor", &[&view.money(lang, value.saturating_mul(i64::from(slot.quantity)))])
        } else {
            slot.item.description.clone()
        };
        two_lines(ui, content, &name, Some(&quantity), &detail, false);
        let rects = right_aligned(buttons, &widths, small, 6.0);
        let id = WidgetId::new("storage-take").with(index).with(&slot.item.id);
        let take = |quantity| GameAction::Command(Command::TakeStoredItem { index: index as i32, quantity });
        if ui.button(id, rects[0], take_one) {
            actions.push(take(1));
        }
        if slot.quantity > 1 && ui.button(id.with("all"), rects[1], take_all) {
            actions.push(take(slot.quantity));
        }
        y += row_height + ROW_GAP;
    }

    // The inventory, one row per item and quality (`storeItem` with that quality). A shipping
    // bin takes only what can be sold.
    y += 6.0;
    y += ui.section(area, y, lang.tr("storage.inventory"), None);
    let rows: Vec<_> = grouped(&state.player.inventory)
        .into_iter()
        .filter(|(slot, _)| {
            !storage.shipping || super::category(view.content, &slot.item).is_none_or(|kind| kind.sellable)
        })
        .collect();
    if rows.is_empty() {
        let (title, detail) = if storage.shipping {
            ("storage.nothingToShip", "storage.nothingToShipDetail")
        } else {
            ("inventory.empty", "inventory.emptyDetail")
        };
        y += ui.empty_state(area, y, Icon::Package, lang.tr(title), lang.tr(detail));
    }
    let plain = farm_sim::vocabulary::crop_qualities(view.content).first().map(|tier| tier.id.clone());
    for (slot, held) in rows {
        let store_one = Button::new(lang.tr("storage.storeOne")).size(12.5);
        let store_all = Button::new(lang.tr("storage.storeAll")).primary().size(12.5);
        let mut widths = vec![ui.button_width(&store_one)];
        if held > 1 {
            widths.push(ui.button_width(&store_all));
        }
        let actions_width = widths.iter().sum::<f32>() + 6.0 * (widths.len() - 1) as f32;
        let (content, buttons) = row(ui, area, y, row_height, actions_width);
        let content = art_tile(ui, view, images, content, slot);
        let name = super::slot_name(view.content, &slot.item.name, slot.quality.as_deref());
        let quantity = format!("\u{00d7}{}", num(held));
        let value = farm_sim::economy::quality_value(view.content, &slot.item, slot.quality.as_deref());
        let detail = if storage.shipping {
            lang.format("shop.each", &[&view.money(lang, value)])
        } else {
            slot.item.description.clone()
        };
        two_lines(ui, content, &name, Some(&quantity), &detail, false);
        let rects = right_aligned(buttons, &widths, small, 6.0);
        let mut id = WidgetId::new("storage-store").with(&slot.item.id);
        if let Some(quality) = &slot.quality {
            id = id.with(quality);
        }
        // The row's own quality (normal included), never another quality's units.
        let quality = slot.quality.clone().or(plain.clone()).unwrap_or_else(|| crop_qualities::NORMAL.to_owned());
        let store = |quantity| {
            GameAction::Command(Command::StoreItem {
                item_id: slot.item.id.clone(),
                quantity,
                quality: Some(quality.clone()),
            })
        };
        if ui.button(id, rects[0], store_one) {
            actions.push(store(1));
        }
        if held > 1 && ui.button(id.with("all"), rects[1], store_all) {
            actions.push(store(held));
        }
        y += row_height + ROW_GAP;
    }
    ui.end_modal(y);
}
