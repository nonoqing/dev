//! Unified process management to avoid Windows child process leaks

use std::process::Command;
use std::sync::LazyLock;
#[cfg(target_os = "macos")]
use std::sync::OnceLock;
#[cfg(unix)]
use std::time::Duration;
use tokio::process::Child;
use tokio::process::Command as TokioCommand;

use log::warn;

#[cfg(windows)]
use std::sync::{Arc, Mutex};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
use win32job::Job;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

static GLOBAL_PROCESS_MANAGER: LazyLock<ProcessManager> = LazyLock::new(ProcessManager::new);

pub struct ProcessManager {
    #[cfg(windows)]
    job: Arc<Mutex<Option<Job>>>,
}

impl ProcessManager {
    fn new() -> Self {
        let manager = Self {
            #[cfg(windows)]
            job: Arc::new(Mutex::new(None)),
        };

        #[cfg(windows)]
        {
            if let Err(e) = manager.initialize_job() {
                warn!("Failed to initialize Windows Job object: {}", e);
            }
        }

        manager
    }

    #[cfg(windows)]
    fn initialize_job(&self) -> Result<(), Box<dyn std::error::Error>> {
        use win32job::{ExtendedLimitInfo, Job};

        let job = Job::create()?;

        // Terminate all child processes when the Job closes
        let mut info = ExtendedLimitInfo::new();
        info.limit_kill_on_job_close();
        job.set_extended_limit_info(&info)?;

        // Assign current process to Job so child processes inherit automatically
        job.assign_current_process()?;

        let mut job_guard = self.job.lock().map_err(|e| {
            std::io::Error::other(format!("Failed to lock process manager job mutex: {}", e))
        })?;
        *job_guard = Some(job);

        Ok(())
    }

    pub fn cleanup_all(&self) {
        #[cfg(windows)]
        {
            let mut job_guard = match self.job.lock() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    warn!("Process manager job mutex was poisoned during cleanup, recovering lock");
                    poisoned.into_inner() as std::sync::MutexGuard<'_, Option<Job>>
                }
            };
            job_guard.take();
        }
    }
}

/// Create synchronous Command (Windows automatically adds CREATE_NO_WINDOW)
pub fn create_command<S: AsRef<std::ffi::OsStr>>(program: S) -> Command {
    let cmd = Command::new(program.as_ref());

    #[cfg(windows)]
    {
        let mut cmd = cmd;
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd
    }

    #[cfg(not(windows))]
    cmd
}

/// Create Tokio async Command (Windows automatically adds CREATE_NO_WINDOW)
pub fn create_tokio_command<S: AsRef<std::ffi::OsStr>>(program: S) -> TokioCommand {
    let cmd = TokioCommand::new(program.as_ref());

    #[cfg(target_os = "macos")]
    {
        let mut cmd = cmd;
        apply_cached_macos_path(&mut cmd);
        cmd
    }

    #[cfg(windows)]
    {
        let mut cmd = cmd;
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd
    }

    #[cfg(not(any(target_os = "macos", windows)))]
    cmd
}

/// Create a Tokio Command that runs a command string through the
/// platform shell, with CREATE_NO_WINDOW and macOS PATH applied.
pub fn create_shell_command(command: &str) -> TokioCommand {
    #[cfg(windows)]
    {
        let mut cmd = create_tokio_command("cmd");
        cmd.arg("/C").arg(command);
        cmd
    }

    #[cfg(not(windows))]
    {
        let mut cmd = create_tokio_command("sh");
        cmd.arg("-c").arg(command);
        cmd
    }
}

#[cfg(target_os = "macos")]
fn apply_cached_macos_path(cmd: &mut TokioCommand) {
    if let Some(path) = cached_macos_path_env() {
        cmd.env("PATH", path);
    }
}

#[cfg(target_os = "macos")]
fn cached_macos_path_env() -> Option<&'static std::ffi::OsString> {
    static MACOS_PATH_ENV: OnceLock<Option<std::ffi::OsString>> = OnceLock::new();
    MACOS_PATH_ENV.get_or_init(build_macos_path_env).as_ref()
}

#[cfg(target_os = "macos")]
fn build_macos_path_env() -> Option<std::ffi::OsString> {
    let existing_path = std::env::var_os("PATH");
    let mut entries = Vec::new();
    if let Some(path) = existing_path {
        entries.extend(std::env::split_paths(&path));
    }
    entries.extend(crate::system::platform_path_entries());

    if entries.is_empty() {
        return None;
    }

    let mut merged = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for path in entries {
        if path.as_os_str().is_empty() {
            continue;
        }
        let key = path.to_string_lossy().to_string();
        if seen.insert(key) {
            merged.push(path);
        }
    }

    std::env::join_paths(merged).ok()
}

pub fn cleanup_all_processes() {
    GLOBAL_PROCESS_MANAGER.cleanup_all();
}

/// Keep descendants of a long-lived service in the process-wide Job.
pub fn contain_current_process_tree() -> std::io::Result<()> {
    #[cfg(windows)]
    if GLOBAL_PROCESS_MANAGER
        .job
        .lock()
        .map_err(|error| std::io::Error::other(error.to_string()))?
        .is_none()
    {
        return Err(std::io::Error::other("Windows process Job is unavailable"));
    }
    Ok(())
}

/// Configure a tokio command to run in its own process group (Unix only).
///
/// On Unix, this sets the process group so that the entire process tree can be
/// terminated via process-group signaling. On non-Unix platforms this is a
/// no-op.
pub fn configure_process_group(command: &mut TokioCommand) {
    #[cfg(unix)]
    {
        command.process_group(0);
    }
    #[cfg(not(unix))]
    {
        let _ = command;
    }
}

/// Terminate a child process and its entire process tree.
///
/// On Unix, sends SIGTERM to the process group, waits briefly, then escalates
/// to SIGKILL if needed. On Windows, uses `taskkill /PID /T /F`. Falls back to
/// `child.start_kill()` if platform-specific signaling fails or is unavailable.
///
/// `label` is a caller-supplied identifier included in log messages for
/// diagnostics (e.g. a connection ID or process name).
pub async fn terminate_child_process_tree(label: &str, mut child: Child) {
    let pid = child.id();

    #[cfg(unix)]
    if let Some(pid) = pid {
        let process_group = format!("-{}", pid);
        match create_tokio_command("kill")
            .arg("-TERM")
            .arg(&process_group)
            .status()
            .await
        {
            Ok(status) if status.success() => {}
            Ok(status) => {
                warn!(
                    "Process group terminate exited unsuccessfully: label={} pid={} status={}",
                    label, pid, status
                );
            }
            Err(error) => {
                warn!(
                    "Failed to terminate process group: label={} pid={} error={}",
                    label, pid, error
                );
            }
        }

        match tokio::time::timeout(Duration::from_millis(750), child.wait()).await {
            Ok(Ok(_)) => return,
            Ok(Err(error)) => {
                warn!(
                    "Failed to wait for process after terminate: label={} pid={} error={}",
                    label, pid, error
                );
            }
            Err(_) => {}
        }

        if let Err(error) = create_tokio_command("kill")
            .arg("-KILL")
            .arg(&process_group)
            .status()
            .await
        {
            warn!(
                "Failed to kill process group: label={} pid={} error={}",
                label, pid, error
            );
        }
        let _ = child.wait().await;
        return;
    }

    #[cfg(windows)]
    if let Some(pid) = pid {
        match create_tokio_command("taskkill")
            .arg("/PID")
            .arg(pid.to_string())
            .arg("/T")
            .arg("/F")
            .status()
            .await
        {
            Ok(status) if status.success() => {
                let _ = child.wait().await;
                return;
            }
            Ok(status) => {
                warn!(
                    "Process tree kill exited unsuccessfully: label={} pid={} status={}",
                    label, pid, status
                );
            }
            Err(error) => {
                warn!(
                    "Failed to kill process tree: label={} pid={} error={}",
                    label, pid, error
                );
            }
        }
    }

    if let Err(error) = child.start_kill() {
        warn!("Failed to kill process: label={} error={}", label, error);
    }
    let _ = child.wait().await;
}
