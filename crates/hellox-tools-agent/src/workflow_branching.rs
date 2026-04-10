use anyhow::{anyhow, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WorkflowConditionInput {
    #[serde(default)]
    pub previous_status: Option<String>,
    #[serde(default)]
    pub previous_result_contains: Option<String>,
    #[serde(default)]
    pub step_status: Option<WorkflowStepStatusCondition>,
    #[serde(default)]
    pub step_result_contains: Option<WorkflowStepResultContainsCondition>,
    #[serde(default)]
    pub all: Option<Vec<WorkflowConditionInput>>,
    #[serde(default)]
    pub any: Option<Vec<WorkflowConditionInput>>,
    #[serde(default)]
    pub not: Option<Box<WorkflowConditionInput>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowStepStatusCondition {
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowStepResultContainsCondition {
    pub name: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct WorkflowStepState {
    pub name: String,
    pub status: String,
    pub result_text: Option<String>,
}

pub fn evaluate_step_condition(
    condition: Option<&WorkflowConditionInput>,
    history: &[WorkflowStepState],
) -> Result<Option<String>> {
    let Some(condition) = condition else {
        return Ok(None);
    };

    if condition_matches(condition, history)? {
        Ok(None)
    } else {
        Ok(Some(format!(
            "condition not met: {}",
            describe_condition(condition)
        )))
    }
}

pub fn summarize_step_statuses(history: &[WorkflowStepState]) -> serde_json::Value {
    let mut completed = 0_u64;
    let mut failed = 0_u64;
    let mut running = 0_u64;
    let mut skipped = 0_u64;

    for step in history {
        match step.status.as_str() {
            "completed" | "coordinated" => completed += 1,
            "failed" => failed += 1,
            "running" => running += 1,
            "skipped" => skipped += 1,
            _ => {}
        }
    }

    serde_json::json!({
        "total_steps": history.len(),
        "completed_steps": completed,
        "failed_steps": failed,
        "running_steps": running,
        "skipped_steps": skipped,
    })
}

fn condition_matches(
    condition: &WorkflowConditionInput,
    history: &[WorkflowStepState],
) -> Result<bool> {
    let mut predicates = Vec::new();

    if let Some(previous_status) = condition.previous_status.as_deref() {
        let previous = history
            .last()
            .ok_or_else(|| anyhow!("workflow condition references missing previous step"))?;
        predicates.push(previous.status == previous_status.trim());
    }

    if let Some(text) = condition.previous_result_contains.as_deref() {
        let expected = text.trim();
        if expected.is_empty() {
            return Err(anyhow!(
                "workflow condition `previous_result_contains` cannot be empty"
            ));
        }
        let previous = history
            .last()
            .ok_or_else(|| anyhow!("workflow condition references missing previous step"))?;
        predicates.push(
            previous
                .result_text
                .as_deref()
                .is_some_and(|value| value.contains(expected)),
        );
    }

    if let Some(step_status) = &condition.step_status {
        let step = find_named_step(history, &step_status.name)?;
        predicates.push(step.status == step_status.status.trim());
    }

    if let Some(step_result_contains) = &condition.step_result_contains {
        let expected = step_result_contains.text.trim();
        if expected.is_empty() {
            return Err(anyhow!(
                "workflow condition `step_result_contains.text` cannot be empty"
            ));
        }
        let step = find_named_step(history, &step_result_contains.name)?;
        predicates.push(
            step.result_text
                .as_deref()
                .is_some_and(|value| value.contains(expected)),
        );
    }

    if let Some(all) = &condition.all {
        if all.is_empty() {
            return Err(anyhow!("workflow condition `all` cannot be empty"));
        }
        predicates.push(
            all.iter()
                .map(|nested| condition_matches(nested, history))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .all(|matched| matched),
        );
    }

    if let Some(any) = &condition.any {
        if any.is_empty() {
            return Err(anyhow!("workflow condition `any` cannot be empty"));
        }
        predicates.push(
            any.iter()
                .map(|nested| condition_matches(nested, history))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .any(|matched| matched),
        );
    }

    if let Some(not) = &condition.not {
        predicates.push(!condition_matches(not, history)?);
    }

    if predicates.is_empty() {
        return Err(anyhow!(
            "workflow step `when` must contain at least one supported predicate"
        ));
    }

    Ok(predicates.into_iter().all(|matched| matched))
}

fn find_named_step<'a>(
    history: &'a [WorkflowStepState],
    name: &str,
) -> Result<&'a WorkflowStepState> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("workflow condition step name cannot be empty"));
    }
    history
        .iter()
        .find(|step| step.name == trimmed)
        .ok_or_else(|| anyhow!("workflow condition references unknown step `{trimmed}`"))
}

fn describe_condition(condition: &WorkflowConditionInput) -> String {
    let mut parts = Vec::new();

    if let Some(previous_status) = condition.previous_status.as_deref() {
        parts.push(format!("previous_status == {}", previous_status.trim()));
    }
    if let Some(text) = condition.previous_result_contains.as_deref() {
        parts.push(format!("previous_result contains {:?}", text.trim()));
    }
    if let Some(step_status) = &condition.step_status {
        parts.push(format!(
            "steps.{}.status == {}",
            step_status.name.trim(),
            step_status.status.trim()
        ));
    }
    if let Some(step_result_contains) = &condition.step_result_contains {
        parts.push(format!(
            "steps.{}.result contains {:?}",
            step_result_contains.name.trim(),
            step_result_contains.text.trim()
        ));
    }
    if let Some(all) = &condition.all {
        parts.push(format!(
            "all({})",
            all.iter()
                .map(describe_condition)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if let Some(any) = &condition.any {
        parts.push(format!(
            "any({})",
            any.iter()
                .map(describe_condition)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if let Some(not) = &condition.not {
        parts.push(format!("not({})", describe_condition(not)));
    }

    if parts.is_empty() {
        "empty condition".to_string()
    } else {
        parts.join(" && ")
    }
}
