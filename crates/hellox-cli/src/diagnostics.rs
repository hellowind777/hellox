use std::collections::BTreeMap;
use std::env;
use std::path::Path;

use anyhow::Result;
use hellox_config::{config_root, HelloxConfig, ProviderConfig};
use hellox_telemetry::{
    cost_report_text, gather_persisted_workspace_stats, stats_report_text, usage_report_text,
    PersistedWorkspaceStats, TaskTelemetrySummary,
};

use crate::tasks::{load_tasks, todo_file_path};

mod agent_backend;

#[derive(Debug, Clone)]
pub struct WorkspaceStats {
    pub persisted: PersistedWorkspaceStats,
    pub task_count: usize,
    pub task_status_counts: BTreeMap<String, usize>,
}

pub fn gather_workspace_stats(workspace_root: &Path) -> Result<WorkspaceStats> {
    gather_workspace_stats_with_roots(
        workspace_root,
        &hellox_config::sessions_root(),
        &hellox_config::memory_root(),
        &hellox_config::shares_root(),
    )
}

fn gather_workspace_stats_with_roots(
    workspace_root: &Path,
    sessions_root: &Path,
    memories_root: &Path,
    shares_root: &Path,
) -> Result<WorkspaceStats> {
    let persisted = gather_persisted_workspace_stats(sessions_root, memories_root, shares_root)?;
    let tasks = load_tasks(workspace_root)?;

    let mut task_status_counts = BTreeMap::new();
    for task in &tasks {
        *task_status_counts
            .entry(task.status.clone())
            .or_insert(0usize) += 1;
    }

    Ok(WorkspaceStats {
        persisted,
        task_count: tasks.len(),
        task_status_counts,
    })
}

pub fn doctor_text(
    workspace_root: &Path,
    config_path: &Path,
    config: &HelloxConfig,
) -> Result<String> {
    let mut lines = Vec::new();
    lines.push(format!(
        "{} workspace_root: {}",
        ok_tag(),
        normalize_path(workspace_root)
    ));

    if config_path.exists() {
        lines.push(format!(
            "{} config_path: {}",
            ok_tag(),
            normalize_path(config_path)
        ));
    } else {
        lines.push(format!(
            "{} config_path: {} (missing, using defaults)",
            warn_tag(),
            normalize_path(config_path)
        ));
    }

    if config.gateway.listen.trim().is_empty() {
        lines.push(format!("{} gateway.listen is empty", warn_tag()));
    } else {
        lines.push(format!(
            "{} gateway.listen: {}",
            ok_tag(),
            config.gateway.listen
        ));
    }

    lines.push(format!(
        "{} permission_mode: {}",
        ok_tag(),
        config.permissions.mode
    ));
    lines.push(format!(
        "{} session.persist: {}",
        ok_tag(),
        config.session.persist
    ));
    lines.push(format!(
        "{} output_style.default: {}",
        ok_tag(),
        config.output_style.default.as_deref().unwrap_or("(none)")
    ));

    for (name, provider) in &config.providers {
        let (kind, env_var) = match provider {
            ProviderConfig::Anthropic { api_key_env, .. } => ("anthropic", api_key_env),
            ProviderConfig::OpenAiCompatible { api_key_env, .. } => {
                ("openai_compatible", api_key_env)
            }
        };
        if env::var(env_var).is_ok() {
            lines.push(format!(
                "{} provider `{name}` ({kind}) env `{env_var}` is set",
                ok_tag()
            ));
        } else {
            lines.push(format!(
                "{} provider `{name}` ({kind}) env `{env_var}` is missing",
                warn_tag()
            ));
        }
    }

    for (name, profile) in &config.profiles {
        if config.providers.contains_key(&profile.provider) {
            lines.push(format!(
                "{} profile `{name}` -> provider `{}` -> model `{}`",
                ok_tag(),
                profile.provider,
                profile.upstream_model
            ));
            lines.push(format!(
                "{} profile `{name}` pricing: {}",
                if profile.pricing.is_some() {
                    ok_tag()
                } else {
                    warn_tag()
                },
                profile
                    .pricing
                    .as_ref()
                    .map(|pricing| format!(
                        "input=${:.2}/1M output=${:.2}/1M",
                        pricing.input_per_million_usd, pricing.output_per_million_usd
                    ))
                    .unwrap_or_else(|| "(not configured)".to_string())
            ));
        } else {
            lines.push(format!(
                "{} profile `{name}` references unknown provider `{}`",
                warn_tag(),
                profile.provider
            ));
        }
    }

    for path in [
        config_root(),
        hellox_config::sessions_root(),
        hellox_config::memory_root(),
        hellox_config::shares_root(),
    ] {
        let status = if path.exists() { ok_tag() } else { warn_tag() };
        lines.push(format!(
            "{status} storage_path: {}{}",
            normalize_path(&path),
            if path.exists() {
                ""
            } else {
                " (will be created on demand)"
            }
        ));
    }

    lines.extend(agent_backend_doctor_lines());

    let task_file = todo_file_path(workspace_root);
    if task_file.exists() {
        match load_tasks(workspace_root) {
            Ok(tasks) => lines.push(format!(
                "{} task_file: {} ({} task(s))",
                ok_tag(),
                normalize_path(&task_file),
                tasks.len()
            )),
            Err(error) => lines.push(format!(
                "{} task_file: {} ({error})",
                warn_tag(),
                normalize_path(&task_file)
            )),
        }
    } else {
        lines.push(format!(
            "{} task_file: {} (not created yet)",
            warn_tag(),
            normalize_path(&task_file)
        ));
    }

    Ok(lines.join("\n"))
}

fn agent_backend_doctor_lines() -> Vec<String> {
    agent_backend::agent_backend_doctor_lines()
}

pub fn status_text(
    workspace_root: &Path,
    config_path: &Path,
    config: &HelloxConfig,
    stats: &WorkspaceStats,
) -> String {
    format!(
        "workspace_root: {}\nconfig_path: {}\ngateway_listen: {}\npermission_mode: {}\nsession_persist: {}\ndefault_output_style: {}\nprofiles: {}\npersisted_sessions: {}\nmemories: {}\nshares: {}\ntasks: {}",
        normalize_path(workspace_root),
        normalize_path(config_path),
        config.gateway.listen,
        config.permissions.mode,
        config.session.persist,
        config.output_style.default.as_deref().unwrap_or("(none)"),
        config.profiles.keys().cloned().collect::<Vec<_>>().join(", "),
        stats.persisted.session_count,
        stats.persisted.memory_count,
        stats.persisted.share_count,
        stats.task_count
    )
}

pub fn usage_text(stats: &WorkspaceStats) -> String {
    usage_report_text(&stats.persisted, Some(&task_summary(stats)))
}

pub fn stats_text(stats: &WorkspaceStats) -> String {
    stats_report_text(&stats.persisted, Some(&task_summary(stats)))
}

pub fn cost_text(stats: &WorkspaceStats, config: &HelloxConfig) -> String {
    cost_report_text(&stats.persisted, config)
}

fn task_summary(stats: &WorkspaceStats) -> TaskTelemetrySummary {
    TaskTelemetrySummary {
        task_count: stats.task_count,
        task_status_counts: stats.task_status_counts.clone(),
    }
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn ok_tag() -> &'static str {
    "[ok]"
}

fn warn_tag() -> &'static str {
    "[warn]"
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use hellox_agent::{PlanningState, StoredSessionSnapshot, StoredSessionUsageTotals};
    use hellox_config::{config_root, HelloxConfig, PermissionMode};

    use crate::tasks::{save_tasks, TaskItem};

    use super::{
        cost_text, doctor_text, gather_workspace_stats_with_roots, stats_text, usage_text,
    };

    fn temp_root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-cli-diagnostics-{suffix}"));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    #[test]
    fn gather_workspace_stats_counts_tasks() {
        let root = temp_root();
        let sessions_root = root.join("sessions");
        let memory_root = root.join("memory");
        let shares_root = root.join("shares");
        save_tasks(
            &root,
            &[TaskItem {
                id: String::from("task-1"),
                content: String::from("ship"),
                status: String::from("pending"),
                priority: None,
                description: None,
                output: None,
            }],
        )
        .expect("save tasks");

        let stats =
            gather_workspace_stats_with_roots(&root, &sessions_root, &memory_root, &shares_root)
                .expect("stats");
        assert_eq!(stats.task_count, 1);
        assert!(usage_text(&stats).contains("workspace_tasks: 1"));
        assert!(stats_text(&stats).contains("tasks_total: 1"));
        assert!(cost_text(&stats, &HelloxConfig::default()).contains("estimated_cost_usd:"));
    }

    #[test]
    fn doctor_text_reports_missing_config_as_warning() {
        let root = temp_root();
        let config_path = root.join("missing-config.toml");
        let text = doctor_text(&root, &config_path, &HelloxConfig::default()).expect("doctor");
        assert!(text.contains("[warn] config_path:"));
        assert!(text.contains("provider `anthropic`"));
        assert!(text.contains("profile `opus` pricing:"));
    }

    #[test]
    fn cost_text_reports_estimated_cost_for_priced_models() {
        let root = temp_root();
        let sessions_root = root.join("sessions");
        let memory_root = root.join("memory");
        let shares_root = root.join("shares");
        fs::create_dir_all(&sessions_root).expect("create sessions root");

        let snapshot = StoredSessionSnapshot {
            session_id: "priced-session".to_string(),
            model: "opus".to_string(),
            permission_mode: Some(PermissionMode::Default),
            output_style_name: None,
            output_style: None,
            persona: None,
            prompt_fragments: Vec::new(),
            config_path: Some(config_root().join("config.toml").display().to_string()),
            planning: PlanningState::default(),
            working_directory: root.display().to_string(),
            shell_name: "powershell".to_string(),
            system_prompt: "system".to_string(),
            created_at: 1,
            updated_at: 2,
            agent_runtime: None,
            usage_by_model: BTreeMap::from([(
                "opus".to_string(),
                StoredSessionUsageTotals {
                    requests: 2,
                    input_tokens: 200_000,
                    output_tokens: 50_000,
                },
            )]),
            messages: Vec::new(),
        };
        fs::write(
            sessions_root.join("priced-session.json"),
            serde_json::to_string_pretty(&snapshot).expect("serialize snapshot"),
        )
        .expect("write snapshot");

        let stats =
            gather_workspace_stats_with_roots(&root, &sessions_root, &memory_root, &shares_root)
                .expect("stats");
        let text = cost_text(&stats, &HelloxConfig::default());
        assert!(text.contains("estimated_cost_usd: 6.750000"), "{text}");
        assert!(text.contains("priced_models: opus"), "{text}");
        assert!(text.contains("input_tokens: 200000"), "{text}");
    }
}
