use std::path::Path;

use anyhow::Result;
use hellox_style::{
    discover_personas as discover_style_personas, format_definition_detail, format_definition_list,
    load_persona as load_style_persona, PromptAssetDefinition,
};

pub type PersonaDefinition = PromptAssetDefinition;

pub fn discover_personas(workspace_root: &Path) -> Result<Vec<PersonaDefinition>> {
    discover_style_personas(workspace_root)
}

pub fn load_persona(name: &str, workspace_root: &Path) -> Result<PersonaDefinition> {
    load_style_persona(name, workspace_root)
}

pub fn format_persona_list(
    personas: &[PersonaDefinition],
    default_name: Option<&str>,
    active_name: Option<&str>,
) -> String {
    format_definition_list(
        personas,
        &single_name(default_name),
        &single_name(active_name),
    )
}

pub fn format_persona_detail(
    persona: &PersonaDefinition,
    default_name: Option<&str>,
    active_name: Option<&str>,
) -> String {
    format_definition_detail(
        persona,
        &single_name(default_name),
        &single_name(active_name),
    )
}

fn single_name(value: Option<&str>) -> Vec<String> {
    value.map(ToString::to_string).into_iter().collect()
}
