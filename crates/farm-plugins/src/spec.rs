//! Plugin specs: what a host needs to load a plugin (port of `PluginSpec`,
//! `Plugins.PluginSpecsFromPacks` and `Plugins.PluginSpecsFromProject`).

use farm_sim::schema::{ContentPack, GameProject, PackPlugin};
use serde::{Deserialize, Serialize};

/// A plugin ready to load (TS `PluginSpec`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSpec {
    /// `packId:pluginId`, globally unique.
    pub id: String,
    /// The pack that ships the plugin.
    pub pack_id: String,
    /// The body of `function (api) { … }`.
    pub source: String,
    /// Hooks the plugin may receive: the plugin's `hooks` that the pack manifest's
    /// `permissions.hooks` also lists (the ones the player approved at install), in the
    /// plugin's order.
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
}

impl PluginSpec {
    /// Whether the plugin may receive `hook`.
    pub fn is_granted(&self, hook: &str) -> bool {
        self.granted_hooks.iter().any(|granted| granted == hook)
    }

    /// Whether the plugin defines the minigame `kind` (a full kind, `packId:name`).
    pub fn defines_minigame(&self, kind: &str) -> bool {
        self.granted_minigames.iter().any(|granted| granted == kind)
    }
}

/// Whether `name` can name a pack minigame kind: like a pack id, lowercase letters, digits and
/// dashes (`^[a-z0-9][a-z0-9-]*$`), at most 64 characters.
pub fn is_valid_minigame_name(name: &str) -> bool {
    name.len() <= 64 && farm_sim::schema::is_valid_pack_id(name)
}

/// The full minigame kinds (`packId:name`) a plugin of `pack` may define: the names in its own
/// `minigames` that the manifest's `permissions.minigames` also lists, in the plugin's order,
/// without duplicates. A name may be written as `packId:name`; names that are not valid
/// minigame names or that name another pack grant nothing.
pub fn granted_minigame_kinds(pack: &ContentPack, plugin: &PackPlugin) -> Vec<String> {
    let pack_id = &pack.manifest.id;
    let local = |name: &str| -> Option<String> {
        let name = name.strip_prefix(pack_id.as_str()).and_then(|rest| rest.strip_prefix(':')).unwrap_or(name);
        is_valid_minigame_name(name).then(|| name.to_owned())
    };
    let permitted: Vec<String> =
        pack.manifest.permissions.minigames.iter().flatten().filter_map(|name| local(name)).collect();
    let mut kinds: Vec<String> = Vec::new();
    for name in plugin.minigames.iter().flatten().filter_map(|name| local(name)) {
        let kind = format!("{pack_id}:{name}");
        if permitted.contains(&name) && !kinds.contains(&kind) {
            kinds.push(kind);
        }
    }
    kinds
}

/// Plugin specs, with capability grants applied, from packs in load order.
pub fn plugin_specs_from_packs<'a>(packs: impl IntoIterator<Item = &'a ContentPack>) -> Vec<PluginSpec> {
    let mut specs = Vec::new();
    for pack in packs {
        let permitted = &pack.manifest.permissions.hooks;
        for plugin in &pack.plugins {
            specs.push(PluginSpec {
                id: format!("{}:{}", pack.manifest.id, plugin.id),
                pack_id: pack.manifest.id.clone(),
                source: plugin.source.clone(),
                granted_hooks: plugin.hooks.iter().filter(|hook| permitted.contains(hook)).cloned().collect(),
                granted_mutations: pack.manifest.permissions.mutations.clone(),
                granted_minigames: granted_minigame_kinds(pack, plugin),
            });
        }
    }
    specs
}

/// Specs from a project's enabled installed packs, in load order.
pub fn plugin_specs_from_project(project: &GameProject) -> Vec<PluginSpec> {
    plugin_specs_from_packs(project.content_packs.iter().filter(|install| install.enabled).map(|install| &install.pack))
}
