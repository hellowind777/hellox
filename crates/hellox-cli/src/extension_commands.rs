use anyhow::Result;

use crate::hooks::{discover_hooks, find_hook, format_hook_detail, format_hook_list};
use crate::skills::{discover_skills, find_skill, format_skill_detail, format_skill_list};

pub fn handle_skills_command(name: Option<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let skills = discover_skills(&cwd)?;
    match name {
        Some(name) => println!("{}", format_skill_detail(find_skill(&skills, &name)?)),
        None => println!("{}", format_skill_list(&skills)),
    }
    Ok(())
}

pub fn handle_hooks_command(name: Option<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let hooks = discover_hooks(&cwd)?;
    match name {
        Some(name) => println!("{}", format_hook_detail(find_hook(&hooks, &name)?)),
        None => println!("{}", format_hook_list(&hooks)),
    }
    Ok(())
}
