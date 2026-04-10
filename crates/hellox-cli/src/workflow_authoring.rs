use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde_json::{Map, Value};

use crate::workflows::{
    load_workflow_detail_from_path, resolve_named_workflow, WorkflowScriptDetail,
};

const WORKFLOW_DIRECTORY: &str = ".hellox/workflows";

#[path = "workflow_authoring_step_layout.rs"]
mod step_layout;

pub(crate) use step_layout::{duplicate_workflow_step, move_workflow_step};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkflowStepDraft {
    pub(crate) name: Option<String>,
    pub(crate) prompt: String,
    pub(crate) when: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) backend: Option<String>,
    pub(crate) step_cwd: Option<String>,
    pub(crate) run_in_background: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct WorkflowStepPatch {
    pub(crate) name: Option<Option<String>>,
    pub(crate) prompt: Option<String>,
    pub(crate) when: Option<Option<String>>,
    pub(crate) model: Option<Option<String>>,
    pub(crate) backend: Option<Option<String>>,
    pub(crate) step_cwd: Option<Option<String>>,
    pub(crate) run_in_background: Option<bool>,
}

impl WorkflowStepPatch {
    pub(crate) fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.prompt.is_none()
            && self.when.is_none()
            && self.model.is_none()
            && self.backend.is_none()
            && self.step_cwd.is_none()
            && self.run_in_background.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkflowAddStepResult {
    pub(crate) detail: WorkflowScriptDetail,
    pub(crate) step_number: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkflowRemoveStepResult {
    pub(crate) detail: WorkflowScriptDetail,
    pub(crate) removed_step_name: Option<String>,
}

pub(crate) fn resolve_existing_workflow_path(root: &Path, name: &str) -> Result<PathBuf> {
    let workflow_name = resolve_named_workflow(root, name)?.ok_or_else(|| {
        anyhow!(
            "workflow `{name}` was not found under `{}`",
            path_text(&workflow_root(root))
        )
    })?;
    workflow_path_from_name(root, &workflow_name)
}

pub(crate) fn add_workflow_step(
    root: &Path,
    path: &Path,
    draft: WorkflowStepDraft,
    index: Option<usize>,
) -> Result<WorkflowAddStepResult> {
    let mut document = load_workflow_document(path)?;
    let steps = steps_mut(&mut document)?;
    let insert_at = normalize_insert_index(index, steps.len())?;
    steps.insert(insert_at, draft_to_value(draft)?);
    save_workflow_document(path, &document)?;

    Ok(WorkflowAddStepResult {
        detail: load_workflow_detail_from_path(root, path, None)?,
        step_number: insert_at + 1,
    })
}

pub(crate) fn update_workflow_step(
    root: &Path,
    path: &Path,
    step_number: usize,
    patch: WorkflowStepPatch,
) -> Result<WorkflowScriptDetail> {
    if patch.is_empty() {
        return Err(anyhow!(
            "workflow update-step requires at least one field change"
        ));
    }

    let mut document = load_workflow_document(path)?;
    let steps = steps_mut(&mut document)?;
    let index = normalize_existing_index(step_number, steps.len())?;
    patch_step_value(&mut steps[index], patch)?;
    save_workflow_document(path, &document)?;
    load_workflow_detail_from_path(root, path, None)
}

pub(crate) fn remove_workflow_step(
    root: &Path,
    path: &Path,
    step_number: usize,
) -> Result<WorkflowRemoveStepResult> {
    let mut document = load_workflow_document(path)?;
    let steps = steps_mut(&mut document)?;
    let index = normalize_existing_index(step_number, steps.len())?;
    let removed = steps.remove(index);
    save_workflow_document(path, &document)?;

    Ok(WorkflowRemoveStepResult {
        detail: load_workflow_detail_from_path(root, path, None)?,
        removed_step_name: removed
            .as_object()
            .and_then(|step| step.get("name"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
    })
}

pub(crate) fn set_workflow_shared_context(
    root: &Path,
    path: &Path,
    value: Option<String>,
) -> Result<WorkflowScriptDetail> {
    let mut document = load_workflow_document(path)?;
    match normalize_optional_text(value) {
        Some(shared_context) => {
            document.insert("shared_context".to_string(), Value::String(shared_context));
        }
        None => {
            document.remove("shared_context");
        }
    }
    save_workflow_document(path, &document)?;
    load_workflow_detail_from_path(root, path, None)
}

pub(crate) fn set_workflow_continue_on_error(
    root: &Path,
    path: &Path,
    enabled: bool,
) -> Result<WorkflowScriptDetail> {
    let mut document = load_workflow_document(path)?;
    if enabled {
        document.insert("continue_on_error".to_string(), Value::Bool(true));
    } else {
        document.remove("continue_on_error");
    }
    save_workflow_document(path, &document)?;
    load_workflow_detail_from_path(root, path, None)
}

fn workflow_root(root: &Path) -> PathBuf {
    root.join(".hellox").join("workflows")
}

fn workflow_path_from_name(root: &Path, name: &str) -> Result<PathBuf> {
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

fn load_workflow_document(path: &Path) -> Result<Map<String, Value>> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read workflow script {}", path_text(path)))?;
    let value = serde_json::from_str::<Value>(&raw)
        .with_context(|| format!("failed to parse workflow script {}", path_text(path)))?;
    match value {
        Value::Object(object) => Ok(object),
        _ => Err(anyhow!(
            "workflow script must be a JSON object: {}",
            path_text(path)
        )),
    }
}

fn save_workflow_document(path: &Path, document: &Map<String, Value>) -> Result<()> {
    let raw = serde_json::to_string_pretty(&Value::Object(document.clone()))
        .context("failed to serialize workflow document")?;
    fs::write(path, format!("{raw}\n"))
        .with_context(|| format!("failed to write workflow script {}", path_text(path)))
}

fn steps_mut(document: &mut Map<String, Value>) -> Result<&mut Vec<Value>> {
    if !document.contains_key("steps") {
        document.insert("steps".to_string(), Value::Array(Vec::new()));
    }

    match document.get_mut("steps") {
        Some(Value::Array(steps)) => Ok(steps),
        Some(_) => Err(anyhow!("workflow `steps` must be a JSON array")),
        None => Err(anyhow!("workflow `steps` field is missing")),
    }
}

fn normalize_insert_index(index: Option<usize>, len: usize) -> Result<usize> {
    match index {
        None => Ok(len),
        Some(0) => Err(anyhow!(
            "workflow step index `0` is out of range; expected 1..={}",
            len + 1
        )),
        Some(index) if index > len + 1 => Err(anyhow!(
            "workflow step index `{index}` is out of range; expected 1..={}",
            len + 1
        )),
        Some(index) => Ok(index - 1),
    }
}

fn normalize_existing_index(step_number: usize, len: usize) -> Result<usize> {
    if step_number == 0 || step_number > len {
        return Err(anyhow!(
            "workflow step number `{step_number}` is out of range; expected 1..={len}"
        ));
    }
    Ok(step_number - 1)
}

fn draft_to_value(draft: WorkflowStepDraft) -> Result<Value> {
    let mut object = Map::new();
    set_optional_string(&mut object, "name", Some(draft.name))?;
    object.insert(
        "prompt".to_string(),
        Value::String(normalize_required_text(draft.prompt, "workflow prompt")?),
    );
    if let Some(when) = parse_optional_json(draft.when, "workflow when condition")? {
        object.insert("when".to_string(), when);
    }
    set_optional_string(&mut object, "model", Some(draft.model))?;
    set_optional_string(&mut object, "backend", Some(draft.backend))?;
    set_optional_string(&mut object, "cwd", Some(draft.step_cwd))?;
    if draft.run_in_background {
        object.insert("run_in_background".to_string(), Value::Bool(true));
    }
    Ok(Value::Object(object))
}

fn patch_step_value(step: &mut Value, patch: WorkflowStepPatch) -> Result<()> {
    let step = step
        .as_object_mut()
        .ok_or_else(|| anyhow!("workflow step must be a JSON object"))?;

    set_optional_string(step, "name", patch.name)?;
    if let Some(prompt) = patch.prompt {
        step.insert(
            "prompt".to_string(),
            Value::String(normalize_required_text(prompt, "workflow prompt")?),
        );
    }
    set_optional_json(step, "when", patch.when, "workflow when condition")?;
    set_optional_string(step, "model", patch.model)?;
    set_optional_string(step, "backend", patch.backend)?;
    set_optional_string(step, "cwd", patch.step_cwd)?;
    if let Some(run_in_background) = patch.run_in_background {
        if run_in_background {
            step.insert("run_in_background".to_string(), Value::Bool(true));
        } else {
            step.remove("run_in_background");
        }
    }
    Ok(())
}

fn set_optional_string(
    object: &mut Map<String, Value>,
    key: &str,
    value: Option<Option<String>>,
) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };

    match normalize_optional_text(value) {
        Some(value) => {
            object.insert(key.to_string(), Value::String(value));
        }
        None => {
            object.remove(key);
        }
    }
    Ok(())
}

fn set_optional_json(
    object: &mut Map<String, Value>,
    key: &str,
    value: Option<Option<String>>,
    label: &str,
) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };

    match parse_optional_json(value, label)? {
        Some(value) => {
            object.insert(key.to_string(), value);
        }
        None => {
            object.remove(key);
        }
    }
    Ok(())
}

fn parse_optional_json(value: Option<String>, label: &str) -> Result<Option<Value>> {
    let Some(value) = normalize_optional_text(value) else {
        return Ok(None);
    };
    let parsed = serde_json::from_str::<Value>(&value)
        .with_context(|| format!("failed to parse {label} as JSON"))?;
    Ok(Some(parsed))
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_required_text(value: String, label: &str) -> Result<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        Err(anyhow!("{label} cannot be empty"))
    } else {
        Ok(value)
    }
}

fn path_text(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}
