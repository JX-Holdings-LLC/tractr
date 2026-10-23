//! What a pack-defined minigame draws: a small, validated list of draw primitives in
//! minigame-local coordinates (docs/PLUGINS.md "Minigames"). A pack plugin's `draw` answers
//! JSON; [`parse_canvas`] turns it into [`CanvasCommand`]s or says why it can't, and the game UI
//! (`farm-ui`) scales them into the minigame window.
//!
//! The canvas is [`CANVAS_WIDTH`] × [`CANVAS_HEIGHT`] units, origin top left, `y` down. Anything
//! outside it is clipped. Three primitives exist:
//!
//! - `{ type: 'rect', x, y, w, h, color?, radius?, outline? }`: a filled rectangle (`outline`: a
//!   stroke of that width instead), with rounded corners of `radius`.
//! - `{ type: 'text', x, y, text, size?, color?, align?, bold? }`: one line of text whose baseline
//!   is at `y`; `align` is `left` (default), `center` or `right` of `x`.
//! - `{ type: 'sprite', x, y, w, h, item }`: an item's art fitted into the rectangle (the same art
//!   the inventory shows). A plain `item` id means the pack's own item (`packId:item`) when the
//!   game has one, else the id as written.
//!
//! Colors are `#rgb`, `#rrggbb`, `#rrggbbaa` or a name of the game's theme ([`ThemeColor`]), so
//! a minigame matches the rest of the game's interface. Every limit ([`CanvasLimits`]) is checked;
//! a draw list that breaks one is refused as a whole.

use serde::Serialize;
use serde_json::{Map, Value};

/// Canvas width in minigame units.
pub const CANVAS_WIDTH: f32 = 400.0;
/// Canvas height in minigame units.
pub const CANVAS_HEIGHT: f32 = 240.0;
/// Largest coordinate or size accepted (in either direction).
pub const MAX_COORDINATE: f64 = 10_000.0;
/// Text sizes are clamped to this range (minigame units).
pub const TEXT_SIZE_RANGE: (f32, f32) = (6.0, 64.0);

/// Bounds on one draw list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanvasLimits {
    /// Most primitives in one draw list.
    pub max_commands: usize,
    /// Most characters in one text primitive.
    pub max_text_chars: usize,
    /// Most characters of text in one draw list, all text primitives together.
    pub max_total_text_chars: usize,
}

impl Default for CanvasLimits {
    fn default() -> Self {
        Self { max_commands: 256, max_text_chars: 200, max_total_text_chars: 4_000 }
    }
}

/// A color of the game's theme (`farm-ui`'s palette).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeColor {
    Text,
    Muted,
    Primary,
    Accent,
    Track,
    Panel,
    Background,
    Error,
}

impl ThemeColor {
    /// Every theme color with its name, as plugins write it.
    pub const ALL: [(&'static str, ThemeColor); 8] = [
        ("text", ThemeColor::Text),
        ("muted", ThemeColor::Muted),
        ("primary", ThemeColor::Primary),
        ("accent", ThemeColor::Accent),
        ("track", ThemeColor::Track),
        ("panel", ThemeColor::Panel),
        ("background", ThemeColor::Background),
        ("error", ThemeColor::Error),
    ];
}

/// A primitive's color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum CanvasColor {
    /// A theme color, so the minigame follows the game's interface.
    Theme(ThemeColor),
    /// Red, green, blue, alpha.
    Rgba([u8; 4]),
}

/// Horizontal alignment of a text primitive at its `x`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CanvasAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// One validated draw primitive, in canvas units.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum CanvasCommand {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: CanvasColor,
        /// Corner radius (at most half the shorter side).
        radius: f32,
        /// Stroke width; `None` fills the rectangle.
        outline: Option<f32>,
    },
    Text {
        x: f32,
        /// The baseline.
        y: f32,
        text: String,
        size: f32,
        color: CanvasColor,
        align: CanvasAlign,
        bold: bool,
    },
    Sprite {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        /// The item id as the plugin wrote it (see the module docs for how it resolves).
        item: String,
    },
}

/// A pack minigame's current frame: what the game UI draws in its window.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinigameCanvas {
    /// The pack that defines the minigame (plain sprite ids name its items).
    pub pack_id: String,
    pub commands: Vec<CanvasCommand>,
}

impl MinigameCanvas {
    /// The item ids a sprite with `item` may mean, in the order to try them: the pack's own item
    /// first for a plain id, then the id as written.
    pub fn sprite_candidates<'a>(&'a self, item: &'a str) -> impl Iterator<Item = String> + 'a {
        let own = (!item.contains(':')).then(|| format!("{}:{item}", self.pack_id));
        own.into_iter().chain(std::iter::once(item.to_owned()))
    }
}

/// The pointer (mouse or touch) over a pack minigame's canvas in one frame: what an `update` gets
/// as `input.pointer`. The game UI reports it while the pointer is over the canvas, or held after
/// going down on it; a press anywhere else never reaches the minigame.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct CanvasPointer {
    /// Position in canvas units (the space `draw` uses), within the canvas: a drag that leaves
    /// it stays on its edge.
    pub x: f32,
    pub y: f32,
    /// The primary button (or the finger) is down.
    pub down: bool,
    /// It went down on the canvas this frame.
    pub pressed: bool,
    /// It went up this frame after going down on the canvas (anywhere).
    pub released: bool,
}

impl CanvasPointer {
    /// A pointer at `(x, y)` in canvas units, clamped into the canvas (non-finite counts as 0).
    pub fn at(x: f32, y: f32, down: bool, pressed: bool, released: bool) -> Self {
        let clamp = |value: f32, max: f32| if value.is_finite() { value.clamp(0.0, max) } else { 0.0 };
        Self { x: clamp(x, CANVAS_WIDTH), y: clamp(y, CANVAS_HEIGHT), down, pressed, released }
    }

    /// Two reports before one frame of the minigame as one: the later position and held state,
    /// the presses and releases of both.
    pub fn then(self, later: Self) -> Self {
        Self { pressed: self.pressed || later.pressed, released: self.released || later.released, ..later }
    }
}

fn number(object: &Map<String, Value>, key: &str, at: &str) -> Result<Option<f32>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => match n.as_f64() {
            Some(v) if v.is_finite() && v.abs() <= MAX_COORDINATE => Ok(Some(v as f32)),
            _ => Err(format!("{at}: {key} must be a number between -{MAX_COORDINATE} and {MAX_COORDINATE}")),
        },
        Some(_) => Err(format!("{at}: {key} must be a number")),
    }
}

fn required(object: &Map<String, Value>, key: &str, at: &str) -> Result<f32, String> {
    number(object, key, at)?.ok_or_else(|| format!("{at}: {key} is required"))
}

fn size(object: &Map<String, Value>, key: &str, at: &str) -> Result<f32, String> {
    let value = required(object, key, at)?;
    if value < 0.0 {
        return Err(format!("{at}: {key} must not be negative"));
    }
    Ok(value)
}

fn hex(digits: &str) -> Option<Vec<u8>> {
    let expand = digits.len() <= 4;
    let step = if expand { 1 } else { 2 };
    (0..digits.len() / step)
        .map(|index| {
            let part = digits.get(index * step..index * step + step)?;
            let value = u8::from_str_radix(part, 16).ok()?;
            Some(if expand { value * 17 } else { value })
        })
        .collect()
}

/// `#rgb`, `#rrggbb`, `#rrggbbaa` or a theme color name.
pub fn parse_color(text: &str) -> Option<CanvasColor> {
    if let Some(digits) = text.strip_prefix('#') {
        if !matches!(digits.len(), 3 | 6 | 8) || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let parts = hex(digits)?;
        let alpha = parts.get(3).copied().unwrap_or(255);
        return Some(CanvasColor::Rgba([parts[0], parts[1], parts[2], alpha]));
    }
    ThemeColor::ALL.iter().find(|(name, _)| *name == text).map(|(_, color)| CanvasColor::Theme(*color))
}

fn color(object: &Map<String, Value>, at: &str, fallback: ThemeColor) -> Result<CanvasColor, String> {
    match object.get("color") {
        None | Some(Value::Null) => Ok(CanvasColor::Theme(fallback)),
        Some(Value::String(text)) => parse_color(text).ok_or_else(|| {
            format!("{at}: color \"{}\" is not #rgb, #rrggbb, #rrggbbaa or a theme color", clip_chars(text, 40))
        }),
        Some(_) => Err(format!("{at}: color must be text")),
    }
}

fn boolean(object: &Map<String, Value>, key: &str, at: &str) -> Result<bool, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(format!("{at}: {key} must be true or false")),
    }
}

/// `text` cut to `max` characters.
pub(crate) fn clip_chars(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_owned(),
    }
}

fn check_keys(object: &Map<String, Value>, allowed: &[&str], at: &str) -> Result<(), String> {
    match object.keys().find(|key| key.as_str() != "type" && !allowed.contains(&key.as_str())) {
        Some(key) => Err(format!("{at}: unknown key \"{}\"", clip_chars(key, 40))),
        None => Ok(()),
    }
}

/// A plugin's draw answer as primitives: an array of at most `limits.max_commands` objects of
/// the three types in the module docs. Errors name the first problem.
pub fn parse_canvas(raw: &Value, limits: &CanvasLimits) -> Result<Vec<CanvasCommand>, String> {
    let Value::Array(items) = raw else { return Err("draw must return an array of draw commands".to_owned()) };
    if items.len() > limits.max_commands {
        return Err(format!(
            "draw returned {} draw commands; at most {} are allowed",
            items.len(),
            limits.max_commands
        ));
    }
    let mut total_text = 0usize;
    let mut commands = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let at = format!("draw command {index}");
        let Value::Object(object) = item else { return Err(format!("{at}: must be an object")) };
        let kind = match object.get("type") {
            Some(Value::String(kind)) => kind.as_str(),
            _ => return Err(format!("{at}: type must be \"rect\", \"text\" or \"sprite\"")),
        };
        let command = match kind {
            "rect" => {
                check_keys(object, &["x", "y", "w", "h", "color", "radius", "outline"], &at)?;
                let (w, h) = (size(object, "w", &at)?, size(object, "h", &at)?);
                let radius = number(object, "radius", &at)?.unwrap_or(0.0).clamp(0.0, w.min(h) / 2.0);
                let outline = match number(object, "outline", &at)? {
                    Some(width) if width <= 0.0 => return Err(format!("{at}: outline must be more than 0")),
                    Some(width) => Some(width.min(64.0)),
                    None => None,
                };
                CanvasCommand::Rect {
                    x: required(object, "x", &at)?,
                    y: required(object, "y", &at)?,
                    w,
                    h,
                    color: color(object, &at, ThemeColor::Primary)?,
                    radius,
                    outline,
                }
            }
            "text" => {
                check_keys(object, &["x", "y", "text", "size", "color", "align", "bold"], &at)?;
                let text = match object.get("text") {
                    Some(Value::String(text)) => text.clone(),
                    Some(Value::Number(n)) => n.to_string(),
                    _ => return Err(format!("{at}: text must be text")),
                };
                let chars = text.chars().count();
                if chars > limits.max_text_chars {
                    return Err(format!(
                        "{at}: text is {chars} characters; at most {} are allowed",
                        limits.max_text_chars
                    ));
                }
                total_text += chars;
                if total_text > limits.max_total_text_chars {
                    return Err(format!(
                        "draw returned more than {} characters of text in all",
                        limits.max_total_text_chars
                    ));
                }
                // One line: control characters (line breaks, tabs) become spaces.
                let text: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
                let align = match object.get("align") {
                    None | Some(Value::Null) => CanvasAlign::Left,
                    Some(Value::String(align)) if align == "left" => CanvasAlign::Left,
                    Some(Value::String(align)) if align == "center" => CanvasAlign::Center,
                    Some(Value::String(align)) if align == "right" => CanvasAlign::Right,
                    Some(_) => return Err(format!("{at}: align must be \"left\", \"center\" or \"right\"")),
                };
                CanvasCommand::Text {
                    x: required(object, "x", &at)?,
                    y: required(object, "y", &at)?,
                    text,
                    size: number(object, "size", &at)?.unwrap_or(14.0).clamp(TEXT_SIZE_RANGE.0, TEXT_SIZE_RANGE.1),
                    color: color(object, &at, ThemeColor::Text)?,
                    align,
                    bold: boolean(object, "bold", &at)?,
                }
            }
            "sprite" => {
                check_keys(object, &["x", "y", "w", "h", "item"], &at)?;
                let item = match object.get("item") {
                    Some(Value::String(item)) if !item.is_empty() && item.len() <= 256 => item.clone(),
                    _ => return Err(format!("{at}: item must be an item id")),
                };
                CanvasCommand::Sprite {
                    x: required(object, "x", &at)?,
                    y: required(object, "y", &at)?,
                    w: size(object, "w", &at)?,
                    h: size(object, "h", &at)?,
                    item,
                }
            }
            other => return Err(format!("{at}: unknown type \"{}\" (rect, text or sprite)", clip_chars(other, 40))),
        };
        commands.push(command);
    }
    Ok(commands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(raw: Value) -> Result<Vec<CanvasCommand>, String> {
        parse_canvas(&raw, &CanvasLimits::default())
    }

    #[test]
    fn parses_the_three_primitives_with_defaults() {
        let commands = parse(json!([
            { "type": "rect", "x": 0, "y": 0, "w": 400, "h": 240, "color": "#123", "radius": 500 },
            { "type": "rect", "x": 10, "y": 10, "w": 20, "h": 30, "outline": 2 },
            { "type": "text", "x": 200, "y": 40, "text": "Score\n3", "align": "center", "bold": true, "size": 200 },
            { "type": "text", "x": 1, "y": 2, "text": 42 },
            { "type": "sprite", "x": 5, "y": 6, "w": 32, "h": 32, "item": "lantern" },
        ]))
        .unwrap();
        assert_eq!(
            commands[0],
            CanvasCommand::Rect {
                x: 0.0,
                y: 0.0,
                w: 400.0,
                h: 240.0,
                color: CanvasColor::Rgba([0x11, 0x22, 0x33, 255]),
                radius: 120.0,
                outline: None
            }
        );
        assert!(matches!(
            commands[1],
            CanvasCommand::Rect { color: CanvasColor::Theme(ThemeColor::Primary), outline: Some(2.0), .. }
        ));
        assert_eq!(
            commands[2],
            CanvasCommand::Text {
                x: 200.0,
                y: 40.0,
                text: "Score 3".into(),
                size: 64.0,
                color: CanvasColor::Theme(ThemeColor::Text),
                align: CanvasAlign::Center,
                bold: true
            }
        );
        assert!(matches!(&commands[3], CanvasCommand::Text { text, .. } if text == "42"));
        assert!(matches!(&commands[4], CanvasCommand::Sprite { item, .. } if item == "lantern"));
    }

    #[test]
    fn colors() {
        assert_eq!(parse_color("#ff000080"), Some(CanvasColor::Rgba([255, 0, 0, 128])));
        assert_eq!(parse_color("#0f0"), Some(CanvasColor::Rgba([0, 255, 0, 255])));
        assert_eq!(parse_color("accent"), Some(CanvasColor::Theme(ThemeColor::Accent)));
        for bad in ["#ff00", "#gg0000", "red", "", "#", "#ééé"] {
            assert_eq!(parse_color(bad), None, "{bad}");
        }
    }

    #[test]
    fn refuses_what_breaks_a_rule() {
        let refused = |raw: Value| parse(raw).unwrap_err();
        assert_eq!(refused(json!({})), "draw must return an array of draw commands");
        assert_eq!(refused(json!([1])), "draw command 0: must be an object");
        assert_eq!(
            refused(json!([{ "type": "circle" }])),
            "draw command 0: unknown type \"circle\" (rect, text or sprite)"
        );
        assert_eq!(refused(json!([{ "type": "rect", "x": 0, "y": 0, "w": 1 }])), "draw command 0: h is required");
        assert_eq!(
            refused(json!([{ "type": "rect", "x": 0, "y": 0, "w": -1, "h": 1 }])),
            "draw command 0: w must not be negative"
        );
        assert_eq!(
            refused(json!([{ "type": "rect", "x": 1e9, "y": 0, "w": 1, "h": 1 }])),
            "draw command 0: x must be a number between -10000 and 10000"
        );
        assert_eq!(
            refused(json!([{ "type": "rect", "x": 0, "y": 0, "w": 1, "h": 1, "widht": 3 }])),
            "draw command 0: unknown key \"widht\""
        );
        assert_eq!(
            refused(json!([{ "type": "text", "x": 0, "y": 0, "text": "x".repeat(201) }])),
            "draw command 0: text is 201 characters; at most 200 are allowed"
        );
        assert_eq!(
            refused(json!([{ "type": "text", "x": 0, "y": 0, "text": "hi", "color": "plaid" }])),
            "draw command 0: color \"plaid\" is not #rgb, #rrggbb, #rrggbbaa or a theme color"
        );
        assert_eq!(
            refused(json!([{ "type": "sprite", "x": 0, "y": 0, "w": 1, "h": 1 }])),
            "draw command 0: item must be an item id"
        );
        let many: Vec<Value> = (0..257).map(|_| json!({ "type": "rect", "x": 0, "y": 0, "w": 1, "h": 1 })).collect();
        assert_eq!(refused(Value::Array(many)), "draw returned 257 draw commands; at most 256 are allowed");
        let wordy: Vec<Value> =
            (0..21).map(|_| json!({ "type": "text", "x": 0, "y": 0, "text": "y".repeat(200) })).collect();
        assert_eq!(refused(Value::Array(wordy)), "draw returned more than 4000 characters of text in all");
    }

    #[test]
    fn plain_sprite_ids_try_the_pack_first() {
        let canvas = MinigameCanvas { pack_id: "glow".into(), commands: Vec::new() };
        assert_eq!(canvas.sprite_candidates("lantern").collect::<Vec<_>>(), ["glow:lantern", "lantern"]);
        assert_eq!(canvas.sprite_candidates("base:seed").collect::<Vec<_>>(), ["base:seed"]);
    }

    #[test]
    fn pointers_stay_on_the_canvas_and_merge_their_edges() {
        let inside = CanvasPointer::at(12.5, 30.0, true, true, false);
        assert_eq!((inside.x, inside.y), (12.5, 30.0));
        let dragged = CanvasPointer::at(-20.0, 999.0, true, false, false);
        assert_eq!((dragged.x, dragged.y), (0.0, CANVAS_HEIGHT));
        assert_eq!(CanvasPointer::at(f32::NAN, f32::INFINITY, false, false, false).x, 0.0);
        let up = CanvasPointer::at(50.0, 60.0, false, false, true);
        assert_eq!(inside.then(up), CanvasPointer { x: 50.0, y: 60.0, down: false, pressed: true, released: true });
        assert_eq!(
            serde_json::to_value(inside).unwrap(),
            json!({ "x": 12.5, "y": 30.0, "down": true, "pressed": true, "released": false })
        );
    }
}
