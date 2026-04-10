use hellox_memory::MemoryScopeSelector;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowCommand {
    List,
    Overview {
        workflow_name: Option<String>,
    },
    Panel {
        workflow_name: Option<String>,
        step_number: Option<usize>,
    },
    Runs {
        workflow_name: Option<String>,
    },
    Validate {
        workflow_name: Option<String>,
    },
    ShowRun {
        run_id: Option<String>,
    },
    LastRun {
        workflow_name: Option<String>,
    },
    Show {
        workflow_name: Option<String>,
    },
    Init {
        workflow_name: Option<String>,
    },
    AddStep {
        workflow_name: Option<String>,
        name: Option<String>,
        prompt: Option<String>,
        index: Option<usize>,
        when: Option<String>,
        model: Option<String>,
        backend: Option<String>,
        step_cwd: Option<String>,
        run_in_background: bool,
    },
    UpdateStep {
        workflow_name: Option<String>,
        step_number: Option<usize>,
        name: Option<String>,
        clear_name: bool,
        prompt: Option<String>,
        when: Option<String>,
        clear_when: bool,
        model: Option<String>,
        clear_model: bool,
        backend: Option<String>,
        clear_backend: bool,
        step_cwd: Option<String>,
        clear_step_cwd: bool,
        run_in_background: Option<bool>,
    },
    RemoveStep {
        workflow_name: Option<String>,
        step_number: Option<usize>,
    },
    SetSharedContext {
        workflow_name: Option<String>,
        value: Option<String>,
    },
    ClearSharedContext {
        workflow_name: Option<String>,
    },
    EnableContinueOnError {
        workflow_name: Option<String>,
    },
    DisableContinueOnError {
        workflow_name: Option<String>,
    },
    Run {
        workflow_name: Option<String>,
        shared_context: Option<String>,
    },
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpCommand {
    List,
    Panel {
        server_name: Option<String>,
    },
    Show {
        server_name: Option<String>,
    },
    Tools {
        server_name: Option<String>,
    },
    Call {
        server_name: Option<String>,
        tool_name: Option<String>,
        input: Option<String>,
    },
    Resources {
        server_name: Option<String>,
    },
    Prompts {
        server_name: Option<String>,
    },
    ReadResource {
        server_name: Option<String>,
        uri: Option<String>,
    },
    GetPrompt {
        server_name: Option<String>,
        prompt_name: Option<String>,
        input: Option<String>,
    },
    AuthShow {
        server_name: Option<String>,
    },
    AuthSetToken {
        server_name: Option<String>,
        bearer_token: Option<String>,
    },
    AuthClear {
        server_name: Option<String>,
    },
    AuthOauthSet {
        server_name: Option<String>,
        client_id: Option<String>,
        authorize_url: Option<String>,
        token_url: Option<String>,
        redirect_url: Option<String>,
        scopes: Vec<String>,
    },
    AuthOauthStart {
        server_name: Option<String>,
    },
    AuthOauthExchange {
        server_name: Option<String>,
        code: Option<String>,
        code_verifier: Option<String>,
    },
    AuthOauthRefresh {
        server_name: Option<String>,
    },
    AuthOauthClear {
        server_name: Option<String>,
    },
    RegistryList {
        cursor: Option<String>,
        limit: Option<usize>,
    },
    RegistryShow {
        name: Option<String>,
    },
    RegistryInstall {
        name: Option<String>,
        server_name: Option<String>,
        scope: Option<String>,
    },
    AddStdio {
        server_name: Option<String>,
        command: Option<String>,
        args: Vec<String>,
    },
    AddSse {
        server_name: Option<String>,
        url: Option<String>,
    },
    AddWs {
        server_name: Option<String>,
        url: Option<String>,
    },
    Enable {
        server_name: Option<String>,
    },
    Disable {
        server_name: Option<String>,
    },
    Remove {
        server_name: Option<String>,
    },
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginCommand {
    List,
    Panel {
        plugin_id: Option<String>,
    },
    Show {
        plugin_id: Option<String>,
    },
    Install {
        source: Option<String>,
        disabled: bool,
    },
    Enable {
        plugin_id: Option<String>,
    },
    Disable {
        plugin_id: Option<String>,
    },
    Remove {
        plugin_id: Option<String>,
    },
    Marketplace(MarketplaceCommand),
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketplaceCommand {
    List,
    Show {
        marketplace_name: Option<String>,
    },
    Add {
        marketplace_name: Option<String>,
        url: Option<String>,
    },
    Enable {
        marketplace_name: Option<String>,
    },
    Disable {
        marketplace_name: Option<String>,
    },
    Remove {
        marketplace_name: Option<String>,
    },
    Help,
}
