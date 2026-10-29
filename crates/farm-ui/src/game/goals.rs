//! The goals panel (M6): collections (items found, the missing ones, rewards; Donate for a
//! collection that takes donations), bundles by group (each line handed in so far, Hand in),
//! achievements (unlocked ones with their day, hidden ones as ??? until then) and the shipping
//! log (totals and last night's shipment). Only the tabs the game uses show. Whether an item
//! may be donated or handed in now comes from `farm_sim::overlay`; the commands check again.

use super::{item_name, GameAction, GameView};
use crate::format::num;
use crate::i18n::Lang;
use crate::icons::Icon;
use crate::layout::{Align, RectExt};
use crate::theme::fade;
use crate::ui::{Ui, WidgetId};
use crate::widgets::{Button, ButtonKind, ModalSpec};
use farm_render::{FontId, Rect, TextAlign};
use farm_sim::overlay;
use farm_sim::schema::{collection_kinds, BundleDefinition, CollectionDefinition, GameContent};
use farm_sim::Command;

const GAP: f32 = 8.0;

/// The goals panel's tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GoalsTab {
    #[default]
    Collections,
    Bundles,
    Achievements,
    Shipped,
    Fish,
}

impl GoalsTab {
    /// The tab's stable name (widget ids) and its label key.
    fn names(self) -> (&'static str, &'static str) {
        match self {
            GoalsTab::Collections => ("Collections", "goals.collections"),
            GoalsTab::Bundles => ("Bundles", "goals.bundles"),
            GoalsTab::Achievements => ("Achievements", "goals.achievements"),
            GoalsTab::Shipped => ("Shipped", "goals.shipped"),
            GoalsTab::Fish => ("Fish", "goals.fish"),
        }
    }
}

/// The tabs the game uses: collections, bundles and achievements when it has any; the shipping
/// log when it has a shipping bin.
pub fn tabs(content: &GameContent) -> Vec<GoalsTab> {
    let mut tabs = Vec::new();
    if !content.collections.is_empty() {
        tabs.push(GoalsTab::Collections);
    }
    if !content.bundles.is_empty() {
        tabs.push(GoalsTab::Bundles);
    }
    if !content.achievements.is_empty() {
        tabs.push(GoalsTab::Achievements);
    }
    if content.machine_types.iter().any(|kind| kind.shipping) {
        tabs.push(GoalsTab::Shipped);
    }
    if content
        .fish_tables
        .iter()
        .flat_map(|table| &table.entries)
        .any(|entry| entry.rules.as_ref().is_some_and(|r| r.min_size.is_some()))
    {
        tabs.push(GoalsTab::Fish);
    }
    tabs
}

/// A small row: a mark, a text, an optional trailing note and button at the right; whether the
/// button was pressed.
fn line_row(
    ui: &mut Ui,
    rect: Rect,
    done: bool,
    text: &str,
    trailing: Option<&str>,
    button: Option<(WidgetId, Button<'_>)>,
) -> bool {
    let colors = ui.theme().colors;
    ui.row_background(rect);
    let mut inner = rect.inset_xy(10.0, 0.0);
    let mut pressed = false;
    if let Some((id, button)) = button {
        let width = ui.button_width(&button);
        let height = ui.button_height(12.0).min(rect.height - 6.0);
        let button_rect = inner.cut_right(width).centered(width, height);
        pressed = ui.button(id, button_rect, button);
        inner.cut_right(8.0);
    }
    if let Some(trailing) = trailing {
        let width = ui.measure(trailing, 12.5, FontId::Regular).ceil();
        let right = inner.cut_right(width);
        ui.label(right, trailing, 12.5, FontId::Regular, colors.muted, Align::End);
        inner.cut_right(8.0);
    }
    let (mark, mark_color) = if done { ("\u{2713}", colors.primary) } else { ("\u{25CB}", colors.muted) };
    let mark_rect = inner.cut_left(18.0);
    ui.label(mark_rect, mark, 13.0, FontId::Bold, mark_color, Align::Start);
    ui.label(inner, text, 13.0, FontId::Regular, if done { colors.text } else { colors.muted }, Align::Start);
    pressed
}

/// A goal's heading: name with a count, a description and a progress bar; returns its height.
fn heading(ui: &mut Ui, area: Rect, top: f32, name: &str, count: &str, description: &str, ratio: Option<f32>) -> f32 {
    let colors = ui.theme().colors;
    let mut y = top;
    let title = ui.line_height(15.0);
    let title_rect = Rect::new(area.x, y, area.width, title);
    let width = ui.label(title_rect, name, 15.0, FontId::Bold, colors.text, Align::Start);
    let count_rect = Rect::new(area.x + width + 8.0, y, area.width - width - 8.0, title);
    ui.label(count_rect, count, 12.5, FontId::Regular, colors.muted, Align::Start);
    y += title;
    if !description.is_empty() {
        y += ui.paragraph(area.x, y, area.width, description, 12.5, FontId::Regular, colors.muted, TextAlign::Left);
    }
    if let Some(ratio) = ratio {
        ui.progress(Rect::new(area.x, y + 4.0, area.width, 6.0), ratio, colors.primary);
        y += 12.0;
    }
    y + 6.0 - top
}

fn collection(
    ui: &mut Ui,
    view: &GameView<'_>,
    area: Rect,
    top: f32,
    definition: &CollectionDefinition,
    actions: &mut Vec<GameAction>,
) -> f32 {
    let lang = ui.lang();
    let colors = ui.theme().colors;
    let progress = view.state.collections.get(&definition.id);
    let found = |item: &str| progress.is_some_and(|progress| progress.items.iter().any(|id| id == item));
    let have = definition.items.iter().filter(|item| found(item)).count();
    let total = definition.items.len().max(1);
    let fill = |text: &str| farm_sim::substitution::fill(view.content, view.state, text).into_owned();
    let count = lang.format("goals.found", &[&num(have), &num(definition.items.len())]);
    let mut y = top;
    y += heading(
        ui,
        area,
        y,
        &fill(&definition.name),
        &count,
        &fill(&definition.description),
        Some(have as f32 / total as f32),
    );
    let donated = definition.kind == collection_kinds::DONATED;
    let columns = if area.width >= 520.0 { 2 } else { 1 };
    let width = (area.width - GAP * (columns - 1) as f32) / columns as f32;
    let height = ui.button_height(12.0) + 8.0;
    for (index, item) in definition.items.iter().enumerate() {
        let column = index % columns;
        let rect = Rect::new(area.x + column as f32 * (width + GAP), y, width, height);
        let done = found(item);
        let button = (donated && !done).then(|| {
            let id = WidgetId::new("goals-donate").with(definition.id.as_str()).with(item.as_str());
            let enabled = overlay::can_donate(view.ctx, view.state, &definition.id, item);
            (id, Button::new(lang.tr("goals.donate")).primary().size(12.0).enabled(enabled))
        });
        let trailing = (!done).then(|| lang.tr("goals.missing"));
        if line_row(ui, rect, done, item_name(view.content, item), trailing, button) {
            actions.push(GameAction::Command(Command::DonateItem {
                collection_id: definition.id.clone(),
                item_id: item.clone(),
            }));
        }
        if column == columns - 1 || index == definition.items.len() - 1 {
            y += height + 4.0;
        }
    }
    let rewarded = progress.map_or(0, |progress| progress.rewarded as usize);
    for (index, reward) in definition.rewards.iter().enumerate() {
        let given = index < rewarded;
        let text = if reward.message.is_empty() {
            lang.format("goals.rewardAt", &[&num(reward.count)])
        } else {
            lang.format("goals.rewardAtWith", &[&num(reward.count), &fill(&reward.message)])
        };
        let mark = if given { "\u{2713} " } else { "\u{2606} " };
        let color = if given { colors.primary } else { colors.muted };
        y += ui.paragraph(
            area.x,
            y + 2.0,
            area.width,
            &format!("{mark}{text}"),
            12.5,
            FontId::Regular,
            color,
            TextAlign::Left,
        ) + 2.0;
    }
    y + 14.0 - top
}

fn bundle(
    ui: &mut Ui,
    view: &GameView<'_>,
    area: Rect,
    top: f32,
    definition: &BundleDefinition,
    actions: &mut Vec<GameAction>,
) -> f32 {
    let lang = ui.lang();
    let progress = view.state.bundles.get(&definition.id);
    let complete = progress.is_some_and(|progress| progress.complete);
    let handed = |line: usize| progress.and_then(|progress| progress.handed.get(line)).copied().unwrap_or(0);
    let full = definition.items.iter().enumerate().filter(|(line, item)| handed(*line) >= item.quantity).count();
    let needed = definition.lines_needed();
    let fill = |text: &str| farm_sim::substitution::fill(view.content, view.state, text).into_owned();
    let count = if complete {
        lang.tr("goals.complete").to_owned()
    } else if needed < definition.items.len() {
        lang.format("goals.anyOf", &[&num(full), &num(needed), &num(definition.items.len())])
    } else {
        lang.format("goals.lines", &[&num(full), &num(needed)])
    };
    let ratio = if complete { 1.0 } else { full as f32 / needed.max(1) as f32 };
    let mut y = top;
    y += heading(ui, area, y, &fill(&definition.name), &count, &fill(&definition.description), Some(ratio));
    let height = ui.button_height(12.0) + 8.0;
    for (line, wanted) in definition.items.iter().enumerate() {
        let got = handed(line).min(wanted.quantity);
        let done = got >= wanted.quantity;
        let name = super::slot_name(view.content, item_name(view.content, &wanted.item_id), wanted.quality.as_deref());
        let amount = format!("{}/{}", num(got), num(wanted.quantity));
        let button = (!done && !complete).then(|| {
            let id = WidgetId::new("goals-hand-in").with(definition.id.as_str()).with(line);
            let enabled = overlay::can_hand_in(view.ctx, view.state, &definition.id, line);
            (id, Button::new(lang.tr("goals.handIn")).primary().size(12.0).enabled(enabled))
        });
        let rect = Rect::new(area.x, y, area.width, height);
        if line_row(ui, rect, done, &name, Some(&amount), button) {
            actions.push(GameAction::Command(Command::HandInBundle {
                bundle_id: definition.id.clone(),
                line: line as i32,
            }));
        }
        y += height + 4.0;
    }
    y + 14.0 - top
}

fn achievements(ui: &mut Ui, view: &GameView<'_>, area: Rect, top: f32) -> f32 {
    let lang = ui.lang();
    let colors = ui.theme().colors;
    let fill = |text: &str| farm_sim::substitution::fill(view.content, view.state, text).into_owned();
    let mut y = top;
    let title = ui.line_height(14.0);
    let detail = ui.line_height(12.5);
    let height = title + detail + 16.0;
    for achievement in &view.content.achievements {
        let unlocked = view.state.achievements.get(&achievement.id).copied();
        let rect = Rect::new(area.x, y, area.width, height);
        ui.row_background(rect);
        let mut inner = rect.inset_xy(12.0, 8.0);
        let badge = inner.cut_left(34.0).centered(30.0, 30.0);
        let (fill_color, icon_color) = match unlocked {
            Some(_) => (fade(colors.accent, 0.25), colors.accent_text),
            None => (fade(colors.muted, 0.15), colors.muted),
        };
        ui.list_mut().fill_round_rect(badge, 8.0, fill_color);
        crate::icons::draw(ui.list_mut(), Icon::Star, badge.inset(6.0), icon_color, colors.row);
        inner.cut_left(10.0);
        if let Some(day) = unlocked {
            let text = lang.format("goals.unlockedOn", &[&num(day)]);
            let width = ui.measure(&text, 12.5, FontId::Regular).ceil();
            let right = inner.cut_right(width);
            ui.label(right, &text, 12.5, FontId::Regular, colors.primary, Align::End);
            inner.cut_right(8.0);
        }
        let secret = achievement.hidden && unlocked.is_none();
        let (name, description) = if secret {
            ("???".to_owned(), lang.tr("goals.hiddenAchievement").to_owned())
        } else {
            (fill(&achievement.name), fill(&achievement.description))
        };
        let text_color = if unlocked.is_some() { colors.text } else { colors.muted };
        ui.label(Rect::new(inner.x, inner.y, inner.width, title), &name, 14.0, FontId::Bold, text_color, Align::Start);
        ui.label(
            Rect::new(inner.x, inner.y + title, inner.width, detail),
            &description,
            12.5,
            FontId::Regular,
            colors.muted,
            Align::Start,
        );
        y += height + GAP;
    }
    y - top
}

fn shipped(ui: &mut Ui, view: &GameView<'_>, area: Rect, top: f32, lang: Lang) -> f32 {
    let state = view.state;
    let height = ui.line_height(13.0) + 14.0;
    let mut y = top;
    y += ui.section(area, y, lang.tr("goals.lastShipment"), None);
    if state.last_shipment.is_empty() {
        y += ui.empty_state(area, y, Icon::Coin, lang.tr("goals.noShipment"), lang.tr("goals.noShipmentDetail"));
    } else {
        let mut total = 0_i64;
        for line in &state.last_shipment {
            total = total.saturating_add(line.money);
            let name = super::slot_name(view.content, item_name(view.content, &line.item_id), line.quality.as_deref());
            let text = format!("{name} \u{00d7}{}", num(line.quantity));
            let money = view.money(lang, line.money);
            line_row(ui, Rect::new(area.x, y, area.width, height), true, &text, Some(&money), None);
            y += height + 4.0;
        }
        let text = lang.format("goals.shipmentTotal", &[&view.money(lang, total)]);
        let colors = ui.theme().colors;
        ui.label(
            Rect::new(area.x, y, area.width, ui.line_height(13.5)),
            &text,
            13.5,
            FontId::Bold,
            colors.text,
            Align::End,
        );
        y += ui.line_height(13.5) + 8.0;
    }
    y += ui.section(area, y, lang.tr("goals.shippedSoFar"), None);
    if state.shipped.is_empty() {
        y += ui.empty_state(
            area,
            y,
            Icon::Package,
            lang.tr("goals.nothingShipped"),
            lang.tr("goals.nothingShippedDetail"),
        );
    }
    let columns = if area.width >= 520.0 { 2 } else { 1 };
    let width = (area.width - GAP * (columns - 1) as f32) / columns as f32;
    let count = state.shipped.len();
    for (index, (item, units)) in state.shipped.iter().enumerate() {
        let column = index % columns;
        let rect = Rect::new(area.x + column as f32 * (width + GAP), y, width, height);
        let amount = format!("\u{00d7}{}", num(*units));
        line_row(ui, rect, true, item_name(view.content, item), Some(&amount), None);
        if column == columns - 1 || index == count - 1 {
            y += height + 4.0;
        }
    }
    y - top
}

pub(crate) fn draw(ui: &mut Ui, view: &GameView<'_>, tab: &mut GoalsTab, actions: &mut Vec<GameAction>) {
    let lang = ui.lang();
    let tabs = tabs(view.content);
    if tabs.is_empty() {
        actions.push(GameAction::ClosePanel);
        return;
    }
    if !tabs.contains(tab) {
        *tab = tabs[0];
    }
    if view.keys_active && ui.tab_delta() != 0 {
        let current = tabs.iter().position(|t| t == tab).unwrap_or(0) as i32;
        let next = (current + ui.tab_delta()).rem_euclid(tabs.len() as i32) as usize;
        *tab = tabs[next];
    }
    let content = view.content;
    let state = view.state;
    let subtitle = match *tab {
        GoalsTab::Fish => lang.format("goals.fishSubtitle", &[&num(state.fish_catches.len())]),
        GoalsTab::Collections => {
            let done = content
                .collections
                .iter()
                .filter(|c| state.collections.get(&c.id).is_some_and(|p| p.items.len() >= c.items.len()))
                .count();
            lang.format("goals.collectionsSubtitle", &[&num(done), &num(content.collections.len())])
        }
        GoalsTab::Bundles => {
            let done = content.bundles.iter().filter(|b| farm_sim::goals::bundle_complete(state, &b.id)).count();
            lang.format("goals.bundlesSubtitle", &[&num(done), &num(content.bundles.len())])
        }
        GoalsTab::Achievements => {
            let done = content.achievements.iter().filter(|a| state.achievements.contains_key(&a.id)).count();
            lang.format("goals.achievementsSubtitle", &[&num(done), &num(content.achievements.len())])
        }
        GoalsTab::Shipped => {
            let units: u64 = state.shipped.values().map(|units| u64::from(*units)).sum();
            lang.format("goals.shippedSubtitle", &[&num(units)])
        }
    };
    let modal = ui.begin_modal(ModalSpec {
        id: WidgetId::new("goals"),
        icon: Icon::Book,
        title: lang.tr("goals.title"),
        subtitle: Some(&subtitle),
        width: 660.0,
        max_height: 660.0,
        footer: 0.0,
        backdrop: true,
    });
    if modal.close {
        actions.push(GameAction::ClosePanel);
    }
    let area = modal.body;
    let mut y = modal.top;

    if tabs.len() > 1 {
        let tab_height = ui.button_height(13.0);
        let mut x = area.x;
        for candidate in &tabs {
            let (name, label) = candidate.names();
            let button = Button::new(lang.tr(label)).kind(ButtonKind::Tab { selected: candidate == tab });
            let width = ui.button_width(&button).max(72.0).min(area.width);
            if x > area.x && x + width > area.x + area.width {
                x = area.x;
                y += tab_height + 6.0;
            }
            if ui.button(WidgetId::new("goals-tab").with(name), Rect::new(x, y, width, tab_height), button) {
                *tab = *candidate;
            }
            x += width + 6.0;
        }
        y += tab_height + 12.0;
    }

    match *tab {
        GoalsTab::Fish => {
            for record in &state.fish_catches {
                let text = item_name(content, &record.item_id);
                let note = lang.format("goals.fishSize", &[&num(record.size)]);
                line_row(ui, Rect::new(area.x, y, area.width, 36.0), true, text, Some(&note), None);
                y += 40.0;
            }
        }
        GoalsTab::Collections => {
            for definition in &content.collections {
                y += collection(ui, view, area, y, definition, actions);
            }
        }
        GoalsTab::Bundles => {
            // By group (a room, a board), in the order the groups first appear.
            let mut groups: Vec<&str> = Vec::new();
            for definition in &content.bundles {
                if !groups.contains(&definition.group.as_str()) {
                    groups.push(&definition.group);
                }
            }
            for group in groups {
                if !group.is_empty() {
                    y += ui.section(area, y, group, None) + 2.0;
                }
                for definition in content.bundles.iter().filter(|b| b.group == group) {
                    y += bundle(ui, view, area, y, definition, actions);
                }
            }
        }
        GoalsTab::Achievements => y += achievements(ui, view, area, y),
        GoalsTab::Shipped => y += shipped(ui, view, area, y, lang),
    }
    ui.end_modal(y);
}
