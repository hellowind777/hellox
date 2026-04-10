use hellox_gateway_api::ToolResultContent;

use super::AgentSession;
use crate::telemetry::AgentTelemetryEvent;

impl AgentSession {
    pub(super) fn emit_tool_event(&self, name: &str, is_error: bool, content: &ToolResultContent) {
        self.emit_telemetry(
            AgentTelemetryEvent::new(
                "tool",
                if is_error {
                    "tool_failed"
                } else {
                    "tool_completed"
                },
            )
            .with_session_id(self.session_id())
            .with_attribute("tool", name.to_string())
            .with_attribute(
                "content_kind",
                match content {
                    ToolResultContent::Text(_) => "text",
                    ToolResultContent::Blocks(_) => "blocks",
                    ToolResultContent::Empty => "empty",
                },
            ),
        );
    }

    pub(super) fn emit_telemetry(&self, event: AgentTelemetryEvent) {
        let Some(telemetry_sink) = &self.telemetry_sink else {
            return;
        };
        if let Err(error) = telemetry_sink.record(event) {
            eprintln!("Warning: failed to persist session telemetry event: {error}");
        }
    }
}
