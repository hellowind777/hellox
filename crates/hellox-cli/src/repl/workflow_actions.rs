use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use hellox_agent::AgentSession;

use crate::workflow_authoring::{
    add_workflow_step, duplicate_workflow_step, move_workflow_step, remove_workflow_step,
    resolve_existing_workflow_path, set_workflow_continue_on_error, set_workflow_shared_context,
    update_workflow_step, WorkflowStepDraft, WorkflowStepPatch,
};
use crate::workflow_command_support::{
    resolve_lookup_path, resolve_lookup_target, resolve_optional_lookup_target,
    resolve_script_path, WorkflowLookupTarget,
};
use crate::workflow_dashboard::{
    initial_workflow_dashboard_state, render_workflow_dashboard_state,
};
use crate::workflow_overview::render_workflow_overview;
use crate::workflow_panel::{render_workflow_panel, render_workflow_panel_detail};
use crate::workflow_runs::{
    execute_and_record_workflow, list_workflow_runs, load_latest_workflow_run, load_workflow_run,
    render_workflow_run_inspect_panel_with_step, render_workflow_run_list,
};
use crate::workflows::{
    initialize_workflow, list_workflows, load_named_workflow_detail,
    load_workflow_detail_from_path, render_workflow_detail, render_workflow_list,
    render_workflow_validation, resolve_named_workflow, validate_explicit_workflow_path,
    validate_named_workflow, validate_workflows, WorkflowRunTarget,
};

use super::commands::WorkflowCommand;
use super::workflow_support::{merge_optional_field, workflow_help_text};

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
        WorkflowCommand::Dashboard { workflow_name } => {
            let mut state = initial_workflow_dashboard_state(workflow_name);
            render_workflow_dashboard_state(session.working_directory(), &mut state)
        }
        WorkflowCommand::Overview { workflow_name } => {
            render_workflow_overview(session.working_directory(), workflow_name.as_deref())
        }
        WorkflowCommand::Panel {
            workflow_name,
            script_path,
            step_number,
        } => match resolve_optional_lookup_target(
            workflow_name,
            script_path.map(PathBuf::from),
            "workflow panel",
        )? {
            Some(WorkflowLookupTarget::Named(workflow_name)) => render_workflow_panel(
                session.working_directory(),
                Some(&workflow_name),
                step_number,
            ),
            Some(WorkflowLookupTarget::Path(path)) => {
                let detail = load_workflow_detail_from_path(
                    session.working_directory(),
                    &resolve_script_path(session.working_directory(), path),
                    None,
                )?;
                render_workflow_panel_detail(session.working_directory(), &detail, step_number)
            }
            None => render_workflow_panel(session.working_directory(), None, step_number),
        },
        WorkflowCommand::Runs { workflow_name } => {
            let runs =
                list_workflow_runs(session.working_directory(), workflow_name.as_deref(), 20)?;
            Ok(render_workflow_run_list(
                session.working_directory(),
                &runs,
                workflow_name.as_deref(),
            ))
        }
        WorkflowCommand::Validate {
            workflow_name,
            script_path,
        } => {
            let results = match resolve_optional_lookup_target(
                workflow_name,
                script_path.map(PathBuf::from),
                "workflow validate",
            )? {
                Some(WorkflowLookupTarget::Named(workflow_name)) => vec![validate_named_workflow(
                    session.working_directory(),
                    &workflow_name,
                )?],
                Some(WorkflowLookupTarget::Path(path)) => vec![validate_explicit_workflow_path(
                    session.working_directory(),
                    &resolve_script_path(session.working_directory(), path),
                )?],
                None => validate_workflows(session.working_directory())?,
            };
            Ok(render_workflow_validation(
                &results,
                session.working_directory(),
            ))
        }
        WorkflowCommand::ShowRun { run_id: None, .. } => {
            Ok("Usage: /workflow show-run <run-id> [step-number]".to_string())
        }
        WorkflowCommand::ShowRun {
            run_id: Some(run_id),
            step_number,
        } => Ok(render_workflow_run_inspect_panel_with_step(
            session.working_directory(),
            &load_workflow_run(session.working_directory(), &run_id)?,
            step_number,
        )),
        WorkflowCommand::LastRun {
            workflow_name,
            step_number,
        } => Ok(render_workflow_run_inspect_panel_with_step(
            session.working_directory(),
            &load_latest_workflow_run(session.working_directory(), workflow_name.as_deref())?,
            step_number,
        )),
        WorkflowCommand::Show {
            workflow_name,
            script_path,
        } => match resolve_lookup_target(
            workflow_name,
            script_path.map(PathBuf::from),
            "workflow show",
        )? {
            WorkflowLookupTarget::Named(workflow_name) => Ok(render_workflow_detail(
                &load_named_workflow_detail(session.working_directory(), &workflow_name)?,
            )),
            WorkflowLookupTarget::Path(path) => Ok(render_workflow_detail(
                &load_workflow_detail_from_path(
                    session.working_directory(),
                    &resolve_script_path(session.working_directory(), path),
                    None,
                )?,
            )),
        },
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
            script_path: None,
            ..
        } => Ok(
            "Usage: /workflow add-step <name> --prompt <text> [...] | /workflow add-step --script-path <path> --prompt <text> [...]"
                .to_string(),
        ),
        WorkflowCommand::AddStep {
            workflow_name: Some(_),
            script_path: Some(_),
            ..
        } => Ok(
            "Usage: choose either `<name>` or `--script-path <path>` for /workflow add-step"
                .to_string(),
        ),
        WorkflowCommand::AddStep {
            workflow_name: Some(_),
            script_path: None,
            prompt: None,
            ..
        }
        | WorkflowCommand::AddStep {
            workflow_name: None,
            script_path: Some(_),
            prompt: None,
            ..
        } => Ok(
            "Usage: /workflow add-step <name> --prompt <text> [...] | /workflow add-step --script-path <path> --prompt <text> [...]"
                .to_string(),
        ),
        WorkflowCommand::AddStep {
            workflow_name,
            script_path,
            name,
            prompt: Some(prompt),
            index,
            when,
            model,
            backend,
            step_cwd,
            run_in_background,
        } => {
            let path = resolve_lookup_path(
                session.working_directory(),
                workflow_name,
                script_path.map(PathBuf::from),
                "workflow add-step",
                |name| resolve_existing_workflow_path(session.working_directory(), name),
            )?;
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
            script_path: None,
            ..
        } => Ok(
            "Usage: /workflow update-step <name> <step-number> [...] | /workflow update-step --script-path <path> <step-number> [...]"
                .to_string(),
        ),
        WorkflowCommand::UpdateStep {
            workflow_name: Some(_),
            script_path: Some(_),
            ..
        } => Ok(
            "Usage: choose either `<name>` or `--script-path <path>` for /workflow update-step"
                .to_string(),
        ),
        WorkflowCommand::UpdateStep {
            workflow_name: Some(_),
            script_path: None,
            step_number: None,
            ..
        }
        | WorkflowCommand::UpdateStep {
            workflow_name: None,
            script_path: Some(_),
            step_number: None,
            ..
        } => Ok(
            "Usage: /workflow update-step <name> <step-number> [...] | /workflow update-step --script-path <path> <step-number> [...]"
                .to_string(),
        ),
        WorkflowCommand::UpdateStep {
            workflow_name,
            script_path,
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
            let path = resolve_lookup_path(
                session.working_directory(),
                workflow_name,
                script_path.map(PathBuf::from),
                "workflow update-step",
                |name| resolve_existing_workflow_path(session.working_directory(), name),
            )?;
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
        WorkflowCommand::DuplicateStep {
            workflow_name: None,
            script_path: None,
            ..
        } => Ok(
            "Usage: /workflow duplicate-step <name> <step-number> [--to <n>] [--name <step-name>] | /workflow duplicate-step --script-path <path> <step-number> [--to <n>] [--name <step-name>]"
                .to_string(),
        ),
        WorkflowCommand::DuplicateStep {
            workflow_name: Some(_),
            script_path: Some(_),
            ..
        } => Ok(
            "Usage: choose either `<name>` or `--script-path <path>` for /workflow duplicate-step"
                .to_string(),
        ),
        WorkflowCommand::DuplicateStep {
            workflow_name: Some(_),
            script_path: None,
            step_number: None,
            ..
        }
        | WorkflowCommand::DuplicateStep {
            workflow_name: None,
            script_path: Some(_),
            step_number: None,
            ..
        } => Ok(
            "Usage: /workflow duplicate-step <name> <step-number> [--to <n>] [--name <step-name>] | /workflow duplicate-step --script-path <path> <step-number> [--to <n>] [--name <step-name>]"
                .to_string(),
        ),
        WorkflowCommand::DuplicateStep {
            workflow_name,
            script_path,
            step_number: Some(step_number),
            to_step_number,
            name,
        } => {
            let path = resolve_lookup_path(
                session.working_directory(),
                workflow_name,
                script_path.map(PathBuf::from),
                "workflow duplicate-step",
                |name| resolve_existing_workflow_path(session.working_directory(), name),
            )?;
            let result = duplicate_workflow_step(
                session.working_directory(),
                &path,
                step_number,
                to_step_number,
                name,
            )?;
            let duplicated_name = result
                .duplicated_step_name
                .as_deref()
                .unwrap_or("(unnamed)");
            Ok(format!(
                "Duplicated workflow step {step_number} into step {} (`{duplicated_name}`).\n{}",
                result.step_number,
                render_workflow_detail(&result.detail)
            ))
        }
        WorkflowCommand::MoveStep {
            workflow_name: None,
            script_path: None,
            ..
        } => Ok(
            "Usage: /workflow move-step <name> <step-number> --to <n> | /workflow move-step --script-path <path> <step-number> --to <n>"
                .to_string(),
        ),
        WorkflowCommand::MoveStep {
            workflow_name: Some(_),
            script_path: Some(_),
            ..
        } => Ok(
            "Usage: choose either `<name>` or `--script-path <path>` for /workflow move-step"
                .to_string(),
        ),
        WorkflowCommand::MoveStep {
            workflow_name: Some(_),
            script_path: None,
            step_number: None,
            ..
        }
        | WorkflowCommand::MoveStep {
            workflow_name: None,
            script_path: Some(_),
            step_number: None,
            ..
        } => Ok(
            "Usage: /workflow move-step <name> <step-number> --to <n> | /workflow move-step --script-path <path> <step-number> --to <n>"
                .to_string(),
        ),
        WorkflowCommand::MoveStep {
            workflow_name: Some(_),
            script_path: None,
            step_number: Some(_),
            to_step_number: None,
        }
        | WorkflowCommand::MoveStep {
            workflow_name: None,
            script_path: Some(_),
            step_number: Some(_),
            to_step_number: None,
        } => Ok(
            "Usage: /workflow move-step <name> <step-number> --to <n> | /workflow move-step --script-path <path> <step-number> --to <n>"
                .to_string(),
        ),
        WorkflowCommand::MoveStep {
            workflow_name,
            script_path,
            step_number: Some(step_number),
            to_step_number: Some(to_step_number),
        } => {
            let path = resolve_lookup_path(
                session.working_directory(),
                workflow_name,
                script_path.map(PathBuf::from),
                "workflow move-step",
                |name| resolve_existing_workflow_path(session.working_directory(), name),
            )?;
            let result = move_workflow_step(
                session.working_directory(),
                &path,
                step_number,
                to_step_number,
            )?;
            let moved_name = result.moved_step_name.as_deref().unwrap_or("(unnamed)");
            Ok(format!(
                "Moved workflow step {step_number} (`{moved_name}`) to step {}.\n{}",
                result.step_number,
                render_workflow_detail(&result.detail)
            ))
        }
        WorkflowCommand::RemoveStep {
            workflow_name: None,
            script_path: None,
            ..
        } => Ok(
            "Usage: /workflow remove-step <name> <step-number> | /workflow remove-step --script-path <path> <step-number>"
                .to_string(),
        ),
        WorkflowCommand::RemoveStep {
            workflow_name: Some(_),
            script_path: Some(_),
            ..
        } => Ok(
            "Usage: choose either `<name>` or `--script-path <path>` for /workflow remove-step"
                .to_string(),
        ),
        WorkflowCommand::RemoveStep {
            workflow_name: Some(_),
            script_path: None,
            step_number: None,
        }
        | WorkflowCommand::RemoveStep {
            workflow_name: None,
            script_path: Some(_),
            step_number: None,
        } => Ok(
            "Usage: /workflow remove-step <name> <step-number> | /workflow remove-step --script-path <path> <step-number>"
                .to_string(),
        ),
        WorkflowCommand::RemoveStep {
            workflow_name,
            script_path,
            step_number: Some(step_number),
        } => {
            let path = resolve_lookup_path(
                session.working_directory(),
                workflow_name,
                script_path.map(PathBuf::from),
                "workflow remove-step",
                |name| resolve_existing_workflow_path(session.working_directory(), name),
            )?;
            let result = remove_workflow_step(session.working_directory(), &path, step_number)?;
            Ok(format!(
                "Removed workflow step {step_number}.\n{}",
                render_workflow_detail(&result.detail)
            ))
        }
        WorkflowCommand::SetSharedContext {
            workflow_name: None,
            script_path: None,
            ..
        } => Ok(
            "Usage: /workflow set-shared-context <name> <text> | /workflow set-shared-context --script-path <path> <text>"
                .to_string(),
        ),
        WorkflowCommand::SetSharedContext {
            workflow_name: Some(_),
            script_path: Some(_),
            ..
        } => Ok(
            "Usage: choose either `<name>` or `--script-path <path>` for /workflow set-shared-context"
                .to_string(),
        ),
        WorkflowCommand::SetSharedContext {
            workflow_name,
            script_path,
            value,
        } => {
            let path = resolve_lookup_path(
                session.working_directory(),
                workflow_name,
                script_path.map(PathBuf::from),
                "workflow set-shared-context",
                |name| resolve_existing_workflow_path(session.working_directory(), name),
            )?;
            let detail = set_workflow_shared_context(session.working_directory(), &path, value)?;
            Ok(format!(
                "Updated shared_context.\n{}",
                render_workflow_detail(&detail)
            ))
        }
        WorkflowCommand::ClearSharedContext {
            workflow_name: None,
            script_path: None,
        } => Ok(
            "Usage: /workflow clear-shared-context <name> | /workflow clear-shared-context --script-path <path>"
                .to_string(),
        ),
        WorkflowCommand::ClearSharedContext {
            workflow_name: Some(_),
            script_path: Some(_),
        } => Ok(
            "Usage: choose either `<name>` or `--script-path <path>` for /workflow clear-shared-context"
                .to_string(),
        ),
        WorkflowCommand::ClearSharedContext {
            workflow_name,
            script_path,
        } => {
            let path = resolve_lookup_path(
                session.working_directory(),
                workflow_name,
                script_path.map(PathBuf::from),
                "workflow clear-shared-context",
                |name| resolve_existing_workflow_path(session.working_directory(), name),
            )?;
            let detail = set_workflow_shared_context(session.working_directory(), &path, None)?;
            Ok(format!(
                "Cleared shared_context.\n{}",
                render_workflow_detail(&detail)
            ))
        }
        WorkflowCommand::EnableContinueOnError {
            workflow_name: None,
            script_path: None,
        } => Ok(
            "Usage: /workflow enable-continue-on-error <name> | /workflow enable-continue-on-error --script-path <path>"
                .to_string(),
        ),
        WorkflowCommand::EnableContinueOnError {
            workflow_name: Some(_),
            script_path: Some(_),
        } => Ok(
            "Usage: choose either `<name>` or `--script-path <path>` for /workflow enable-continue-on-error"
                .to_string(),
        ),
        WorkflowCommand::EnableContinueOnError {
            workflow_name,
            script_path,
        } => {
            let path = resolve_lookup_path(
                session.working_directory(),
                workflow_name,
                script_path.map(PathBuf::from),
                "workflow enable-continue-on-error",
                |name| resolve_existing_workflow_path(session.working_directory(), name),
            )?;
            let detail =
                set_workflow_continue_on_error(session.working_directory(), &path, true)?;
            Ok(format!(
                "Enabled continue_on_error.\n{}",
                render_workflow_detail(&detail)
            ))
        }
        WorkflowCommand::DisableContinueOnError {
            workflow_name: None,
            script_path: None,
        } => Ok(
            "Usage: /workflow disable-continue-on-error <name> | /workflow disable-continue-on-error --script-path <path>"
                .to_string(),
        ),
        WorkflowCommand::DisableContinueOnError {
            workflow_name: Some(_),
            script_path: Some(_),
        } => Ok(
            "Usage: choose either `<name>` or `--script-path <path>` for /workflow disable-continue-on-error"
                .to_string(),
        ),
        WorkflowCommand::DisableContinueOnError {
            workflow_name,
            script_path,
        } => {
            let path = resolve_lookup_path(
                session.working_directory(),
                workflow_name,
                script_path.map(PathBuf::from),
                "workflow disable-continue-on-error",
                |name| resolve_existing_workflow_path(session.working_directory(), name),
            )?;
            let detail =
                set_workflow_continue_on_error(session.working_directory(), &path, false)?;
            Ok(format!(
                "Disabled continue_on_error.\n{}",
                render_workflow_detail(&detail)
            ))
        }
        WorkflowCommand::Run {
            workflow_name: None,
            script_path: None,
            ..
        } => Ok(
            "Usage: /workflow run <name> [shared_context] | /workflow run --script-path <path> [shared_context]"
                .to_string(),
        ),
        WorkflowCommand::Run {
            workflow_name: Some(_),
            script_path: Some(_),
            ..
        } => Ok(
            "Usage: choose either `<name>` or `--script-path <path>` for /workflow run"
                .to_string(),
        ),
        WorkflowCommand::Run {
            workflow_name,
            script_path,
            shared_context,
        } => {
            let target = match resolve_lookup_target(
                workflow_name,
                script_path.map(PathBuf::from),
                "workflow run",
            )? {
                WorkflowLookupTarget::Named(workflow_name) => {
                    WorkflowRunTarget::Named(workflow_name)
                }
                WorkflowLookupTarget::Path(path) => {
                    WorkflowRunTarget::Path(resolve_script_path(session.working_directory(), path))
                }
            };
            execute_and_record_workflow(
                session,
                target,
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
