use std::borrow::Cow::{self, Owned};

use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::{Hint, Hinter};
use rustyline::validate::Validator;
use rustyline::{Context, Helper, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplCompletion {
    pub value: String,
    pub description: Option<String>,
}

impl ReplCompletion {
    pub fn described(value: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            description: Some(description.into()),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplPromptState {
    pub placeholder: Option<String>,
    pub completions: Vec<ReplCompletion>,
    pub shell_lines: Vec<String>,
}

impl ReplPromptState {
    pub fn with_placeholder_and_completions(
        placeholder: Option<String>,
        completions: Vec<ReplCompletion>,
    ) -> Self {
        Self::with_shell(placeholder, Vec::new(), completions)
    }

    pub fn with_shell(
        placeholder: Option<String>,
        shell_lines: Vec<String>,
        completions: Vec<ReplCompletion>,
    ) -> Self {
        Self {
            placeholder,
            completions,
            shell_lines,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReplInlineHint {
    display: String,
    completion: Option<String>,
}

impl Hint for ReplInlineHint {
    fn display(&self) -> &str {
        &self.display
    }

    fn completion(&self) -> Option<&str> {
        self.completion.as_deref()
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ReplInputHelper {
    state: ReplPromptState,
}

impl ReplInputHelper {
    pub(crate) fn set_state(&mut self, state: ReplPromptState) {
        self.state = state;
    }

    fn slash_fragment<'a>(line: &'a str, pos: usize) -> Option<&'a str> {
        if pos > line.len() {
            return None;
        }
        let prefix = &line[..pos];
        if !prefix.starts_with('/') {
            return None;
        }
        let command_len = prefix.find(char::is_whitespace).unwrap_or(prefix.len());
        if pos > command_len {
            return None;
        }
        Some(&prefix[..pos])
    }
}

impl Completer for ReplInputHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Self::Candidate>)> {
        let Some(fragment) = Self::slash_fragment(line, pos) else {
            return Ok((0, Vec::new()));
        };

        let matches = self
            .state
            .completions
            .iter()
            .filter(|candidate| candidate.value.starts_with(fragment))
            .map(|candidate| Pair {
                display: match &candidate.description {
                    Some(description) => format!("{} — {}", candidate.value, description),
                    None => candidate.value.clone(),
                },
                replacement: candidate.value.clone(),
            })
            .collect::<Vec<_>>();

        Ok((0, matches))
    }
}

impl Hinter for ReplInputHelper {
    type Hint = ReplInlineHint;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<Self::Hint> {
        if line.is_empty() && pos == 0 {
            return self
                .state
                .placeholder
                .as_ref()
                .map(|placeholder| ReplInlineHint {
                    display: placeholder.clone(),
                    completion: None,
                });
        }

        let fragment = Self::slash_fragment(line, pos)?;
        let candidate = self.state.completions.iter().find(|candidate| {
            candidate.value.starts_with(fragment) && candidate.value.len() > fragment.len()
        })?;
        let remainder = candidate.value[fragment.len()..].to_string();

        Some(ReplInlineHint {
            display: remainder.clone(),
            completion: Some(remainder),
        })
    }
}

impl Highlighter for ReplInputHelper {
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Owned(format!("\x1b[90m{hint}\x1b[0m"))
    }
}

impl Validator for ReplInputHelper {}

impl Helper for ReplInputHelper {}

#[cfg(test)]
mod tests {
    use rustyline::completion::Completer;
    use rustyline::hint::{Hint, Hinter};
    use rustyline::history::DefaultHistory;
    use rustyline::Context;

    use super::{ReplCompletion, ReplInputHelper, ReplPromptState};

    fn helper() -> ReplInputHelper {
        let mut helper = ReplInputHelper::default();
        helper.set_state(ReplPromptState::with_placeholder_and_completions(
            Some("Explain this Rust workspace".to_string()),
            vec![
                ReplCompletion::described("/help", "show available commands"),
                ReplCompletion::described("/status", "show the active session"),
                ReplCompletion::described("/workflow", "list workflow commands"),
            ],
        ));
        helper
    }

    #[test]
    fn empty_line_uses_placeholder_hint() {
        let history = DefaultHistory::new();
        let context = Context::new(&history);

        let hint = helper().hint("", 0, &context).expect("placeholder hint");

        assert_eq!(hint.display(), "Explain this Rust workspace");
        assert_eq!(hint.completion(), None);
    }

    #[test]
    fn slash_prefix_uses_command_suffix_hint() {
        let history = DefaultHistory::new();
        let context = Context::new(&history);

        let hint = helper().hint("/st", 3, &context).expect("slash hint");

        assert_eq!(hint.display(), "atus");
        assert_eq!(hint.completion(), Some("atus"));
    }

    #[test]
    fn slash_completion_lists_matching_commands() {
        let history = DefaultHistory::new();
        let context = Context::new(&history);

        let (start, matches) = helper()
            .complete("/w", 2, &context)
            .expect("complete slash command");

        assert_eq!(start, 0);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].replacement, "/workflow");
        assert!(matches[0].display.contains("workflow commands"));
    }

    #[test]
    fn command_completion_stops_after_first_token() {
        let history = DefaultHistory::new();
        let context = Context::new(&history);

        let (_, matches) = helper()
            .complete("/workflow ru", "/workflow ru".len(), &context)
            .expect("ignore subcommand completion");

        assert!(matches.is_empty());
    }
}
