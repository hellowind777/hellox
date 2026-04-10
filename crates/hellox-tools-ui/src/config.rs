use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hellox_config::{load_or_default, save_config, HelloxConfig, PermissionMode};
use serde_json::{json, Value};

use crate::UiToolContext;
use hellox_tool_runtime::{LocalTool, LocalToolResult};

pub struct ConfigTool;

#[async_trait]
impl<C> LocalTool<C> for ConfigTool
where
    C: UiToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "Config".to_string(),
            description: Some("Inspect or update a small set of local config values.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": { "type": "string" },
                    "value": {},
                    "clear": { "type": "boolean" }
                }
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let config_path = context.config_path().to_path_buf();
        let mut config = load_or_default(Some(config_path.clone()))?;
        let key = optional_string(&input, "key");
        if key.is_none() {
            return Ok(LocalToolResult::text(render_json(json!({
                "config_path": normalize_path(context.config_path()),
                "config": config,
            }))?));
        }

        let key = key.expect("checked above");
        let clear = input.get("clear").and_then(Value::as_bool).unwrap_or(false);
        if !clear
            && !input
                .as_object()
                .is_some_and(|map| map.contains_key("value"))
        {
            return Err(anyhow!("config updates require a `value` or `clear: true`"));
        }

        apply_config_update(&mut config, &key, input.get("value"), clear)?;
        context.ensure_write_allowed(&config_path).await?;
        save_config(Some(config_path), &config)?;

        Ok(LocalToolResult::text(render_json(json!({
            "config_path": normalize_path(context.config_path()),
            "updated_key": key,
            "config": config,
        }))?))
    }
}

fn apply_config_update(
    config: &mut HelloxConfig,
    key: &str,
    value: Option<&Value>,
    clear: bool,
) -> Result<()> {
    match key {
        "session.model" => {
            config.session.model = required_value_string(value, clear, key)?;
        }
        "session.persist" => {
            config.session.persist = required_value_bool(value, clear, key)?;
        }
        "permissions.mode" => {
            let value = required_value_string(value, clear, key)?;
            config.permissions.mode = value
                .parse::<PermissionMode>()
                .map_err(anyhow::Error::msg)?;
        }
        "gateway.listen" => {
            config.gateway.listen = required_value_string(value, clear, key)?;
        }
        "output_style.default" => {
            config.output_style.default = if clear {
                None
            } else {
                Some(required_value_string(value, false, key)?)
            };
        }
        "prompt.persona" => {
            config.prompt.persona = if clear {
                None
            } else {
                Some(required_value_string(value, false, key)?)
            };
        }
        "prompt.fragments" => {
            config.prompt.fragments = if clear {
                Vec::new()
            } else {
                required_value_string_list(value, false, key)?
            };
        }
        _ => {
            return Err(anyhow!(
                "unsupported config key `{key}`; supported keys: session.model, session.persist, permissions.mode, gateway.listen, output_style.default, prompt.persona, prompt.fragments"
            ));
        }
    }

    Ok(())
}

fn required_value_string(value: Option<&Value>, clear: bool, key: &str) -> Result<String> {
    if clear {
        return Err(anyhow!("config key `{key}` does not support `clear: true`"));
    }
    let text = value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| anyhow!("config key `{key}` requires a non-empty string value"))?;
    Ok(text.to_string())
}

fn required_value_bool(value: Option<&Value>, clear: bool, key: &str) -> Result<bool> {
    if clear {
        return Err(anyhow!("config key `{key}` does not support `clear: true`"));
    }
    value
        .and_then(Value::as_bool)
        .ok_or_else(|| anyhow!("config key `{key}` requires a boolean value"))
}

fn required_value_string_list(
    value: Option<&Value>,
    clear: bool,
    key: &str,
) -> Result<Vec<String>> {
    if clear {
        return Err(anyhow!("config key `{key}` does not support `clear: true`"));
    }

    let values = match value {
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                    .map(ToString::to_string)
                    .ok_or_else(|| {
                        anyhow!("config key `{key}` requires an array of non-empty strings")
                    })
            })
            .collect::<Result<Vec<_>>>()?,
        Some(Value::String(text)) => text
            .split(',')
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        _ => {
            return Err(anyhow!(
                "config key `{key}` requires a non-empty string array value"
            ));
        }
    };

    if values.is_empty() {
        return Err(anyhow!(
            "config key `{key}` requires at least one non-empty string value"
        ));
    }

    Ok(values)
}

fn optional_string(input: &Value, key: &str) -> Option<String> {
    input
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn normalize_path(path: &std::path::Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn render_json(value: Value) -> Result<String> {
    serde_json::to_string_pretty(&value).map_err(Into::into)
}
