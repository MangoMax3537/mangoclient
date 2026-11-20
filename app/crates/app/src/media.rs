//! Screenshots, logs, stats and storage. Ports of `screenshots.js`,
//! `gamelogs.js` and `storage.js`.

use crate::error::{AppError, Res};
use crate::image_protocol::file_url;
use crate::models::{
    LogEntry, LogKind, LogText, LogUpload, Screenshot, ScreenshotAction, StatsDay, StatsProfile,
    StatsSummary, StorageKey, StorageUsage,
};
use crate::service::{lock, AppService};
use flate2::read::GzDecoder;
use mango_core::Profile;
use mango_runtime::log_file_for;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// A longer log is shown from its end; that is where the error is.
pub const MAX_LOG_BYTES: usize = 4 * 1024 * 1024;
/// mclo.gs rejects pastes past its own limits.
pub const UPLOAD_MAX_LINES: usize = 25_000;
/// A gzip stream that expands past this is not a log.
const MAX_GZ_TOTAL: u64 = 1024 * 1024 * 1024;

fn mtime_ms(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// A bare file name: no separators, no `.`/`..`, no NUL.
fn bare_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', '\0'])
        && Path::new(name).file_name().is_some_and(|n| n == name)
}

// ---- screenshots ----

fn is_screenshot(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    l.ends_with(".png") || l.ends_with(".jpg") || l.ends_with(".jpeg")
}

impl AppService {
    fn screenshot_dir(&self, profile_id: &str) -> Res<PathBuf> {
        self.profile(profile_id)?;
        Ok(self
            .inner
            .paths
            .instance_dir(profile_id)?
            .join("screenshots"))
    }

    /// Newest first.
    pub async fn screenshots_list(&self, profile_id: String) -> Res<Vec<Screenshot>> {
        let dir = self.screenshot_dir(&profile_id)?;
        Ok(tokio::task::spawn_blocking(move || {
            let Ok(rd) = std::fs::read_dir(&dir) else {
                return Vec::new(); // the game has not taken one yet
            };
            let mut shots: Vec<Screenshot> = rd
                .flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    if !is_screenshot(&name) {
                        return None;
                    }
                    // `symlink_metadata`: a link is never listed (or served).
                    let meta = std::fs::symlink_metadata(e.path()).ok()?;
                    meta.is_file().then(|| Screenshot {
                        url: file_url(&e.path()),
                        size: meta.len(),
                        taken: mtime_ms(&meta),
                        name,
                    })
                })
                .collect();
            shots.sort_by(|a, b| b.taken.cmp(&a.taken).then_with(|| a.name.cmp(&b.name)));
            shots
        })
        .await?)
    }

    pub async fn screenshot_action(
        &self,
        profile_id: String,
        name: String,
        action: ScreenshotAction,
    ) -> Res<()> {
        let dir = self.screenshot_dir(&profile_id)?;
        if !bare_name(&name) || !is_screenshot(&name) {
            return Err(AppError::new("invalid_name", "Invalid screenshot name"));
        }
        let file = dir.join(&name);
        let is_file = std::fs::symlink_metadata(&file).is_ok_and(|m| m.is_file());
        if !is_file {
            return Err(AppError::new("not_found", "Screenshot not found"));
        }
        match action {
            ScreenshotAction::Delete => std::fs::remove_file(&file)?,
            ScreenshotAction::Reveal => self.inner.host.opener.reveal(&file)?,
            ScreenshotAction::Copy => self
                .inner
                .host
                .clipboard
                .copy_image(&file)
                .map_err(|e| AppError::new("clipboard", e))?,
        }
        Ok(())
    }

    // ---- logs ----

    fn log_dirs(&self, profile: &Profile) -> Res<(PathBuf, PathBuf, PathBuf)> {
        let instance = self.inner.paths.instance_dir(&profile.id)?;
        Ok((
            instance.join("logs"),
            instance.join("crash-reports"),
            log_file_for(&self.inner.paths.logs(), &profile.name, &profile.id),
        ))
    }

    pub async fn logs_list(&self, profile_id: String) -> Res<Vec<LogEntry>> {
        let profile = self.profile(&profile_id)?;
        let (game, crash, launcher) = self.log_dirs(&profile)?;
        Ok(tokio::task::spawn_blocking(move || {
            let mut entries = list_dir(&game, LogKind::Game);
            entries.extend(list_dir(&crash, LogKind::Crash));
            if let Ok(meta) = std::fs::symlink_metadata(&launcher) {
                if meta.is_file() {
                    let name = launcher
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    entries.push(LogEntry {
                        id: format!("launcher:{name}"),
                        name,
                        size: meta.len(),
                        modified: mtime_ms(&meta),
                        kind: LogKind::Launcher,
                    });
                }
            }
            entries.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.id.cmp(&b.id)));
            // `latest.log` leads: its mtime can lag behind a freshly rotated archive.
            if let Some(i) = entries
                .iter()
                .position(|e| e.kind == LogKind::Game && e.name == "latest.log")
            {
                let latest = entries.remove(i);
                entries.insert(0, latest);
            }
            entries
        })
        .await?)
    }

    /// `game:<name>|crash:<name>|launcher:<name>` -> the file. Basename only.
    fn log_file(&self, profile_id: &str, id: &str) -> Res<(PathBuf, String)> {
        let unknown = || AppError::new("unknown_log", "Unknown log");
        let profile = self.profile(profile_id)?;
        let (kind, name) = id.split_once(':').ok_or_else(unknown)?;
        if !bare_name(name) {
            return Err(unknown());
        }
        let (game, crash, launcher) = self.log_dirs(&profile)?;
        let file = match kind {
            "game" if is_log_name(name) => game.join(name),
            "crash" if is_log_name(name) => crash.join(name),
            "launcher" if launcher.file_name().is_some_and(|n| n == name) => launcher,
            _ => return Err(unknown()),
        };
        Ok((file, name.to_string()))
    }

    pub async fn logs_read(&self, profile_id: String, id: String) -> Res<LogText> {
        let (file, _) = self.log_file(&profile_id, &id)?;
        read_log(file).await
    }

    pub async fn logs_upload(&self, profile_id: String, id: String) -> Res<LogUpload> {
        let (file, _) = self.log_file(&profile_id, &id)?;
        let text = read_log(file).await?.text;
        let lines: Vec<&str> = text.lines().collect();
        let body = if lines.len() > UPLOAD_MAX_LINES {
            lines[lines.len() - UPLOAD_MAX_LINES..].join("\n")
        } else {
            text.clone()
        };
        upload_to_mclogs(&self.inner.opts, &body).await
    }

    pub async fn logs_delete(&self, profile_id: String, id: String) -> Res<()> {
        let (file, name) = self.log_file(&profile_id, &id)?;
        if name == "latest.log" {
            return Err(AppError::new(
                "log_protected",
                "The current log cannot be deleted",
            ));
        }
        let is_file = std::fs::symlink_metadata(&file).is_ok_and(|m| m.is_file());
        if !is_file {
            return Err(AppError::new("not_found", "Log not found"));
        }
        std::fs::remove_file(&file)?;
        Ok(())
    }

    // ---- stats ----

    pub fn stats_summary(&self, days: u32) -> Res<StatsSummary> {
        let days = days.clamp(1, 366);
        let profiles = self.inner.with_store(|s| s.profiles.clone());
        let summary = mango_mango::stats::Stats::new(&self.inner.paths).summary(&profiles, days);

        // Session counts are not part of the crate's summary: count them from
        // the journal for the same window (local midnight of the first day).
        let from_ms = summary
            .days
            .first()
            .and_then(|d| chrono::NaiveDate::parse_from_str(&d.date, "%Y-%m-%d").ok())
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .and_then(|dt| dt.and_local_timezone(chrono::Local).earliest())
            .map_or(i64::MIN, |dt| dt.timestamp_millis());
        let journal = read_journal(&self.inner.paths.stats_file());
        let mut sessions: HashMap<&str, u64> = HashMap::new();
        let mut in_window = 0u64;
        let mut longest = 0u64;
        for s in journal.iter().filter(|s| s.s >= from_ms) {
            *sessions.entry(s.p.as_str()).or_default() += 1;
            in_window += 1;
            longest = longest.max(s.ms);
        }

        let per_profile = summary
            .per_profile
            .iter()
            .filter(|p| p.window_ms > 0)
            .map(|p| StatsProfile {
                profile_id: p.id.clone(),
                name: p.name.clone(),
                ms: p.window_ms,
                sessions: sessions.get(p.id.as_str()).copied().unwrap_or(0),
            })
            .collect();
        Ok(StatsSummary {
            days: summary
                .days
                .into_iter()
                .map(|d| StatsDay {
                    date: d.date,
                    ms: d.ms,
                })
                .collect(),
            per_profile,
            total_ms: summary.window_ms,
            sessions: in_window,
            longest_ms: longest,
        })
    }

    // ---- storage ----

    fn storage_dir(&self, key: StorageKey) -> PathBuf {
        let p = &self.inner.paths;
        match key {
            StorageKey::Instances => p.instances(),
            StorageKey::Versions => p.versions(),
            StorageKey::Libraries => p.libraries(),
            StorageKey::Assets => p.assets(),
            StorageKey::Runtimes => p.runtimes(),
            StorageKey::Natives => p.natives(),
            StorageKey::Cache => p.cache(),
            StorageKey::Logs => p.logs(),
        }
    }

    pub async fn storage_usage(&self) -> Res<Vec<StorageUsage>> {
        let dirs: Vec<(StorageKey, PathBuf)> = StorageKey::ALL
            .into_iter()
            .map(|k| (k, self.storage_dir(k)))
            .collect();
        Ok(tokio::task::spawn_blocking(move || {
            dirs.into_iter()
                .map(|(key, dir)| StorageUsage {
                    key,
                    bytes: dir_size(&dir),
                    // Instances hold worlds and are never cleared automatically.
                    clearable: key != StorageKey::Instances,
                })
                .collect()
        })
        .await?)
    }

    pub async fn storage_clear(&self, key: StorageKey) -> Res<Vec<StorageUsage>> {
        if key == StorageKey::Instances {
            return Err(AppError::new(
                "not_clearable",
                "This folder holds your worlds and is never cleared automatically",
            ));
        }
        if !self.game_running().is_empty() || !lock(&self.inner.launching).is_empty() {
            return Err(AppError::new(
                "game_running",
                "Close the game before clearing storage",
            ));
        }
        let dir = self.storage_dir(key);
        tokio::task::spawn_blocking(move || {
            empty_dir(&dir);
            std::fs::create_dir_all(&dir)
        })
        .await??;
        self.storage_usage().await
    }
}

fn is_log_name(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    l.ends_with(".log") || l.ends_with(".log.gz") || l.ends_with(".txt")
}

fn list_dir(dir: &Path, kind: LogKind) -> Vec<LogEntry> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let prefix = match kind {
        LogKind::Crash => "crash",
        LogKind::Launcher => "launcher",
        LogKind::Game => "game",
    };
    rd.flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if !is_log_name(&name) {
                return None;
            }
            let meta = std::fs::symlink_metadata(e.path()).ok()?;
            meta.is_file().then(|| LogEntry {
                id: format!("{prefix}:{name}"),
                size: meta.len(),
                modified: mtime_ms(&meta),
                kind,
                name,
            })
        })
        .collect()
}

async fn read_log(file: PathBuf) -> Res<LogText> {
    tokio::task::spawn_blocking(move || read_tail(&file, MAX_LOG_BYTES))
        .await?
        .map_err(AppError::from)
}

/// The last `max` bytes of a log (gzip streamed), lossily decoded.
pub(crate) fn read_tail(file: &Path, max: usize) -> std::io::Result<LogText> {
    let meta = std::fs::symlink_metadata(file)?;
    if !meta.is_file() {
        return Err(std::io::Error::from(std::io::ErrorKind::NotFound));
    }
    let gz = file
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("gz"));
    let mut f = std::fs::File::open(file)?;
    if !gz {
        let len = usize::try_from(meta.len()).unwrap_or(usize::MAX);
        let take = len.min(max);
        f.seek(SeekFrom::Start((len - take) as u64))?;
        let mut buf = Vec::with_capacity(take);
        f.take(take as u64).read_to_end(&mut buf)?;
        return Ok(LogText {
            text: String::from_utf8_lossy(&buf).into_owned(),
            truncated: len > take,
        });
    }
    let mut dec = GzDecoder::new(f);
    let mut tail: Vec<u8> = Vec::new();
    let mut total: u64 = 0;
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let n = dec.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > MAX_GZ_TOTAL {
            return Err(std::io::Error::other("log expands past the size limit"));
        }
        tail.extend_from_slice(&chunk[..n]);
        if tail.len() > max * 2 {
            let cut = tail.len() - max;
            tail.drain(..cut);
        }
    }
    if tail.len() > max {
        let cut = tail.len() - max;
        tail.drain(..cut);
    }
    Ok(LogText {
        truncated: total > tail.len() as u64,
        text: String::from_utf8_lossy(&tail).into_owned(),
    })
}

/// `application/x-www-form-urlencoded` value.
fn form_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"*-._".contains(&b) {
            out.push(b as char);
        } else if b == b' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

async fn upload_to_mclogs(opts: &crate::service::ServiceOptions, body: &str) -> Res<LogUpload> {
    let client = reqwest::Client::builder()
        .timeout(opts.fetch.timeout)
        .user_agent(mango_net::UA)
        .build()
        .map_err(|e| AppError::new("net", e.to_string()))?;
    let res = client
        .post(&opts.mclogs_api)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!("content={}", form_encode(body)))
        .send()
        .await
        .map_err(|e| AppError::new("net", e.to_string()))?;
    let status = res.status();
    let data: Option<serde_json::Value> = res.json().await.ok();
    let ok = data
        .as_ref()
        .and_then(|d| d.get("success"))
        .and_then(serde_json::Value::as_bool)
        == Some(true);
    if !status.is_success() || !ok {
        let reason = data
            .as_ref()
            .and_then(|d| d.get("error"))
            .and_then(serde_json::Value::as_str)
            .map_or_else(
                || format!("mclo.gs refused the upload (HTTP {})", status.as_u16()),
                str::to_string,
            );
        return Err(AppError::new("upload_failed", reason));
    }
    let url = data
        .as_ref()
        .and_then(|d| d.get("url"))
        .and_then(serde_json::Value::as_str)
        .filter(|u| mango_mango::security::is_allowed_external_url(u))
        .ok_or_else(|| AppError::new("upload_failed", "mclo.gs returned no usable link"))?;
    Ok(LogUpload {
        url: url.to_string(),
    })
}

fn read_journal(file: &Path) -> Vec<mango_mango::stats::SessionEntry> {
    let Ok(bytes) = std::fs::read(file) else {
        return Vec::new();
    };
    let Ok(serde_json::Value::Object(data)) = serde_json::from_slice(&bytes) else {
        return Vec::new();
    };
    data.get("sessions")
        .and_then(serde_json::Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Recursive size; symlinks count as their own entry and are not followed.
pub(crate) fn dir_size(dir: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue; // never created, or removed under us
        };
        for e in rd.flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                _ => {
                    if let Ok(m) = std::fs::symlink_metadata(e.path()) {
                        total += m.len();
                    }
                }
            }
        }
    }
    total
}

/// Empty a directory without removing it; what is in use stays.
fn empty_dir(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let _ = match e.file_type() {
            Ok(t) if t.is_dir() => std::fs::remove_dir_all(e.path()),
            _ => std::fs::remove_file(e.path()),
        };
    }
}
