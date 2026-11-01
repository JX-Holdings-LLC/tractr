//! The trusted tier (#241): a creator's own project scripts.
//!
//! A project script is JavaScript of the game itself, kept in the project (`scripts`, one
//! `scripts/<id>.js` file each in a project folder) rather than in a pack. It runs in the same
//! sandbox, with the same API and the same determinism as a pack plugin, but it is trusted:
//!
//! - it hears every hook and may answer every mutation (`*`), and reads everything;
//! - ids are used as written: `giveItem` with `itemId: 'turnip'` gives the game's own turnip,
//!   and a condition, outcome, entity type or item use it defines has the plain name it gives;
//! - budgets come from the project's `settings.scripts` ([`ScriptBudgets`]);
//! - an overrun is never a strike: the script is rebuilt and keeps running, and the error is
//!   reported each time (Play Mode shows it as an error with the script's stack; an exported
//!   game logs it). See [`PluginSpec::trusted`].
//!
//! Player-installed mods stay in the pack tier: only the project itself can hold scripts.

use crate::host::PluginHostOptions;
use crate::spec::PluginSpec;
use farm_sim::schema::{GameProject, ScriptSettings};

/// The plugin id prefix of project scripts: `script/<id>` (pack ids cannot hold a `/`).
pub const PROJECT_SCRIPT_PREFIX: &str = farm_sim::schema::PROJECT_SCRIPT_PREFIX;

/// The specs of a project's enabled scripts, in project order.
pub fn project_script_specs(project: &GameProject) -> Vec<PluginSpec> {
    project
        .scripts
        .iter()
        .flatten()
        .filter(|script| !script.disabled && !script.id.trim().is_empty())
        .map(|script| script_spec(&script.id, &script.source))
        .collect()
}

/// The spec of one project script.
pub fn script_spec(id: &str, source: &str) -> PluginSpec {
    PluginSpec {
        id: format!("{PROJECT_SCRIPT_PREFIX}{id}"),
        pack_id: String::new(),
        source: source.to_owned(),
        granted_hooks: Vec::new(),
        granted_mutations: Some(vec!["*".to_owned()]),
        granted_minigames: Vec::new(),
        granted_reads: Vec::new(),
        trusted: true,
    }
}

/// Whether `plugin_id` is a project script's.
pub fn is_project_script(plugin_id: &str) -> bool {
    plugin_id.starts_with(PROJECT_SCRIPT_PREFIX)
}

/// The budgets of project scripts: the project's `settings.scripts` over the pack defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptBudgets {
    /// Fuel per handler call.
    pub fuel_per_call: u64,
    /// Fuel per second of game time.
    pub fuel_per_window: u64,
    /// Fuel to start a script.
    pub init_fuel: u64,
}

impl ScriptBudgets {
    /// The budgets `settings` asks for, within sane bounds (at least 1 million per call, at
    /// most 2 billion), over `defaults`.
    pub fn from_settings(settings: Option<&ScriptSettings>, defaults: &PluginHostOptions) -> Self {
        let clamp = |value: Option<u64>, default: u64| value.map_or(default, |v| v.clamp(1_000_000, 2_000_000_000));
        Self {
            fuel_per_call: clamp(settings.and_then(|s| s.fuel_per_call), defaults.fuel_per_call),
            fuel_per_window: clamp(settings.and_then(|s| s.fuel_per_second), defaults.fuel_per_window),
            init_fuel: clamp(settings.and_then(|s| s.init_fuel), defaults.init_fuel),
        }
    }
}
