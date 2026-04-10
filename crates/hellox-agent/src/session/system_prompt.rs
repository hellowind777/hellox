use hellox_tools_ui::load_brief;

use super::{workspace_brief, AgentSession};

impl AgentSession {
    pub(super) fn effective_system_prompt(&self) -> String {
        let mut prompt = self.system_prompt.clone();

        match load_brief(&self.context.working_directory) {
            Ok(Some(record)) => {
                let section = workspace_brief::workspace_brief_section(&record);
                if !section.is_empty() {
                    prompt.push_str("\n\n");
                    prompt.push_str(&section);
                }
            }
            Ok(None) => {}
            Err(error) => {
                eprintln!("Warning: failed to load workspace brief: {error}");
            }
        }

        match self.context.planning_state() {
            Ok(planning) => {
                if let Some(guidance) = planning.prompt_guidance() {
                    prompt.push_str("\n\n");
                    prompt.push_str(&guidance);
                }
            }
            Err(_) => {}
        }

        prompt
    }
}
