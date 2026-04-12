use std::borrow::Cow::{self, Owned};

use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::{Hint, Hinter};
use rustyline::validate::Validator;
use rustyline::{Context, Helper, Result};

const SLASH_OVERLAY_MAX_ITEMS: usize = 5;

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

    fn matching_completions<'a>(&'a self, fragment: &str) -> Vec<&'a ReplCompletion> {
        self.state
            .completions
            .iter()
            .filter(|candidate| candidate.value.starts_with(fragment))
            .collect()
    }

    fn slash_hint_display(&self, fragment: &str, matches: &[&ReplCompletion]) -> String {
        let inline = matches
            .first()
            .map(|candidate| candidate.value[fragment.len()..].to_string())
            .unwrap_or_default();
        let overlay = self.render_slash_overlay(matches);

        if overlay.is_empty() {
            inline
        } else {
            format!("{inline}{overlay}")
        }
    }

    fn render_slash_overlay(&self, matches: &[&ReplCompletion]) -> String {
        if matches.is_empty() {
            return String::new();
        }

        let mut lines = matches
            .iter()
            .take(SLASH_OVERLAY_MAX_ITEMS)
            .enumerate()
            .map(|(index, candidate)| self.render_slash_overlay_line(candidate, index == 0))
            .collect::<Vec<_>>();

        if matches.len() > SLASH_OVERLAY_MAX_ITEMS {
            lines.push("│   …".to_string());
        }

        format!("\n{}", lines.join("\n"))
    }

    fn render_slash_overlay_line(&self, candidate: &ReplCompletion, selected: bool) -> String {
        let marker = if selected { "›" } else { " " };
        match candidate.description.as_deref() {
            Some(description) => format!("│ {marker} {} — {description}", candidate.value),
            None => format!("│ {marker} {}", candidate.value),
        }
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
        let matches = self.matching_completions(fragment);
        let candidate = matches.first()?;
        let remainder = candidate.value[fragment.len()..].to_string();

        Some(ReplInlineHint {
            display: self.slash_hint_display(fragment, &matches),
            completion: if remainder.is_empty() {
                None
            } else {
                Some(remainder)
            },
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

        assert!(hint.display().starts_with("atus"));
        assert!(hint.display().contains("/status — show the active session"));
        assert_eq!(hint.completion(), Some("atus"));
    }

    #[test]
    fn slash_root_hint_renders_overlay_candidates() {
        let history = DefaultHistory::new();
        let context = Context::new(&history);

        let hint = helper().hint("/", 1, &context).expect("slash overlay hint");

        assert!(hint
            .display()
            .contains("\n│ › /help — show available commands"));
        assert!(hint
            .display()
            .contains("\n│   /status — show the active session"));
        assert_eq!(hint.completion(), Some("help"));
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
