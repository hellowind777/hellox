use std::io::{self, Write};

use anyhow::{Context, Result};
use crossterm::cursor::MoveUp;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType};
use crossterm::ExecutableCommand;

const ANSI_RESET: &str = "\x1b[0m";
const ANSI_BOLD: &str = "\x1b[1m";
const ANSI_DIM: &str = "\x1b[2m";
const ANSI_SUGGESTION: &str = "\x1b[36m";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct InteractiveOption<T> {
    pub(super) label: String,
    pub(super) value: T,
}

pub(super) fn select_interactive<T: Copy>(
    options: &[InteractiveOption<T>],
    default_index: usize,
    footer_text: &str,
    exit_pending_text: &str,
) -> Result<Option<T>> {
    if options.is_empty() {
        return Ok(None);
    }

    let _raw_mode = RawModeGuard::activate()?;
    let mut stdout = io::stdout();
    let mut rendered_line_count = 0usize;
    let mut selection = default_index.min(options.len() - 1);
    let mut exit_pending = false;

    loop {
        let footer = if exit_pending {
            exit_pending_text
        } else {
            footer_text
        };
        rendered_line_count = redraw_lines(
            &mut stdout,
            rendered_line_count,
            &render_lines(options, selection, footer),
        )?;

        match event::read().context("failed to read interactive selection input")? {
            Event::Key(key) if is_key_press(key) => {
                match resolve_key_action(key, options.len(), exit_pending) {
                    SelectionKeyAction::MovePrevious => {
                        selection = if selection == 0 {
                            options.len() - 1
                        } else {
                            selection - 1
                        };
                        exit_pending = false;
                    }
                    SelectionKeyAction::MoveNext => {
                        selection = (selection + 1) % options.len();
                        exit_pending = false;
                    }
                    SelectionKeyAction::Confirm => {
                        clear_rendered_lines(&mut stdout, rendered_line_count)?;
                        return Ok(Some(options[selection].value));
                    }
                    SelectionKeyAction::Choose(index) => {
                        clear_rendered_lines(&mut stdout, rendered_line_count)?;
                        return Ok(Some(options[index].value));
                    }
                    SelectionKeyAction::Cancel => {
                        clear_rendered_lines(&mut stdout, rendered_line_count)?;
                        return Ok(None);
                    }
                    SelectionKeyAction::ArmExit => {
                        exit_pending = true;
                    }
                    SelectionKeyAction::ExitImmediately => {
                        clear_rendered_lines(&mut stdout, rendered_line_count)?;
                        return Ok(None);
                    }
                    SelectionKeyAction::Ignore => {}
                }
            }
            _ => {}
        }
    }
}

fn render_lines<T>(
    options: &[InteractiveOption<T>],
    selected_index: usize,
    footer: &str,
) -> Vec<String> {
    let mut lines = options
        .iter()
        .enumerate()
        .map(|(index, option)| {
            let marker = if index == selected_index { "❯" } else { " " };
            format!("{marker} {}. {}", index + 1, option.label)
        })
        .collect::<Vec<_>>();
    lines.push(String::new());
    lines.push(footer.to_string());
    lines
}

fn redraw_lines(
    stdout: &mut io::Stdout,
    previous_line_count: usize,
    lines: &[String],
) -> Result<usize> {
    if previous_line_count > 0 {
        stdout
            .execute(MoveUp(previous_line_count as u16))
            .context("failed to reposition selection cursor")?;
        stdout
            .execute(Clear(ClearType::FromCursorDown))
            .context("failed to clear interactive selection")?;
    }

    for line in lines {
        writeln!(stdout, "{}", style_line(line))?;
    }
    stdout.flush()?;
    Ok(lines.len())
}

fn clear_rendered_lines(stdout: &mut io::Stdout, line_count: usize) -> Result<()> {
    if line_count == 0 {
        return Ok(());
    }

    stdout
        .execute(MoveUp(line_count as u16))
        .context("failed to reposition selection cursor for clear")?;
    stdout
        .execute(Clear(ClearType::FromCursorDown))
        .context("failed to clear selection lines")?;
    stdout.flush()?;
    Ok(())
}

fn style_line(line: &str) -> String {
    let trimmed = line.trim_start();
    if trimmed.starts_with('❯') {
        return format!("{ANSI_BOLD}{ANSI_SUGGESTION}{line}{ANSI_RESET}");
    }
    if trimmed.starts_with("Enter ")
        || trimmed.starts_with("Press Ctrl+C")
        || trimmed.starts_with("Enter 确认")
        || trimmed.starts_with("再按一次 Ctrl+C")
    {
        return format!("{ANSI_DIM}{line}{ANSI_RESET}");
    }
    line.to_string()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectionKeyAction {
    MovePrevious,
    MoveNext,
    Confirm,
    Choose(usize),
    Cancel,
    ArmExit,
    ExitImmediately,
    Ignore,
}

fn resolve_key_action(
    key: KeyEvent,
    option_count: usize,
    exit_pending: bool,
) -> SelectionKeyAction {
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
    {
        return if exit_pending {
            SelectionKeyAction::ExitImmediately
        } else {
            SelectionKeyAction::ArmExit
        };
    }

    match key.code {
        KeyCode::Up | KeyCode::Left => SelectionKeyAction::MovePrevious,
        KeyCode::Down | KeyCode::Right => SelectionKeyAction::MoveNext,
        KeyCode::Enter => SelectionKeyAction::Confirm,
        KeyCode::Esc => SelectionKeyAction::Cancel,
        KeyCode::Char(ch) => number_key_to_index(ch, option_count)
            .map(SelectionKeyAction::Choose)
            .unwrap_or(SelectionKeyAction::Ignore),
        _ => SelectionKeyAction::Ignore,
    }
}

fn number_key_to_index(ch: char, option_count: usize) -> Option<usize> {
    ch.to_digit(10)
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value >= 1 && *value <= option_count)
        .map(|value| value - 1)
}

fn is_key_press(key: KeyEvent) -> bool {
    matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

struct RawModeGuard;

impl RawModeGuard {
    fn activate() -> Result<Self> {
        enable_raw_mode().context("failed to enable raw mode for interactive selection")?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

#[cfg(test)]
mod tests {
    use super::{number_key_to_index, render_lines, InteractiveOption};

    #[test]
    fn number_keys_map_to_visible_options() {
        assert_eq!(number_key_to_index('1', 3), Some(0));
        assert_eq!(number_key_to_index('3', 3), Some(2));
        assert_eq!(number_key_to_index('4', 3), None);
        assert_eq!(number_key_to_index('x', 3), None);
    }

    #[test]
    fn render_lines_marks_selected_option_and_footer() {
        let lines = render_lines(
            &[
                InteractiveOption {
                    label: "OpenAI Compatible 接口".to_string(),
                    value: 1_u8,
                },
                InteractiveOption {
                    label: "Anthropic 兼容接口".to_string(),
                    value: 2_u8,
                },
            ],
            0,
            "Enter 确认 · Esc 退出引导",
        );

        assert_eq!(lines[0], "❯ 1. OpenAI Compatible 接口");
        assert_eq!(lines[1], "  2. Anthropic 兼容接口");
        assert_eq!(lines.last().expect("footer"), "Enter 确认 · Esc 退出引导");
    }
}
