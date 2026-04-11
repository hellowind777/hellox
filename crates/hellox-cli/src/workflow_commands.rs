use anyhow::Result;

use crate::cli_workflow_types::WorkflowCommands;
use crate::workflow_command_authoring::handle_workflow_authoring_command;
use crate::workflow_command_support::{
    build_workflow_session, path_text, preferred_workflow_config_path, resolve_lookup_target,
    resolve_optional_lookup_target, resolve_script_path, workflow_command_cwd, workspace_root,
    WorkflowLookupTarget,
};
use crate::workflow_dashboard::{
    initial_workflow_dashboard_state, render_workflow_dashboard_state, run_workflow_dashboard_loop,
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
    render_workflow_validation, validate_explicit_workflow_path, validate_named_workflow,
    validate_workflows, WorkflowRunTarget,
};

pub(crate) async fn handle_workflow_command(command: WorkflowCommands) -> Result<()> {
    match command {
        WorkflowCommands::Dashboard { workflow_name, cwd } => {
            let root = workspace_root(cwd)?;
            run_workflow_dashboard_loop(&root, workflow_name).await
        }
        other => {
            println!("{}", workflow_command_text(other).await?);
            Ok(())
        }
    }
}

pub(crate) async fn workflow_command_text(command: WorkflowCommands) -> Result<String> {
    let root = workspace_root(workflow_command_cwd(&command).cloned())?;
    if let Some(rendered) = handle_workflow_authoring_command(&root, &command)? {
        return Ok(rendered);
    }

    match command {
        WorkflowCommands::List { .. } => {
            let workflows = list_workflows(&root)?;
            Ok(render_workflow_list(&root, &workflows))
        }
        WorkflowCommands::Dashboard { workflow_name, .. } => {
            let mut state = initial_workflow_dashboard_state(workflow_name);
            render_workflow_dashboard_state(&root, &mut state)
        }
        WorkflowCommands::Overview { workflow_name, .. } => {
            render_workflow_overview(&root, workflow_name.as_deref())
        }
        WorkflowCommands::Panel {
            workflow_name,
            script_path,
            step,
            ..
        } => match resolve_optional_lookup_target(workflow_name, script_path, "workflow panel")? {
            Some(WorkflowLookupTarget::Named(name)) => {
                render_workflow_panel(&root, Some(&name), step)
            }
            Some(WorkflowLookupTarget::Path(path)) => {
                let detail =
                    load_workflow_detail_from_path(&root, &resolve_script_path(&root, path), None)?;
                render_workflow_panel_detail(&root, &detail, step)
            }
            None => render_workflow_panel(&root, None, step),
        },
        WorkflowCommands::Runs {
            workflow_name,
            limit,
            ..
        } => {
            let runs = list_workflow_runs(&root, workflow_name.as_deref(), limit)?;
            Ok(render_workflow_run_list(
                &root,
                &runs,
                workflow_name.as_deref(),
            ))
        }
        WorkflowCommands::Validate {
            workflow_name,
            script_path,
            ..
        } => {
            let results = match resolve_optional_lookup_target(
                workflow_name,
                script_path,
                "workflow validate",
            )? {
                Some(WorkflowLookupTarget::Named(name)) => {
                    vec![validate_named_workflow(&root, &name)?]
                }
                Some(WorkflowLookupTarget::Path(path)) => {
                    vec![validate_explicit_workflow_path(
                        &root,
                        &resolve_script_path(&root, path),
                    )?]
                }
                None => validate_workflows(&root)?,
            };
            Ok(render_workflow_validation(&results, &root))
        }
        WorkflowCommands::ShowRun { run_id, step, .. } => {
            Ok(render_workflow_run_inspect_panel_with_step(
                &root,
                &load_workflow_run(&root, &run_id)?,
                step,
            ))
        }
        WorkflowCommands::LastRun {
            workflow_name,
            step,
            ..
        } => Ok(render_workflow_run_inspect_panel_with_step(
            &root,
            &load_latest_workflow_run(&root, workflow_name.as_deref())?,
            step,
        )),
        WorkflowCommands::Show {
            workflow_name,
            script_path,
            ..
        } => match resolve_lookup_target(workflow_name, script_path, "workflow show")? {
            WorkflowLookupTarget::Named(name) => Ok(render_workflow_detail(
                &load_named_workflow_detail(&root, &name)?,
            )),
            WorkflowLookupTarget::Path(path) => Ok(render_workflow_detail(
                &load_workflow_detail_from_path(&root, &resolve_script_path(&root, path), None)?,
            )),
        },
        WorkflowCommands::Init {
            workflow_name,
            shared_context,
            continue_on_error,
            force,
            ..
        } => {
            let path = initialize_workflow(
                &root,
                &workflow_name,
                shared_context,
                continue_on_error,
                force,
            )?;
            Ok(format!(
                "Initialized workflow `{}` at `{}`.",
                workflow_name,
                path_text(&path)
            ))
        }
        WorkflowCommands::Run {
            workflow_name,
            script_path,
            shared_context,
            continue_on_error,
            config,
            ..
        } => {
            let target = match resolve_lookup_target(workflow_name, script_path, "workflow run")? {
                WorkflowLookupTarget::Named(name) => WorkflowRunTarget::Named(name),
                WorkflowLookupTarget::Path(path) => {
                    WorkflowRunTarget::Path(resolve_script_path(&root, path))
                }
            };
            let session = build_workflow_session(
                config.or_else(|| preferred_workflow_config_path(&root)),
                root,
            )?;
            execute_and_record_workflow(
                &session,
                target,
                shared_context,
                continue_on_error.then_some(true),
            )
            .await
        }
        WorkflowCommands::AddStep { .. }
        | WorkflowCommands::UpdateStep { .. }
        | WorkflowCommands::DuplicateStep { .. }
        | WorkflowCommands::MoveStep { .. }
        | WorkflowCommands::RemoveStep { .. }
        | WorkflowCommands::SetSharedContext { .. }
        | WorkflowCommands::ClearSharedContext { .. }
        | WorkflowCommands::EnableContinueOnError { .. }
        | WorkflowCommands::DisableContinueOnError { .. } => unreachable!(),
    }
}
