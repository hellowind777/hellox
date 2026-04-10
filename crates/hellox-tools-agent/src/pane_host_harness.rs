use std::cell::RefCell;
use std::collections::VecDeque;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

pub const PANE_HOST_RECORD_ENV: &str = "HELLOX_AGENT_PANE_HOST_RECORD_PATH";
pub const PANE_HOST_REPLAY_ENV: &str = "HELLOX_AGENT_PANE_HOST_REPLAY_PATH";

thread_local! {
    static THREAD_RECORD_PATH: RefCell<Option<PathBuf>> = RefCell::new(None);
    static THREAD_REPLAY_PATH: RefCell<Option<PathBuf>> = RefCell::new(None);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneHostCommandOutput {
    pub success: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct PaneHostRecord {
    context: String,
    program: String,
    argv: Vec<String>,
    success: bool,
    stdout: String,
    stderr: String,
}

#[derive(Debug)]
struct ReplayHarness {
    path: PathBuf,
    queue: VecDeque<PaneHostRecord>,
}

static REPLAY_HARNESS: OnceLock<Mutex<Option<ReplayHarness>>> = OnceLock::new();

fn replay_harness_state() -> &'static Mutex<Option<ReplayHarness>> {
    REPLAY_HARNESS.get_or_init(|| Mutex::new(None))
}

pub fn run_pane_host_command(
    program: &str,
    argv: &[String],
    context: &str,
) -> Result<PaneHostCommandOutput> {
    if let Some(replay_path) = thread_replay_path_override() {
        return replay_pane_host_command(&replay_path, program, argv, context);
    }
    if let Some(replay_path) = env::var_os(PANE_HOST_REPLAY_ENV) {
        return replay_pane_host_command(Path::new(&replay_path), program, argv, context);
    }

    let output = run_command_output(program, argv, context)?;
    if let Some(record_path) = thread_record_path_override() {
        record_pane_host_command(&record_path, program, argv, context, &output)?;
    } else if let Some(record_path) = env::var_os(PANE_HOST_RECORD_ENV) {
        record_pane_host_command(Path::new(&record_path), program, argv, context, &output)?;
    }
    Ok(output)
}

fn thread_record_path_override() -> Option<PathBuf> {
    THREAD_RECORD_PATH.with(|path| path.borrow().clone())
}

fn thread_replay_path_override() -> Option<PathBuf> {
    THREAD_REPLAY_PATH.with(|path| path.borrow().clone())
}

fn run_command_output(
    program: &str,
    argv: &[String],
    context: &str,
) -> Result<PaneHostCommandOutput> {
    let output = Command::new(program)
        .args(argv)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("failed to invoke {context}"))?;
    Ok(PaneHostCommandOutput {
        success: output.status.success(),
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

fn record_pane_host_command(
    record_path: &Path,
    program: &str,
    argv: &[String],
    context: &str,
    output: &PaneHostCommandOutput,
) -> Result<()> {
    if let Some(parent) = record_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create pane-host record directory `{}`",
                    parent.display()
                )
            })?;
        }
    }

    let record = PaneHostRecord {
        context: context.to_string(),
        program: program.to_string(),
        argv: argv.to_vec(),
        success: output.success,
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    };

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(record_path)
        .with_context(|| {
            format!(
                "failed to open pane-host record file `{}`",
                record_path.display()
            )
        })?;
    writeln!(file, "{}", serde_json::to_string(&record)?)?;
    Ok(())
}

fn replay_pane_host_command(
    replay_path: &Path,
    program: &str,
    argv: &[String],
    context: &str,
) -> Result<PaneHostCommandOutput> {
    let mut guard = replay_harness_state()
        .lock()
        .map_err(|_| anyhow!("pane-host replay harness mutex is poisoned"))?;

    let queue = match guard.as_mut() {
        Some(harness) if harness.path == replay_path => &mut harness.queue,
        _ => {
            let queue = load_replay_file(replay_path)?;
            *guard = Some(ReplayHarness {
                path: replay_path.to_path_buf(),
                queue,
            });
            &mut guard
                .as_mut()
                .expect("replay harness just initialized")
                .queue
        }
    };

    let Some(next) = queue.pop_front() else {
        return Err(anyhow!(
            "pane-host replay exhausted: missing record for {context} ({program} {:?})",
            argv
        ));
    };

    let expected = PaneHostRecord {
        context: context.to_string(),
        program: program.to_string(),
        argv: argv.to_vec(),
        success: next.success,
        stdout: next.stdout.clone(),
        stderr: next.stderr.clone(),
    };

    if next.context != expected.context
        || next.program != expected.program
        || next.argv != expected.argv
    {
        return Err(anyhow!(
            "pane-host replay mismatch:\nexpected: {expected:?}\nactual: {next:?}"
        ));
    }

    Ok(PaneHostCommandOutput {
        success: next.success,
        stdout: next.stdout.into_bytes(),
        stderr: next.stderr.into_bytes(),
    })
}

fn load_replay_file(replay_path: &Path) -> Result<VecDeque<PaneHostRecord>> {
    let file = OpenOptions::new()
        .read(true)
        .open(replay_path)
        .with_context(|| {
            format!(
                "failed to open pane-host replay file `{}`",
                replay_path.display()
            )
        })?;
    let reader = BufReader::new(file);
    let mut queue = VecDeque::new();
    for (index, line) in reader.lines().enumerate() {
        let line =
            line.with_context(|| format!("failed to read pane-host replay line {}", index + 1))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let record = serde_json::from_str::<PaneHostRecord>(trimmed).with_context(|| {
            format!(
                "failed to parse pane-host replay line {} as JSON",
                index + 1
            )
        })?;
        queue.push_back(record);
    }
    Ok(queue)
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::PaneHostRecord;
    use crate::native_pane_backend::launch_native_pane;
    use crate::native_pane_backend_preflight::NativePaneBackend;
    use crate::native_pane_layout::{
        build_iterm_script, build_tmux_new_session_args, build_tmux_select_layout_args,
        pane_group_name, pane_group_title, pane_title, shell_join,
    };

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

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

        fn remove(key: &'static str) -> Self {
            let previous = env::var(key).ok();
            env::remove_var(key);
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

    fn unique_fixture_path(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        env::temp_dir().join(format!("hellox-pane-host-{label}-{nanos}.jsonl"))
    }

    fn write_fixture(path: &std::path::Path, records: &[PaneHostRecord]) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let text = records
            .iter()
            .map(|record| serde_json::to_string(record).expect("serialize fixture record"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(path, format!("{text}\n")).expect("write fixture file");
    }

    struct ThreadReplayGuard {
        previous: Option<std::path::PathBuf>,
    }

    impl ThreadReplayGuard {
        fn enable(path: &std::path::Path) -> Self {
            let previous = super::THREAD_REPLAY_PATH.with(|value| value.borrow().clone());
            super::THREAD_REPLAY_PATH.with(|value| *value.borrow_mut() = Some(path.to_path_buf()));
            Self { previous }
        }
    }

    impl Drop for ThreadReplayGuard {
        fn drop(&mut self) {
            let previous = self.previous.clone();
            super::THREAD_REPLAY_PATH.with(|value| *value.borrow_mut() = previous);
        }
    }

    #[test]
    fn pane_host_replay_drives_tmux_launch_sequence() {
        let _env_lock = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *super::replay_harness_state()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;

        let fixture_path = unique_fixture_path("tmux-launch");
        let _replay = ThreadReplayGuard::enable(&fixture_path);
        let _record = EnvGuard::remove(super::PANE_HOST_RECORD_ENV);
        let _backend_command = EnvGuard::set("HELLOX_AGENT_BACKEND_COMMAND", "[\"hx\"]");

        let session_id = "session-1";
        let agent_name = Some("alice");
        let pane_group = Some("team-alpha");
        let layout_strategy = Some("fanout");
        let layout_slot = Some("primary");
        let job_path = std::path::Path::new("job.json");

        let worker_command = shell_join(&[
            "hx".to_string(),
            "--job".to_string(),
            job_path.display().to_string(),
        ]);
        let title = pane_title(session_id, agent_name);
        let group = pane_group_name(session_id, pane_group);
        let launch_args = build_tmux_new_session_args(&group, &title, &worker_command);
        let layout_args = build_tmux_select_layout_args(&format!("{group}:0"), "main-vertical");

        write_fixture(
            &fixture_path,
            &[
                PaneHostRecord {
                    context: "tmux pane launch".to_string(),
                    program: "tmux".to_string(),
                    argv: launch_args,
                    success: true,
                    stdout: "%1\n".to_string(),
                    stderr: String::new(),
                },
                PaneHostRecord {
                    context: "tmux pane layout".to_string(),
                    program: "tmux".to_string(),
                    argv: layout_args,
                    success: true,
                    stdout: String::new(),
                    stderr: String::new(),
                },
            ],
        );

        let pane_target = launch_native_pane(
            NativePaneBackend::Tmux,
            session_id,
            job_path,
            agent_name,
            pane_group,
            layout_strategy,
            layout_slot,
            None,
        )
        .expect("launch tmux pane via replay");

        assert_eq!(pane_target, "%1");
    }

    #[test]
    fn pane_host_replay_drives_iterm_launch_sequence() {
        let _env_lock = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *super::replay_harness_state()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;

        let fixture_path = unique_fixture_path("iterm-launch");
        let _replay = ThreadReplayGuard::enable(&fixture_path);
        let _record = EnvGuard::remove(super::PANE_HOST_RECORD_ENV);
        let _backend_command = EnvGuard::set("HELLOX_AGENT_BACKEND_COMMAND", "[\"hx\"]");

        let session_id = "session-2";
        let agent_name = Some("bob");
        let pane_group = Some("team-beta");
        let layout_slot = Some("right");
        let job_path = std::path::Path::new("job.json");

        let worker_command = shell_join(&[
            "hx".to_string(),
            "--job".to_string(),
            job_path.display().to_string(),
        ]);
        let title = pane_title(session_id, agent_name);
        let group = pane_group_title(session_id, pane_group);
        let script = build_iterm_script(&worker_command, &title, &group, layout_slot, None);

        write_fixture(
            &fixture_path,
            &[PaneHostRecord {
                context: "iTerm pane launch".to_string(),
                program: "osascript".to_string(),
                argv: vec!["-e".to_string(), script],
                success: true,
                stdout: "123\n".to_string(),
                stderr: String::new(),
            }],
        );

        let pane_target = launch_native_pane(
            NativePaneBackend::ITerm,
            session_id,
            job_path,
            agent_name,
            pane_group,
            None,
            layout_slot,
            None,
        )
        .expect("launch iterm pane via replay");

        assert_eq!(pane_target, "123");
    }
}
