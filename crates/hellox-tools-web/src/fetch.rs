use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hellox_tool_runtime::{required_string, LocalTool, LocalToolResult};
use reqwest::{header, Url};
use scraper::{Html, Selector};
use serde_json::{json, Value};

use crate::client::http_client;
use crate::support::collapse_whitespace;

const MAX_FETCH_BYTES: usize = 250_000;

pub struct WebFetchTool;

#[async_trait]
impl<C> LocalTool<C> for WebFetchTool
where
    C: Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "WebFetch".to_string(),
            description: Some("Fetch a URL and return extracted text content".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "HTTP or HTTPS URL to fetch." },
                    "prompt": { "type": "string", "description": "Optional caller hint describing what content matters most." },
                    "max_bytes": { "type": "integer", "description": "Maximum response bytes to read before truncating." }
                },
                "required": ["url"]
            }),
        }
    }

    async fn call(&self, input: Value, _context: &C) -> Result<LocalToolResult> {
        let raw_url = required_string(&input, "url")?;
        let prompt = input.get("prompt").and_then(Value::as_str);
        let max_bytes = input
            .get("max_bytes")
            .and_then(Value::as_u64)
            .unwrap_or(MAX_FETCH_BYTES as u64) as usize;
        let url = parse_http_url(raw_url)?;

        let client = http_client();
        let response = client
            .get(url.clone())
            .send()
            .await
            .with_context(|| format!("failed to fetch {url}"))?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        let bytes = read_response_body(response, max_bytes).await?;
        let raw = String::from_utf8_lossy(&bytes.body).into_owned();
        let extracted = extract_page_text(&content_type, &raw);

        Ok(LocalToolResult::text(
            serde_json::to_string_pretty(&json!({
                "url": url.as_str(),
                "prompt": prompt,
                "bytes": bytes.bytes_read,
                "truncated": bytes.truncated,
                "code": status.as_u16(),
                "codeText": status.canonical_reason().unwrap_or("unknown"),
                "contentType": content_type,
                "result": extracted,
            }))
            .context("failed to serialize WebFetch result")?,
        ))
    }
}

fn parse_http_url(raw: &str) -> Result<Url> {
    let url = Url::parse(raw).with_context(|| format!("invalid url `{raw}`"))?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        other => Err(anyhow!("unsupported url scheme `{other}`")),
    }
}

fn extract_page_text(content_type: &str, body: &str) -> String {
    if content_type.contains("html") || body.contains("<html") {
        extract_html_text(body)
    } else if content_type.contains("json") {
        serde_json::from_str::<Value>(body)
            .map(|value| serde_json::to_string_pretty(&value).unwrap_or_else(|_| body.to_string()))
            .unwrap_or_else(|_| body.to_string())
    } else {
        body.to_string()
    }
}

fn extract_html_text(html: &str) -> String {
    let document = Html::parse_document(html);
    let body_selector = Selector::parse("body").expect("valid selector");
    let title_selector = Selector::parse("title").expect("valid selector");
    let title = document
        .select(&title_selector)
        .next()
        .map(|node| collapse_whitespace(&node.text().collect::<Vec<_>>().join(" ")))
        .filter(|value| !value.is_empty());
    let body = document
        .select(&body_selector)
        .next()
        .map(|node| collapse_whitespace(&node.text().collect::<Vec<_>>().join(" ")))
        .unwrap_or_else(|| {
            collapse_whitespace(&document.root_element().text().collect::<Vec<_>>().join(" "))
        });

    match title {
        Some(title) if !body.starts_with(&title) => format!("Title: {title}\n\n{body}"),
        _ => body,
    }
}

async fn read_response_body(response: reqwest::Response, max_bytes: usize) -> Result<BodyRead> {
    let mut response = response;
    let mut body = Vec::new();
    let mut truncated = false;

    while let Some(chunk) = response
        .chunk()
        .await
        .context("failed to read response body")?
    {
        let remaining = max_bytes.saturating_sub(body.len());
        if remaining == 0 {
            truncated = true;
            break;
        }
        if chunk.len() > remaining {
            body.extend_from_slice(&chunk[..remaining]);
            truncated = true;
            break;
        }
        body.extend_from_slice(&chunk);
    }

    let bytes_read = body.len();
    Ok(BodyRead {
        body,
        bytes_read,
        truncated,
    })
}

struct BodyRead {
    body: Vec<u8>,
    bytes_read: usize,
    truncated: bool,
}

#[cfg(test)]
mod tests {
    use super::extract_html_text;

    #[test]
    fn html_fetch_extracts_title_and_body_text() {
        let html =
            "<html><head><title>Example</title></head><body><h1>Hello</h1><p>World</p></body></html>";
        let text = extract_html_text(html);
        assert!(text.contains("Title: Example"));
        assert!(text.contains("Hello World"));
    }
}
