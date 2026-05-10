//! Daylog's shared data layer.
//!
//! Pure Rust — no Tauri, no Wry, no WebKit. The daylog TUI
//! (`crates/daylog`) depends on this crate as the single source of truth
//! for talking to aw-server, aggregating events, resolving categories,
//! and modeling time ranges.
//!
//! Anything machine-state-coupled (tracker install, systemd / XDG
//! autostart, GNOME extension, freedesktop icons) stays in the daylog
//! crate, not here.

pub mod aggregate;
pub mod aw_client;
pub mod categories;
pub mod kpi;
pub mod paths;
pub mod queries;
pub mod time;
