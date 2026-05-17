//! Pure-Rust data layer for daylog. Reads aw-server-rust's SQLite file
//! directly, runs the AQL-equivalent transforms in-process, and emits
//! the typed rows the TUI consumes. Previously a separate `daylog-core`
//! crate; collapsed into `daylog-tui` at v0.2.0 so a single
//! `version = "X.Y.Z"` stamp covers the whole project.

pub mod aggregate;
pub mod aw_client;
pub mod categories;
pub mod datastore;
pub mod kpi;
pub mod paths;
pub mod queries;
pub mod snapshot;
pub mod time;
pub mod transforms;

// Flat re-exports so consumers write `use crate::data::{TimeRange,
// AwClient, KpiSummary};` without naming the inner module. Verified to
// have no name collisions across the 10 modules at the time of merge —
// if you add a new item, run `cargo check` and resolve any duplicates by
// dropping the offending name from the glob and importing via its
// module path.
pub use self::aggregate::*;
pub use self::aw_client::*;
pub use self::categories::*;
pub use self::datastore::*;
pub use self::kpi::*;
pub use self::paths::*;
pub use self::queries::*;
pub use self::snapshot::*;
pub use self::time::*;
pub use self::transforms::*;
