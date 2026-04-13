use crate::startup::AppLanguage;

const WELCOME_ART_LINES: &[&str] = &[
    "     *                                       █████▓▓░     ",
    "                                 *         ███▓░     ░░   ",
    "            ░░░░░░                        ███▓░           ",
    "    ░░░   ░░░░░░░░░░                      ███▓░           ",
    "   ░░░░░░░░░░░░░░░░░    *                ██▓░░      ▓   ",
    "                                             ░▓▓███▓▓░    ",
    " *                                 ░░░░                   ",
    "                                 ░░░░░░░░                 ",
    "                               ░░░░░░░░░░░░░░           ",
    "      █████████                                       * ",
    "      ██▄█████▄██                        *                ",
    "      █████████      *                                   ",
    "·······█ █   █ █······································",
];

pub(crate) fn welcome_v2_lines(language: AppLanguage) -> Vec<String> {
    let mut lines = vec![
        welcome_header(language),
        "······················································".to_string(),
        String::new(),
    ];
    lines.extend(WELCOME_ART_LINES.iter().map(|line| (*line).to_string()));
    lines
}

fn welcome_header(language: AppLanguage) -> String {
    match language {
        AppLanguage::English => format!("Welcome to hellox v{} ", env!("CARGO_PKG_VERSION")),
        AppLanguage::SimplifiedChinese => {
            format!("欢迎使用 hellox v{} ", env!("CARGO_PKG_VERSION"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::welcome_v2_lines;
    use crate::startup::AppLanguage;

    #[test]
    fn welcome_v2_lines_include_header_and_ascii_art() {
        let lines = welcome_v2_lines(AppLanguage::SimplifiedChinese);
        assert_eq!(
            lines.first().expect("header"),
            &format!("欢迎使用 hellox v{} ", env!("CARGO_PKG_VERSION"))
        );
        assert!(lines.iter().any(|line| line.contains("██▄█████▄██")));
    }
}
