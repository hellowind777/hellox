use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use hellox_config::session_file_path;
use hellox_config::PermissionMode;
use hellox_gateway_api::{Message, MessageContent, Usage};
use hellox_style::NamedPrompt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::planning::PlanningState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSessionSnapshot {
    pub session_id: String,
    pub model: String,
    #[serde(default)]
    pub permission_mode: Option<PermissionMode>,
    #[serde(default)]
    pub output_style_name: Option<String>,
    #[serde(default)]
    pub output_style: Option<NamedPrompt>,
    #[serde(default)]
    pub persona: Option<NamedPrompt>,
    #[serde(default)]
    pub prompt_fragments: Vec<NamedPrompt>,
    #[serde(default)]
    pub config_path: Option<String>,
    #[serde(default)]
    pub planning: PlanningState,
    pub working_directory: String,
    pub shell_name: String,
    pub system_prompt: String,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub agent_runtime: Option<StoredAgentRuntime>,
    #[serde(default)]
    pub usage_by_model: BTreeMap<String, StoredSessionUsageTotals>,
    pub messages: Vec<StoredSessionMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredAgentRuntime {
    pub status: String,
    #[serde(default)]
    pub background: bool,
    #[serde(default)]
    pub resumed: bool,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub permission_mode: Option<PermissionMode>,
    #[serde(default)]
    pub started_at: Option<u64>,
    #[serde(default)]
    pub finished_at: Option<u64>,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub pane_target: Option<String>,
    #[serde(default)]
    pub layout_slot: Option<String>,
    #[serde(default)]
    pub iterations: Option<usize>,
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredSessionUsageTotals {
    #[serde(default)]
    pub requests: u64,
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSessionMessage {
    pub role: String,
    pub content: MessageContent,
}

#[derive(Debug, Clone)]
pub struct StoredSession {
    pub session_id: String,
    pub path: PathBuf,
    pub snapshot: StoredSessionSnapshot,
}

impl StoredSession {
    pub fn load(session_id: &str) -> Result<Self> {
        let path = session_file_path(session_id);
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read session {}", path.display()))?;
        let snapshot = serde_json::from_str::<StoredSessionSnapshot>(&raw)
            .with_context(|| format!("failed to parse session {}", path.display()))?;
        Ok(Self {
            session_id: session_id.to_string(),
            path,
            snapshot,
        })
    }

    pub fn create(
        session_id: Option<String>,
        model: String,
        permission_mode: PermissionMode,
        output_style: Option<NamedPrompt>,
        persona: Option<NamedPrompt>,
        prompt_fragments: Vec<NamedPrompt>,
        config_path: &Path,
        working_directory: &Path,
        shell_name: &str,
        system_prompt: String,
    ) -> Self {
        let session_id = session_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let timestamp = unix_timestamp();
        let snapshot = StoredSessionSnapshot {
            session_id: session_id.clone(),
            model,
            permission_mode: Some(permission_mode),
            output_style_name: output_style.as_ref().map(|style| style.name.clone()),
            output_style,
            persona,
            prompt_fragments,
            config_path: Some(config_path.display().to_string()),
            planning: PlanningState::default(),
            working_directory: working_directory.display().to_string(),
            shell_name: shell_name.to_string(),
            system_prompt,
            created_at: timestamp,
            updated_at: timestamp,
            agent_runtime: None,
            usage_by_model: BTreeMap::new(),
            messages: Vec::new(),
        };

        Self {
            path: session_file_path(&session_id),
            session_id,
            snapshot,
        }
    }

    pub fn save(&mut self, messages: &[Message]) -> Result<()> {
        self.preserve_runtime_from_disk_if_needed();
        self.snapshot.updated_at = unix_timestamp();
        self.snapshot.messages = messages
            .iter()
            .map(|message| StoredSessionMessage {
                role: match message.role {
                    hellox_gateway_api::MessageRole::User => "user".to_string(),
                    hellox_gateway_api::MessageRole::Assistant => "assistant".to_string(),
                },
                content: message.content.clone(),
            })
            .collect();

        self.persist_snapshot()
    }

    pub fn save_runtime(&mut self, runtime: StoredAgentRuntime) -> Result<()> {
        self.snapshot.updated_at = unix_timestamp();
        self.snapshot.agent_runtime = Some(runtime);
        self.persist_snapshot()
    }

    fn preserve_runtime_from_disk_if_needed(&mut self) {
        if self.snapshot.agent_runtime.is_some() || !self.path.exists() {
            return;
        }

        if let Ok(raw) = fs::read_to_string(&self.path) {
            if let Ok(snapshot) = serde_json::from_str::<StoredSessionSnapshot>(&raw) {
                self.snapshot.agent_runtime = snapshot.agent_runtime;
            }
        }
    }

    fn persist_snapshot(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create session dir {}", parent.display()))?;
        }
        let raw =
            serde_json::to_string_pretty(&self.snapshot).context("failed to serialize session")?;
        fs::write(&self.path, raw)
            .with_context(|| format!("failed to write session {}", self.path.display()))?;
        Ok(())
    }

    pub fn record_usage(&mut self, model: &str, usage: &Usage) {
        let entry = self
            .snapshot
            .usage_by_model
            .entry(model.to_string())
            .or_default();
        entry.requests += 1;
        entry.input_tokens += u64::from(usage.input_tokens);
        entry.output_tokens += u64::from(usage.output_tokens);
    }

    pub fn restore_messages(&self) -> Vec<Message> {
        self.snapshot
            .messages
            .iter()
            .map(|message| Message {
                role: if message.role == "assistant" {
                    hellox_gateway_api::MessageRole::Assistant
                } else {
                    hellox_gateway_api::MessageRole::User
                },
                content: message.content.clone(),
            })
            .collect()
    }
}

impl StoredSessionSnapshot {
    pub fn total_requests(&self) -> u64 {
        self.usage_by_model
            .values()
            .map(|usage| usage.requests)
            .sum()
    }

    pub fn total_input_tokens(&self) -> u64 {
        self.usage_by_model
            .values()
            .map(|usage| usage.input_tokens)
            .sum()
    }

    pub fn total_output_tokens(&self) -> u64 {
        self.usage_by_model
            .values()
            .map(|usage| usage.output_tokens)
            .sum()
    }
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
