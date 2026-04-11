use hellox_agent::PlanItem;
use hellox_tui::{render_selector, status_badge, SelectorEntry};

pub(super) fn render_plan_selector(plan: &[PlanItem], step_number: Option<usize>) -> Vec<String> {
    let entries = plan
        .iter()
        .enumerate()
        .map(|(index, item)| build_plan_entry(index, item, step_number))
        .collect::<Vec<_>>();
    render_selector(&entries)
}

pub(super) fn render_step_lens(plan: &[PlanItem], step_number: Option<usize>) -> Vec<String> {
    let Some((index, step)) = plan_entry(plan, step_number) else {
        return vec!["(no accepted steps)".to_string()];
    };

    let lines = vec![
        format!("status: {}", status_badge(&step.status)),
        format!("step: {}", step.step),
        format!("step_chars: {}", step.step.chars().count()),
        format!(
            "next_update: `hellox plan update-step <session-id> {} --step \"{}:<text>\"`",
            index + 1,
            step.status
        ),
        format!(
            "- reorder: `hellox plan remove-step <session-id> {}` + `hellox plan add-step <session-id> --index <n> --step \"<status>:<text>\"`",
            index + 1
        ),
        format!("- repl update: `/plan update {} <status>:<text>`", index + 1),
        format!("- panel: `/plan panel {}`", index + 1),
    ];

    render_selector(&[SelectorEntry::new(format!("Step {}", index + 1), lines)
        .with_badge(status_badge(&step.status))
        .selected(true)])
}

pub(super) fn render_allowed_prompt_selector(prompts: &[String]) -> Vec<String> {
    let entries = prompts
        .iter()
        .enumerate()
        .map(|(index, prompt)| {
            SelectorEntry::new(
                format!("Prompt {}", index + 1),
                vec![
                    format!("prompt: {}", preview_text(prompt, 96)),
                    format!("chars: {}", prompt.chars().count()),
                    format!("remove: `/plan disallow {prompt}`"),
                ],
            )
        })
        .collect::<Vec<_>>();
    render_selector(&entries)
}

fn build_plan_entry(index: usize, item: &PlanItem, step_number: Option<usize>) -> SelectorEntry {
    let lines = vec![
        format!("step: {}", preview_text(&item.step, 96)),
        format!("step_chars: {}", item.step.chars().count()),
        format!("update: `/plan update {} <status>:<text>`", index + 1),
        format!("remove: `/plan remove {}`", index + 1),
        format!("focus: `/plan panel {}`", index + 1),
    ];

    SelectorEntry::new(format!("Step {}", index + 1), lines)
        .with_badge(status_badge(&item.status))
        .selected(step_number == Some(index + 1))
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

fn plan_entry(plan: &[PlanItem], step_number: Option<usize>) -> Option<(usize, &PlanItem)> {
    step_number
        .and_then(|step_number| {
            step_number
                .checked_sub(1)
                .and_then(|index| plan.get(index).map(|item| (index, item)))
        })
        .or_else(|| plan.first().map(|item| (0, item)))
}
