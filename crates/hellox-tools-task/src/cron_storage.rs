use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, Context, Result};
use chrono::Local;
use hellox_config::{scheduled_tasks_path_for, HelloxConfig};
use serde::{Deserialize, Serialize};

use crate::cron::{cron_to_human, next_run_after, parse_cron_expression};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ScheduledTaskRecord {
    pub(crate) id: String,
    pub(crate) cron: String,
    pub(crate) human_schedule: String,
    pub(crate) prompt: String,
    pub(crate) recurring: bool,
    pub(crate) durable: bool,
    pub(crate) created_at_ms: i64,
    pub(crate) next_run_at_ms: i64,
}

static SESSION_TASKS: OnceLock<Mutex<Vec<ScheduledTaskRecord>>> = OnceLock::new();

fn session_tasks() -> &'static Mutex<Vec<ScheduledTaskRecord>> {
    SESSION_TASKS.get_or_init(|| Mutex::new(Vec::new()))
}

pub(crate) fn list_tasks(config_path: &Path) -> Result<Vec<ScheduledTaskRecord>> {
    let mut merged = durable_tasks(config_path)?;
    merged.extend(
        session_tasks()
            .lock()
            .map_err(|_| anyhow!("session scheduled task store is poisoned"))?
            .clone(),
    );
    merged.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(merged)
}

pub(crate) fn add_task(
    config_path: &Path,
    config: &HelloxConfig,
    cron: &str,
    prompt: &str,
    recurring: bool,
    durable: bool,
) -> Result<ScheduledTaskRecord> {
    let all = list_tasks(config_path)?;
    if all.len() >= config.scheduler.max_jobs {
        return Err(anyhow!(
            "too many scheduled jobs (max {})",
            config.scheduler.max_jobs
        ));
    }

    let parsed = parse_cron_expression(cron)?;
    let now = Local::now();
    let next_run_at = next_run_after(&parsed, now)
        .ok_or_else(|| anyhow!("cron expression `{cron}` does not match within the next year"))?;

    let next_id = all
        .iter()
        .filter_map(|task| task.id.strip_prefix("cron-"))
        .filter_map(|suffix| suffix.parse::<usize>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    let record = ScheduledTaskRecord {
        id: format!("cron-{next_id}"),
        cron: cron.to_string(),
        human_schedule: cron_to_human(cron),
        prompt: prompt.trim().to_string(),
        recurring,
        durable,
        created_at_ms: now.timestamp_millis(),
        next_run_at_ms: next_run_at.timestamp_millis(),
    };

    if durable {
        let mut tasks = durable_tasks(config_path)?;
        tasks.push(record.clone());
        save_durable_tasks(config_path, &tasks)?;
    } else {
        session_tasks()
            .lock()
            .map_err(|_| anyhow!("session scheduled task store is poisoned"))?
            .push(record.clone());
    }

    Ok(record)
}

pub(crate) fn remove_task(config_path: &Path, task_id: &str) -> Result<bool> {
    let mut removed = false;

    {
        let mut tasks = session_tasks()
            .lock()
            .map_err(|_| anyhow!("session scheduled task store is poisoned"))?;
        let original_len = tasks.len();
        tasks.retain(|task| task.id != task_id);
        removed |= tasks.len() != original_len;
    }

    let mut durable = durable_tasks(config_path)?;
    let original_len = durable.len();
    durable.retain(|task| task.id != task_id);
    if durable.len() != original_len {
        save_durable_tasks(config_path, &durable)?;
        removed = true;
    }

    Ok(removed)
}

fn durable_tasks(config_path: &Path) -> Result<Vec<ScheduledTaskRecord>> {
    let path = scheduled_task_file_path(config_path);
    if !path.exists() {
        return Ok(Vec::new());
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read scheduled task file {}", path.display()))?;
    serde_json::from_str::<Vec<ScheduledTaskRecord>>(&raw)
        .with_context(|| format!("failed to parse scheduled task file {}", path.display()))
}

fn save_durable_tasks(config_path: &Path, tasks: &[ScheduledTaskRecord]) -> Result<()> {
    let path = scheduled_task_file_path(config_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create scheduled task directory {}",
                parent.display()
            )
        })?;
    }
    let raw = serde_json::to_string_pretty(tasks).context("failed to serialize scheduled tasks")?;
    fs::write(&path, raw)
        .with_context(|| format!("failed to write scheduled task file {}", path.display()))
}

pub(crate) fn scheduled_task_file_path(config_path: &Path) -> PathBuf {
    scheduled_tasks_path_for(config_path)
}

#[cfg(test)]
pub(crate) fn reset_session_tasks() {
    if let Ok(mut tasks) = session_tasks().lock() {
        tasks.clear();
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use hellox_config::HelloxConfig;

    use super::{add_task, list_tasks, remove_task, reset_session_tasks, scheduled_task_file_path};

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("hellox-cron-storage-{suffix}"));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    #[test]
    fn add_list_and_remove_durable_task() {
        reset_session_tasks();
        let root = temp_dir();
        let config_path = root.join("config.toml");
        let config = HelloxConfig::default();

        let created = add_task(
            &config_path,
            &config,
            "*/5 * * * *",
            "check logs",
            true,
            true,
        )
        .expect("create task");
        assert_eq!(created.id, "cron-1");
        assert!(scheduled_task_file_path(&config_path).exists());

        let listed = list_tasks(&config_path).expect("list tasks");
        assert_eq!(listed.len(), 1);
        assert!(remove_task(&config_path, "cron-1").expect("remove task"));
        assert!(list_tasks(&config_path)
            .expect("list after remove")
            .is_empty());
    }
}
