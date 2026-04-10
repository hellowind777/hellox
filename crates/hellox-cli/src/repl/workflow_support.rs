pub(super) fn workflow_help_text() -> String {
    [
        "Workflow commands:",
        "  /workflow                 List project workflow scripts",
        "  /workflow overview [name] Show a selector-style workflow overview",
        "  /workflow panel [name] [n] Show an authoring panel with copyable edit actions",
        "  /workflow runs [name]     List recorded workflow runs",
        "  /workflow validate [name] Validate project workflow scripts",
        "  /workflow show-run <id>   Show a recorded workflow run",
        "  /workflow last-run [name] Show the latest recorded workflow run",
        "  /workflow show <name>     Show a workflow script definition",
        "  /workflow init <name>     Create a starter workflow script",
        "  /workflow add-step <name> --prompt <text> Add a workflow step",
        "  /workflow update-step <name> <n> ... Edit a workflow step",
        "  /workflow duplicate-step <name> <n> [--to <m>] Duplicate a workflow step",
        "  /workflow move-step <name> <n> --to <m> Reorder a workflow step",
        "  /workflow remove-step <name> <n> Remove a workflow step",
        "  /workflow set-shared-context <name> <text> Set workflow shared context",
        "  /workflow clear-shared-context <name> Clear workflow shared context",
        "  /workflow enable-continue-on-error <name> Enable continue_on_error",
        "  /workflow disable-continue-on-error <name> Disable continue_on_error",
        "  /workflow run <name> [shared_context] Run a workflow script locally",
        "  /workflow <name> [shared_context] Shortcut for `/workflow run ...`",
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
