use std::env;
use std::process::{Command, Stdio};
#[cfg(test)]
use std::sync::{Mutex, OnceLock};

pub const TMUX_BACKEND: &str = "tmux_pane";
pub const ITERM_BACKEND: &str = "iterm_pane";

pub const PANE_BACKEND_ENV: &str = "HELLOX_AGENT_PANE_BACKEND";
pub const TMUX_COMMAND_ENV: &str = "HELLOX_AGENT_TMUX_COMMAND";
pub const ITERM_COMMAND_ENV: &str = "HELLOX_AGENT_ITERM_COMMAND";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneCommandPrefixStatus {
    pub env_name: &'static str,
    pub source: &'static str,
    pub prefix: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneBackendPreflight {
    pub requested_backend_raw: Option<String>,
    pub requested_backend: Option<String>,
    pub detected_backend: Option<String>,
    pub tmux_available: bool,
    pub tmux_attached: bool,
    pub iterm_available: bool,
    pub iterm_reason: String,
    pub tmux_command: PaneCommandPrefixStatus,
    pub iterm_command: PaneCommandPrefixStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePaneBackend {
    Tmux,
    ITerm,
}

impl NativePaneBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tmux => TMUX_BACKEND,
            Self::ITerm => ITERM_BACKEND,
        }
    }
}

pub fn detect_native_pane_backend() -> Option<NativePaneBackend> {
    if let Ok(raw) = env::var(PANE_BACKEND_ENV) {
        match raw.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "tmux" | "tmux_pane" => return Some(NativePaneBackend::Tmux),
            "iterm" | "iterm_pane" => return Some(NativePaneBackend::ITerm),
            _ => {}
        }
    }

    if env::var_os("TMUX").is_some() || command_available("tmux", &["-V"]) {
        return Some(NativePaneBackend::Tmux);
    }

    #[cfg(target_os = "macos")]
    {
        if env::var("TERM_PROGRAM")
            .ok()
            .as_deref()
            .is_some_and(|value| value == "iTerm.app")
            || command_available("osascript", &["-e", "return \"iterm\""])
        {
            return Some(NativePaneBackend::ITerm);
        }
    }

    None
}

pub fn pane_backend_preflight() -> PaneBackendPreflight {
    let requested_backend_raw = env::var(PANE_BACKEND_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let requested_backend = requested_backend_raw
        .as_deref()
        .and_then(normalize_requested_backend_name)
        .map(ToString::to_string);
    let tmux_attached = env::var_os("TMUX").is_some();
    let tmux_available = tmux_attached || command_available("tmux", &["-V"]);
    let iterm_available = detect_iterm_host();

    PaneBackendPreflight {
        requested_backend_raw,
        requested_backend,
        detected_backend: detect_native_pane_backend()
            .map(NativePaneBackend::as_str)
            .map(ToString::to_string),
        tmux_available,
        tmux_attached,
        iterm_available,
        iterm_reason: iterm_unavailable_reason().to_string(),
        tmux_command: command_prefix_status_from_env(TMUX_COMMAND_ENV, &["tmux"]),
        iterm_command: command_prefix_status_from_env(ITERM_COMMAND_ENV, &["osascript"]),
    }
}

fn command_available(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn command_prefix_status_from_env(
    env_name: &'static str,
    default: &[&str],
) -> PaneCommandPrefixStatus {
    match env::var(env_name) {
        Ok(raw) if !raw.trim().is_empty() => match serde_json::from_str::<Vec<String>>(&raw) {
            Ok(prefix)
                if !prefix.is_empty() && prefix.iter().all(|item| !item.trim().is_empty()) =>
            {
                PaneCommandPrefixStatus {
                    env_name,
                    source: "env",
                    prefix,
                    error: None,
                }
            }
            Ok(_) => PaneCommandPrefixStatus {
                env_name,
                source: "env",
                prefix: Vec::new(),
                error: Some("expected non-empty JSON array of non-empty strings".to_string()),
            },
            Err(error) => PaneCommandPrefixStatus {
                env_name,
                source: "env",
                prefix: Vec::new(),
                error: Some(format!("expected JSON array of strings ({error})")),
            },
        },
        _ => PaneCommandPrefixStatus {
            env_name,
            source: "default",
            prefix: default.iter().map(|item| item.to_string()).collect(),
            error: None,
        },
    }
}

fn normalize_requested_backend_name(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "tmux" | "tmux_pane" => Some(TMUX_BACKEND),
        "iterm" | "iterm_pane" => Some(ITERM_BACKEND),
        _ => None,
    }
}

fn detect_iterm_host() -> bool {
    #[cfg(target_os = "macos")]
    {
        if env::var("TERM_PROGRAM")
            .ok()
            .as_deref()
            .is_some_and(|value| value == "iTerm.app")
            || command_available("osascript", &["-e", "return \"iterm\""])
        {
            return true;
        }
    }

    false
}

fn iterm_unavailable_reason() -> &'static str {
    if cfg!(target_os = "macos") {
        "unavailable"
    } else {
        "unsupported (non-macos)"
    }
}

#[cfg(test)]
pub(crate) fn pane_backend_test_env_lock() -> &'static Mutex<()> {
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    ENV_LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = env::var(key).ok();
            env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.as_ref() {
                env::set_var(self.key, previous);
            } else {
                env::remove_var(self.key);
            }
        }
    }

    #[test]
    fn pane_backend_preflight_honors_requested_backend_override() {
        let _env_lock = pane_backend_test_env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _backend = EnvGuard::set(PANE_BACKEND_ENV, "iterm");
        let _command = EnvGuard::set(
            ITERM_COMMAND_ENV,
            "[\"pwsh\",\"-NoProfile\",\"-Command\",\"return\"]",
        );

        let report = pane_backend_preflight();
        assert_eq!(report.requested_backend.as_deref(), Some(ITERM_BACKEND));
        assert_eq!(report.detected_backend.as_deref(), Some(ITERM_BACKEND));
        assert_eq!(report.iterm_command.source, "env");
        assert!(report.iterm_command.error.is_none());
    }

    #[test]
    fn pane_backend_preflight_reports_invalid_command_prefix() {
        let _env_lock = pane_backend_test_env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _command = EnvGuard::set(TMUX_COMMAND_ENV, "{not-json}");

        let report = pane_backend_preflight();
        assert_eq!(report.tmux_command.source, "env");
        assert!(report.tmux_command.error.is_some());
        assert!(report.tmux_command.prefix.is_empty());
    }
}
