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
    let mut cmd = Command::new("tasklist");
    hide_console(&mut cmd);
    let out = cmd
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

/// How far a process may have started from the recorded launch time. The
/// recorded time is taken just before the spawn; boot-time based clocks are
/// only second-accurate.
pub const START_TOLERANCE_MS: u64 = 10_000;

/// Does a process that started at `process_start_ms` (unix ms) fit a session
/// recorded at `recorded_ms`? An unknown start time cannot be shown to
/// mismatch, so it passes (the Java-name check still applies). A process
/// that started clearly before the launch, or long after it, is a stranger
/// that reused the pid.
pub fn start_time_matches(
    recorded_ms: u64,
    process_start_ms: Option<u64>,
    tolerance_ms: u64,
) -> bool {
    match process_start_ms {
        None => true,
        Some(start) => start.abs_diff(recorded_ms) <= tolerance_ms,
    }
}

/// Start time of a process in unix milliseconds, when the OS tells us.
pub fn process_start_ms(pid: u32) -> Option<u64> {
    start_ms_impl(pid)
}

#[cfg(target_os = "linux")]
fn start_ms_impl(pid: u32) -> Option<u64> {
    // Field 22 of /proc/<pid>/stat: start time in clock ticks since boot.
    // USER_HZ is 100 on every Linux ABI.
    const HZ: u64 = 100;
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, rest) = stat.rsplit_once(')')?;
    // `rest` starts at field 3 (state), so field 22 is index 19.
    let ticks: u64 = rest.split_whitespace().nth(19)?.parse().ok()?;
    let btime: u64 = std::fs::read_to_string("/proc/stat")
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("btime "))?
        .trim()
        .parse()
        .ok()?;
    Some(btime * 1000 + ticks * 1000 / HZ)
}

/// `ps -o etime=` is `[[dd-]hh:]mm:ss` since the start, independent of the
/// time zone (unlike `lstart`).
#[cfg(all(unix, not(target_os = "linux")))]
fn start_ms_impl(pid: u32) -> Option<u64> {
    let out = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "etime="])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let elapsed = parse_etime_secs(&String::from_utf8_lossy(&out.stdout))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis() as u64;
    Some(now.saturating_sub(elapsed * 1000))
}

/// PowerShell is present on every supported Windows; `wmic` is not.
#[cfg(windows)]
fn start_ms_impl(pid: u32) -> Option<u64> {
    let script = format!(
        "[DateTimeOffset]::new((Get-Process -Id {pid}).StartTime).ToUnixTimeMilliseconds()"
    );
    let mut cmd = Command::new("powershell");
    hide_console(&mut cmd);
    let out = cmd
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// `[[dd-]hh:]mm:ss` to seconds.
#[cfg_attr(any(windows, target_os = "linux"), allow(dead_code))]
pub(crate) fn parse_etime_secs(raw: &str) -> Option<u64> {
    let raw = raw.trim();
    let (days, rest) = match raw.split_once('-') {
        Some((d, r)) => (d.parse::<u64>().ok()?, r),
        None => (0, raw),
    };
    let parts: Vec<u64> = rest
        .split(':')
        .map(|p| p.parse::<u64>().ok())
        .collect::<Option<_>>()?;
    let secs = match parts.as_slice() {
        [m, s] => m * 60 + s,
        [h, m, s] => h * 3600 + m * 60 + s,
        _ => return None,
    };
    Some(days * 86_400 + secs)
}

/// Ask a verified Java process to terminate (SIGTERM; `taskkill` = close the window on Windows), so the game saves.
pub fn terminate_java(pid: u32) -> Result<()> {
    terminate_java_started(pid, None, false)
}

/// Like [`terminate_java`], and when `started_at_ms` is given the process
/// must also have started then (a reused pid of another JVM is refused).
/// `force` kills without asking (SIGKILL; `taskkill /F`): the follow-up when the game did not end in time.
pub fn terminate_java_started(pid: u32, started_at_ms: Option<u64>, force: bool) -> Result<()> {
    if let Some(recorded) = started_at_ms {
        if is_game_alive(pid)
            && !start_time_matches(recorded, process_start_ms(pid), START_TOLERANCE_MS)
        {
            return Err(RuntimeError::NotJava(pid));
        }
    }
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
        .args([if force { "-KILL" } else { "-TERM" }, &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    #[cfg(windows)]
    let status = {
        let mut cmd = Command::new("taskkill");
        hide_console(&mut cmd);
        cmd
    }
    .args(if force {
        vec!["/PID".to_string(), pid.to_string(), "/F".to_string()]
    } else {
        vec!["/PID".to_string(), pid.to_string()]
    })
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status();
    // Without /F, taskkill closes the game window (Minecraft saves and quits); a JVM without a window refuses it: kill.
    #[cfg(windows)]
    if !force && !matches!(status, Ok(s) if s.success()) {
        return terminate_java_started(pid, started_at_ms, true);
    }
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

/// `kill -0` only asks whether the pid exists (and may be signalled); it sends nothing.
#[cfg(all(unix, not(target_os = "linux")))]
fn probe_exists_other(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// CSV rows of `tasklist` are `"image","pid",...`; any row means the pid exists.
#[cfg(windows)]
fn probe_exists_other(pid: u32) -> bool {
    let mut cmd = Command::new("tasklist");
    hide_console(&mut cmd);
    let out = cmd
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    let Ok(out) = out else { return false };
    let needle = format!("\",\"{pid}\",");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .any(|l| l.contains(&needle))
}

/// Windows: spawned helper tools must not open a console window of their own.
#[cfg(windows)]
fn hide_console(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
#[allow(dead_code)]
fn hide_console(_cmd: &mut Command) {}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn start_time_comparison_is_a_pure_tolerance_check() {
        let t = 1_700_000_000_000;
        assert!(start_time_matches(t, Some(t), 10_000));
        assert!(start_time_matches(t, Some(t + 9_999), 10_000));
        assert!(start_time_matches(t, Some(t - 9_999), 10_000));
        // A JVM that started an hour earlier or later is another process.
        assert!(!start_time_matches(t, Some(t - 3_600_000), 10_000));
        assert!(!start_time_matches(t, Some(t + 3_600_000), 10_000));
        assert!(!start_time_matches(t, Some(t + 10_001), 10_000));
        // Unknown start time cannot prove a mismatch.
        assert!(start_time_matches(t, None, 10_000));
    }

    #[test]
    fn etime_parsing() {
        assert_eq!(parse_etime_secs("   05:07\n"), Some(307));
        assert_eq!(parse_etime_secs("01:05:07"), Some(3907));
        assert_eq!(parse_etime_secs("2-01:05:07"), Some(2 * 86_400 + 3907));
        assert_eq!(parse_etime_secs(""), None);
        assert_eq!(parse_etime_secs("x:y"), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn own_process_start_time_is_recent_and_plausible() {
        let start = process_start_ms(std::process::id()).unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        assert!(start <= now + 2_000, "start {start} now {now}");
        assert!(
            now - start < 6 * 3_600_000,
            "test binary started within hours"
        );
    }
}
