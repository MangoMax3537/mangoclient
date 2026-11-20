//! MangoConfig injection, install telemetry, URL/path security policy and
//! play statistics. No UI, no Tauri.
#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod error;
pub mod mangoconfig;
pub mod security;
pub mod stats;
pub mod telemetry;

pub use error::{Error, Result};
