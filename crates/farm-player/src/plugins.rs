//! The plugin sandbox (`farm-plugins`) attached to a [`PlaySession`](crate::session::PlaySession).

use crate::session::{describe_plugin_error, SessionPlugins};
use farm_cart::CartPlugin;
use farm_plugins::{
    GameView, PackMinigame, PluginError, PluginHostOptions, PluginRuntime, PluginSpec, ScriptBudgets, WasmPluginHost,
};
use farm_sim::schema::GameProject;
use farm_sim::{Command, HookEvent};
use serde_json::Value;

impl SessionPlugins for PluginRuntime<WasmPluginHost> {
    fn dispatch(&mut self, events: &[HookEvent], depth: u32, game: GameView<'_>) {
        // Errors are kept in the runtime's bounded recent-error list.
        self.dispatch_events_in(events, depth, Some(game));
    }

    fn drain_commands(&mut self) -> Vec<(Command, u32)> {
        self.drain()
            .into_iter()
            .map(|queued| {
                let depth = queued.depth;
                (queued.into_command(), depth)
            })
            .collect()
    }

    fn begin_tick(&mut self, tick: u64) {
        PluginRuntime::begin_tick(self, tick);
    }

    fn restore(&mut self, saved: &indexmap::IndexMap<String, Value>, game: GameView<'_>) {
        // Errors are kept in the runtime's bounded recent-error list.
        PluginRuntime::restore_in(self, saved, Some(game));
    }

    fn restore_rebuilt(&mut self, saved: &indexmap::IndexMap<String, Value>, game: GameView<'_>) {
        PluginRuntime::restore_rebuilt_in(self, saved, Some(game));
    }

    fn condition_host(&mut self) -> Option<Box<dyn farm_sim::PluginConditions>> {
        PluginRuntime::condition_host(self).map(|host| Box::new(host) as Box<dyn farm_sim::PluginConditions>)
    }

    fn call_screen(&mut self, hook: &str, payload: &str, game: GameView<'_>) -> Option<Result<Value, PluginError>> {
        self.host_mut().call_value(hook, payload, Some(game), None)
    }

    fn queue_screen_answer(&mut self, hook: &str, answer: &Value, game: GameView<'_>) {
        PluginRuntime::queue_answer(self, hook, answer, Some(game));
    }

    fn screen_hooks(&self) -> Vec<String> {
        self.hook_names().iter().filter(|hook| hook.starts_with(farm_plugins::SCREEN_HOOK_PREFIX)).cloned().collect()
    }

    fn recent_errors(&self) -> Vec<String> {
        PluginRuntime::recent_errors(self).map(describe_plugin_error).collect()
    }

    fn start_minigame(&mut self, kind: &str, config: &Value, seed: u32) -> Option<Result<PackMinigame, PluginError>> {
        self.host().start_minigame(kind, config, seed)
    }

    fn report_error(&mut self, error: PluginError) {
        self.report(error);
    }
}

/// The sandbox for a cartridge's plugins; `None` when the game ships none. A plugin whose id is
/// `script/<id>` is one of the game's own project scripts (#241): trusted, with the budgets of
/// `scripts` (the game's `settings.scripts`).
pub fn for_cartridge(
    plugins: &[CartPlugin],
    options: PluginHostOptions,
    scripts: Option<&farm_sim::schema::ScriptSettings>,
) -> Option<Box<dyn SessionPlugins>> {
    if plugins.is_empty() {
        return None;
    }
    let options =
        PluginHostOptions { script_budgets: Some(ScriptBudgets::from_settings(scripts, &options)), ..options };
    let specs: Vec<PluginSpec> = plugins
        .iter()
        .map(|plugin| PluginSpec {
            id: plugin.id.clone(),
            pack_id: plugin.pack_id.clone(),
            source: plugin.source.clone(),
            granted_hooks: plugin.granted_hooks.clone(),
            granted_mutations: plugin.granted_mutations.clone(),
            granted_minigames: plugin.granted_minigames.clone(),
            granted_reads: plugin.granted_reads.clone(),
            trusted: farm_plugins::is_project_script(&plugin.id),
        })
        .collect();
    let host = WasmPluginHost::new(specs.clone(), options);
    Some(Box::new(PluginRuntime::new(host, &specs)))
}

/// The sandbox for an editor project's enabled plugins; `None` when no enabled pack ships one.
pub fn for_project(project: &GameProject, options: PluginHostOptions) -> Option<Box<dyn SessionPlugins>> {
    PluginRuntime::from_project(project, options).map(|runtime| Box::new(runtime) as Box<dyn SessionPlugins>)
}
