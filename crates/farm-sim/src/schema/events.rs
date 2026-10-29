//! Port of `Events.cs` (packages/engine-schemas/src/events.ts).
//!
//! Event & trigger system (M3). Events belong to a scene (or '' = global), fire on a trigger
//! kind, gate on combinable conditions, and run sequenced outcomes. Fire-once semantics use an
//! auto-managed flag; repeatable is opt-in. Evaluation order is deterministic (content order).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

fn one() -> u32 {
    1
}

fn default_true() -> bool {
    true
}

/// TS `EventConditionSchema` — discriminated union on `type`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum EventCondition {
    /// Player entered a tile (or region when x2/y2 present).
    #[serde(rename = "enterTile")]
    EnterTile {
        #[serde(default, with = "crate::units::int")]
        x: i32,
        #[serde(default, with = "crate::units::int")]
        y: i32,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
        x2: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
        y2: Option<i32>,
    },
    /// Player interacted while facing a tile (or region).
    #[serde(rename = "interactTile")]
    InteractTile {
        #[serde(default, with = "crate::units::int")]
        x: i32,
        #[serde(default, with = "crate::units::int")]
        y: i32,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
        x2: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
        y2: Option<i32>,
    },
    #[serde(rename = "hasItem")]
    HasItem {
        #[serde(default)]
        item_id: String,
        #[serde(default = "one", with = "crate::units::count")]
        quantity: u32,
    },
    #[serde(rename = "inventorySpace")]
    InventorySpace {
        #[serde(default)]
        item_id: String,
        /// int, positive.
        #[serde(default = "one", with = "crate::units::count")]
        quantity: u32,
    },
    #[serde(rename = "flag")]
    Flag {
        #[serde(default)]
        flag: String,
        #[serde(default = "default_true")]
        value: bool,
    },
    #[serde(rename = "dayRange")]
    DayRange {
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
        min_day: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
        max_day: Option<u32>,
    },
    #[serde(rename = "season")]
    Season {
        #[serde(default)]
        seasons: Vec<String>,
    },
    #[serde(rename = "yearRange")]
    YearRange {
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
        min_year: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
        max_year: Option<u32>,
    },
    #[serde(rename = "timeOfDay")]
    TimeOfDay {
        #[serde(default, with = "crate::units::micro_minutes")]
        min_minute: u32,
        #[serde(default, with = "crate::units::micro_minutes")]
        max_minute: u32,
    },
    #[serde(rename = "questStatus")]
    QuestStatus {
        #[serde(default)]
        quest_id: String,
        /// One of [`super::quest_statuses`].
        #[serde(default)]
        status: String,
    },
    /// Friendship threshold with an NPC (M4 heart events).
    #[serde(rename = "friendship")]
    Friendship {
        #[serde(default)]
        npc_id: String,
        #[serde(default, with = "crate::units::int")]
        min: i32,
    },
    /// Current weather (M4).
    #[serde(rename = "weather")]
    Weather {
        #[serde(default)]
        weather_ids: Vec<String>,
    },
    /// Today is the named festival (M9 calendar).
    #[serde(rename = "festivalId")]
    FestivalId {
        #[serde(default)]
        festival_id: String,
    },
    /// A variable (a flag holding a number or text, M3) compared with a number (`value`) or,
    /// when `text` is set, with a text. `compare` is one of [`super::variable_comparisons`].
    #[serde(rename = "variable")]
    Variable {
        #[serde(default)]
        name: String,
        #[serde(default = "default_compare")]
        compare: String,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::units::signed_milli::opt")]
        value: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    /// Every condition of the group holds (M3).
    #[serde(rename = "all")]
    All {
        #[serde(default)]
        conditions: Vec<EventCondition>,
    },
    /// At least one condition of the group holds (M3).
    #[serde(rename = "any")]
    Any {
        #[serde(default)]
        conditions: Vec<EventCondition>,
    },
    /// None of the conditions of the group holds (M3).
    #[serde(rename = "not")]
    Not {
        #[serde(default)]
        conditions: Vec<EventCondition>,
    },
    /// A random chance, 0..1 (M3).
    #[serde(rename = "chance")]
    Chance {
        #[serde(default, with = "crate::units::probability")]
        chance: u64,
    },
    /// The player is in one of these scenes (M3).
    #[serde(rename = "scene")]
    Scene {
        #[serde(default)]
        scene_ids: Vec<String>,
    },
    /// Whole hearts with an NPC within a range (M4); an empty `npcId` is the NPC the player is
    /// talking to.
    #[serde(rename = "hearts")]
    Hearts {
        #[serde(default)]
        npc_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
        min: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
        max: Option<i32>,
    },
    /// The day of the season within a range (M4).
    #[serde(rename = "dayOfSeason")]
    DayOfSeason {
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
        min_day: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
        max_day: Option<u32>,
    },
    /// The player's relationship with an NPC (M4), one of [`super::relationship_statuses`]; an
    /// empty `npcId` is the NPC the player is talking to, else anyone.
    #[serde(rename = "relationship")]
    Relationship {
        #[serde(default)]
        npc_id: String,
        #[serde(default)]
        status: String,
    },
    /// The player has at least `min` money (M6).
    #[serde(rename = "money")]
    Money {
        #[serde(default, with = "crate::units::money")]
        min: i64,
    },
    /// The player's level in a skill is at least `min` (M6).
    #[serde(rename = "skillLevel")]
    SkillLevel {
        #[serde(default)]
        skill: String,
        #[serde(default = "one", with = "crate::units::count")]
        min: u32,
    },
    /// The deepest mine floor reached is at least `min` (M6).
    #[serde(rename = "mineFloor")]
    MineFloor {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mine_id: Option<String>,
        #[serde(default = "one", with = "crate::units::count")]
        min: u32,
    },
    /// A collection holds at least `min` items; absent `min`: all of its items (M6).
    #[serde(rename = "collection")]
    Collection {
        #[serde(default)]
        collection_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
        min: Option<u32>,
    },
    /// A bundle is complete (M6).
    #[serde(rename = "bundle")]
    Bundle {
        #[serde(default)]
        bundle_id: String,
    },
    /// An achievement is unlocked (M6).
    #[serde(rename = "achievement")]
    Achievement {
        #[serde(default)]
        achievement_id: String,
    },
    /// At least `min` units shipped of an item; an empty `itemId`: of everything (M6).
    #[serde(rename = "shipped")]
    Shipped {
        #[serde(default)]
        item_id: String,
        #[serde(default = "one", with = "crate::units::count")]
        min: u32,
    },
    /// At least `min` standing buildings of a kind, its upgrades counting (M5).
    #[serde(rename = "building")]
    Building {
        #[serde(default)]
        building_id: String,
        #[serde(default = "one", with = "crate::units::count")]
        min: u32,
    },
    /// At least `min` monsters slain of a kind; an empty `monsterId`: of any (M7).
    #[serde(rename = "slain")]
    Slain {
        #[serde(default)]
        monster_id: String,
        #[serde(default = "one", with = "crate::units::count")]
        min: u32,
    },
    /// Today is an NPC's birthday; an empty `npcId`: anyone's (M6).
    #[serde(rename = "birthday")]
    Birthday {
        #[serde(default)]
        npc_id: String,
    },
    #[serde(rename = "weekday")]
    Weekday {
        #[serde(default)]
        weekday_ids: Vec<String>,
    },
    #[serde(rename = "perk")]
    Perk {
        #[serde(default)]
        skill: String,
        #[serde(default)]
        perk_id: String,
        #[serde(default)]
        choice_id: String,
    },
    #[serde(rename = "buff")]
    Buff {
        #[serde(default)]
        id: String,
    },
}

fn default_compare() -> String {
    variable_comparisons::EQ.to_owned()
}

/// How a `variable` condition compares (M3).
pub mod variable_comparisons {
    pub const EQ: &str = "eq";
    pub const NE: &str = "ne";
    pub const LT: &str = "lt";
    pub const LE: &str = "le";
    pub const GT: &str = "gt";
    pub const GE: &str = "ge";

    pub const ALL: &[&str] = &[EQ, NE, LT, LE, GT, GE];
}

/// A relationship's stage (M4).
pub mod relationship_statuses {
    pub const SINGLE: &str = "single";
    pub const DATING: &str = "dating";
    pub const ENGAGED: &str = "engaged";
    pub const MARRIED: &str = "married";

    pub const ALL: &[&str] = &[SINGLE, DATING, ENGAGED, MARRIED];
}

impl EventCondition {
    /// The discriminator literal (TS `condition.type`).
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::EnterTile { .. } => "enterTile",
            Self::InteractTile { .. } => "interactTile",
            Self::HasItem { .. } => "hasItem",
            Self::InventorySpace { .. } => "inventorySpace",
            Self::Flag { .. } => "flag",
            Self::DayRange { .. } => "dayRange",
            Self::Season { .. } => "season",
            Self::YearRange { .. } => "yearRange",
            Self::TimeOfDay { .. } => "timeOfDay",
            Self::QuestStatus { .. } => "questStatus",
            Self::Friendship { .. } => "friendship",
            Self::Weather { .. } => "weather",
            Self::FestivalId { .. } => "festivalId",
            Self::Weekday { .. } => "weekday",
            Self::Variable { .. } => "variable",
            Self::All { .. } => "all",
            Self::Any { .. } => "any",
            Self::Not { .. } => "not",
            Self::Chance { .. } => "chance",
            Self::Scene { .. } => "scene",
            Self::Hearts { .. } => "hearts",
            Self::DayOfSeason { .. } => "dayOfSeason",
            Self::Relationship { .. } => "relationship",
            Self::Money { .. } => "money",
            Self::Perk { .. } => "perk",
            Self::Buff { .. } => "buff",
            Self::SkillLevel { .. } => "skillLevel",
            Self::MineFloor { .. } => "mineFloor",
            Self::Collection { .. } => "collection",
            Self::Bundle { .. } => "bundle",
            Self::Achievement { .. } => "achievement",
            Self::Shipped { .. } => "shipped",
            Self::Building { .. } => "building",
            Self::Slain { .. } => "slain",
            Self::Birthday { .. } => "birthday",
        }
    }
}

/// TS `EventOutcomeSchema.type` enum.
pub mod event_outcome_types {
    pub const MESSAGE: &str = "message";
    pub const MODIFY_FRIENDSHIP: &str = "modifyFriendship";
    pub const MODIFY_ENERGY: &str = "modifyEnergy";
    pub const WATER_AREA: &str = "waterArea";
    pub const GIVE_ITEM: &str = "giveItem";
    pub const TAKE_ITEM: &str = "takeItem";
    pub const GIVE_MONEY: &str = "giveMoney";
    pub const TAKE_MONEY: &str = "takeMoney";
    pub const SET_FLAG: &str = "setFlag";
    pub const CLEAR_FLAG: &str = "clearFlag";
    pub const START_QUEST: &str = "startQuest";
    pub const COMPLETE_QUEST: &str = "completeQuest";
    pub const SPAWN_NPC: &str = "spawnNPC";
    pub const REMOVE_NPC: &str = "removeNPC";
    pub const CHANGE_TILE: &str = "changeTile";
    pub const WARP_PLAYER: &str = "warpPlayer";
    pub const START_DIALOGUE: &str = "startDialogue";
    pub const LOCK_TRANSITION: &str = "lockTransition";
    pub const UNLOCK_TRANSITION: &str = "unlockTransition";
    pub const PLAY_SOUND: &str = "playSound";
    /// Run a creator-defined action (extensibility layer).
    pub const PERFORM_ACTION: &str = "performAction";
    /// Open a declared minigame; its score resolves via the command log.
    pub const START_MINIGAME: &str = "startMinigame";
    /// Play a music asset until the scene changes (`soundId`; empty: the game's music again).
    pub const PLAY_MUSIC: &str = "playMusic";
    /// Play a cutscene (`cutsceneId`, M2).
    pub const START_CUTSCENE: &str = "startCutscene";
    /// Set a variable to a number (`amount`) or a text (`text`, M3).
    pub const SET_VARIABLE: &str = "setVariable";
    /// Add `amount` to a number variable (M3).
    pub const ADD_VARIABLE: &str = "addVariable";
    /// Walk an NPC to a tile of its scene, or put it on a tile of another (M3).
    pub const MOVE_NPC: &str = "moveNpc";
    /// Change today's weather (M3).
    pub const SET_WEATHER: &str = "setWeather";
    /// Give `amount` experience in a skill (M3).
    pub const GIVE_EXPERIENCE: &str = "giveExperience";
    /// Add an item to a shop (M3): `amount` is its price (absent: the item's), `itemQuantity` a daily limit.
    pub const ADD_SHOP_STOCK: &str = "addShopStock";
    /// Stop a shop selling an item (M3).
    pub const REMOVE_SHOP_STOCK: &str = "removeShopStock";
    /// Put a gathering node on a tile (M3).
    pub const PLACE_NODE: &str = "placeNode";
    /// Clear the node on a tile (M3).
    pub const REMOVE_NODE: &str = "removeNode";
    /// Put a machine on a tile (M3).
    pub const PLACE_MACHINE: &str = "placeMachine";
    /// Clear the machine on a tile (M3).
    pub const REMOVE_MACHINE: &str = "removeMachine";
    /// Change the player's relationship with an NPC (`status`, M4).
    pub const SET_RELATIONSHIP: &str = "setRelationship";
    /// Send a letter (`mailId`, M6): it arrives the next morning.
    pub const SEND_MAIL: &str = "sendMail";
    /// Restore (or, negative, take) `amount` health (M7).
    pub const SET_INVENTORY_SIZE: &str = "setInventorySize";
    pub const ADD_INVENTORY_SIZE: &str = "addInventorySize";
    pub const ADD_MAX_ENERGY: &str = "addMaxEnergy";
    pub const ADD_MAX_HEALTH: &str = "addMaxHealth";
    pub const APPLY_BUFF: &str = "applyBuff";
    pub const MODIFY_HEALTH: &str = "modifyHealth";

    pub const ALL: &[&str] = &[
        MESSAGE,
        MODIFY_FRIENDSHIP,
        MODIFY_ENERGY,
        WATER_AREA,
        GIVE_ITEM,
        TAKE_ITEM,
        GIVE_MONEY,
        TAKE_MONEY,
        SET_FLAG,
        CLEAR_FLAG,
        START_QUEST,
        COMPLETE_QUEST,
        SPAWN_NPC,
        REMOVE_NPC,
        CHANGE_TILE,
        WARP_PLAYER,
        START_DIALOGUE,
        LOCK_TRANSITION,
        UNLOCK_TRANSITION,
        PLAY_SOUND,
        PERFORM_ACTION,
        START_MINIGAME,
        PLAY_MUSIC,
        START_CUTSCENE,
        SET_VARIABLE,
        ADD_VARIABLE,
        MOVE_NPC,
        SET_WEATHER,
        GIVE_EXPERIENCE,
        ADD_SHOP_STOCK,
        REMOVE_SHOP_STOCK,
        PLACE_NODE,
        REMOVE_NODE,
        PLACE_MACHINE,
        REMOVE_MACHINE,
        SET_RELATIONSHIP,
        SEND_MAIL,
        MODIFY_HEALTH,
        SET_INVENTORY_SIZE,
        ADD_INVENTORY_SIZE,
        ADD_MAX_ENERGY,
        ADD_MAX_HEALTH,
        APPLY_BUFF,
    ];
}

/// The outcome types as an enum: [`event_outcome_types`] typed, so the engine dispatches on a
/// variant instead of comparing strings. The authored `type` stays a string in
/// [`EventOutcome`] (an unknown type round-trips and is reported by the authoring checks, then
/// does nothing at run time); [`EventOutcome::kind`] parses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeKind {
    Message,
    ModifyFriendship,
    ModifyEnergy,
    WaterArea,
    GiveItem,
    TakeItem,
    GiveMoney,
    TakeMoney,
    SetFlag,
    ClearFlag,
    StartQuest,
    CompleteQuest,
    SpawnNpc,
    RemoveNpc,
    ChangeTile,
    WarpPlayer,
    StartDialogue,
    LockTransition,
    UnlockTransition,
    PlaySound,
    PerformAction,
    StartMinigame,
    PlayMusic,
    StartCutscene,
    SetVariable,
    AddVariable,
    MoveNpc,
    SetWeather,
    GiveExperience,
    AddShopStock,
    RemoveShopStock,
    PlaceNode,
    RemoveNode,
    PlaceMachine,
    RemoveMachine,
    SetRelationship,
    SendMail,
    ModifyHealth,
    SetInventorySize,
    AddInventorySize,
    AddMaxEnergy,
    AddMaxHealth,
    ApplyBuff,
}

impl OutcomeKind {
    /// Every kind, in [`event_outcome_types::ALL`] order.
    pub const ALL: [OutcomeKind; 43] = [
        Self::Message,
        Self::ModifyFriendship,
        Self::ModifyEnergy,
        Self::WaterArea,
        Self::GiveItem,
        Self::TakeItem,
        Self::GiveMoney,
        Self::TakeMoney,
        Self::SetFlag,
        Self::ClearFlag,
        Self::StartQuest,
        Self::CompleteQuest,
        Self::SpawnNpc,
        Self::RemoveNpc,
        Self::ChangeTile,
        Self::WarpPlayer,
        Self::StartDialogue,
        Self::LockTransition,
        Self::UnlockTransition,
        Self::PlaySound,
        Self::PerformAction,
        Self::StartMinigame,
        Self::PlayMusic,
        Self::StartCutscene,
        Self::SetVariable,
        Self::AddVariable,
        Self::MoveNpc,
        Self::SetWeather,
        Self::GiveExperience,
        Self::AddShopStock,
        Self::RemoveShopStock,
        Self::PlaceNode,
        Self::RemoveNode,
        Self::PlaceMachine,
        Self::RemoveMachine,
        Self::SetRelationship,
        Self::SendMail,
        Self::ModifyHealth,
        Self::SetInventorySize,
        Self::AddInventorySize,
        Self::AddMaxEnergy,
        Self::AddMaxHealth,
        Self::ApplyBuff,
    ];

    /// The authored `type` literal (one of [`event_outcome_types`]).
    pub fn name(self) -> &'static str {
        use event_outcome_types as t;
        match self {
            Self::Message => t::MESSAGE,
            Self::ModifyFriendship => t::MODIFY_FRIENDSHIP,
            Self::ModifyEnergy => t::MODIFY_ENERGY,
            Self::WaterArea => t::WATER_AREA,
            Self::GiveItem => t::GIVE_ITEM,
            Self::TakeItem => t::TAKE_ITEM,
            Self::GiveMoney => t::GIVE_MONEY,
            Self::TakeMoney => t::TAKE_MONEY,
            Self::SetFlag => t::SET_FLAG,
            Self::ClearFlag => t::CLEAR_FLAG,
            Self::StartQuest => t::START_QUEST,
            Self::CompleteQuest => t::COMPLETE_QUEST,
            Self::SpawnNpc => t::SPAWN_NPC,
            Self::RemoveNpc => t::REMOVE_NPC,
            Self::ChangeTile => t::CHANGE_TILE,
            Self::WarpPlayer => t::WARP_PLAYER,
            Self::StartDialogue => t::START_DIALOGUE,
            Self::LockTransition => t::LOCK_TRANSITION,
            Self::UnlockTransition => t::UNLOCK_TRANSITION,
            Self::PlaySound => t::PLAY_SOUND,
            Self::PerformAction => t::PERFORM_ACTION,
            Self::StartMinigame => t::START_MINIGAME,
            Self::PlayMusic => t::PLAY_MUSIC,
            Self::StartCutscene => t::START_CUTSCENE,
            Self::SetVariable => t::SET_VARIABLE,
            Self::AddVariable => t::ADD_VARIABLE,
            Self::MoveNpc => t::MOVE_NPC,
            Self::SetWeather => t::SET_WEATHER,
            Self::GiveExperience => t::GIVE_EXPERIENCE,
            Self::AddShopStock => t::ADD_SHOP_STOCK,
            Self::RemoveShopStock => t::REMOVE_SHOP_STOCK,
            Self::PlaceNode => t::PLACE_NODE,
            Self::RemoveNode => t::REMOVE_NODE,
            Self::PlaceMachine => t::PLACE_MACHINE,
            Self::RemoveMachine => t::REMOVE_MACHINE,
            Self::SetRelationship => t::SET_RELATIONSHIP,
            Self::SendMail => t::SEND_MAIL,
            Self::SetInventorySize => t::SET_INVENTORY_SIZE,
            Self::AddInventorySize => t::ADD_INVENTORY_SIZE,
            Self::AddMaxEnergy => t::ADD_MAX_ENERGY,
            Self::AddMaxHealth => t::ADD_MAX_HEALTH,
            Self::ApplyBuff => t::APPLY_BUFF,
            Self::ModifyHealth => t::MODIFY_HEALTH,
        }
    }

    /// The kind an authored `type` names; `None` for an unknown type (case-sensitive, like the
    /// schema).
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }
}

/// A single event/action outcome. Not a discriminated union: one flat object whose `type`
/// selects which optional fields apply.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EventOutcome {
    /// One of [`event_outcome_types`].
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub item_quantity: Option<u32>,
    /// finite.
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::signed_milli::opt")]
    pub amount: Option<i64>,
    /// int, 0..10.
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::count::opt")]
    pub radius: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flag_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quest_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npc_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dialogue_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub tile_x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub tile_y: Option<i32>,
    /// One of [`super::tile_types`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_tile_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::int::opt")]
    pub y: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minigame_id: Option<String>,
    /// The cutscene `startCutscene` plays (M2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cutscene_id: Option<String>,
    /// The variable `setVariable` / `addVariable` change (M3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variable: Option<String>,
    /// The text `setVariable` stores (M3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The weather `setWeather` makes today's (M3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weather_id: Option<String>,
    /// The skill `giveExperience` trains (M3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
    /// The shop `addShopStock` / `removeShopStock` change (M3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shop_id: Option<String>,
    /// The node type `placeNode` puts down (M3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_type_id: Option<String>,
    /// The machine type `placeMachine` puts down (M3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine_type_id: Option<String>,
    /// The relationship `setRelationship` sets (M4), one of [`super::relationship_statuses`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// The letter `sendMail` sends (M6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mail_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buff: Option<super::BuffDefinition>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl EventOutcome {
    /// An outcome of `kind` with no fields set (the plugin mutations that share the outcome
    /// executor build theirs this way).
    pub fn of(kind: OutcomeKind) -> Self {
        Self { r#type: kind.name().to_owned(), ..Self::default() }
    }

    /// The typed [`OutcomeKind`] of `type`; `None` when the type is unknown.
    pub fn kind(&self) -> Option<OutcomeKind> {
        OutcomeKind::parse(&self.r#type)
    }
}

/// TS `EventTriggerSchema`, and the triggers M3 added.
pub mod event_triggers {
    pub const ENTER: &str = "enter";
    pub const INTERACT: &str = "interact";
    pub const TICK: &str = "tick";
    /// Each morning, after the night (M3).
    pub const DAY_START: &str = "dayStart";
    /// When the clock reaches the event's `minute` (M3).
    pub const TIME: &str = "time";
    /// A crop was harvested (subject: the crop type or the item, M3).
    pub const HARVEST: &str = "harvest";
    /// An item was sold (subject: the item, M3).
    pub const SELL: &str = "sell";
    /// A recipe was crafted (subject: the recipe or an item it made, M3).
    pub const CRAFT: &str = "craft";
    /// A fish was caught (subject: the item, M3).
    pub const CATCH: &str = "catch";
    /// An item was picked up from the ground (subject: the item, M3).
    pub const PICK_UP: &str = "pickUp";
    /// Friendship with an NPC changed (subject: the NPC, M3).
    pub const FRIENDSHIP: &str = "friendship";
    /// A quest started or was completed (subject: the quest, M3).
    pub const QUEST: &str = "quest";
    /// A building was finished or upgraded (subject: the building kind, M5).
    pub const BUILD: &str = "build";
    /// A bundle was completed (subject: the bundle, M6).
    pub const BUNDLE: &str = "bundle";
    /// An achievement was unlocked (subject: the achievement, M6).
    pub const ACHIEVEMENT: &str = "achievement";
    /// A monster was slain (subject: the monster kind, M7).
    pub const SLAY: &str = "slay";

    pub const ALL: &[&str] = &[
        ENTER,
        INTERACT,
        TICK,
        DAY_START,
        TIME,
        HARVEST,
        SELL,
        CRAFT,
        CATCH,
        PICK_UP,
        FRIENDSHIP,
        QUEST,
        BUILD,
        BUNDLE,
        ACHIEVEMENT,
        SLAY,
    ];
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GameEvent {
    pub id: String,
    pub name: String,
    /// Scene the event lives in; empty string = evaluated in every scene.
    pub scene_id: String,
    /// One of [`event_triggers`].
    pub trigger: String,
    pub conditions: Vec<EventCondition>,
    pub outcomes: Vec<EventOutcome>,
    pub active: bool,
    pub repeatable: bool,
    /// What the trigger is about (M3): an item (harvest, sell, craft, catch, pickUp), an NPC
    /// (friendship) or a quest (quest). Absent or empty: anything.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// The time of day a `time` trigger fires at (M3).
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::units::micro_minutes::opt")]
    pub minute: Option<u32>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Flag used to record that a non-repeatable event has fired.
pub fn event_fired_flag(event_id: &str) -> String {
    format!("event:{event_id}:fired")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_kinds_cover_every_outcome_type_in_order() {
        let names: Vec<&str> = OutcomeKind::ALL.iter().map(|kind| kind.name()).collect();
        assert_eq!(names, event_outcome_types::ALL);
        for kind in OutcomeKind::ALL {
            assert_eq!(OutcomeKind::parse(kind.name()), Some(kind));
            assert_eq!(EventOutcome::of(kind).kind(), Some(kind));
        }
        assert_eq!(OutcomeKind::parse("givItem"), None);
        assert_eq!(OutcomeKind::parse("GiveItem"), None);
        assert_eq!(EventOutcome::default().kind(), None);
    }
}
