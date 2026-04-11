pub(super) fn workflow_help_text() -> String {
    [
        "Workflow commands:",
        "  /workflow                 List project workflow scripts",
        "  /workflow dashboard [name] Open the interactive workflow dashboard shell",
        "  /workflow dashboard --script-path <path> Open one explicit workflow dashboard shell",
        "  /workflow overview [name] Show a selector-style workflow overview",
        "  /workflow overview --script-path <path> Show one explicit workflow overview",
        "  /workflow panel [name] [n] Show an authoring panel with copyable edit actions",
        "  /workflow panel --script-path <path> [n] Open one explicit workflow script",
        "  /workflow runs [name]     List recorded workflow runs",
        "  /workflow runs --script-path <path> List recorded runs for one explicit script",
        "  /workflow validate [name] Validate project workflow scripts",
        "  /workflow validate --script-path <path> Validate one explicit workflow script",
        "  /workflow show-run <id> [n] Show a recorded workflow run",
        "  /workflow last-run [name] [n] Show the latest recorded workflow run",
        "  /workflow last-run --script-path <path> [n] Show the latest recorded run for one explicit script",
        "  /workflow show <name>     Show a workflow script definition",
        "  /workflow show --script-path <path> Show one explicit workflow script",
        "  /workflow init <name>     Create a starter workflow script",
        "  /workflow add-step <name> --prompt <text> Add a workflow step",
        "  /workflow add-step --script-path <path> --prompt <text> Add a step to an explicit script",
        "  /workflow update-step <name> <n> ... Edit a workflow step",
        "  /workflow update-step --script-path <path> <n> ... Edit an explicit workflow script",
        "  /workflow duplicate-step <name> <n> [--to <m>] Duplicate a workflow step",
        "  /workflow move-step <name> <n> --to <m> Reorder a workflow step",
        "  /workflow remove-step <name> <n> Remove a workflow step",
        "  /workflow set-shared-context <name> <text> Set workflow shared context",
        "  /workflow clear-shared-context <name> Clear workflow shared context",
        "  /workflow enable-continue-on-error <name> Enable continue_on_error",
        "  /workflow disable-continue-on-error <name> Disable continue_on_error",
        "  /workflow run <name> [shared_context] Run a workflow script locally",
        "  /workflow run --script-path <path> [shared_context] Run one explicit workflow script",
        "  /workflow <name> [shared_context] Shortcut for `/workflow run ...`",
        "  focused workflow panel/show-run/dashboard: `first` / `prev` / `next` / `last`",
    ]
    .join("\n")
}

pub(super) fn merge_optional_field(value: Option<String>, clear: bool) -> Option<Option<String>> {
    if clear {
        Some(None)
    } else {
        value.map(Some)
    }
}
