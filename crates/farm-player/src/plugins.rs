//! The plugin sandbox (`farm-plugins`) attached to a [`PlaySession`](crate::session::PlaySession).

use crate::session::{describe_plugin_error, SessionPlugins};
use farm_cart::CartPlugin;
use farm_plugins::{PackMinigame, PluginError, PluginHostOptions, PluginRuntime, PluginSpec, WasmPluginHost};
use farm_sim::schema::GameProject;
use farm_sim::{Command, HookEvent};
use serde_json::Value;

impl SessionPlugins for PluginRuntime<WasmPluginHost> {
    fn dispatch(&mut self, events: &[HookEvent], depth: u32) {
        // Errors are kept in the runtime's bounded recent-error list.
        self.dispatch_events_at(events, depth);
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

/// The sandbox for a cartridge's plugins; `None` when the game ships none.
pub fn for_cartridge(plugins: &[CartPlugin], options: PluginHostOptions) -> Option<Box<dyn SessionPlugins>> {
    if plugins.is_empty() {
        return None;
    }
    let specs: Vec<PluginSpec> = plugins
        .iter()
        .map(|plugin| PluginSpec {
            id: plugin.id.clone(),
            pack_id: plugin.pack_id.clone(),
            source: plugin.source.clone(),
            granted_hooks: plugin.granted_hooks.clone(),
            granted_mutations: plugin.granted_mutations.clone(),
            granted_minigames: plugin.granted_minigames.clone(),
        })
        .collect();
    let host = WasmPluginHost::new(specs.clone(), options);
    Some(Box::new(PluginRuntime::new(host, &specs)))
}

/// The sandbox for an editor project's enabled plugins; `None` when no enabled pack ships one.
pub fn for_project(project: &GameProject, options: PluginHostOptions) -> Option<Box<dyn SessionPlugins>> {
    PluginRuntime::from_project(project, options).map(|runtime| Box::new(runtime) as Box<dyn SessionPlugins>)
}
