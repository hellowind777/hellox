use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use hellox_agent::AgentSession;
use serde::Deserialize;
use serde_json::{json, Map, Value};

const WORKFLOW_DIRECTORY: &str = ".hellox/workflows";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkflowScriptSummary {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    pub(crate) step_count: usize,
    pub(crate) continue_on_error: bool,
    pub(crate) shared_context: Option<String>,
    pub(crate) dynamic_command: bool,
    pub(crate) validation_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkflowStepSummary {
    pub(crate) name: Option<String>,
    pub(crate) prompt_chars: usize,
    pub(crate) when: bool,
    pub(crate) model: Option<String>,
    pub(crate) backend: Option<String>,
    pub(crate) cwd: Option<String>,
    pub(crate) run_in_background: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkflowScriptDetail {
    pub(crate) summary: WorkflowScriptSummary,
    pub(crate) steps: Vec<WorkflowStepSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkflowValidationSummary {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    pub(crate) valid: bool,
    pub(crate) dynamic_command: bool,
    pub(crate) step_count: Option<usize>,
    pub(crate) messages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkflowRunTarget {
    Named(String),
    Path(PathBuf),
}

#[derive(Debug, Clone, Default, Deserialize)]
struct WorkflowScriptFile {
    #[serde(default)]
    steps: Vec<WorkflowStepFile>,
    #[serde(default)]
    continue_on_error: Option<bool>,
    #[serde(default)]
    shared_context: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct WorkflowStepFile {
    #[serde(default)]
    name: Option<String>,
    prompt: String,
    #[serde(default)]
    when: Option<Value>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    backend: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    run_in_background: Option<bool>,
}

pub(crate) fn list_workflows(root: &Path) -> Result<Vec<WorkflowScriptSummary>> {
    let workflow_root = workflow_root(root);
    if !workflow_root.exists() {
        return Ok(Vec::new());
    }

    let mut workflows = Vec::new();
    for path in discover_workflow_paths(root)? {
        workflows.push(summarize_workflow_path(root, &path));
    }
    workflows.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(workflows)
}

pub(crate) fn list_invocable_workflows(root: &Path) -> Result<Vec<WorkflowScriptSummary>> {
    Ok(list_workflows(root)?
        .into_iter()
        .filter(|workflow| workflow.validation_error.is_none() && workflow.dynamic_command)
        .collect())
}

pub(crate) fn resolve_named_workflow(root: &Path, name: &str) -> Result<Option<String>> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    Ok(list_workflows(root)?
        .into_iter()
        .filter(|workflow| workflow.validation_error.is_none())
        .find(|workflow| workflow.name.eq_ignore_ascii_case(trimmed))
        .map(|workflow| workflow.name))
}

pub(crate) fn load_named_workflow_detail(root: &Path, name: &str) -> Result<WorkflowScriptDetail> {
    let workflow_name = resolve_named_workflow(root, name)?.ok_or_else(|| {
        anyhow!(
            "workflow `{name}` was not found under `{}`",
            path_text(&workflow_root(root))
        )
    })?;
    let path = named_workflow_path(root, &workflow_name)?;
    load_workflow_detail_from_path(root, &path, Some(workflow_name))
}

pub(crate) fn load_workflow_detail_from_path(
    root: &Path,
    path: &Path,
    workflow_name: Option<String>,
) -> Result<WorkflowScriptDetail> {
    let file = load_workflow_script_file(path)?;
    let name = workflow_name.unwrap_or_else(|| workflow_name_from_path(root, path));
    Ok(detail_from_file(name, path.to_path_buf(), file))
}

pub(crate) fn validate_workflows(root: &Path) -> Result<Vec<WorkflowValidationSummary>> {
    let workflow_root = workflow_root(root);
    if !workflow_root.exists() {
        return Ok(Vec::new());
    }

    let mut results = discover_workflow_paths(root)?
        .into_iter()
        .map(|path| validate_workflow_path(root, &path))
        .collect::<Vec<_>>();
    results.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(results)
}

pub(crate) fn validate_named_workflow(
    root: &Path,
    workflow_name: &str,
) -> Result<WorkflowValidationSummary> {
    let path = named_workflow_path(root, workflow_name)?;
    if !path.exists() {
        return Err(anyhow!(
            "workflow `{workflow_name}` was not found under `{}`",
            path_text(&workflow_root(root))
        ));
    }
    Ok(validate_workflow_path(root, &path))
}

pub(crate) fn validate_explicit_workflow_path(
    root: &Path,
    path: &Path,
) -> Result<WorkflowValidationSummary> {
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    if !resolved.exists() {
        return Err(anyhow!(
            "workflow script does not exist: {}",
            path_text(&resolved)
        ));
    }
    Ok(validate_workflow_path(root, &resolved))
}

pub(crate) fn render_workflow_list(root: &Path, workflows: &[WorkflowScriptSummary]) -> String {
    if workflows.is_empty() {
        return format!(
            "No workflow scripts found under `{}`.",
            path_text(&workflow_root(root))
        );
    }

    let mut lines = vec![format!(
        "Workflow scripts under `{}`:",
        path_text(&workflow_root(root))
    )];
    for workflow in workflows {
        let dynamic_command = if workflow.dynamic_command {
            "yes"
        } else {
            "no"
        };
        match &workflow.validation_error {
            Some(error) => lines.push(format!(
                "- {} — invalid, dynamic_command: {}, error: {}, path: {}",
                workflow.name,
                dynamic_command,
                error,
                path_text(&workflow.path)
            )),
            None => {
                let shared_context = workflow.shared_context.as_deref().unwrap_or("(none)");
                lines.push(format!(
                    "- {} — valid, {} step(s), dynamic_command: {}, continue_on_error: {}, shared_context: {}, path: {}",
                    workflow.name,
                    workflow.step_count,
                    dynamic_command,
                    workflow.continue_on_error,
                    shared_context,
                    path_text(&workflow.path)
                ));
            }
        }
    }
    lines.join("\n")
}

pub(crate) fn render_workflow_detail(detail: &WorkflowScriptDetail) -> String {
    let mut lines = vec![
        format!("workflow: {}", detail.summary.name),
        format!("path: {}", path_text(&detail.summary.path)),
        format!("steps: {}", detail.summary.step_count),
        format!("continue_on_error: {}", detail.summary.continue_on_error),
        format!(
            "dynamic_command: {}",
            if detail.summary.dynamic_command {
                "yes"
            } else {
                "no"
            }
        ),
        format!(
            "shared_context: {}",
            detail.summary.shared_context.as_deref().unwrap_or("(none)")
        ),
    ];

    if detail.steps.is_empty() {
        lines.push("step_details: (none)".to_string());
        return lines.join("\n");
    }

    lines.push("step_details:".to_string());
    for (index, step) in detail.steps.iter().enumerate() {
        let name = step.name.as_deref().unwrap_or("(unnamed)");
        let mut attributes = vec![format!("prompt_chars={}", step.prompt_chars)];
        if step.when {
            attributes.push("when=true".to_string());
        }
        if let Some(model) = &step.model {
            attributes.push(format!("model={model}"));
        }
        if let Some(backend) = &step.backend {
            attributes.push(format!("backend={backend}"));
        }
        if let Some(cwd) = &step.cwd {
            attributes.push(format!("cwd={cwd}"));
        }
        if step.run_in_background {
            attributes.push("background=true".to_string());
        }
        lines.push(format!(
            "  {}. {} [{}]",
            index + 1,
            name,
            attributes.join(", ")
        ));
    }

    lines.join("\n")
}

pub(crate) fn render_workflow_validation(
    results: &[WorkflowValidationSummary],
    root: &Path,
) -> String {
    if results.is_empty() {
        return format!(
            "No workflow scripts found under `{}`.",
            path_text(&workflow_root(root))
        );
    }

    let mut lines = vec![format!(
        "Workflow validation under `{}`:",
        path_text(&workflow_root(root))
    )];
    for result in results {
        let status = if result.valid { "valid" } else { "invalid" };
        let dynamic_command = if result.dynamic_command { "yes" } else { "no" };
        let step_count = result
            .step_count
            .map(|count| count.to_string())
            .unwrap_or_else(|| "?".to_string());
        lines.push(format!(
            "- {} — {}, {} step(s), dynamic_command: {}, path: {}",
            result.name,
            status,
            step_count,
            dynamic_command,
            path_text(&result.path)
        ));
        for message in &result.messages {
            lines.push(format!("  - {message}"));
        }
    }
    lines.join("\n")
}

pub(crate) fn initialize_workflow(
    root: &Path,
    workflow_name: &str,
    shared_context: Option<String>,
    continue_on_error: bool,
    force: bool,
) -> Result<PathBuf> {
    let path = named_workflow_path(root, workflow_name)?;
    if path.exists() && !force {
        return Err(anyhow!(
            "workflow script already exists: {} (use `--force` to overwrite)",
            path_text(&path)
        ));
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("failed to create workflow directory {}", path_text(parent))
        })?;
    }

    let mut document = Map::new();
    if let Some(shared_context) = normalize_optional_text(shared_context) {
        document.insert("shared_context".to_string(), Value::String(shared_context));
    }
    if continue_on_error {
        document.insert("continue_on_error".to_string(), Value::Bool(true));
    }
    document.insert(
        "steps".to_string(),
        json!([
            {
                "name": "review",
                "prompt": "Review the current workspace state and summarize the most important findings."
            },
            {
                "name": "act",
                "prompt": "Continue from {{workflow.previous_result}} and complete the next concrete task."
            }
        ]),
    );

    let raw = serde_json::to_string_pretty(&Value::Object(document))
        .context("failed to serialize workflow template")?;
    fs::write(&path, format!("{raw}\n"))
        .with_context(|| format!("failed to write workflow script {}", path_text(&path)))?;
    Ok(path)
}

pub(crate) async fn execute_workflow(
    session: &AgentSession,
    target: WorkflowRunTarget,
    shared_context: Option<String>,
    continue_on_error: Option<bool>,
) -> Result<String> {
    let mut input = Map::new();
    match target {
        WorkflowRunTarget::Named(name) => {
            input.insert("script".to_string(), Value::String(name));
        }
        WorkflowRunTarget::Path(path) => {
            input.insert("script_path".to_string(), Value::String(path_text(&path)));
        }
    }
    if let Some(shared_context) = normalize_optional_text(shared_context) {
        input.insert("shared_context".to_string(), Value::String(shared_context));
    }
    if let Some(continue_on_error) = continue_on_error {
        input.insert(
            "continue_on_error".to_string(),
            Value::Bool(continue_on_error),
        );
    }
    session
        .run_local_tool("workflow", Value::Object(input))
        .await
}

fn discover_workflow_paths(root: &Path) -> Result<Vec<PathBuf>> {
    let workflow_root = workflow_root(root);
    if !workflow_root.exists() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    collect_workflow_paths(&workflow_root, &mut paths)?;
    paths.sort_by(|left, right| path_text(left).cmp(&path_text(right)));
    Ok(paths)
}

fn collect_workflow_paths(directory: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(directory)
        .with_context(|| format!("failed to read workflow directory {}", path_text(directory)))?
    {
        let entry = entry.with_context(|| {
            format!(
                "failed to inspect workflow entry under {}",
                path_text(directory)
            )
        })?;
        let path = entry.path();
        if entry
            .file_type()
            .with_context(|| format!("failed to inspect workflow entry {}", path_text(&path)))?
            .is_dir()
        {
            collect_workflow_paths(&path, paths)?;
            continue;
        }

        if path.extension().and_then(|value| value.to_str()) == Some("json") {
            paths.push(path);
        }
    }

    Ok(())
}

fn summarize_workflow_path(root: &Path, path: &Path) -> WorkflowScriptSummary {
    match load_workflow_detail_from_path(root, path, None) {
        Ok(detail) => detail.summary,
        Err(error) => {
            let name = workflow_name_from_path(root, path);
            WorkflowScriptSummary {
                name: name.clone(),
                path: path.to_path_buf(),
                step_count: 0,
                continue_on_error: false,
                shared_context: None,
                dynamic_command: is_invocable_workflow_name(&name),
                validation_error: Some(error.to_string()),
            }
        }
    }
}

fn validate_workflow_path(root: &Path, path: &Path) -> WorkflowValidationSummary {
    let name = workflow_name_from_path(root, path);
    let dynamic_command = is_invocable_workflow_name(&name);

    match load_workflow_script_file(path) {
        Ok(file) => validation_from_file(name, path.to_path_buf(), file, dynamic_command),
        Err(error) => WorkflowValidationSummary {
            name,
            path: path.to_path_buf(),
            valid: false,
            dynamic_command,
            step_count: None,
            messages: vec![error.to_string()],
        },
    }
}

fn validation_from_file(
    name: String,
    path: PathBuf,
    file: WorkflowScriptFile,
    dynamic_command: bool,
) -> WorkflowValidationSummary {
    let mut valid = true;
    let mut messages = Vec::new();

    if file.steps.is_empty() {
        valid = false;
        messages.push("workflow must define at least one step".to_string());
    }

    let mut counts = BTreeMap::new();
    let mut unnamed_steps = 0usize;
    for step in &file.steps {
        match normalize_optional_text(step.name.clone()) {
            Some(name) => {
                *counts.entry(name).or_insert(0usize) += 1;
            }
            None => unnamed_steps += 1,
        }
    }

    let duplicates = counts
        .into_iter()
        .filter_map(|(name, count)| (count > 1).then_some(name))
        .collect::<BTreeSet<_>>();
    if !duplicates.is_empty() {
        messages.push(format!(
            "duplicate step names: {}",
            duplicates.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    if unnamed_steps > 0 {
        messages.push(format!(
            "unnamed steps: {unnamed_steps} (allowed, but they cannot be referenced via `steps.<name>` placeholders)"
        ));
    }
    if !dynamic_command {
        messages.push(
            "dynamic `/name` invocation is unavailable because the workflow name is nested or contains whitespace"
                .to_string(),
        );
    }

    WorkflowValidationSummary {
        name,
        path,
        valid,
        dynamic_command,
        step_count: Some(file.steps.len()),
        messages,
    }
}

fn load_workflow_script_file(path: &Path) -> Result<WorkflowScriptFile> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read workflow script {}", path_text(path)))?;
    serde_json::from_str::<WorkflowScriptFile>(&raw)
        .with_context(|| format!("failed to parse workflow script {}", path_text(path)))
}

fn detail_from_file(name: String, path: PathBuf, file: WorkflowScriptFile) -> WorkflowScriptDetail {
    let summary = WorkflowScriptSummary {
        dynamic_command: is_invocable_workflow_name(&name),
        name,
        path,
        step_count: file.steps.len(),
        continue_on_error: file.continue_on_error.unwrap_or(false),
        shared_context: normalize_optional_text(file.shared_context),
        validation_error: None,
    };
    let steps = file
        .steps
        .into_iter()
        .map(|step| WorkflowStepSummary {
            name: normalize_optional_text(step.name),
            prompt_chars: step.prompt.trim().chars().count(),
            when: step.when.is_some(),
            model: normalize_optional_text(step.model),
            backend: normalize_optional_text(step.backend),
            cwd: normalize_optional_text(step.cwd),
            run_in_background: step.run_in_background.unwrap_or(false),
        })
        .collect();
    WorkflowScriptDetail { summary, steps }
}

fn named_workflow_path(root: &Path, name: &str) -> Result<PathBuf> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("workflow name cannot be empty"));
    }

    let relative = PathBuf::from(trimmed);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(anyhow!(
            "workflow name must stay within `{WORKFLOW_DIRECTORY}`"
        ));
    }

    let mut path = workflow_root(root).join(relative);
    if path.extension().is_none() {
        path.set_extension("json");
    }
    Ok(path)
}

fn workflow_name_from_path(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(workflow_root(root)).unwrap_or(path);
    let without_extension = relative.with_extension("");
    path_text(&without_extension)
}

fn workflow_root(root: &Path) -> PathBuf {
    root.join(".hellox").join("workflows")
}

fn is_invocable_workflow_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && !name.chars().any(char::is_whitespace)
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn path_text(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}
