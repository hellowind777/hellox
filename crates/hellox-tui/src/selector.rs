#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectorEntry {
    pub title: String,
    pub badge: Option<String>,
    pub lines: Vec<String>,
    pub selected: bool,
}

impl SelectorEntry {
    pub fn new(title: impl Into<String>, lines: Vec<String>) -> Self {
        Self {
            title: title.into(),
            badge: None,
            lines,
            selected: false,
        }
    }

    pub fn with_badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

pub fn render_selector(entries: &[SelectorEntry]) -> Vec<String> {
    render_selector_with_start(entries, 1)
}

pub fn render_selector_with_start(entries: &[SelectorEntry], start_index: usize) -> Vec<String> {
    if entries.is_empty() {
        return vec!["(none)".to_string()];
    }

    let first_index = start_index.max(1);
    let last_index = first_index + entries.len() - 1;
    let width = last_index.to_string().len();
    let mut lines = Vec::new();

    for (index, entry) in entries.iter().enumerate() {
        let marker = if entry.selected { ">" } else { " " };
        let display_index = first_index + index;
        let mut header = format!(
            "{marker} [{:>width$}] {}",
            display_index,
            entry.title,
            width = width
        );
        if let Some(badge) = &entry.badge {
            header.push_str(&format!(" — {badge}"));
        }
        lines.push(header);

        if entry.lines.is_empty() {
            lines.push("    (none)".to_string());
        } else {
            lines.extend(entry.lines.iter().map(|line| format!("    {line}")));
        }

        if index + 1 < entries.len() {
            lines.push(String::new());
        }
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::{render_selector, render_selector_with_start, SelectorEntry};

    #[test]
    fn selector_renders_numbered_entries() {
        let lines = render_selector(&[
            SelectorEntry::new("release-review", vec!["steps: 2".to_string()]).with_badge("VALID"),
            SelectorEntry::new("ship", vec!["steps: 1".to_string()]).selected(true),
        ]);

        assert_eq!(lines[0], "  [1] release-review — VALID");
        assert_eq!(lines[1], "    steps: 2");
        assert!(lines.iter().any(|line| line == "> [2] ship"));
    }

    #[test]
    fn selector_renders_empty_state() {
        let lines = render_selector(&[]);
        assert_eq!(lines, vec!["(none)".to_string()]);
    }

    #[test]
    fn selector_supports_custom_start_index() {
        let lines = render_selector_with_start(
            &[SelectorEntry::new(
                "custom-run",
                vec!["status: failed".to_string()],
            )],
            8,
        );

        assert_eq!(lines[0], "  [8] custom-run");
        assert_eq!(lines[1], "    status: failed");
    }
}
