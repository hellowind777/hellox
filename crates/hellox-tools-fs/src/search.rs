use anyhow::{Context, Result};
use async_trait::async_trait;
use regex::RegexBuilder;
use serde_json::{json, Value};

use crate::support::{
    collect_paths, compile_glob_pattern, matches_glob, matches_include, parse_include_patterns,
    read_searchable_text, render_context_lines, render_match_list, DEFAULT_MATCH_LIMIT,
};
use crate::FsToolContext;
use hellox_tool_runtime::{display_path, required_string, LocalTool, LocalToolResult};

pub struct GlobTool;
pub struct GrepTool;

#[async_trait]
impl<C> LocalTool<C> for GlobTool
where
    C: FsToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "Glob".to_string(),
            description: Some("Match files in the workspace using a glob pattern".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Glob pattern, for example `**/*.rs`." },
                    "path": { "type": "string", "description": "Directory to search. Defaults to the current workspace." },
                    "max_results": { "type": "integer", "description": "Maximum number of matches to return." }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let pattern_text = required_string(&input, "pattern")?;
        let raw_path = input.get("path").and_then(Value::as_str).unwrap_or(".");
        let max_results = input
            .get("max_results")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_MATCH_LIMIT as u64) as usize;

        let root = context.resolve_path(raw_path);
        let pattern = compile_glob_pattern(pattern_text)
            .with_context(|| format!("invalid glob pattern `{pattern_text}`"))?;

        let matches = collect_paths(&root)?
            .into_iter()
            .filter(|path| matches_glob(&pattern, &root, path))
            .map(|path| display_path(context.working_directory(), &path))
            .collect::<Vec<_>>();

        Ok(LocalToolResult::text(render_match_list(
            matches,
            max_results,
        )))
    }
}

#[async_trait]
impl<C> LocalTool<C> for GrepTool
where
    C: FsToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "Grep".to_string(),
            description: Some(
                "Search text files in the workspace using a regular expression".to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Regular expression to search for." },
                    "path": { "type": "string", "description": "Directory to search. Defaults to the current workspace." },
                    "include": {
                        "oneOf": [
                            { "type": "string" },
                            { "type": "array", "items": { "type": "string" } }
                        ],
                        "description": "Optional glob pattern or patterns used to filter searched files."
                    },
                    "case_sensitive": { "type": "boolean", "description": "Whether the search is case-sensitive. Defaults to true." },
                    "context": { "type": "integer", "description": "Number of surrounding lines to include before and after each match." },
                    "max_matches": { "type": "integer", "description": "Maximum number of matching lines to return." }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let pattern = required_string(&input, "pattern")?;
        let raw_path = input.get("path").and_then(Value::as_str).unwrap_or(".");
        let case_sensitive = input
            .get("case_sensitive")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let context_lines = input.get("context").and_then(Value::as_u64).unwrap_or(0) as usize;
        let max_matches = input
            .get("max_matches")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_MATCH_LIMIT as u64) as usize;

        let regex = RegexBuilder::new(pattern)
            .case_insensitive(!case_sensitive)
            .build()
            .with_context(|| format!("invalid regex pattern `{pattern}`"))?;

        let include_patterns = parse_include_patterns(input.get("include"))?;
        let root = context.resolve_path(raw_path);
        let mut output = Vec::new();
        let mut total_matches = 0usize;

        for path in collect_paths(&root)? {
            if !path.is_file() || !matches_include(&include_patterns, &root, &path) {
                continue;
            }

            let Some(text) = read_searchable_text(&path)? else {
                continue;
            };
            let lines = text.lines().collect::<Vec<_>>();

            for (index, line) in lines.iter().enumerate() {
                if !regex.is_match(line) {
                    continue;
                }

                total_matches += 1;
                if total_matches > max_matches {
                    break;
                }

                output.push(format!(
                    "{}:{}:{}",
                    display_path(context.working_directory(), &path),
                    index + 1,
                    line
                ));
                if context_lines > 0 {
                    output.extend(render_context_lines(&lines, index, context_lines));
                }
            }

            if total_matches > max_matches {
                break;
            }
        }

        if output.is_empty() {
            return Ok(LocalToolResult::text("(no matches)".to_string()));
        }
        if total_matches > max_matches {
            output.push(format!("... truncated at {max_matches} matches"));
        }
        Ok(LocalToolResult::text(output.join("\n")))
    }
}
