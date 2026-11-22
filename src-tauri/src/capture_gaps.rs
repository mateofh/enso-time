//! Classifies every minute of a range by why it was (or was not) captured, from Ensō's
//! uptime log (`runtime_sessions`), the captured window blocks and the AFK watcher's
//! presence intervals. Before the uptime log existed, gaps can only be inferred.

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;

use crate::runtime_sessions::RuntimeSessionV1;

const STEP_SECONDS: i64 = 60;
/// A session covers up to one tick past its last heartbeat.
const SESSION_SLACK_SECONDS: i64 = 60;
const MAX_GAPS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reason {
    Captured,
    /// Ensō running with healthy capture but no window event (e.g. screen locked).
    Idle,
    Asleep,
    Quit,
    ClosedOrOff,
    WatcherDown,
    Paused,
}

impl Reason {
    fn code(self) -> &'static str {
        match self {
            Reason::Captured => "captured",
            Reason::Idle => "idle",
            Reason::Asleep => "asleep",
            Reason::Quit => "quit",
            Reason::ClosedOrOff => "closed-or-off",
            Reason::WatcherDown => "watcher-down",
            Reason::Paused => "paused",
        }
    }

    fn is_gap(self) -> bool {
        !matches!(self, Reason::Captured | Reason::Idle)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageTotalsV1 {
    pub captured: i64,
    pub idle: i64,
    pub asleep: i64,
    pub quit: i64,
    pub closed_or_off: i64,
    pub watcher_down: i64,
    pub paused: i64,
}

impl CoverageTotalsV1 {
    fn add(&mut self, reason: Reason, seconds: i64) {
        let slot = match reason {
            Reason::Captured => &mut self.captured,
            Reason::Idle => &mut self.idle,
            Reason::Asleep => &mut self.asleep,
            Reason::Quit => &mut self.quit,
            Reason::ClosedOrOff => &mut self.closed_or_off,
            Reason::WatcherDown => &mut self.watcher_down,
            Reason::Paused => &mut self.paused,
        };
        *slot += seconds;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageGapV1 {
    pub start: String,
    pub end: String,
    /// `asleep`, `quit`, `closed-or-off`, `watcher-down` or `paused`.
    pub reason: String,
    /// True before the uptime log existed: deduced from captured data only.
    pub inferred: bool,
    /// Part of the gap inside the work schedule.
    pub schedule_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureCoverageV1 {
    /// Start of Ensō's uptime log; earlier gaps are inferred.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracking_since: Option<String>,
    /// Inside the work schedule, which is what coverage is judged on.
    pub schedule: CoverageTotalsV1,
    pub all: CoverageTotalsV1,
    /// Longest gaps inside the schedule, longest first.
    pub gaps: Vec<CoverageGapV1>,
}

/// Sorted, merged copy of the intervals so membership is a binary search.
fn merged(intervals: &[(DateTime<Utc>, DateTime<Utc>)]) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
    let mut items = intervals
        .iter()
        .copied()
        .filter(|(start, end)| end > start)
        .collect::<Vec<_>>();
    items.sort_by_key(|(start, _)| *start);
    let mut out: Vec<(DateTime<Utc>, DateTime<Utc>)> = Vec::with_capacity(items.len());
    for (start, end) in items {
        match out.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => out.push((start, end)),
        }
    }
    out
}

fn contains(intervals: &[(DateTime<Utc>, DateTime<Utc>)], at: DateTime<Utc>) -> bool {
    let index = intervals.partition_point(|(start, _)| *start <= at);
    index > 0 && intervals[index - 1].1 > at
}

fn classify_minute(
    at: DateTime<Utc>,
    sessions: &[RuntimeSessionV1],
    tracking_since: Option<DateTime<Utc>>,
    window: &[(DateTime<Utc>, DateTime<Utc>)],
    presence: &[(DateTime<Utc>, DateTime<Utc>)],
) -> (Reason, bool) {
    if contains(window, at) {
        return (Reason::Captured, false);
    }
    let slack = Duration::seconds(SESSION_SLACK_SECONDS);
    // The tick slack can overlap the next session; the most recent one wins.
    if let Some(session) = sessions
        .iter()
        .filter(|session| session.started_at <= at && at <= session.last_seen_at + slack)
        .max_by_key(|session| (session.started_at, session.id))
    {
        let reason = if session.paused {
            Reason::Paused
        } else if session.window_status != "running" {
            Reason::WatcherDown
        } else {
            Reason::Idle
        };
        return (reason, false);
    }
    if tracking_since.is_none_or(|since| at < since) {
        // No uptime log yet: presence without windows means the window watcher was down;
        // nothing at all cannot tell "Ensō closed" from "machine off".
        let reason = if contains(presence, at) {
            Reason::WatcherDown
        } else {
            Reason::ClosedOrOff
        };
        return (reason, true);
    }
    let previous = sessions
        .iter()
        .filter(|session| session.last_seen_at < at)
        .max_by_key(|session| session.last_seen_at);
    let reason = match previous.and_then(|session| session.ended_reason.as_deref()) {
        Some("sleep") => Reason::Asleep,
        Some("quit") => Reason::Quit,
        _ => Reason::ClosedOrOff,
    };
    (reason, false)
}

pub fn classify(
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    sessions: &[RuntimeSessionV1],
    tracking_since: Option<DateTime<Utc>>,
    window: &[(DateTime<Utc>, DateTime<Utc>)],
    presence: &[(DateTime<Utc>, DateTime<Utc>)],
    in_schedule: &dyn Fn(DateTime<Utc>) -> bool,
) -> CaptureCoverageV1 {
    let window = merged(window);
    let presence = merged(presence);
    let mut schedule = CoverageTotalsV1::default();
    let mut all = CoverageTotalsV1::default();
    let mut gaps: Vec<CoverageGapV1> = Vec::new();
    let mut current: Option<(Reason, bool, DateTime<Utc>, DateTime<Utc>, i64)> = None;
    let mut cursor = from;
    while cursor < to {
        let next = (cursor + Duration::seconds(STEP_SECONDS)).min(to);
        let seconds = (next - cursor).num_seconds();
        let middle = cursor + (next - cursor) / 2;
        let (reason, inferred) =
            classify_minute(middle, sessions, tracking_since, &window, &presence);
        let scheduled = in_schedule(middle);
        all.add(reason, seconds);
        if scheduled {
            schedule.add(reason, seconds);
        }
        let scheduled_seconds = if scheduled { seconds } else { 0 };
        match &mut current {
            Some((open_reason, open_inferred, _, end, schedule_seconds))
                if *open_reason == reason && *open_inferred == inferred =>
            {
                *end = next;
                *schedule_seconds += scheduled_seconds;
            }
            _ => {
                if let Some(gap) = current.take() {
                    push_gap(&mut gaps, gap);
                }
                current = Some((reason, inferred, cursor, next, scheduled_seconds));
            }
        }
        cursor = next;
    }
    if let Some(gap) = current {
        push_gap(&mut gaps, gap);
    }
    gaps.sort_by(|left, right| right.schedule_seconds.cmp(&left.schedule_seconds));
    gaps.truncate(MAX_GAPS);
    CaptureCoverageV1 {
        tracking_since: tracking_since.map(|since| since.to_rfc3339()),
        schedule,
        all,
        gaps,
    }
}

fn push_gap(
    gaps: &mut Vec<CoverageGapV1>,
    (reason, inferred, start, end, schedule_seconds): (
        Reason,
        bool,
        DateTime<Utc>,
        DateTime<Utc>,
        i64,
    ),
) {
    if reason.is_gap() && schedule_seconds > 0 {
        gaps.push(CoverageGapV1 {
            start: start.to_rfc3339(),
            end: end.to_rfc3339(),
            reason: reason.code().into(),
            inferred,
            schedule_seconds,
        });
    }
}

/// Seconds of `[from, to)` inside the work schedule, sampled per minute.
pub fn schedule_seconds_between(
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    in_schedule: &dyn Fn(DateTime<Utc>) -> bool,
) -> i64 {
    let mut total = 0;
    let mut cursor = from;
    while cursor < to {
        let next = (cursor + Duration::seconds(STEP_SECONDS)).min(to);
        if in_schedule(cursor + (next - cursor) / 2) {
            total += (next - cursor).num_seconds();
        }
        cursor = next;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(minutes: i64) -> DateTime<Utc> {
        crate::models::parse_utc("2026-11-23T13:00:00Z").unwrap() + Duration::minutes(minutes)
    }

    fn session(
        id: i64,
        from: i64,
        to: i64,
        window_status: &str,
        paused: bool,
        ended: Option<&str>,
    ) -> RuntimeSessionV1 {
        RuntimeSessionV1 {
            id,
            started_at: at(from),
            last_seen_at: at(to),
            window_status: window_status.into(),
            afk_status: "running".into(),
            capture_issue: None,
            paused,
            ended_reason: ended.map(Into::into),
        }
    }

    fn always(_: DateTime<Utc>) -> bool {
        true
    }

    #[test]
    fn every_reason_is_classified_from_the_uptime_log() {
        let sessions = vec![
            session(1, 0, 60, "running", false, Some("sleep")),
            session(2, 90, 120, "stopped", false, Some("state")),
            session(3, 120, 150, "running", true, Some("quit")),
            session(4, 200, 230, "running", false, Some("lost")),
            session(5, 300, 360, "running", false, None),
        ];
        // Windows were captured for the first 30 minutes and the last 30.
        let window = vec![(at(0), at(30)), (at(330), at(360))];
        let coverage = classify(
            at(0),
            at(360),
            &sessions,
            Some(at(0)),
            &window,
            &[],
            &always,
        );
        let minutes = |seconds: i64| seconds / 60;
        let totals = &coverage.schedule;
        assert_eq!(minutes(totals.captured), 60);
        // Idle: 30..61 in session 1 (one tick of slack), 200..231 in session 4 and
        // 300..330 in session 5.
        assert_eq!(minutes(totals.idle), 31 + 31 + 30);
        assert_eq!(minutes(totals.asleep), 90 - 61);
        assert_eq!(minutes(totals.watcher_down), 30);
        assert_eq!(minutes(totals.paused), 31);
        assert_eq!(minutes(totals.quit), 200 - 151);
        assert_eq!(minutes(totals.closed_or_off), 300 - 231);
        assert_eq!(coverage.all, coverage.schedule);
        let reasons = coverage
            .gaps
            .iter()
            .map(|gap| gap.reason.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            reasons,
            vec!["closed-or-off", "quit", "paused", "watcher-down", "asleep"]
        );
        assert!(coverage.gaps.iter().all(|gap| !gap.inferred));
    }

    #[test]
    fn before_the_uptime_log_gaps_are_inferred_from_presence() {
        // Presence (AFK watcher) without windows for an hour, then nothing for an hour.
        let window = vec![(at(0), at(60))];
        let presence = vec![(at(0), at(120))];
        let coverage = classify(at(0), at(180), &[], None, &window, &presence, &always);
        assert_eq!(coverage.schedule.captured, 3_600);
        assert_eq!(coverage.schedule.watcher_down, 3_600);
        assert_eq!(coverage.schedule.closed_or_off, 3_600);
        assert!(coverage.gaps.iter().all(|gap| gap.inferred));
        assert!(coverage.tracking_since.is_none());
    }

    #[test]
    fn outside_the_schedule_time_counts_only_in_all_and_never_becomes_a_listed_gap() {
        let in_schedule = |moment: DateTime<Utc>| moment < at(60);
        let coverage = classify(
            at(0),
            at(180),
            &[],
            Some(at(0)),
            &[(at(0), at(60))],
            &[],
            &in_schedule,
        );
        assert_eq!(coverage.schedule.captured, 3_600);
        assert_eq!(coverage.schedule.closed_or_off, 0);
        assert_eq!(coverage.all.closed_or_off, 7_200);
        assert!(coverage.gaps.is_empty());
        assert_eq!(
            schedule_seconds_between(at(0), at(180), &in_schedule),
            3_600
        );
    }
}
