use hellox_memory::MemoryScopeSelector;

pub use crate::mcp_command_types::McpCommand;
pub use crate::plugin_command_types::{MarketplaceCommand, PluginCommand};
pub use crate::workflow_command_types::WorkflowCommand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplCommand {
    Help,
    Status,
    Doctor,
    Usage,
    Stats,
    Cost,
    Brief(BriefCommand),
    Tools(ToolsCommand),
    Install(InstallCommand),
    Upgrade(UpgradeCommand),
    OutputStyle(OutputStyleCommand),
    Persona(PersonaCommand),
    PromptFragment(PromptFragmentCommand),
    Search { query: Option<String> },
    Skills { name: Option<String> },
    Hooks { name: Option<String> },
    RemoteEnv(RemoteEnvCommand),
    Teleport(TeleportCommand),
    Assistant(AssistantCommand),
    Bridge(BridgeCommand),
    Ide(IdeCommand),
    Mcp(McpCommand),
    Plugin(PluginCommand),
    Memory(MemoryCommand),
    Session(SessionCommand),
    Tasks(TaskCommand),
    Workflow(WorkflowCommand),
    Config(ConfigCommand),
    Plan(PlanCommand),
    Permissions { value: Option<String> },
    Resume { session_id: Option<String> },
    Share { path: Option<String> },
    Compact { instructions: Option<String> },
    Rewind,
    Clear,
    Exit,
    Model(ModelCommand),
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BriefCommand {
    Show,
    Set { message: Option<String> },
    Clear,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolsCommand {
    Search { query: Option<String>, limit: usize },
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigCommand {
    Show,
    Panel,
    Path,
    Keys,
    Set {
        key: Option<String>,
        value: Option<String>,
    },
    Clear {
        key: Option<String>,
    },
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanCommand {
    Show,
    Panel,
    Enter,
    Add {
        step: Option<String>,
        index: Option<usize>,
    },
    Update {
        step_number: Option<usize>,
        step: Option<String>,
    },
    Remove {
        step_number: Option<usize>,
    },
    Allow {
        prompt: Option<String>,
    },
    Disallow {
        prompt: Option<String>,
    },
    Exit {
        steps: Vec<String>,
        allowed_prompts: Vec<String>,
    },
    Clear,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelCommand {
    Current,
    Panel { profile_name: Option<String> },
    List,
    Show { profile_name: Option<String> },
    Use { value: Option<String> },
    Default { profile_name: Option<String> },
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallCommand {
    Status,
    Plan {
        source: Option<String>,
        target: Option<String>,
    },
    Apply {
        source: Option<String>,
        target: Option<String>,
        force: bool,
    },
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradeCommand {
    Status,
    Plan {
        source: Option<String>,
        target: Option<String>,
    },
    Apply {
        source: Option<String>,
        target: Option<String>,
        backup: bool,
        force: bool,
    },
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputStyleCommand {
    Current,
    Panel { style_name: Option<String> },
    List,
    Show { style_name: Option<String> },
    Use { style_name: Option<String> },
    Clear,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersonaCommand {
    Current,
    Panel { persona_name: Option<String> },
    List,
    Show { persona_name: Option<String> },
    Use { persona_name: Option<String> },
    Clear,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptFragmentCommand {
    Current,
    Panel { fragment_name: Option<String> },
    List,
    Show { fragment_name: Option<String> },
    Use { fragment_names: Vec<String> },
    Clear,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeCommand {
    Status,
    Sessions,
    Show { session_id: Option<String> },
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdeCommand {
    Status,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteEnvCommand {
    List,
    Show {
        environment_name: Option<String>,
    },
    Add {
        environment_name: Option<String>,
        url: Option<String>,
        token_env: Option<String>,
        account_id: Option<String>,
        device_id: Option<String>,
    },
    Enable {
        environment_name: Option<String>,
    },
    Disable {
        environment_name: Option<String>,
    },
    Remove {
        environment_name: Option<String>,
    },
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeleportCommand {
    Plan {
        environment_name: Option<String>,
        session_id: Option<String>,
    },
    Connect {
        environment_name: Option<String>,
        session_id: Option<String>,
    },
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssistantCommand {
    List {
        environment_name: Option<String>,
    },
    Show {
        session_id: Option<String>,
        environment_name: Option<String>,
    },
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionCommand {
    Current,
    Panel {
        session_id: Option<String>,
    },
    List,
    Show {
        session_id: Option<String>,
    },
    Share {
        session_id: Option<String>,
        path: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryCommand {
    Current,
    Panel {
        archived: bool,
        memory_id: Option<String>,
    },
    List {
        archived: bool,
    },
    Show {
        archived: bool,
        memory_id: Option<String>,
    },
    Search {
        archived: bool,
        query: Option<String>,
    },
    Clusters {
        archived: bool,
        limit: usize,
        semantic: bool,
    },
    Prune {
        scope: MemoryScopeSelector,
        older_than_days: u64,
        keep_latest: usize,
        apply: bool,
    },
    Archive {
        scope: MemoryScopeSelector,
        older_than_days: u64,
        keep_latest: usize,
        apply: bool,
    },
    Decay {
        scope: MemoryScopeSelector,
        older_than_days: u64,
        keep_latest: usize,
        max_summary_lines: usize,
        max_summary_chars: usize,
        apply: bool,
    },
    Save {
        instructions: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskCommand {
    List,
    Panel {
        task_id: Option<String>,
    },
    Add {
        content: Option<String>,
    },
    Show {
        task_id: Option<String>,
    },
    Update {
        task_id: Option<String>,
        content: Option<String>,
        priority: Option<String>,
        clear_priority: bool,
        description: Option<String>,
        clear_description: bool,
        status: Option<String>,
        output: Option<String>,
        clear_output: bool,
    },
    Output {
        task_id: Option<String>,
    },
    Stop {
        task_id: Option<String>,
        reason: Option<String>,
    },
    Start {
        task_id: Option<String>,
    },
    Done {
        task_id: Option<String>,
    },
    Cancel {
        task_id: Option<String>,
    },
    Remove {
        task_id: Option<String>,
    },
    Clear {
        target: Option<String>,
    },
}
