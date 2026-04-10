use hellox_agent::PlanningState;
use hellox_tui::{render_panel, KeyValueRow, PanelSection};

#[path = "plan_panel_selector.rs"]
mod selector;

use selector::{render_allowed_prompt_selector, render_plan_selector, render_step_lens};

pub(crate) fn render_plan_panel(session_id: Option<&str>, planning: &PlanningState) -> String {
    let metadata = vec![
        KeyValueRow::new("session_id", session_id.unwrap_or("(none)")),
        KeyValueRow::new("plan_mode", planning.active.to_string()),
        KeyValueRow::new("steps", planning.plan.len().to_string()),
        KeyValueRow::new(
            "allowed_prompts",
            planning.allowed_prompts.len().to_string(),
        ),
    ];

    let sections = vec![
        PanelSection::new(
            "Accepted plan selector",
            render_plan_selector(&planning.plan),
        ),
        PanelSection::new("Focused step lens", render_step_lens(&planning.plan)),
        PanelSection::new(
            "Allowed prompt selector",
            render_allowed_prompt_selector(&planning.allowed_prompts),
        ),
        PanelSection::new("Action palette", plan_cli_palette(session_id)),
        PanelSection::new("REPL palette", plan_repl_palette()),
    ];

    render_panel("Plan panel", &metadata, &sections)
}

fn plan_cli_palette(session_id: Option<&str>) -> Vec<String> {
    let session_id = session_id.unwrap_or("<session-id>");
    vec![
        format!("- show (raw): `hellox plan show {session_id}`"),
        format!("- panel: `hellox plan panel {session_id}`"),
        format!("- add step: `hellox plan add-step {session_id} --step \"pending:<text>\"`"),
        format!(
            "- update step: `hellox plan update-step {session_id} <n> --step \"completed:<text>\"`"
        ),
        format!("- remove step: `hellox plan remove-step {session_id} <n>`"),
        format!("- allow prompt: `hellox plan allow {session_id} \"<prompt>\"`"),
        format!("- accept plan: `hellox plan exit {session_id} --step \"completed:<text>\"`"),
        format!("- clear: `hellox plan clear {session_id}`"),
    ]
}

fn plan_repl_palette() -> Vec<String> {
    vec![
        "- show (raw): `/plan` or `/plan show`".to_string(),
        "- panel: `/plan panel`".to_string(),
        "- enter: `/plan enter`".to_string(),
        "- add: `/plan add [--index <n>] <status>:<text>`".to_string(),
        "- update: `/plan update <n> <status>:<text>`".to_string(),
        "- remove: `/plan remove <n>`".to_string(),
        "- allow: `/plan allow <prompt>`".to_string(),
        "- exit: `/plan exit --step <status>:<text>... [--allow <prompt>...]`".to_string(),
        "- clear: `/plan clear`".to_string(),
    ]
}
