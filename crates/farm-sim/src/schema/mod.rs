//! The data shapes (port of `src/FarmEngine.Schemas`, itself a port of
//! `packages/engine-schemas`). One C# file → one module here, everything re-exported flat.
//!
//! Serialization rules: simulation quantities are integers in fixed units, read from and written
//! to JSON in authoring units through the `crate::units` serde adapters (docs/NUMERICS.md; only
//! screen-facing fields stay `f64`), JSON names are camelCase, optional fields are omitted when
//! `None`, present-as-null fields (zod `.nullable()`) are plain `Option`s that serialize `null`,
//! property initializers are the struct's `Default` (applied when a key is missing), and zod
//! `.passthrough()` records carry unknown keys in a flattened `extra` map.

pub mod actors;
pub mod animals;
pub mod combat;
pub mod content;
pub mod crafting;
pub mod cutscenes;
pub mod economy;
pub mod events;
pub mod extensibility;
pub mod farm;
pub mod fishing;
pub mod game_content;
pub mod goals;
pub mod graphics;
pub mod interface;
pub mod mining;
pub mod nodes;
pub mod nullable;
pub mod packs;
pub mod primitives;
pub mod project;
pub mod quests;
pub mod relationships;
pub mod save;
pub mod settings;
pub mod social;
pub mod vocabulary;
pub mod weather;
pub mod world;

pub use actors::*;
pub use animals::*;
pub use combat::*;
pub use content::*;
pub use crafting::*;
pub use cutscenes::*;
pub use economy::*;
pub use events::*;
pub use extensibility::*;
pub use farm::*;
pub use fishing::*;
pub use game_content::*;
pub use goals::*;
pub use graphics::*;
pub use interface::*;
pub use mining::*;
pub use nodes::*;
pub use packs::*;
pub use primitives::*;
pub use project::*;
pub use quests::*;
pub use relationships::*;
pub use save::*;
pub use settings::*;
pub use social::*;
pub use vocabulary::*;
pub use weather::*;
pub use world::*;

/// `skip_serializing_if` for a flag that is only written when set.
pub(crate) fn is_false(value: &bool) -> bool {
    !*value
}

/// `skip_serializing_if` for a count that is only written when not zero.
pub(crate) fn is_zero(value: &u32) -> bool {
    *value == 0
}

/// `skip_serializing_if` for a whole number that is only written when not zero.
pub(crate) fn is_zero_i32(value: &i32) -> bool {
    *value == 0
}
