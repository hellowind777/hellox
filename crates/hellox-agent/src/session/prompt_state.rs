use std::path::Path;

use anyhow::Result;
use hellox_style::{load_output_style, NamedPrompt};

use super::{AgentSession, OutputStylePrompt, PersonaPrompt, PromptFragment};
use crate::prompt::build_default_system_prompt;
use hellox_config::PermissionMode;

pub(super) fn restore_output_style(
    stored: Option<NamedPrompt>,
    legacy_name: Option<&str>,
    working_directory: &Path,
) -> Option<NamedPrompt> {
    stored.or_else(|| {
        legacy_name.and_then(|name| {
            load_output_style(name, working_directory)
                .ok()
                .map(|definition| NamedPrompt {
                    name: definition.name,
                    prompt: definition.prompt,
                })
        })
    })
}

impl AgentSession {
    pub fn persona_name(&self) -> Option<&str> {
        self.persona_name.as_deref()
    }

    pub fn prompt_fragment_names(&self) -> &[String] {
        &self.prompt_fragment_names
    }

    pub fn set_model(&mut self, model: impl Into<String>) -> Result<()> {
        let model = model.into();
        self.options.model = model.clone();
        if let Some(session_store) = &mut self.session_store {
            session_store.snapshot.model = model;
            session_store.save(&self.messages)?;
        }
        Ok(())
    }

    pub fn set_permission_mode(&mut self, mode: PermissionMode) -> Result<()> {
        self.context.permission_policy.set_mode(mode.clone());
        if let Some(session_store) = &mut self.session_store {
            session_store.snapshot.permission_mode = Some(mode);
            session_store.save(&self.messages)?;
        }
        Ok(())
    }

    pub fn set_output_style(&mut self, output_style: Option<OutputStylePrompt>) -> Result<()> {
        self.output_style_name = output_style.as_ref().map(|style| style.name.clone());
        self.options.output_style = output_style;
        self.refresh_system_prompt()
    }

    pub fn set_persona(&mut self, persona: Option<PersonaPrompt>) -> Result<()> {
        self.persona_name = persona.as_ref().map(|item| item.name.clone());
        self.options.persona = persona;
        self.refresh_system_prompt()
    }

    pub fn set_prompt_fragments(&mut self, prompt_fragments: Vec<PromptFragment>) -> Result<()> {
        self.prompt_fragment_names = prompt_fragments
            .iter()
            .map(|fragment| fragment.name.clone())
            .collect::<Vec<_>>();
        self.options.prompt_fragments = prompt_fragments;
        self.refresh_system_prompt()
    }

    fn refresh_system_prompt(&mut self) -> Result<()> {
        self.system_prompt = build_default_system_prompt(
            &self.context.working_directory,
            &self.shell_name,
            self.options.output_style.as_ref(),
            self.options.persona.as_ref(),
            &self.options.prompt_fragments,
        );

        if let Some(session_store) = &mut self.session_store {
            session_store.snapshot.output_style_name = self.output_style_name.clone();
            session_store.snapshot.output_style = self.options.output_style.clone();
            session_store.snapshot.persona = self.options.persona.clone();
            session_store.snapshot.prompt_fragments = self.options.prompt_fragments.clone();
            session_store.snapshot.system_prompt = self.system_prompt.clone();
            session_store.save(&self.messages)?;
        }

        Ok(())
    }
}
