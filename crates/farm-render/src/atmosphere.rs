//! Port of `Atmosphere.cs`: the day/night multiply tint, seasonal foliage tints, and the
//! deterministic weather particles (a hash of tick and index, no random source, so a frame is a
//! pure function of the snapshot). A game's own look (M1: daylight keyframes, day length,
//! foliage tint, weather overlays) replaces the built-in one part by part.

use crate::css_color::{self, Color};
use crate::num::{cs_clamp, round_half_even, to_byte, to_int};
use farm_sim::schema::{overlay_styles, DaylightKey, WeatherOverlayDefinition};

/// (minute of day, RGB multiplier) keyframes; linear between them.
const DAYLIGHT: [(f64, f32, f32, f32); 8] = [
    (0.0, 0.42, 0.50, 0.86), // deep night: cool blue
    (5.0 * 60.0, 0.42, 0.50, 0.86),
    (6.0 * 60.0 + 30.0, 1.00, 0.82, 0.66), // dawn: warm
    (8.0 * 60.0, 1.0, 1.0, 1.0),           // full day
    (17.0 * 60.0, 1.0, 1.0, 1.0),
    (18.0 * 60.0 + 40.0, 1.00, 0.74, 0.56), // dusk: warm
    (20.0 * 60.0 + 30.0, 0.42, 0.50, 0.86),
    (24.0 * 60.0, 0.42, 0.50, 0.86),
];

/// Minutes in a day without a game's own length.
pub const DAY_MINUTES: f64 = 1440.0;

/// The multiply tint for a minute of day: white at midday (no tint), warm around dawn and dusk,
/// cool blue at night. Always opaque.
pub fn daylight_tint(time_minutes: f64) -> Color {
    daylight_tint_in(time_minutes, DAY_MINUTES, &[])
}

/// [`daylight_tint`] for a day of `day_minutes` (anything but a positive number reads as 1440)
/// and a game's own keyframes. Empty `keys` stretch the built-in curve over the day; otherwise
/// the tint runs linearly from key to key (unparseable colours are skipped), wrapping from the
/// last key of the day to the first.
pub fn daylight_tint_in(time_minutes: f64, day_minutes: f64, keys: &[DaylightKey]) -> Color {
    let day = if day_minutes.is_finite() && day_minutes > 0.0 { day_minutes } else { DAY_MINUTES };
    let minute = ((time_minutes % day) + day) % day;
    if let Some(color) = keyed_tint(minute, day, keys) {
        return color;
    }
    let scale = day / DAY_MINUTES;
    for i in 1..DAYLIGHT.len() {
        let (m1, r1, g1, b1) = DAYLIGHT[i];
        let m1 = m1 * scale;
        if minute > m1 {
            continue;
        }
        let (m0, r0, g0, b0) = DAYLIGHT[i - 1];
        let m0 = m0 * scale;
        let t = if m1 == m0 { 0.0 } else { ((minute - m0) / (m1 - m0)) as f32 };
        return Color::rgb(channel(r0, r1, t), channel(g0, g1, t), channel(b0, b1, t));
    }
    Color::WHITE
}

/// The tint between a game's own keyframes; `None` without a usable key.
fn keyed_tint(minute: f64, day: f64, keys: &[DaylightKey]) -> Option<Color> {
    let mut points: Vec<(f64, Color)> = keys
        .iter()
        .filter(|key| key.minute.is_finite())
        .filter_map(|key| {
            let color = css_color::try_parse(&key.color)?;
            Some((((key.minute % day) + day) % day, Color::rgb(color.r, color.g, color.b)))
        })
        .collect();
    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (first, last) = (*points.first()?, *points.last()?);
    let (from, to, minute) = match points.iter().position(|(at, _)| *at >= minute) {
        Some(i) if i > 0 => (points[i - 1], points[i], minute),
        // Before the first key or after the last: between the last key and tomorrow's first.
        _ => ((last.0, last.1), (first.0 + day, first.1), if minute < last.0 { minute + day } else { minute }),
    };
    let t = if to.0 == from.0 { 0.0 } else { (minute - from.0) / (to.0 - from.0) };
    let mix = |a: u8, b: u8| to_byte(round_half_even(f64::from(a) + ((f64::from(b) - f64::from(a)) * t)));
    Some(Color::rgb(mix(from.1.r, to.1.r), mix(from.1.g, to.1.g), mix(from.1.b, to.1.b)))
}

/// True when [`daylight_tint`] changes nothing (midday).
pub fn is_neutral(tint: Color) -> bool {
    tint.r == 255 && tint.g == 255 && tint.b == 255
}

/// Multiply tint for foliage (trees, weeds) per season: autumn ochre, winter frost; `None` for
/// spring, summer and unknown seasons. Grass tiles have their own seasonal art.
pub fn foliage_tint(season: Option<&str>) -> Option<Color> {
    match season {
        Some("fall" | "autumn") => Some(Color::rgb(0xF0, 0xB8, 0x70)),
        Some("winter") => Some(Color::rgb(0xB8, 0xD0, 0xE8)),
        _ => None,
    }
}

/// A game's foliage tint (`#rrggbb`) when it parses, else the built-in [`foliage_tint`] of the
/// season. Always opaque.
pub fn foliage_tint_in(season: Option<&str>, custom: Option<&str>) -> Option<Color> {
    match custom.and_then(css_color::try_parse) {
        Some(color) => Some(Color::rgb(color.r, color.g, color.b)),
        None => foliage_tint(season),
    }
}

/// The particles of a weather overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherOverlay {
    /// Slanted streaks falling fast.
    Rain,
    /// Round flakes falling slowly and swaying.
    Snow,
    /// Small specks drifting sideways.
    Dust,
    /// No particles: the tint only.
    TintOnly,
}

impl WeatherOverlay {
    /// The style an overlay definition names (`rain`, `snow`, `dust` or `none`).
    pub fn from_style(style: &str) -> Option<Self> {
        match style {
            overlay_styles::RAIN => Some(Self::Rain),
            overlay_styles::SNOW => Some(Self::Snow),
            overlay_styles::DUST => Some(Self::Dust),
            overlay_styles::NONE => Some(Self::TintOnly),
            _ => None,
        }
    }

    /// The particle colour of the style when a definition gives none.
    pub fn default_color(self) -> Color {
        match self {
            Self::Rain => RAIN_DROP,
            Self::Snow => SNOW_FLAKE,
            Self::Dust | Self::TintOnly => DUST_SPECK,
        }
    }
}

const RAIN_TINT: Color = Color::rgb(0xC8, 0xD2, 0xE6);
const RAIN_DROP: Color = Color::rgba(0xD8, 0xEC, 0xFF, 0x9A);
const SNOW_FLAKE: Color = Color::rgba(0xFF, 0xFF, 0xFF, 0xDC);
const DUST_SPECK: Color = Color::rgba(0xD2, 0xB4, 0x8C, 0xB0);

/// How today's weather draws: particle style, colour, amount and speed (relative to the
/// built-in style), and an optional opaque multiply tint over the scene.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherLook {
    pub style: WeatherOverlay,
    pub color: Color,
    pub density: f64,
    pub speed: f64,
    pub tint: Option<Color>,
}

/// The look of today's weather: its overlay definition when there is one (with a style the
/// renderer knows), else the built-in rain or snow [`weather_overlay`] guesses.
pub fn weather_look(
    weather_id: Option<&str>,
    overlay_hint: Option<&str>,
    definition: Option<&WeatherOverlayDefinition>,
) -> Option<WeatherLook> {
    if let Some((definition, style)) =
        definition.and_then(|definition| Some((definition, WeatherOverlay::from_style(&definition.style)?)))
    {
        let number = |value: f64| if value.is_finite() && value >= 0.0 { value } else { 1.0 };
        return Some(WeatherLook {
            style,
            color: css_color::try_parse(&definition.color).unwrap_or_else(|| style.default_color()),
            density: number(definition.density),
            speed: number(definition.speed),
            tint: definition
                .tint
                .as_deref()
                .and_then(css_color::try_parse)
                .map(|tint| Color::rgb(tint.r, tint.g, tint.b)),
        });
    }
    let style = weather_overlay(weather_id, overlay_hint)?;
    Some(WeatherLook {
        style,
        color: style.default_color(),
        density: 1.0,
        speed: 1.0,
        tint: (style == WeatherOverlay::Rain).then_some(RAIN_TINT),
    })
}

/// Overlay kind for a weather id: the definition's hint (`rain` | `snow`) wins, then the id is
/// guessed (`snow`/`blizzard` → snow, `rain`/`storm` → rain).
pub fn weather_overlay(weather_id: Option<&str>, overlay_hint: Option<&str>) -> Option<WeatherOverlay> {
    match overlay_hint {
        Some("rain") => return Some(WeatherOverlay::Rain),
        Some("snow") => return Some(WeatherOverlay::Snow),
        _ => {}
    }
    let id = weather_id?.to_lowercase();
    if id.contains("snow") || id.contains("blizzard") {
        Some(WeatherOverlay::Snow)
    } else if id.contains("rain") || id.contains("storm") {
        Some(WeatherOverlay::Rain)
    } else {
        None
    }
}

/// Deterministic 32-bit mix (same inputs ⇒ same output on every platform).
pub fn hash(a: u32, b: u32) -> u32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_add(0x7F4A_7C15);
    h ^= h >> 16;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 16;
    h
}

/// Uniform [0, 1) from a hash.
pub fn unit(a: u32, b: u32) -> f64 {
    f64::from(hash(a, b)) / 4_294_967_296.0
}

/// Number of weather particles for a viewport (about one per 40×40 px, 8..=600).
pub fn particle_count(view_width: f64, view_height: f64) -> i32 {
    to_int(cs_clamp(round_half_even(view_width * view_height / 1600.0), 8.0, 600.0))
}

fn channel(a: f32, b: f32, t: f32) -> u8 {
    // C# does this in single precision, then rounds half to even as a double.
    to_byte(round_half_even(f64::from(255.0_f32 * (a + ((b - a) * t)))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daylight_is_neutral_at_midday_warm_at_dusk_and_blue_at_night() {
        assert_eq!(daylight_tint(13.0 * 60.0), Color::WHITE);
        assert!(is_neutral(daylight_tint(12.0 * 60.0)));
        let night = daylight_tint(0.0);
        assert_eq!(night, Color::rgb(107, 128, 219));
        assert_eq!(daylight_tint(1440.0 + 60.0), daylight_tint(60.0));
        assert_eq!(daylight_tint(-60.0), daylight_tint(23.0 * 60.0));
        let dusk = daylight_tint(18.0 * 60.0 + 40.0);
        assert_eq!(dusk, Color::rgb(255, 189, 143));
        // Halfway between dawn (6:30) and full day (8:00).
        let morning = daylight_tint(7.0 * 60.0 + 15.0);
        assert_eq!(morning, Color::rgb(255, 232, 212));
    }

    #[test]
    fn foliage_and_weather_overlays() {
        assert_eq!(foliage_tint(Some("autumn")), Some(Color::rgb(0xF0, 0xB8, 0x70)));
        assert_eq!(foliage_tint(Some("winter")), Some(Color::rgb(0xB8, 0xD0, 0xE8)));
        assert_eq!(foliage_tint(Some("spring")), None);
        assert_eq!(foliage_tint(None), None);
        assert_eq!(weather_overlay(Some("sun"), Some("snow")), Some(WeatherOverlay::Snow));
        assert_eq!(weather_overlay(Some("Heavy-Storm"), None), Some(WeatherOverlay::Rain));
        assert_eq!(weather_overlay(Some("blizzard"), Some("fog")), Some(WeatherOverlay::Snow));
        assert_eq!(weather_overlay(Some("sun"), None), None);
        assert_eq!(weather_overlay(None, None), None);
    }

    #[test]
    fn a_day_of_another_length_stretches_the_built_in_curve() {
        for minute in [0.0, 300.0, 390.0, 435.0, 600.0, 1120.0, 1230.0, 1439.0] {
            assert_eq!(daylight_tint_in(minute * 2.0, 2880.0, &[]), daylight_tint(minute), "minute {minute}");
        }
        assert_eq!(daylight_tint_in(3000.0, 2880.0, &[]), daylight_tint(60.0));
        // Nonsense lengths read as 1440; keys without a colour are skipped.
        assert_eq!(daylight_tint_in(500.0, 0.0, &[]), daylight_tint(500.0));
        let blank = DaylightKey { minute: 10.0, color: "nope".into() };
        assert_eq!(daylight_tint_in(500.0, f64::NAN, std::slice::from_ref(&blank)), daylight_tint(500.0));
        // One key: the same tint all day.
        let dusk = DaylightKey { minute: 900.0, color: "#ff8040".into() };
        assert_eq!(daylight_tint_in(10.0, 1440.0, &[dusk.clone(), blank]), Color::rgb(0xff, 0x80, 0x40));
    }

    #[test]
    fn weather_looks_come_from_the_definition_then_the_built_in_guess() {
        let rain = weather_look(Some("rain"), Some("rain"), None).unwrap();
        assert_eq!((rain.style, rain.color, rain.tint), (WeatherOverlay::Rain, RAIN_DROP, Some(RAIN_TINT)));
        let built_in = farm_sim::schema::builtin_weather_overlays();
        assert_eq!(weather_look(Some("rain"), None, Some(&built_in[0])), Some(rain));
        assert_eq!(weather_look(Some("snow"), None, Some(&built_in[1])), weather_look(Some("snow"), None, None));
        let dust = WeatherOverlayDefinition {
            style: "dust".into(),
            color: "".into(),
            density: 0.5,
            speed: f64::NAN,
            tint: Some("#d0c0a080".into()),
            ..WeatherOverlayDefinition::default()
        };
        let look = weather_look(Some("sun"), None, Some(&dust)).unwrap();
        assert_eq!(look.style, WeatherOverlay::Dust);
        assert_eq!((look.color, look.density, look.speed), (DUST_SPECK, 0.5, 1.0));
        assert_eq!(look.tint, Some(Color::rgb(0xd0, 0xc0, 0xa0)), "tints are opaque");
        // A style the renderer doesn't know: the id is guessed, as without a definition.
        let odd = WeatherOverlayDefinition { style: "fog".into(), ..dust };
        assert_eq!(weather_look(Some("storm"), None, Some(&odd)).map(|look| look.style), Some(WeatherOverlay::Rain));
        assert_eq!(weather_look(Some("sun"), None, Some(&odd)), None);
        assert_eq!(foliage_tint_in(Some("winter"), Some("#102030")), Some(Color::rgb(0x10, 0x20, 0x30)));
        assert_eq!(foliage_tint_in(Some("winter"), Some("bad")), foliage_tint(Some("winter")));
    }

    #[test]
    fn particles_are_a_pure_function_of_their_index() {
        assert_eq!(hash(1, 2), hash(1, 2));
        assert_ne!(hash(1, 2), hash(2, 1));
        assert!((0.0..1.0).contains(&unit(7, 3)));
        assert_eq!(particle_count(640.0, 416.0), 166);
        assert_eq!(particle_count(10.0, 10.0), 8);
        assert_eq!(particle_count(4000.0, 4000.0), 600);
    }
}
