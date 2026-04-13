use hellox_repl::ReplCompletion;

pub(super) fn needs_slash_normalization(raw_input: &str) -> bool {
    let trimmed = raw_input.trim();
    if !trimmed.starts_with('/') || trimmed.is_empty() {
        return false;
    }

    if trimmed.split_whitespace().count() != 1 {
        return false;
    }

    let trailing = raw_input
        .chars()
        .rev()
        .take_while(|character| character.is_whitespace())
        .collect::<Vec<_>>();

    trailing.len() >= 2 || trailing.contains(&'\t')
}

pub(super) fn normalize_repl_input(raw_input: &str, completions: &[ReplCompletion]) -> String {
    let trimmed = raw_input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if !needs_slash_normalization(raw_input) {
        return trimmed.to_string();
    }

    completions
        .iter()
        .find(|candidate| candidate.value.starts_with(trimmed) && candidate.value != trimmed)
        .map(|candidate| candidate.value.clone())
        .unwrap_or_else(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use hellox_repl::ReplCompletion;

    use super::{needs_slash_normalization, normalize_repl_input};

    fn completions() -> Vec<ReplCompletion> {
        vec![
            ReplCompletion::described("/help", "show commands"),
            ReplCompletion::described("/status", "show session"),
            ReplCompletion::described("/stats", "show statistics"),
        ]
    }

    #[test]
    fn trailing_tab_spaces_enable_first_match_fallback() {
        assert_eq!(
            normalize_repl_input("/st        ", &completions()),
            "/status"
        );
    }

    #[test]
    fn plain_fragment_stays_unexpanded_without_completion_trigger() {
        assert_eq!(normalize_repl_input("/st", &completions()), "/st");
    }

    #[test]
    fn subcommands_do_not_trigger_root_command_fallback() {
        assert_eq!(
            normalize_repl_input("/workflow run        ", &completions()),
            "/workflow run"
        );
    }

    #[test]
    fn completion_trigger_requires_tab_like_trailing_whitespace() {
        assert!(needs_slash_normalization("/st\t"));
        assert!(needs_slash_normalization("/st        "));
        assert!(!needs_slash_normalization("/st"));
        assert!(!needs_slash_normalization("/st "));
    }
}
