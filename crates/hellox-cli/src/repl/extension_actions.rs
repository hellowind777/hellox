use anyhow::Result;
use hellox_agent::AgentSession;

use crate::hooks::{discover_hooks, find_hook, format_hook_detail, format_hook_list};
use crate::skills::{discover_skills, find_skill, format_skill_detail, format_skill_list};

pub(super) fn handle_skills_command(
    name: Option<String>,
    session: &AgentSession,
) -> Result<String> {
    let skills = discover_skills(session.working_directory())?;
    Ok(match name {
        Some(name) => format_skill_detail(find_skill(&skills, &name)?),
        None => format_skill_list(&skills),
    })
}

pub(super) fn handle_hooks_command(name: Option<String>, session: &AgentSession) -> Result<String> {
    let hooks = discover_hooks(session.working_directory())?;
    Ok(match name {
        Some(name) => format_hook_detail(find_hook(&hooks, &name)?),
        None => format_hook_list(&hooks),
    })
}
