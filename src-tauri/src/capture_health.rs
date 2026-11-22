//! How complete the captured data is, and what to fix. Process mapping is only as good as
//! its inputs: window titles (rules, case ids) and the browser connector (which site).
//! Aggregates over the timeline; no titles or domains leave this module.

use std::collections::HashMap;

use serde::Serialize;

use crate::connectors::ConnectorClientV1;
use crate::models::TimelineBlockV1;
use crate::process_context::ParsedObservation;

/// Below this share of titled window time, titles are considered missing.
const MIN_TITLED_SHARE: f64 = 0.1;
/// Below this share of browser time with a site, the connector is not reporting.
const MIN_BROWSER_DOMAIN_SHARE: f64 = 0.5;
/// Less browser time than this is too little to judge the connector.
const MIN_BROWSER_SECONDS: i64 = 30 * 60;
/// Less window time than this is too little to judge titles.
const MIN_WINDOW_SECONDS: i64 = 30 * 60;
const MAX_UNKNOWN_APPS: usize = 3;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnknownAppV1 {
    pub app: String,
    pub seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureHealthV1 {
    pub active_seconds: i64,
    pub unknown_share: f64,
    /// Apps with the most unclassified time, longest first.
    pub top_unknown: Vec<UnknownAppV1>,
    /// `off` (setting disabled), `missing` (enabled but not arriving), `ok`, `insufficient`.
    pub titles: String,
    pub titled_share: f64,
    /// Missing titles are fixed by granting Accessibility (macOS only).
    pub accessibility_fix: bool,
    pub browser_seconds: i64,
    pub browser_domain_share: f64,
    /// `not-paired`, `not-reporting`, `ok`, `insufficient` (too little browser time).
    pub browser_connector: String,
}

fn share(part: i64, total: i64) -> f64 {
    if total <= 0 {
        0.0
    } else {
        part as f64 / total as f64
    }
}

fn is_window_activity(block: &TimelineBlockV1) -> bool {
    !block.afk
        && !block.id.starts_with("manual-")
        && block.app != crate::process_context::EXCLUDED_APP_LABEL
}

/// Only time covered by a live observation counts: the block's dominant domain may come
/// from a single report.
fn covered_seconds(block: &TimelineBlockV1, observations: &[ParsedObservation<'_>]) -> i64 {
    let (Ok(start), Ok(end)) = (
        crate::models::parse_utc(&block.start),
        crate::models::parse_utc(&block.end),
    ) else {
        return 0;
    };
    crate::process_context::context_covered_seconds(&block.app, start, end, observations)
}

pub fn capture_health(
    blocks: &[TimelineBlockV1],
    capture_titles: bool,
    connectors: &[ConnectorClientV1],
    observations: &[ParsedObservation<'_>],
) -> CaptureHealthV1 {
    let mut active = 0;
    let mut titled = 0;
    let mut unknown = 0;
    let mut browser = 0;
    let mut browser_with_domain = 0;
    let mut unknown_by_app: HashMap<String, i64> = HashMap::new();
    for block in blocks.iter().filter(|block| is_window_activity(block)) {
        let seconds = block.duration_seconds.max(0);
        active += seconds;
        if block.title.is_some() {
            titled += seconds;
        }
        if block.category == "unknown" {
            unknown += seconds;
            *unknown_by_app
                .entry(crate::process_context::app_identity(&block.app))
                .or_default() += seconds;
        }
        if crate::process_context::accepts_browser_context(&block.app) {
            browser += seconds;
            browser_with_domain += covered_seconds(block, observations).min(seconds);
        }
    }
    let mut top_unknown = unknown_by_app
        .into_iter()
        .map(|(app, seconds)| UnknownAppV1 { app, seconds })
        .collect::<Vec<_>>();
    top_unknown.sort_by(|left, right| {
        right
            .seconds
            .cmp(&left.seconds)
            .then_with(|| left.app.cmp(&right.app))
    });
    top_unknown.truncate(MAX_UNKNOWN_APPS);

    let titled_share = share(titled, active);
    let titles = if !capture_titles {
        "off"
    } else if active < MIN_WINDOW_SECONDS {
        "insufficient"
    } else if titled_share < MIN_TITLED_SHARE {
        "missing"
    } else {
        "ok"
    };
    let browser_domain_share = share(browser_with_domain, browser);
    let paired = connectors
        .iter()
        .any(|client| client.kind == "browser" && client.revoked_at.is_none());
    let browser_connector = if browser < MIN_BROWSER_SECONDS {
        "insufficient"
    } else if !paired {
        "not-paired"
    } else if browser_domain_share < MIN_BROWSER_DOMAIN_SHARE {
        "not-reporting"
    } else {
        "ok"
    };
    CaptureHealthV1 {
        active_seconds: active,
        unknown_share: share(unknown, active),
        top_unknown,
        titles: titles.into(),
        titled_share,
        accessibility_fix: cfg!(target_os = "macos") && titles == "missing",
        browser_seconds: browser,
        browser_domain_share,
        browser_connector: browser_connector.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(app: &str, seconds: i64, category: &str) -> TimelineBlockV1 {
        TimelineBlockV1 {
            id: format!("{app}-{seconds}-{category}"),
            start: "2026-11-21T10:00:00Z".into(),
            end: (crate::models::parse_utc("2026-11-21T10:00:00Z").unwrap()
                + chrono::Duration::seconds(seconds))
            .to_rfc3339(),
            duration_seconds: seconds,
            app: app.into(),
            title: None,
            activity_label: None,
            category: category.into(),
            confidence: 0.0,
            afk: false,
            source: "window".into(),
            check_in_id: None,
            classification_source: "builtin".into(),
            rule_id: None,
            correctable: true,
            context_domain: None,
            context_project: None,
            case_refs: Vec::new(),
        }
    }

    /// One browser report per minute from `from_minute` for `minutes`, like extension 0.2.0.
    fn reports(from_minute: i64, minutes: i64) -> Vec<crate::intelligence::ContextObservationV2> {
        (from_minute..from_minute + minutes)
            .map(|minute| {
                let at = (crate::models::parse_utc("2026-11-21T10:00:00Z").unwrap()
                    + chrono::Duration::minutes(minute))
                .to_rfc3339();
                crate::intelligence::ContextObservationV2 {
                    schema_version: 2,
                    id: format!("browser-{minute}"),
                    source: "browser".into(),
                    observed_at: at.clone(),
                    duration_seconds: 0,
                    app: Some("Google Chrome".into()),
                    title: None,
                    domain: Some("mail.google.com".into()),
                    project: None,
                    repository: None,
                    branch: None,
                    file_type: None,
                    text_excerpt: None,
                    ocr_terms: Vec::new(),
                    sensitivity: "normal".into(),
                    client_id: None,
                    created_at: at,
                }
            })
            .collect()
    }

    fn browser_client(revoked: bool) -> ConnectorClientV1 {
        ConnectorClientV1 {
            id: "c1".into(),
            kind: "browser".into(),
            name: "Chrome".into(),
            origin: None,
            created_at: "2026-11-20T10:00:00Z".into(),
            last_seen_at: None,
            revoked_at: revoked.then(|| "2026-11-21T10:00:00Z".into()),
        }
    }

    #[test]
    fn titles_enabled_but_absent_and_unpaired_browser_are_reported() {
        let blocks = vec![
            block("Google Chrome", 3_600, "unknown"),
            block("Figma", 1_800, "creation"),
            block("Notes", 600, "unknown"),
        ];
        let health = capture_health(&blocks, true, &[browser_client(true)], &[]);
        assert_eq!(health.titles, "missing");
        assert_eq!(health.accessibility_fix, cfg!(target_os = "macos"));
        assert_eq!(health.browser_connector, "not-paired");
        assert_eq!(health.active_seconds, 6_000);
        assert!((health.unknown_share - 0.7).abs() < 1e-9);
        assert_eq!(health.top_unknown[0].app, "google chrome");
        assert_eq!(health.top_unknown[1].app, "notes");
    }

    #[test]
    fn healthy_capture_is_ok_and_off_is_respected() {
        let mut chrome = block("Google Chrome", 3_600, "communication");
        chrome.context_domain = Some("mail.google.com".into());
        chrome.title = Some("Bandeja".into());
        let mut figma = block("Figma", 1_800, "creation");
        figma.title = Some("Diseño".into());
        let blocks = vec![chrome, figma];
        let observed = reports(0, 60);
        let parsed = crate::process_context::parse_observations(&observed);
        let health = capture_health(&blocks, true, &[browser_client(false)], &parsed);
        assert_eq!(health.titles, "ok");
        assert!(health.browser_domain_share > 0.99);
        assert_eq!(health.browser_connector, "ok");
        assert!(health.top_unknown.is_empty());
        assert_eq!(capture_health(&blocks, false, &[], &[]).titles, "off");
    }

    #[test]
    fn paired_connector_without_sites_is_not_reporting_and_small_samples_are_insufficient() {
        let mut chrome = block("Google Chrome", 3_600, "unknown");
        // The timeline assigns the block's domain from a single report…
        chrome.context_domain = Some("mail.google.com".into());
        let observed = reports(0, 1);
        let parsed = crate::process_context::parse_observations(&observed);
        let health = capture_health(&[chrome], true, &[browser_client(false)], &parsed);
        // …but that report only covers its hold window, not the whole hour.
        assert_eq!(health.browser_connector, "not-reporting");
        assert!(health.browser_domain_share < 0.05);
        let small = capture_health(&[block("Google Chrome", 60, "unknown")], true, &[], &[]);
        assert_eq!(small.browser_connector, "insufficient");
        assert_eq!(small.titles, "insufficient");
    }

    #[test]
    fn afk_manual_and_excluded_time_do_not_count() {
        let mut away = block("Figma", 9_000, "unknown");
        away.afk = true;
        let mut manual = block("Figma", 9_000, "admin");
        manual.id = "manual-1".into();
        let excluded = block(crate::process_context::EXCLUDED_APP_LABEL, 9_000, "unknown");
        let health = capture_health(&[away, manual, excluded], true, &[], &[]);
        assert_eq!(health.active_seconds, 0);
        assert!(health.top_unknown.is_empty());
    }
}
