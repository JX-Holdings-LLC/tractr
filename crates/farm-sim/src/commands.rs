//! Commands are the ONLY way player intent mutates game state (port of `Commands.cs` /
//! commands.ts). Determinism contract: same seed + same command log ⇒ same state. JSON shape is
//! identical to the TS union (`{"type":"move","dir":"up"}`).

use crate::schema::{PlayerLook, PluginMutation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum Command {
    /// Free movement (v4): set the held movement intent (each axis −1/0/1).
    #[serde(rename = "setMoveIntent")]
    SetMoveIntent {
        #[serde(with = "crate::units::truncated")]
        dx: i32,
        #[serde(with = "crate::units::truncated")]
        dy: i32,
    },
    /// Discrete one-tile step — scripted movement / legacy primitive.
    #[serde(rename = "move")]
    Move { dir: String },
    #[serde(rename = "useTool")]
    UseTool { tool: String },
    #[serde(rename = "interact")]
    Interact,
    /// `interact` with the player's planting choice: the named seed (else the first seed held
    /// that grows this season), and a fertilizer only when one is named. Anywhere but open soil
    /// it is a plain `interact`. Plain `interact` plants the first seed held and uses the first
    /// fertilizer held (the rule recorded games were played with).
    #[serde(rename = "interactWith")]
    InteractWith {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seed_item_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fertilizer_item_id: Option<String>,
    },
    #[serde(rename = "chooseDialogueOption")]
    ChooseDialogueOption {
        #[serde(with = "crate::units::index")]
        index: i32,
    },
    #[serde(rename = "closeDialogue")]
    CloseDialogue,
    #[serde(rename = "sleep")]
    Sleep,
    #[serde(rename = "openShop")]
    OpenShop { shop_id: String },
    #[serde(rename = "closeShop")]
    CloseShop,
    #[serde(rename = "buyItem")]
    BuyItem {
        item_id: String,
        #[serde(with = "crate::units::count")]
        quantity: u32,
    },
    #[serde(rename = "sellItem")]
    SellItem {
        item_id: String,
        #[serde(with = "crate::units::count")]
        quantity: u32,
        /// The crop quality to sell (one of `crop_qualities`, `normal` included). Absent: any
        /// quality, lowest first, each unit at its own quality's price.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        quality: Option<String>,
    },
    #[serde(rename = "repairTool")]
    RepairTool { item_id: String },
    #[serde(rename = "craft")]
    Craft { recipe_id: String },
    #[serde(rename = "placeMachine")]
    PlaceMachine { machine_type_id: String },
    #[serde(rename = "machineLoad")]
    MachineLoad { recipe_id: String },
    /// Pick the idle machine on the facing tile back up (its item returns to the inventory).
    #[serde(rename = "pickUpMachine")]
    PickUpMachine,
    #[serde(rename = "giveGift")]
    GiveGift { item_id: String },
    #[serde(rename = "descendMine")]
    DescendMine {
        #[serde(with = "crate::units::count")]
        floor: u32,
    },
    #[serde(rename = "exitMine")]
    ExitMine,
    /// Run a creator-defined action (extensibility layer).
    #[serde(rename = "performAction")]
    PerformAction { action_id: String },
    /// Use an inventory item (runs its bound action).
    #[serde(rename = "useItem")]
    UseItem { item_id: String },
    #[serde(rename = "startMinigame")]
    StartMinigame { minigame_id: String },
    /// Resolve the open minigame with a score in [0, 1].
    #[serde(rename = "resolveMinigame")]
    ResolveMinigame {
        #[serde(with = "crate::units::probability")]
        score: u64,
    },
    #[serde(rename = "cancelMinigame")]
    CancelMinigame,
    /// Past the cutscene line shown (a `say` step, or a `choice` without answers), M2.
    #[serde(rename = "advanceCutscene")]
    AdvanceCutscene,
    /// Answer the cutscene's question with option `index` (in the step's order).
    #[serde(rename = "chooseCutsceneOption")]
    ChooseCutsceneOption {
        #[serde(with = "crate::units::index")]
        index: i32,
    },
    /// Skip the rest of a skippable cutscene (`crate::cutscenes` says what still happens).
    #[serde(rename = "skipCutscene")]
    SkipCutscene,
    /// Put on a piece of clothing from the inventory (M4); what the slot held goes back.
    #[serde(rename = "wearItem")]
    WearItem { item_id: String },
    /// Take off what is worn in `slot` (one of `wear_slots`), back into the inventory (M4).
    #[serde(rename = "takeOff")]
    TakeOff { slot: String },
    /// The character creator's answers (M4): the player's name, the farm's name and colours.
    /// Only before the first tick of a new game.
    #[serde(rename = "setPlayerLook")]
    SetPlayerLook {
        #[serde(default)]
        name: String,
        #[serde(default)]
        farm_name: String,
        #[serde(default)]
        look: PlayerLook,
    },
    /// Put items into the open chest or shipping bin (M5); absent `quality`: any, lowest first.
    #[serde(rename = "storeItem")]
    StoreItem {
        item_id: String,
        #[serde(with = "crate::units::count")]
        quantity: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        quality: Option<String>,
    },
    /// Take items from slot `index` of the open chest or shipping bin back (M5).
    #[serde(rename = "takeStoredItem")]
    TakeStoredItem {
        #[serde(with = "crate::units::index")]
        index: i32,
        #[serde(with = "crate::units::count")]
        quantity: u32,
    },
    #[serde(rename = "closeStorage")]
    CloseStorage,
    /// Order a building from the open shop's builder (M5), its top-left tile at `(x, y)`.
    #[serde(rename = "orderBuilding")]
    OrderBuilding {
        building_id: String,
        scene_id: String,
        #[serde(with = "crate::units::int")]
        x: i32,
        #[serde(with = "crate::units::int")]
        y: i32,
    },
    /// Upgrade a standing building (`id`, a building on the map) into `buildingId` (M5).
    #[serde(rename = "upgradeBuilding")]
    UpgradeBuilding { id: String, building_id: String },
    /// Buy an animal from the open shop into a building that houses it (M5).
    #[serde(rename = "buyAnimal")]
    BuyAnimal {
        species_id: String,
        /// The building on the map it goes to.
        building: String,
        #[serde(default)]
        name: String,
    },
    /// The farm map of a new game (M5). Only before the first tick.
    #[serde(rename = "chooseFarmLayout")]
    ChooseFarmLayout { layout_id: String },
    /// Hand an item in to a collection that takes donations (M6).
    #[serde(rename = "donateItem")]
    DonateItem { collection_id: String, item_id: String },
    /// Hand in what a bundle line still needs (M6).
    #[serde(rename = "handInBundle")]
    HandInBundle {
        bundle_id: String,
        #[serde(with = "crate::units::index")]
        line: i32,
    },
    /// Read letter `index` of the mailbox, taking what it holds (M6).
    #[serde(rename = "readMail")]
    ReadMail {
        #[serde(with = "crate::units::index")]
        index: i32,
    },
    /// A validated mutation returned by a sandboxed plugin hook (M5).
    #[serde(rename = "pluginMutation")]
    PluginMutation { plugin_id: String, mutation: PluginMutation },
}

impl Command {
    /// The discriminator literal (TS `command.type`).
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::SetMoveIntent { .. } => "setMoveIntent",
            Self::Move { .. } => "move",
            Self::UseTool { .. } => "useTool",
            Self::Interact => "interact",
            Self::InteractWith { .. } => "interactWith",
            Self::ChooseDialogueOption { .. } => "chooseDialogueOption",
            Self::CloseDialogue => "closeDialogue",
            Self::Sleep => "sleep",
            Self::OpenShop { .. } => "openShop",
            Self::CloseShop => "closeShop",
            Self::BuyItem { .. } => "buyItem",
            Self::SellItem { .. } => "sellItem",
            Self::RepairTool { .. } => "repairTool",
            Self::Craft { .. } => "craft",
            Self::PlaceMachine { .. } => "placeMachine",
            Self::MachineLoad { .. } => "machineLoad",
            Self::PickUpMachine => "pickUpMachine",
            Self::GiveGift { .. } => "giveGift",
            Self::DescendMine { .. } => "descendMine",
            Self::ExitMine => "exitMine",
            Self::PerformAction { .. } => "performAction",
            Self::UseItem { .. } => "useItem",
            Self::StartMinigame { .. } => "startMinigame",
            Self::ResolveMinigame { .. } => "resolveMinigame",
            Self::CancelMinigame => "cancelMinigame",
            Self::AdvanceCutscene => "advanceCutscene",
            Self::ChooseCutsceneOption { .. } => "chooseCutsceneOption",
            Self::SkipCutscene => "skipCutscene",
            Self::WearItem { .. } => "wearItem",
            Self::TakeOff { .. } => "takeOff",
            Self::SetPlayerLook { .. } => "setPlayerLook",
            Self::StoreItem { .. } => "storeItem",
            Self::TakeStoredItem { .. } => "takeStoredItem",
            Self::CloseStorage => "closeStorage",
            Self::OrderBuilding { .. } => "orderBuilding",
            Self::UpgradeBuilding { .. } => "upgradeBuilding",
            Self::BuyAnimal { .. } => "buyAnimal",
            Self::ChooseFarmLayout { .. } => "chooseFarmLayout",
            Self::DonateItem { .. } => "donateItem",
            Self::HandInBundle { .. } => "handInBundle",
            Self::ReadMail { .. } => "readMail",
            Self::PluginMutation { .. } => "pluginMutation",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trips_the_command_json_shape() {
        let command: Command = serde_json::from_value(json!({"type": "move", "dir": "up"})).unwrap();
        assert_eq!(command, Command::Move { dir: "up".to_owned() });
        assert_eq!(serde_json::to_value(&command).unwrap(), json!({"type": "move", "dir": "up"}));

        let unit: Command = serde_json::from_value(json!({"type": "interact"})).unwrap();
        assert_eq!(unit, Command::Interact);
        assert_eq!(serde_json::to_value(&unit).unwrap(), json!({"type": "interact"}));

        let nested: Command = serde_json::from_value(json!({
            "type": "pluginMutation",
            "pluginId": "morning-hum",
            "mutation": {"type": "giveItem", "itemId": "seed-wheat", "quantity": 2}
        }))
        .unwrap();
        assert_eq!(
            nested,
            Command::PluginMutation {
                plugin_id: "morning-hum".to_owned(),
                mutation: PluginMutation::GiveItem { item_id: "seed-wheat".to_owned(), quantity: 2 },
            }
        );
        assert_eq!(nested.type_name(), "pluginMutation");

        let plant: Command =
            serde_json::from_value(json!({"type": "interactWith", "seedItemId": "seed-tomato"})).unwrap();
        assert_eq!(
            plant,
            Command::InteractWith { seed_item_id: Some("seed-tomato".to_owned()), fertilizer_item_id: None }
        );
        assert_eq!(serde_json::to_value(&plant).unwrap(), json!({"type": "interactWith", "seedItemId": "seed-tomato"}));
        let sell: Command =
            serde_json::from_value(json!({"type": "sellItem", "itemId": "crop-wheat", "quantity": 2})).unwrap();
        assert_eq!(sell, Command::SellItem { item_id: "crop-wheat".to_owned(), quantity: 2, quality: None });
        assert_eq!(
            serde_json::to_value(&sell).unwrap(),
            json!({"type": "sellItem", "itemId": "crop-wheat", "quantity": 2})
        );

        for (command, shape) in [
            (Command::AdvanceCutscene, json!({"type": "advanceCutscene"})),
            (Command::ChooseCutsceneOption { index: 1 }, json!({"type": "chooseCutsceneOption", "index": 1})),
            (Command::SkipCutscene, json!({"type": "skipCutscene"})),
        ] {
            assert_eq!(serde_json::to_value(&command).unwrap(), shape);
            assert_eq!(serde_json::from_value::<Command>(shape.clone()).unwrap(), command);
            assert_eq!(Some(command.type_name()), shape["type"].as_str());
        }
    }
}
