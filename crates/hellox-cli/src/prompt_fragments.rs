use std::path::Path;

use anyhow::Result;
use hellox_style::{
    discover_prompt_fragments as discover_style_prompt_fragments, format_definition_detail,
    format_definition_list, load_prompt_fragment as load_style_prompt_fragment,
    PromptAssetDefinition,
};

pub type PromptFragmentDefinition = PromptAssetDefinition;

pub fn discover_prompt_fragments(workspace_root: &Path) -> Result<Vec<PromptFragmentDefinition>> {
    discover_style_prompt_fragments(workspace_root)
}

pub fn load_prompt_fragment(name: &str, workspace_root: &Path) -> Result<PromptFragmentDefinition> {
    load_style_prompt_fragment(name, workspace_root)
}

pub fn format_prompt_fragment_list(
    fragments: &[PromptFragmentDefinition],
    default_names: &[String],
    active_names: &[String],
) -> String {
    format_definition_list(fragments, default_names, active_names)
}

pub fn format_prompt_fragment_detail(
    fragment: &PromptFragmentDefinition,
    default_names: &[String],
    active_names: &[String],
) -> String {
    format_definition_detail(fragment, default_names, active_names)
}
