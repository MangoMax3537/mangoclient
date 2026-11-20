//! Window-control commands of the custom title bar.
//!
//! They act on the main window, so only the Tauri shell can implement them
//! (it accepts calls from the `main` webview only). The dev server and the
//! CLI answer `window.unsupported`.

use crate::error::{AppError, Res};
use serde::Deserialize;
use ts_rs::TS;

/// The edge or corner a resize drag starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum ResizeDirection {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

impl ResizeDirection {
    /// The wire spelling (`northeast`, ...).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::North => "north",
            Self::South => "south",
            Self::East => "east",
            Self::West => "west",
            Self::NorthEast => "northeast",
            Self::NorthWest => "northwest",
            Self::SouthEast => "southeast",
            Self::SouthWest => "southwest",
        }
    }
}

/// What every window command answers outside the desktop shell.
pub fn window_unsupported<T>() -> Res<T> {
    Err(AppError::new(
        "window.unsupported",
        "Window controls are only available in the desktop app",
    ))
}
