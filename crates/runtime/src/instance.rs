//! A running (or adopted) game process.

use crate::error::{Result, RuntimeError};
use crate::follower::{FollowStart, LogFollower, DEFAULT_POLL};
use crate::logparse::{LogLevel, LogParser};
use crate::process::{is_game_alive, terminate_java};
use mango_core::Session;
use mango_minecraft::LaunchCommand;
use regex::Regex;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How long a live process counts as "running" when the log never says so.
pub const RUNNING_FALLBACK: Duration = Duration::from_secs(8);
/// Liveness polling of adopted sessions.
pub const ADOPT_POLL: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    Running,
    Stopped,
    Crashed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitInfo {
    /// `None` when killed by a signal or when the code is unknown (adopted).
    pub code: Option<i32>,
    pub state: GameState,
    /// Play time to persist; `None` when it was already flushed.
    pub played_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameEvent {
    Log { line: String, level: LogLevel },
    Running,
    Exited(ExitInfo),
}

pub type EventCb = Arc<dyn Fn(GameEvent) + Send + Sync>;

pub struct SpawnOptions {
    pub command: LaunchCommand,
    pub log_file: PathBuf,
    pub profile_id: String,
    pub version_id: String,
    pub poll_interval: Duration,
    pub running_fallback: Duration,
    pub on_event: EventCb,
}

impl SpawnOptions {
    pub fn new(
        command: LaunchCommand,
        log_file: PathBuf,
        profile_id: impl Into<String>,
        version_id: impl Into<String>,
        on_event: EventCb,
    ) -> Self {
        Self {
            command,
            log_file,
            profile_id: profile_id.into(),
            version_id: version_id.into(),
            poll_interval: DEFAULT_POLL,
            running_fallback: RUNNING_FALLBACK,
            on_event,
        }
    }
}

pub struct AdoptOptions {
    pub profile_id: String,
    pub log_file: PathBuf,
    pub poll_interval: Duration,
    pub alive_interval: Duration,
    pub on_event: EventCb,
}

impl AdoptOptions {
    pub fn new(profile_id: impl Into<String>, log_file: PathBuf, on_event: EventCb) -> Self {
        Self {
            profile_id: profile_id.into(),
            log_file,
            poll_interval: DEFAULT_POLL,
            alive_interval: ADOPT_POLL,
            on_event,
        }
    }
}

struct Inner {
    profile_id: String,
    version_id: String,
    pid: u32,
    started_at_ms: u64,
    adopted: bool,
    log_file: PathBuf,
    running: AtomicBool,
    stop_requested: AtomicBool,
    flushed: AtomicBool,
    exit: Mutex<Option<ExitInfo>>,
    exit_cv: Condvar,
    on_event: EventCb,
    parser: Mutex<LogParser>,
    follower: Mutex<Option<LogFollower>>,
}

fn running_re() -> Option<&'static Regex> {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"Setting user:|LWJGL Version|Created:.*minecraft:textures|OpenAL initialized")
            .ok()
    })
    .as_ref()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

impl Inner {
    fn emit(&self, event: GameEvent) {
        (self.on_event)(event);
    }

    fn mark_running(&self) {
        if !self.running.swap(true, Ordering::SeqCst) {
            self.emit(GameEvent::Running);
        }
    }

    fn has_exited(&self) -> bool {
        self.exit.lock().map(|e| e.is_some()).unwrap_or(true)
    }

    fn feed_text(&self, text: &str) {
        let Ok(mut parser) = self.parser.lock() else {
            return;
        };
        parser.feed(text, &mut |line, level| self.line(line, level));
    }

    fn line(&self, line: &str, level: LogLevel) {
        self.emit(GameEvent::Log {
            line: line.to_string(),
            level,
        });
        if !self.running.load(Ordering::SeqCst) && running_re().is_some_and(|re| re.is_match(line))
        {
            self.mark_running();
        }
    }

    fn flush_play_time(&self) -> Option<u64> {
        if self.flushed.swap(true, Ordering::SeqCst) {
            return None;
        }
        Some(now_ms().saturating_sub(self.started_at_ms))
    }

    /// Common exit path: flush time, drain the log, announce.
    fn finish(&self, code: Option<i32>) {
        if self.has_exited() {
            return;
        }
        let played_ms = self.flush_play_time();
        let follower = self.follower.lock().ok().and_then(|mut f| f.take());
        if let Some(f) = follower {
            f.stop();
        }
        if let Ok(mut parser) = self.parser.lock() {
            parser.finish(&mut |line, level| self.line(line, level));
        }
        // An adopted pid is not our child, so its exit code is unknown.
        let clean = code == Some(0)
            || self.stop_requested.load(Ordering::SeqCst)
            || (self.adopted && code.is_none());
        let state = if clean {
            GameState::Stopped
        } else {
            GameState::Crashed
        };
        let text = match code {
            Some(c) => format!("Game exited with code {c}"),
            None => "Game exited".to_string(),
        };
        self.emit(GameEvent::Log {
            line: text,
            level: if clean {
                LogLevel::Info
            } else {
                LogLevel::Error
            },
        });
        let info = ExitInfo {
            code,
            state,
            played_ms,
        };
        if let Ok(mut slot) = self.exit.lock() {
            *slot = Some(info.clone());
        }
        self.exit_cv.notify_all();
        self.emit(GameEvent::Exited(info));
    }

    fn start_follower(self: &Arc<Self>, interval: Duration, start: FollowStart) {
        let me = Arc::clone(self);
        let follower = LogFollower::start(&self.log_file, interval, start, move |text| {
            me.feed_text(text);
        });
        if let Ok(mut slot) = self.follower.lock() {
            *slot = Some(follower);
        }
    }
}

/// Handle to a game process. Cheap to clone.
#[derive(Clone)]
pub struct GameInstance {
    inner: Arc<Inner>,
}

/// `logs/<slug>-<id8>.log`: a name a human can pick out of the folder.
pub fn log_file_for(logs_dir: &Path, profile_name: &str, profile_id: &str) -> PathBuf {
    let mut slug = String::new();
    let mut dash = false;
    for c in profile_name.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if dash && !slug.is_empty() {
                slug.push('-');
            }
            dash = false;
            slug.push(c);
        } else {
            dash = true;
        }
    }
    if slug.is_empty() {
        slug.push_str("instance");
    }
    let id8: String = profile_id.chars().take(8).collect();
    logs_dir.join(format!("{slug}-{id8}.log"))
}

/// Is the recorded session still a live Java process?
pub fn is_session_alive(session: &Session) -> bool {
    is_game_alive(session.pid)
}

fn build_command(cmd: &LaunchCommand, out: File, err: File) -> Command {
    let mut c = Command::new(&cmd.program);
    c.args(&cmd.args)
        .current_dir(&cmd.cwd)
        .envs(cmd.env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Own process group: survives the launcher's terminal and signals.
        c.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW);
    }
    c
}

impl GameInstance {
    /// Spawn the game detached, logging to `opts.log_file` (truncated).
    pub fn spawn(opts: SpawnOptions) -> Result<GameInstance> {
        if let Some(dir) = opts.log_file.parent() {
            std::fs::create_dir_all(dir).map_err(|source| RuntimeError::LogFile {
                path: dir.to_path_buf(),
                source,
            })?;
        }
        let log_err = |source| RuntimeError::LogFile {
            path: opts.log_file.clone(),
            source,
        };
        // One file for both streams, so stdout and stderr keep their order.
        let out = File::create(&opts.log_file).map_err(log_err)?;
        let err = out.try_clone().map_err(log_err)?;
        let mut command = build_command(&opts.command, out, err);
        let mut attempts = 0;
        let mut child = loop {
            match command.spawn() {
                Ok(c) => break c,
                // ETXTBSY: a just-written executable may still be held open.
                Err(e) if e.raw_os_error() == Some(26) && attempts < 5 => {
                    attempts += 1;
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(source) => {
                    return Err(RuntimeError::Spawn {
                        program: opts.command.program.clone(),
                        source,
                    })
                }
            }
        };
        drop(command);
        let inner = Arc::new(Inner {
            profile_id: opts.profile_id,
            version_id: opts.version_id,
            pid: child.id(),
            started_at_ms: now_ms(),
            adopted: false,
            log_file: opts.log_file,
            running: AtomicBool::new(false),
            stop_requested: AtomicBool::new(false),
            flushed: AtomicBool::new(false),
            exit: Mutex::new(None),
            exit_cv: Condvar::new(),
            on_event: opts.on_event,
            parser: Mutex::new(LogParser::new()),
            follower: Mutex::new(None),
        });
        inner.start_follower(opts.poll_interval, FollowStart::Beginning);

        let waiter = Arc::clone(&inner);
        std::thread::spawn(move || {
            let code = child.wait().ok().and_then(|s| s.code());
            waiter.finish(code);
        });

        // Nothing in the log confirms the window for every version, so assume
        // success when the process is still alive shortly after the start.
        let fallback = Arc::clone(&inner);
        let limit = opts.running_fallback;
        std::thread::spawn(move || {
            let begun = Instant::now();
            while begun.elapsed() < limit {
                if fallback.has_exited() || fallback.running.load(Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            if !fallback.has_exited() {
                fallback.mark_running();
            }
        });
        Ok(GameInstance { inner })
    }

    /// Re-attach to a session recorded before a launcher restart. `None` when
    /// the pid is gone or is no longer a Java process.
    pub fn adopt(session: &Session, opts: AdoptOptions) -> Option<GameInstance> {
        if !is_game_alive(session.pid) {
            return None;
        }
        let inner = Arc::new(Inner {
            profile_id: opts.profile_id,
            version_id: session.version_id.clone(),
            pid: session.pid,
            started_at_ms: session.started_at,
            adopted: true,
            log_file: opts.log_file,
            running: AtomicBool::new(true),
            stop_requested: AtomicBool::new(false),
            flushed: AtomicBool::new(false),
            exit: Mutex::new(None),
            exit_cv: Condvar::new(),
            on_event: opts.on_event,
            parser: Mutex::new(LogParser::new()),
            follower: Mutex::new(None),
        });
        inner.start_follower(opts.poll_interval, FollowStart::End);
        let watcher = Arc::clone(&inner);
        let slow = opts.alive_interval;
        std::thread::spawn(move || {
            let mut since = Instant::now();
            loop {
                std::thread::sleep(Duration::from_millis(25));
                if watcher.has_exited() {
                    return;
                }
                // After a stop request check quickly instead of every 10s.
                let every = if watcher.stop_requested.load(Ordering::SeqCst) {
                    Duration::from_millis(250).min(slow)
                } else {
                    slow
                };
                if since.elapsed() < every {
                    continue;
                }
                since = Instant::now();
                if !is_game_alive(watcher.pid) {
                    // The exit code of a process that is not our child is unknown.
                    watcher.finish(None);
                    return;
                }
            }
        });
        Some(GameInstance { inner })
    }

    pub fn pid(&self) -> u32 {
        self.inner.pid
    }
    pub fn profile_id(&self) -> &str {
        &self.inner.profile_id
    }
    pub fn version_id(&self) -> &str {
        &self.inner.version_id
    }
    pub fn log_file(&self) -> &Path {
        &self.inner.log_file
    }
    pub fn started_at_ms(&self) -> u64 {
        self.inner.started_at_ms
    }
    /// `true` once the window was detected (or the fallback fired).
    pub fn is_running(&self) -> bool {
        self.inner.running.load(Ordering::SeqCst) && !self.has_exited()
    }
    pub fn has_exited(&self) -> bool {
        self.inner.has_exited()
    }

    /// The record the app persists in `profile.session`.
    pub fn session(&self) -> Session {
        Session {
            pid: self.inner.pid,
            version_id: self.inner.version_id.clone(),
            started_at: self.inner.started_at_ms,
        }
    }

    /// Ask the game to terminate. Refuses a pid that is not verifiably Java.
    pub fn stop(&self) -> Result<()> {
        // Set the flag first: the exit watcher can observe the exit before
        // this method returns, and a requested stop must never read as a crash.
        self.inner.stop_requested.store(true, Ordering::SeqCst);
        if let Err(e) = terminate_java(self.inner.pid) {
            self.inner.stop_requested.store(false, Ordering::SeqCst);
            return Err(e);
        }
        Ok(())
    }

    /// Persist-once play time in ms: `Some` the first call only. The shutdown
    /// path calls this directly, so the exit handler cannot count it twice.
    pub fn flush_play_time(&self) -> Option<u64> {
        self.inner.flush_play_time()
    }

    /// Block until the game has exited (or the timeout passes).
    pub fn wait_exit(&self, timeout: Option<Duration>) -> Option<ExitInfo> {
        let guard = self.inner.exit.lock().ok()?;
        match timeout {
            Some(t) => {
                let (guard, _) = self
                    .inner
                    .exit_cv
                    .wait_timeout_while(guard, t, |e| e.is_none())
                    .ok()?;
                guard.clone()
            }
            None => self
                .inner
                .exit_cv
                .wait_while(guard, |e| e.is_none())
                .ok()?
                .clone(),
        }
    }
}
