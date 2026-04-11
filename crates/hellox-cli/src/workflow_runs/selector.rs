use hellox_tui::{render_selector, render_selector_with_start, status_badge, SelectorEntry};

use super::{WorkflowRunRecord, WORKFLOW_RUN_SELECTOR_PREVIEW_LIMIT};

pub(super) fn render_run_selector(records: &[WorkflowRunRecord]) -> Vec<String> {
    render_run_selector_with_start(records, 1)
}

pub(crate) fn render_run_selector_with_start(
    records: &[WorkflowRunRecord],
    start_index: usize,
) -> Vec<String> {
    let entries = records
        .iter()
        .take(WORKFLOW_RUN_SELECTOR_PREVIEW_LIMIT)
        .map(build_run_entry)
        .collect::<Vec<_>>();
    if start_index <= 1 {
        render_selector(&entries)
    } else {
        render_selector_with_start(&entries, start_index)
    }
}

pub(super) fn render_step_lens(
    record: &WorkflowRunRecord,
    step_number: Option<usize>,
) -> Vec<String> {
    let Some((index, step)) = select_step(record, step_number) else {
        return vec!["(no recorded steps)".to_string()];
    };

    let mut lines = vec![
        format!("status: {}", status_badge(&step.status)),
        format!(
            "result_chars: {}",
            step.result_text
                .as_ref()
                .map(|text| text.chars().count().to_string())
                .unwrap_or_else(|| "-".to_string())
        ),
        format!("has_error: {}", yes_no(step.error.is_some())),
        format!("has_reason: {}", yes_no(step.reason.is_some())),
    ];
    if let Some(reason) = &step.reason {
        lines.push(format!("reason: {}", preview_text(reason)));
    }
    if let Some(error) = &step.error {
        lines.push(format!("error: {}", preview_text(error)));
    }
    if let Some(result_text) = &step.result_text {
        lines.push(format!("result: {}", preview_text(result_text)));
    }

    render_selector(&[SelectorEntry::new(step.name.clone(), lines)
        .with_badge(status_badge(&step.status))
        .selected(true)])
    .into_iter()
    .enumerate()
    .map(|(line_index, line)| {
        if line_index == 0 {
            format!("{line} (step {})", index + 1)
        } else {
            line
        }
    })
    .collect()
}

fn build_run_entry(record: &WorkflowRunRecord) -> SelectorEntry {
    let workflow = record.workflow_name.as_deref().unwrap_or("(custom path)");
    let source = record
        .workflow_source
        .as_deref()
        .or(record.requested_script_path.as_deref())
        .unwrap_or("(inline)");
    let mut lines = vec![
        format!("workflow: {workflow}"),
        format!("finished_at: {}", record.finished_at),
        format!(
            "summary: c{}/f{}/r{}/s{}",
            record.summary.completed_steps,
            record.summary.failed_steps,
            record.summary.running_steps,
            record.summary.skipped_steps
        ),
    ];

    if let Some((index, step)) = select_step(record, None) {
        lines.push(format!(
            "primary_step: [{}] {} — {}",
            index + 1,
            step.name,
            status_badge(&step.status)
        ));
        lines.push(format!(
            "focus: `hellox workflow show-run {} {}`",
            record.run_id,
            index + 1
        ));
    } else {
        if let Some(error) = &record.error {
            lines.push(format!("error: {}", preview_text(error)));
        }
        lines.push(format!(
            "focus: `hellox workflow show-run {}`",
            record.run_id
        ));
    }

    lines.push(format!(
        "shared_context: {}",
        preview_text(record.shared_context.as_deref().unwrap_or("(none)"))
    ));
    lines.push(format!("source: {}", preview_text(source)));
    lines.push(format!("next: `{}`", next_follow_up_hint(record)));

    SelectorEntry::new(record.run_id.clone(), lines).with_badge(status_badge(&record.status))
}

fn select_step(
    record: &WorkflowRunRecord,
    step_number: Option<usize>,
) -> Option<(usize, &super::WorkflowRunStepRecord)> {
    if let Some(step_number) = step_number {
        if step_number == 0 {
            return None;
        }
        return record.steps.iter().enumerate().nth(step_number - 1);
    }

    record
        .steps
        .iter()
        .enumerate()
        .find(|(_, step)| step.status.eq_ignore_ascii_case("failed"))
        .or_else(|| {
            record
                .steps
                .iter()
                .enumerate()
                .find(|(_, step)| step.status.eq_ignore_ascii_case("running"))
        })
        .or_else(|| record.steps.iter().enumerate().next())
}

fn preview_text(value: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= 120 {
        compact
    } else {
        format!("{}...", compact.chars().take(117).collect::<String>())
    }
}

fn next_follow_up_hint(record: &WorkflowRunRecord) -> String {
    if let Some(workflow_name) = record.workflow_name.as_deref() {
        format!("hellox workflow last-run {workflow_name}")
    } else if let Some(script_path) = record.requested_script_path.as_deref() {
        format!("hellox workflow run --script-path {script_path}")
    } else {
        format!("hellox workflow show-run {}", record.run_id)
    }
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}
