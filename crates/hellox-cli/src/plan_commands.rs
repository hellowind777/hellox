use anyhow::{anyhow, Result};
use hellox_agent::{PlanItem, PlanningState, StoredSession};

use crate::cli_types::PlanCommands;
use crate::plan_panel::render_plan_panel;

pub fn handle_plan_command(command: PlanCommands) -> Result<()> {
    println!("{}", plan_command_text(command)?);
    Ok(())
}

pub(crate) fn plan_command_text(command: PlanCommands) -> Result<String> {
    match command {
        PlanCommands::Show { session_id } => {
            let stored = StoredSession::load(&session_id)?;
            Ok(format_session_planning(
                &session_id,
                &stored.snapshot.planning,
            ))
        }
        PlanCommands::Panel {
            session_id,
            step_number,
        } => {
            let stored = StoredSession::load(&session_id)?;
            match render_plan_panel(Some(&session_id), &stored.snapshot.planning, step_number) {
                Ok(panel) => Ok(panel),
                Err(error) => Ok(format!("Unable to render plan panel: {error}")),
            }
        }
        PlanCommands::Enter { session_id } => {
            let mut stored = StoredSession::load(&session_id)?;
            stored.snapshot.planning.enter();
            let messages = stored.restore_messages();
            stored.save(&messages)?;
            Ok(format!(
                "Plan mode enabled for `{session_id}`.\n{}",
                format_session_planning(&session_id, &stored.snapshot.planning)
            ))
        }
        PlanCommands::AddStep {
            session_id,
            step,
            index,
        } => mutate_session_planning(&session_id, |planning| {
            let step_number = planning.add_step(parse_plan_step_spec(&step)?, index)?;
            Ok(format!("Added plan step {step_number} for `{session_id}`."))
        }),
        PlanCommands::UpdateStep {
            session_id,
            step_number,
            step,
        } => mutate_session_planning(&session_id, |planning| {
            planning.update_step(step_number, parse_plan_step_spec(&step)?)?;
            Ok(format!(
                "Updated plan step {step_number} for `{session_id}`."
            ))
        }),
        PlanCommands::RemoveStep {
            session_id,
            step_number,
        } => mutate_session_planning(&session_id, |planning| {
            let removed = planning.remove_step(step_number)?;
            Ok(format!(
                "Removed plan step {step_number} (`{}`) for `{session_id}`.",
                removed.step
            ))
        }),
        PlanCommands::Allow { session_id, prompt } => {
            mutate_session_planning(&session_id, |planning| {
                if planning.allow_prompt(prompt.clone())? {
                    Ok(format!("Allowed prompt added for `{session_id}`."))
                } else {
                    Ok(format!(
                        "Allowed prompt already present for `{session_id}`."
                    ))
                }
            })
        }
        PlanCommands::Disallow { session_id, prompt } => {
            mutate_session_planning(&session_id, |planning| {
                if planning.disallow_prompt(&prompt)? {
                    Ok(format!("Allowed prompt removed for `{session_id}`."))
                } else {
                    Err(anyhow!("allowed prompt `{prompt}` was not found"))
                }
            })
        }
        PlanCommands::Exit {
            session_id,
            steps,
            allowed_prompts,
        } => {
            if steps.is_empty() {
                return Err(anyhow!(
                    "plan exit requires at least one `--step <status>:<text>` value"
                ));
            }

            let mut stored = StoredSession::load(&session_id)?;
            stored
                .snapshot
                .planning
                .exit(parse_plan_step_specs(&steps)?, allowed_prompts)?;
            let messages = stored.restore_messages();
            stored.save(&messages)?;
            Ok(format!(
                "Stored accepted plan for `{session_id}`.\n{}",
                format_session_planning(&session_id, &stored.snapshot.planning)
            ))
        }
        PlanCommands::Clear { session_id } => {
            let mut stored = StoredSession::load(&session_id)?;
            stored.snapshot.planning = PlanningState::default();
            let messages = stored.restore_messages();
            stored.save(&messages)?;
            Ok(format!(
                "Cleared planning state for `{session_id}`.\n{}",
                format_session_planning(&session_id, &stored.snapshot.planning)
            ))
        }
    }
}

fn mutate_session_planning<F>(session_id: &str, mutator: F) -> Result<String>
where
    F: FnOnce(&mut PlanningState) -> Result<String>,
{
    let mut stored = StoredSession::load(session_id)?;
    let message = mutator(&mut stored.snapshot.planning)?;
    let messages = stored.restore_messages();
    stored.save(&messages)?;
    Ok(format!(
        "{message}\n{}",
        format_session_planning(session_id, &stored.snapshot.planning)
    ))
}

pub(crate) fn parse_plan_step_specs(specs: &[String]) -> Result<Vec<PlanItem>> {
    specs
        .iter()
        .map(|spec| parse_plan_step_spec(spec))
        .collect::<Result<Vec<_>>>()
}

pub(crate) fn parse_plan_step_spec(spec: &str) -> Result<PlanItem> {
    let trimmed = spec.trim();
    let Some((status, step)) = trimmed.split_once(':') else {
        return Err(anyhow!(
            "invalid plan step `{trimmed}`; use `<status>:<step>`"
        ));
    };

    let item = PlanItem {
        step: step.trim().to_string(),
        status: status.trim().to_string(),
    }
    .normalized();
    item.validate()?;
    Ok(item)
}

pub(crate) fn format_planning_state(planning: &PlanningState) -> String {
    let mut lines = vec![
        format!("plan_mode: {}", planning.active),
        format!("accepted_steps: {}", planning.plan.len()),
    ];

    if planning.plan.is_empty() {
        lines.push("plan: (none)".to_string());
    } else {
        lines.push("plan:".to_string());
        for (index, item) in planning.plan.iter().enumerate() {
            lines.push(format!("{}. [{}] {}", index + 1, item.status, item.step));
        }
    }

    if planning.allowed_prompts.is_empty() {
        lines.push("allowed_prompts: (none)".to_string());
    } else {
        lines.push(format!(
            "allowed_prompts: {}",
            planning.allowed_prompts.join(" | ")
        ));
    }

    lines.join("\n")
}

fn format_session_planning(session_id: &str, planning: &PlanningState) -> String {
    format!(
        "session_id: {session_id}\n{}",
        format_planning_state(planning)
    )
}

#[cfg(test)]
mod tests {
    use super::{format_planning_state, parse_plan_step_spec};
    use hellox_agent::PlanningState;

    #[test]
    fn parse_plan_step_requires_status_and_text() {
        let item = parse_plan_step_spec("in_progress:Refine CLI plan mode").expect("parse step");
        assert_eq!(item.status, "in_progress");
        assert_eq!(item.step, "Refine CLI plan mode");
    }

    #[test]
    fn format_planning_state_lists_steps_and_allowed_prompts() {
        let mut planning = PlanningState::default();
        planning.enter();
        planning
            .exit(
                vec![parse_plan_step_spec("completed:Audit docs").expect("step")],
                vec![String::from("continue implementation")],
            )
            .expect("exit planning");

        let text = format_planning_state(&planning);
        assert!(text.contains("1. [completed] Audit docs"));
        assert!(text.contains("continue implementation"));
    }

    #[test]
    fn plan_authoring_helpers_preserve_numbered_output() {
        let mut planning = PlanningState::default();
        planning
            .add_step(
                parse_plan_step_spec("pending:Draft plan surface").expect("step"),
                None,
            )
            .expect("add step");
        planning
            .add_step(
                parse_plan_step_spec("in_progress:Ship plan authoring").expect("step"),
                None,
            )
            .expect("add step");
        planning
            .allow_prompt("continue implementation".to_string())
            .expect("allow prompt");

        let text = format_planning_state(&planning);
        assert!(text.contains("1. [pending] Draft plan surface"));
        assert!(text.contains("2. [in_progress] Ship plan authoring"));
        assert!(text.contains("continue implementation"));
    }
}
