//! Tool rules — extracted from src/lib/tools.ts, behavior-identical (characterization-tested).
//! Port of `Tools.cs` / tools.ts. What each tool does is content since M1
//! ([`crate::schema::ToolDefinition`]); these are the rules every tool shares.

use crate::schema::{GameContent, Item, ToolDefinition};
use crate::vocabulary;

/// JS truthiness of an optional string: `undefined` and `""` are falsy.
fn tool_type_of(item: &Item) -> Option<&str> {
    item.tool_type.as_deref().filter(|tool_type| !tool_type.is_empty())
}

/// The game's definition of the tool `item` is, if it is one.
pub fn get_tool_from_item<'a>(content: &'a GameContent, item: &Item) -> Option<&'a ToolDefinition> {
    let tool_type = tool_type_of(item)?;
    vocabulary::tools(content).iter().find(|tool| tool.id == tool_type)
}

/// JS truthiness of an optional number: `undefined`, `0` and `NaN` are falsy (`None`).
/// JS `value || undefined` for an optional number: 0 counts as absent.
fn truthy_number(value: Option<i32>) -> Option<i32> {
    value.filter(|v| *v != 0)
}

/// `tool` with `amount` less durability (not below 0); a tool without durability is unchanged.
pub fn damage_tool_durability(tool: &Item, amount: i32) -> Item {
    let (Some(durability), Some(_)) = (truthy_number(tool.durability), truthy_number(tool.max_durability)) else {
        return tool.clone();
    };
    Item { durability: Some(durability.saturating_sub(amount).max(0)), ..tool.clone() }
}

/// Whether a breakable tool is at 0 durability.
pub fn is_tool_broken(tool: &Item) -> bool {
    // M2 fix: a tool at exactly 0 durability IS broken (the old falsy check
    // meant tools could never break; repair shops make breakage meaningful).
    match (tool.durability, tool.max_durability) {
        (Some(durability), Some(_)) => durability <= 0,
        _ => false,
    }
}

/// `tool` repaired by `amount` (a full repair when `None` or 0), up to its maximum.
/// Test-only: the engine no longer calls it; the tests pin the reference behaviour.
#[cfg(test)]
pub fn repair_tool(tool: &Item, amount: Option<i32>) -> Item {
    let (Some(durability), Some(max_durability)) = (truthy_number(tool.durability), truthy_number(tool.max_durability))
    else {
        return tool.clone();
    };
    let repair_amount = truthy_number(amount).unwrap_or(max_durability);
    Item { durability: Some(max_durability.min(durability.saturating_add(repair_amount))), ..tool.clone() }
}

/// Port of tests/unit/tools.characterization.test.ts (`ToolsCharacterizationTests.cs`;
/// src/lib/tools.ts re-exports the Core tool rules). Pins current behavior, quirks included:
/// durability 0 / maxDurability 0 are falsy no-ops for damage and repair, repair amount 0 is a
/// FULL repair, negative damage heals without a clamp, and canUseTool only inspects tile.type.
#[cfg(test)]
mod tests {
    use super::*;

    const ALL_TOOL_TYPES: [&str; 6] = ["watering-can", "hoe", "axe", "pickaxe", "scythe", "fishing-rod"];

    fn make_item(r#type: &str, tool_type: Option<&str>, durability: Option<i32>, max_durability: Option<i32>) -> Item {
        Item {
            id: "item-1".to_owned(),
            name: "Test Item".to_owned(),
            description: "A test item".to_owned(),
            r#type: r#type.to_owned(),
            stackable: false,
            max_stack: 1,
            value: 10,
            tool_type: tool_type.map(str::to_owned),
            durability,
            max_durability,
            ..Item::default()
        }
    }

    fn make_tool(tool_type: &str, durability: Option<i32>, max_durability: Option<i32>) -> Item {
        make_item("tool", Some(tool_type), durability, max_durability)
    }

    // --- getToolFromItem ---

    #[test]
    fn get_tool_from_item_returns_the_definition_or_null() {
        let content = crate::schema::GameContent::default();
        for tool_type in ALL_TOOL_TYPES {
            assert_eq!(get_tool_from_item(&content, &make_tool(tool_type, None, None)).map(|tool| tool.id.as_str()), Some(tool_type));
        }
        assert_eq!(get_tool_from_item(&content, &make_item("crop", None, None, None)), None);
        assert_eq!(get_tool_from_item(&content, &make_item("tool", Some("laser"), None, None)), None);
    }

    // --- damageToolDurability ---

    #[test]
    fn subtracts_the_given_amount_and_returns_a_new_object() {
        let tool = make_tool("axe", Some(50), Some(100));
        let result = damage_tool_durability(&tool, 10);
        assert_ne!(tool, result);
        assert_eq!(result.durability, Some(40));
        assert_eq!(result.max_durability, Some(100));
        assert_eq!(tool.durability, Some(50));
    }

    #[test]
    fn damage_pins_its_behavior() {
        assert_eq!(damage_tool_durability(&make_tool("axe", Some(50), Some(100)), 1).durability, Some(49)); // the TS default amount
        assert_eq!(damage_tool_durability(&make_tool("axe", Some(3), Some(100)), 10).durability, Some(0)); // clamps at 0
        let no_durability = make_tool("axe", None, Some(100));
        assert_eq!(damage_tool_durability(&no_durability, 5), no_durability);
        let no_max = make_tool("axe", Some(50), None);
        assert_eq!(damage_tool_durability(&no_max, 5), no_max);
        // QUIRK: durability 0 is falsy, so a fully depleted tool is a no-op
        let depleted = make_tool("axe", Some(0), Some(100));
        assert_eq!(damage_tool_durability(&depleted, 5), depleted);
        // QUIRK: maxDurability 0 is falsy
        let zero_max = make_tool("axe", Some(50), Some(0));
        assert_eq!(damage_tool_durability(&zero_max, 5), zero_max);
        // QUIRK: a negative amount heals the tool with no upper clamp
        assert_eq!(damage_tool_durability(&make_tool("axe", Some(95), Some(100)), -10).durability, Some(105));
    }

    // --- isToolBroken ---

    #[test]
    fn is_tool_broken_pins_its_behavior() {
        assert!(!is_tool_broken(&make_tool("hoe", None, None)));
        assert!(is_tool_broken(&make_tool("hoe", Some(0), Some(100)))); // M2 fix: exactly 0 IS broken
        assert!(is_tool_broken(&make_tool("hoe", Some(-1), Some(100))));
        assert!(is_tool_broken(&make_tool("hoe", Some(-100), Some(100))));
        assert!(!is_tool_broken(&make_tool("hoe", Some(1), Some(100))));
        assert!(!is_tool_broken(&make_tool("hoe", Some(100), Some(100))));
        assert!(!is_tool_broken(&make_tool("hoe", Some(-5), None))); // requires maxDurability
    }

    // --- repairTool ---

    #[test]
    fn partial_repair_adds_the_given_amount_and_returns_a_new_object() {
        let tool = make_tool("pickaxe", Some(40), Some(100));
        let result = repair_tool(&tool, Some(25));
        assert_ne!(tool, result);
        assert_eq!(result.durability, Some(65));
        assert_eq!(tool.durability, Some(40));
    }

    #[test]
    fn repair_pins_its_behavior() {
        assert_eq!(repair_tool(&make_tool("pickaxe", Some(7), Some(100)), None).durability, Some(100)); // full repair
        assert_eq!(repair_tool(&make_tool("pickaxe", Some(90), Some(100)), Some(50)).durability, Some(100)); // clamps
        let no_durability = make_tool("pickaxe", None, Some(100));
        assert_eq!(repair_tool(&no_durability, Some(10)), no_durability);
        let no_max = make_tool("pickaxe", Some(40), None);
        assert_eq!(repair_tool(&no_max, Some(10)), no_max);
        // QUIRK: a fully depleted tool can NEVER be repaired
        let depleted = make_tool("pickaxe", Some(0), Some(100));
        assert_eq!(repair_tool(&depleted, Some(50)), depleted);
        assert_eq!(repair_tool(&depleted, None), depleted);
        // QUIRK: amount 0 is falsy → FULL repair
        assert_eq!(repair_tool(&make_tool("pickaxe", Some(30), Some(100)), Some(0)).durability, Some(100));
        // a negative amount reduces durability (upper clamp only)
        assert_eq!(repair_tool(&make_tool("pickaxe", Some(30), Some(100)), Some(-10)).durability, Some(20));
    }
}
