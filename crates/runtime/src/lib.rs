//! Game process runtime: detached spawn, log following, state detection,
//! stop and adoption of live sessions. Port of the process half of
//! `src/main/launcher.js`. No Tauri dependency.
//!
//! The game outlives the launcher: stdio goes to a log file that is polled,
//! never to pipes, and the child gets its own process group (unix) or
//! `DETACHED_PROCESS | CREATE_NO_WINDOW` (windows).

mod error;
mod follower;
mod instance;
mod logparse;
mod process;

#[cfg(test)]
mod tests;

pub use error::{Result, RuntimeError};
pub use follower::{FollowStart, LogFollower, DEFAULT_POLL, MAX_READ_BYTES};
pub use instance::{
    is_session_alive, log_file_for, AdoptOptions, EventCb, ExitInfo, GameEvent, GameInstance,
    GameState, SpawnOptions, ADOPT_POLL, RUNNING_FALLBACK, STOP_GRACE,
};
pub use logparse::{LogLevel, LogParser, Utf8Chunker};
pub use process::{
    is_game_alive, is_java_name, process_start_ms, start_time_matches, terminate_java,
    terminate_java_started, START_TOLERANCE_MS,
};
