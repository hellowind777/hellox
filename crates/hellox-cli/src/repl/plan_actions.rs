use anyhow::{anyhow, Result};

use hellox_agent::AgentSession;

use crate::plan_commands::{format_planning_state, parse_plan_step_spec, parse_plan_step_specs};
use crate::plan_panel::render_plan_panel;

use super::commands::PlanCommand;

pub(super) async fn handle_plan_command(
    command: PlanCommand,
    session: &mut AgentSession,
) -> Result<String> {
    match command {
        PlanCommand::Show => Ok(format_planning_state(&session.planning_state())),
        PlanCommand::Panel { step_number } => {
            let planning = session.planning_state();
            match render_plan_panel(session.session_id(), &planning, step_number) {
                Ok(panel) => Ok(panel),
                Err(error) => Ok(format!("Unable to render plan panel: {error}")),
            }
        }
        PlanCommand::Enter => {
            let mut planning = session.planning_state();
            planning.enter();
            session.set_planning_state(planning)?;
            Ok(format!(
                "Plan mode enabled.\n{}",
                format_planning_state(&session.planning_state())
            ))
        }
        PlanCommand::Add { step: None, .. } => {
            Ok("Usage: /plan add [--index <n>] <status>:<text>".to_string())
        }
        PlanCommand::Add {
            step: Some(step),
            index,
        } => {
            let mut planning = session.planning_state();
            let step_number = planning.add_step(parse_plan_step_spec(&step)?, index)?;
            session.set_planning_state(planning)?;
            Ok(format!(
                "Added plan step {step_number}.\n{}",
                format_planning_state(&session.planning_state())
            ))
        }
        PlanCommand::Update {
            step_number: None, ..
        } => Ok("Usage: /plan update <step-number> <status>:<text>".to_string()),
        PlanCommand::Update {
            step_number: Some(_),
            step: None,
        } => Ok("Usage: /plan update <step-number> <status>:<text>".to_string()),
        PlanCommand::Update {
            step_number: Some(step_number),
            step: Some(step),
        } => {
            let mut planning = session.planning_state();
            planning.update_step(step_number, parse_plan_step_spec(&step)?)?;
            session.set_planning_state(planning)?;
            Ok(format!(
                "Updated plan step {step_number}.\n{}",
                format_planning_state(&session.planning_state())
            ))
        }
        PlanCommand::Remove { step_number: None } => {
            Ok("Usage: /plan remove <step-number>".to_string())
        }
        PlanCommand::Remove {
            step_number: Some(step_number),
        } => {
            let mut planning = session.planning_state();
            let removed = planning.remove_step(step_number)?;
            session.set_planning_state(planning)?;
            Ok(format!(
                "Removed plan step {step_number} (`{}`).\n{}",
                removed.step,
                format_planning_state(&session.planning_state())
            ))
        }
        PlanCommand::Allow { prompt: None } => Ok("Usage: /plan allow <prompt>".to_string()),
        PlanCommand::Allow {
            prompt: Some(prompt),
        } => {
            let mut planning = session.planning_state();
            let message = if planning.allow_prompt(prompt)? {
                "Allowed prompt added."
            } else {
                "Allowed prompt already present."
            };
            session.set_planning_state(planning)?;
            Ok(format!(
                "{message}\n{}",
                format_planning_state(&session.planning_state())
            ))
        }
        PlanCommand::Disallow { prompt: None } => {
            Ok("Usage: /plan disallow <prompt>".to_string())
        }
        PlanCommand::Disallow {
            prompt: Some(prompt),
        } => {
            let mut planning = session.planning_state();
            if !planning.disallow_prompt(&prompt)? {
                return Err(anyhow!("allowed prompt `{prompt}` was not found"));
            }
            session.set_planning_state(planning)?;
            Ok(format!(
                "Allowed prompt removed.\n{}",
                format_planning_state(&session.planning_state())
            ))
        }
        PlanCommand::Exit {
            steps,
            allowed_prompts,
        } => {
            if steps.is_empty() {
                return Ok(
                    "Usage: /plan exit --step <status>:<text> [--step <status>:<text>]... [--allow <prompt>]..."
                        .to_string(),
                );
            }

            let plan = match parse_plan_step_specs(&steps) {
                Ok(plan) => plan,
                Err(error) => {
                    return Ok(format!(
                        "{}\nUsage: /plan exit --step <status>:<text> [--step <status>:<text>]... [--allow <prompt>]...",
                        error
                    ))
                }
            };

            let mut planning = session.planning_state();
            planning.exit(plan, allowed_prompts)?;
            session.set_planning_state(planning)?;
            Ok(format!(
                "Accepted plan stored.\n{}",
                format_planning_state(&session.planning_state())
            ))
        }
        PlanCommand::Clear => {
            session.set_planning_state(hellox_agent::PlanningState::default())?;
            Ok(format!(
                "Planning state cleared.\n{}",
                format_planning_state(&session.planning_state())
            ))
        }
        PlanCommand::Help => Ok(
            "Usage: /plan [show|panel [step-number]|enter|add [--index <n>] <status>:<text>|update <step-number> <status>:<text>|remove <step-number>|allow <prompt>|disallow <prompt>|exit --step <status>:<text>... [--allow <prompt>...]|clear]"
                .to_string(),
        ),
    }
}
