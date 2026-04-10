use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use hellox_config::config_root;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDefinition {
    pub name: String,
    pub scope: String,
    pub path: PathBuf,
    pub description: Option<String>,
    pub when_to_use: Option<String>,
    pub allowed_tools: Vec<String>,
    pub hooks: Vec<String>,
}

pub fn discover_skills(cwd: &Path) -> Result<Vec<SkillDefinition>> {
    let mut skills = Vec::new();
    collect_skills(&config_root().join("skills"), "user", &mut skills)?;
    collect_skills(&cwd.join(".hellox").join("skills"), "project", &mut skills)?;
    skills.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.scope.cmp(&right.scope))
    });
    Ok(skills)
}

pub fn find_skill<'a>(skills: &'a [SkillDefinition], name: &str) -> Result<&'a SkillDefinition> {
    skills
        .iter()
        .find(|skill| skill.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| anyhow!("Skill `{name}` was not found"))
}

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

fn collect_skills(root: &Path, scope: &str, skills: &mut Vec<SkillDefinition>) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }

    for path in collect_markdown_files(root)? {
        skills.push(parse_skill(&path, scope)?);
    }

    Ok(())
}

fn parse_skill(path: &Path, scope: &str) -> Result<SkillDefinition> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read skill file {}", path.display()))?;
    let (frontmatter, body) = split_frontmatter(&raw);
    let name = frontmatter
        .get("name")
        .cloned()
        .or_else(|| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
        })
        .ok_or_else(|| anyhow!("skill file `{}` is missing a name", path.display()))?;

    Ok(SkillDefinition {
        name,
        scope: scope.to_string(),
        path: path.to_path_buf(),
        description: frontmatter
            .get("description")
            .cloned()
            .or_else(|| first_body_line(&body)),
        when_to_use: frontmatter.get("whenToUse").cloned(),
        allowed_tools: frontmatter_list(&frontmatter, "allowedTools"),
        hooks: frontmatter_list(&frontmatter, "hooks"),
    })
}

fn split_frontmatter(raw: &str) -> (std::collections::BTreeMap<String, String>, String) {
    let normalized = raw.replace("\r\n", "\n");
    if !normalized.starts_with("---\n") {
        return (std::collections::BTreeMap::new(), normalized);
    }

    if let Some(remainder) = normalized.strip_prefix("---\n") {
        if let Some((frontmatter, body)) = remainder.split_once("\n---\n") {
            return (parse_frontmatter(frontmatter), body.to_string());
        }
    }

    (std::collections::BTreeMap::new(), normalized)
}

fn parse_frontmatter(raw: &str) -> std::collections::BTreeMap<String, String> {
    let mut values = std::collections::BTreeMap::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            values.insert(
                key.trim().to_string(),
                value.trim().trim_matches('"').to_string(),
            );
        }
    }
    values
}

fn frontmatter_list(
    frontmatter: &std::collections::BTreeMap<String, String>,
    key: &str,
) -> Vec<String> {
    frontmatter
        .get(key)
        .map(|value| {
            value
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .map(|item| item.trim().trim_matches('"'))
                .filter(|item| !item.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn first_body_line(body: &str) -> Option<String> {
    body.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
}

fn collect_markdown_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root)
        .with_context(|| format!("failed to read skills root {}", root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_markdown_files(&path)?);
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        {
            files.push(path);
        }
    }
    Ok(files)
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
        let root = std::env::temp_dir().join(format!("hellox-skills-{suffix}"));
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
