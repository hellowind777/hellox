use hellox_agent::PlanItem;
use hellox_tui::{render_selector, status_badge, SelectorEntry};

pub(super) fn render_plan_selector(plan: &[PlanItem]) -> Vec<String> {
    let entries = plan
        .iter()
        .enumerate()
        .map(build_plan_entry)
        .collect::<Vec<_>>();
    render_selector(&entries)
}

pub(super) fn render_step_lens(plan: &[PlanItem]) -> Vec<String> {
    let Some(first) = plan.first() else {
        return vec!["(no accepted steps)".to_string()];
    };

    let lines = vec![
        format!("status: {}", status_badge(&first.status)),
        format!("step_chars: {}", first.step.chars().count()),
        format!("next_update: `hellox plan update-step <session-id> 1 --step \"{}:<text>\"`", first.status),
        "- reorder: `hellox plan remove-step <session-id> 1` + `hellox plan add-step <session-id> --index <n> --step \"<status>:<text>\"`".to_string(),
        "- repl update: `/plan update 1 <status>:<text>`".to_string(),
    ];

    render_selector(&[SelectorEntry::new("Step 1".to_string(), lines)
        .with_badge(status_badge(&first.status))
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

fn build_plan_entry((index, item): (usize, &PlanItem)) -> SelectorEntry {
    let lines = vec![
        format!("step: {}", preview_text(&item.step, 96)),
        format!("step_chars: {}", item.step.chars().count()),
        format!("update: `/plan update {} <status>:<text>`", index + 1),
        format!("remove: `/plan remove {}`", index + 1),
    ];

    SelectorEntry::new(format!("Step {}", index + 1), lines).with_badge(status_badge(&item.status))
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
