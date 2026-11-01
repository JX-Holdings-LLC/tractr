//! A plugin screen (#240, docs/PLUGINS.md "Screens"): the widget tree a plugin's `draw`
//! answered ([`farm_runtime::screens`]), laid out with the game's own widgets inside a modal.
//! Text sizes, contrast and focus come from the game's theme and accessibility settings like
//! every other screen; choosing a button, list row, slot or tab sends
//! [`GameAction::PluginScreen`], which the host passes to the screen's `action`.

use super::{draw_item_art, item_name, GameAction, GameView};
use crate::icons::Icon;
use crate::layout::{Align, RectExt};
use crate::ui::{HitKind, Ui, WidgetId};
use crate::widgets::{Button, ModalSpec};
use farm_render::{FontId, ImageStore, Rect, TextAlign};
use farm_runtime::screens::{ScreenWidget, TextSize};

/// What the host hands the game UI for the open plugin screen.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginScreenView {
    /// The screen's hook (`screen:packId:name`): actions name it.
    pub hook: String,
    pub title: String,
    /// The last tree its `draw` answered, or why there is none.
    pub content: Result<ScreenWidget, String>,
}

struct Layout<'u, 'v, 'i> {
    ui: &'u mut Ui,
    view: &'v GameView<'v>,
    images: &'i mut ImageStore,
    hook: String,
    actions: Vec<GameAction>,
    next_id: usize,
}

impl Layout<'_, '_, '_> {
    fn id(&mut self) -> WidgetId {
        self.next_id += 1;
        WidgetId::new("plugin-screen").with(self.next_id)
    }

    fn act(&mut self, action: &str) {
        self.actions.push(GameAction::PluginScreen { hook: self.hook.clone(), action: action.to_owned() });
    }

    /// Lays `widget` out at the top of `area`; returns the height it took.
    fn widget(&mut self, widget: &ScreenWidget, area: Rect) -> f32 {
        let colors = self.ui.theme().colors;
        match widget {
            ScreenWidget::Text { text, muted, size } => {
                let size = match size {
                    TextSize::Small => 12.5,
                    TextSize::Normal => 14.0,
                    TextSize::Large => 17.0,
                };
                let color = if *muted { colors.muted } else { colors.text };
                let height = self.ui.paragraph_height(text, size, FontId::Regular, area.width);
                self.ui.paragraph(area.x, area.y, area.width, text, size, FontId::Regular, color, TextAlign::Left);
                height + 6.0
            }
            ScreenWidget::Heading { text } => self.ui.section(area, area.y, text, None),
            ScreenWidget::Column { children } => {
                let mut y = area.y;
                for child in children {
                    y += self.widget(child, Rect::new(area.x, y, area.width, area.height));
                }
                y - area.y
            }
            ScreenWidget::Row { children } => {
                if children.is_empty() {
                    return 0.0;
                }
                let gap = 10.0;
                let width = (area.width - gap * (children.len() as f32 - 1.0)) / children.len() as f32;
                let mut tallest: f32 = 0.0;
                for (index, child) in children.iter().enumerate() {
                    let x = area.x + index as f32 * (width + gap);
                    tallest = tallest.max(self.widget(child, Rect::new(x, area.y, width, area.height)));
                }
                tallest
            }
            ScreenWidget::List { items } => {
                let title = self.ui.line_height(14.0);
                let detail = self.ui.line_height(12.5);
                let mut y = area.y;
                for row in items {
                    let height = title + if row.detail.is_some() { detail } else { 0.0 } + 16.0;
                    let rect = Rect::new(area.x, y, area.width, height);
                    let id = self.id();
                    let clickable = row.action.is_some();
                    let state = self.ui.interact(id, rect, HitKind::Button, clickable);
                    let fill = if clickable && state.hovered { colors.button_hover } else { colors.row };
                    self.ui.list_mut().fill_round_rect(rect, 8.0, fill);
                    self.ui.list_mut().stroke_round_rect(rect.inset(0.5), 8.0, colors.row_border, 1.0);
                    if state.focus_visible {
                        self.ui.focus_ring(rect, 8.0);
                    }
                    let mut inner = rect.inset_xy(12.0, 8.0);
                    if let Some(icon) = &row.icon {
                        let tile = inner.cut_left(28.0).centered(28.0, 28.0);
                        if let Some(item) = self.view.content.items.iter().find(|item| item.id == *icon) {
                            draw_item_art(self.ui, self.images, &self.view.art, self.view.content, item, tile);
                        }
                        inner.cut_left(8.0);
                    }
                    self.ui.label(
                        Rect::new(inner.x, inner.y, inner.width, title),
                        &row.text,
                        14.0,
                        FontId::Regular,
                        colors.text,
                        Align::Start,
                    );
                    if let Some(text) = &row.detail {
                        self.ui.label(
                            Rect::new(inner.x, inner.y + title, inner.width, detail),
                            text,
                            12.5,
                            FontId::Regular,
                            colors.muted,
                            Align::Start,
                        );
                    }
                    if state.clicked {
                        if let Some(action) = &row.action {
                            self.act(action);
                        }
                    }
                    y += height + 4.0;
                }
                y - area.y
            }
            ScreenWidget::Slots { slots, columns } => {
                let size = 48.0;
                let gap = 6.0;
                let columns = (*columns as usize).max(1);
                for (index, slot) in slots.iter().enumerate() {
                    let (col, row) = (index % columns, index / columns);
                    let rect =
                        Rect::new(area.x + col as f32 * (size + gap), area.y + row as f32 * (size + gap), size, size);
                    let id = self.id();
                    let state = self.ui.interact(id, rect, HitKind::Button, slot.action.is_some());
                    self.ui.list_mut().fill_round_rect(
                        rect,
                        6.0,
                        if state.hovered { colors.button_hover } else { colors.row },
                    );
                    self.ui.list_mut().stroke_round_rect(rect.inset(0.5), 6.0, colors.row_border, 1.0);
                    if state.focus_visible {
                        self.ui.focus_ring(rect, 6.0);
                    }
                    if let Some(item_id) = &slot.item_id {
                        if let Some(item) = self.view.content.items.iter().find(|item| item.id == *item_id) {
                            draw_item_art(
                                self.ui,
                                self.images,
                                &self.view.art,
                                self.view.content,
                                item,
                                rect.inset(6.0),
                            );
                        }
                        if slot.quantity > 1 {
                            let text = crate::format::num(slot.quantity);
                            self.ui.label(rect.inset(3.0), &text, 11.0, FontId::Bold, colors.text, Align::End);
                        }
                    }
                    if state.clicked {
                        if let Some(action) = &slot.action {
                            self.act(action);
                        }
                    }
                }
                let rows = slots.len().div_ceil(columns);
                rows as f32 * (size + gap)
            }
            ScreenWidget::Tabs { tabs, selected } => {
                let height = self.ui.button_height(13.0);
                let mut x = area.x;
                for tab in tabs {
                    let mut button = Button::new(&tab.label);
                    if tab.id == *selected {
                        button = button.primary();
                    }
                    let width = self.ui.button_width(&button);
                    let id = self.id();
                    if self.ui.button(id, Rect::new(x, area.y, width, height), button) {
                        let action = format!("tab:{}", tab.id);
                        self.act(&action);
                    }
                    x += width + 6.0;
                }
                height + 10.0
            }
            ScreenWidget::Button { label, action, primary, enabled } => {
                let mut button = Button::new(label).enabled(*enabled);
                if *primary {
                    button = button.primary();
                }
                let width = self.ui.button_width(&button).min(area.width);
                let height = self.ui.button_height(13.0);
                let id = self.id();
                if self.ui.button(id, Rect::new(area.x, area.y, width, height), button) {
                    self.act(action);
                }
                height + 8.0
            }
            ScreenWidget::Item { item_id, quantity } => {
                let height = 40.0;
                let mut inner = Rect::new(area.x, area.y, area.width, height);
                let tile = inner.cut_left(36.0).centered(32.0, 32.0);
                if let Some(item) = self.view.content.items.iter().find(|item| item.id == *item_id) {
                    draw_item_art(self.ui, self.images, &self.view.art, self.view.content, item, tile);
                }
                inner.cut_left(8.0);
                let name = item_name(self.view.content, item_id).to_owned();
                let text =
                    if *quantity > 1 { format!("{name} \u{00d7}{}", crate::format::num(*quantity)) } else { name };
                self.ui.label(inner, &text, 14.0, FontId::Regular, colors.text, Align::Start);
                height + 4.0
            }
            ScreenWidget::Portrait { npc_id } => {
                let name = self
                    .view
                    .content
                    .npcs
                    .iter()
                    .find(|npc| npc.id == *npc_id)
                    .map_or_else(|| npc_id.clone(), |npc| npc.name.clone());
                let height = 56.0;
                let mut inner = Rect::new(area.x, area.y, area.width, height);
                let frame = inner.cut_left(52.0).centered(48.0, 48.0);
                self.ui.list_mut().fill_round_rect(frame, 24.0, colors.row);
                self.ui.list_mut().stroke_round_rect(frame.inset(0.5), 24.0, colors.row_border, 1.0);
                let initial: String = name.chars().take(1).collect();
                self.ui.label(frame, &initial, 20.0, FontId::Bold, colors.text, Align::Center);
                inner.cut_left(10.0);
                self.ui.label(inner, &name, 15.0, FontId::Bold, colors.text, Align::Start);
                height + 4.0
            }
            ScreenWidget::Progress { value, label } => {
                let mut y = area.y;
                if let Some(label) = label {
                    let height = self.ui.line_height(12.5);
                    self.ui.label(
                        Rect::new(area.x, y, area.width, height),
                        label,
                        12.5,
                        FontId::Regular,
                        colors.muted,
                        Align::Start,
                    );
                    y += height + 2.0;
                }
                self.ui.progress(Rect::new(area.x, y, area.width, 10.0), *value, colors.accent);
                y + 16.0 - area.y
            }
        }
    }
}

/// Draws the open plugin screen.
pub(crate) fn draw(
    ui: &mut Ui,
    view: &GameView<'_>,
    images: &mut ImageStore,
    screen: &PluginScreenView,
    actions: &mut Vec<GameAction>,
) {
    let modal = ui.begin_modal(ModalSpec {
        id: WidgetId::new("plugin-screen"),
        icon: Icon::Menu,
        title: &screen.title,
        subtitle: None,
        width: 600.0,
        max_height: 640.0,
        footer: 0.0,
        backdrop: true,
    });
    if modal.close {
        actions.push(GameAction::ClosePanel);
    }
    let area = modal.body;
    let mut y = modal.top;
    match &screen.content {
        Ok(tree) => {
            let mut layout = Layout { ui, view, images, hook: screen.hook.clone(), actions: Vec::new(), next_id: 0 };
            y += layout.widget(tree, Rect::new(area.x, y, area.width, area.height));
            actions.append(&mut layout.actions);
        }
        Err(error) => {
            let colors = ui.theme().colors;
            let height = ui.paragraph_height(error, 13.0, FontId::Regular, area.width);
            ui.paragraph(area.x, y, area.width, error, 13.0, FontId::Regular, colors.error, TextAlign::Left);
            y += height;
        }
    }
    ui.end_modal(y);
}
