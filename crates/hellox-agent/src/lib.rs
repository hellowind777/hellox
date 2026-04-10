mod client;
mod permissions;
mod planning;
mod prompt;
mod session;
mod storage;
mod telemetry;
mod tools;
mod worker_job;

pub use client::GatewayClient;
pub use hellox_compact::{compact_messages, CompactMode, CompactResult};
pub use hellox_query::QueryTurnResult as AgentTurnResult;
pub use permissions::{
    ApprovalHandler, ConsoleApprovalHandler, PermissionPolicy, QuestionHandler, UserQuestion,
};
pub use planning::{PlanItem, PlanningState};
pub use prompt::{build_default_system_prompt, OutputStylePrompt, PersonaPrompt, PromptFragment};
pub use session::{AgentOptions, AgentSession};
pub use storage::{
    StoredAgentRuntime, StoredSession, StoredSessionMessage, StoredSessionSnapshot,
    StoredSessionUsageTotals,
};
pub use telemetry::{AgentTelemetryEvent, SharedTelemetrySink, TelemetrySink};
pub use tools::{
    default_tool_registry, pane_backend_preflight, PaneBackendPreflight, PaneCommandPrefixStatus,
    ToolExecutionContext, ToolRegistry,
};
pub use worker_job::{detached_job_path, DetachedAgentJob};
