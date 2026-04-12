use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchedulerConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_max_jobs")]
    pub max_jobs: usize,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            max_jobs: default_max_jobs(),
        }
    }
}

fn default_enabled() -> bool {
    true
}

fn default_max_jobs() -> usize {
    50
}
