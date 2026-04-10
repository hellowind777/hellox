use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;
use hellox_config::HelloxConfig;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplAction {
    Continue,
    Exit,
    Resume(String),
    Submit(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplExit {
    Exit,
    Resume(String),
}

#[derive(Debug, Clone)]
pub struct ReplMetadata {
    pub config: HelloxConfig,
    pub config_path: PathBuf,
    pub memory_root: PathBuf,
    pub plugins_root: PathBuf,
    pub sessions_root: PathBuf,
    pub shares_root: PathBuf,
}

#[async_trait]
pub trait ReplLoopDriver<Session> {
    fn banner_lines(&self, _session: &Session) -> Vec<String> {
        vec![
            String::from("hellox repl"),
            String::from("type `exit` or `/exit` to quit"),
        ]
    }

    fn prompt_label(&self) -> &str {
        "hellox> "
    }

    async fn handle_input(
        &self,
        input: &str,
        session: &mut Session,
        metadata: &ReplMetadata,
    ) -> Result<ReplAction>;

    async fn handle_submit(
        &self,
        prompt: String,
        session: &mut Session,
        metadata: &ReplMetadata,
    ) -> Result<()>;
}

pub async fn run_repl_loop<Session, Driver>(
    session: &mut Session,
    metadata: &ReplMetadata,
    driver: &Driver,
) -> Result<ReplExit>
where
    Driver: ReplLoopDriver<Session> + Send + Sync,
{
    for line in driver.banner_lines(session) {
        println!("{line}");
    }

    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        match init_rustyline(metadata) {
            Ok(editor) => return run_rustyline_loop(editor, session, metadata, driver).await,
            Err(_) => {
                // If rustyline can't initialize (e.g. locked/unsupported TTY), fall back to the
                // plain stdin loop instead of failing the whole repl.
            }
        }
    }

    loop {
        print!("{}", driver.prompt_label());
        io::stdout().flush()?;

        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        match driver.handle_input(trimmed, session, metadata).await? {
            ReplAction::Continue => continue,
            ReplAction::Exit => return Ok(ReplExit::Exit),
            ReplAction::Resume(session_id) => return Ok(ReplExit::Resume(session_id)),
            ReplAction::Submit(prompt) => {
                driver.handle_submit(prompt, session, metadata).await?;
            }
        }
    }
}

fn init_rustyline(metadata: &ReplMetadata) -> Result<DefaultEditor> {
    let mut editor = DefaultEditor::new()?;
    let history_path = repl_history_path(metadata);
    if let Some(parent) = history_path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }
    let _ = editor.load_history(&history_path);
    Ok(editor)
}

async fn run_rustyline_loop<Session, Driver>(
    mut editor: DefaultEditor,
    session: &mut Session,
    metadata: &ReplMetadata,
    driver: &Driver,
) -> Result<ReplExit>
where
    Driver: ReplLoopDriver<Session> + Send + Sync,
{
    let history_path = repl_history_path(metadata);

    let result = loop {
        match editor.readline(driver.prompt_label()) {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let _ = editor.add_history_entry(trimmed);
                match driver.handle_input(trimmed, session, metadata).await? {
                    ReplAction::Continue => continue,
                    ReplAction::Exit => break ReplExit::Exit,
                    ReplAction::Resume(session_id) => break ReplExit::Resume(session_id),
                    ReplAction::Submit(prompt) => {
                        driver.handle_submit(prompt, session, metadata).await?;
                    }
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => break ReplExit::Exit,
            Err(error) => return Err(error.into()),
        }
    };

    let _ = editor.save_history(&history_path);
    Ok(result)
}

fn repl_history_path(metadata: &ReplMetadata) -> PathBuf {
    let root = metadata
        .config_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    root.join("repl-history.txt")
}
