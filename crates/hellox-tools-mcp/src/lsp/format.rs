use std::collections::BTreeSet;

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FormattedLspResult {
    pub(crate) text: String,
    pub(crate) result_count: usize,
    pub(crate) file_count: usize,
}

pub(crate) fn format_operation(operation: &str, value: &Value) -> FormattedLspResult {
    match operation {
        "hover" => format_hover(value),
        "documentSymbol" => format_document_symbols(value),
        "workspaceSymbol" => format_workspace_symbols(value),
        "prepareCallHierarchy" => format_call_hierarchy_items(value),
        "incomingCalls" => format_calls(value, "from"),
        "outgoingCalls" => format_calls(value, "to"),
        _ => format_locations(value),
    }
}

fn format_hover(value: &Value) -> FormattedLspResult {
    let contents = value
        .get("contents")
        .map(render_hover_contents)
        .unwrap_or_else(|| "(no hover information)".to_string());
    let result_count = usize::from(contents != "(no hover information)");
    FormattedLspResult {
        text: contents,
        result_count,
        file_count: result_count,
    }
}

fn render_hover_contents(value: &Value) -> String {
    match value {
        Value::String(text) => text.trim().to_string(),
        Value::Array(items) => items
            .iter()
            .map(render_hover_contents)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        Value::Object(map) => map
            .get("value")
            .and_then(Value::as_str)
            .or_else(|| map.get("contents").and_then(Value::as_str))
            .unwrap_or("")
            .trim()
            .to_string(),
        _ => String::new(),
    }
}

fn format_locations(value: &Value) -> FormattedLspResult {
    let locations = normalized_locations(value);
    let file_count = unique_file_count(locations.iter().filter_map(|entry| entry.uri.as_deref()));
    let text = if locations.is_empty() {
        "(no results)".to_string()
    } else {
        locations
            .iter()
            .map(|entry| {
                let line = entry.line + 1;
                let character = entry.character + 1;
                format!(
                    "{}:{}:{}",
                    entry.uri.as_deref().unwrap_or("(unknown)"),
                    line,
                    character
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    FormattedLspResult {
        text,
        result_count: locations.len(),
        file_count,
    }
}

fn format_document_symbols(value: &Value) -> FormattedLspResult {
    let empty = Vec::new();
    let mut lines = Vec::new();
    collect_document_symbols(value.as_array().unwrap_or(&empty), 0, &mut lines);
    FormattedLspResult {
        text: if lines.is_empty() {
            "(no symbols)".to_string()
        } else {
            lines.join("\n")
        },
        result_count: lines.len(),
        file_count: usize::from(!lines.is_empty()),
    }
}

fn collect_document_symbols(items: &[Value], depth: usize, lines: &mut Vec<String>) {
    for item in items {
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("(unnamed)");
        let line = item
            .get("range")
            .and_then(|value| value.get("start"))
            .and_then(|value| value.get("line"))
            .and_then(Value::as_u64)
            .unwrap_or(0)
            + 1;
        lines.push(format!("{}{} @ line {}", "  ".repeat(depth), name, line));
        if let Some(children) = item.get("children").and_then(Value::as_array) {
            collect_document_symbols(children, depth + 1, lines);
        }
    }
}

fn format_workspace_symbols(value: &Value) -> FormattedLspResult {
    let empty = Vec::new();
    let items = value.as_array().unwrap_or(&empty);
    let lines = items
        .iter()
        .map(|item| {
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("(unnamed)");
            let container = item
                .get("containerName")
                .and_then(Value::as_str)
                .unwrap_or("(global)");
            let uri = item
                .get("location")
                .and_then(|value| value.get("uri"))
                .and_then(Value::as_str)
                .unwrap_or("(unknown)");
            format!("{name} — {container} — {uri}")
        })
        .collect::<Vec<_>>();
    let file_count = unique_file_count(items.iter().filter_map(|item| {
        item.get("location")
            .and_then(|value| value.get("uri"))
            .and_then(Value::as_str)
    }));
    FormattedLspResult {
        text: if lines.is_empty() {
            "(no symbols)".to_string()
        } else {
            lines.join("\n")
        },
        result_count: lines.len(),
        file_count,
    }
}

fn format_call_hierarchy_items(value: &Value) -> FormattedLspResult {
    let empty = Vec::new();
    let items = value.as_array().unwrap_or(&empty);
    let lines = items
        .iter()
        .map(|item| {
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("(unnamed)");
            let uri = item
                .get("uri")
                .and_then(Value::as_str)
                .unwrap_or("(unknown)");
            format!("{name} — {uri}")
        })
        .collect::<Vec<_>>();
    let file_count = unique_file_count(
        items
            .iter()
            .filter_map(|item| item.get("uri").and_then(Value::as_str)),
    );
    FormattedLspResult {
        text: if lines.is_empty() {
            "(no call hierarchy items)".to_string()
        } else {
            lines.join("\n")
        },
        result_count: lines.len(),
        file_count,
    }
}

fn format_calls(value: &Value, key: &str) -> FormattedLspResult {
    let empty = Vec::new();
    let calls = value.as_array().unwrap_or(&empty);
    let lines = calls
        .iter()
        .map(|item| {
            let target = item.get(key).unwrap_or(&Value::Null);
            let name = target
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("(unnamed)");
            let uri = target
                .get("uri")
                .and_then(Value::as_str)
                .unwrap_or("(unknown)");
            format!("{name} — {uri}")
        })
        .collect::<Vec<_>>();
    let file_count = unique_file_count(calls.iter().filter_map(|item| {
        item.get(key)
            .and_then(|value| value.get("uri"))
            .and_then(Value::as_str)
    }));
    FormattedLspResult {
        text: if lines.is_empty() {
            "(no calls)".to_string()
        } else {
            lines.join("\n")
        },
        result_count: lines.len(),
        file_count,
    }
}

#[derive(Debug)]
struct LocationEntry {
    uri: Option<String>,
    line: usize,
    character: usize,
}

fn normalized_locations(value: &Value) -> Vec<LocationEntry> {
    let items = if let Some(array) = value.as_array() {
        array.clone()
    } else if value.is_null() {
        Vec::new()
    } else {
        vec![value.clone()]
    };

    items
        .into_iter()
        .map(|item| {
            let target = item
                .get("targetUri")
                .map(|_| {
                    (
                        item.get("targetUri"),
                        item.get("targetSelectionRange")
                            .or_else(|| item.get("targetRange")),
                    )
                })
                .unwrap_or((item.get("uri"), item.get("range")));
            let start = target
                .1
                .and_then(|range| range.get("start"))
                .cloned()
                .unwrap_or(Value::Null);
            LocationEntry {
                uri: target.0.and_then(Value::as_str).map(ToString::to_string),
                line: start.get("line").and_then(Value::as_u64).unwrap_or(0) as usize,
                character: start.get("character").and_then(Value::as_u64).unwrap_or(0) as usize,
            }
        })
        .collect()
}

fn unique_file_count<'a>(items: impl Iterator<Item = &'a str>) -> usize {
    let mut unique = BTreeSet::new();
    for item in items {
        unique.insert(item.to_string());
    }
    unique.len()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::format_operation;

    #[test]
    fn formats_location_results() {
        let formatted = format_operation(
            "goToDefinition",
            &json!([{
                "uri": "file:///repo/src/main.rs",
                "range": { "start": { "line": 3, "character": 2 } }
            }]),
        );
        assert!(formatted.text.contains("file:///repo/src/main.rs:4:3"));
        assert_eq!(formatted.result_count, 1);
        assert_eq!(formatted.file_count, 1);
    }

    #[test]
    fn formats_hover_results() {
        let formatted = format_operation(
            "hover",
            &json!({
                "contents": {
                    "kind": "markdown",
                    "value": "```rs\nfn main()\n```"
                }
            }),
        );
        assert!(formatted.text.contains("fn main()"));
        assert_eq!(formatted.result_count, 1);
    }
}
