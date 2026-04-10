use anyhow::Result;
use hellox_gateway_api::AnthropicCompatResponse;

use super::AgentSession;

impl AgentSession {
    pub fn persist_now(&mut self) -> Result<()> {
        self.persist()
    }

    pub(super) fn store_response_usage(&mut self, response: &AnthropicCompatResponse) {
        if let Some(session_store) = &mut self.session_store {
            session_store.record_usage(&response.model, &response.usage);
        }
    }

    pub(super) fn persist(&mut self) -> Result<()> {
        if let Some(session_store) = &mut self.session_store {
            session_store.snapshot.planning = self.context.planning_state()?;
            session_store.save(&self.messages)?;
        }
        Ok(())
    }
}
