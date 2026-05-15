//! Read-only SQLite accessor for ActivityWatch's event store.
//!
//! Background: both aw-server-rust and the older aw-server (Python)
//! persist events to a local SQLite file. The HTTP /query/ AQL endpoint
//! serializes all evaluation on a per-server lock, which capped daylog's
//! cold-start at 30-50s on multi-day windows. We bypass HTTP and read
//! the SQLite file directly.
//!
//! Both server flavors are supported because daylog's installer only
//! provisions aw-server-rust on fresh machines, but plenty of users
//! (including the one this work was prompted by) already have aw-server
//! Python running from an older ActivityWatch desktop install. Daylog's
//! wizard skips its bootstrap when something is already listening on
//! `:5600`, so either flavor can be the live one.
//!
//! Paths (Linux, via `dirs::data_dir()`):
//! - Rust:   `~/.local/share/activitywatch/aw-server-rust/sqlite.db`
//! - Python: `~/.local/share/activitywatch/aw-server/peewee-sqlite.v2.db`
//!
//! Detection: whichever file exists wins; Rust takes precedence if both
//! are present (matches daylog's stated installation direction).
//!
//! Locking: neither flavor uses WAL — both run in DELETE journal mode
//! with single-writer transactions. Our handle sets `busy_timeout=5s` so
//! a commit window blocks us briefly instead of failing immediately.
//! `query_only=ON` is belt-and-suspenders next to `SQLITE_OPEN_READ_ONLY`.
//!
//! Schemas:
//! - Rust (`user_version=4`): `events(id, bucketrow, starttime INTEGER,
//!   endtime INTEGER, data TEXT)`. starttime/endtime are nanosecond Unix
//!   timestamps as i64. `buckets(id, name, ...)`.
//! - Python (peewee, no `user_version`): `eventmodel(id, bucket_id,
//!   timestamp DATETIME, duration DECIMAL, datastr VARCHAR)`. timestamp
//!   is ISO 8601 TEXT formatted as `"YYYY-MM-DD HH:MM:SS.ffffff+00:00"`.
//!   `bucketmodel(key, id, ...)` — `key` is the FK target, `id` is the
//!   bucket name. Both schemas store the per-event payload as a JSON
//!   string with the same shape.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Duration as StdDuration;

use chrono::{DateTime, NaiveDateTime, Utc};
use rusqlite::{params, Connection, OpenFlags};
use serde_json::Value;

use crate::aw_client::Event;

#[derive(Debug, thiserror::Error)]
pub enum DatastoreError {
    #[error("no aw-server SQLite file found (looked under dirs::data_dir())")]
    NotFound,
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("malformed row: {0}")]
    Parse(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schema {
    /// aw-server-rust (`sqlite.db`, nanosecond INTEGER timestamps).
    Rust,
    /// aw-server (Python/peewee, `peewee-sqlite.v2.db`, ISO TEXT timestamps).
    Python,
}

/// Resolve the live aw-server SQLite file and its schema flavor. Rust
/// wins if both files exist. Returns `None` if no platform data dir is
/// available or neither file is present.
pub fn detect_db() -> Option<(PathBuf, Schema)> {
    let aw = dirs::data_dir()?.join("activitywatch");
    let rust = aw.join("aw-server-rust").join("sqlite.db");
    if rust.exists() {
        return Some((rust, Schema::Rust));
    }
    let python = aw.join("aw-server").join("peewee-sqlite.v2.db");
    if python.exists() {
        return Some((python, Schema::Python));
    }
    None
}

fn open_conn() -> Result<(Connection, Schema), DatastoreError> {
    let (path, schema) = detect_db().ok_or(DatastoreError::NotFound)?;
    // NO_MUTEX keeps the SQLite handle itself lock-free; our outer
    // Mutex<Connection> serializes Rust-side access.
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(&path, flags)?;
    conn.busy_timeout(StdDuration::from_secs(5))?;
    conn.pragma_update(None, "query_only", true)?;
    Ok((conn, schema))
}

struct Shared {
    conn: Mutex<Connection>,
    schema: Schema,
}

/// Process-wide shared connection. First caller opens; failures are not
/// memoized so a still-installing aw-server eventually succeeds.
fn with_conn<F, T>(f: F) -> Result<T, DatastoreError>
where
    F: FnOnce(&Connection, Schema) -> Result<T, DatastoreError>,
{
    static SLOT: OnceLock<Shared> = OnceLock::new();
    if SLOT.get().is_none() {
        let (conn, schema) = open_conn()?;
        let _ = SLOT.set(Shared {
            conn: Mutex::new(conn),
            schema,
        });
    }
    let shared = SLOT.get().expect("init above");
    let guard = shared.conn.lock().expect("conn mutex poisoned");
    f(&guard, shared.schema)
}

/// PostgreSQL-flavored format string that matches peewee's serialized
/// `DATETIME` form: `YYYY-MM-DD HH:MM:SS.ffffff+00:00`. We bind range
/// bounds in this format so lex comparison works against stored values.
const PY_TS_FMT: &str = "%Y-%m-%d %H:%M:%S%.6f+00:00";

/// All events whose `[timestamp, timestamp+duration)` overlaps
/// `[start, end]`, across every bucket whose name starts with
/// `bucket_prefix`. Times are clipped to the query window so callers can
/// sum `duration` without overcounting at the edges. Returned events
/// are sorted ascending by timestamp.
pub fn events_in_range(
    bucket_prefix: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<Event>, DatastoreError> {
    if start > end {
        return Ok(Vec::new());
    }
    with_conn(|conn, schema| match schema {
        Schema::Rust => events_in_range_rust(conn, bucket_prefix, start, end),
        Schema::Python => events_in_range_python(conn, bucket_prefix, start, end),
    })
}

/// True if at least one bucket name starts with `prefix`. Replaces the
/// HTTP-based `bucket_prefix_present` check.
pub fn bucket_exists_for_prefix(prefix: &str) -> Result<bool, DatastoreError> {
    with_conn(|conn, schema| match schema {
        Schema::Rust => {
            let like = format!("{prefix}%");
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM buckets WHERE name LIKE ?1",
                params![like],
                |row| row.get(0),
            )?;
            Ok(n > 0)
        }
        Schema::Python => {
            let like = format!("{prefix}%");
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM bucketmodel WHERE id LIKE ?1",
                params![like],
                |row| row.get(0),
            )?;
            Ok(n > 0)
        }
    })
}

fn events_in_range_rust(
    conn: &Connection,
    bucket_prefix: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<Event>, DatastoreError> {
    let start_ns = start
        .timestamp_nanos_opt()
        .ok_or_else(|| DatastoreError::Parse("start out of i64 range".into()))?;
    let end_ns = end
        .timestamp_nanos_opt()
        .ok_or_else(|| DatastoreError::Parse("end out of i64 range".into()))?;
    let like = format!("{bucket_prefix}%");
    let bucket_ids: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT id FROM buckets WHERE name LIKE ?1")?;
        let ids: Vec<i64> = stmt
            .query_map(params![like], |row| row.get::<_, i64>(0))?
            .filter_map(|r| r.ok())
            .collect();
        ids
    };
    if bucket_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(
        "SELECT id, starttime, endtime, data
         FROM events
         WHERE bucketrow = ?1 AND endtime >= ?2 AND starttime <= ?3
         ORDER BY starttime ASC",
    )?;
    let mut events: Vec<Event> = Vec::new();
    for bucket_id in bucket_ids {
        let rows = stmt.query_map(params![bucket_id, start_ns, end_ns], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        for r in rows {
            let (id, mut st_ns, mut et_ns, data_str) = r?;
            if st_ns < start_ns {
                st_ns = start_ns;
            }
            if et_ns > end_ns {
                et_ns = end_ns;
            }
            let timestamp = DateTime::from_timestamp(
                st_ns.div_euclid(1_000_000_000),
                st_ns.rem_euclid(1_000_000_000) as u32,
            )
            .ok_or_else(|| DatastoreError::Parse("starttime out of chrono range".into()))?;
            let duration_secs = (et_ns - st_ns) as f64 / 1_000_000_000.0;
            let data: Value = serde_json::from_str(&data_str)
                .map_err(|e| DatastoreError::Parse(format!("event data: {e}")))?;
            events.push(Event {
                id: Some(id as u64),
                timestamp,
                duration: duration_secs,
                data,
            });
        }
    }
    events.sort_by_key(|e| e.timestamp);
    Ok(events)
}

fn events_in_range_python(
    conn: &Connection,
    bucket_prefix: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<Event>, DatastoreError> {
    let like = format!("{bucket_prefix}%");
    let bucket_keys: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT key FROM bucketmodel WHERE id LIKE ?1")?;
        let keys: Vec<i64> = stmt
            .query_map(params![like], |row| row.get::<_, i64>(0))?
            .filter_map(|r| r.ok())
            .collect();
        keys
    };
    if bucket_keys.is_empty() {
        return Ok(Vec::new());
    }
    // Overlap predicate: `timestamp <= end AND timestamp + duration*86400 >= start`
    // expressed via `julianday()` so SQLite handles the DATETIME-plus-
    // fractional-seconds arithmetic. The TEXT-typed timestamp on the
    // LHS uses the indexed lex comparison; julianday() runs per row but
    // only after the index has pruned.
    let start_str = start.format(PY_TS_FMT).to_string();
    let end_str = end.format(PY_TS_FMT).to_string();
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, duration, datastr
         FROM eventmodel
         WHERE bucket_id = ?1
           AND timestamp <= ?2
           AND julianday(timestamp) + (duration / 86400.0) >= julianday(?3)
         ORDER BY timestamp ASC",
    )?;
    let mut events: Vec<Event> = Vec::new();
    for bucket_key in bucket_keys {
        let rows = stmt.query_map(
            params![bucket_key, &end_str, &start_str],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, f64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )?;
        for r in rows {
            let (id, ts_str, duration_secs, data_str) = r?;
            let mut timestamp = parse_python_ts(&ts_str)?;
            let mut duration = duration_secs;
            // Clip to query window so duration sums are right at edges.
            let event_end = timestamp + chrono::Duration::nanoseconds(
                (duration * 1_000_000_000.0) as i64,
            );
            if timestamp < start {
                duration -= (start - timestamp).num_milliseconds() as f64 / 1000.0;
                timestamp = start;
            }
            if event_end > end {
                duration -= (event_end - end).num_milliseconds() as f64 / 1000.0;
            }
            if duration < 0.0 {
                duration = 0.0;
            }
            let data: Value = serde_json::from_str(&data_str)
                .map_err(|e| DatastoreError::Parse(format!("event datastr: {e}")))?;
            events.push(Event {
                id: Some(id as u64),
                timestamp,
                duration,
                data,
            });
        }
    }
    events.sort_by_key(|e| e.timestamp);
    Ok(events)
}

fn parse_python_ts(s: &str) -> Result<DateTime<Utc>, DatastoreError> {
    // peewee writes UTC; rows may or may not carry the `+00:00` suffix.
    // Strip the suffix (if present) and parse the naive form, then
    // re-attach UTC. `%.f` accepts any fractional-second precision.
    let trimmed = s.strip_suffix("+00:00").unwrap_or(s);
    let naive = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S%.f")
        .map_err(|_| DatastoreError::Parse(format!("unrecognized timestamp format: {s}")))?;
    Ok(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn rust_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE buckets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT UNIQUE NOT NULL,
                type TEXT NOT NULL,
                client TEXT NOT NULL,
                hostname TEXT NOT NULL,
                created TEXT NOT NULL,
                data TEXT NOT NULL DEFAULT '{}'
            );
            CREATE TABLE events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                bucketrow INTEGER NOT NULL,
                starttime INTEGER NOT NULL,
                endtime INTEGER NOT NULL,
                data TEXT NOT NULL,
                FOREIGN KEY (bucketrow) REFERENCES buckets(id)
            );
            ",
        )
        .unwrap();
        conn
    }

    fn python_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE bucketmodel (
                key INTEGER NOT NULL PRIMARY KEY,
                id VARCHAR(255) NOT NULL,
                created DATETIME NOT NULL,
                name VARCHAR(255),
                type VARCHAR(255) NOT NULL,
                client VARCHAR(255) NOT NULL,
                hostname VARCHAR(255) NOT NULL,
                datastr VARCHAR(255)
            );
            CREATE TABLE eventmodel (
                id INTEGER NOT NULL PRIMARY KEY,
                bucket_id INTEGER NOT NULL,
                timestamp DATETIME NOT NULL,
                duration DECIMAL(10, 5) NOT NULL,
                datastr VARCHAR(255) NOT NULL,
                FOREIGN KEY (bucket_id) REFERENCES bucketmodel(key)
            );
            ",
        )
        .unwrap();
        conn
    }

    fn seed_rust(conn: &Connection, name: &str) -> i64 {
        conn.execute(
            "INSERT INTO buckets (name, type, client, hostname, created)
             VALUES (?1, 'currentwindow', 'aw-watcher-window', 'h', '2026-01-01T00:00:00Z')",
            params![name],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn seed_rust_event(conn: &Connection, bucketrow: i64, st_ns: i64, et_ns: i64, data: &str) {
        conn.execute(
            "INSERT INTO events (bucketrow, starttime, endtime, data) VALUES (?1, ?2, ?3, ?4)",
            params![bucketrow, st_ns, et_ns, data],
        )
        .unwrap();
    }

    fn seed_python(conn: &Connection, name: &str) -> i64 {
        conn.execute(
            "INSERT INTO bucketmodel (id, created, type, client, hostname)
             VALUES (?1, '2026-01-01 00:00:00.000000+00:00', 'currentwindow', 'aw-watcher-window', 'h')",
            params![name],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn seed_python_event(
        conn: &Connection,
        bucket_key: i64,
        ts: DateTime<Utc>,
        duration_secs: f64,
        data: &str,
    ) {
        conn.execute(
            "INSERT INTO eventmodel (bucket_id, timestamp, duration, datastr)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                bucket_key,
                ts.format(PY_TS_FMT).to_string(),
                duration_secs,
                data
            ],
        )
        .unwrap();
    }

    fn day(year: i32, month: u32, day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, 0, 0, 0).unwrap()
    }

    // --- Rust-schema tests ---

    #[test]
    fn rust_empty_when_no_buckets_match_prefix() {
        let conn = rust_conn();
        seed_rust(&conn, "aw-watcher-web-brave_host");
        let got = events_in_range_rust(
            &conn,
            "aw-watcher-window_",
            day(2026, 5, 1),
            day(2026, 5, 8),
        )
        .unwrap();
        assert!(got.is_empty());
    }

    #[test]
    fn rust_events_clipped_to_query_range() {
        let conn = rust_conn();
        let bucket = seed_rust(&conn, "aw-watcher-window_host");
        let window_start = day(2026, 5, 1);
        let window_end = day(2026, 5, 2);
        let st_ns = window_start.timestamp_nanos_opt().unwrap() - 3_600 * 1_000_000_000;
        let et_ns = window_end.timestamp_nanos_opt().unwrap() + 1_800 * 1_000_000_000;
        seed_rust_event(&conn, bucket, st_ns, et_ns, r#"{"app":"brave"}"#);

        let got =
            events_in_range_rust(&conn, "aw-watcher-window_", window_start, window_end).unwrap();
        assert_eq!(got.len(), 1);
        let expected_secs = (window_end - window_start).num_seconds() as f64;
        assert!(
            (got[0].duration - expected_secs).abs() < 1e-6,
            "clipped duration must equal window length, got {} expected {}",
            got[0].duration,
            expected_secs
        );
        assert_eq!(
            got[0].data.get("app").and_then(|v| v.as_str()),
            Some("brave")
        );
    }

    #[test]
    fn rust_multibucket_merge_is_sorted() {
        let conn = rust_conn();
        let b1 = seed_rust(&conn, "aw-watcher-web-brave_h");
        let b2 = seed_rust(&conn, "aw-watcher-web-firefox_h");
        let base = day(2026, 5, 1).timestamp_nanos_opt().unwrap();
        seed_rust_event(
            &conn,
            b1,
            base + 200 * 1_000_000_000,
            base + 300 * 1_000_000_000,
            r#"{"url":"https://a.com"}"#,
        );
        seed_rust_event(
            &conn,
            b2,
            base + 100 * 1_000_000_000,
            base + 150 * 1_000_000_000,
            r#"{"url":"https://b.com"}"#,
        );

        let got =
            events_in_range_rust(&conn, "aw-watcher-web-", day(2026, 5, 1), day(2026, 5, 2))
                .unwrap();
        assert_eq!(got.len(), 2);
        assert!(got[0].timestamp < got[1].timestamp);
    }

    // --- Python-schema tests ---

    #[test]
    fn python_empty_when_no_buckets_match_prefix() {
        let conn = python_conn();
        seed_python(&conn, "aw-watcher-web-brave_host");
        let got = events_in_range_python(
            &conn,
            "aw-watcher-window_",
            day(2026, 5, 1),
            day(2026, 5, 8),
        )
        .unwrap();
        assert!(got.is_empty());
    }

    #[test]
    fn python_events_clipped_to_query_range() {
        let conn = python_conn();
        let bucket = seed_python(&conn, "aw-watcher-window_host");
        let window_start = day(2026, 5, 1);
        let window_end = day(2026, 5, 2);
        let event_start = window_start - chrono::Duration::hours(1);
        // duration runs 1h before window_start through 30min after window_end:
        // total length = 1h + 24h + 30min = 25.5h.
        let total_duration_secs = (window_end + chrono::Duration::minutes(30) - event_start)
            .num_seconds() as f64;
        seed_python_event(
            &conn,
            bucket,
            event_start,
            total_duration_secs,
            r#"{"app":"brave"}"#,
        );

        let got =
            events_in_range_python(&conn, "aw-watcher-window_", window_start, window_end).unwrap();
        assert_eq!(got.len(), 1);
        let expected_secs = (window_end - window_start).num_seconds() as f64;
        assert!(
            (got[0].duration - expected_secs).abs() < 1.0,
            "clipped duration must equal window length, got {} expected {}",
            got[0].duration,
            expected_secs
        );
        assert_eq!(got[0].timestamp, window_start, "timestamp clipped to window start");
        assert_eq!(
            got[0].data.get("app").and_then(|v| v.as_str()),
            Some("brave")
        );
    }

    #[test]
    fn python_multibucket_merge_is_sorted() {
        let conn = python_conn();
        let b1 = seed_python(&conn, "aw-watcher-web-brave_h");
        let b2 = seed_python(&conn, "aw-watcher-web-firefox_h");
        let base = day(2026, 5, 1);
        seed_python_event(
            &conn,
            b1,
            base + chrono::Duration::seconds(200),
            100.0,
            r#"{"url":"https://a.com"}"#,
        );
        seed_python_event(
            &conn,
            b2,
            base + chrono::Duration::seconds(100),
            50.0,
            r#"{"url":"https://b.com"}"#,
        );

        let got = events_in_range_python(
            &conn,
            "aw-watcher-web-",
            day(2026, 5, 1),
            day(2026, 5, 2),
        )
        .unwrap();
        assert_eq!(got.len(), 2);
        assert!(got[0].timestamp < got[1].timestamp);
    }

    #[test]
    fn parse_python_ts_handles_offset_and_naive() {
        // Both representations must collapse to the same UTC instant.
        let with_off = parse_python_ts("2026-05-15 06:23:09.227000+00:00").unwrap();
        let naive = parse_python_ts("2026-05-15 06:23:09.227000").unwrap();
        assert_eq!(with_off, naive);
        assert_eq!(with_off.timestamp_subsec_millis(), 227);
    }

    #[test]
    fn python_event_starting_before_window_is_clipped_in_duration() {
        // Event runs from 12:00 to 14:00. Window is 13:00 to 15:00.
        // Expected clipped duration: 1h (from 13:00 to 14:00).
        let conn = python_conn();
        let bucket = seed_python(&conn, "aw-watcher-window_h");
        let ts = day(2026, 5, 1) + chrono::Duration::hours(12);
        seed_python_event(&conn, bucket, ts, 7200.0, r#"{"app":"x"}"#);
        let got = events_in_range_python(
            &conn,
            "aw-watcher-window_",
            day(2026, 5, 1) + chrono::Duration::hours(13),
            day(2026, 5, 1) + chrono::Duration::hours(15),
        )
        .unwrap();
        assert_eq!(got.len(), 1);
        assert!(
            (got[0].duration - 3600.0).abs() < 1.0,
            "expected 1h clipped duration, got {}",
            got[0].duration
        );
    }
}
