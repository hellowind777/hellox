use anyhow::{anyhow, Result};
use hellox_config::{HelloxConfig, MarketplaceConfig};

pub fn build_marketplace(url: String, description: Option<String>) -> MarketplaceConfig {
    MarketplaceConfig {
        enabled: true,
        url,
        description: sanitize_optional(description),
    }
}

pub fn format_marketplace_list(config: &HelloxConfig) -> String {
    if config.plugins.marketplaces.is_empty() {
        return "No plugin marketplaces configured.".to_string();
    }

    let mut lines = vec!["marketplace\tenabled\turl\tdescription".to_string()];
    for (name, marketplace) in &config.plugins.marketplaces {
        lines.push(format!(
            "{}\t{}\t{}\t{}",
            name,
            marketplace.enabled,
            marketplace.url,
            marketplace.description.as_deref().unwrap_or("-")
        ));
    }
    lines.join("\n")
}

pub fn format_marketplace_detail(name: &str, marketplace: &MarketplaceConfig) -> String {
    let mut lines = vec![
        format!("name: {name}"),
        format!("enabled: {}", marketplace.enabled),
        format!("url: {}", marketplace.url),
    ];

    if let Some(description) = &marketplace.description {
        lines.push(format!("description: {description}"));
    }

    lines.join("\n")
}

pub fn add_marketplace(
    config: &mut HelloxConfig,
    marketplace_name: String,
    marketplace: MarketplaceConfig,
) -> Result<()> {
    if config.plugins.marketplaces.contains_key(&marketplace_name) {
        return Err(anyhow!(
            "Plugin marketplace `{marketplace_name}` already exists"
        ));
    }
    config
        .plugins
        .marketplaces
        .insert(marketplace_name, marketplace);
    Ok(())
}

pub fn get_marketplace<'a>(
    config: &'a HelloxConfig,
    marketplace_name: &str,
) -> Result<&'a MarketplaceConfig> {
    config
        .plugins
        .marketplaces
        .get(marketplace_name)
        .ok_or_else(|| anyhow!("Plugin marketplace `{marketplace_name}` was not found"))
}

pub fn set_marketplace_enabled(
    config: &mut HelloxConfig,
    marketplace_name: &str,
    enabled: bool,
) -> Result<()> {
    let marketplace = config
        .plugins
        .marketplaces
        .get_mut(marketplace_name)
        .ok_or_else(|| anyhow!("Plugin marketplace `{marketplace_name}` was not found"))?;
    marketplace.enabled = enabled;
    Ok(())
}

pub fn remove_marketplace(
    config: &mut HelloxConfig,
    marketplace_name: &str,
) -> Result<MarketplaceConfig> {
    config
        .plugins
        .marketplaces
        .remove(marketplace_name)
        .ok_or_else(|| anyhow!("Plugin marketplace `{marketplace_name}` was not found"))
}

fn sanitize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

#[cfg(test)]
mod tests {
    use hellox_config::HelloxConfig;

    use super::{
        add_marketplace, build_marketplace, format_marketplace_detail, format_marketplace_list,
        get_marketplace, remove_marketplace, set_marketplace_enabled,
    };

    #[test]
    fn add_list_toggle_and_remove_marketplaces() {
        let mut config = HelloxConfig::default();
        add_marketplace(
            &mut config,
            String::from("official"),
            build_marketplace(
                String::from("https://plugins.example.test/index.json"),
                Some(String::from("Official plugin feed")),
            ),
        )
        .expect("add marketplace");

        let rendered = format_marketplace_list(&config);
        assert!(rendered.contains("official"));

        set_marketplace_enabled(&mut config, "official", false).expect("disable");
        let detail = format_marketplace_detail(
            "official",
            get_marketplace(&config, "official").expect("marketplace"),
        );
        assert!(detail.contains("enabled: false"));

        let removed = remove_marketplace(&mut config, "official").expect("remove");
        assert_eq!(removed.url, "https://plugins.example.test/index.json");
    }
}
