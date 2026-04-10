use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::team_storage::default_layout_strategy_name;

const FANOUT: &str = "fanout";
const HORIZONTAL: &str = "horizontal";
const VERTICAL: &str = "vertical";
const GRID: &str = "grid";

pub fn parse_layout_strategy(input: &Value, key: &str) -> Result<Option<String>> {
    match input.get(key).and_then(Value::as_str) {
        Some(value) => normalize_layout_strategy(Some(value)).map(Some),
        None => Ok(None),
    }
}

pub fn normalize_layout_strategy(value: Option<&str>) -> Result<String> {
    match value
        .map(|value| value.trim().to_ascii_lowercase().replace('-', "_"))
        .as_deref()
    {
        None | Some("") => Ok(default_layout_strategy_name().to_string()),
        Some(FANOUT | HORIZONTAL | VERTICAL | GRID) => Ok(value
            .expect("checked above")
            .trim()
            .to_ascii_lowercase()
            .replace('-', "_")),
        Some(other) => Err(anyhow!(
            "unsupported team layout `{other}`; use one of: fanout, horizontal, vertical, grid"
        )),
    }
}

pub fn assign_layout_slots(strategy: &str, member_count: usize) -> Result<Vec<String>> {
    let strategy = normalize_layout_strategy(Some(strategy))?;
    let mut slots = Vec::with_capacity(member_count);
    for index in 0..member_count {
        let slot = match (strategy.as_str(), index) {
            (_, 0) => "primary".to_string(),
            (HORIZONTAL, index) => format_indexed_slot("right", index),
            (VERTICAL, index) => format_indexed_slot("bottom", index),
            (GRID, index) if index % 2 == 1 => format_indexed_slot("right", index.div_ceil(2)),
            (GRID, index) => format_indexed_slot("bottom", index / 2),
            (FANOUT, 1) => "right".to_string(),
            (FANOUT, index) => format_indexed_slot("bottom", index - 1),
            _ => format!("member-{}", index + 1),
        };
        slots.push(slot);
    }
    Ok(slots)
}

pub fn pane_group_for_team(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "hellox-team".to_string()
    } else {
        format!("hellox-{trimmed}")
    }
}

pub fn anchor_slot_for_layout_slot(layout_slot: Option<&str>) -> Option<String> {
    let slot = layout_slot?.trim();
    if slot.is_empty() || slot == "primary" {
        return None;
    }

    if let Some(index) = slot.strip_prefix("right-") {
        return Some(previous_indexed_slot("right", index));
    }
    if let Some(index) = slot.strip_prefix("bottom-") {
        return Some(previous_indexed_slot("bottom", index));
    }
    if slot.starts_with("right") || slot.starts_with("bottom") {
        return Some("primary".to_string());
    }
    Some("primary".to_string())
}

fn format_indexed_slot(prefix: &str, index: usize) -> String {
    if index <= 1 {
        prefix.to_string()
    } else {
        format!("{prefix}-{index}")
    }
}

fn previous_indexed_slot(prefix: &str, index: &str) -> String {
    match index.parse::<usize>() {
        Ok(2) => prefix.to_string(),
        Ok(value) if value > 2 => format!("{prefix}-{}", value - 1),
        _ => "primary".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn assigns_fanout_layout_slots() {
        let slots = assign_layout_slots("fanout", 4).expect("layout slots");
        assert_eq!(slots, ["primary", "right", "bottom", "bottom-2"]);
    }

    #[test]
    fn assigns_horizontal_layout_slots() {
        let slots = assign_layout_slots("horizontal", 3).expect("layout slots");
        assert_eq!(slots, ["primary", "right", "right-2"]);
    }

    #[test]
    fn parses_layout_strategy_from_input() {
        let layout = parse_layout_strategy(&json!({ "layout": "vertical" }), "layout")
            .expect("parse layout");
        assert_eq!(layout.as_deref(), Some("vertical"));
    }

    #[test]
    fn resolves_anchor_slots_for_layout_sequence() {
        assert_eq!(anchor_slot_for_layout_slot(Some("primary")), None);
        assert_eq!(
            anchor_slot_for_layout_slot(Some("right")).as_deref(),
            Some("primary")
        );
        assert_eq!(
            anchor_slot_for_layout_slot(Some("right-2")).as_deref(),
            Some("right")
        );
        assert_eq!(
            anchor_slot_for_layout_slot(Some("bottom-3")).as_deref(),
            Some("bottom-2")
        );
    }
}
