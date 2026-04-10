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
