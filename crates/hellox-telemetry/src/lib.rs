use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use hellox_agent::{AgentTelemetryEvent, StoredSessionUsageTotals, TelemetrySink};
use hellox_config::{estimate_cost_usd, pricing_for_model, telemetry_events_path, HelloxConfig};
use hellox_session::SessionSummary;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedWorkspaceStats {
    pub session_count: usize,
    pub message_count: usize,
    pub memory_count: usize,
    pub share_count: usize,
    pub request_count: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub usage_by_model: BTreeMap<String, StoredSessionUsageTotals>,
    pub largest_session: Option<(String, usize)>,
    pub newest_session: Option<(String, u64)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskTelemetrySummary {
    pub task_count: usize,
    pub task_status_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelemetryEvent {
    pub recorded_at: u64,
    pub domain: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct JsonlTelemetrySink {
    path: PathBuf,
}

impl JsonlTelemetrySink {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn default_path() -> PathBuf {
        telemetry_events_path()
    }
}

impl TelemetrySink for JsonlTelemetrySink {
    fn record(&self, event: AgentTelemetryEvent) -> Result<()> {
        append_event(
            &self.path,
            &TelemetryEvent {
                recorded_at: unix_timestamp(),
                domain: event.domain,
                name: event.name,
                session_id: event.session_id,
                attributes: event.attributes,
            },
        )
    }
}

pub fn default_jsonl_telemetry_sink() -> Arc<dyn TelemetrySink> {
    Arc::new(JsonlTelemetrySink::new(JsonlTelemetrySink::default_path()))
}

pub fn gather_persisted_workspace_stats(
    sessions_root: &Path,
    memories_root: &Path,
    shares_root: &Path,
) -> Result<PersistedWorkspaceStats> {
    let sessions = hellox_session::list_sessions(sessions_root)?;
    let memories = hellox_memory::list_memories(memories_root)?;
    let usage_by_model = aggregate_usage_by_model(&sessions);

    Ok(PersistedWorkspaceStats {
        session_count: sessions.len(),
        message_count: sessions.iter().map(|session| session.message_count).sum(),
        memory_count: memories.len(),
        share_count: count_markdown_files(shares_root),
        request_count: usage_by_model.values().map(|usage| usage.requests).sum(),
        input_tokens: usage_by_model
            .values()
            .map(|usage| usage.input_tokens)
            .sum(),
        output_tokens: usage_by_model
            .values()
            .map(|usage| usage.output_tokens)
            .sum(),
        usage_by_model,
        largest_session: sessions
            .iter()
            .max_by_key(|session| session.message_count)
            .map(|session| (session.session_id.clone(), session.message_count)),
        newest_session: sessions
            .iter()
            .max_by_key(|session| session.updated_at)
            .map(|session| (session.session_id.clone(), session.updated_at)),
    })
}

pub fn usage_report_text(
    stats: &PersistedWorkspaceStats,
    tasks: Option<&TaskTelemetrySummary>,
) -> String {
    let tasks = tasks.cloned().unwrap_or_default();
    format!(
        "persisted_sessions: {}\npersisted_messages: {}\ntracked_requests: {}\ninput_tokens: {}\noutput_tokens: {}\ncaptured_memories: {}\nshared_transcripts: {}\nworkspace_tasks: {}\ntask_statuses: {}",
        stats.session_count,
        stats.message_count,
        stats.request_count,
        stats.input_tokens,
        stats.output_tokens,
        stats.memory_count,
        stats.share_count,
        tasks.task_count,
        render_task_status_counts(&tasks.task_status_counts)
    )
}

pub fn stats_report_text(
    stats: &PersistedWorkspaceStats,
    tasks: Option<&TaskTelemetrySummary>,
) -> String {
    let tasks = tasks.cloned().unwrap_or_default();
    let average_messages = if stats.session_count == 0 {
        0.0
    } else {
        stats.message_count as f64 / stats.session_count as f64
    };

    let mut lines = vec![
        format!("sessions_total: {}", stats.session_count),
        format!("messages_total: {}", stats.message_count),
        format!("average_messages_per_session: {:.2}", average_messages),
        format!("requests_total: {}", stats.request_count),
        format!("input_tokens_total: {}", stats.input_tokens),
        format!("output_tokens_total: {}", stats.output_tokens),
        format!("memory_files_total: {}", stats.memory_count),
        format!("share_exports_total: {}", stats.share_count),
        format!("tasks_total: {}", tasks.task_count),
        format!(
            "tasks_by_status: {}",
            render_task_status_counts(&tasks.task_status_counts)
        ),
    ];

    if let Some((session_id, message_count)) = &stats.largest_session {
        lines.push(format!(
            "largest_session: {} ({} message(s))",
            session_id, message_count
        ));
    }
    if let Some((session_id, updated_at)) = &stats.newest_session {
        lines.push(format!(
            "newest_session: {} (updated_at={})",
            session_id, updated_at
        ));
    }

    lines.join("\n")
}

pub fn cost_report_text(stats: &PersistedWorkspaceStats, config: &HelloxConfig) -> String {
    let breakdown = compute_cost_breakdown(stats, config);
    let mut lines = vec![
        format!("estimated_cost_usd: {:.6}", breakdown.estimated_cost_usd),
        format!("priced_models: {}", render_list(&breakdown.priced_models)),
        format!(
            "unpriced_models: {}",
            render_list(&breakdown.unpriced_models)
        ),
        format!("tracked_requests: {}", stats.request_count),
        format!("input_tokens: {}", stats.input_tokens),
        format!("output_tokens: {}", stats.output_tokens),
    ];

    if !breakdown.model_lines.is_empty() {
        lines.push("per_model:".to_string());
        lines.extend(breakdown.model_lines);
    }

    lines.join("\n")
}

pub fn format_event_jsonl(event: &TelemetryEvent) -> Result<String> {
    Ok(serde_json::to_string(event)?)
}

pub fn append_event(path: &Path, event: &TelemetryEvent) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", format_event_jsonl(event)?)?;
    Ok(())
}

pub fn read_events(path: &Path) -> Result<Vec<TelemetryEvent>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = OpenOptions::new().read(true).open(path)?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        events.push(serde_json::from_str::<TelemetryEvent>(&line)?);
    }
    Ok(events)
}

struct CostBreakdown {
    estimated_cost_usd: f64,
    priced_models: Vec<String>,
    unpriced_models: Vec<String>,
    model_lines: Vec<String>,
}

fn compute_cost_breakdown(stats: &PersistedWorkspaceStats, config: &HelloxConfig) -> CostBreakdown {
    let mut priced_models = Vec::new();
    let mut unpriced_models = Vec::new();
    let mut model_lines = Vec::new();
    let mut estimated_cost_usd = 0.0_f64;

    for (model, usage) in &stats.usage_by_model {
        if let Some(pricing) = pricing_for_model(config, model) {
            let cost = estimate_cost_usd(pricing, usage.input_tokens, usage.output_tokens);
            estimated_cost_usd += cost;
            priced_models.push(model.clone());
            model_lines.push(format!(
                "- {} | requests={} | input_tokens={} | output_tokens={} | estimated_cost_usd={:.6}",
                model, usage.requests, usage.input_tokens, usage.output_tokens, cost
            ));
        } else {
            unpriced_models.push(model.clone());
            model_lines.push(format!(
                "- {} | requests={} | input_tokens={} | output_tokens={} | estimated_cost_usd=unpriced",
                model, usage.requests, usage.input_tokens, usage.output_tokens
            ));
        }
    }

    CostBreakdown {
        estimated_cost_usd,
        priced_models,
        unpriced_models,
        model_lines,
    }
}

fn aggregate_usage_by_model(
    sessions: &[SessionSummary],
) -> BTreeMap<String, StoredSessionUsageTotals> {
    let mut usage_by_model: BTreeMap<String, StoredSessionUsageTotals> = BTreeMap::new();
    for session in sessions {
        for (model, usage) in &session.usage_by_model {
            let entry = usage_by_model.entry(model.clone()).or_default();
            entry.requests += usage.requests;
            entry.input_tokens += usage.input_tokens;
            entry.output_tokens += usage.output_tokens;
        }
    }
    usage_by_model
}

fn count_markdown_files(root: &Path) -> usize {
    if !root.exists() {
        return 0;
    }

    fs::read_dir(root)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("md"))
        .count()
}

fn render_list(items: &[String]) -> String {
    if items.is_empty() {
        "none".to_string()
    } else {
        items.join(", ")
    }
}

fn render_task_status_counts(counts: &BTreeMap<String, usize>) -> String {
    if counts.is_empty() {
        return "none".to_string();
    }

    counts
        .iter()
        .map(|(status, count)| format!("{status}={count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use hellox_agent::{
        PlanningState, StoredSessionSnapshot, StoredSessionUsageTotals, TelemetrySink,
    };
    use hellox_config::{config_root, HelloxConfig, PermissionMode};

    use super::{
        append_event, cost_report_text, format_event_jsonl, gather_persisted_workspace_stats,
        read_events, stats_report_text, usage_report_text, JsonlTelemetrySink,
        TaskTelemetrySummary, TelemetryEvent,
    };

    fn temp_root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-telemetry-{suffix}"));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    #[test]
    fn workspace_stats_roll_up_sessions_and_tasks() {
        let root = temp_root();
        let sessions_root = root.join("sessions");
        let memory_root = root.join("memory");
        let shares_root = root.join("shares");
        fs::create_dir_all(&sessions_root).expect("create sessions root");
        fs::create_dir_all(memory_root.join("sessions")).expect("create session memory root");
        fs::create_dir_all(&shares_root).expect("create share root");
        fs::write(shares_root.join("share-1.md"), "# transcript").expect("write share");

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
            messages: vec![serde_json::from_value(serde_json::json!({
                "role": "user",
                "content": "hello"
            }))
            .expect("message")],
        };
        fs::write(
            sessions_root.join("priced-session.json"),
            serde_json::to_string_pretty(&snapshot).expect("serialize snapshot"),
        )
        .expect("write snapshot");

        let stats = gather_persisted_workspace_stats(&sessions_root, &memory_root, &shares_root)
            .expect("stats");
        let tasks = TaskTelemetrySummary {
            task_count: 1,
            task_status_counts: BTreeMap::from([(String::from("pending"), 1)]),
        };
        assert_eq!(stats.session_count, 1);
        assert!(usage_report_text(&stats, Some(&tasks)).contains("workspace_tasks: 1"));
        assert!(stats_report_text(&stats, Some(&tasks)).contains("tasks_total: 1"));
        assert!(cost_report_text(&stats, &HelloxConfig::default())
            .contains("estimated_cost_usd: 6.750000"));
    }

    #[test]
    fn telemetry_event_formats_as_jsonl() {
        let line = format_event_jsonl(&TelemetryEvent {
            recorded_at: 123,
            domain: String::from("tool"),
            name: String::from("tool_finished"),
            session_id: Some(String::from("session-1")),
            attributes: BTreeMap::from([(String::from("tool"), String::from("Read"))]),
        })
        .expect("format event");
        assert!(line.contains("\"domain\":\"tool\""));
        assert!(line.contains("\"tool\":\"Read\""));
    }

    #[test]
    fn jsonl_sink_appends_events() {
        let root = temp_root();
        let path = root.join("telemetry-events.jsonl");
        let sink = JsonlTelemetrySink::new(path.clone());
        sink.record(hellox_agent::AgentTelemetryEvent::new(
            "session",
            "turn_completed",
        ))
        .expect("record event");
        append_event(
            &path,
            &TelemetryEvent {
                recorded_at: 2,
                domain: String::from("tool"),
                name: String::from("tool_completed"),
                session_id: None,
                attributes: BTreeMap::new(),
            },
        )
        .expect("append event");

        let events = read_events(&path).expect("read events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].domain, "session");
        assert_eq!(events[1].name, "tool_completed");
    }
}
