//! The shop (web ShopDialog, C# `PlayOverlays.Shop`): buy the season's stock within daily limits,
//! sell inventory, repair tools; a builder's buildings and the animals a shop sells (M5,
//! [`super::building`]). Buttons are enabled from the engine's own numbers (stock left from
//! `farm_sim::overlay`); the purchase itself is an engine command that checks again.

use super::building::{self, Placement};
use super::{right_aligned, row, GameAction, GameView, ShopTab};
use crate::format::num;
use crate::icons::Icon;
use crate::layout::{Align, RectExt};
use crate::ui::{Ui, WidgetId};
use crate::widgets::{Button, ButtonKind, ModalSpec};
use farm_render::{FontId, Rect};
use farm_sim::economy;
use farm_sim::schema::{crop_qualities, InventorySlot};
use farm_sim::{tools, Command};

const ROW_GAP: f32 = 8.0;

/// The tab's stable name (widget ids) and its label key.
fn tab_names(tab: ShopTab) -> (&'static str, &'static str) {
    match tab {
        ShopTab::Buy => ("Buy", "shop.buy"),
        ShopTab::Sell => ("Sell", "shop.sell"),
        ShopTab::Repair => ("Repair", "shop.repair"),
        ShopTab::Build => ("Build", "shop.build"),
        ShopTab::Animals => ("Animals", "shop.animals"),
    }
}

/// Title and detail lines in a row's content area.
pub(crate) fn two_lines(
    ui: &mut Ui,
    area: Rect,
    title: &str,
    trailing: Option<&str>,
    detail: &str,
    detail_error: bool,
) {
    let colors = ui.theme().colors;
    let title_height = ui.line_height(14.0);
    let detail_height = ui.line_height(12.5);
    let top = area.y + (area.height - title_height - detail_height) / 2.0;
    let mut title_rect = Rect::new(area.x, top, area.width, title_height);
    let title_width = ui.label(title_rect, title, 14.0, FontId::Bold, colors.text, Align::Start);
    if let Some(trailing) = trailing {
        title_rect.cut_left(title_width + 6.0);
        ui.label(title_rect, trailing, 12.5, FontId::Regular, colors.muted, Align::Start);
    }
    let detail_rect = Rect::new(area.x, top + title_height, area.width, detail_height);
    ui.label(
        detail_rect,
        detail,
        12.5,
        FontId::Regular,
        if detail_error { colors.error } else { colors.muted },
        Align::Start,
    );
}

pub(crate) fn draw(
    ui: &mut Ui,
    view: &GameView<'_>,
    tab: &mut ShopTab,
    placement: &mut Option<Placement>,
    actions: &mut Vec<GameAction>,
) {
    if view.state.shop.is_none() {
        return;
    }
    let Some(shop) = view.overlay.shop.as_deref() else { return };
    let state = view.state;
    let player = &state.player;
    let colors = ui.theme().colors;
    let lang = ui.lang();

    // A builder or an animal shop without stock opens on what it does sell.
    let builds = !view.overlay.building_options.is_empty();
    let animals = !view.overlay.animal_options.is_empty();
    let mut tabs = Vec::new();
    if !shop.stock.is_empty() || !(builds || animals) {
        tabs.push(ShopTab::Buy);
    }
    if builds {
        tabs.push(ShopTab::Build);
    }
    if animals {
        tabs.push(ShopTab::Animals);
    }
    if shop.buys_items {
        tabs.push(ShopTab::Sell);
    }
    if shop.repairs_tools
        || shop.tool_upgrades.as_ref().is_some_and(|rules| !rules.is_empty())
        || !player.tool_upgrades.is_empty()
    {
        tabs.push(ShopTab::Repair);
    }
    if !tabs.contains(tab) {
        *tab = tabs[0];
    }
    if view.keys_active && ui.tab_delta() != 0 {
        let current = tabs.iter().position(|t| t == tab).unwrap_or(0) as i32;
        let next = (current + ui.tab_delta()).rem_euclid(tabs.len() as i32) as usize;
        *tab = tabs[next];
    }

    let subtitle = lang.format("shop.yourMoney", &[&view.money(lang, player.money)]);
    let modal = ui.begin_modal(ModalSpec {
        id: WidgetId::new("shop"),
        icon: Icon::Store,
        title: &shop.name,
        subtitle: Some(&subtitle),
        width: 600.0,
        max_height: 620.0,
        footer: 0.0,
        backdrop: true,
    });
    if modal.close {
        actions.push(GameAction::Command(Command::CloseShop));
    }
    let area = modal.body;
    let mut y = modal.top;

    // Tabs.
    let tab_height = ui.button_height(13.0);
    let mut x = area.x;
    for candidate in &tabs {
        let (name, label) = tab_names(*candidate);
        let button = Button::new(lang.tr(label)).kind(ButtonKind::Tab { selected: candidate == tab });
        let width = ui.button_width(&button).max(72.0);
        let id = WidgetId::new("shop-tab").with(name);
        if ui.button(id, Rect::new(x, y, width, tab_height), button) {
            *tab = *candidate;
        }
        x += width + 6.0;
    }
    y += tab_height + 12.0;

    let small = ui.button_height(12.5);
    let row_height = (ui.line_height(14.0) + ui.line_height(12.5) + 16.0).max(small + 16.0);
    match *tab {
        ShopTab::Build => y = building::build_rows(ui, view, area, y, placement, actions),
        ShopTab::Animals => y = building::animal_rows(ui, view, area, y, actions),
        ShopTab::Sell => {
            // One row per item and quality, totalled across the slots holding it.
            let mut sellable: Vec<(&InventorySlot, u32)> = Vec::new();
            for slot in player
                .inventory
                .iter()
                .filter(|slot| super::category(view.content, &slot.item).is_none_or(|kind| kind.sellable))
            {
                match sellable
                    .iter_mut()
                    .find(|(first, _)| first.item.id == slot.item.id && first.quality == slot.quality)
                {
                    Some((_, total)) => *total = total.saturating_add(slot.quantity),
                    None => sellable.push((slot, slot.quantity)),
                }
            }
            if sellable.is_empty() {
                y += ui.empty_state(
                    area,
                    y,
                    Icon::Package,
                    lang.tr("shop.nothingToSell"),
                    lang.tr("shop.nothingToSellDetail"),
                );
            }
            for (slot, held) in sellable {
                let unit = farm_sim::commerce::price(
                    view.ctx,
                    state,
                    shop,
                    &slot.item.id,
                    economy::sell_unit_price_with_quality(view.content, &slot.item, slot.quality.as_deref(), shop),
                    true,
                );
                let sell_one = Button::new(lang.tr("shop.sellOne")).primary().size(12.5);
                let sell_all = Button::new(lang.tr("shop.sellAll")).size(12.5);
                let mut widths = vec![ui.button_width(&sell_one)];
                if held > 1 {
                    widths.push(ui.button_width(&sell_all));
                }
                let actions_width = widths.iter().sum::<f32>() + 6.0 * (widths.len() - 1) as f32;
                let (content, buttons) = row(ui, area, y, row_height, actions_width);
                let quantity = slot.item.stackable.then(|| format!("\u{00d7}{}", num(held)));
                let each = lang.format("shop.each", &[&view.money(lang, unit)]);
                let name = super::slot_name(view.content, &slot.item.name, slot.quality.as_deref());
                two_lines(ui, content, &name, quantity.as_deref(), &each, false);
                let rects = right_aligned(buttons, &widths, small, 6.0);
                let mut id = WidgetId::new("shop-sell").with(&slot.item.id);
                if let Some(quality) = &slot.quality {
                    id = id.with(quality);
                }
                // The row's own quality (normal included), never another quality's units.
                let plain = farm_sim::vocabulary::crop_qualities(view.content).first().map(|tier| tier.id.clone());
                let quality = slot.quality.clone().or(plain).unwrap_or_else(|| crop_qualities::NORMAL.to_owned());
                let sell = |quantity| {
                    GameAction::Command(Command::SellItem {
                        item_id: slot.item.id.clone(),
                        quantity,
                        quality: Some(quality.clone()),
                    })
                };
                if ui.button(id, rects[0], sell_one) {
                    actions.push(sell(1));
                }
                if held > 1 && ui.button(id.with("all"), rects[1], sell_all) {
                    actions.push(sell(held));
                }
                y += row_height + ROW_GAP;
            }
        }
        ShopTab::Repair => {
            for order in player.tool_upgrades.iter().filter(|order| order.shop_id == shop.id) {
                let ready = order.ready_day <= state.clock.day;
                let label = if ready {
                    lang.tr("shop.collectUpgrade").to_owned()
                } else {
                    lang.format("shop.upgradeReady", &[&order.ready_day])
                };
                let button = Button::new(&label)
                    .primary()
                    .size(12.5)
                    .enabled(ready && player.inventory.len() < player.max_inventory_size as usize);
                let (content, buttons) = row(ui, area, y, row_height, 140.0);
                two_lines(ui, content, &order.item.name, None, lang.tr("shop.toolUpgrade"), false);
                if ui.button(WidgetId::new("tool-collect").with(&order.item.id), buttons, button) {
                    actions.push(GameAction::Command(Command::CollectToolUpgrade { item_id: order.item.id.clone() }));
                }
                y += row_height + ROW_GAP;
            }
            for rule in shop.tool_upgrades.iter().flatten() {
                if !player.inventory.iter().any(|slot| slot.item.id == rule.from_item_id) {
                    continue;
                }
                let Some(target) = view.content.items.iter().find(|item| item.id == rule.to_item_id) else {
                    continue;
                };
                let label = lang.format("shop.upgradeFor", &[&view.money(lang, rule.cost)]);
                let button = Button::new(&label).primary().size(12.5).enabled(player.money >= rule.cost);
                let width = ui.button_width(&button).max(100.0);
                let (content, buttons) = row(ui, area, y, row_height, width);
                let materials = rule
                    .materials
                    .iter()
                    .map(|ingredient| format!("{} {}", ingredient.quantity, ingredient.item_id))
                    .collect::<Vec<_>>()
                    .join(", ");
                two_lines(
                    ui,
                    content,
                    &target.name,
                    None,
                    &lang.format("shop.upgradeMaterials", &[&rule.nights, &materials]),
                    false,
                );
                if ui.button(WidgetId::new("tool-upgrade").with(&rule.from_item_id), buttons, button) {
                    actions.push(GameAction::Command(Command::UpgradeTool { item_id: rule.from_item_id.clone() }));
                }
                y += row_height + ROW_GAP;
            }

            let damaged: Vec<_> = player
                .inventory
                .iter()
                .filter(|slot| super::category(view.content, &slot.item).is_some_and(|kind| kind.equippable))
                .filter(|slot| matches!((slot.item.durability, slot.item.max_durability), (Some(d), Some(m)) if d < m))
                .collect();
            if damaged.is_empty() {
                y += ui.empty_state(area, y, Icon::Hammer, lang.tr("shop.toolsFine"), lang.tr("shop.toolsFineDetail"));
            }
            for slot in damaged {
                let (durability, max) = (slot.item.durability.unwrap_or(0), slot.item.max_durability.unwrap_or(0));
                let cost = economy::repair_cost(i64::from(max) - i64::from(durability), shop);
                let broken = tools::is_tool_broken(&slot.item);
                let label = lang.format("shop.repairFor", &[&view.money(lang, cost)]);
                let button = Button::new(&label).primary().size(12.5).enabled(player.money >= cost);
                let width = ui.button_width(&button);
                let (content, buttons) = row(ui, area, y, row_height, width);
                let mut detail = lang.format("shop.durability", &[&num(durability), &num(max)]);
                if broken {
                    detail.push_str(" \u{2014} ");
                    detail.push_str(lang.tr("shop.broken"));
                }
                two_lines(ui, content, &slot.item.name, None, &detail, broken);
                let rect = right_aligned(buttons, &[width], small, 6.0)[0];
                if ui.button(WidgetId::new("shop-repair").with(&slot.item.id), rect, button) {
                    actions.push(GameAction::Command(Command::RepairTool { item_id: slot.item.id.clone() }));
                }
                y += row_height + ROW_GAP;
            }
        }
        ShopTab::Buy => {
            let season = &state.clock.season;
            let stock: Vec<_> = shop
                .stock
                .iter()
                .filter_map(|entry| Some((entry, view.content.items.iter().find(|item| item.id == entry.item_id)?)))
                .filter(|(entry, _)| match &entry.seasons {
                    Some(seasons) if !seasons.is_empty() => seasons.contains(season),
                    _ => true,
                })
                .collect();
            if stock.is_empty() {
                y += ui.empty_state(area, y, Icon::Store, lang.tr("shop.noStock"), lang.tr("shop.noStockDetail"));
            }
            for (entry, item) in stock {
                let price = farm_sim::commerce::price(
                    view.ctx,
                    state,
                    shop,
                    &item.id,
                    entry.price.unwrap_or(item.value),
                    false,
                );
                let remaining = view.overlay.remaining(&entry.item_id);
                let mut detail = item.description.clone();
                if let Some(limit) = entry.daily_limit {
                    if !detail.is_empty() {
                        detail.push_str(" \u{00b7} ");
                    }
                    let left = remaining.map_or_else(|| "Infinity".to_owned(), num);
                    detail.push_str(&lang.format("shop.leftToday", &[&left, &num(limit)]));
                }
                let price_text = view.money(lang, price);
                let price_width = ui.measure(&price_text, 14.0, farm_render::FontId::Bold).ceil();
                let already_owned = entry.upgrade.as_ref().is_some_and(|upgrade| {
                    player.purchased_upgrades.contains(&farm_sim::progression::purchase_key(&shop.id, &upgrade.id))
                });
                let buy = Button::new(lang.tr("shop.buy"))
                    .primary()
                    .size(12.5)
                    .enabled(!already_owned && player.money >= price && remaining.is_none_or(|left| left >= 1));
                let five = Button::new("\u{00d7}5")
                    .size(12.5)
                    .enabled(player.money >= price.saturating_mul(5) && remaining.is_none_or(|left| left >= 5));
                let mut widths = vec![price_width, ui.button_width(&buy).max(52.0)];
                if item.stackable && entry.upgrade.is_none() {
                    widths.push(ui.button_width(&five).max(40.0));
                }
                let actions_width = widths.iter().sum::<f32>() + 6.0 * (widths.len() - 1) as f32;
                let (content, buttons) = row(ui, area, y, row_height, actions_width);
                two_lines(ui, content, &item.name, None, &detail, false);
                let rects = right_aligned(buttons, &widths, small, 6.0);
                ui.label(rects[0], &price_text, 14.0, FontId::Bold, colors.text, Align::End);
                let id = WidgetId::new("shop-buy").with(&entry.item_id);
                if ui.button(id, rects[1], buy) {
                    actions.push(GameAction::Command(Command::BuyItem { item_id: entry.item_id.clone(), quantity: 1 }));
                }
                if item.stackable && entry.upgrade.is_none() && ui.button(id.with("5"), rects[2], five) {
                    actions.push(GameAction::Command(Command::BuyItem { item_id: entry.item_id.clone(), quantity: 5 }));
                }
                y += row_height + ROW_GAP;
            }
        }
    }
    ui.end_modal(y);
}
