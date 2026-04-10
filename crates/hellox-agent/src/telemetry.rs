use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::Result;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentTelemetryEvent {
    pub domain: String,
    pub name: String,
    pub session_id: Option<String>,
    pub attributes: BTreeMap<String, String>,
}

impl AgentTelemetryEvent {
    pub fn new(domain: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            domain: domain.into(),
            name: name.into(),
            session_id: None,
            attributes: BTreeMap::new(),
        }
    }

    pub fn with_session_id(mut self, session_id: Option<&str>) -> Self {
        self.session_id = session_id.map(|value| value.to_string());
        self
    }

    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }
}

pub trait TelemetrySink: Send + Sync {
    fn record(&self, event: AgentTelemetryEvent) -> Result<()>;
}

pub type SharedTelemetrySink = Arc<dyn TelemetrySink>;
