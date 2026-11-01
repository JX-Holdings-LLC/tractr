//! Port of `Interface.cs` (packages/engine-schemas/src/interface.ts).

use super::graphics::VisualRef;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// TS `GamePanelSchema.entries[].kind` enum.
pub mod game_panel_entry_kinds {
    pub const TEXT: &str = "text";
    pub const MONEY: &str = "money";
    pub const ENERGY: &str = "energy";
    pub const DAY: &str = "day";
    pub const ITEM: &str = "item";
    pub const FLAG: &str = "flag";
    pub const ACTION: &str = "action";

    pub const ALL: &[&str] = &[TEXT, MONEY, ENERGY, DAY, ITEM, FLAG, ACTION];
}

/// TS `GamePanelSchema.entries` item (inline object).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GamePanelEntry {
    pub label: String,
    /// One of [`game_panel_entry_kinds`].
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GamePanel {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible_flag: Option<String>,
    /// At most 40 entries.
    pub entries: Vec<GamePanelEntry>,
}

/// The look of a game's interface (F# `UiSkin`): fonts, 9-slice panel art, icons, colours,
/// layout and title screen. Absent parts keep the built-in look. Read by `farm-ui` only; the
/// simulation never looks at it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UiSkin {
    /// A `font` asset for headings, titles and buttons.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading_font: Option<String>,
    /// A `font` asset for body text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_font: Option<String>,
    /// A `font` asset for numbers (money, the clock, quantities).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_font: Option<String>,
    /// Colours by palette role ([`ui_palette_roles`]): `#rrggbb` or `#rrggbbaa`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colors: Option<IndexMap<String, String>>,
    /// 9-slice art by part ([`ui_skin_parts`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub panels: Option<IndexMap<String, UiNineSlice>>,
    /// Icons by name ([`ui_skin_icons`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<IndexMap<String, VisualRef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<UiLayout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<UiTitleScreen>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Panel art drawn in nine parts: the corners keep their size, the edges stretch along one side
/// and the middle both ways.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UiNineSlice {
    pub visual: VisualRef,
    /// Source pixels of each edge that do not stretch.
    #[serde(with = "crate::units::screen")]
    pub left: f64,
    #[serde(with = "crate::units::screen")]
    pub top: f64,
    #[serde(with = "crate::units::screen")]
    pub right: f64,
    #[serde(with = "crate::units::screen")]
    pub bottom: f64,
    /// UI pixels per source pixel of the edges (1..8; absent: 1).
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::screen::opt")]
    pub scale: Option<f64>,
}

/// Where the interface puts things.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UiLayout {
    /// One of [`ui_hud_positions`]; absent: top.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hud_position: Option<String>,
    /// Show the toolbar buttons; absent: shown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub toolbar: Option<bool>,
    /// One of [`ui_clock_styles`]; absent: `12h`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clock_style: Option<String>,
    /// One of [`ui_dialogue_positions`]; absent: bottom.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dialogue_position: Option<String>,
    /// The side of a portrait in the dialogue box, in UI pixels (32..160; absent: 48).
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::screen::opt")]
    pub portrait_size: Option<f64>,
}

/// The title screen: what is behind it, the logo and where the menu sits.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UiTitleScreen {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<VisualRef>,
    /// A scene drawn behind the menu instead of the start scene.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo: Option<VisualRef>,
    /// One of [`ui_menu_positions`]; absent: center.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu_position: Option<String>,
}

/// F# `UiPaletteRoles`.
pub mod ui_palette_roles {
    pub const ALL: &[&str] = &[
        "backdrop",
        "background",
        "panel",
        "panelBorder",
        "panelHeader",
        "hud",
        "hudBorder",
        "row",
        "rowBorder",
        "text",
        "muted",
        "faint",
        "accent",
        "onAccent",
        "accentText",
        "primary",
        "secondary",
        "warning",
        "error",
        "successTint",
        "errorTint",
        "button",
        "buttonHover",
        "buttonPressed",
        "buttonBorder",
        "focus",
        "track",
        "shadow",
    ];
}

/// F# `UiSkinParts`.
pub mod ui_skin_parts {
    pub const PANEL: &str = "panel";
    pub const HUD: &str = "hud";
    pub const BUTTON: &str = "button";
    pub const BUTTON_HOVER: &str = "buttonHover";
    pub const BUTTON_PRESSED: &str = "buttonPressed";
    pub const BUTTON_DISABLED: &str = "buttonDisabled";
    pub const SLOT: &str = "slot";
    pub const TOOLTIP: &str = "tooltip";
    pub const DIALOGUE: &str = "dialogue";
    pub const PORTRAIT_FRAME: &str = "portraitFrame";

    pub const ALL: &[&str] =
        &[PANEL, HUD, BUTTON, BUTTON_HOVER, BUTTON_PRESSED, BUTTON_DISABLED, SLOT, TOOLTIP, DIALOGUE, PORTRAIT_FRAME];
}

/// F# `UiSkinIcons`.
pub mod ui_skin_icons {
    pub const ALL: &[&str] = &[
        "money",
        "energy",
        "health",
        "season",
        "day",
        "weather",
        "time",
        "inventory",
        "quests",
        "craft",
        "goals",
        "mail",
        "sleep",
        "menu",
    ];
}

/// F# `UiHudPositions`.
pub mod ui_hud_positions {
    pub const TOP: &str = "top";
    pub const BOTTOM: &str = "bottom";
    pub const ALL: &[&str] = &[TOP, BOTTOM];
}

/// F# `UiClockStyles`.
pub mod ui_clock_styles {
    pub const TWELVE_HOUR: &str = "12h";
    pub const TWENTY_FOUR_HOUR: &str = "24h";
    pub const HIDDEN: &str = "hidden";
    pub const ALL: &[&str] = &[TWELVE_HOUR, TWENTY_FOUR_HOUR, HIDDEN];
}

/// F# `UiDialoguePositions`.
pub mod ui_dialogue_positions {
    pub const BOTTOM: &str = "bottom";
    pub const TOP: &str = "top";
    pub const ALL: &[&str] = &[BOTTOM, TOP];
}

/// F# `UiMenuPositions`.
pub mod ui_menu_positions {
    pub const CENTER: &str = "center";
    pub const LEFT: &str = "left";
    pub const RIGHT: &str = "right";
    pub const BOTTOM: &str = "bottom";
    pub const ALL: &[&str] = &[CENTER, LEFT, RIGHT, BOTTOM];
}
