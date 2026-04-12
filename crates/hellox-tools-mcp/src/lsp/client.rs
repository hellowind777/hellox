use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use anyhow::{anyhow, Context, Result};
use reqwest::Url;
use serde_json::{json, Value};

use crate::lsp::config::ResolvedLspServer;

pub(crate) trait LspClient {
    fn initialize(&mut self, root: &Path) -> Result<()>;
    fn did_open(&mut self, file_path: &Path, language_id: &str, text: &str) -> Result<()>;
    fn request(&mut self, method: &str, params: Value) -> Result<Value>;
}

pub(crate) struct ProcessLspClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl ProcessLspClient {
    pub(crate) fn spawn(server: &ResolvedLspServer) -> Result<Self> {
        let mut command = Command::new(&server.config.command);
        command
            .args(&server.config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command.current_dir(
            server
                .config
                .cwd
                .as_deref()
                .map(Path::new)
                .unwrap_or(&server.workspace_root),
        );
        command.envs(&server.config.env);

        let mut child = command
            .spawn()
            .with_context(|| format!("failed to launch LSP server `{}`", server.name))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("failed to capture LSP stdin for `{}`", server.name))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("failed to capture LSP stdout for `{}`", server.name))?;

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        })
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<()> {
        self.write_message(json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }))
    }

    fn write_message(&mut self, value: Value) -> Result<()> {
        let body = serde_json::to_vec(&value).context("failed to serialize LSP message")?;
        write!(self.stdin, "Content-Length: {}\r\n\r\n", body.len())
            .context("failed to write LSP header")?;
        self.stdin
            .write_all(&body)
            .context("failed to write LSP body")?;
        self.stdin.flush().context("failed to flush LSP stdin")
    }

    fn read_response(&mut self, expected_id: u64) -> Result<Value> {
        loop {
            let value = self.read_message()?;
            if value.get("id").and_then(Value::as_u64) == Some(expected_id) {
                if let Some(error) = value.get("error") {
                    return Err(anyhow!("LSP request failed: {error}"));
                }
                return Ok(value.get("result").cloned().unwrap_or(Value::Null));
            }
        }
    }

    fn read_message(&mut self) -> Result<Value> {
        let mut content_length = None;
        loop {
            let mut line = String::new();
            let bytes = self
                .stdout
                .read_line(&mut line)
                .context("failed to read LSP header line")?;
            if bytes == 0 {
                let mut stderr = String::new();
                if let Some(mut pipe) = self.child.stderr.take() {
                    let _ = pipe.read_to_string(&mut stderr);
                }
                return Err(anyhow!(
                    "LSP server closed stdout unexpectedly{}",
                    if stderr.trim().is_empty() {
                        String::new()
                    } else {
                        format!("; stderr: {}", stderr.trim())
                    }
                ));
            }
            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                break;
            }
            if let Some(value) = trimmed.strip_prefix("Content-Length:") {
                content_length = Some(
                    value
                        .trim()
                        .parse::<usize>()
                        .context("invalid LSP Content-Length header")?,
                );
            }
        }

        let content_length =
            content_length.ok_or_else(|| anyhow!("missing LSP Content-Length header"))?;
        let mut body = vec![0; content_length];
        self.stdout
            .read_exact(&mut body)
            .context("failed to read LSP message body")?;
        serde_json::from_slice(&body).context("failed to parse LSP JSON message")
    }
}

impl LspClient for ProcessLspClient {
    fn initialize(&mut self, root: &Path) -> Result<()> {
        let root_uri = Url::from_file_path(root)
            .map_err(|_| anyhow!("failed to build file URL for `{}`", root.display()))?;
        let _ = self.request(
            "initialize",
            json!({
                "processId": null,
                "rootUri": root_uri.as_str(),
                "capabilities": {}
            }),
        )?;
        self.notify("initialized", json!({}))
    }

    fn did_open(&mut self, file_path: &Path, language_id: &str, text: &str) -> Result<()> {
        let uri = Url::from_file_path(file_path)
            .map_err(|_| anyhow!("failed to build file URL for `{}`", file_path.display()))?;
        self.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri.as_str(),
                    "languageId": language_id,
                    "version": 1,
                    "text": text
                }
            }),
        )
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.write_message(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }))?;
        self.read_response(id)
    }
}

impl Drop for ProcessLspClient {
    fn drop(&mut self) {
        let _ = self.request("shutdown", json!(null));
        let _ = self.notify("exit", json!(null));
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
