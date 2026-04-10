use std::path::Path;

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::workflows::WorkflowScriptDetail;

use super::{
    load_workflow_detail_from_path, load_workflow_document, normalize_existing_index,
    normalize_insert_index, normalize_optional_text, save_workflow_document, set_optional_string,
    steps_mut,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkflowDuplicateStepResult {
    pub(crate) detail: WorkflowScriptDetail,
    pub(crate) step_number: usize,
    pub(crate) duplicated_step_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkflowMoveStepResult {
    pub(crate) detail: WorkflowScriptDetail,
    pub(crate) step_number: usize,
    pub(crate) moved_step_name: Option<String>,
}

pub(crate) fn duplicate_workflow_step(
    root: &Path,
    path: &Path,
    step_number: usize,
    to_step_number: Option<usize>,
    name: Option<String>,
) -> Result<WorkflowDuplicateStepResult> {
    let mut document = load_workflow_document(path)?;
    let steps = steps_mut(&mut document)?;
    let source_index = normalize_existing_index(step_number, steps.len())?;
    let duplicate_name = match normalize_optional_text(name) {
        Some(name) => Some(name),
        None => steps[source_index]
            .as_object()
            .and_then(|step| step.get("name"))
            .and_then(Value::as_str)
            .map(|step_name| unique_duplicate_name(step_name, steps)),
    };
    let insert_at = normalize_insert_index(
        to_step_number.or_else(|| step_number.checked_add(1)),
        steps.len(),
    )?;
    let mut duplicated = steps[source_index].clone();
    let duplicated_object = duplicated
        .as_object_mut()
        .ok_or_else(|| anyhow!("workflow step must be a JSON object"))?;
    set_optional_string(duplicated_object, "name", Some(duplicate_name.clone()))?;
    steps.insert(insert_at, duplicated);
    save_workflow_document(path, &document)?;

    Ok(WorkflowDuplicateStepResult {
        detail: load_workflow_detail_from_path(root, path, None)?,
        step_number: insert_at + 1,
        duplicated_step_name: duplicate_name,
    })
}

pub(crate) fn move_workflow_step(
    root: &Path,
    path: &Path,
    step_number: usize,
    to_step_number: usize,
) -> Result<WorkflowMoveStepResult> {
    let mut document = load_workflow_document(path)?;
    let steps = steps_mut(&mut document)?;
    let source_index = normalize_existing_index(step_number, steps.len())?;
    let destination_index = normalize_existing_index(to_step_number, steps.len())?;
    let moved_step_name = steps[source_index]
        .as_object()
        .and_then(|step| step.get("name"))
        .and_then(Value::as_str)
        .map(ToString::to_string);

    let moved = steps.remove(source_index);
    let insert_at = if destination_index > source_index {
        destination_index - 1
    } else {
        destination_index
    };
    steps.insert(insert_at, moved);
    save_workflow_document(path, &document)?;

    Ok(WorkflowMoveStepResult {
        detail: load_workflow_detail_from_path(root, path, None)?,
        step_number: insert_at + 1,
        moved_step_name,
    })
}

fn unique_duplicate_name(name: &str, steps: &[Value]) -> String {
    let existing = steps
        .iter()
        .filter_map(|step| step.as_object())
        .filter_map(|step| step.get("name"))
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();

    let mut suffix = 1_usize;
    loop {
        let candidate = if suffix == 1 {
            format!("{name} copy")
        } else {
            format!("{name} copy {suffix}")
        };
        if !existing
            .iter()
            .any(|existing_name| *existing_name == candidate)
        {
            return candidate;
        }
        suffix += 1;
    }
}
