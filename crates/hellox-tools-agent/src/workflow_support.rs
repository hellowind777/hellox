use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hellox_config::PermissionMode;
use hellox_gateway_api::ToolDefinition;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::shared::AgentRunRequest;
use crate::workflow_branching::{WorkflowConditionInput, WorkflowStepState};

#[async_trait]
pub trait WorkflowToolContext {
    fn working_directory(&self) -> &Path;
    fn resolve_path(&self, raw: &str) -> PathBuf;
    async fn run_workflow_step(&self, request: AgentRunRequest) -> Result<Value>;
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowStepInput {
    #[serde(default)]
    pub name: Option<String>,
    pub prompt: String,
    #[serde(default)]
    pub when: Option<WorkflowConditionInput>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub permission_mode: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub max_turns: Option<u64>,
    #[serde(default)]
    pub run_in_background: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ResolvedWorkflowInput {
    pub steps: Vec<WorkflowStepInput>,
    pub continue_on_error: bool,
    pub shared_context: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct WorkflowScriptDefinition {
    #[serde(default)]
    steps: Vec<WorkflowStepInput>,
    #[serde(default)]
    continue_on_error: Option<bool>,
    #[serde(default)]
    shared_context: Option<String>,
}

pub fn workflow_tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "Workflow".to_string(),
        description: Some(
            "Run a local multi-step workflow by sequencing nested agent tasks with shared context templates.".to_string(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "script": { "type": "string" },
                "script_path": { "type": "string" },
                "steps": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "prompt": { "type": "string" },
                            "when": { "type": "object" },
                            "model": { "type": "string" },
                            "backend": { "type": "string" },
                            "permission_mode": { "type": "string" },
                            "cwd": { "type": "string" },
                            "session_id": { "type": "string" },
                            "max_turns": { "type": "integer", "minimum": 1, "maximum": 64 },
                            "run_in_background": { "type": "boolean" }
                        },
                        "required": ["prompt"]
                    }
                },
                "continue_on_error": { "type": "boolean" },
                "shared_context": { "type": "string" }
            }
        }),
    }
}

pub fn resolve_workflow_input(
    input: &Value,
    context: &impl WorkflowToolContext,
) -> Result<ResolvedWorkflowInput> {
    let (script_definition, source) = load_workflow_script(input, context)?;

    let steps = match input.get("steps").cloned() {
        Some(value) => serde_json::from_value::<Vec<WorkflowStepInput>>(value)
            .context("failed to parse `steps`")?,
        None => script_definition.steps,
    };
    if steps.is_empty() {
        return Err(anyhow!(
            "workflow requires either a non-empty `steps` array or a workflow script with steps"
        ));
    }

    Ok(ResolvedWorkflowInput {
        continue_on_error: input
            .get("continue_on_error")
            .and_then(Value::as_bool)
            .unwrap_or(script_definition.continue_on_error.unwrap_or(false)),
        shared_context: optional_string(input.get("shared_context"))
            .or_else(|| normalize_shared_context(script_definition.shared_context)),
        steps,
        source,
    })
}

pub fn render_prompt_template(
    template: &str,
    shared_context: Option<&str>,
    history: &[WorkflowStepState],
) -> Result<String> {
    let mut rendered = String::new();
    let mut remaining = template;

    while let Some(start) = remaining.find("{{") {
        rendered.push_str(&remaining[..start]);
        let placeholder_source = &remaining[start + 2..];
        let end = placeholder_source
            .find("}}")
            .ok_or_else(|| anyhow!("workflow template contains an unclosed placeholder"))?;
        let key = placeholder_source[..end].trim();
        rendered.push_str(resolve_placeholder(key, shared_context, history)?);
        remaining = &placeholder_source[end + 2..];
    }

    rendered.push_str(remaining);
    Ok(rendered)
}

pub fn parse_step_permission_mode(step: &WorkflowStepInput) -> Result<Option<PermissionMode>> {
    match step.permission_mode.as_deref() {
        Some(value) => value
            .parse::<PermissionMode>()
            .map(Some)
            .map_err(|error| anyhow!(error)),
        None => Ok(None),
    }
}

fn load_workflow_script(
    input: &Value,
    context: &impl WorkflowToolContext,
) -> Result<(WorkflowScriptDefinition, Option<String>)> {
    let script_name = optional_string(input.get("script"));
    let script_path = optional_string(input.get("script_path"));

    if script_name.is_some() && script_path.is_some() {
        return Err(anyhow!(
            "workflow accepts either `script` or `script_path`, but not both"
        ));
    }

    let Some(path) = script_name
        .map(|name| named_workflow_script_path(context.working_directory(), &name))
        .transpose()?
        .or_else(|| script_path.map(|path| context.resolve_path(&path)))
    else {
        return Ok((WorkflowScriptDefinition::default(), None));
    };

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read workflow script {}", path.display()))?;
    let definition = serde_json::from_str::<WorkflowScriptDefinition>(&raw)
        .with_context(|| format!("failed to parse workflow script {}", path.display()))?;
    Ok((
        definition,
        Some(display_relative_path(context.working_directory(), &path)),
    ))
}

fn named_workflow_script_path(root: &Path, script: &str) -> Result<PathBuf> {
    let trimmed = script.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("workflow `script` cannot be empty"));
    }

    let relative = PathBuf::from(trimmed);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(anyhow!(
            "workflow `script` must stay within `.hellox/workflows`"
        ));
    }

    let mut path = root.join(".hellox").join("workflows").join(relative);
    if path.extension().is_none() {
        path.set_extension("json");
    }
    Ok(path)
}

fn resolve_placeholder<'a>(
    key: &str,
    shared_context: Option<&'a str>,
    history: &'a [WorkflowStepState],
) -> Result<&'a str> {
    match key {
        "workflow.shared_context" => shared_context
            .ok_or_else(|| anyhow!("workflow template references missing shared_context")),
        "workflow.previous_result" => history
            .last()
            .and_then(|step| step.result_text.as_deref())
            .ok_or_else(|| anyhow!("workflow template references missing previous result")),
        "workflow.previous_status" => history
            .last()
            .map(|step| step.status.as_str())
            .ok_or_else(|| anyhow!("workflow template references missing previous status")),
        _ if key.starts_with("steps.") => resolve_step_placeholder(key, history),
        _ => Err(anyhow!(
            "workflow template placeholder `{key}` is not supported"
        )),
    }
}

fn resolve_step_placeholder<'a>(key: &str, history: &'a [WorkflowStepState]) -> Result<&'a str> {
    let path = key
        .strip_prefix("steps.")
        .ok_or_else(|| anyhow!("workflow template placeholder `{key}` is not supported"))?;
    let (name, field) = path
        .rsplit_once('.')
        .ok_or_else(|| anyhow!("workflow template placeholder `{key}` is not supported"))?;
    let step = history
        .iter()
        .find(|step| step.name == name)
        .ok_or_else(|| anyhow!("workflow template references unknown step `{name}`"))?;

    match field {
        "result" => step
            .result_text
            .as_deref()
            .ok_or_else(|| anyhow!("workflow step `{name}` does not have a text result")),
        "status" => Ok(step.status.as_str()),
        _ => Err(anyhow!(
            "workflow template placeholder `{key}` is not supported"
        )),
    }
}

fn normalize_shared_context(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn optional_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn display_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use async_trait::async_trait;
    use serde_json::json;

    use super::*;

    struct TestContext {
        working_directory: PathBuf,
    }

    #[async_trait]
    impl WorkflowToolContext for TestContext {
        fn working_directory(&self) -> &Path {
            &self.working_directory
        }

        fn resolve_path(&self, raw: &str) -> PathBuf {
            self.working_directory.join(raw)
        }

        async fn run_workflow_step(&self, _request: AgentRunRequest) -> Result<Value> {
            unreachable!("workflow support tests do not execute nested workflow steps")
        }
    }

    #[test]
    fn render_prompt_template_expands_shared_and_previous_status() {
        let rendered = render_prompt_template(
            "ctx={{workflow.shared_context}} status={{workflow.previous_status}}",
            Some("shared"),
            &[WorkflowStepState {
                name: "step-1".to_string(),
                status: "completed".to_string(),
                result_text: Some("done".to_string()),
            }],
        )
        .expect("render template");
        assert_eq!(rendered, "ctx=shared status=completed");
    }

    #[test]
    fn resolve_workflow_input_uses_inline_steps() {
        let context = TestContext {
            working_directory: PathBuf::from("D:/workspace"),
        };
        let resolved = resolve_workflow_input(
            &json!({
                "steps": [{ "prompt": "hello" }],
                "shared_context": "ctx"
            }),
            &context,
        )
        .expect("resolve workflow");
        assert_eq!(resolved.steps.len(), 1);
        assert_eq!(resolved.shared_context.as_deref(), Some("ctx"));
        assert!(resolved.source.is_none());
    }

    #[test]
    fn parse_step_permission_mode_accepts_known_modes() {
        let step = WorkflowStepInput {
            name: None,
            prompt: "hello".to_string(),
            when: None,
            model: None,
            backend: None,
            permission_mode: Some("bypass_permissions".to_string()),
            cwd: None,
            session_id: None,
            max_turns: None,
            run_in_background: None,
        };
        assert!(parse_step_permission_mode(&step).is_ok());
    }
}
