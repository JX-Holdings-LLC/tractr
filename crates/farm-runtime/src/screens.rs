//! What a plugin screen shows (docs/PLUGINS.md "Screens", #240): a small, validated tree of the
//! game interface's own widgets, not raw draw commands, so a plugin screen looks like the rest
//! of the game and inherits its accessibility settings (text size, contrast, reduced motion,
//! focus and keyboard navigation), which the game UI (`farm-ui`) applies when it lays the tree
//! out.
//!
//! A screen's `draw(state)` answers one widget; [`parse_screen`] turns the JSON into
//! [`ScreenWidget`]s or says why it can't. The widgets:
//!
//! | Widget | Fields |
//! |---|---|
//! | `{ type: 'text', text, muted?, size? }` | A wrapped paragraph; `size` is `small`, `normal` or `large`. |
//! | `{ type: 'heading', text }` | A section heading. |
//! | `{ type: 'column', children }` / `{ type: 'row', children }` | Children stacked down, or side by side. |
//! | `{ type: 'list', items: [{ text, detail?, icon?, action? }] }` | A list of rows; a row with `action` is a button. |
//! | `{ type: 'slots', slots: [{ itemId?, quantity?, action? }], columns? }` | A grid of item slots (the inventory's look). |
//! | `{ type: 'tabs', tabs: [{ id, label }], selected }` | A tab strip; choosing a tab sends the action `tab:<id>`. |
//! | `{ type: 'button', label, action, primary?, enabled? }` | A button. |
//! | `{ type: 'item', itemId, quantity? }` | An item's icon with its name. |
//! | `{ type: 'portrait', npcId }` | A character's portrait with their name. |
//! | `{ type: 'progress', value, label? }` | A bar, `value` from 0 to 1. |
//!
//! Choosing a button, a list row, a slot or a tab calls the screen's `action(action, state)`,
//! which answers mutations like any handler. Every limit ([`ScreenLimits`]) is checked; a tree
//! that breaks one is refused as a whole.

use serde::Serialize;
use serde_json::{Map, Value};

/// Bounds on one screen's widget tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenLimits {
    /// Most widgets in the tree, list rows and slots included.
    pub max_widgets: usize,
    /// Deepest nesting of `column` / `row`.
    pub max_depth: usize,
    /// Most characters of one text.
    pub max_text_chars: usize,
}

impl Default for ScreenLimits {
    fn default() -> Self {
        Self { max_widgets: 512, max_depth: 8, max_text_chars: 2000 }
    }
}

/// The size of a `text` widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TextSize {
    Small,
    #[default]
    Normal,
    Large,
}

/// One row of a `list`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListRow {
    pub text: String,
    pub detail: Option<String>,
    /// An item whose art is the row's icon.
    pub icon: Option<String>,
    pub action: Option<String>,
}

/// One slot of a `slots` grid.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Slot {
    pub item_id: Option<String>,
    pub quantity: u32,
    pub action: Option<String>,
}

/// One tab of a `tabs` strip.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tab {
    pub id: String,
    pub label: String,
}

/// A widget of a plugin screen.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ScreenWidget {
    Text { text: String, muted: bool, size: TextSize },
    Heading { text: String },
    Column { children: Vec<ScreenWidget> },
    Row { children: Vec<ScreenWidget> },
    List { items: Vec<ListRow> },
    Slots { slots: Vec<Slot>, columns: u32 },
    Tabs { tabs: Vec<Tab>, selected: String },
    Button { label: String, action: String, primary: bool, enabled: bool },
    Item { item_id: String, quantity: u32 },
    Portrait { npc_id: String },
    Progress { value: f32, label: Option<String> },
}

impl ScreenWidget {
    /// Every action the tree can send, in reading order (keyboard focus order).
    pub fn actions(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.collect_actions(&mut out);
        out
    }

    fn collect_actions(&self, out: &mut Vec<String>) {
        match self {
            Self::Column { children } | Self::Row { children } => {
                children.iter().for_each(|child| child.collect_actions(out));
            }
            Self::List { items } => out.extend(items.iter().filter_map(|row| row.action.clone())),
            Self::Slots { slots, .. } => out.extend(slots.iter().filter_map(|slot| slot.action.clone())),
            Self::Tabs { tabs, .. } => out.extend(tabs.iter().map(|tab| format!("tab:{}", tab.id))),
            Self::Button { action, enabled: true, .. } => out.push(action.clone()),
            _ => {}
        }
    }
}

struct Parser {
    limits: ScreenLimits,
    widgets: usize,
}

fn object<'v>(value: &'v Value, at: &str) -> Result<&'v Map<String, Value>, String> {
    value.as_object().ok_or_else(|| format!("{at}: expected an object"))
}

impl Parser {
    fn count(&mut self, at: &str) -> Result<(), String> {
        self.widgets += 1;
        if self.widgets > self.limits.max_widgets {
            return Err(format!("{at}: more than {} widgets", self.limits.max_widgets));
        }
        Ok(())
    }

    fn text(&self, map: &Map<String, Value>, key: &str, at: &str) -> Result<String, String> {
        match map.get(key) {
            Some(Value::String(text)) if text.chars().count() <= self.limits.max_text_chars => Ok(text.clone()),
            Some(Value::String(_)) => Err(format!("{at}.{key}: at most {} characters", self.limits.max_text_chars)),
            Some(Value::Number(n)) => Ok(n.to_string()),
            _ => Err(format!("{at}.{key}: expected a string")),
        }
    }

    fn optional_text(&self, map: &Map<String, Value>, key: &str, at: &str) -> Result<Option<String>, String> {
        match map.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(_) => self.text(map, key, at).map(Some),
        }
    }

    fn flag(map: &Map<String, Value>, key: &str, default: bool) -> bool {
        map.get(key).and_then(Value::as_bool).unwrap_or(default)
    }

    fn count_of(map: &Map<String, Value>, key: &str) -> u32 {
        map.get(key).and_then(Value::as_f64).filter(|n| n.is_finite() && *n >= 0.0).map_or(0, |n| n.min(1e6) as u32)
    }

    fn list<'v>(map: &'v Map<String, Value>, key: &str, at: &str) -> Result<&'v Vec<Value>, String> {
        map.get(key).and_then(Value::as_array).ok_or_else(|| format!("{at}.{key}: expected a list"))
    }

    fn widget(&mut self, value: &Value, at: &str, depth: usize) -> Result<ScreenWidget, String> {
        self.count(at)?;
        let map = object(value, at)?;
        let kind = map.get("type").and_then(Value::as_str).ok_or_else(|| format!("{at}.type: expected a string"))?;
        Ok(match kind {
            "text" => ScreenWidget::Text {
                text: self.text(map, "text", at)?,
                muted: Self::flag(map, "muted", false),
                size: match map.get("size").and_then(Value::as_str) {
                    None | Some("normal") => TextSize::Normal,
                    Some("small") => TextSize::Small,
                    Some("large") => TextSize::Large,
                    Some(other) => return Err(format!("{at}.size: unknown size \"{other}\"")),
                },
            },
            "heading" => ScreenWidget::Heading { text: self.text(map, "text", at)? },
            "column" | "row" => {
                if depth >= self.limits.max_depth {
                    return Err(format!("{at}: nested deeper than {}", self.limits.max_depth));
                }
                let mut children = Vec::new();
                for (index, child) in Self::list(map, "children", at)?.iter().enumerate() {
                    children.push(self.widget(child, &format!("{at}.children[{index}]"), depth + 1)?);
                }
                if kind == "column" {
                    ScreenWidget::Column { children }
                } else {
                    ScreenWidget::Row { children }
                }
            }
            "list" => {
                let mut items = Vec::new();
                for (index, row) in Self::list(map, "items", at)?.iter().enumerate() {
                    let at = format!("{at}.items[{index}]");
                    self.count(&at)?;
                    let row = object(row, &at)?;
                    items.push(ListRow {
                        text: self.text(row, "text", &at)?,
                        detail: self.optional_text(row, "detail", &at)?,
                        icon: self.optional_text(row, "icon", &at)?,
                        action: self.optional_text(row, "action", &at)?,
                    });
                }
                ScreenWidget::List { items }
            }
            "slots" => {
                let mut slots = Vec::new();
                for (index, slot) in Self::list(map, "slots", at)?.iter().enumerate() {
                    let at = format!("{at}.slots[{index}]");
                    self.count(&at)?;
                    let slot = object(slot, &at)?;
                    slots.push(Slot {
                        item_id: self.optional_text(slot, "itemId", &at)?,
                        quantity: Self::count_of(slot, "quantity"),
                        action: self.optional_text(slot, "action", &at)?,
                    });
                }
                let columns = Self::count_of(map, "columns");
                ScreenWidget::Slots { slots, columns: if columns == 0 { 6 } else { columns.min(12) } }
            }
            "tabs" => {
                let mut tabs = Vec::new();
                for (index, tab) in Self::list(map, "tabs", at)?.iter().enumerate() {
                    let at = format!("{at}.tabs[{index}]");
                    self.count(&at)?;
                    let tab = object(tab, &at)?;
                    tabs.push(Tab { id: self.text(tab, "id", &at)?, label: self.text(tab, "label", &at)? });
                }
                let selected = self.optional_text(map, "selected", at)?.unwrap_or_default();
                ScreenWidget::Tabs { tabs, selected }
            }
            "button" => ScreenWidget::Button {
                label: self.text(map, "label", at)?,
                action: self.text(map, "action", at)?,
                primary: Self::flag(map, "primary", false),
                enabled: Self::flag(map, "enabled", true),
            },
            "item" => {
                ScreenWidget::Item { item_id: self.text(map, "itemId", at)?, quantity: Self::count_of(map, "quantity") }
            }
            "portrait" => ScreenWidget::Portrait { npc_id: self.text(map, "npcId", at)? },
            "progress" => ScreenWidget::Progress {
                value: map
                    .get("value")
                    .and_then(Value::as_f64)
                    .filter(|n| n.is_finite())
                    .map_or(0.0, |n| n.clamp(0.0, 1.0) as f32),
                label: self.optional_text(map, "label", at)?,
            },
            other => return Err(format!("{at}.type: unknown widget \"{other}\"")),
        })
    }
}

/// Parse a screen's `draw` answer, checking every limit.
pub fn parse_screen(value: &Value, limits: ScreenLimits) -> Result<ScreenWidget, String> {
    Parser { limits, widgets: 0 }.widget(value, "screen", 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_widgets_and_lists_their_actions() {
        let tree = parse_screen(
            &json!({ "type": "column", "children": [
                { "type": "heading", "text": "Fair" },
                { "type": "tabs", "tabs": [{ "id": "a", "label": "A" }], "selected": "a" },
                { "type": "list", "items": [{ "text": "Ribbon", "action": "buy" }, { "text": "Sold out" }] },
                { "type": "slots", "slots": [{ "itemId": "seed", "quantity": 3, "action": "pick:0" }] },
                { "type": "button", "label": "Off", "action": "never", "enabled": false },
                { "type": "progress", "value": 2 }
            ] }),
            ScreenLimits::default(),
        )
        .unwrap();
        assert_eq!(tree.actions(), ["tab:a", "buy", "pick:0"]);
    }

    #[test]
    fn refuses_what_breaks_a_limit() {
        let limits = ScreenLimits { max_widgets: 3, max_depth: 1, max_text_chars: 5 };
        let deep = json!({ "type": "column", "children": [{ "type": "row", "children": [] }] });
        assert!(parse_screen(&deep, limits).unwrap_err().contains("nested"));
        let long = json!({ "type": "text", "text": "toolong" });
        assert!(parse_screen(&long, limits).unwrap_err().contains("at most 5"));
        let many = json!({ "type": "list", "items": [{ "text": "a" }, { "text": "b" }, { "text": "c" }] });
        assert!(parse_screen(&many, limits).unwrap_err().contains("more than 3"));
        assert!(parse_screen(&json!({ "type": "canvas" }), limits).unwrap_err().contains("unknown widget"));
    }
}
