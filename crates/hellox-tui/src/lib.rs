mod selector;
mod workflow_dashboard;

pub use selector::{render_selector, render_selector_with_start, SelectorEntry};
pub use workflow_dashboard::{
    parse_workflow_dashboard_command, workflow_dashboard_help_text, WorkflowDashboardCommand,
    WorkflowDashboardOpenTarget, WorkflowDashboardState, WorkflowDashboardView,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyValueRow {
    pub label: String,
    pub value: String,
}

impl KeyValueRow {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub title: String,
    pub lines: Vec<String>,
}

impl Card {
    pub fn new(title: impl Into<String>, lines: Vec<String>) -> Self {
        Self {
            title: title.into(),
            lines,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelSection {
    pub title: String,
    pub lines: Vec<String>,
}

impl PanelSection {
    pub fn new(title: impl Into<String>, lines: Vec<String>) -> Self {
        Self {
            title: title.into(),
            lines,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    pub fn new(headers: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        Self { headers, rows }
    }
}

pub fn render_panel(title: &str, metadata: &[KeyValueRow], sections: &[PanelSection]) -> String {
    let mut lines = vec![title.to_string()];
    if !metadata.is_empty() {
        lines.extend(render_key_value_rows(metadata));
    }
    for section in sections {
        lines.push(String::new());
        lines.extend(render_section(section));
    }
    lines.join("\n")
}

pub fn render_section(section: &PanelSection) -> Vec<String> {
    let mut lines = vec![format!("== {} ==", section.title)];
    if section.lines.is_empty() {
        lines.push("(none)".to_string());
    } else {
        lines.extend(section.lines.iter().cloned());
    }
    lines
}

pub fn render_card(title: &str, lines: &[String]) -> Vec<String> {
    let mut rendered = vec![format!("[{title}]")];
    if lines.is_empty() {
        rendered.push("  (none)".to_string());
    } else {
        rendered.extend(lines.iter().map(|line| format!("  {line}")));
    }
    rendered
}

pub fn render_cards(cards: &[Card]) -> Vec<String> {
    let mut rendered = Vec::new();
    for (index, card) in cards.iter().enumerate() {
        if index > 0 {
            rendered.push(String::new());
        }
        rendered.extend(render_card(&card.title, &card.lines));
    }
    rendered
}

pub fn render_key_value_rows(rows: &[KeyValueRow]) -> Vec<String> {
    let width = rows.iter().map(|row| row.label.len()).max().unwrap_or(0);
    rows.iter()
        .map(|row| format!("{:<width$} : {}", row.label, row.value, width = width))
        .collect()
}

pub fn render_table(table: &Table) -> Vec<String> {
    let column_count = table
        .rows
        .iter()
        .map(Vec::len)
        .max()
        .unwrap_or(0)
        .max(table.headers.len());
    if column_count == 0 {
        return vec!["(none)".to_string()];
    }

    let mut widths = vec![0; column_count];
    for (index, header) in table.headers.iter().enumerate() {
        widths[index] = widths[index].max(header.len());
    }
    for row in &table.rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.len());
        }
    }

    let mut lines = Vec::new();
    if !table.headers.is_empty() {
        lines.push(render_table_row(&table.headers, &widths));
        lines.push(render_separator_row(&widths));
    }

    if table.rows.is_empty() {
        lines.push("(none)".to_string());
    } else {
        for row in &table.rows {
            lines.push(render_table_row(row, &widths));
        }
    }

    lines
}

pub fn status_badge(status: &str) -> String {
    match status {
        "completed" | "coordinated" => "COMPLETED".to_string(),
        "failed" => "FAILED".to_string(),
        "running" => "RUNNING".to_string(),
        "skipped" => "SKIPPED".to_string(),
        "cancelled" => "CANCELLED".to_string(),
        "valid" => "VALID".to_string(),
        "invalid" => "INVALID".to_string(),
        other => other.to_ascii_uppercase(),
    }
}

fn render_table_row(cells: &[String], widths: &[usize]) -> String {
    widths
        .iter()
        .enumerate()
        .map(|(index, width)| {
            let cell = cells.get(index).map(String::as_str).unwrap_or("");
            format!("{cell:<width$}", width = width)
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn render_separator_row(widths: &[usize]) -> String {
    widths
        .iter()
        .map(|width| "-".repeat(*width))
        .collect::<Vec<_>>()
        .join("-+-")
}

#[cfg(test)]
mod tests {
    use super::{
        render_card, render_cards, render_key_value_rows, render_panel, render_table, status_badge,
        Card, KeyValueRow, PanelSection, Table,
    };

    #[test]
    fn panel_renders_metadata_and_sections() {
        let text = render_panel(
            "Sample panel",
            &[KeyValueRow::new("status", "completed")],
            &[PanelSection::new("Actions", vec!["- run".to_string()])],
        );
        assert!(text.contains("Sample panel"));
        assert!(text.contains("status : completed"));
        assert!(text.contains("== Actions =="));
        assert!(text.contains("- run"));
    }

    #[test]
    fn card_indents_body_lines() {
        let lines = render_card("release-review", &["steps: 2".to_string()]);
        assert_eq!(lines[0], "[release-review]");
        assert_eq!(lines[1], "  steps: 2");
    }

    #[test]
    fn cards_insert_blank_lines_between_entries() {
        let lines = render_cards(&[
            Card::new("one", vec!["steps: 1".to_string()]),
            Card::new("two", vec!["steps: 2".to_string()]),
        ]);
        assert_eq!(lines[0], "[one]");
        assert!(lines.contains(&String::new()));
        assert!(lines.iter().any(|line| line == "[two]"));
    }

    #[test]
    fn status_badges_are_normalized() {
        assert_eq!(status_badge("completed"), "COMPLETED");
        assert_eq!(status_badge("valid"), "VALID");
        assert_eq!(status_badge("mixed"), "MIXED");
    }

    #[test]
    fn key_value_rows_align_labels() {
        let lines = render_key_value_rows(&[
            KeyValueRow::new("path", "a"),
            KeyValueRow::new("shared_context", "b"),
        ]);
        assert!(lines[0].contains("path"));
        assert!(lines[1].contains("shared_context"));
        assert!(lines[0].contains(" : "));
        assert!(lines[1].contains(" : "));
    }

    #[test]
    fn table_renders_headers_separator_and_rows() {
        let lines = render_table(&Table::new(
            vec!["#".to_string(), "step".to_string(), "status".to_string()],
            vec![
                vec![
                    "1".to_string(),
                    "review".to_string(),
                    "COMPLETED".to_string(),
                ],
                vec!["2".to_string(), "ship".to_string(), "FAILED".to_string()],
            ],
        ));
        assert_eq!(lines[0], "# | step   | status   ");
        assert!(lines[1].contains("-+-"));
        assert!(lines[2].contains("review"));
        assert!(lines[3].contains("FAILED"));
    }

    #[test]
    fn table_renders_empty_state_when_no_rows() {
        let lines = render_table(&Table::new(vec!["name".to_string()], Vec::new()));
        assert!(lines.iter().any(|line| line == "(none)"));
    }
}
