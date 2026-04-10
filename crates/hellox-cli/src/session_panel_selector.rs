use hellox_agent::StoredSessionSnapshot;
use hellox_session::SessionSummary;
use hellox_tui::{render_selector, SelectorEntry};

pub(super) fn render_session_selector(sessions: &[SessionSummary]) -> Vec<String> {
    let entries = sessions.iter().map(build_session_entry).collect::<Vec<_>>();
    render_selector(&entries)
}

pub(super) fn render_session_lens(snapshot: &StoredSessionSnapshot) -> Vec<String> {
    let lines = vec![
        format!(
            "permission_mode: {}",
            snapshot
                .permission_mode
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "(from current config)".to_string())
        ),
        format!(
            "output_style: {}",
            snapshot.output_style_name.as_deref().unwrap_or("(none)")
        ),
        format!(
            "persona: {}",
            snapshot
                .persona
                .as_ref()
                .map(|persona| persona.name.as_str())
                .unwrap_or("(none)")
        ),
        format!("prompt_fragments: {}", render_prompt_fragments(snapshot)),
        format!("plan_mode: {}", yes_no(snapshot.planning.active)),
        format!("plan_steps: {}", snapshot.planning.plan.len()),
        format!("messages: {}", snapshot.messages.len()),
        format!("requests: {}", snapshot.total_requests()),
        format!("input_tokens: {}", snapshot.total_input_tokens()),
        format!("output_tokens: {}", snapshot.total_output_tokens()),
        format!(
            "working_directory: {}",
            preview_text(&snapshot.working_directory, 96)
        ),
        format!("show: `hellox session show {}`", snapshot.session_id),
    ];

    render_selector(&[SelectorEntry::new(snapshot.session_id.clone(), lines)
        .with_badge(snapshot.model.clone())
        .selected(true)])
}

fn build_session_entry(session: &SessionSummary) -> SelectorEntry {
    let lines = vec![
        format!("messages: {}", session.message_count),
        format!("updated_at: {}", session.updated_at),
        format!("requests: {}", total_requests(session)),
        format!("input_tokens: {}", total_input_tokens(session)),
        format!("output_tokens: {}", total_output_tokens(session)),
        format!("cwd: {}", preview_text(&session.working_directory, 72)),
        format!("open: `hellox session panel {}`", session.session_id),
        format!("resume: `/resume {}`", session.session_id),
    ];

    SelectorEntry::new(session.session_id.clone(), lines).with_badge(session.model.clone())
}

fn render_prompt_fragments(snapshot: &StoredSessionSnapshot) -> String {
    if snapshot.prompt_fragments.is_empty() {
        "(none)".to_string()
    } else {
        snapshot
            .prompt_fragments
            .iter()
            .map(|fragment| fragment.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn total_requests(session: &SessionSummary) -> u64 {
    session
        .usage_by_model
        .values()
        .map(|totals| totals.requests)
        .sum()
}

fn total_input_tokens(session: &SessionSummary) -> u64 {
    session
        .usage_by_model
        .values()
        .map(|totals| totals.input_tokens)
        .sum()
}

fn total_output_tokens(session: &SessionSummary) -> u64 {
    session
        .usage_by_model
        .values()
        .map(|totals| totals.output_tokens)
        .sum()
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn preview_text(value: &str, max_chars: usize) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max_chars {
        compact
    } else {
        let head = compact
            .chars()
            .take(max_chars.saturating_sub(3))
            .collect::<String>();
        format!("{head}...")
    }
}
