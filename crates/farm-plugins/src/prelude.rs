//! The plugin API beyond `api.on`: JavaScript ([`PRELUDE`]) that the guest applies to every
//! plugin's `api` before its source runs, giving it a frozen object with the whole API
//! (docs/PLUGINS.md).
//!
//! | Function | Hook it registers | Notes |
//! |---|---|---|
//! | `api.on(hook, fn)` | `hook` | The guest's own. |
//! | `api.state.*` | none | Read API (#237): each call is one `api.host({ read: … })`. |
//! | `api.minigame(name, game)` | `minigame:<name>` | [`crate::minigame`]. |
//! | `api.condition(name, fn)` | `condition:<pack>:<name>` | `fn(args, state, context)`; the answer is `[bool]` (#239). |
//! | `api.outcome(name, fn)` | `outcome:<pack>:<name>` | `fn(args, state)` answers mutations (#239). |
//! | `api.entity(name, handlers)` | `entity:<pack>:<name>` | `onTick`, `onInteract`, `onDayStart` (#238). |
//! | `api.itemUse(name, fn)` | `use:<pack>:<name>` | `fn(use, state)` answers mutations (#238). |
//! | `api.timer(name, fn)` | `timer` | One hook for all of a plugin's timers; the prelude picks `fn` by the timer's name (`'*'`: any) (#238). |
//! | `api.screen(name, screen)` | `screen:<pack>:<name>` | `{ title, hotkey }` describe it; `draw(state)` answers a widget tree, `action(action, state)` mutations (#240). |
//!
//! The guest's `api.on` ignores hooks the instance is not granted, so each of these registers
//! only what the pack's permissions allow. `<pack>` is the plugin's pack id; a project script
//! (#241) has none, and its names are used as written. Everything is checked on the host side,
//! so the prelude is a convenience, not a boundary.

/// The prelude: a JavaScript function `extend(base, pack)` that the guest's bootstrap
/// evaluates once when the sandbox is prepared and applies to every plugin's `api`, so starting
/// a plugin does not pay for compiling it (`crates/farm-plugin-guest/src/prelude.js`).
pub const PRELUDE: &str = include_str!("../../farm-plugin-guest/src/prelude.js");

/// The granted-hooks JSON a plugin's instance starts with (guest ABI 2): its hooks and its pack,
/// which names its systems (`entity:<pack>:<name>`, …).
pub(crate) fn granted_json(hooks: &[&str], pack_id: &str, script: bool) -> String {
    serde_json::json!({ "hooks": hooks, "pack": pack_id, "script": script }).to_string()
}
