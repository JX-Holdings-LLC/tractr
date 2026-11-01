//! Plugin specs: what a host needs to load a plugin (port of `PluginSpec`,
//! `Plugins.PluginSpecsFromPacks` and `Plugins.PluginSpecsFromProject`).

use farm_sim::hooks::extension_hooks;
use farm_sim::schema::{ContentPack, GameProject, PackPlugin};
use serde::{Deserialize, Serialize};

/// A plugin ready to load (TS `PluginSpec`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSpec {
    /// `packId:pluginId`, globally unique (`script/<id>` for a project script).
    pub id: String,
    /// The pack that ships the plugin (empty for a project script).
    pub pack_id: String,
    /// The body of `function (api) { … }`.
    pub source: String,
    /// Hooks the plugin may receive: the plugin's `hooks` that the pack manifest's
    /// `permissions.hooks` also lists (the ones the player approved at install), in the
    /// plugin's order, then the hooks of its plugin systems ([`granted_extension_hooks`]).
    pub granted_hooks: Vec<String>,
    /// The pack manifest's `permissions.mutations`: what the plugin's answers may do (see
    /// [`MutationGrants`](crate::MutationGrants)). `None` when the manifest declares none: the
    /// defaults apply and answers to `onEffect` / `onCommand` are dropped.
    #[serde(default)]
    pub granted_mutations: Option<Vec<String>>,
    /// The minigame kinds the plugin defines that the pack manifest's `permissions.minigames`
    /// also lists, as full kinds (`packId:name`), in the plugin's order (see
    /// [`granted_minigame_kinds`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub granted_minigames: Vec<String>,
    /// The pack manifest's `permissions.read` (#237): what the plugin may read of the game
    /// ([`crate::ReadGrants`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub granted_reads: Vec<String>,
    /// A creator's own project script (#241): every hook, mutation and read, ids as written,
    /// and an overrun is an error instead of a strike.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub trusted: bool,
}

impl PluginSpec {
    /// Whether the plugin may receive `hook`. Every plugin may hear `timer`: only the plugin
    /// that started a timer hears it, and starting one needs the `startTimer` capability.
    pub fn is_granted(&self, hook: &str) -> bool {
        self.trusted || hook == extension_hooks::TIMER || self.granted_hooks.iter().any(|granted| granted == hook)
    }

    /// Whether the plugin defines the minigame `kind` (a full kind, `packId:name`).
    pub fn defines_minigame(&self, kind: &str) -> bool {
        self.granted_minigames.iter().any(|granted| granted == kind)
    }

    /// The granted-hooks JSON the guest lets the plugin register with: its hooks (and `timer`),
    /// or `*` for a trusted script, with its pack.
    pub(crate) fn granted_json(&self) -> String {
        if self.trusted {
            // A trusted script may register any hook (`"*"`, guest ABI 2).
            return crate::prelude::granted_json(&["*"], &self.pack_id, true);
        }
        let mut hooks: Vec<&str> = self.granted_hooks.iter().map(String::as_str).collect();
        hooks.push(extension_hooks::TIMER);
        crate::prelude::granted_json(&hooks, &self.pack_id, false)
    }
}

/// Whether `name` can name a pack minigame kind: like a pack id, lowercase letters, digits and
/// dashes (`^[a-z0-9][a-z0-9-]*$`), at most 64 characters.
pub fn is_valid_minigame_name(name: &str) -> bool {
    name.len() <= 64 && farm_sim::schema::is_valid_pack_id(name)
}

/// The valid local names of `names` for pack `pack_id`: `packId:name` and `name` both mean
/// `name`; names that are not valid (see [`is_valid_minigame_name`]) or name another pack are
/// left out.
pub fn local_names(pack_id: &str, names: Option<&[String]>) -> Vec<String> {
    names
        .unwrap_or_default()
        .iter()
        .filter_map(|name| {
            let name = name.strip_prefix(pack_id).and_then(|rest| rest.strip_prefix(':')).unwrap_or(name);
            is_valid_minigame_name(name).then(|| name.to_owned())
        })
        .collect()
}

/// The names in `wanted` that `permitted` also lists, in `wanted`'s order, without duplicates,
/// as full names (`packId:name`).
fn granted(pack_id: &str, wanted: Option<&[String]>, permitted: Option<&[String]>) -> Vec<String> {
    let permitted = local_names(pack_id, permitted);
    let mut out: Vec<String> = Vec::new();
    for name in local_names(pack_id, wanted) {
        let full = format!("{pack_id}:{name}");
        if permitted.contains(&name) && !out.contains(&full) {
            out.push(full);
        }
    }
    out
}

/// The full minigame kinds (`packId:name`) a plugin of `pack` may define: the names in its own
/// `minigames` that the manifest's `permissions.minigames` also lists, in the plugin's order,
/// without duplicates. A name may be written as `packId:name`; names that are not valid
/// minigame names or that name another pack grant nothing.
pub fn granted_minigame_kinds(pack: &ContentPack, plugin: &PackPlugin) -> Vec<String> {
    granted(&pack.manifest.id, plugin.minigames.as_deref(), pack.manifest.permissions.minigames.as_deref())
}

/// The hooks of a plugin's own systems, granted without being listed in `hooks`:
///
/// - `condition:packId:name` and `outcome:packId:name` for the conditions and outcomes it
///   declares (#239);
/// - `entity:packId:name` for its entity types that `permissions.entities` lists, and
///   `use:packId:name` for its item uses that `permissions.itemUses` lists (#238);
/// - `screen:packId:name` for its screens that `permissions.screens` lists (#240).
///
/// `timer` is not listed: every plugin may register a timer handler and hears only its own
/// timers, which it can start only with the `startTimer` capability (see
/// [`PluginSpec::is_granted`]).
pub fn granted_extension_hooks(pack: &ContentPack, plugin: &PackPlugin) -> Vec<String> {
    let pack_id = &pack.manifest.id;
    let permissions = &pack.manifest.permissions;
    let mut hooks: Vec<String> = Vec::new();
    let mut push = |hook: String| {
        if !hooks.contains(&hook) {
            hooks.push(hook);
        }
    };
    let functions = |list: &Option<Vec<farm_sim::schema::PluginFunction>>| -> Vec<String> {
        list.iter().flatten().map(|function| function.name.clone()).collect()
    };
    for name in local_names(pack_id, Some(&functions(&plugin.conditions))) {
        push(format!("{}{pack_id}:{name}", extension_hooks::CONDITION));
    }
    for name in local_names(pack_id, Some(&functions(&plugin.outcomes))) {
        push(format!("{}{pack_id}:{name}", extension_hooks::OUTCOME));
    }
    for full in granted(pack_id, plugin.entities.as_deref(), permissions.entities.as_deref()) {
        push(format!("{}{full}", extension_hooks::ENTITY));
    }
    for full in granted(pack_id, plugin.item_uses.as_deref(), permissions.item_uses.as_deref()) {
        push(format!("{}{full}", extension_hooks::ITEM_USE));
    }
    let screens: Vec<String> = plugin.screens.iter().flatten().map(|screen| screen.name.clone()).collect();
    for full in granted(pack_id, Some(&screens), permissions.screens.as_deref()) {
        push(format!("{}{full}", crate::SCREEN_HOOK_PREFIX));
    }
    hooks
}

/// The spec of `plugin` of `pack`, with its grants applied.
pub fn plugin_spec(pack: &ContentPack, plugin: &PackPlugin) -> PluginSpec {
    let permitted = &pack.manifest.permissions.hooks;
    let mut granted_hooks: Vec<String> = plugin.hooks.iter().filter(|hook| permitted.contains(hook)).cloned().collect();
    for hook in granted_extension_hooks(pack, plugin) {
        if !granted_hooks.contains(&hook) {
            granted_hooks.push(hook);
        }
    }
    PluginSpec {
        id: format!("{}:{}", pack.manifest.id, plugin.id),
        pack_id: pack.manifest.id.clone(),
        source: plugin.source.clone(),
        granted_hooks,
        granted_mutations: pack.manifest.permissions.mutations.clone(),
        granted_minigames: granted_minigame_kinds(pack, plugin),
        granted_reads: pack.manifest.permissions.read.clone().unwrap_or_default(),
        trusted: false,
    }
}

/// Plugin specs, with capability grants applied, from packs in load order.
pub fn plugin_specs_from_packs<'a>(packs: impl IntoIterator<Item = &'a ContentPack>) -> Vec<PluginSpec> {
    let mut specs = Vec::new();
    for pack in packs {
        for plugin in &pack.plugins {
            specs.push(plugin_spec(pack, plugin));
        }
    }
    specs
}

/// Specs from a project: its own scripts first (#241, trusted), then its enabled installed
/// packs' plugins, in load order.
pub fn plugin_specs_from_project(project: &GameProject) -> Vec<PluginSpec> {
    let mut specs = crate::scripts::project_script_specs(project);
    specs.extend(plugin_specs_from_packs(
        project.content_packs.iter().filter(|install| install.enabled).map(|install| &install.pack),
    ));
    specs
}
