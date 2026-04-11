use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

use super::{
    derive_workflow_name_from_source, normalize_filter, normalize_required_text, path_text,
    workflow_run_path, workflow_runs_root, WorkflowRunRecord,
};
use crate::workflows::WorkflowRunTarget;

pub(crate) fn list_workflow_runs(
    root: &Path,
    filter: Option<&WorkflowRunTarget>,
    limit: usize,
) -> Result<Vec<WorkflowRunRecord>> {
    if limit == 0 {
        return Err(anyhow!("workflow runs limit must be at least 1"));
    }

    let mut records = load_all_workflow_runs(root)?;
    if let Some(filter) = filter {
        records.retain(|record| matches_workflow_filter(root, record, filter));
    }
    records.sort_by(|left, right| {
        right
            .finished_at
            .cmp(&left.finished_at)
            .then_with(|| right.run_id.cmp(&left.run_id))
    });
    records.truncate(limit);
    Ok(records)
}

pub(crate) fn load_workflow_run(root: &Path, run_id: &str) -> Result<WorkflowRunRecord> {
    let run_id = normalize_required_text(run_id, "workflow run id")?;
    if !is_safe_run_id(&run_id) {
        return Err(anyhow!(
            "workflow run id must only contain ASCII letters, numbers, `-`, or `_`"
        ));
    }

    let path = workflow_run_path(root, &run_id);
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read workflow run record {}", path_text(&path)))?;
    serde_json::from_str::<WorkflowRunRecord>(&raw)
        .with_context(|| format!("failed to parse workflow run record {}", path_text(&path)))
}

pub(crate) fn load_latest_workflow_run(
    root: &Path,
    filter: Option<&WorkflowRunTarget>,
) -> Result<WorkflowRunRecord> {
    list_workflow_runs(root, filter, 1)?
        .into_iter()
        .next()
        .ok_or_else(|| match filter {
            Some(WorkflowRunTarget::Named(name)) => {
                let name = name.trim();
                anyhow!(
                    "no workflow runs found for workflow `{name}` under `{}`",
                    path_text(&workflow_runs_root(root))
                )
            }
            Some(WorkflowRunTarget::Path(path)) => anyhow!(
                "no workflow runs found for script path `{}` under `{}`",
                path_text(path),
                path_text(&workflow_runs_root(root))
            ),
            None => anyhow!(
                "no workflow runs found under `{}`",
                path_text(&workflow_runs_root(root))
            ),
        })
}

pub(super) fn save_workflow_run(root: &Path, record: &WorkflowRunRecord) -> Result<()> {
    let path = workflow_run_path(root, &record.run_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create workflow run directory {}",
                path_text(parent)
            )
        })?;
    }
    let raw = serde_json::to_string_pretty(record).context("failed to serialize workflow run")?;
    fs::write(&path, format!("{raw}\n"))
        .with_context(|| format!("failed to write workflow run record {}", path_text(&path)))
}

fn load_all_workflow_runs(root: &Path) -> Result<Vec<WorkflowRunRecord>> {
    let runs_root = workflow_runs_root(root);
    if !runs_root.exists() {
        return Ok(Vec::new());
    }

    let mut records = Vec::new();
    for entry in fs::read_dir(&runs_root).with_context(|| {
        format!(
            "failed to read workflow run directory {}",
            path_text(&runs_root)
        )
    })? {
        let entry = entry.with_context(|| {
            format!(
                "failed to inspect workflow run entry under {}",
                path_text(&runs_root)
            )
        })?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read workflow run record {}", path_text(&path)))?;
        let record = serde_json::from_str::<WorkflowRunRecord>(&raw)
            .with_context(|| format!("failed to parse workflow run record {}", path_text(&path)))?;
        records.push(record);
    }
    Ok(records)
}

fn matches_workflow_filter(
    root: &Path,
    record: &WorkflowRunRecord,
    filter: &WorkflowRunTarget,
) -> bool {
    match filter {
        WorkflowRunTarget::Named(name) => matches_named_workflow_filter(record, name),
        WorkflowRunTarget::Path(path) => matches_path_workflow_filter(root, record, path),
    }
}

fn matches_named_workflow_filter(record: &WorkflowRunRecord, filter: &str) -> bool {
    let Some(filter) = normalize_filter(Some(filter)) else {
        return true;
    };
    record
        .workflow_name
        .as_deref()
        .is_some_and(|name| name.eq_ignore_ascii_case(&filter))
        || record
            .workflow_source
            .as_deref()
            .and_then(derive_workflow_name_from_source)
            .is_some_and(|name| name.eq_ignore_ascii_case(&filter))
}

fn matches_path_workflow_filter(
    root: &Path,
    record: &WorkflowRunRecord,
    filter_path: &Path,
) -> bool {
    let filter = normalized_path_text(root, filter_path);
    record
        .requested_script_path
        .as_deref()
        .into_iter()
        .chain(record.workflow_source.as_deref())
        .any(|candidate| normalized_path_text(root, Path::new(candidate)) == filter)
}

fn normalized_path_text(root: &Path, path: &Path) -> String {
    if path.is_absolute() {
        path_text(path)
    } else {
        path_text(&root.join(path))
    }
}

fn is_safe_run_id(value: &str) -> bool {
    value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}
