use std::path::Path;

use anyhow::Result;

use crate::cli_workflow_types::WorkflowCommands;
use crate::workflow_authoring::{
    add_workflow_step, duplicate_workflow_step, move_workflow_step, remove_workflow_step,
    resolve_existing_workflow_path, set_workflow_continue_on_error, set_workflow_shared_context,
    update_workflow_step, WorkflowStepDraft, WorkflowStepPatch,
};
use crate::workflow_command_support::{
    merge_background_flags, merge_optional_field, path_text, resolve_lookup_path,
};
use crate::workflows::render_workflow_detail;

pub(crate) fn handle_workflow_authoring_command(
    root: &Path,
    command: &WorkflowCommands,
) -> Result<Option<String>> {
    let rendered = match command {
        WorkflowCommands::AddStep {
            workflow_name,
            script_path,
            name,
            prompt,
            index,
            when,
            model,
            backend,
            step_cwd,
            run_in_background,
            ..
        } => {
            let path = resolve_lookup_path(
                root,
                workflow_name.clone(),
                script_path.clone(),
                "workflow add-step",
                |name| resolve_existing_workflow_path(root, name),
            )?;
            let result = add_workflow_step(
                root,
                &path,
                WorkflowStepDraft {
                    name: name.clone(),
                    prompt: prompt.clone(),
                    when: when.clone(),
                    model: model.clone(),
                    backend: backend.clone(),
                    step_cwd: step_cwd.clone(),
                    run_in_background: *run_in_background,
                },
                *index,
            )?;
            Some(format!(
                "Added workflow step {} at `{}`.\n{}",
                result.step_number,
                path_text(&result.detail.summary.path),
                render_workflow_detail(&result.detail)
            ))
        }
        WorkflowCommands::UpdateStep {
            workflow_name,
            step_number,
            script_path,
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
            foreground,
            ..
        } => {
            let path = resolve_lookup_path(
                root,
                workflow_name.clone(),
                script_path.clone(),
                "workflow update-step",
                |name| resolve_existing_workflow_path(root, name),
            )?;
            let patch = WorkflowStepPatch {
                name: merge_optional_field(name.clone(), *clear_name),
                prompt: prompt.clone(),
                when: merge_optional_field(when.clone(), *clear_when),
                model: merge_optional_field(model.clone(), *clear_model),
                backend: merge_optional_field(backend.clone(), *clear_backend),
                step_cwd: merge_optional_field(step_cwd.clone(), *clear_step_cwd),
                run_in_background: merge_background_flags(*run_in_background, *foreground)?,
            };
            let detail = update_workflow_step(root, &path, *step_number, patch)?;
            Some(format!(
                "Updated workflow step {} at `{}`.\n{}",
                step_number,
                path_text(&detail.summary.path),
                render_workflow_detail(&detail)
            ))
        }
        WorkflowCommands::DuplicateStep {
            workflow_name,
            step_number,
            script_path,
            to_step_number,
            name,
            ..
        } => {
            let path = resolve_lookup_path(
                root,
                workflow_name.clone(),
                script_path.clone(),
                "workflow duplicate-step",
                |workflow_name| resolve_existing_workflow_path(root, workflow_name),
            )?;
            let result =
                duplicate_workflow_step(root, &path, *step_number, *to_step_number, name.clone())?;
            let duplicated_name = result
                .duplicated_step_name
                .as_deref()
                .unwrap_or("(unnamed)");
            Some(format!(
                "Duplicated workflow step {} into step {} (`{}`) at `{}`.\n{}",
                step_number,
                result.step_number,
                duplicated_name,
                path_text(&result.detail.summary.path),
                render_workflow_detail(&result.detail)
            ))
        }
        WorkflowCommands::MoveStep {
            workflow_name,
            step_number,
            script_path,
            to_step_number,
            ..
        } => {
            let path = resolve_lookup_path(
                root,
                workflow_name.clone(),
                script_path.clone(),
                "workflow move-step",
                |workflow_name| resolve_existing_workflow_path(root, workflow_name),
            )?;
            let result = move_workflow_step(root, &path, *step_number, *to_step_number)?;
            let moved_name = result.moved_step_name.as_deref().unwrap_or("(unnamed)");
            Some(format!(
                "Moved workflow step {} (`{}`) to step {} in `{}`.\n{}",
                step_number,
                moved_name,
                result.step_number,
                path_text(&result.detail.summary.path),
                render_workflow_detail(&result.detail)
            ))
        }
        WorkflowCommands::RemoveStep {
            workflow_name,
            step_number,
            script_path,
            ..
        } => {
            let path = resolve_lookup_path(
                root,
                workflow_name.clone(),
                script_path.clone(),
                "workflow remove-step",
                |name| resolve_existing_workflow_path(root, name),
            )?;
            let result = remove_workflow_step(root, &path, *step_number)?;
            let removed_name = result.removed_step_name.as_deref().unwrap_or("(unnamed)");
            Some(format!(
                "Removed workflow step {} (`{}`) from `{}`.\n{}",
                step_number,
                removed_name,
                path_text(&result.detail.summary.path),
                render_workflow_detail(&result.detail)
            ))
        }
        WorkflowCommands::SetSharedContext {
            workflow_name,
            value,
            script_path,
            ..
        } => {
            let path = resolve_lookup_path(
                root,
                workflow_name.clone(),
                script_path.clone(),
                "workflow set-shared-context",
                |name| resolve_existing_workflow_path(root, name),
            )?;
            let detail = set_workflow_shared_context(root, &path, value.clone())?;
            Some(format!(
                "Updated shared_context for `{}`.\n{}",
                path_text(&detail.summary.path),
                render_workflow_detail(&detail)
            ))
        }
        WorkflowCommands::ClearSharedContext {
            workflow_name,
            script_path,
            ..
        } => {
            let path = resolve_lookup_path(
                root,
                workflow_name.clone(),
                script_path.clone(),
                "workflow clear-shared-context",
                |name| resolve_existing_workflow_path(root, name),
            )?;
            let detail = set_workflow_shared_context(root, &path, None)?;
            Some(format!(
                "Cleared shared_context for `{}`.\n{}",
                path_text(&detail.summary.path),
                render_workflow_detail(&detail)
            ))
        }
        WorkflowCommands::EnableContinueOnError {
            workflow_name,
            script_path,
            ..
        } => {
            let path = resolve_lookup_path(
                root,
                workflow_name.clone(),
                script_path.clone(),
                "workflow enable-continue-on-error",
                |name| resolve_existing_workflow_path(root, name),
            )?;
            let detail = set_workflow_continue_on_error(root, &path, true)?;
            Some(format!(
                "Enabled continue_on_error for `{}`.\n{}",
                path_text(&detail.summary.path),
                render_workflow_detail(&detail)
            ))
        }
        WorkflowCommands::DisableContinueOnError {
            workflow_name,
            script_path,
            ..
        } => {
            let path = resolve_lookup_path(
                root,
                workflow_name.clone(),
                script_path.clone(),
                "workflow disable-continue-on-error",
                |name| resolve_existing_workflow_path(root, name),
            )?;
            let detail = set_workflow_continue_on_error(root, &path, false)?;
            Some(format!(
                "Disabled continue_on_error for `{}`.\n{}",
                path_text(&detail.summary.path),
                render_workflow_detail(&detail)
            ))
        }
        _ => None,
    };

    Ok(rendered)
}
