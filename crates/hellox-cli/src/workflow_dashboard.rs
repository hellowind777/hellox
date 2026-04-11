use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use hellox_tui::{
    parse_workflow_dashboard_command, workflow_dashboard_help_text, WorkflowDashboardCommand,
    WorkflowDashboardOpenTarget, WorkflowDashboardState, WorkflowDashboardView,
};
use serde_json::Value;

use crate::workflow_authoring::{
    add_workflow_step, duplicate_workflow_step, move_workflow_step, remove_workflow_step,
    resolve_existing_workflow_path, set_workflow_continue_on_error, set_workflow_shared_context,
    update_workflow_step, WorkflowStepDraft, WorkflowStepPatch,
};
use crate::workflow_command_support::{
    build_workflow_session, merge_optional_field, path_text, preferred_workflow_config_path,
    resolve_optional_lookup_target, resolve_script_path, WorkflowLookupTarget,
};
use crate::workflow_overview::{
    list_workflow_focus_selection_items, list_workflow_focus_selection_items_for_path,
    list_workflow_overview_selection_items, render_workflow_overview,
    render_workflow_overview_for_path, WorkflowOverviewFocusSelectionItem,
    WorkflowOverviewSelectionItem,
};
use crate::workflow_panel::{
    list_workflow_panel_selection_items, render_workflow_panel,
    render_workflow_panel_detail_with_target, WorkflowPanelSelectionItem,
};
use crate::workflow_runs::{
    execute_and_record_workflow, list_workflow_runs, load_latest_workflow_run, load_workflow_run,
    render_workflow_run_inspect_panel_with_step, render_workflow_run_list,
    select_workflow_run_step_number, WORKFLOW_RUN_SELECTOR_PREVIEW_LIMIT,
};
use crate::workflow_step_navigation::{
    execute_workflow_step_navigation, parse_workflow_step_navigation, WorkflowStepNavigationResult,
    WorkflowStepNavigationShortcut,
};
use crate::workflow_step_shortcuts::{
    execute_workflow_step_shortcut_for_path, parse_workflow_step_shortcut,
};
use crate::workflows::{
    initialize_workflow, list_workflows, load_named_workflow_detail,
    load_workflow_detail_from_path, render_workflow_detail, render_workflow_validation,
    validate_explicit_workflow_path, validate_named_workflow, validate_workflows,
    WorkflowRunTarget, WorkflowScriptDetail,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkflowDashboardHandleOutcome {
    NotHandled,
    Print(String),
    RunActiveWorkflow {
        target: WorkflowRunTarget,
        target_label: String,
        shared_context: Option<String>,
    },
    Close,
    Quit,
}

struct RenderedWorkflowDashboard {
    text: String,
    open_targets: Vec<WorkflowDashboardOpenTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DashboardWorkflowTarget {
    Named(String),
    Path(PathBuf),
}

impl DashboardWorkflowTarget {
    fn run_target(&self) -> WorkflowRunTarget {
        match self {
            Self::Named(workflow_name) => WorkflowRunTarget::Named(workflow_name.clone()),
            Self::Path(path) => WorkflowRunTarget::Path(path.clone()),
        }
    }

    fn label(&self) -> String {
        match self {
            Self::Named(workflow_name) => workflow_name.clone(),
            Self::Path(path) => path_text(path),
        }
    }

    fn edit_path(&self, root: &Path) -> Result<PathBuf> {
        match self {
            Self::Named(workflow_name) => resolve_existing_workflow_path(root, workflow_name),
            Self::Path(path) => Ok(path.clone()),
        }
    }

    fn panel_view(&self, step_number: Option<usize>) -> WorkflowDashboardView {
        match self {
            Self::Named(workflow_name) => WorkflowDashboardView::PanelFocus {
                workflow_name: workflow_name.clone(),
                step_number,
            },
            Self::Path(path) => WorkflowDashboardView::PanelPathFocus {
                script_path: path_text(path),
                step_number,
            },
        }
    }

    fn overview_view(&self) -> WorkflowDashboardView {
        match self {
            Self::Named(workflow_name) => WorkflowDashboardView::OverviewFocus {
                workflow_name: workflow_name.clone(),
            },
            Self::Path(path) => WorkflowDashboardView::OverviewPathFocus {
                script_path: path_text(path),
            },
        }
    }

    fn panel_hint(&self) -> String {
        match self {
            Self::Named(workflow_name) => format!("panel {workflow_name} <n>"),
            Self::Path(path) => format!("panel --script-path {} <n>", path_text(path)),
        }
    }
}

pub(crate) fn initial_workflow_dashboard_state(
    workflow_name: Option<String>,
    script_path: Option<String>,
) -> WorkflowDashboardState {
    match (
        normalize_optional_text(workflow_name),
        normalize_optional_text(script_path),
    ) {
        (Some(workflow_name), None) => {
            WorkflowDashboardState::new(WorkflowDashboardView::OverviewFocus { workflow_name })
        }
        (None, Some(script_path)) => {
            WorkflowDashboardState::new(WorkflowDashboardView::OverviewPathFocus { script_path })
        }
        _ => WorkflowDashboardState::new(WorkflowDashboardView::OverviewList),
    }
}

pub(crate) fn render_workflow_dashboard_state(
    root: &Path,
    state: &mut WorkflowDashboardState,
) -> Result<String> {
    let rendered = render_workflow_dashboard_view(root, state.current())?;
    state.set_open_targets(rendered.open_targets);
    let footer = match state.current() {
        WorkflowDashboardView::PanelFocus { .. }
        | WorkflowDashboardView::PanelPathFocus { .. }
        | WorkflowDashboardView::RunInspect { .. } => {
            "open <n> | next | prev | first | last | back | help | quit"
        }
        _ => "open <n> | back | help | quit",
    };
    Ok(format!("{}\n\n== Dashboard ==\n{footer}", rendered.text,))
}

pub(crate) fn handle_workflow_dashboard_input(
    root: &Path,
    state: &mut WorkflowDashboardState,
    input: &str,
) -> Result<WorkflowDashboardHandleOutcome> {
    let Some(command) = parse_workflow_dashboard_command(input) else {
        return Ok(WorkflowDashboardHandleOutcome::NotHandled);
    };

    let outcome = match command {
        WorkflowDashboardCommand::Unknown => {
            match handle_contextual_step_shortcut(root, state, input)? {
                Some(outcome) => Ok(outcome),
                None => Ok(WorkflowDashboardHandleOutcome::NotHandled),
            }
        }
        WorkflowDashboardCommand::Error(message) => {
            Ok(WorkflowDashboardHandleOutcome::Print(message))
        }
        WorkflowDashboardCommand::Help => Ok(WorkflowDashboardHandleOutcome::Print(
            workflow_dashboard_help_text(),
        )),
        WorkflowDashboardCommand::Close => Ok(WorkflowDashboardHandleOutcome::Close),
        WorkflowDashboardCommand::Quit => Ok(WorkflowDashboardHandleOutcome::Quit),
        WorkflowDashboardCommand::Back => back_and_render(root, state),
        WorkflowDashboardCommand::Open { index } => open_and_render(root, state, index),
        WorkflowDashboardCommand::Overview {
            workflow_name,
            script_path,
        } => {
            let view = match resolve_optional_lookup_target(
                workflow_name,
                script_path.map(PathBuf::from),
                "dashboard overview",
            )? {
                Some(WorkflowLookupTarget::Named(workflow_name)) => {
                    WorkflowDashboardView::OverviewFocus { workflow_name }
                }
                Some(WorkflowLookupTarget::Path(path)) => {
                    WorkflowDashboardView::OverviewPathFocus {
                        script_path: path_text(&resolve_script_path(root, path)),
                    }
                }
                None => WorkflowDashboardView::OverviewList,
            };
            navigate_and_render(root, state, view)
        }
        WorkflowDashboardCommand::Show {
            workflow_name,
            script_path,
        } => {
            let target = resolve_dashboard_command_target(
                root,
                workflow_name,
                script_path,
                active_workflow_target(root, state),
                "show",
            )?;
            Ok(WorkflowDashboardHandleOutcome::Print(match target {
                Some(DashboardWorkflowTarget::Named(workflow_name)) => {
                    render_workflow_detail(&load_named_workflow_detail(root, &workflow_name)?)
                }
                Some(DashboardWorkflowTarget::Path(path)) => {
                    render_workflow_detail(&load_workflow_detail_from_path(root, &path, None)?)
                }
                None => {
                    return Err(anyhow!(
                        "Usage: show <workflow-name> | show --script-path <path>"
                    ))
                }
            }))
        }
        WorkflowDashboardCommand::Run { shared_context } => {
            let target = active_workflow_target(root, state).ok_or_else(|| {
                anyhow!(
                    "Run a focused workflow first via `overview <name>`, `overview --script-path <path>`, `panel <name>`, or `panel --script-path <path>`."
                )
            })?;
            Ok(WorkflowDashboardHandleOutcome::RunActiveWorkflow {
                target_label: target.label(),
                target: target.run_target(),
                shared_context: normalize_optional_text(shared_context),
            })
        }
        WorkflowDashboardCommand::Panel {
            workflow_name,
            script_path,
            step_number,
        } => match resolve_dashboard_command_target(
            root,
            workflow_name,
            script_path,
            active_workflow_target(root, state),
            "panel",
        )? {
            Some(DashboardWorkflowTarget::Named(workflow_name)) => navigate_and_render(
                root,
                state,
                WorkflowDashboardView::PanelFocus {
                    workflow_name,
                    step_number,
                },
            ),
            Some(DashboardWorkflowTarget::Path(path)) => navigate_and_render(
                root,
                state,
                WorkflowDashboardView::PanelPathFocus {
                    script_path: path_text(&path),
                    step_number,
                },
            ),
            None if step_number.is_some() => Ok(WorkflowDashboardHandleOutcome::Print(
                "Usage: panel <workflow-name> [step-number]".to_string(),
            )),
            None => navigate_and_render(root, state, WorkflowDashboardView::PanelList),
        },
        WorkflowDashboardCommand::Runs {
            workflow_name,
            script_path,
        } => match resolve_dashboard_command_target(
            root,
            workflow_name,
            script_path,
            active_workflow_target(root, state),
            "runs",
        )? {
            Some(DashboardWorkflowTarget::Named(workflow_name)) => navigate_and_render(
                root,
                state,
                WorkflowDashboardView::Runs {
                    workflow_name: Some(workflow_name),
                },
            ),
            Some(DashboardWorkflowTarget::Path(path)) => navigate_and_render(
                root,
                state,
                WorkflowDashboardView::RunsPath {
                    script_path: path_text(&path),
                },
            ),
            None => navigate_and_render(
                root,
                state,
                WorkflowDashboardView::Runs {
                    workflow_name: None,
                },
            ),
        },
        WorkflowDashboardCommand::Validate {
            workflow_name,
            script_path,
        } => {
            let target = resolve_dashboard_command_target(
                root,
                workflow_name,
                script_path,
                active_workflow_target(root, state),
                "validate",
            )?;
            let results = match target {
                Some(DashboardWorkflowTarget::Named(workflow_name)) => {
                    vec![validate_named_workflow(root, &workflow_name)?]
                }
                Some(DashboardWorkflowTarget::Path(path)) => {
                    vec![validate_explicit_workflow_path(root, &path)?]
                }
                None => validate_workflows(root)?,
            };
            Ok(WorkflowDashboardHandleOutcome::Print(
                render_workflow_validation(&results, root),
            ))
        }
        WorkflowDashboardCommand::Init { workflow_name } => {
            let workflow_name = normalize_optional_text(workflow_name)
                .ok_or_else(|| anyhow!("Usage: init <name>"))?;
            let path = initialize_workflow(root, &workflow_name, None, false, false)?;
            let text = replace_and_render(
                root,
                state,
                WorkflowDashboardView::OverviewFocus {
                    workflow_name: workflow_name.clone(),
                },
            )?;
            Ok(WorkflowDashboardHandleOutcome::Print(format!(
                "Initialized workflow `{workflow_name}` at `{}`.\n\n{text}",
                path_text(&path)
            )))
        }
        WorkflowDashboardCommand::AddStep {
            name,
            prompt,
            index,
            when,
            model,
            backend,
            step_cwd,
            run_in_background,
        } => add_step_to_active_workflow(
            root,
            state,
            name,
            prompt,
            index,
            when,
            model,
            backend,
            step_cwd,
            run_in_background,
        ),
        WorkflowDashboardCommand::UpdateStep {
            step_number,
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
        } => update_active_workflow_step(
            root,
            state,
            step_number,
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
        ),
        WorkflowDashboardCommand::ShowRun {
            run_id,
            step_number,
        } => match normalize_optional_text(run_id) {
            Some(run_id) => navigate_and_render(
                root,
                state,
                WorkflowDashboardView::RunInspect {
                    run_id,
                    step_number,
                },
            ),
            None => Ok(WorkflowDashboardHandleOutcome::Print(
                "Usage: show-run <run-id> [step-number]".to_string(),
            )),
        },
        WorkflowDashboardCommand::LastRun {
            workflow_name,
            script_path,
            step_number,
        } => {
            let filter = resolve_dashboard_command_target(
                root,
                workflow_name,
                script_path,
                active_workflow_target(root, state),
                "last-run",
            )?
            .map(|target| target.run_target());
            let record = load_latest_workflow_run(root, filter.as_ref())?;
            navigate_and_render(
                root,
                state,
                WorkflowDashboardView::RunInspect {
                    run_id: record.run_id,
                    step_number,
                },
            )
        }
        WorkflowDashboardCommand::SetSharedContext { value } => {
            let target = active_workflow_target(root, state)
                .ok_or_else(|| anyhow!("Set shared context from a focused workflow view first."))?;
            let path = target.edit_path(root)?;
            set_workflow_shared_context(root, &path, value)?;
            refresh_active_workflow_view(root, state, &target, "Updated shared_context.")
        }
        WorkflowDashboardCommand::ClearSharedContext => {
            let target = active_workflow_target(root, state).ok_or_else(|| {
                anyhow!("Clear shared context from a focused workflow view first.")
            })?;
            let path = target.edit_path(root)?;
            set_workflow_shared_context(root, &path, None)?;
            refresh_active_workflow_view(root, state, &target, "Cleared shared_context.")
        }
        WorkflowDashboardCommand::EnableContinueOnError => {
            let target = active_workflow_target(root, state).ok_or_else(|| {
                anyhow!("Enable continue_on_error from a focused workflow view first.")
            })?;
            let path = target.edit_path(root)?;
            set_workflow_continue_on_error(root, &path, true)?;
            refresh_active_workflow_view(root, state, &target, "Enabled continue_on_error.")
        }
        WorkflowDashboardCommand::DisableContinueOnError => {
            let target = active_workflow_target(root, state).ok_or_else(|| {
                anyhow!("Disable continue_on_error from a focused workflow view first.")
            })?;
            let path = target.edit_path(root)?;
            set_workflow_continue_on_error(root, &path, false)?;
            refresh_active_workflow_view(root, state, &target, "Disabled continue_on_error.")
        }
        WorkflowDashboardCommand::Duplicate { to_step_number } => {
            duplicate_current_panel_step(root, state, to_step_number)
        }
        WorkflowDashboardCommand::DuplicateStep {
            step_number,
            to_step_number,
            name,
        } => duplicate_active_workflow_step(root, state, step_number, to_step_number, name),
        WorkflowDashboardCommand::Move { to_step_number } => {
            move_current_panel_step(root, state, to_step_number)
        }
        WorkflowDashboardCommand::MoveStep {
            step_number,
            to_step_number,
        } => move_active_workflow_step(root, state, step_number, to_step_number),
        WorkflowDashboardCommand::Remove => remove_current_panel_step(root, state),
        WorkflowDashboardCommand::RemoveStep { step_number } => {
            remove_active_workflow_step(root, state, step_number)
        }
    };

    outcome.or_else(|error| Ok(WorkflowDashboardHandleOutcome::Print(error.to_string())))
}

pub(crate) async fn run_workflow_dashboard_loop(
    root: &Path,
    workflow_name: Option<String>,
    script_path: Option<String>,
) -> Result<()> {
    let mut state = initial_workflow_dashboard_state(workflow_name, script_path);
    println!("{}", render_workflow_dashboard_state(root, &mut state)?);

    loop {
        print!("workflow-dashboard> ");
        io::stdout().flush()?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break;
        }

        match handle_workflow_dashboard_input(root, &mut state, &input)? {
            WorkflowDashboardHandleOutcome::NotHandled => {
                println!(
                    "Unknown dashboard command. Use `help` to list workflow dashboard commands."
                );
            }
            WorkflowDashboardHandleOutcome::Print(text) => println!("{text}"),
            WorkflowDashboardHandleOutcome::RunActiveWorkflow {
                target,
                target_label,
                shared_context,
            } => {
                let session = build_workflow_session(
                    preferred_workflow_config_path(root),
                    root.to_path_buf(),
                )?;
                match execute_and_record_workflow(&session, target.clone(), shared_context, None)
                    .await
                {
                    Ok(result_text) => println!(
                        "{}",
                        complete_workflow_dashboard_run(
                            root,
                            &mut state,
                            &target,
                            &target_label,
                            &result_text
                        )?
                    ),
                    Err(error) => println!("{error}"),
                }
            }
            WorkflowDashboardHandleOutcome::Close | WorkflowDashboardHandleOutcome::Quit => break,
        }
    }

    Ok(())
}

pub(crate) fn complete_workflow_dashboard_run(
    root: &Path,
    state: &mut WorkflowDashboardState,
    target: &WorkflowRunTarget,
    target_label: &str,
    result_text: &str,
) -> Result<String> {
    let run_id = parse_recorded_workflow_run_id(result_text).or_else(|| {
        load_latest_workflow_run(root, Some(target))
            .ok()
            .map(|record| record.run_id)
    });

    match run_id {
        Some(run_id) => {
            let text = navigate_to_run_and_render(root, state, run_id)?;
            Ok(format!("Executed workflow `{target_label}`.\n\n{text}"))
        }
        None => Ok(format!(
            "Executed workflow `{target_label}`.\n\n{result_text}"
        )),
    }
}

fn render_workflow_dashboard_view(
    root: &Path,
    view: &WorkflowDashboardView,
) -> Result<RenderedWorkflowDashboard> {
    match view {
        WorkflowDashboardView::OverviewList => Ok(RenderedWorkflowDashboard {
            text: render_workflow_overview(root, None)?,
            open_targets: list_workflow_overview_selection_items(root)?
                .into_iter()
                .map(map_overview_selection_item)
                .collect(),
        }),
        WorkflowDashboardView::OverviewFocus { workflow_name } => {
            let _detail = load_named_workflow_detail(root, workflow_name)?;
            let items = list_workflow_focus_selection_items(root, workflow_name)?;
            Ok(RenderedWorkflowDashboard {
                text: render_workflow_overview(root, Some(workflow_name))?,
                open_targets: items
                    .into_iter()
                    .map(|item| {
                        map_focus_overview_selection_item(
                            &DashboardWorkflowTarget::Named(workflow_name.clone()),
                            item,
                        )
                    })
                    .collect(),
            })
        }
        WorkflowDashboardView::OverviewPathFocus { script_path } => {
            let resolved_path = resolve_script_path(root, PathBuf::from(script_path));
            let items = list_workflow_focus_selection_items_for_path(root, &resolved_path)?;
            Ok(RenderedWorkflowDashboard {
                text: render_workflow_overview_for_path(root, &resolved_path)?,
                open_targets: items
                    .into_iter()
                    .map(|item| {
                        map_focus_overview_selection_item(
                            &DashboardWorkflowTarget::Path(resolved_path.clone()),
                            item,
                        )
                    })
                    .collect(),
            })
        }
        WorkflowDashboardView::PanelList => {
            let workflows = list_workflows(root)?;
            Ok(RenderedWorkflowDashboard {
                text: render_workflow_panel(root, None, None)?,
                open_targets: workflows
                    .into_iter()
                    .map(|workflow| WorkflowDashboardOpenTarget::PanelWorkflow(workflow.name))
                    .collect(),
            })
        }
        WorkflowDashboardView::PanelFocus {
            workflow_name,
            step_number,
        } => {
            let _detail = load_named_workflow_detail(root, workflow_name)?;
            let items = list_workflow_panel_selection_items(root, workflow_name)?;
            Ok(RenderedWorkflowDashboard {
                text: render_workflow_panel(root, Some(workflow_name), *step_number)?,
                open_targets: items
                    .into_iter()
                    .map(|item| map_panel_selection_item(workflow_name, item))
                    .collect(),
            })
        }
        WorkflowDashboardView::PanelPathFocus {
            script_path,
            step_number,
        } => {
            let resolved_path = resolve_script_path(root, PathBuf::from(script_path));
            let detail = load_workflow_detail_from_path(root, &resolved_path, None)?;
            let runs = list_workflow_runs(
                root,
                Some(&WorkflowRunTarget::Path(resolved_path.clone())),
                WORKFLOW_RUN_SELECTOR_PREVIEW_LIMIT,
            )?;
            let items = build_path_panel_selection_items(&detail, runs);
            Ok(RenderedWorkflowDashboard {
                text: render_workflow_panel_detail_with_target(
                    root,
                    &detail,
                    &WorkflowRunTarget::Path(resolved_path.clone()),
                    *step_number,
                )?,
                open_targets: items
                    .into_iter()
                    .map(|item| map_path_panel_selection_item(&resolved_path, item))
                    .collect(),
            })
        }
        WorkflowDashboardView::Runs { workflow_name } => {
            let filter = workflow_name
                .as_ref()
                .map(|workflow_name| WorkflowRunTarget::Named(workflow_name.clone()));
            let runs =
                list_workflow_runs(root, filter.as_ref(), WORKFLOW_RUN_SELECTOR_PREVIEW_LIMIT)?;
            Ok(RenderedWorkflowDashboard {
                text: render_workflow_run_list(root, &runs, filter.as_ref()),
                open_targets: runs
                    .into_iter()
                    .map(|record| WorkflowDashboardOpenTarget::Run(record.run_id))
                    .collect(),
            })
        }
        WorkflowDashboardView::RunsPath { script_path } => {
            let resolved_path = resolve_script_path(root, PathBuf::from(script_path));
            let filter = WorkflowRunTarget::Path(resolved_path.clone());
            let runs =
                list_workflow_runs(root, Some(&filter), WORKFLOW_RUN_SELECTOR_PREVIEW_LIMIT)?;
            Ok(RenderedWorkflowDashboard {
                text: render_workflow_run_list(root, &runs, Some(&filter)),
                open_targets: runs
                    .into_iter()
                    .map(|record| WorkflowDashboardOpenTarget::Run(record.run_id))
                    .collect(),
            })
        }
        WorkflowDashboardView::RunInspect {
            run_id,
            step_number,
        } => {
            let record = load_workflow_run(root, run_id)?;
            Ok(RenderedWorkflowDashboard {
                text: render_workflow_run_inspect_panel_with_step(root, &record, *step_number),
                open_targets: record
                    .steps
                    .iter()
                    .enumerate()
                    .map(|(index, _)| WorkflowDashboardOpenTarget::RunStep {
                        run_id: record.run_id.clone(),
                        step_number: index + 1,
                    })
                    .collect(),
            })
        }
    }
}

fn navigate_and_render(
    root: &Path,
    state: &mut WorkflowDashboardState,
    next: WorkflowDashboardView,
) -> Result<WorkflowDashboardHandleOutcome> {
    let mut preview = state.clone();
    preview.navigate_to(next);
    let text = render_workflow_dashboard_state(root, &mut preview)?;
    *state = preview;
    Ok(WorkflowDashboardHandleOutcome::Print(text))
}

fn back_and_render(
    root: &Path,
    state: &mut WorkflowDashboardState,
) -> Result<WorkflowDashboardHandleOutcome> {
    let mut preview = state.clone();
    if !preview.back() {
        return Ok(WorkflowDashboardHandleOutcome::Print(
            "Already at the top-level workflow dashboard.".to_string(),
        ));
    }
    let text = render_workflow_dashboard_state(root, &mut preview)?;
    *state = preview;
    Ok(WorkflowDashboardHandleOutcome::Print(text))
}

fn open_and_render(
    root: &Path,
    state: &mut WorkflowDashboardState,
    index: usize,
) -> Result<WorkflowDashboardHandleOutcome> {
    let mut preview = state.clone();
    if let Err(message) = preview.open(index) {
        return Ok(WorkflowDashboardHandleOutcome::Print(message));
    }
    let text = render_workflow_dashboard_state(root, &mut preview)?;
    *state = preview;
    Ok(WorkflowDashboardHandleOutcome::Print(text))
}

fn handle_contextual_step_shortcut(
    root: &Path,
    state: &mut WorkflowDashboardState,
    input: &str,
) -> Result<Option<WorkflowDashboardHandleOutcome>> {
    if let Some(shortcut) = parse_workflow_step_navigation(input) {
        let shortcut = match shortcut {
            Ok(shortcut) => shortcut,
            Err(usage) => return Ok(Some(WorkflowDashboardHandleOutcome::Print(usage))),
        };

        let outcome = match state.current() {
            WorkflowDashboardView::PanelFocus { .. }
            | WorkflowDashboardView::PanelPathFocus { .. } => {
                let (target, selected_step, step_count) = current_panel_step_context(root, state)?;
                let result =
                    match execute_workflow_step_navigation(selected_step, step_count, shortcut) {
                        Ok(result) => result,
                        Err(message) => {
                            return Ok(Some(WorkflowDashboardHandleOutcome::Print(message)));
                        }
                    };
                let text =
                    replace_and_render(root, state, target.panel_view(Some(result.step_number)))?;
                Some(WorkflowDashboardHandleOutcome::Print(format!(
                    "{}\n\n{text}",
                    navigation_message(shortcut, "workflow step", step_count, result)
                )))
            }
            WorkflowDashboardView::RunInspect { .. } => {
                let (run_id, selected_step, step_count) = current_run_step_context(root, state)?;
                let result =
                    match execute_workflow_step_navigation(selected_step, step_count, shortcut) {
                        Ok(result) => result,
                        Err(message) => {
                            return Ok(Some(WorkflowDashboardHandleOutcome::Print(message)));
                        }
                    };
                let text = replace_and_render(
                    root,
                    state,
                    WorkflowDashboardView::RunInspect {
                        run_id,
                        step_number: Some(result.step_number),
                    },
                )?;
                Some(WorkflowDashboardHandleOutcome::Print(format!(
                    "{}\n\n{text}",
                    navigation_message(shortcut, "recorded step", step_count, result)
                )))
            }
            _ => None,
        };

        if outcome.is_some() {
            return Ok(outcome);
        }
    }

    if !matches!(
        state.current(),
        WorkflowDashboardView::PanelFocus { .. } | WorkflowDashboardView::PanelPathFocus { .. }
    ) {
        return Ok(None);
    }
    let Some(shortcut) = parse_workflow_step_shortcut(input) else {
        return Ok(None);
    };
    let shortcut = match shortcut {
        Ok(shortcut) => shortcut,
        Err(usage) => return Ok(Some(WorkflowDashboardHandleOutcome::Print(usage))),
    };

    let (target, selected_step) = current_panel_focus(root, state)?;
    let result = execute_workflow_step_shortcut_for_path(
        root,
        &target.edit_path(root)?,
        selected_step,
        shortcut,
    )?;
    let text = replace_and_render(root, state, target.panel_view(result.selected_step))?;
    Ok(Some(WorkflowDashboardHandleOutcome::Print(format!(
        "{}\n\n{text}",
        result.message
    ))))
}

fn navigate_to_run_and_render(
    root: &Path,
    state: &mut WorkflowDashboardState,
    run_id: String,
) -> Result<String> {
    let mut preview = state.clone();
    preview.navigate_to(WorkflowDashboardView::RunInspect {
        run_id,
        step_number: None,
    });
    let text = render_workflow_dashboard_state(root, &mut preview)?;
    *state = preview;
    Ok(text)
}

fn add_step_to_active_workflow(
    root: &Path,
    state: &mut WorkflowDashboardState,
    name: Option<String>,
    prompt: Option<String>,
    index: Option<usize>,
    when: Option<String>,
    model: Option<String>,
    backend: Option<String>,
    step_cwd: Option<String>,
    run_in_background: bool,
) -> Result<WorkflowDashboardHandleOutcome> {
    let target = active_workflow_target(root, state)
        .ok_or_else(|| anyhow!("Add a step from a focused workflow view first."))?;
    let prompt = normalize_optional_text(prompt).ok_or_else(|| {
        anyhow!(
            "Usage: add-step --prompt <text> [--name <step-name>] [--index <n>] [--when <json>] [--model <name>] [--backend <name>] [--step-cwd <path>] [--background]"
        )
    })?;
    let path = target.edit_path(root)?;
    let result = add_workflow_step(
        root,
        &path,
        WorkflowStepDraft {
            name: normalize_optional_text(name),
            prompt,
            when: normalize_optional_text(when),
            model: normalize_optional_text(model),
            backend: normalize_optional_text(backend),
            step_cwd: normalize_optional_text(step_cwd),
            run_in_background,
        },
        index,
    )?;
    let text = replace_and_render(root, state, target.panel_view(Some(result.step_number)))?;
    Ok(WorkflowDashboardHandleOutcome::Print(format!(
        "Added workflow step {}.\n\n{text}",
        result.step_number
    )))
}

#[allow(clippy::too_many_arguments)]
fn update_active_workflow_step(
    root: &Path,
    state: &mut WorkflowDashboardState,
    step_number: Option<usize>,
    name: Option<String>,
    clear_name: bool,
    prompt: Option<String>,
    when: Option<String>,
    clear_when: bool,
    model: Option<String>,
    clear_model: bool,
    backend: Option<String>,
    clear_backend: bool,
    step_cwd: Option<String>,
    clear_step_cwd: bool,
    run_in_background: Option<bool>,
) -> Result<WorkflowDashboardHandleOutcome> {
    let target = active_workflow_target(root, state)
        .ok_or_else(|| anyhow!("Update a step from a focused workflow view first."))?;
    let step_number = match step_number {
        Some(step_number) => step_number,
        None => current_panel_focus(root, state)?.1,
    };
    let path = target.edit_path(root)?;
    let _detail = update_workflow_step(
        root,
        &path,
        step_number,
        WorkflowStepPatch {
            name: merge_optional_field(normalize_optional_text(name), clear_name),
            prompt: normalize_optional_text(prompt),
            when: merge_optional_field(normalize_optional_text(when), clear_when),
            model: merge_optional_field(normalize_optional_text(model), clear_model),
            backend: merge_optional_field(normalize_optional_text(backend), clear_backend),
            step_cwd: merge_optional_field(normalize_optional_text(step_cwd), clear_step_cwd),
            run_in_background,
        },
    )?;
    let text = replace_and_render(root, state, target.panel_view(Some(step_number)))?;
    Ok(WorkflowDashboardHandleOutcome::Print(format!(
        "Updated workflow step {step_number}.\n\n{text}"
    )))
}

fn duplicate_current_panel_step(
    root: &Path,
    state: &mut WorkflowDashboardState,
    to_step_number: Option<usize>,
) -> Result<WorkflowDashboardHandleOutcome> {
    let (target, selected_step) = current_panel_focus(root, state)?;
    let path = target.edit_path(root)?;
    let result = duplicate_workflow_step(root, &path, selected_step, to_step_number, None)?;
    let duplicated_name = result
        .duplicated_step_name
        .as_deref()
        .unwrap_or("(unnamed)");
    let text = replace_and_render(root, state, target.panel_view(Some(result.step_number)))?;
    Ok(WorkflowDashboardHandleOutcome::Print(format!(
        "Duplicated workflow step {selected_step} into step {} (`{duplicated_name}`).\n\n{text}",
        result.step_number
    )))
}

fn duplicate_active_workflow_step(
    root: &Path,
    state: &mut WorkflowDashboardState,
    step_number: Option<usize>,
    to_step_number: Option<usize>,
    name: Option<String>,
) -> Result<WorkflowDashboardHandleOutcome> {
    let target = active_workflow_target(root, state)
        .ok_or_else(|| anyhow!("Duplicate a step from a focused workflow view first."))?;
    let selected_step = resolve_dashboard_step_number(root, state, &target, step_number)?;
    let path = target.edit_path(root)?;
    let result = duplicate_workflow_step(root, &path, selected_step, to_step_number, name)?;
    let duplicated_name = result
        .duplicated_step_name
        .as_deref()
        .unwrap_or("(unnamed)");
    let text = replace_and_render(root, state, target.panel_view(Some(result.step_number)))?;
    Ok(WorkflowDashboardHandleOutcome::Print(format!(
        "Duplicated workflow step {selected_step} into step {} (`{duplicated_name}`).\n\n{text}",
        result.step_number
    )))
}

fn move_current_panel_step(
    root: &Path,
    state: &mut WorkflowDashboardState,
    to_step_number: Option<usize>,
) -> Result<WorkflowDashboardHandleOutcome> {
    let to_step_number = to_step_number.ok_or_else(|| anyhow!("Usage: move <to-step-number>"))?;
    let (target, selected_step) = current_panel_focus(root, state)?;
    let path = target.edit_path(root)?;
    let result = move_workflow_step(root, &path, selected_step, to_step_number)?;
    let moved_name = result.moved_step_name.as_deref().unwrap_or("(unnamed)");
    let text = replace_and_render(root, state, target.panel_view(Some(result.step_number)))?;
    Ok(WorkflowDashboardHandleOutcome::Print(format!(
        "Moved workflow step {selected_step} (`{moved_name}`) to step {}.\n\n{text}",
        result.step_number
    )))
}

fn move_active_workflow_step(
    root: &Path,
    state: &mut WorkflowDashboardState,
    step_number: Option<usize>,
    to_step_number: Option<usize>,
) -> Result<WorkflowDashboardHandleOutcome> {
    let target = active_workflow_target(root, state)
        .ok_or_else(|| anyhow!("Move a step from a focused workflow view first."))?;
    let to_step_number =
        to_step_number.ok_or_else(|| anyhow!("Usage: move-step [step-number] --to <n>"))?;
    let selected_step = resolve_dashboard_step_number(root, state, &target, step_number)?;
    let path = target.edit_path(root)?;
    let result = move_workflow_step(root, &path, selected_step, to_step_number)?;
    let moved_name = result.moved_step_name.as_deref().unwrap_or("(unnamed)");
    let text = replace_and_render(root, state, target.panel_view(Some(result.step_number)))?;
    Ok(WorkflowDashboardHandleOutcome::Print(format!(
        "Moved workflow step {selected_step} (`{moved_name}`) to step {}.\n\n{text}",
        result.step_number
    )))
}

fn remove_current_panel_step(
    root: &Path,
    state: &mut WorkflowDashboardState,
) -> Result<WorkflowDashboardHandleOutcome> {
    let (target, selected_step) = current_panel_focus(root, state)?;
    let path = target.edit_path(root)?;
    let result = remove_workflow_step(root, &path, selected_step)?;
    let removed_name = result.removed_step_name.as_deref().unwrap_or("(unnamed)");
    let next_step =
        (!result.detail.steps.is_empty()).then_some(selected_step.min(result.detail.steps.len()));
    let text = replace_and_render(root, state, target.panel_view(next_step))?;
    Ok(WorkflowDashboardHandleOutcome::Print(format!(
        "Removed workflow step {selected_step} (`{removed_name}`).\n\n{text}"
    )))
}

fn remove_active_workflow_step(
    root: &Path,
    state: &mut WorkflowDashboardState,
    step_number: Option<usize>,
) -> Result<WorkflowDashboardHandleOutcome> {
    let target = active_workflow_target(root, state)
        .ok_or_else(|| anyhow!("Remove a step from a focused workflow view first."))?;
    let selected_step = resolve_dashboard_step_number(root, state, &target, step_number)?;
    let path = target.edit_path(root)?;
    let result = remove_workflow_step(root, &path, selected_step)?;
    let removed_name = result.removed_step_name.as_deref().unwrap_or("(unnamed)");
    let next_step =
        (!result.detail.steps.is_empty()).then_some(selected_step.min(result.detail.steps.len()));
    let text = replace_and_render(root, state, target.panel_view(next_step))?;
    Ok(WorkflowDashboardHandleOutcome::Print(format!(
        "Removed workflow step {selected_step} (`{removed_name}`).\n\n{text}"
    )))
}

fn replace_and_render(
    root: &Path,
    state: &mut WorkflowDashboardState,
    next: WorkflowDashboardView,
) -> Result<String> {
    let mut preview = state.clone();
    preview.replace_current(next);
    let text = render_workflow_dashboard_state(root, &mut preview)?;
    *state = preview;
    Ok(text)
}

fn refresh_active_workflow_view(
    root: &Path,
    state: &mut WorkflowDashboardState,
    target: &DashboardWorkflowTarget,
    message: &str,
) -> Result<WorkflowDashboardHandleOutcome> {
    let next = match state.current() {
        WorkflowDashboardView::PanelFocus { step_number, .. } => target.panel_view(*step_number),
        WorkflowDashboardView::OverviewFocus { .. }
        | WorkflowDashboardView::OverviewPathFocus { .. } => target.overview_view(),
        WorkflowDashboardView::PanelPathFocus { step_number, .. } => {
            target.panel_view(*step_number)
        }
        _ => target.panel_view(None),
    };
    let text = replace_and_render(root, state, next)?;
    Ok(WorkflowDashboardHandleOutcome::Print(format!(
        "{message}\n\n{text}"
    )))
}

fn current_panel_focus(
    root: &Path,
    state: &WorkflowDashboardState,
) -> Result<(DashboardWorkflowTarget, usize)> {
    let (target, selected_step, _) = current_panel_step_context(root, state)?;
    Ok((target, selected_step))
}

fn resolve_dashboard_step_number(
    root: &Path,
    state: &WorkflowDashboardState,
    target: &DashboardWorkflowTarget,
    step_number: Option<usize>,
) -> Result<usize> {
    match step_number {
        Some(step_number) => Ok(step_number),
        None => match state.current() {
            WorkflowDashboardView::PanelFocus { .. }
            | WorkflowDashboardView::PanelPathFocus { .. } => {
                current_panel_focus(root, state).map(|(_, step)| step)
            }
            _ => Err(anyhow!(
                "Provide a step number or open `{}` first.",
                target.panel_hint()
            )),
        },
    }
}

fn current_panel_step_context(
    root: &Path,
    state: &WorkflowDashboardState,
) -> Result<(DashboardWorkflowTarget, usize, usize)> {
    let (target, detail, step_number) = match state.current() {
        WorkflowDashboardView::PanelFocus {
            workflow_name,
            step_number,
        } => (
            DashboardWorkflowTarget::Named(workflow_name.clone()),
            load_named_workflow_detail(root, workflow_name)?,
            *step_number,
        ),
        WorkflowDashboardView::PanelPathFocus {
            script_path,
            step_number,
        } => {
            let resolved_path = resolve_script_path(root, PathBuf::from(script_path));
            (
                DashboardWorkflowTarget::Path(resolved_path.clone()),
                load_workflow_detail_from_path(root, &resolved_path, None)?,
                *step_number,
            )
        }
        _ => {
            return Err(anyhow!(
                "This dashboard action only works from a focused workflow panel. Use `panel <name>` or `panel --script-path <path>` first."
            ))
        }
    };

    if detail.steps.is_empty() {
        return Err(anyhow!(
            "workflow `{}` has no steps to edit yet",
            detail.summary.name
        ));
    }

    let selected_step = step_number.unwrap_or(1);
    if selected_step == 0 || selected_step > detail.steps.len() {
        return Err(anyhow!(
            "workflow panel step `{selected_step}` is out of range; expected 1..={}",
            detail.steps.len()
        ));
    }

    Ok((target, selected_step, detail.steps.len()))
}

fn current_run_step_context(
    root: &Path,
    state: &WorkflowDashboardState,
) -> Result<(String, usize, usize)> {
    let WorkflowDashboardView::RunInspect {
        run_id,
        step_number,
    } = state.current()
    else {
        return Err(anyhow!(
            "This dashboard action only works from a recorded workflow run. Use `show-run <id>` first."
        ));
    };

    let record = load_workflow_run(root, run_id)?;
    if record.steps.is_empty() {
        return Err(anyhow!(
            "workflow run `{}` has no recorded steps",
            record.run_id
        ));
    }

    let selected_step = select_workflow_run_step_number(&record, *step_number).unwrap_or(1);
    Ok((record.run_id, selected_step, record.steps.len()))
}

fn active_workflow_target(
    root: &Path,
    state: &WorkflowDashboardState,
) -> Option<DashboardWorkflowTarget> {
    match state.current() {
        WorkflowDashboardView::OverviewFocus { workflow_name }
        | WorkflowDashboardView::PanelFocus { workflow_name, .. } => {
            Some(DashboardWorkflowTarget::Named(workflow_name.clone()))
        }
        WorkflowDashboardView::OverviewPathFocus { script_path } => Some(
            DashboardWorkflowTarget::Path(resolve_script_path(root, PathBuf::from(script_path))),
        ),
        WorkflowDashboardView::PanelPathFocus { script_path, .. } => Some(
            DashboardWorkflowTarget::Path(resolve_script_path(root, PathBuf::from(script_path))),
        ),
        WorkflowDashboardView::Runs {
            workflow_name: Some(workflow_name),
        } => Some(DashboardWorkflowTarget::Named(workflow_name.clone())),
        WorkflowDashboardView::RunsPath { script_path } => Some(DashboardWorkflowTarget::Path(
            resolve_script_path(root, PathBuf::from(script_path)),
        )),
        WorkflowDashboardView::RunInspect { run_id, .. } => {
            load_workflow_run(root, run_id).ok().and_then(|record| {
                record
                    .workflow_name
                    .map(DashboardWorkflowTarget::Named)
                    .or_else(|| {
                        record
                            .requested_script_path
                            .or(record.workflow_source)
                            .map(|path| {
                                DashboardWorkflowTarget::Path(resolve_script_path(
                                    root,
                                    PathBuf::from(path),
                                ))
                            })
                    })
            })
        }
        WorkflowDashboardView::OverviewList
        | WorkflowDashboardView::PanelList
        | WorkflowDashboardView::Runs {
            workflow_name: None,
        } => None,
    }
}

fn map_overview_selection_item(item: WorkflowOverviewSelectionItem) -> WorkflowDashboardOpenTarget {
    match item {
        WorkflowOverviewSelectionItem::Workflow(workflow_name) => {
            WorkflowDashboardOpenTarget::OverviewWorkflow(workflow_name)
        }
        WorkflowOverviewSelectionItem::Run(run_id) => {
            WorkflowDashboardOpenTarget::OverviewRun(run_id)
        }
    }
}

fn map_focus_overview_selection_item(
    target: &DashboardWorkflowTarget,
    item: WorkflowOverviewFocusSelectionItem,
) -> WorkflowDashboardOpenTarget {
    match item {
        WorkflowOverviewFocusSelectionItem::Step(step_number) => match target {
            DashboardWorkflowTarget::Named(workflow_name) => {
                WorkflowDashboardOpenTarget::PanelStep {
                    workflow_name: workflow_name.clone(),
                    step_number,
                }
            }
            DashboardWorkflowTarget::Path(path) => WorkflowDashboardOpenTarget::PanelPathStep {
                script_path: path_text(path),
                step_number,
            },
        },
        WorkflowOverviewFocusSelectionItem::Run(run_id) => WorkflowDashboardOpenTarget::Run(run_id),
    }
}

fn map_panel_selection_item(
    workflow_name: &str,
    item: WorkflowPanelSelectionItem,
) -> WorkflowDashboardOpenTarget {
    match item {
        WorkflowPanelSelectionItem::Step(step_number) => WorkflowDashboardOpenTarget::PanelStep {
            workflow_name: workflow_name.to_string(),
            step_number,
        },
        WorkflowPanelSelectionItem::Run(run_id) => WorkflowDashboardOpenTarget::Run(run_id),
    }
}

fn map_path_panel_selection_item(
    script_path: &Path,
    item: WorkflowPanelSelectionItem,
) -> WorkflowDashboardOpenTarget {
    match item {
        WorkflowPanelSelectionItem::Step(step_number) => {
            WorkflowDashboardOpenTarget::PanelPathStep {
                script_path: path_text(script_path),
                step_number,
            }
        }
        WorkflowPanelSelectionItem::Run(run_id) => WorkflowDashboardOpenTarget::Run(run_id),
    }
}

fn build_path_panel_selection_items(
    detail: &WorkflowScriptDetail,
    runs: Vec<crate::workflow_runs::WorkflowRunRecord>,
) -> Vec<WorkflowPanelSelectionItem> {
    let mut items = (1..=detail.steps.len())
        .map(WorkflowPanelSelectionItem::Step)
        .collect::<Vec<_>>();
    items.extend(
        runs.into_iter()
            .map(|record| WorkflowPanelSelectionItem::Run(record.run_id)),
    );
    items
}

fn resolve_dashboard_command_target(
    root: &Path,
    workflow_name: Option<String>,
    script_path: Option<String>,
    active_target: Option<DashboardWorkflowTarget>,
    label: &str,
) -> Result<Option<DashboardWorkflowTarget>> {
    match resolve_optional_lookup_target(
        workflow_name,
        script_path.map(PathBuf::from),
        &format!("dashboard {label}"),
    )? {
        Some(WorkflowLookupTarget::Named(workflow_name)) => {
            Ok(Some(DashboardWorkflowTarget::Named(workflow_name)))
        }
        Some(WorkflowLookupTarget::Path(path)) => Ok(Some(DashboardWorkflowTarget::Path(
            resolve_script_path(root, path),
        ))),
        None => Ok(active_target),
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_recorded_workflow_run_id(result_text: &str) -> Option<String> {
    serde_json::from_str::<Value>(result_text)
        .ok()
        .and_then(|value| value.get("run_id").cloned())
        .and_then(|value| value.as_str().map(ToString::to_string))
        .filter(|value| !value.trim().is_empty())
}

fn navigation_message(
    shortcut: WorkflowStepNavigationShortcut,
    label: &str,
    step_count: usize,
    result: WorkflowStepNavigationResult,
) -> String {
    if result.changed {
        format!("Focused {label} {} of {step_count}.", result.step_number)
    } else {
        format!(
            "Already on the {} {label} ({} of {step_count}).",
            shortcut.boundary_name(),
            result.step_number
        )
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        complete_workflow_dashboard_run, handle_workflow_dashboard_input,
        initial_workflow_dashboard_state, path_text, render_workflow_dashboard_state,
        WorkflowDashboardHandleOutcome,
    };

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-cli-workflow-dashboard-{suffix}"));
        fs::create_dir_all(&root).expect("create temp dir");
        root
    }

    fn write_workflow(root: &Path, relative: &str, raw: &str) {
        let path = root.join(".hellox").join("workflows").join(relative);
        fs::create_dir_all(path.parent().expect("workflow dir")).expect("create workflow dir");
        fs::write(path, raw).expect("write workflow");
    }

    fn write_explicit_workflow(root: &Path, relative: &str, raw: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("workflow dir")).expect("create workflow dir");
        fs::write(path, raw).expect("write workflow");
    }

    fn write_run(root: &Path, run_id: &str, workflow_name: &str) {
        let path = root
            .join(".hellox")
            .join("workflow-runs")
            .join(format!("{run_id}.json"));
        fs::create_dir_all(path.parent().expect("run dir")).expect("create run dir");
        fs::write(
            path,
            serde_json::to_string_pretty(&serde_json::json!({
                "run_id": run_id,
                "status": "completed",
                "workflow_name": workflow_name,
                "workflow_source": format!(".hellox/workflows/{workflow_name}.json"),
                "started_at": 1,
                "finished_at": 2,
                "summary": {
                    "total_steps": 2,
                    "completed_steps": 2,
                    "failed_steps": 0,
                    "running_steps": 0,
                    "skipped_steps": 0
                },
                "steps": [
                    { "name": "review", "status": "completed", "result_text": "ok" },
                    { "name": "ship", "status": "completed", "result_text": "done" }
                ],
                "result_text": "done"
            }))
            .expect("serialize run"),
        )
        .expect("write run");
    }

    fn write_run_for_script_path(root: &Path, run_id: &str, script_path: &str) {
        let path = root
            .join(".hellox")
            .join("workflow-runs")
            .join(format!("{run_id}.json"));
        fs::create_dir_all(path.parent().expect("run dir")).expect("create run dir");
        fs::write(
            path,
            serde_json::to_string_pretty(&serde_json::json!({
                "run_id": run_id,
                "status": "completed",
                "workflow_name": null,
                "workflow_source": script_path,
                "requested_script_path": script_path,
                "started_at": 1,
                "finished_at": 2,
                "summary": {
                    "total_steps": 2,
                    "completed_steps": 2,
                    "failed_steps": 0,
                    "running_steps": 0,
                    "skipped_steps": 0
                },
                "steps": [
                    { "name": "review", "status": "completed", "result_text": "ok" },
                    { "name": "ship", "status": "completed", "result_text": "done" }
                ],
                "result_text": "done"
            }))
            .expect("serialize run"),
        )
        .expect("write run");
    }

    #[test]
    fn dashboard_initial_render_focuses_named_workflow() {
        let root = temp_dir();
        write_workflow(
            &root,
            "release-review.json",
            r#"{ "steps": [{ "name": "review", "prompt": "review release" }] }"#,
        );

        let mut state =
            initial_workflow_dashboard_state(Some(String::from("release-review")), None);
        let text = render_workflow_dashboard_state(&root, &mut state).expect("render dashboard");
        assert!(text.contains("Workflow overview: release-review"));
        assert!(text.contains("== Dashboard =="));
    }

    #[test]
    fn dashboard_initial_render_focuses_explicit_script_path() {
        let root = temp_dir();
        write_explicit_workflow(
            &root,
            "scripts/custom-release.json",
            r#"{
  "steps": [
    { "name": "review", "prompt": "review release" },
    { "name": "ship", "prompt": "ship release" }
  ]
}"#,
        );

        let mut state = initial_workflow_dashboard_state(
            None,
            Some(String::from("scripts/custom-release.json")),
        );
        let text = render_workflow_dashboard_state(&root, &mut state).expect("render dashboard");
        assert!(text.contains("Workflow overview: scripts/custom-release"));
        assert!(text.contains("focus: `/workflow panel --script-path"));
        assert!(text.contains("scripts/custom-release.json"));
        assert_eq!(
            state.current(),
            &hellox_tui::WorkflowDashboardView::OverviewPathFocus {
                script_path: String::from("scripts/custom-release.json"),
            }
        );
    }

    #[test]
    fn dashboard_overview_command_supports_explicit_script_path() {
        let root = temp_dir();
        write_explicit_workflow(
            &root,
            "scripts/custom-release.json",
            r#"{ "steps": [{ "name": "review", "prompt": "review release" }] }"#,
        );

        let mut state = initial_workflow_dashboard_state(None, None);
        let output = handle_workflow_dashboard_input(
            &root,
            &mut state,
            "overview --script-path scripts/custom-release.json",
        )
        .expect("open explicit workflow overview");
        match output {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Workflow overview: scripts/custom-release"));
            }
            other => panic!("expected explicit workflow overview output, got {other:?}"),
        }
        assert_eq!(
            state.current(),
            &hellox_tui::WorkflowDashboardView::OverviewPathFocus {
                script_path: path_text(&root.join("scripts").join("custom-release.json")),
            }
        );
    }

    #[test]
    fn dashboard_open_navigates_between_views() {
        let root = temp_dir();
        write_workflow(
            &root,
            "alpha.json",
            r#"{ "steps": [{ "prompt": "alpha" }] }"#,
        );
        write_workflow(
            &root,
            "release-review.json",
            r#"{ "steps": [{ "name": "review", "prompt": "review release" }] }"#,
        );

        let mut state = initial_workflow_dashboard_state(None, None);
        let text = render_workflow_dashboard_state(&root, &mut state).expect("render overview");
        assert!(text.contains("Workflow overview selector"));

        let output =
            handle_workflow_dashboard_input(&root, &mut state, "2").expect("open second workflow");
        match output {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Workflow overview: release-review"));
            }
            other => panic!("expected dashboard output, got {other:?}"),
        }
    }

    #[test]
    fn dashboard_authoring_commands_update_focused_panel() {
        let root = temp_dir();
        write_workflow(
            &root,
            "release-review.json",
            r#"{
  "steps": [
    { "name": "review", "prompt": "review release" },
    { "name": "ship", "prompt": "ship release" }
  ]
}"#,
        );

        let mut state =
            initial_workflow_dashboard_state(Some(String::from("release-review")), None);
        let _ = render_workflow_dashboard_state(&root, &mut state).expect("render overview");
        let _ = handle_workflow_dashboard_input(&root, &mut state, "panel 2")
            .expect("focus panel step");

        let duplicated =
            handle_workflow_dashboard_input(&root, &mut state, "dup").expect("duplicate step");
        match duplicated {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Duplicated workflow step 2 into step 3"));
            }
            other => panic!("expected dashboard duplicate output, got {other:?}"),
        }

        let moved =
            handle_workflow_dashboard_input(&root, &mut state, "move 1").expect("move step");
        match moved {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Moved workflow step 3"));
            }
            other => panic!("expected dashboard move output, got {other:?}"),
        }

        let removed =
            handle_workflow_dashboard_input(&root, &mut state, "rm").expect("remove step");
        match removed {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Removed workflow step 1"));
            }
            other => panic!("expected dashboard remove output, got {other:?}"),
        }
    }

    #[test]
    fn dashboard_explicit_step_commands_work_from_overview_focus() {
        let root = temp_dir();
        write_workflow(
            &root,
            "release-review.json",
            r#"{
  "steps": [
    { "name": "review", "prompt": "review release" },
    { "name": "ship", "prompt": "ship release" }
  ]
}"#,
        );

        let mut state =
            initial_workflow_dashboard_state(Some(String::from("release-review")), None);
        let _ = render_workflow_dashboard_state(&root, &mut state).expect("render dashboard");

        let duplicated = handle_workflow_dashboard_input(
            &root,
            &mut state,
            "duplicate-step 2 --to 1 --name ship copy",
        )
        .expect("duplicate explicit step");
        match duplicated {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Duplicated workflow step 2 into step 1 (`ship copy`)."));
                assert!(text.contains("Workflow authoring panel: release-review"));
            }
            other => panic!("expected dashboard duplicate-step output, got {other:?}"),
        }

        let moved = handle_workflow_dashboard_input(&root, &mut state, "move-step 3 --to 2")
            .expect("move explicit step");
        match moved {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Moved workflow step 3 (`ship`) to step 2."));
            }
            other => panic!("expected dashboard move-step output, got {other:?}"),
        }

        let removed = handle_workflow_dashboard_input(&root, &mut state, "remove-step 1")
            .expect("remove explicit step");
        match removed {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Removed workflow step 1 (`ship copy`)."));
            }
            other => panic!("expected dashboard remove-step output, got {other:?}"),
        }
    }

    #[test]
    fn dashboard_add_and_update_step_commands_refresh_focused_panel() {
        let root = temp_dir();
        write_workflow(
            &root,
            "release-review.json",
            r#"{
  "steps": [
    { "name": "review", "prompt": "review release" }
  ]
}"#,
        );

        let mut state =
            initial_workflow_dashboard_state(Some(String::from("release-review")), None);
        let _ = render_workflow_dashboard_state(&root, &mut state).expect("render dashboard");

        let added = handle_workflow_dashboard_input(
            &root,
            &mut state,
            "add-step --prompt summarize findings --name summarize --index 2 --background",
        )
        .expect("add dashboard step");
        match added {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Added workflow step 2"));
                assert!(text.contains("Workflow authoring panel: release-review"));
            }
            other => panic!("expected dashboard add-step output, got {other:?}"),
        }

        let updated = handle_workflow_dashboard_input(
            &root,
            &mut state,
            "update-step --clear-name --prompt ship release --foreground",
        )
        .expect("update dashboard step");
        match updated {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Updated workflow step 2"));
                assert!(text.contains("Workflow authoring panel: release-review"));
            }
            other => panic!("expected dashboard update-step output, got {other:?}"),
        }

        let raw = fs::read_to_string(
            root.join(".hellox")
                .join("workflows")
                .join("release-review.json"),
        )
        .expect("read workflow json");
        let value = serde_json::from_str::<serde_json::Value>(&raw).expect("parse workflow json");
        let steps = value
            .get("steps")
            .and_then(serde_json::Value::as_array)
            .expect("workflow steps");
        assert_eq!(steps.len(), 2);
        assert!(steps[1].get("name").is_none());
        assert_eq!(
            steps[1].get("prompt").and_then(serde_json::Value::as_str),
            Some("ship release")
        );
        assert_eq!(
            steps[1]
                .get("run_in_background")
                .and_then(serde_json::Value::as_bool),
            None
        );
    }

    #[test]
    fn dashboard_panel_shortcuts_support_field_edits() {
        let root = temp_dir();
        write_workflow(
            &root,
            "release-review.json",
            r#"{
  "steps": [
    { "name": "review", "prompt": "review release" }
  ]
}"#,
        );

        let mut state =
            initial_workflow_dashboard_state(Some(String::from("release-review")), None);
        let _ = render_workflow_dashboard_state(&root, &mut state).expect("render dashboard");
        let _ = handle_workflow_dashboard_input(&root, &mut state, "panel 1")
            .expect("focus panel step");

        let renamed = handle_workflow_dashboard_input(&root, &mut state, "name ship review")
            .expect("rename step");
        match renamed {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Updated workflow step 1 name."));
                assert!(text.contains("Workflow authoring panel: release-review"));
            }
            other => panic!("expected dashboard rename output, got {other:?}"),
        }

        let mode = handle_workflow_dashboard_input(&root, &mut state, "background")
            .expect("set background mode");
        match mode {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Set workflow step 1 to background mode."));
            }
            other => panic!("expected dashboard background output, got {other:?}"),
        }

        let raw = fs::read_to_string(
            root.join(".hellox")
                .join("workflows")
                .join("release-review.json"),
        )
        .expect("read workflow json");
        let value = serde_json::from_str::<serde_json::Value>(&raw).expect("parse workflow json");
        let step = value
            .get("steps")
            .and_then(serde_json::Value::as_array)
            .and_then(|steps| steps.first())
            .expect("workflow step");
        assert_eq!(
            step.get("name").and_then(serde_json::Value::as_str),
            Some("ship review")
        );
        assert_eq!(
            step.get("run_in_background")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn dashboard_navigation_shortcuts_switch_focused_panel_steps() {
        let root = temp_dir();
        write_workflow(
            &root,
            "release-review.json",
            r#"{
  "steps": [
    { "name": "review", "prompt": "review release" },
    { "name": "ship", "prompt": "ship release" }
  ]
}"#,
        );

        let mut state =
            initial_workflow_dashboard_state(Some(String::from("release-review")), None);
        let _ = render_workflow_dashboard_state(&root, &mut state).expect("render dashboard");
        let _ = handle_workflow_dashboard_input(&root, &mut state, "panel 1")
            .expect("focus panel step");

        let moved =
            handle_workflow_dashboard_input(&root, &mut state, "next").expect("focus next step");
        match moved {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Focused workflow step 2 of 2."));
                assert!(text.contains("> | 2 | ship"));
            }
            other => panic!("expected dashboard navigation output, got {other:?}"),
        }
        assert_eq!(
            state.current(),
            &hellox_tui::WorkflowDashboardView::PanelFocus {
                workflow_name: String::from("release-review"),
                step_number: Some(2),
            }
        );

        let bounded = handle_workflow_dashboard_input(&root, &mut state, "next")
            .expect("keep last focused step");
        match bounded {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Already on the last workflow step (2 of 2)."));
            }
            other => panic!("expected dashboard bounded navigation output, got {other:?}"),
        }
    }

    #[test]
    fn dashboard_navigation_shortcuts_switch_run_inspect_steps() {
        let root = temp_dir();
        write_workflow(
            &root,
            "release-review.json",
            r#"{ "steps": [{ "name": "review", "prompt": "review release" }] }"#,
        );
        write_run(&root, "run-123", "release-review");

        let mut state =
            initial_workflow_dashboard_state(Some(String::from("release-review")), None);
        let _ = render_workflow_dashboard_state(&root, &mut state).expect("render dashboard");
        let _ = handle_workflow_dashboard_input(&root, &mut state, "show-run run-123 1")
            .expect("open run inspect");

        let moved =
            handle_workflow_dashboard_input(&root, &mut state, "last").expect("focus last step");
        match moved {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Focused recorded step 2 of 2."));
                assert!(text.contains("> [1] ship"));
            }
            other => panic!("expected dashboard run navigation output, got {other:?}"),
        }
        assert_eq!(
            state.current(),
            &hellox_tui::WorkflowDashboardView::RunInspect {
                run_id: String::from("run-123"),
                step_number: Some(2),
            }
        );
    }

    #[test]
    fn dashboard_run_command_returns_active_workflow_request() {
        let root = temp_dir();
        write_workflow(
            &root,
            "release-review.json",
            r#"{ "steps": [{ "name": "review", "prompt": "review release" }] }"#,
        );

        let mut state =
            initial_workflow_dashboard_state(Some(String::from("release-review")), None);
        let _ = render_workflow_dashboard_state(&root, &mut state).expect("render dashboard");

        let outcome = handle_workflow_dashboard_input(&root, &mut state, "run ship carefully")
            .expect("request workflow run");
        match outcome {
            WorkflowDashboardHandleOutcome::RunActiveWorkflow {
                target,
                target_label,
                shared_context,
            } => {
                assert_eq!(
                    target,
                    crate::workflows::WorkflowRunTarget::Named(String::from("release-review"))
                );
                assert_eq!(target_label, "release-review");
                assert_eq!(shared_context, Some(String::from("ship carefully")));
            }
            other => panic!("expected run request outcome, got {other:?}"),
        }
    }

    #[test]
    fn dashboard_run_completion_opens_recorded_run() {
        let root = temp_dir();
        write_workflow(
            &root,
            "release-review.json",
            r#"{ "steps": [{ "name": "review", "prompt": "review release" }] }"#,
        );
        write_run(&root, "run-123", "release-review");

        let mut state =
            initial_workflow_dashboard_state(Some(String::from("release-review")), None);
        let _ = render_workflow_dashboard_state(&root, &mut state).expect("render dashboard");

        let text = complete_workflow_dashboard_run(
            &root,
            &mut state,
            &crate::workflows::WorkflowRunTarget::Named(String::from("release-review")),
            "release-review",
            r#"{ "run_id": "run-123", "status": "completed" }"#,
        )
        .expect("complete dashboard run");

        assert!(text.contains("Executed workflow `release-review`"));
        assert!(text.contains("Workflow run inspect panel: run-123"));
        assert_eq!(
            state.current(),
            &hellox_tui::WorkflowDashboardView::RunInspect {
                run_id: String::from("run-123"),
                step_number: None,
            }
        );
    }

    #[test]
    fn dashboard_last_run_uses_current_workflow_context() {
        let root = temp_dir();
        write_workflow(
            &root,
            "release-review.json",
            r#"{ "steps": [{ "name": "review", "prompt": "review release" }] }"#,
        );
        write_run(&root, "run-123", "release-review");

        let mut state =
            initial_workflow_dashboard_state(Some(String::from("release-review")), None);
        let _ = render_workflow_dashboard_state(&root, &mut state).expect("render dashboard");

        let output =
            handle_workflow_dashboard_input(&root, &mut state, "last-run").expect("open last run");
        match output {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Workflow run inspect panel: run-123"));
            }
            other => panic!("expected last-run dashboard output, got {other:?}"),
        }
    }

    #[test]
    fn dashboard_last_run_supports_explicit_step_focus() {
        let root = temp_dir();
        write_workflow(
            &root,
            "release-review.json",
            r#"{ "steps": [{ "name": "review", "prompt": "review release" }] }"#,
        );
        write_run(&root, "run-123", "release-review");

        let mut state =
            initial_workflow_dashboard_state(Some(String::from("release-review")), None);
        let _ = render_workflow_dashboard_state(&root, &mut state).expect("render dashboard");

        let output = handle_workflow_dashboard_input(&root, &mut state, "last-run 2")
            .expect("open last run with step");
        match output {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Workflow run inspect panel: run-123"));
                assert!(text.contains("> [1] ship"));
            }
            other => panic!("expected focused last-run dashboard output, got {other:?}"),
        }
        assert_eq!(
            state.current(),
            &hellox_tui::WorkflowDashboardView::RunInspect {
                run_id: String::from("run-123"),
                step_number: Some(2),
            }
        );
    }

    #[test]
    fn dashboard_overview_focus_numeric_recent_run_opens_recorded_run() {
        let root = temp_dir();
        write_workflow(
            &root,
            "release-review.json",
            r#"{ "steps": [{ "name": "review", "prompt": "review release" }] }"#,
        );
        write_run(&root, "run-123", "release-review");

        let mut state =
            initial_workflow_dashboard_state(Some(String::from("release-review")), None);
        let text = render_workflow_dashboard_state(&root, &mut state).expect("render dashboard");
        assert!(text.contains("== Recent runs =="));
        assert!(text.contains("[2] run-123"));

        let output =
            handle_workflow_dashboard_input(&root, &mut state, "2").expect("open recent run");
        match output {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Workflow run inspect panel: run-123"));
                assert!(text.contains("== Primary step lens =="));
            }
            other => panic!("expected recent-run dashboard output, got {other:?}"),
        }
        assert_eq!(
            state.current(),
            &hellox_tui::WorkflowDashboardView::RunInspect {
                run_id: String::from("run-123"),
                step_number: None,
            }
        );
    }

    #[test]
    fn dashboard_panel_focus_open_recent_run_uses_same_selector_order() {
        let root = temp_dir();
        write_workflow(
            &root,
            "release-review.json",
            r#"{ "steps": [{ "name": "review", "prompt": "review release" }] }"#,
        );
        write_run(&root, "run-123", "release-review");

        let mut state =
            initial_workflow_dashboard_state(Some(String::from("release-review")), None);
        let _ = render_workflow_dashboard_state(&root, &mut state).expect("render dashboard");
        let _ = handle_workflow_dashboard_input(&root, &mut state, "panel 1")
            .expect("focus panel step");

        let output =
            handle_workflow_dashboard_input(&root, &mut state, "open 2").expect("open run");
        match output {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Workflow run inspect panel: run-123"));
                assert!(text.contains("== Primary step lens =="));
            }
            other => panic!("expected panel recent-run dashboard output, got {other:?}"),
        }
        assert_eq!(
            state.current(),
            &hellox_tui::WorkflowDashboardView::RunInspect {
                run_id: String::from("run-123"),
                step_number: None,
            }
        );
    }

    #[test]
    fn dashboard_supports_show_validate_and_init_commands() {
        let root = temp_dir();
        write_workflow(
            &root,
            "release-review.json",
            r#"{ "steps": [{ "name": "review", "prompt": "review release" }] }"#,
        );

        let mut state =
            initial_workflow_dashboard_state(Some(String::from("release-review")), None);
        let _ = render_workflow_dashboard_state(&root, &mut state).expect("render dashboard");

        let shown =
            handle_workflow_dashboard_input(&root, &mut state, "show").expect("show workflow");
        match shown {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("workflow: release-review"));
            }
            other => panic!("expected workflow show output, got {other:?}"),
        }

        let validated = handle_workflow_dashboard_input(&root, &mut state, "validate")
            .expect("validate workflow");
        match validated {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("release-review"));
                assert!(text.contains("valid"));
            }
            other => panic!("expected workflow validate output, got {other:?}"),
        }

        let initialized =
            handle_workflow_dashboard_input(&root, &mut state, "init ship").expect("init workflow");
        match initialized {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Initialized workflow `ship`"));
                assert!(text.contains("Workflow overview: ship"));
            }
            other => panic!("expected workflow init output, got {other:?}"),
        }
    }

    #[test]
    fn dashboard_supports_shared_context_and_continue_on_error_updates() {
        let root = temp_dir();
        write_workflow(
            &root,
            "release-review.json",
            r#"{ "steps": [{ "name": "review", "prompt": "review release" }] }"#,
        );

        let mut state =
            initial_workflow_dashboard_state(Some(String::from("release-review")), None);
        let _ = render_workflow_dashboard_state(&root, &mut state).expect("render dashboard");

        let updated =
            handle_workflow_dashboard_input(&root, &mut state, "set-shared-context ship carefully")
                .expect("set shared_context");
        match updated {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Updated shared_context."));
                assert!(text.contains("shared_context"));
                assert!(text.contains("ship carefully"));
            }
            other => panic!("expected workflow shared_context output, got {other:?}"),
        }

        let enabled =
            handle_workflow_dashboard_input(&root, &mut state, "enable-continue-on-error")
                .expect("enable continue_on_error");
        match enabled {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Enabled continue_on_error."));
                assert!(text.contains("continue_on_error"));
                assert!(text.contains("true"));
            }
            other => panic!("expected workflow continue_on_error output, got {other:?}"),
        }

        let cleared = handle_workflow_dashboard_input(&root, &mut state, "clear-shared-context")
            .expect("clear shared_context");
        match cleared {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Cleared shared_context."));
                assert!(text.contains("shared_context"));
                assert!(text.contains("(none)"));
            }
            other => panic!("expected workflow clear shared_context output, got {other:?}"),
        }

        let disabled =
            handle_workflow_dashboard_input(&root, &mut state, "disable-continue-on-error")
                .expect("disable continue_on_error");
        match disabled {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Disabled continue_on_error."));
                assert!(text.contains("continue_on_error"));
                assert!(text.contains("false"));
            }
            other => panic!("expected workflow disable continue_on_error output, got {other:?}"),
        }
    }

    #[test]
    fn dashboard_explicit_script_path_supports_core_commands() {
        let root = temp_dir();
        write_explicit_workflow(
            &root,
            "scripts/custom-release.json",
            r#"{
  "steps": [
    { "name": "review", "prompt": "review release" },
    { "name": "ship", "prompt": "ship release" }
  ]
}"#,
        );
        write_run_for_script_path(&root, "run-123", "scripts/custom-release.json");

        let mut state = initial_workflow_dashboard_state(
            None,
            Some(String::from("scripts/custom-release.json")),
        );
        let _ = render_workflow_dashboard_state(&root, &mut state).expect("render dashboard");

        let shown =
            handle_workflow_dashboard_input(&root, &mut state, "show").expect("show workflow");
        match shown {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("workflow: scripts/custom-release"));
            }
            other => panic!("expected explicit workflow show output, got {other:?}"),
        }

        let validated = handle_workflow_dashboard_input(&root, &mut state, "validate")
            .expect("validate explicit workflow");
        match validated {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("scripts/custom-release"));
                assert!(text.contains("valid"));
            }
            other => panic!("expected explicit workflow validate output, got {other:?}"),
        }

        let runs =
            handle_workflow_dashboard_input(&root, &mut state, "runs").expect("list workflow runs");
        match runs {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("run-123"));
                assert!(text.contains("--script-path"));
            }
            other => panic!("expected explicit workflow runs output, got {other:?}"),
        }
        match state.current() {
            hellox_tui::WorkflowDashboardView::RunsPath { script_path } => {
                assert!(script_path.ends_with("scripts/custom-release.json"));
            }
            other => panic!("expected explicit workflow runs view, got {other:?}"),
        }

        let latest = handle_workflow_dashboard_input(&root, &mut state, "last-run 2")
            .expect("open latest explicit workflow run");
        match latest {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Workflow run inspect panel: run-123"));
                assert!(text.contains("> [1] ship"));
            }
            other => panic!("expected explicit workflow last-run output, got {other:?}"),
        }

        let focused_panel =
            handle_workflow_dashboard_input(&root, &mut state, "panel 2").expect("focus panel");
        match focused_panel {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Workflow authoring panel: scripts/custom-release"));
            }
            other => panic!("expected explicit workflow panel output, got {other:?}"),
        }

        let renamed = handle_workflow_dashboard_input(&root, &mut state, "name ship release")
            .expect("rename explicit workflow step");
        match renamed {
            WorkflowDashboardHandleOutcome::Print(text) => {
                assert!(text.contains("Updated workflow step 2 name."));
            }
            other => panic!("expected explicit workflow rename output, got {other:?}"),
        }

        let raw = fs::read_to_string(root.join("scripts").join("custom-release.json"))
            .expect("read script");
        let value = serde_json::from_str::<serde_json::Value>(&raw).expect("parse workflow json");
        let steps = value
            .get("steps")
            .and_then(serde_json::Value::as_array)
            .expect("workflow steps");
        assert_eq!(
            steps[1].get("name").and_then(serde_json::Value::as_str),
            Some("ship release")
        );

        let outcome = handle_workflow_dashboard_input(&root, &mut state, "run ship carefully")
            .expect("request explicit workflow run");
        match outcome {
            WorkflowDashboardHandleOutcome::RunActiveWorkflow {
                target,
                target_label,
                shared_context,
            } => {
                assert_eq!(
                    target,
                    crate::workflows::WorkflowRunTarget::Path(
                        root.join("scripts").join("custom-release.json")
                    )
                );
                assert!(target_label.ends_with("scripts/custom-release.json"));
                assert_eq!(shared_context, Some(String::from("ship carefully")));
            }
            other => panic!("expected explicit workflow run request, got {other:?}"),
        }
    }
}
