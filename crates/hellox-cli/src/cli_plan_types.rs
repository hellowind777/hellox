use clap::Subcommand;

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum PlanCommands {
    Show {
        session_id: String,
    },
    Panel {
        session_id: String,
    },
    Enter {
        session_id: String,
    },
    AddStep {
        session_id: String,
        #[arg(long = "step")]
        step: String,
        #[arg(long = "index")]
        index: Option<usize>,
    },
    UpdateStep {
        session_id: String,
        step_number: usize,
        #[arg(long = "step")]
        step: String,
    },
    RemoveStep {
        session_id: String,
        step_number: usize,
    },
    Allow {
        session_id: String,
        prompt: String,
    },
    Disallow {
        session_id: String,
        prompt: String,
    },
    #[command(alias = "accept")]
    Exit {
        session_id: String,
        #[arg(long = "step")]
        steps: Vec<String>,
        #[arg(long = "allow")]
        allowed_prompts: Vec<String>,
    },
    Clear {
        session_id: String,
    },
}
