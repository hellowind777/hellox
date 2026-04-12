mod core_tables;
mod feature_tables;
mod help_tables;

use crate::startup::AppLanguage;

use core_tables::{
    AUTH_REPLACEMENTS, EMPTY_STATE_REPLACEMENTS, ERROR_REPLACEMENTS, HEADING_REPLACEMENTS,
    MEMORY_REPLACEMENTS, SEARCH_REPLACEMENTS, SESSION_REPLACEMENTS,
};
use feature_tables::{
    LABEL_REPLACEMENTS, MCP_REPLACEMENTS, PLUGIN_REMOTE_REPLACEMENTS, STYLE_REPLACEMENTS,
    TASK_REPLACEMENTS, WORKFLOW_REPLACEMENTS,
};
use help_tables::HELP_REPLACEMENTS;

type Replacement = (&'static str, &'static str);

pub(super) fn print_localized_repl_output(language: AppLanguage, text: String) {
    println!("{}", localize_user_visible_output(language, text));
}

pub(super) fn localized_invalid_selection_text(
    language: AppLanguage,
    upper_bound: usize,
    rerun: &str,
) -> String {
    match language {
        AppLanguage::English => {
            format!("Invalid selection. Choose 1..{upper_bound} or re-run `{rerun}`.")
        }
        AppLanguage::SimplifiedChinese => {
            format!("选择无效。请选择 1..{upper_bound}，或重新运行 `{rerun}`。")
        }
    }
}

pub(crate) fn localize_user_visible_output(language: AppLanguage, text: String) -> String {
    if matches!(language, AppLanguage::English) {
        return text;
    }

    normalize_localized_spacing(
        [
            HEADING_REPLACEMENTS,
            ERROR_REPLACEMENTS,
            EMPTY_STATE_REPLACEMENTS,
            AUTH_REPLACEMENTS,
            SESSION_REPLACEMENTS,
            MEMORY_REPLACEMENTS,
            SEARCH_REPLACEMENTS,
            WORKFLOW_REPLACEMENTS,
            MCP_REPLACEMENTS,
            STYLE_REPLACEMENTS,
            TASK_REPLACEMENTS,
            PLUGIN_REMOTE_REPLACEMENTS,
            LABEL_REPLACEMENTS,
            HELP_REPLACEMENTS,
        ]
        .into_iter()
        .fold(localize_search_miss_copy(text), apply_replacements),
    )
}

fn apply_replacements(text: String, replacements: &[Replacement]) -> String {
    replacements
        .iter()
        .fold(text, |acc, (from, to)| acc.replace(from, to))
}

fn normalize_localized_spacing(text: String) -> String {
    apply_replacements(text, &[("用法： ", "用法："), ("。.", "。")])
}

fn localize_search_miss_copy(text: String) -> String {
    let prefix = "No search hits for `";
    let suffix = "`.";
    let mut localized = String::new();
    let mut remaining = text.as_str();

    while let Some(start) = remaining.find(prefix) {
        let search_start = start + prefix.len();
        let after_prefix = &remaining[search_start..];
        let Some(query_end) = after_prefix.find(suffix) else {
            break;
        };

        localized.push_str(&remaining[..start]);
        let query = &after_prefix[..query_end];
        localized.push_str("未找到 `");
        localized.push_str(query);
        localized.push_str("` 的搜索结果。");
        remaining = &after_prefix[query_end + suffix.len()..];
    }

    localized.push_str(remaining);
    localized
}

#[cfg(test)]
mod tests {
    use crate::startup::AppLanguage;

    use super::{localize_user_visible_output, localized_invalid_selection_text};

    #[test]
    fn localizes_common_usage_and_error_copy() {
        let text = localize_user_visible_output(
            AppLanguage::SimplifiedChinese,
            "Usage: /workflow add-step <name> --prompt <text>\nUnable to render task panel: boom"
                .to_string(),
        );

        assert!(text.contains("用法：/workflow add-step"));
        assert!(text.contains("无法渲染任务面板"));
    }

    #[test]
    fn localizes_deep_command_status_copy() {
        let text = localize_user_visible_output(
            AppLanguage::SimplifiedChinese,
            "Added MCP server `docs` to `config.toml`.\nserver: docs\nresult:\n{}".to_string(),
        );

        assert!(text.contains("已添加 MCP 服务器 `docs`"));
        assert!(text.contains("服务器： docs"));
        assert!(text.contains("结果："));
    }

    #[test]
    fn localizes_selector_errors() {
        assert_eq!(
            localized_invalid_selection_text(AppLanguage::SimplifiedChinese, 3, "/workflow panel"),
            "选择无效。请选择 1..3，或重新运行 `/workflow panel`。"
        );
    }

    #[test]
    fn leaves_english_copy_unchanged() {
        let text = "Usage: /config set <key> <value>".to_string();
        assert_eq!(
            localize_user_visible_output(AppLanguage::English, text.clone()),
            text
        );
    }

    #[test]
    fn localizes_auth_tables_and_summaries() {
        let text = localize_user_visible_output(
            AppLanguage::SimplifiedChinese,
            "accounts: 1\nprovider_keys: 2\ntrusted_devices: 0\n\naccount_id\tprovider\tscopes\tupdated_at\texpires_at\taccess_token".to_string(),
        );

        assert!(text.contains("账号数： 1"));
        assert!(text.contains("Provider Key 数： 2"));
        assert!(text.contains("账号 ID\t供应商\t范围"));
    }

    #[test]
    fn localizes_auth_and_mcp_confirmation_copy() {
        let text = localize_user_visible_output(
            AppLanguage::SimplifiedChinese,
            "Stored provider key `openai`.\nStored MCP bearer token for `docs`.".to_string(),
        );

        assert!(text.contains("已保存 provider key `openai`"));
        assert!(text.contains("已保存 MCP bearer token"));
    }

    #[test]
    fn localizes_session_and_memory_tables() {
        let text = localize_user_visible_output(
            AppLanguage::SimplifiedChinese,
            "session_id\tmodel\tmessages\tupdated_at\tworking_directory\nabc\topus\t2\t123\tD:/repo\n\nmemory_id\tscope\tage\tupdated_at\tpath\nmem-1\tsession\tfresh <1m\t123\tD:/repo/.hellox/memory/sessions/mem-1.md".to_string(),
        );

        assert!(text.contains("会话 ID\t模型\t消息数\t更新时间\t工作目录"));
        assert!(text.contains("记忆 ID\t范围\t时效\t更新时间\t路径"));
        assert!(text.contains("刚刚 <1 分钟"));
    }

    #[test]
    fn localizes_session_detail_and_search_outputs() {
        let text = localize_user_visible_output(
            AppLanguage::SimplifiedChinese,
            "session_id: abc\npermission_mode: bypass_permissions\nplan_mode: false\nworking_directory: D:/repo\nmessages: 2\ninput_tokens: 10\n\nsource\tsource_id\tlocation\tpreview\nsession\tabc\tmessage 2\taccepted architecture".to_string(),
        );

        assert!(text.contains("会话 ID： abc"));
        assert!(text.contains("权限模式： 跳过权限"));
        assert!(text.contains("计划模式： 未启用"));
        assert!(text.contains("来源\t来源 ID\t位置\t预览"));
        assert!(text.contains("会话\tabc\t第 2 条消息"));
    }

    #[test]
    fn search_replacements_do_not_mutate_task_add_copy() {
        let text = localize_user_visible_output(
            AppLanguage::SimplifiedChinese,
            "Added task `task-1`.".to_string(),
        );

        assert_eq!(text, "已添加任务 `task-1`.");
    }
}
