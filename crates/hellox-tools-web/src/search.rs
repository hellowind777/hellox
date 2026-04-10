use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hellox_tool_runtime::{required_string, LocalTool, LocalToolResult};
use reqwest::Url;
use scraper::{Html, Selector};
use serde_json::{json, Value};

use crate::client::http_client;
use crate::support::collapse_whitespace;

const DEFAULT_SEARCH_LIMIT: usize = 8;

pub struct WebSearchTool;

#[async_trait]
impl<C> LocalTool<C> for WebSearchTool
where
    C: Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "WebSearch".to_string(),
            description: Some("Search the web and return filtered result hits".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query." },
                    "allowed_domains": { "oneOf": [{ "type": "string" }, { "type": "array", "items": { "type": "string" }}]},
                    "blocked_domains": { "oneOf": [{ "type": "string" }, { "type": "array", "items": { "type": "string" }}]},
                    "max_results": { "type": "integer", "description": "Maximum number of hits to return." }
                },
                "required": ["query"]
            }),
        }
    }

    async fn call(&self, input: Value, _context: &C) -> Result<LocalToolResult> {
        let query = required_string(&input, "query")?;
        let allowed_domains = parse_domain_list(input.get("allowed_domains"))?;
        let blocked_domains = parse_domain_list(input.get("blocked_domains"))?;
        let max_results = input
            .get("max_results")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_SEARCH_LIMIT as u64) as usize;

        let client = http_client();
        let response = client
            .get("https://html.duckduckgo.com/html/")
            .query(&[("q", query)])
            .send()
            .await
            .context("failed to query DuckDuckGo search")?;
        let status = response.status();
        let html = response
            .text()
            .await
            .context("failed to read search response")?;
        if !status.is_success() {
            return Err(anyhow!(
                "search request failed with HTTP {}",
                status.as_u16()
            ));
        }

        let hits = parse_duckduckgo_results(&html)?
            .into_iter()
            .filter(|hit| domain_allowed(hit.domain.as_deref(), &allowed_domains, &blocked_domains))
            .take(max_results)
            .map(|hit| {
                json!({
                    "title": hit.title,
                    "url": hit.url,
                    "domain": hit.domain,
                    "snippet": hit.snippet,
                })
            })
            .collect::<Vec<_>>();

        Ok(LocalToolResult::text(
            serde_json::to_string_pretty(&json!({
                "query": query,
                "engine": "duckduckgo_html",
                "total_results": hits.len(),
                "hits": hits,
            }))
            .context("failed to serialize WebSearch result")?,
        ))
    }
}

fn parse_domain_list(value: Option<&Value>) -> Result<Vec<String>> {
    match value {
        None => Ok(Vec::new()),
        Some(Value::String(domain)) => Ok(vec![normalize_domain(domain)]),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(normalize_domain)
                    .ok_or_else(|| anyhow!("domain filters must be strings"))
            })
            .collect(),
        Some(_) => Err(anyhow!("domain filters must be a string or string array")),
    }
}

fn normalize_domain(domain: &str) -> String {
    domain.trim().trim_matches('.').to_ascii_lowercase()
}

fn domain_allowed(domain: Option<&str>, allowed: &[String], blocked: &[String]) -> bool {
    let Some(domain) = domain.map(normalize_domain) else {
        return allowed.is_empty();
    };
    if blocked.iter().any(|item| domain_matches(&domain, item)) {
        return false;
    }
    allowed.is_empty() || allowed.iter().any(|item| domain_matches(&domain, item))
}

fn domain_matches(domain: &str, filter: &str) -> bool {
    domain == filter || domain.ends_with(&format!(".{filter}"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchHit {
    title: String,
    url: String,
    domain: Option<String>,
    snippet: String,
}

fn parse_duckduckgo_results(html: &str) -> Result<Vec<SearchHit>> {
    let document = Html::parse_document(html);
    let result_selector = Selector::parse(".result").expect("valid selector");
    let link_selector = Selector::parse("a.result__a").expect("valid selector");
    let snippet_selector = Selector::parse(".result__snippet").expect("valid selector");

    let mut hits = Vec::new();
    for result in document.select(&result_selector) {
        let Some(link) = result.select(&link_selector).next() else {
            continue;
        };
        let title = collapse_whitespace(&link.text().collect::<Vec<_>>().join(" "));
        if title.is_empty() {
            continue;
        }
        let Some(href) = link.value().attr("href") else {
            continue;
        };
        let url = decode_duckduckgo_result_url(href)?;
        let domain = Url::parse(&url)
            .ok()
            .and_then(|parsed| parsed.host_str().map(normalize_domain));
        let snippet = result
            .select(&snippet_selector)
            .next()
            .map(|node| collapse_whitespace(&node.text().collect::<Vec<_>>().join(" ")))
            .unwrap_or_default();

        hits.push(SearchHit {
            title,
            url,
            domain,
            snippet,
        });
    }
    Ok(hits)
}

fn decode_duckduckgo_result_url(raw: &str) -> Result<String> {
    let raw = if raw.starts_with("//") {
        format!("https:{raw}")
    } else {
        raw.to_string()
    };
    let parsed = Url::parse(&raw).with_context(|| format!("invalid search result url `{raw}`"))?;
    if parsed.domain() == Some("duckduckgo.com") {
        if let Some((_, value)) = parsed.query_pairs().find(|(key, _)| key == "uddg") {
            return Ok(value.into_owned());
        }
    }
    Ok(parsed.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        decode_duckduckgo_result_url, domain_allowed, parse_duckduckgo_results, SearchHit,
    };

    #[test]
    fn search_parser_extracts_hits_and_decodes_redirects() {
        let hits = parse_duckduckgo_results(&compile_sample_search_html()).expect("parse hits");
        assert_eq!(hits.len(), 2);
        assert_eq!(
            hits[0],
            SearchHit {
                title: "OpenAI Docs".to_string(),
                url: "https://platform.openai.com/docs".to_string(),
                domain: Some("platform.openai.com".to_string()),
                snippet: "Official API documentation.".to_string(),
            }
        );
    }

    #[test]
    fn domain_filters_allow_subdomains_and_block_matches() {
        assert!(domain_allowed(
            Some("platform.openai.com"),
            &[String::from("openai.com")],
            &[]
        ));
        assert!(!domain_allowed(
            Some("news.openai.com"),
            &[],
            &[String::from("openai.com")]
        ));
    }

    #[test]
    fn duckduckgo_redirect_urls_are_decoded() {
        let decoded = decode_duckduckgo_result_url(
            "//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fdocs",
        )
        .expect("decode");
        assert_eq!(decoded, "https://example.com/docs");
    }

    fn compile_sample_search_html() -> String {
        r#"
        <html>
          <body>
            <div class="result">
              <h2 class="result__title">
                <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fplatform.openai.com%2Fdocs">OpenAI Docs</a>
              </h2>
              <div class="result__snippet">Official API documentation.</div>
            </div>
            <div class="result">
              <h2 class="result__title">
                <a class="result__a" href="https://example.com/post">Example Post</a>
              </h2>
              <div class="result__snippet">An example result.</div>
            </div>
          </body>
        </html>
        "#
        .to_string()
    }
}
