//! Polling log follower.
//!
//! The game writes to a file instead of pipes so that closing the launcher
//! cannot break its stdout; the price is that the live view polls.

use crate::logparse::Utf8Chunker;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// Largest chunk read per poll.
pub const MAX_READ_BYTES: u64 = 256 * 1024;
pub const DEFAULT_POLL: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowStart {
    Beginning,
    /// Skip what is already in the file (adopted sessions).
    End,
}

struct Pump {
    path: PathBuf,
    offset: u64,
    decoder: Utf8Chunker,
}

impl Pump {
    /// Read at most one chunk; returns decoded text when there was news.
    fn step(&mut self) -> Option<String> {
        let size = std::fs::metadata(&self.path).ok()?.len();
        if size < self.offset {
            // A new session truncated the file.
            self.offset = 0;
            self.decoder.reset();
        }
        if size == self.offset {
            return None;
        }
        let mut file = File::open(&self.path).ok()?;
        file.seek(SeekFrom::Start(self.offset)).ok()?;
        let want = (size - self.offset).min(MAX_READ_BYTES);
        let mut buf = Vec::new();
        file.take(want).read_to_end(&mut buf).ok()?;
        self.offset += buf.len() as u64;
        let text = self.decoder.push(&buf);
        (!text.is_empty()).then_some(text)
    }

    /// Drain everything currently in the file.
    fn drain(&mut self, on_text: &mut dyn FnMut(&str)) {
        while let Some(text) = self.step() {
            on_text(&text);
        }
    }
}

struct Shared {
    stop: Mutex<bool>,
    cv: Condvar,
}

/// Follows a growing file and hands decoded text to a callback.
pub struct LogFollower {
    shared: Arc<Shared>,
    handle: Option<JoinHandle<()>>,
}

impl LogFollower {
    pub fn start<F>(path: &Path, interval: Duration, start: FollowStart, mut on_text: F) -> Self
    where
        F: FnMut(&str) + Send + 'static,
    {
        let offset = match start {
            FollowStart::Beginning => 0,
            FollowStart::End => std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        };
        let mut pump = Pump {
            path: path.to_path_buf(),
            offset,
            decoder: Utf8Chunker::new(),
        };
        let shared = Arc::new(Shared {
            stop: Mutex::new(false),
            cv: Condvar::new(),
        });
        let thread_shared = Arc::clone(&shared);
        let handle = std::thread::spawn(move || loop {
            // One chunk per tick keeps a flood from monopolising the thread.
            if let Some(text) = pump.step() {
                on_text(&text);
            }
            let Ok(guard) = thread_shared.stop.lock() else {
                return;
            };
            let Ok((guard, _)) = thread_shared
                .cv
                .wait_timeout_while(guard, interval, |stop| !*stop)
            else {
                return;
            };
            if *guard {
                drop(guard);
                // Pick up the last lines the game wrote.
                pump.drain(&mut on_text);
                return;
            }
        });
        LogFollower {
            shared,
            handle: Some(handle),
        }
    }

    /// Read whatever is left, then stop following.
    pub fn stop(mut self) {
        self.stop_inner();
    }

    fn stop_inner(&mut self) {
        if let Ok(mut s) = self.shared.stop.lock() {
            *s = true;
        }
        self.shared.cv.notify_all();
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for LogFollower {
    fn drop(&mut self) {
        self.stop_inner();
    }
}
