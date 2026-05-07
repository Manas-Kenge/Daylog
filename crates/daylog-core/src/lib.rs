//! Daylog's shared data layer.
//!
//! Pure Rust — no Tauri, no Wry, no WebKit. Both the Tauri desktop app
//! (`src-tauri`) and future TUI binary (`crates/daylog-tui`) depend on
//! this crate. Single source of truth for talking to aw-server,
//! aggregating events, resolving categories, and modeling time ranges.
//!
//! Anything Tauri-coupled (lifecycle, tracking install, freedesktop
//! icons, app handle resolution) stays in `src-tauri`.

pub mod aggregate;
pub mod aw_client;
pub mod categories;
pub mod paths;
pub mod queries;
pub mod time;
