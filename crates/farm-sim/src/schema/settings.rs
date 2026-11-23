//! Port of `Settings.cs` (packages/engine-schemas/src/settings.ts).
//!
//! Per-project gameplay settings (vision.md: "light vs sim" — heavy systems are toggleable so
//! cozy configs stay cozy).

use super::combat::CombatSettings;
use super::farm::FarmSettings;
use super::primitives::CLASSIC_SEASONS;
use super::relationships::RelationshipSettings;
use crate::units;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CalendarSeason {
    pub id: String,
    pub name: String,
    /// In-game days this season lasts. int, positive.
    #[serde(with = "crate::units::count")]
    pub days: u32,
    /// How warm the season is, for crops with a temperature range (M1).
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub temperature: Option<i32>,
    /// A music asset that plays in the season instead of the game's own music (M2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub music: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FestivalAttendance {
    pub npc_id: String,
    #[serde(with = "crate::units::int")]
    pub x: i32,
    #[serde(with = "crate::units::int")]
    pub y: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dialogue_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FestivalContest {
    pub item_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_quality: Option<String>,
    pub consume_item: bool,
    pub rewards: Vec<super::EventOutcome>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FestivalActivity {
    #[serde(with = "crate::units::int")]
    pub x: i32,
    #[serde(with = "crate::units::int")]
    pub y: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minigame_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cutscene_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contest: Option<FestivalContest>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FestivalProgram {
    pub scene_id: String,
    pub entrance_scene_id: String,
    #[serde(with = "crate::units::int")]
    pub entrance_x: i32,
    #[serde(with = "crate::units::int")]
    pub entrance_y: i32,
    #[serde(with = "crate::units::int")]
    pub arrival_x: i32,
    #[serde(with = "crate::units::int")]
    pub arrival_y: i32,
    #[serde(with = "crate::units::count")]
    pub open_minute: u32,
    #[serde(with = "crate::units::count")]
    pub close_minute: u32,
    pub end_day: bool,
    pub attendance: Vec<FestivalAttendance>,
    pub conditions: Vec<super::EventCondition>,
    pub on_miss: Vec<super::EventOutcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shop_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shop_tile: Option<super::GridPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity: Option<FestivalActivity>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CalendarFestival {
    pub id: String,
    pub name: String,
    /// Season this festival falls in (matches a CalendarSeason id).
    pub season_id: String,
    /// 1-based day within that season. int, positive.
    #[serde(with = "crate::units::count")]
    pub day: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program: Option<FestivalProgram>,
    /// A music asset that plays all festival day (M2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub music: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CalendarWeekday {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub words: Option<IndexMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CalendarConfig {
    /// Ordered list of seasons; the calendar year is the sum of their lengths.
    pub seasons: Vec<CalendarSeason>,
    /// Named festival days, each pinned to a season + day-of-season.
    pub festivals: Vec<CalendarFestival>,
    /// Absent in legacy projects; an empty list disables the week.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weekdays: Option<Vec<CalendarWeekday>>,
    /// Zero-based offset of absolute day 1 in the week.
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub weekday_offset: Option<u32>,
}

impl Default for CalendarConfig {
    fn default() -> Self {
        Self { seasons: classic_calendar_seasons(), festivals: Vec::new(), weekdays: None, weekday_offset: None }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TimeConfig {
    /// Minute-of-day the player wakes up (6:00). int.
    #[serde(with = "crate::units::count")]
    pub day_start_minute: u32,
    /// Minute-of-day the player collapses if still awake (26:00 = 2am). int.
    #[serde(with = "crate::units::count")]
    pub day_end_minute: u32,
    /// In-game minutes that pass per real-time second. positive, at most 1440.
    #[serde(with = "crate::units::minute_rate")]
    pub minutes_per_real_second: u32,
    /// Whether the clock stops while a dialogue, shop or minigame is open (and, in the player,
    /// while a menu such as the inventory or quest log is open), so reading costs no daytime.
    /// Absent means on ([`TimeConfig::pauses_in_modals`]); v8 ran the clock on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pause_in_modals: Option<bool>,
    /// Minutes in a day (M1); absent: [`units::MINUTES_PER_DAY`].
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub day_minutes: Option<u32>,
}

/// The fewest minutes a day may have.
pub const MIN_DAY_MINUTES: u32 = 60;

impl TimeConfig {
    /// [`TimeConfig::pause_in_modals`], on unless a project turns it off.
    pub fn pauses_in_modals(&self) -> bool {
        self.pause_in_modals != Some(false)
    }

    /// Minutes in a day: [`TimeConfig::day_minutes`], else 1440. A day shorter than
    /// [`MIN_DAY_MINUTES`] reads as that.
    pub fn minutes_per_day(&self) -> u32 {
        self.day_minutes.unwrap_or(units::MINUTES_PER_DAY).max(MIN_DAY_MINUTES)
    }
}

impl Default for TimeConfig {
    fn default() -> Self {
        Self {
            day_start_minute: 6 * 60,
            day_end_minute: 26 * 60,
            minutes_per_real_second: units::MINUTE / units::TICKS_PER_SECOND,
            pause_in_modals: None,
            day_minutes: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MovementConfig {
    /// Player walk speed in tiles per second (free movement). positive.
    #[serde(with = "crate::units::speed")]
    pub player_speed: i32,
}

impl Default for MovementConfig {
    fn default() -> Self {
        Self { player_speed: units::from_authoring::<units::Speed>(4.5) }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectSettings {
    pub movement: MovementConfig,
    pub energy_enabled: bool,
    /// positive.
    #[serde(with = "crate::units::energy")]
    pub max_energy: i32,
    /// Fraction of energy restored after a collapse (vs full sleep). 0..1.
    #[serde(with = "crate::units::milli")]
    pub collapse_energy_fraction: u32,
    /// Money penalty charged on collapse. nonnegative.
    #[serde(with = "crate::units::money")]
    pub collapse_money_penalty: i64,
    pub time: TimeConfig,
    /// Creator-configurable calendar (M9): seasons, their lengths, and festival days.
    pub calendar: CalendarConfig,
    /// Player skills (M4g): XP per action category, levels unlock recipes.
    pub skills_enabled: bool,
    /// XP thresholds per level (index = level).
    #[serde(with = "crate::units::count::vec")]
    pub skill_level_curve: Vec<u32>,
    /// Game-text locale (M7 i18n): packs may carry per-locale string tables.
    pub locale: String,
    /// Optional, creator-controlled credit shown in exported games.
    pub show_made_with_credit: bool,
    /// What the game calls money, energy and the skills (M1); absent: the built-in words.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub words: Option<GameWords>,
    /// The day/night and foliage tints (M1); absent: the built-in look.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub atmosphere: Option<AtmosphereSettings>,
    /// Friendship decay, dating, marriage and children (M4); absent: none of them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationships: Option<RelationshipSettings>,
    /// A new game asks for the player's name, the farm's name and the player's colours (M4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub character_creator: Option<bool>,
    /// The farm scene, its layouts and pests (M5); absent: no layouts, no pests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub farm: Option<FarmSettings>,
    /// Player health, monsters and fainting (M7); absent: no combat.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub combat: Option<CombatSettings>,
    /// Optional caps for permanent player upgrades.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progression: Option<super::ProgressionLimits>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_definitions: Option<Vec<super::SkillDefinition>>,
    /// Budgets of the project's own scripts (#241); absent: the pack plugin defaults.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scripts: Option<super::ScriptSettings>,
    /// Which systems the game has and how its days end (#243); absent: a classic farming game.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<GameRules>,
}

/// How a game sleeps (#243), `GameRules::sleep`.
pub mod sleep_policies {
    /// The day ends in bed or in a collapse (the classic farming day).
    pub const REQUIRED: &str = "required";
    /// The player may sleep; a day nobody slept through still ends and the next begins.
    pub const OPTIONAL: &str = "optional";
    /// Nobody sleeps: the clock runs round the whole day and the date rolls over at midnight.
    pub const NONE: &str = "none";

    pub const ALL: &[&str] = &[REQUIRED, OPTIONAL, NONE];
}

/// What the clock reaching the day's end does (#243), `GameRules::day_end`.
pub mod day_end_rules {
    /// The player collapses: the collapse penalty, part of the energy, the next morning.
    pub const COLLAPSE: &str = "collapse";
    /// The date rolls over and the night's work is done; the player carries on.
    pub const ROLLOVER: &str = "rollover";

    pub const ALL: &[&str] = &[COLLAPSE, ROLLOVER];
}

/// What running out of energy does (#243), `GameRules::exhaustion`.
pub mod exhaustion_rules {
    /// The player collapses and the day ends.
    pub const COLLAPSE: &str = "collapse";
    /// The player is worn out: nothing that costs energy works until some returns.
    pub const REST: &str = "rest";

    pub const ALL: &[&str] = &[COLLAPSE, REST];
}

/// A tile the player is put on (after a collapse).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WakeSpot {
    pub scene_id: String,
    #[serde(with = "crate::units::int")]
    pub x: i32,
    #[serde(with = "crate::units::int")]
    pub y: i32,
}

/// The rules of a game that is not (only) a classic farming game (#243): whether it has farming
/// at all, how it sleeps, and what the end of a day and of the player's energy do. The four
/// things a classic night does together are separate here: the date rolling over (with the
/// night's work: crops, machines, animals, shops, schedules), recovery, the collapse penalty,
/// and moving the player. Every part left out keeps the classic rule, so a project without
/// `rules` plays as it always did. Stamina is `energyEnabled` and combat `combat.enabled`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GameRules {
    /// One of [`sleep_policies`]; absent: required.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sleep: Option<String>,
    /// One of [`day_end_rules`]; absent: collapse when sleep is required, else rollover. A game
    /// without sleep has no day's end: its date rolls over at midnight.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub day_end: Option<String>,
    /// One of [`exhaustion_rules`]; absent: collapse when sleep is required, else rest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exhaustion: Option<String>,
    /// The fraction of the player's energy a rollover nobody slept through gives back, 0..1;
    /// absent: none where the player may sleep, all of it where nobody sleeps.
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::milli::opt")]
    pub rollover_energy_fraction: Option<u32>,
    /// Off: no tilling, watering or planting, and crops on the map do not grow; absent: on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub farming: Option<bool>,
    /// Where a collapse puts the player; absent: where they fell.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collapse_wake: Option<WakeSpot>,
}

/// [`GameRules`] with every default filled in: what the engine asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rules<'a> {
    /// The player may send `sleep`.
    pub can_sleep: bool,
    /// The clock runs round the whole day (no day window) and the date rolls at midnight.
    pub continuous: bool,
    /// The day's end (or, in a continuous game, nothing) collapses the player.
    pub day_end_collapses: bool,
    /// Running out of energy collapses the player; otherwise they are worn out.
    pub exhaustion_collapses: bool,
    /// Thousandths of the player's energy a rollover without sleep gives back.
    pub rollover_energy: u32,
    pub farming: bool,
    pub collapse_wake: Option<&'a WakeSpot>,
}

impl<'a> Rules<'a> {
    /// The classic farming game: what a project without `rules` plays.
    pub const CLASSIC: Rules<'static> = Rules {
        can_sleep: true,
        continuous: false,
        day_end_collapses: true,
        exhaustion_collapses: true,
        rollover_energy: 0,
        farming: true,
        collapse_wake: None,
    };

    /// The rules `settings` asks for. A word the engine does not know keeps the default for its
    /// sleep policy (Problems reports it).
    pub fn of(settings: &'a ProjectSettings) -> Self {
        let Some(rules) = settings.rules.as_ref() else { return Rules::CLASSIC };
        let sleep = rules.sleep.as_deref().unwrap_or(sleep_policies::REQUIRED);
        let continuous = sleep == sleep_policies::NONE;
        let required = !continuous && sleep != sleep_policies::OPTIONAL;
        let day_end_collapses = !continuous
            && match rules.day_end.as_deref() {
                Some(day_end_rules::COLLAPSE) => true,
                Some(day_end_rules::ROLLOVER) => false,
                _ => required,
            };
        let exhaustion_collapses = match rules.exhaustion.as_deref() {
            Some(exhaustion_rules::COLLAPSE) => true,
            Some(exhaustion_rules::REST) => false,
            _ => required,
        };
        Rules {
            can_sleep: !continuous,
            continuous,
            day_end_collapses,
            exhaustion_collapses,
            rollover_energy: rules
                .rollover_energy_fraction
                .unwrap_or(if continuous { units::MILLI_ONE } else { 0 })
                .min(units::MILLI_ONE),
            farming: rules.farming != Some(false),
            collapse_wake: rules.collapse_wake.as_ref(),
        }
    }
}

/// The words of a game (M1): what it calls money, energy and the skills, in every language.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GameWords {
    /// How an amount of money is written, `{0}` standing for the number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub money: Option<String>,
    /// What energy is called.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub energy: Option<String>,
    /// Skill names by skill id.
    pub skills: IndexMap<String, String>,
    /// The same words in other languages, by locale code.
    pub locales: IndexMap<String, GameWords>,
}

impl GameWords {
    /// The words for `locale`: the locale's own where it has them, else these.
    pub fn for_locale(&self, locale: &str) -> GameWords {
        let mut words = GameWords {
            money: self.money.clone(),
            energy: self.energy.clone(),
            skills: self.skills.clone(),
            locales: IndexMap::new(),
        };
        if let Some(local) = self.locales.get(locale) {
            if local.money.is_some() {
                words.money.clone_from(&local.money);
            }
            if local.energy.is_some() {
                words.energy.clone_from(&local.energy);
            }
            for (skill, name) in &local.skills {
                words.skills.insert(skill.clone(), name.clone());
            }
        }
        words
    }
}

/// A day/night tint keyframe: at `minute` of the day the scene is multiplied by `color`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DaylightKey {
    #[serde(with = "crate::units::screen")]
    pub minute: f64,
    /// `#rrggbb` multiply colour.
    pub color: String,
}

/// The look of a game's sky (M1). Empty or absent parts keep the look below them: a scene's
/// look, then its season's, then the game's, then the built-in.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AtmosphereLook {
    pub daylight: Vec<DaylightKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub foliage_tint: Option<String>,
}

/// A game's atmosphere (M1): its look, and a look per season.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AtmosphereSettings {
    /// The look of every season without its own.
    pub all_seasons: AtmosphereLook,
    /// Looks by season id.
    pub seasons: IndexMap<String, AtmosphereLook>,
}

impl AtmosphereSettings {
    /// The look of `season` in `scene`: each part from the first of the scene's look, the
    /// season's, then the game's that has it (`None` where none does: the built-in look).
    pub fn resolve(
        settings: Option<&AtmosphereSettings>,
        season: &str,
        scene: Option<&AtmosphereLook>,
    ) -> AtmosphereLook {
        let mut layers: Vec<&AtmosphereLook> = Vec::new();
        layers.extend(scene);
        if let Some(settings) = settings {
            layers.extend(settings.seasons.get(season));
            layers.push(&settings.all_seasons);
        }
        AtmosphereLook {
            daylight: layers
                .iter()
                .find(|look| !look.daylight.is_empty())
                .map(|look| look.daylight.clone())
                .unwrap_or_default(),
            foliage_tint: layers.iter().find_map(|look| look.foliage_tint.clone()),
        }
    }
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            movement: MovementConfig::default(),
            energy_enabled: true,
            max_energy: units::points(100),
            collapse_energy_fraction: 500,
            collapse_money_penalty: 50,
            time: TimeConfig::default(),
            calendar: CalendarConfig::default(),
            skills_enabled: true,
            skill_level_curve: vec![0, 50, 150, 300, 500, 750, 1050, 1400, 1800, 2250],
            locale: "en".to_owned(),
            show_made_with_credit: false,
            words: None,
            atmosphere: None,
            relationships: None,
            character_creator: None,
            farm: None,
            combat: None,
            progression: None,
            skill_definitions: None,
            scripts: None,
            rules: None,
        }
    }
}

/// The classic four-season, 28-day-per-season calendar (today's hardcoded default).
pub fn classic_calendar_seasons() -> Vec<CalendarSeason> {
    CLASSIC_SEASONS
        .iter()
        .map(|id| {
            let mut chars = id.chars();
            let name = match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            };
            CalendarSeason { id: (*id).to_owned(), name, days: 28, temperature: None, music: None }
        })
        .collect()
}

/// TS `DEFAULT_PROJECT_SETTINGS = ProjectSettingsSchema.parse({})`.
pub fn default_project_settings() -> ProjectSettings {
    ProjectSettings::default()
}

impl ProjectSettings {
    pub fn skill_definitions(&self) -> &[super::SkillDefinition] {
        self.skill_definitions.as_deref().unwrap_or(&[])
    }
}
