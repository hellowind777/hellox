use std::collections::BTreeMap;
use std::env;
use std::path::Path;

use anyhow::Result;
use hellox_auth::{get_provider_key, LocalAuthStoreBackend};
use hellox_config::{config_root, HelloxConfig, ProviderConfig};
use hellox_telemetry::{
    cost_report_text, gather_persisted_workspace_stats, stats_report_text, usage_report_text,
    PersistedWorkspaceStats, TaskTelemetrySummary,
};

use crate::startup::AppLanguage;
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
    language: AppLanguage,
) -> Result<String> {
    let mut lines = Vec::new();
    let auth_store = LocalAuthStoreBackend::default().load_auth_store().ok();
    lines.push(format!(
        "{} {}: {}",
        ok_tag_for(language),
        doctor_label(language, "workspace_root"),
        normalize_path(workspace_root)
    ));

    if config_path.exists() {
        lines.push(format!(
            "{} {}: {}",
            ok_tag_for(language),
            doctor_label(language, "config_path"),
            normalize_path(config_path)
        ));
    } else {
        lines.push(format!(
            "{} {}: {} {}",
            warn_tag_for(language),
            doctor_label(language, "config_path"),
            normalize_path(config_path),
            doctor_missing_config_note(language)
        ));
    }

    if config.gateway.listen.trim().is_empty() {
        lines.push(format!(
            "{} {}",
            warn_tag_for(language),
            doctor_gateway_empty_text(language)
        ));
    } else {
        lines.push(format!(
            "{} {}: {}",
            ok_tag_for(language),
            doctor_label(language, "gateway_listen"),
            config.gateway.listen
        ));
    }

    lines.push(format!(
        "{} {}: {}",
        ok_tag_for(language),
        doctor_label(language, "permission_mode"),
        config.permissions.mode
    ));
    lines.push(format!(
        "{} {}: {}",
        ok_tag_for(language),
        doctor_label(language, "session_persist"),
        config.session.persist
    ));
    lines.push(format!(
        "{} {}: {}",
        ok_tag_for(language),
        doctor_label(language, "default_output_style"),
        config
            .output_style
            .default
            .as_deref()
            .unwrap_or(none_text(language))
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
                "{} {}",
                ok_tag_for(language),
                doctor_provider_env_set_text(language, name, kind, env_var)
            ));
        } else if auth_store
            .as_ref()
            .and_then(|store| get_provider_key(store, name))
            .is_some()
        {
            lines.push(format!(
                "{} {}",
                ok_tag_for(language),
                doctor_provider_auth_store_set_text(language, name, kind)
            ));
        } else {
            lines.push(format!(
                "{} {}",
                warn_tag_for(language),
                doctor_provider_key_missing_text(language, name, kind, env_var)
            ));
        }
    }

    for (name, profile) in &config.profiles {
        if config.providers.contains_key(&profile.provider) {
            lines.push(format!(
                "{} {}",
                ok_tag_for(language),
                doctor_profile_target_text(
                    language,
                    name,
                    &profile.provider,
                    &profile.upstream_model
                )
            ));
            lines.push(format!(
                "{} {}",
                if profile.pricing.is_some() {
                    ok_tag_for(language)
                } else {
                    warn_tag_for(language)
                },
                doctor_profile_pricing_text(language, name, profile.pricing.as_ref())
            ));
        } else {
            lines.push(format!(
                "{} {}",
                warn_tag_for(language),
                doctor_unknown_provider_text(language, name, &profile.provider)
            ));
        }
    }

    for path in [
        config_root(),
        hellox_config::sessions_root(),
        hellox_config::memory_root(),
        hellox_config::shares_root(),
    ] {
        let status = if path.exists() {
            ok_tag_for(language)
        } else {
            warn_tag_for(language)
        };
        lines.push(format!(
            "{status} {}: {}{}",
            doctor_label(language, "storage_path"),
            normalize_path(&path),
            if path.exists() {
                ""
            } else {
                storage_missing_suffix(language)
            }
        ));
    }

    lines.extend(agent_backend_doctor_lines(language));

    let task_file = todo_file_path(workspace_root);
    if task_file.exists() {
        match load_tasks(workspace_root) {
            Ok(tasks) => lines.push(format!(
                "{} {}: {} ({})",
                ok_tag_for(language),
                doctor_label(language, "task_file"),
                normalize_path(&task_file),
                task_count_suffix(language, tasks.len())
            )),
            Err(error) => lines.push(format!(
                "{} {}: {} ({error})",
                warn_tag_for(language),
                doctor_label(language, "task_file"),
                normalize_path(&task_file)
            )),
        }
    } else {
        lines.push(format!(
            "{} {}: {} {}",
            warn_tag_for(language),
            doctor_label(language, "task_file"),
            normalize_path(&task_file),
            task_file_missing_suffix(language)
        ));
    }

    Ok(lines.join("\n"))
}

fn agent_backend_doctor_lines(language: AppLanguage) -> Vec<String> {
    agent_backend::agent_backend_doctor_lines(language)
}

pub fn status_text(
    workspace_root: &Path,
    config_path: &Path,
    config: &HelloxConfig,
    stats: &WorkspaceStats,
    language: AppLanguage,
) -> String {
    match language {
        AppLanguage::English => format!(
            "workspace_root: {}\nconfig_path: {}\ngateway_listen: {}\npermission_mode: {}\nsession_persist: {}\ndefault_output_style: {}\nprofiles: {}\npersisted_sessions: {}\nmemories: {}\nshares: {}\ntasks: {}",
            normalize_path(workspace_root),
            normalize_path(config_path),
            config.gateway.listen,
            config.permissions.mode,
            config.session.persist,
            config.output_style.default.as_deref().unwrap_or(none_text(language)),
            config.profiles.keys().cloned().collect::<Vec<_>>().join(", "),
            stats.persisted.session_count,
            stats.persisted.memory_count,
            stats.persisted.share_count,
            stats.task_count
        ),
        AppLanguage::SimplifiedChinese => format!(
            "工作区目录：{}\n配置路径：{}\ngateway 监听：{}\n权限模式：{}\n会话持久化：{}\n默认输出风格：{}\n模型档案：{}\n已持久化会话：{}\n记忆数：{}\n分享导出数：{}\n任务数：{}",
            normalize_path(workspace_root),
            normalize_path(config_path),
            config.gateway.listen,
            config.permissions.mode,
            config.session.persist,
            config.output_style.default.as_deref().unwrap_or(none_text(language)),
            config.profiles.keys().cloned().collect::<Vec<_>>().join(", "),
            stats.persisted.session_count,
            stats.persisted.memory_count,
            stats.persisted.share_count,
            stats.task_count
        ),
    }
}

pub fn usage_text(stats: &WorkspaceStats, language: AppLanguage) -> String {
    localize_metric_report(
        usage_report_text(&stats.persisted, Some(&task_summary(stats))),
        language,
    )
}

pub fn stats_text(stats: &WorkspaceStats, language: AppLanguage) -> String {
    localize_metric_report(
        stats_report_text(&stats.persisted, Some(&task_summary(stats))),
        language,
    )
}

pub fn cost_text(stats: &WorkspaceStats, config: &HelloxConfig, language: AppLanguage) -> String {
    localize_metric_report(cost_report_text(&stats.persisted, config), language)
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

fn ok_tag_for(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "[ok]",
        AppLanguage::SimplifiedChinese => "[通过]",
    }
}

fn warn_tag_for(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "[warn]",
        AppLanguage::SimplifiedChinese => "[警告]",
    }
}

fn doctor_label(language: AppLanguage, key: &str) -> String {
    match language {
        AppLanguage::English => match key {
            "workspace_root" => "workspace_root".to_string(),
            "config_path" => "config_path".to_string(),
            "gateway_listen" => "gateway.listen".to_string(),
            "permission_mode" => "permission_mode".to_string(),
            "session_persist" => "session.persist".to_string(),
            "default_output_style" => "output_style.default".to_string(),
            "storage_path" => "storage_path".to_string(),
            "task_file" => "task_file".to_string(),
            _ => key.to_string(),
        },
        AppLanguage::SimplifiedChinese => match key {
            "workspace_root" => "工作区目录".to_string(),
            "config_path" => "配置路径".to_string(),
            "gateway_listen" => "gateway 监听".to_string(),
            "permission_mode" => "权限模式".to_string(),
            "session_persist" => "会话持久化".to_string(),
            "default_output_style" => "默认输出风格".to_string(),
            "storage_path" => "存储路径".to_string(),
            "task_file" => "任务文件".to_string(),
            _ => key.to_string(),
        },
    }
}

fn doctor_missing_config_note(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "(missing, using defaults)",
        AppLanguage::SimplifiedChinese => "（缺失，当前使用默认配置）",
    }
}

fn doctor_gateway_empty_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "gateway.listen is empty",
        AppLanguage::SimplifiedChinese => "gateway.listen 为空",
    }
}

fn doctor_provider_env_set_text(
    language: AppLanguage,
    name: &str,
    kind: &str,
    env_var: &str,
) -> String {
    match language {
        AppLanguage::English => format!("provider `{name}` ({kind}) env `{env_var}` is set"),
        AppLanguage::SimplifiedChinese => {
            format!("provider `{name}`（{kind}）的环境变量 `{env_var}` 已设置")
        }
    }
}

fn doctor_provider_auth_store_set_text(language: AppLanguage, name: &str, kind: &str) -> String {
    match language {
        AppLanguage::English => format!("provider `{name}` ({kind}) auth store key is set"),
        AppLanguage::SimplifiedChinese => {
            format!("provider `{name}`（{kind}）的 auth store key 已设置")
        }
    }
}

fn doctor_provider_key_missing_text(
    language: AppLanguage,
    name: &str,
    kind: &str,
    env_var: &str,
) -> String {
    match language {
        AppLanguage::English => {
            format!("provider `{name}` ({kind}) env `{env_var}` and auth store key are missing")
        }
        AppLanguage::SimplifiedChinese => {
            format!("provider `{name}`（{kind}）缺少环境变量 `{env_var}`，auth store key 也未设置")
        }
    }
}

fn doctor_profile_target_text(
    language: AppLanguage,
    name: &str,
    provider: &str,
    model: &str,
) -> String {
    match language {
        AppLanguage::English => {
            format!("profile `{name}` -> provider `{provider}` -> model `{model}`")
        }
        AppLanguage::SimplifiedChinese => {
            format!("模型档案 `{name}` -> provider `{provider}` -> 模型 `{model}`")
        }
    }
}

fn doctor_profile_pricing_text(
    language: AppLanguage,
    name: &str,
    pricing: Option<&hellox_config::ModelPricing>,
) -> String {
    let pricing_text = pricing
        .map(|pricing| {
            format!(
                "input=${:.2}/1M output=${:.2}/1M",
                pricing.input_per_million_usd, pricing.output_per_million_usd
            )
        })
        .unwrap_or_else(|| match language {
            AppLanguage::English => "(not configured)".to_string(),
            AppLanguage::SimplifiedChinese => "（未配置）".to_string(),
        });

    match language {
        AppLanguage::English => format!("profile `{name}` pricing: {pricing_text}"),
        AppLanguage::SimplifiedChinese => format!("模型档案 `{name}` 定价：{pricing_text}"),
    }
}

fn doctor_unknown_provider_text(language: AppLanguage, name: &str, provider: &str) -> String {
    match language {
        AppLanguage::English => {
            format!("profile `{name}` references unknown provider `{provider}`")
        }
        AppLanguage::SimplifiedChinese => {
            format!("模型档案 `{name}` 引用了未知 provider `{provider}`")
        }
    }
}

fn storage_missing_suffix(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => " (will be created on demand)",
        AppLanguage::SimplifiedChinese => "（按需创建）",
    }
}

fn task_count_suffix(language: AppLanguage, count: usize) -> String {
    match language {
        AppLanguage::English => format!("{count} task(s)"),
        AppLanguage::SimplifiedChinese => format!("{count} 个任务"),
    }
}

fn task_file_missing_suffix(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "(not created yet)",
        AppLanguage::SimplifiedChinese => "（尚未创建）",
    }
}

fn none_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "(none)",
        AppLanguage::SimplifiedChinese => "（无）",
    }
}

fn localize_metric_report(report: String, language: AppLanguage) -> String {
    if matches!(language, AppLanguage::English) {
        return report;
    }

    report
        .lines()
        .map(|line| {
            if let Some(rest) = line.strip_prefix("- ") {
                return format!("- {}", localize_metric_detail(rest));
            }
            if let Some((key, value)) = line.split_once(':') {
                return format!(
                    "{}：{}",
                    localized_metric_key(key.trim()),
                    localize_metric_value(value.trim())
                );
            }
            localize_metric_value(line.trim())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn localize_metric_detail(line: &str) -> String {
    line.replace("requests=", "请求数=")
        .replace("input_tokens=", "输入 tokens=")
        .replace("output_tokens=", "输出 tokens=")
        .replace("estimated_cost_usd=", "预估成本(USD)=")
        .replace("estimated_cost_usd=unpriced", "预估成本(USD)=未定价")
}

fn localized_metric_key(key: &str) -> String {
    match key {
        "persisted_sessions" => "已持久化会话".to_string(),
        "persisted_messages" => "已持久化消息".to_string(),
        "tracked_requests" => "已跟踪请求".to_string(),
        "input_tokens" => "输入 tokens".to_string(),
        "output_tokens" => "输出 tokens".to_string(),
        "captured_memories" => "已捕获记忆".to_string(),
        "shared_transcripts" => "已导出转录".to_string(),
        "workspace_tasks" => "工作区任务".to_string(),
        "task_statuses" => "任务状态".to_string(),
        "sessions_total" => "会话总数".to_string(),
        "messages_total" => "消息总数".to_string(),
        "average_messages_per_session" => "平均每会话消息数".to_string(),
        "requests_total" => "请求总数".to_string(),
        "input_tokens_total" => "输入 tokens 总数".to_string(),
        "output_tokens_total" => "输出 tokens 总数".to_string(),
        "memory_files_total" => "记忆文件总数".to_string(),
        "share_exports_total" => "分享导出总数".to_string(),
        "tasks_total" => "任务总数".to_string(),
        "tasks_by_status" => "按状态统计任务".to_string(),
        "largest_session" => "最大会话".to_string(),
        "newest_session" => "最新会话".to_string(),
        "estimated_cost_usd" => "预估成本(USD)".to_string(),
        "priced_models" => "已定价模型".to_string(),
        "unpriced_models" => "未定价模型".to_string(),
        "per_model" => "按模型明细".to_string(),
        _ => key.to_string(),
    }
}

fn localize_metric_value(value: &str) -> String {
    value
        .replace("none", "无")
        .replace("message(s)", "条消息")
        .replace("updated_at=", "更新时间=")
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

    use crate::startup::AppLanguage;
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
        assert!(usage_text(&stats, AppLanguage::English).contains("workspace_tasks: 1"));
        assert!(stats_text(&stats, AppLanguage::English).contains("tasks_total: 1"));
        assert!(
            cost_text(&stats, &HelloxConfig::default(), AppLanguage::English)
                .contains("estimated_cost_usd:")
        );
    }

    #[test]
    fn doctor_text_reports_missing_config_as_warning() {
        let root = temp_root();
        let config_path = root.join("missing-config.toml");
        let text = doctor_text(
            &root,
            &config_path,
            &HelloxConfig::default(),
            AppLanguage::English,
        )
        .expect("doctor");
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
        let text = cost_text(&stats, &HelloxConfig::default(), AppLanguage::English);
        assert!(text.contains("estimated_cost_usd: 6.750000"), "{text}");
        assert!(text.contains("priced_models: opus"), "{text}");
        assert!(text.contains("input_tokens: 200000"), "{text}");
    }
}
