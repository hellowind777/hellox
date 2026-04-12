use hellox_config::{default_config_path, load_or_default, HelloxConfig};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AppLanguage {
    #[default]
    English,
    SimplifiedChinese,
}

impl AppLanguage {
    pub fn locale_tag(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::SimplifiedChinese => "zh-CN",
        }
    }

    pub fn accepts_input(self, value: &str) -> bool {
        match self {
            Self::English => matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "y" | "yes"
            ),
            Self::SimplifiedChinese => matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "y" | "yes" | "是"
            ),
        }
    }

    pub fn rejects_input(self, value: &str) -> bool {
        match self {
            Self::English => matches!(value.trim().to_ascii_lowercase().as_str(), "2" | "n" | "no"),
            Self::SimplifiedChinese => matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "2" | "n" | "no" | "否"
            ),
        }
    }
}

pub fn resolve_app_language(config: &HelloxConfig) -> AppLanguage {
    config
        .ui
        .language
        .as_deref()
        .and_then(parse_language)
        .or_else(|| sys_locale::get_locale().as_deref().and_then(parse_language))
        .unwrap_or(AppLanguage::English)
}

pub fn resolve_default_app_language() -> AppLanguage {
    let config = load_or_default(Some(default_config_path())).unwrap_or_default();
    resolve_app_language(&config)
}

fn parse_language(raw: &str) -> Option<AppLanguage> {
    let value = raw.trim().to_ascii_lowercase();
    if value.starts_with("zh") {
        return Some(AppLanguage::SimplifiedChinese);
    }
    if value.starts_with("en") {
        return Some(AppLanguage::English);
    }
    None
}

#[cfg(test)]
mod tests {
    use hellox_config::HelloxConfig;

    use super::{resolve_app_language, AppLanguage};

    #[test]
    fn explicit_ui_language_overrides_detection() {
        let mut config = HelloxConfig::default();
        config.ui.language = Some("zh-CN".to_string());
        assert_eq!(
            resolve_app_language(&config),
            AppLanguage::SimplifiedChinese
        );
    }

    #[test]
    fn trust_prompt_inputs_support_numeric_and_text_values() {
        assert!(AppLanguage::English.accepts_input("1"));
        assert!(AppLanguage::English.accepts_input("yes"));
        assert!(AppLanguage::English.rejects_input("2"));
        assert!(AppLanguage::English.rejects_input("no"));
        assert!(AppLanguage::SimplifiedChinese.accepts_input("是"));
        assert!(AppLanguage::SimplifiedChinese.rejects_input("否"));
    }
}
