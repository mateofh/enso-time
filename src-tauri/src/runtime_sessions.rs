//! Ensō's own uptime log. The scheduler records a tick every 30 s; consecutive ticks with
//! the same capture state are compacted into one row, so a day is a handful of rows.
//!
//! How each row ends explains the gap that follows it:
//! - `quit`: the app was closed from its own exit path.
//! - `sleep`: the same process missed ticks, so the machine was suspended (any OS: the
//!   scheduler thread cannot run while the system sleeps).
//! - `state`: capture state changed; the next row starts at the same instant (no gap).
//! - `lost`: a new process found the row still open: crash, forced quit or power off.

use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use crate::storage::LocalStore;

/// More than this between ticks of the same process means it was suspended.
pub const MAX_TICK_GAP_SECONDS: i64 = 90;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStateV1 {
    pub window_status: String,
    pub afk_status: String,
    pub capture_issue: Option<String>,
    pub paused: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSessionV1 {
    pub id: i64,
    pub started_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub window_status: String,
    pub afk_status: String,
    pub capture_issue: Option<String>,
    pub paused: bool,
    pub ended_reason: Option<String>,
}

impl RuntimeSessionV1 {
    fn state(&self) -> RuntimeStateV1 {
        RuntimeStateV1 {
            window_status: self.window_status.clone(),
            afk_status: self.afk_status.clone(),
            capture_issue: self.capture_issue.clone(),
            paused: self.paused,
        }
    }
}

/// In-memory memory of the previous tick of this process.
#[derive(Debug, Default)]
pub struct Heartbeat {
    last_tick: Option<DateTime<Utc>>,
}

impl Heartbeat {
    pub fn tick(
        &mut self,
        store: &LocalStore,
        now: DateTime<Utc>,
        state: &RuntimeStateV1,
    ) -> Result<(), String> {
        let slept = self
            .last_tick
            .is_some_and(|previous| (now - previous).num_seconds() > MAX_TICK_GAP_SECONDS);
        record_tick(store, now, state, self.last_tick.is_some(), slept)?;
        self.last_tick = Some(now);
        Ok(())
    }
}

const SELECT_COLUMNS: &str =
    "id, started_at, last_seen_at, window_status, afk_status, capture_issue, paused, ended_reason";

fn row_to_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawSession> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
    ))
}

type RawSession = (
    i64,
    String,
    String,
    String,
    String,
    Option<String>,
    bool,
    Option<String>,
);

fn parse_session(raw: RawSession) -> Option<RuntimeSessionV1> {
    let (id, started, last_seen, window_status, afk_status, capture_issue, paused, ended_reason) =
        raw;
    Some(RuntimeSessionV1 {
        id,
        started_at: crate::models::parse_utc(&started).ok()?,
        last_seen_at: crate::models::parse_utc(&last_seen).ok()?,
        window_status,
        afk_status,
        capture_issue,
        paused,
        ended_reason,
    })
}

fn open_session(connection: &rusqlite::Connection) -> Result<Option<RuntimeSessionV1>, String> {
    connection
        .query_row(
            &format!(
                "SELECT {SELECT_COLUMNS} FROM runtime_sessions
                 WHERE ended_reason IS NULL ORDER BY id DESC LIMIT 1"
            ),
            [],
            row_to_session,
        )
        .optional()
        .map_err(|error| error.to_string())
        .map(|raw| raw.and_then(parse_session))
}

fn end_session(
    connection: &rusqlite::Connection,
    id: i64,
    last_seen: Option<DateTime<Utc>>,
    reason: &str,
) -> Result<(), String> {
    match last_seen {
        Some(at) => connection.execute(
            "UPDATE runtime_sessions SET last_seen_at = ?2, ended_reason = ?3 WHERE id = ?1",
            params![id, at.to_rfc3339(), reason],
        ),
        None => connection.execute(
            "UPDATE runtime_sessions SET ended_reason = ?2 WHERE id = ?1",
            params![id, reason],
        ),
    }
    .map(|_| ())
    .map_err(|error| error.to_string())
}

/// `same_process` is false on the first tick after launch; `slept` means this process
/// missed ticks.
pub fn record_tick(
    store: &LocalStore,
    now: DateTime<Utc>,
    state: &RuntimeStateV1,
    same_process: bool,
    slept: bool,
) -> Result<(), String> {
    let connection = store.connection()?;
    if let Some(open) = open_session(&connection)? {
        if !same_process {
            end_session(&connection, open.id, None, "lost")?;
        } else if slept {
            end_session(&connection, open.id, None, "sleep")?;
        } else if open.state() == *state {
            connection
                .execute(
                    "UPDATE runtime_sessions SET last_seen_at = ?2 WHERE id = ?1",
                    params![open.id, now.to_rfc3339()],
                )
                .map_err(|error| error.to_string())?;
            return Ok(());
        } else {
            end_session(&connection, open.id, Some(now), "state")?;
        }
    }
    connection
        .execute(
            "INSERT INTO runtime_sessions
               (started_at, last_seen_at, window_status, afk_status, capture_issue, paused)
             VALUES (?1, ?1, ?2, ?3, ?4, ?5)",
            params![
                now.to_rfc3339(),
                state.window_status,
                state.afk_status,
                state.capture_issue,
                state.paused
            ],
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Called from the app's exit path. A no-op when nothing is open.
pub fn close_session(store: &LocalStore, now: DateTime<Utc>) -> Result<(), String> {
    let connection = store.connection()?;
    if let Some(open) = open_session(&connection)? {
        end_session(&connection, open.id, Some(now), "quit")?;
    }
    Ok(())
}

/// Sessions overlapping `[from, to]`, plus the last one before `from` so the gap at the
/// start of the range can be explained. Ordered by start.
pub fn sessions_between(
    store: &LocalStore,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<RuntimeSessionV1>, String> {
    let connection = store.connection()?;
    let (from, to) = (from.to_rfc3339(), to.to_rfc3339());
    let mut sessions = Vec::new();
    let previous = connection
        .query_row(
            &format!(
                "SELECT {SELECT_COLUMNS} FROM runtime_sessions
                 WHERE last_seen_at < ?1 ORDER BY last_seen_at DESC LIMIT 1"
            ),
            [&from],
            row_to_session,
        )
        .optional()
        .map_err(|error| error.to_string())?;
    sessions.extend(previous.and_then(parse_session));
    let mut statement = connection
        .prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM runtime_sessions
             WHERE last_seen_at >= ?1 AND started_at <= ?2 ORDER BY started_at, id"
        ))
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([&from, &to], row_to_session)
        .map_err(|error| error.to_string())?;
    for row in rows {
        sessions.extend(parse_session(row.map_err(|error| error.to_string())?));
    }
    Ok(sessions)
}

/// When the uptime log starts. Gaps before it can only be inferred.
pub fn tracking_since(store: &LocalStore) -> Result<Option<DateTime<Utc>>, String> {
    let connection = store.connection()?;
    let first: Option<String> = connection
        .query_row("SELECT MIN(started_at) FROM runtime_sessions", [], |row| {
            row.get(0)
        })
        .map_err(|error| error.to_string())?;
    Ok(first.and_then(|value| crate::models::parse_utc(&value).ok()))
}

/// The most recent stretch where Ensō stopped without its own exit path (`lost`), as
/// `(last seen, next start)`. `None` when there is none or the app never restarted.
pub fn last_lost_gap(store: &LocalStore) -> Result<Option<(DateTime<Utc>, DateTime<Utc>)>, String> {
    let connection = store.connection()?;
    let row: Option<(String, String)> = connection
        .query_row(
            "SELECT lost.last_seen_at, next.started_at
             FROM runtime_sessions AS lost
             JOIN runtime_sessions AS next ON next.id = (
               SELECT MIN(id) FROM runtime_sessions WHERE id > lost.id
             )
             WHERE lost.ended_reason = 'lost'
             ORDER BY lost.id DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    Ok(row.and_then(|(from, to)| {
        Some((
            crate::models::parse_utc(&from).ok()?,
            crate::models::parse_utc(&to).ok()?,
        ))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use tempfile::TempDir;

    fn store() -> (TempDir, LocalStore) {
        let temp = TempDir::new().unwrap();
        let store = LocalStore::open(&temp.path().join("enso.sqlite")).unwrap();
        (temp, store)
    }

    fn running() -> RuntimeStateV1 {
        RuntimeStateV1 {
            window_status: "running".into(),
            afk_status: "running".into(),
            capture_issue: None,
            paused: false,
        }
    }

    fn at(minutes: i64) -> DateTime<Utc> {
        crate::models::parse_utc("2026-11-23T13:00:00Z").unwrap() + Duration::minutes(minutes)
    }

    fn all(store: &LocalStore) -> Vec<RuntimeSessionV1> {
        sessions_between(store, at(-10_000), at(10_000)).unwrap()
    }

    #[test]
    fn identical_ticks_compact_into_one_row() {
        let (_temp, store) = store();
        let mut heartbeat = Heartbeat::default();
        for tick in 0..20 {
            heartbeat
                .tick(&store, at(0) + Duration::seconds(30 * tick), &running())
                .unwrap();
        }
        let sessions = all(&store);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].started_at, at(0));
        assert_eq!(sessions[0].last_seen_at, at(0) + Duration::seconds(570));
        assert_eq!(sessions[0].ended_reason, None);
    }

    #[test]
    fn state_change_sleep_quit_and_lost_end_rows_with_their_reason() {
        let (_temp, store) = store();
        let mut heartbeat = Heartbeat::default();
        heartbeat.tick(&store, at(0), &running()).unwrap();
        let mut down = running();
        down.window_status = "stopped".into();
        heartbeat.tick(&store, at(1), &down).unwrap();
        // Missed ticks for 20 minutes: the machine slept.
        heartbeat.tick(&store, at(21), &down).unwrap();
        close_session(&store, at(22)).unwrap();
        // A new process starts; then it dies without quitting and another one starts.
        let mut second = Heartbeat::default();
        second.tick(&store, at(30), &running()).unwrap();
        let mut third = Heartbeat::default();
        third.tick(&store, at(90), &running()).unwrap();

        let sessions = all(&store);
        let reasons = sessions
            .iter()
            .map(|session| session.ended_reason.as_deref())
            .collect::<Vec<_>>();
        assert_eq!(
            reasons,
            vec![
                Some("state"),
                Some("sleep"),
                Some("quit"),
                Some("lost"),
                None
            ]
        );
        assert_eq!(sessions[0].last_seen_at, at(1));
        assert_eq!(sessions[1].started_at, at(1));
        assert_eq!(sessions[1].last_seen_at, at(1));
        assert_eq!(sessions[2].started_at, at(21));
        assert_eq!(last_lost_gap(&store).unwrap(), Some((at(30), at(90))));
        assert_eq!(tracking_since(&store).unwrap(), Some(at(0)));
    }

    #[test]
    fn retention_prunes_old_sessions_and_delete_all_removes_every_session() {
        let (_temp, store) = store();
        let old = crate::models::parse_utc("2026-01-01T10:00:00Z").unwrap();
        Heartbeat::default().tick(&store, old, &running()).unwrap();
        close_session(&store, old + Duration::minutes(5)).unwrap();
        Heartbeat::default()
            .tick(&store, Utc::now(), &running())
            .unwrap();
        store.prune_context_and_ai_runs(Some(30)).unwrap();
        let remaining = sessions_between(&store, old - Duration::days(1), Utc::now()).unwrap();
        assert_eq!(remaining.len(), 1);
        assert!(remaining[0].started_at > old);
        store.delete_all().unwrap();
        assert!(tracking_since(&store).unwrap().is_none());
    }

    #[test]
    fn range_query_includes_the_session_before_the_range() {
        let (_temp, store) = store();
        let mut heartbeat = Heartbeat::default();
        heartbeat.tick(&store, at(0), &running()).unwrap();
        close_session(&store, at(5)).unwrap();
        let mut next = Heartbeat::default();
        next.tick(&store, at(100), &running()).unwrap();
        let sessions = sessions_between(&store, at(50), at(200)).unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].ended_reason.as_deref(), Some("quit"));
        assert_eq!(close_session(&store, at(300)), Ok(()));
        assert_eq!(close_session(&store, at(301)), Ok(()));
    }
}
