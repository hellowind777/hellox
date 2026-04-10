use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanItem {
    pub step: String,
    pub status: String,
}

impl PlanItem {
    pub fn validate(&self) -> Result<()> {
        if self.step.trim().is_empty() {
            return Err(anyhow!("plan step cannot be empty"));
        }
        if !matches!(self.status.trim(), "pending" | "in_progress" | "completed") {
            return Err(anyhow!(
                "unsupported plan status `{}`; use pending, in_progress, or completed",
                self.status
            ));
        }
        Ok(())
    }

    pub fn normalized(&self) -> Self {
        Self {
            step: self.step.trim().to_string(),
            status: self.status.trim().to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PlanningState {
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub plan: Vec<PlanItem>,
    #[serde(default)]
    pub allowed_prompts: Vec<String>,
}

impl PlanningState {
    pub fn enter(&mut self) {
        self.active = true;
    }

    pub fn exit(&mut self, plan: Vec<PlanItem>, allowed_prompts: Vec<String>) -> Result<()> {
        let mut normalized_plan = Vec::with_capacity(plan.len());
        for item in plan {
            let item = item.normalized();
            item.validate()?;
            normalized_plan.push(item);
        }

        self.active = false;
        self.plan = normalized_plan;
        self.allowed_prompts = normalize_allowed_prompts(allowed_prompts)?;
        Ok(())
    }

    pub fn add_step(&mut self, item: PlanItem, index: Option<usize>) -> Result<usize> {
        let item = item.normalized();
        item.validate()?;
        let step_number =
            normalize_plan_step_index(index.unwrap_or(self.plan.len() + 1), self.plan.len() + 1)?;
        self.plan.insert(step_number - 1, item);
        Ok(step_number)
    }

    pub fn update_step(&mut self, step_number: usize, item: PlanItem) -> Result<()> {
        let item = item.normalized();
        item.validate()?;
        let index = normalize_plan_step_index(step_number, self.plan.len())?;
        self.plan[index - 1] = item;
        Ok(())
    }

    pub fn remove_step(&mut self, step_number: usize) -> Result<PlanItem> {
        let index = normalize_plan_step_index(step_number, self.plan.len())?;
        Ok(self.plan.remove(index - 1))
    }

    pub fn allow_prompt(&mut self, prompt: String) -> Result<bool> {
        let prompt = normalize_allowed_prompt(prompt)?;
        if self
            .allowed_prompts
            .iter()
            .any(|existing| existing == &prompt)
        {
            return Ok(false);
        }
        self.allowed_prompts.push(prompt);
        Ok(true)
    }

    pub fn disallow_prompt(&mut self, prompt: &str) -> Result<bool> {
        let prompt = normalize_allowed_prompt(prompt.to_string())?;
        let original_len = self.allowed_prompts.len();
        self.allowed_prompts.retain(|item| item != &prompt);
        Ok(self.allowed_prompts.len() != original_len)
    }

    pub fn prompt_guidance(&self) -> Option<String> {
        if !self.active && self.plan.is_empty() {
            return None;
        }

        let mut lines = Vec::new();
        lines.push("Planning state:".to_string());
        lines.push(format!(
            "- plan_mode: {}",
            if self.active { "active" } else { "inactive" }
        ));

        if self.active {
            lines.push(
                "- instruction: plan mode is active; refine the plan before attempting a final implementation response.".to_string(),
            );
        }

        if !self.plan.is_empty() {
            lines.push("- accepted_plan:".to_string());
            for item in &self.plan {
                lines.push(format!("  - [{}] {}", item.status, item.step));
            }
        }

        if !self.allowed_prompts.is_empty() {
            lines.push(format!(
                "- allowed_prompts: {}",
                self.allowed_prompts.join(" | ")
            ));
        }

        Some(lines.join("\n"))
    }
}

fn normalize_plan_step_index(step_number: usize, max_allowed: usize) -> Result<usize> {
    if step_number == 0 || step_number > max_allowed {
        return Err(anyhow!(
            "plan step number `{step_number}` is out of range; expected 1..={max_allowed}"
        ));
    }
    Ok(step_number)
}

fn normalize_allowed_prompt(prompt: String) -> Result<String> {
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return Err(anyhow!("allowed prompt cannot be empty"));
    }
    Ok(prompt)
}

fn normalize_allowed_prompts(prompts: Vec<String>) -> Result<Vec<String>> {
    let mut normalized = Vec::new();
    for prompt in prompts {
        let prompt = normalize_allowed_prompt(prompt)?;
        if !normalized.iter().any(|existing| existing == &prompt) {
            normalized.push(prompt);
        }
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::{PlanItem, PlanningState};

    #[test]
    fn validates_and_normalizes_plan_items() {
        let item = PlanItem {
            step: "  Review code  ".to_string(),
            status: "in_progress".to_string(),
        }
        .normalized();
        item.validate().expect("valid item");
        assert_eq!(item.step, "Review code");
    }

    #[test]
    fn prompt_guidance_reflects_active_plan_state() {
        let mut state = PlanningState::default();
        state.enter();
        state
            .exit(
                vec![PlanItem {
                    step: "Write tests".to_string(),
                    status: "completed".to_string(),
                }],
                vec![String::from("continue implementation")],
            )
            .expect("exit plan mode");

        let guidance = state.prompt_guidance().expect("guidance");
        assert!(guidance.contains("accepted_plan"));
        assert!(guidance.contains("Write tests"));
        assert!(guidance.contains("continue implementation"));
    }

    #[test]
    fn step_authoring_mutates_plan_in_place() {
        let mut state = PlanningState::default();
        state
            .add_step(
                PlanItem {
                    step: "Audit docs".to_string(),
                    status: "completed".to_string(),
                },
                None,
            )
            .expect("add first");
        state
            .add_step(
                PlanItem {
                    step: "Implement workflow UI".to_string(),
                    status: "in_progress".to_string(),
                },
                Some(1),
            )
            .expect("insert");
        state
            .update_step(
                2,
                PlanItem {
                    step: "Refresh docs".to_string(),
                    status: "pending".to_string(),
                },
            )
            .expect("update");

        let removed = state.remove_step(1).expect("remove");
        assert_eq!(removed.step, "Implement workflow UI");
        assert_eq!(state.plan.len(), 1);
        assert_eq!(state.plan[0].step, "Refresh docs");
    }

    #[test]
    fn prompt_authoring_deduplicates_and_validates_entries() {
        let mut state = PlanningState::default();
        assert!(state
            .allow_prompt("continue implementation".to_string())
            .expect("add prompt"));
        assert!(!state
            .allow_prompt(" continue implementation ".to_string())
            .expect("dedupe prompt"));
        assert!(state
            .disallow_prompt("continue implementation")
            .expect("remove prompt"));
        assert!(!state
            .disallow_prompt("continue implementation")
            .expect("prompt already removed"));
    }
}
