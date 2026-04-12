use std::path::Path;

pub use hellox_config::{discover_skills, find_skill, SkillDefinition};

pub fn format_skill_list(skills: &[SkillDefinition]) -> String {
    if skills.is_empty() {
        return "No skills discovered.".to_string();
    }

    let mut lines = vec!["name\tscope\tpath\tdescription".to_string()];
    for skill in skills {
        lines.push(format!(
            "{}\t{}\t{}\t{}",
            skill.name,
            skill.scope,
            normalize_path(&skill.path),
            skill.description.as_deref().unwrap_or("-")
        ));
    }
    lines.join("\n")
}

pub fn format_skill_detail(skill: &SkillDefinition) -> String {
    let mut lines = vec![
        format!("name: {}", skill.name),
        format!("scope: {}", skill.scope),
        format!("path: {}", normalize_path(&skill.path)),
    ];

    if let Some(description) = &skill.description {
        lines.push(format!("description: {description}"));
    }
    if let Some(when_to_use) = &skill.when_to_use {
        lines.push(format!("when_to_use: {when_to_use}"));
    }
    lines.push(format!(
        "allowed_tools: {}",
        if skill.allowed_tools.is_empty() {
            "(none)".to_string()
        } else {
            skill.allowed_tools.join(", ")
        }
    ));
    lines.push(format!(
        "hooks: {}",
        if skill.hooks.is_empty() {
            "(none)".to_string()
        } else {
            skill.hooks.join(", ")
        }
    ));

    lines.join("\n")
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{discover_skills, find_skill, format_skill_detail, format_skill_list};

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("hellox-cli-skills-{suffix}"));
        fs::create_dir_all(&root).expect("create temp dir");
        root
    }

    #[test]
    fn discovers_and_formats_project_skills() {
        let root = temp_dir();
        let skills_root = root.join(".hellox").join("skills");
        fs::create_dir_all(&skills_root).expect("create skills root");
        fs::write(
            skills_root.join("review.md"),
            r#"---
name: review
description: Review the current change set.
whenToUse: Use when validating a patch.
allowedTools: [Read, Grep]
hooks: [pre_tool]
---
Review skill body."#,
        )
        .expect("write skill");

        let skills = discover_skills(&root).expect("discover skills");
        assert_eq!(skills.len(), 1);

        let rendered = format_skill_list(&skills);
        assert!(rendered.contains("review"));
        let detail = format_skill_detail(find_skill(&skills, "review").expect("find skill"));
        assert!(detail.contains("allowed_tools: Read, Grep"));
        assert!(detail.contains("hooks: pre_tool"));
    }
}
