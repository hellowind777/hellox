use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use async_trait::async_trait;
use hellox_config::HelloxConfig;
use rustyline::error::ReadlineError;
use rustyline::history::DefaultHistory;
use rustyline::{
    Cmd, CompletionType, ConditionalEventHandler, Config, Editor, Event, EventContext,
    EventHandler, KeyCode, KeyEvent, Modifiers, RepeatCount,
};

use crate::input_helper::{ReplInputHelper, ReplPromptState};

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

    fn prompt_label(&self, _session: &Session, _metadata: &ReplMetadata) -> String {
        String::from("❯ ")
    }

    fn prompt_state(&self, _session: &Session, _metadata: &ReplMetadata) -> ReplPromptState {
        ReplPromptState::default()
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
        let prompt_state = driver.prompt_state(session, metadata);
        let prompt = compose_prompt_text(&driver.prompt_label(session, metadata), &prompt_state);
        if prompt_state.shell_lines.is_empty() {
            if let Some(placeholder) = prompt_state.placeholder.as_deref() {
                println!("{placeholder}");
            }
        }
        print!("{prompt}");
        io::stdout().flush()?;

        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        let raw_line = strip_line_endings(&line);
        let trimmed = raw_line.trim();

        if trimmed.is_empty() {
            continue;
        }

        match driver.handle_input(raw_line, session, metadata).await? {
            ReplAction::Continue => continue,
            ReplAction::Exit => return Ok(ReplExit::Exit),
            ReplAction::Resume(session_id) => return Ok(ReplExit::Resume(session_id)),
            ReplAction::Submit(prompt) => {
                driver.handle_submit(prompt, session, metadata).await?;
            }
        }
    }
}

fn init_rustyline(metadata: &ReplMetadata) -> Result<Editor<ReplInputHelper, DefaultHistory>> {
    let config = Config::builder()
        .history_ignore_space(true)
        .completion_type(CompletionType::List)
        .completion_show_all_if_ambiguous(true)
        .completion_prompt_limit(40)
        .build();
    let mut editor = Editor::with_config(config)?;
    let prompt_state = Arc::new(Mutex::new(ReplPromptState::default()));
    editor.set_helper(Some(ReplInputHelper::with_state_handle(
        prompt_state.clone(),
    )));
    editor.bind_sequence(
        KeyEvent(KeyCode::Tab, Modifiers::NONE),
        EventHandler::Conditional(Box::new(SlashTabEventHandler {
            prompt_state: prompt_state.clone(),
        })),
    );
    editor.bind_sequence(
        KeyEvent::ctrl('I'),
        EventHandler::Conditional(Box::new(SlashTabEventHandler { prompt_state })),
    );
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
    mut editor: Editor<ReplInputHelper, DefaultHistory>,
    session: &mut Session,
    metadata: &ReplMetadata,
    driver: &Driver,
) -> Result<ReplExit>
where
    Driver: ReplLoopDriver<Session> + Send + Sync,
{
    let history_path = repl_history_path(metadata);

    let result = loop {
        let prompt_state = driver.prompt_state(session, metadata);
        let prompt = compose_prompt_text(&driver.prompt_label(session, metadata), &prompt_state);
        if let Some(helper) = editor.helper_mut() {
            helper.set_state(prompt_state);
        }
        match editor.readline(&prompt) {
            Ok(line) => {
                let raw_line = strip_line_endings(&line);
                let trimmed = raw_line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let _ = editor.add_history_entry(trimmed);
                match driver.handle_input(raw_line, session, metadata).await? {
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

fn strip_line_endings(line: &str) -> &str {
    line.trim_end_matches(['\r', '\n'])
}

struct SlashTabEventHandler {
    prompt_state: Arc<Mutex<ReplPromptState>>,
}

impl ConditionalEventHandler for SlashTabEventHandler {
    fn handle(&self, evt: &Event, _: RepeatCount, _: bool, ctx: &EventContext) -> Option<Cmd> {
        let _ = evt;
        let state = self
            .prompt_state
            .lock()
            .expect("repl prompt state poisoned");
        ReplInputHelper::best_completion_remainder(&state, ctx.line(), ctx.pos())
            .map(|remainder| Cmd::Insert(1, remainder))
    }
}

fn compose_prompt_text(label: &str, state: &ReplPromptState) -> String {
    if state.shell_lines.is_empty() {
        return label.to_string();
    }

    let mut lines = state.shell_lines.clone();
    lines.push(label.to_string());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use crate::input_helper::{ReplInputHelper, ReplPromptState};

    use super::{compose_prompt_text, strip_line_endings};

    #[test]
    fn compose_prompt_text_appends_label_after_shell_lines() {
        let state = ReplPromptState::with_shell(
            Some("Explain this repository".to_string()),
            vec![
                "╭─ local chat · model opus · trusted workspace".to_string(),
                "│ /help commands".to_string(),
            ],
            Vec::new(),
        );

        assert_eq!(
            compose_prompt_text("╰─ ❯ ", &state),
            "╭─ local chat · model opus · trusted workspace\n│ /help commands\n╰─ ❯ "
        );
    }

    #[test]
    fn slash_tab_prefers_hint_completion_for_command_fragment() {
        let state = ReplPromptState::with_placeholder_and_completions(
            None,
            vec![
                crate::input_helper::ReplCompletion::described("/help", "show commands"),
                crate::input_helper::ReplCompletion::described("/status", "show session"),
            ],
        );

        assert!(ReplInputHelper::best_completion_remainder(&state, "/", 1).is_some());
        assert!(ReplInputHelper::best_completion_remainder(&state, "/sta", 4).is_some());
        assert!(ReplInputHelper::best_completion_remainder(&state, "plain", 5).is_none());
        assert!(ReplInputHelper::best_completion_remainder(&state, "/workflow run", 10).is_none());
        assert!(ReplInputHelper::best_completion_remainder(&state, "/status", 7).is_none());
    }

    #[test]
    fn strip_line_endings_preserves_trailing_spaces() {
        assert_eq!(strip_line_endings("/st    \r\n"), "/st    ");
        assert_eq!(strip_line_endings("/status"), "/status");
    }
}
