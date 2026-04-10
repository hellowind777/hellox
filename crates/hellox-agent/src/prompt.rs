use std::path::Path;

use hellox_style::{compose_prompt_layers, NamedPrompt, PromptLayers};

pub type OutputStylePrompt = NamedPrompt;
pub type PersonaPrompt = NamedPrompt;
pub type PromptFragment = NamedPrompt;

pub fn build_default_system_prompt(
    cwd: &Path,
    shell_name: &str,
    output_style: Option<&OutputStylePrompt>,
    persona: Option<&PersonaPrompt>,
    prompt_fragments: &[PromptFragment],
) -> String {
    // Claude Code-style tool naming. The shell tool is platform-specific.
    let shell_tool = if cfg!(windows) { "PowerShell" } else { "Bash" };
    let prompt = format!(
        concat!(
            "You are hellox, a Rust-native terminal coding agent.\n\n",
            "# Environment\n",
            "- Working directory: {cwd}\n",
            "- Shell: {shell}\n\n",
            "# Mission\n",
            "- Complete the user's software-engineering task end-to-end.\n",
            "- Use tools when they materially improve correctness or speed.\n",
            "- Prefer reading the codebase before editing it.\n",
            "- Keep user-facing output concise and factual.\n\n",
            "# Tool use\n",
            "- Use Read and ListFiles before making assumptions.\n",
            "- Use Write and Edit for deterministic file edits.\n",
            "- Use {shell_tool} for commands, builds, tests, and repo inspection.\n",
            "- If a tool fails, explain the failure briefly and adapt.\n\n",
            "# Safety\n",
            "- Avoid destructive actions unless the user clearly requested them.\n",
            "- Do not fabricate tool results, file contents, commands, or URLs.\n",
            "- When you complete the task, give a short report with the key result."
        ),
        cwd = cwd.display(),
        shell = shell_name,
        shell_tool = shell_tool,
    );
    compose_prompt_layers(
        &prompt,
        &PromptLayers {
            output_style: output_style.cloned(),
            persona: persona.cloned(),
            fragments: prompt_fragments.to_vec(),
        },
    )
}
