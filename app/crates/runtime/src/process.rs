//! Process identity checks and signalling without `unsafe`.
//!
//! A session outlives the launcher, so a recorded pid has to be re-checked:
//! operating systems reuse pids, and mistaking a stranger's process for the
//! game would mean showing it as running or killing it.

use crate::error::{Result, RuntimeError};
use std::process::{Command, Stdio};

/// Does the process image name look like a JVM (`java`, `javaw.exe`)?
pub fn is_java_name(name: &str) -> bool {
    name.to_ascii_lowercase().contains("java")
}

#[cfg(target_os = "linux")]
fn probe(pid: u32) -> bool {
    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default();
    if !is_java_name(comm.trim()) {
        return false;
    }
    // A zombie is not running any more.
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
    let state = stat
        .rsplit_once(')')
        .and_then(|(_, rest)| rest.trim_start().chars().next());
    state != Some('Z')
}

#[cfg(all(unix, not(target_os = "linux")))]
fn probe(pid: u32) -> bool {
    Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|o| o.status.success() && is_java_name(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or(false)
}

#[cfg(windows)]
fn probe(pid: u32) -> bool {
    let out = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    let Ok(out) = out else { return false };
    String::from_utf8_lossy(&out.stdout).lines().any(|l| {
        let l = l.trim().trim_start_matches('"').to_ascii_lowercase();
        l.starts_with("java.exe\"") || l.starts_with("javaw.exe\"")
    })
}

/// Is `pid` alive and a Java process?
pub fn is_game_alive(pid: u32) -> bool {
    pid != 0 && probe(pid)
}

/// Ask a verified Java process to terminate (SIGTERM; `taskkill /F` on Windows).
pub fn terminate_java(pid: u32) -> Result<()> {
    if !is_game_alive(pid) {
        // Never signal something we cannot prove is the game.
        return if probe_exists_other(pid) {
            Err(RuntimeError::NotJava(pid))
        } else {
            Err(RuntimeError::NotAlive(pid))
        };
    }
    #[cfg(unix)]
    let status = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    #[cfg(windows)]
    let status = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(RuntimeError::Stop {
            pid,
            reason: format!("signal command exited with {s}"),
        }),
        Err(e) => Err(RuntimeError::Stop {
            pid,
            reason: e.to_string(),
        }),
    }
}

/// Distinguish "gone" from "alive but not java" for the error code.
#[cfg(target_os = "linux")]
fn probe_exists_other(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

#[cfg(not(target_os = "linux"))]
fn probe_exists_other(_pid: u32) -> bool {
    false
}
