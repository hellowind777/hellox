use anyhow::Result;
use async_trait::async_trait;
use hellox_config::{discover_skills, find_skill};
use hellox_tool_runtime::{required_string, LocalTool, LocalToolResult};
use serde_json::{json, Value};

use crate::UiToolContext;

pub struct SkillTool;

#[async_trait]
impl<C> LocalTool<C> for SkillTool
where
    C: UiToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "Skill".to_string(),
            description: Some(
                "Load a local skill from ~/.hellox/skills or .hellox/skills and return its prompt payload."
                    .to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "skill": { "type": "string" },
                    "args": {
                        "description": "Optional arguments passed into the selected skill."
                    }
                },
                "required": ["skill"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let skill_name = required_string(&input, "skill")?;
        let skills = discover_skills(context.working_directory())?;
        let skill = find_skill(&skills, skill_name)?;
        let args = input.get("args").cloned().unwrap_or(Value::Null);
        let resolved_prompt = resolved_prompt(&skill.body, &args)?;

        Ok(LocalToolResult::text(serde_json::to_string_pretty(
            &json!({
                "name": skill.name,
                "scope": skill.scope,
                "path": normalize_path(&skill.path),
                "description": skill.description,
                "when_to_use": skill.when_to_use,
                "allowed_tools": skill.allowed_tools,
                "hooks": skill.hooks,
                "body": skill.body,
                "args": args,
                "resolved_prompt": resolved_prompt,
            }),
        )?))
    }
}

fn resolved_prompt(body: &str, args: &Value) -> Result<String> {
    let trimmed = body.trim();
    let rendered_args = render_args(args)?;
    if rendered_args.is_empty() {
        return Ok(trimmed.to_string());
    }
    if trimmed.is_empty() {
        return Ok(format!("Arguments:\n{rendered_args}"));
    }
    Ok(format!("{trimmed}\n\nArguments:\n{rendered_args}"))
}

fn render_args(args: &Value) -> Result<String> {
    match args {
        Value::Null => Ok(String::new()),
        Value::String(text) => Ok(text.trim().to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Number(value) => Ok(value.to_string()),
        Value::Array(_) | Value::Object(_) => {
            serde_json::to_string_pretty(args).map_err(Into::into)
        }
    }
}

fn normalize_path(path: &std::path::Path) -> String {
    path.display().to_string().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{render_args, resolved_prompt};

    #[test]
    fn renders_structured_args_as_pretty_json() {
        let rendered =
            render_args(&serde_json::json!({"focus": ["diff", "tests"]})).expect("render args");
        assert!(rendered.contains("\"focus\""), "{rendered}");
        assert!(rendered.contains("\"diff\""), "{rendered}");
    }

    #[test]
    fn appends_args_to_skill_body() {
        let resolved = resolved_prompt("Review the patch.", &Value::String("src/main.rs".into()))
            .expect("resolve prompt");
        assert!(resolved.contains("Review the patch."));
        assert!(resolved.contains("Arguments:"));
        assert!(resolved.contains("src/main.rs"));
    }
}
