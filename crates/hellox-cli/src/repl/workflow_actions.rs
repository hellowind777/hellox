use std::path::Path;

use anyhow::{anyhow, Result};
use hellox_agent::AgentSession;

use crate::workflow_authoring::{
    add_workflow_step, remove_workflow_step, resolve_existing_workflow_path,
    set_workflow_continue_on_error, set_workflow_shared_context, update_workflow_step,
    WorkflowStepDraft, WorkflowStepPatch,
};
use crate::workflow_overview::render_workflow_overview;
use crate::workflow_panel::render_workflow_panel;
use crate::workflow_runs::{
    execute_and_record_workflow, list_workflow_runs, load_latest_workflow_run, load_workflow_run,
    render_workflow_run_inspect_panel, render_workflow_run_list,
};
use crate::workflows::{
    initialize_workflow, list_workflows, load_named_workflow_detail, render_workflow_detail,
    render_workflow_list, render_workflow_validation, resolve_named_workflow,
    validate_named_workflow, validate_workflows, WorkflowRunTarget,
};

use super::commands::WorkflowCommand;

pub(super) async fn handle_workflow_command(
    command: WorkflowCommand,
    session: &mut AgentSession,
) -> Result<String> {
    match command {
        WorkflowCommand::List => {
            let workflows = list_workflows(session.working_directory())?;
            Ok(render_workflow_list(
                session.working_directory(),
                &workflows,
            ))
        }
        WorkflowCommand::Overview { workflow_name } => {
            render_workflow_overview(session.working_directory(), workflow_name.as_deref())
        }
        WorkflowCommand::Panel {
            workflow_name,
            step_number,
        } => render_workflow_panel(
            session.working_directory(),
            workflow_name.as_deref(),
            step_number,
        ),
        WorkflowCommand::Runs { workflow_name } => {
            let runs =
                list_workflow_runs(session.working_directory(), workflow_name.as_deref(), 20)?;
            Ok(render_workflow_run_list(
                session.working_directory(),
                &runs,
                workflow_name.as_deref(),
            ))
        }
        WorkflowCommand::Validate { workflow_name } => {
            let results = match workflow_name {
                Some(workflow_name) => vec![validate_named_workflow(
                    session.working_directory(),
                    &workflow_name,
                )?],
                None => validate_workflows(session.working_directory())?,
            };
            Ok(render_workflow_validation(
                &results,
                session.working_directory(),
            ))
        }
        WorkflowCommand::ShowRun { run_id: None } => {
            Ok("Usage: /workflow show-run <run-id>".to_string())
        }
        WorkflowCommand::ShowRun { run_id: Some(run_id) } => Ok(render_workflow_run_inspect_panel(
            session.working_directory(),
            &load_workflow_run(session.working_directory(), &run_id)?,
        )),
        WorkflowCommand::LastRun { workflow_name } => Ok(render_workflow_run_inspect_panel(
            session.working_directory(),
            &load_latest_workflow_run(session.working_directory(), workflow_name.as_deref())?,
        )),
        WorkflowCommand::Show { workflow_name } => {
            let workflow_name =
                workflow_name.ok_or_else(|| anyhow!("Usage: /workflow show <name>"))?;
            Ok(render_workflow_detail(&load_named_workflow_detail(
                session.working_directory(),
                &workflow_name,
            )?))
        }
        WorkflowCommand::Init { workflow_name } => {
            let workflow_name =
                workflow_name.ok_or_else(|| anyhow!("Usage: /workflow init <name>"))?;
            let path = initialize_workflow(
                session.working_directory(),
                &workflow_name,
                None,
                false,
                false,
            )?;
            Ok(format!(
                "Initialized workflow `{}` at `{}`.",
                workflow_name,
                path.display().to_string().replace('\\', "/")
            ))
        }
        WorkflowCommand::AddStep {
            workflow_name: None,
            ..
        } => Ok(
            "Usage: /workflow add-step <name> --prompt <text> [--name <step-name>] [--index <n>] [--when <json>] [--model <name>] [--backend <name>] [--step-cwd <path>] [--background]"
                .to_string(),
        ),
        WorkflowCommand::AddStep {
            workflow_name: Some(_),
            prompt: None,
            ..
        } => Ok(
            "Usage: /workflow add-step <name> --prompt <text> [--name <step-name>] [--index <n>] [--when <json>] [--model <name>] [--backend <name>] [--step-cwd <path>] [--background]"
                .to_string(),
        ),
        WorkflowCommand::AddStep {
            workflow_name: Some(workflow_name),
            name,
            prompt: Some(prompt),
            index,
            when,
            model,
            backend,
            step_cwd,
            run_in_background,
        } => {
            let path = resolve_existing_workflow_path(session.working_directory(), &workflow_name)?;
            let result = add_workflow_step(
                session.working_directory(),
                &path,
                WorkflowStepDraft {
                    name,
                    prompt,
                    when,
                    model,
                    backend,
                    step_cwd,
                    run_in_background,
                },
                index,
            )?;
            Ok(format!(
                "Added workflow step {}.\n{}",
                result.step_number,
                render_workflow_detail(&result.detail)
            ))
        }
        WorkflowCommand::UpdateStep {
            workflow_name: None,
            ..
        } => Ok(
            "Usage: /workflow update-step <name> <step-number> [--name <step-name>|--clear-name] [--prompt <text>] [--when <json>|--clear-when] [--model <name>|--clear-model] [--backend <name>|--clear-backend] [--step-cwd <path>|--clear-step-cwd] [--background|--foreground]"
                .to_string(),
        ),
        WorkflowCommand::UpdateStep {
            workflow_name: Some(_),
            step_number: None,
            ..
        } => Ok(
            "Usage: /workflow update-step <name> <step-number> [--name <step-name>|--clear-name] [--prompt <text>] [--when <json>|--clear-when] [--model <name>|--clear-model] [--backend <name>|--clear-backend] [--step-cwd <path>|--clear-step-cwd] [--background|--foreground]"
                .to_string(),
        ),
        WorkflowCommand::UpdateStep {
            workflow_name: Some(workflow_name),
            step_number: Some(step_number),
            name,
            clear_name,
            prompt,
            when,
            clear_when,
            model,
            clear_model,
            backend,
            clear_backend,
            step_cwd,
            clear_step_cwd,
            run_in_background,
        } => {
            let path = resolve_existing_workflow_path(session.working_directory(), &workflow_name)?;
            let patch = WorkflowStepPatch {
                name: merge_optional_field(name, clear_name),
                prompt,
                when: merge_optional_field(when, clear_when),
                model: merge_optional_field(model, clear_model),
                backend: merge_optional_field(backend, clear_backend),
                step_cwd: merge_optional_field(step_cwd, clear_step_cwd),
                run_in_background,
            };
            let detail = update_workflow_step(session.working_directory(), &path, step_number, patch)?;
            Ok(format!(
                "Updated workflow step {step_number}.\n{}",
                render_workflow_detail(&detail)
            ))
        }
        WorkflowCommand::RemoveStep {
            workflow_name: None,
            ..
        } => Ok("Usage: /workflow remove-step <name> <step-number>".to_string()),
        WorkflowCommand::RemoveStep {
            workflow_name: Some(_),
            step_number: None,
        } => Ok("Usage: /workflow remove-step <name> <step-number>".to_string()),
        WorkflowCommand::RemoveStep {
            workflow_name: Some(workflow_name),
            step_number: Some(step_number),
        } => {
            let path = resolve_existing_workflow_path(session.working_directory(), &workflow_name)?;
            let result = remove_workflow_step(session.working_directory(), &path, step_number)?;
            Ok(format!(
                "Removed workflow step {step_number}.\n{}",
                render_workflow_detail(&result.detail)
            ))
        }
        WorkflowCommand::SetSharedContext {
            workflow_name: None,
            ..
        } => Ok("Usage: /workflow set-shared-context <name> <text>".to_string()),
        WorkflowCommand::SetSharedContext {
            workflow_name: Some(workflow_name),
            value,
        } => {
            let path = resolve_existing_workflow_path(session.working_directory(), &workflow_name)?;
            let detail = set_workflow_shared_context(session.working_directory(), &path, value)?;
            Ok(format!(
                "Updated shared_context.\n{}",
                render_workflow_detail(&detail)
            ))
        }
        WorkflowCommand::ClearSharedContext {
            workflow_name: None,
        } => Ok("Usage: /workflow clear-shared-context <name>".to_string()),
        WorkflowCommand::ClearSharedContext {
            workflow_name: Some(workflow_name),
        } => {
            let path = resolve_existing_workflow_path(session.working_directory(), &workflow_name)?;
            let detail = set_workflow_shared_context(session.working_directory(), &path, None)?;
            Ok(format!(
                "Cleared shared_context.\n{}",
                render_workflow_detail(&detail)
            ))
        }
        WorkflowCommand::EnableContinueOnError {
            workflow_name: None,
        } => Ok("Usage: /workflow enable-continue-on-error <name>".to_string()),
        WorkflowCommand::EnableContinueOnError {
            workflow_name: Some(workflow_name),
        } => {
            let path = resolve_existing_workflow_path(session.working_directory(), &workflow_name)?;
            let detail =
                set_workflow_continue_on_error(session.working_directory(), &path, true)?;
            Ok(format!(
                "Enabled continue_on_error.\n{}",
                render_workflow_detail(&detail)
            ))
        }
        WorkflowCommand::DisableContinueOnError {
            workflow_name: None,
        } => Ok("Usage: /workflow disable-continue-on-error <name>".to_string()),
        WorkflowCommand::DisableContinueOnError {
            workflow_name: Some(workflow_name),
        } => {
            let path = resolve_existing_workflow_path(session.working_directory(), &workflow_name)?;
            let detail =
                set_workflow_continue_on_error(session.working_directory(), &path, false)?;
            Ok(format!(
                "Disabled continue_on_error.\n{}",
                render_workflow_detail(&detail)
            ))
        }
        WorkflowCommand::Run {
            workflow_name,
            shared_context,
        } => {
            let workflow_name = workflow_name
                .ok_or_else(|| anyhow!("Usage: /workflow run <name> [shared_context]"))?;
            execute_and_record_workflow(
                session,
                WorkflowRunTarget::Named(workflow_name),
                shared_context,
                None,
            )
            .await
        }
        WorkflowCommand::Help => Ok(workflow_help_text()),
    }
}

pub(super) fn resolve_dynamic_workflow_invocation(
    input: &str,
    root: &Path,
) -> Result<Option<(String, Option<String>)>> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return Ok(None);
    }

    let body = trimmed.trim_start_matches('/').trim();
    if body.is_empty() {
        return Ok(None);
    }

    let (candidate, remainder) = match body.find(char::is_whitespace) {
        Some(index) => (&body[..index], Some(body[index..].trim())),
        None => (body, None),
    };

    let Some(workflow_name) = resolve_named_workflow(root, candidate)? else {
        return Ok(None);
    };

    let shared_context = remainder
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);

    Ok(Some((workflow_name, shared_context)))
}

pub(super) fn workflow_help_text() -> String {
    [
        "Workflow commands:",
        "  /workflow                 List project workflow scripts",
        "  /workflow overview [name] Show a selector-style workflow overview",
        "  /workflow panel [name] [n] Show an authoring panel with copyable edit actions",
        "  /workflow runs [name]     List recorded workflow runs",
        "  /workflow validate [name] Validate project workflow scripts",
        "  /workflow show-run <id>   Show a recorded workflow run",
        "  /workflow last-run [name] Show the latest recorded workflow run",
        "  /workflow show <name>     Show a workflow script definition",
        "  /workflow init <name>     Create a starter workflow script",
        "  /workflow add-step <name> --prompt <text> Add a workflow step",
        "  /workflow update-step <name> <n> ... Edit a workflow step",
        "  /workflow remove-step <name> <n> Remove a workflow step",
        "  /workflow set-shared-context <name> <text> Set workflow shared context",
        "  /workflow clear-shared-context <name> Clear workflow shared context",
        "  /workflow enable-continue-on-error <name> Enable continue_on_error",
        "  /workflow disable-continue-on-error <name> Disable continue_on_error",
        "  /workflow run <name> [shared_context] Run a workflow script locally",
        "  /workflow <name> [shared_context] Shortcut for `/workflow run ...`",
    ]
    .join("\n")
}

fn merge_optional_field(value: Option<String>, clear: bool) -> Option<Option<String>> {
    if clear {
        Some(None)
    } else {
        value.map(Some)
    }
}
