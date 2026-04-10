use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{anyhow, Context, Result};
use serde_json::Value;

use super::{
    build_notification, build_request, initialize_params, process_incoming_message,
    TransportSession,
};

pub(super) struct StdioSession {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    stderr_buffer: Arc<Mutex<String>>,
    next_id: u64,
}

impl StdioSession {
    pub(super) fn spawn(
        server_name: &str,
        command: &str,
        args: &[String],
        env: &BTreeMap<String, String>,
        cwd: Option<&str>,
    ) -> Result<Self> {
        let mut child = Command::new(command);
        child
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(cwd) = cwd {
            child.current_dir(cwd);
        }
        child.envs(env);

        let mut child = child
            .spawn()
            .with_context(|| format!("Failed to spawn MCP stdio server `{server_name}`."))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("MCP stdio server `{server_name}` did not expose stdin."))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("MCP stdio server `{server_name}` did not expose stdout."))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("MCP stdio server `{server_name}` did not expose stderr."))?;

        let stderr_buffer = Arc::new(Mutex::new(String::new()));
        spawn_stderr_reader(stderr, Arc::clone(&stderr_buffer));

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            stderr_buffer,
            next_id: 1,
        })
    }

    fn send_message(&mut self, message: &Value) -> Result<()> {
        writeln!(self.stdin, "{}", serde_json::to_string(message)?)
            .context("Failed to write MCP stdio request.")?;
        self.stdin
            .flush()
            .context("Failed to flush MCP stdio request.")
    }

    fn read_response(&mut self, request_id: u64) -> Result<Value> {
        loop {
            let mut line = String::new();
            let read = self
                .stdout
                .read_line(&mut line)
                .context("Failed to read MCP stdio response.")?;
            if read == 0 {
                return Err(anyhow!(
                    "MCP stdio server exited before replying. stderr:\n{}",
                    self.stderr_text()
                ));
            }

            let payload = line.trim();
            if payload.is_empty() {
                continue;
            }

            if let Some(result) = process_incoming_message(payload, request_id, |message| {
                self.send_message(&message)
            })? {
                return Ok(result);
            }
        }
    }

    fn stderr_text(&self) -> String {
        self.stderr_buffer
            .lock()
            .map(|buffer| {
                if buffer.trim().is_empty() {
                    "(empty)".to_string()
                } else {
                    buffer.trim().to_string()
                }
            })
            .unwrap_or_else(|_| "(stderr unavailable)".to_string())
    }
}

impl TransportSession for StdioSession {
    fn request(&mut self, method: &str, params: Option<Value>) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.send_message(&build_request(id, method, params))?;
        self.read_response(id)
    }

    fn initialize(&mut self) -> Result<()> {
        self.request("initialize", Some(initialize_params()))?;
        self.send_message(&build_notification("notifications/initialized", None))
    }

    fn terminate(&mut self) -> Result<()> {
        let _ = self.child.kill();
        let _ = self.child.wait();
        Ok(())
    }
}

fn spawn_stderr_reader(stderr: ChildStderr, buffer: Arc<Mutex<String>>) {
    thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        loop {
            let mut line = String::new();
            let Ok(read) = reader.read_line(&mut line) else {
                break;
            };
            if read == 0 {
                break;
            }
            if let Ok(mut guard) = buffer.lock() {
                guard.push_str(line.trim_end());
                guard.push('\n');
            }
        }
    });
}
