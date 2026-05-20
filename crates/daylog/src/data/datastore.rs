use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Duration as StdDuration;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OpenFlags};
use serde_json::Value;

use crate::data::aw_client::Event;

#[derive(Debug, thiserror::Error)]
pub enum DatastoreError {
    #[error("aw-server-rust sqlite.db not found at {0}")]
    NotFound(PathBuf),
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("malformed row: {0}")]
    Parse(String),
}

pub fn db_path() -> Option<PathBuf> {
    Some(
        dirs::data_dir()?
            .join("activitywatch")
            .join("aw-server-rust")
            .join("sqlite.db"),
    )
}

fn open_conn() -> Result<Connection, DatastoreError> {
    let path = db_path()
        .ok_or_else(|| DatastoreError::NotFound(PathBuf::from("<no platform data dir>")))?;
    if !path.exists() {
        return Err(DatastoreError::NotFound(path));
    }
    // NO_MUTEX keeps the SQLite handle itself lock-free; our outer
    // Mutex<Connection> serializes Rust-side access.
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(&path, flags)?;
    conn.busy_timeout(StdDuration::from_secs(5))?;
    conn.pragma_update(None, "query_only", true)?;
    Ok(conn)
}

/// Shared lazy-init connection; init failures not memoized.
fn with_conn<F, T>(f: F) -> Result<T, DatastoreError>
where
    F: FnOnce(&Connection) -> Result<T, DatastoreError>,
{
    static SLOT: OnceLock<Mutex<Connection>> = OnceLock::new();
    if SLOT.get().is_none() {
        let conn = open_conn()?;
        // Racy set is fine: loser's Connection is just dropped.
        let _ = SLOT.set(Mutex::new(conn));
    }
    let guard = SLOT
        .get()
        .expect("init above")
        .lock()
        .expect("conn mutex poisoned");
    f(&guard)
}

/// Cross-bucket events overlapping [start, end], clipped to the window,
/// sorted ASC by timestamp.
pub fn events_in_range(
    bucket_prefix: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<Event>, DatastoreError> {
    let start_ns = start
        .timestamp_nanos_opt()
        .ok_or_else(|| DatastoreError::Parse("start timestamp out of i64 range".into()))?;
    let end_ns = end
        .timestamp_nanos_opt()
        .ok_or_else(|| DatastoreError::Parse("end timestamp out of i64 range".into()))?;
    if start_ns > end_ns {
        return Ok(Vec::new());
    }

    with_conn(|conn| {
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

        let mut events: Vec<Event> = Vec::new();
        let mut stmt = conn.prepare(
            "SELECT id, starttime, endtime, data
             FROM events
             WHERE bucketrow = ?1 AND endtime >= ?2 AND starttime <= ?3
             ORDER BY starttime ASC",
        )?;
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
    })
}

pub fn bucket_exists_for_prefix(prefix: &str) -> Result<bool, DatastoreError> {
    with_conn(|conn| {
        let like = format!("{prefix}%");
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM buckets WHERE name LIKE ?1",
            params![like],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixture_conn() -> Connection {
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

    fn seed_bucket(conn: &Connection, name: &str) -> i64 {
        conn.execute(
            "INSERT INTO buckets (name, type, client, hostname, created)
             VALUES (?1, 'currentwindow', 'aw-watcher-window', 'h', '2026-01-01T00:00:00Z')",
            params![name],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn seed_event(conn: &Connection, bucketrow: i64, st_ns: i64, et_ns: i64, data: &str) {
        conn.execute(
            "INSERT INTO events (bucketrow, starttime, endtime, data) VALUES (?1, ?2, ?3, ?4)",
            params![bucketrow, st_ns, et_ns, data],
        )
        .unwrap();
    }

    fn read_events(
        conn: &Connection,
        prefix: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<Event> {
        let start_ns = start.timestamp_nanos_opt().unwrap();
        let end_ns = end.timestamp_nanos_opt().unwrap();
        let like = format!("{prefix}%");
        let mut stmt = conn
            .prepare("SELECT id FROM buckets WHERE name LIKE ?1")
            .unwrap();
        let bucket_ids: Vec<i64> = stmt
            .query_map(params![like], |row| row.get::<_, i64>(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        let mut out: Vec<Event> = Vec::new();
        let mut stmt = conn
            .prepare(
                "SELECT id, starttime, endtime, data FROM events
                 WHERE bucketrow = ?1 AND endtime >= ?2 AND starttime <= ?3
                 ORDER BY starttime ASC",
            )
            .unwrap();
        for bucket_id in bucket_ids {
            let rows = stmt
                .query_map(params![bucket_id, start_ns, end_ns], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })
                .unwrap();
            for r in rows {
                let (id, mut st_ns, mut et_ns, data_str) = r.unwrap();
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
                .unwrap();
                let duration_secs = (et_ns - st_ns) as f64 / 1_000_000_000.0;
                out.push(Event {
                    id: Some(id as u64),
                    timestamp,
                    duration: duration_secs,
                    data: serde_json::from_str(&data_str).unwrap(),
                });
            }
        }
        out.sort_by_key(|e| e.timestamp);
        out
    }

    fn day(year: i32, month: u32, day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, 0, 0, 0).unwrap()
    }

    #[test]
    fn empty_when_no_buckets_match_prefix() {
        let conn = fixture_conn();
        seed_bucket(&conn, "aw-watcher-web-brave_host");
        let got = read_events(
            &conn,
            "aw-watcher-window_",
            day(2026, 5, 1),
            day(2026, 5, 8),
        );
        assert!(got.is_empty());
    }

    #[test]
    fn events_clipped_to_query_range() {
        let conn = fixture_conn();
        let bucket = seed_bucket(&conn, "aw-watcher-window_host");
        let window_start = day(2026, 5, 1);
        let window_end = day(2026, 5, 2);
        let st_ns = window_start.timestamp_nanos_opt().unwrap() - 3_600 * 1_000_000_000;
        let et_ns = window_end.timestamp_nanos_opt().unwrap() + 1_800 * 1_000_000_000;
        seed_event(&conn, bucket, st_ns, et_ns, r#"{"app":"brave"}"#);

        let got = read_events(&conn, "aw-watcher-window_", window_start, window_end);
        assert_eq!(got.len(), 1);
        let expected_secs = (window_end - window_start).num_seconds() as f64;
        assert!(
            (got[0].duration - expected_secs).abs() < 1e-6,
            "clipped duration must equal the window length, got {} expected {}",
            got[0].duration,
            expected_secs
        );
        assert_eq!(
            got[0].data.get("app").and_then(|v| v.as_str()),
            Some("brave")
        );
    }

    #[test]
    fn events_from_multiple_prefixed_buckets_are_merged_and_sorted() {
        let conn = fixture_conn();
        let b1 = seed_bucket(&conn, "aw-watcher-web-brave_h");
        let b2 = seed_bucket(&conn, "aw-watcher-web-firefox_h");
        let base = day(2026, 5, 1).timestamp_nanos_opt().unwrap();
        seed_event(
            &conn,
            b1,
            base + 200 * 1_000_000_000,
            base + 300 * 1_000_000_000,
            r#"{"url":"https://a.com"}"#,
        );
        seed_event(
            &conn,
            b2,
            base + 100 * 1_000_000_000,
            base + 150 * 1_000_000_000,
            r#"{"url":"https://b.com"}"#,
        );

        let got = read_events(
            &conn,
            "aw-watcher-web-",
            day(2026, 5, 1),
            day(2026, 5, 2),
        );
        assert_eq!(got.len(), 2);
        assert!(got[0].timestamp < got[1].timestamp);
    }

    #[test]
    fn empty_when_window_is_before_any_event() {
        let conn = fixture_conn();
        let bucket = seed_bucket(&conn, "aw-watcher-window_host");
        let base = day(2026, 5, 10).timestamp_nanos_opt().unwrap();
        seed_event(&conn, bucket, base, base + 60 * 1_000_000_000, r#"{"app":"x"}"#);
        let got = read_events(
            &conn,
            "aw-watcher-window_",
            day(2026, 5, 1),
            day(2026, 5, 2),
        );
        assert!(got.is_empty(), "window before any event must return empty");
    }
}
