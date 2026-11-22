//! Pure helpers that turn already-captured context into process-mapping signal.
//!
//! Nothing here captures new data: it filters excluded apps, redacts window titles,
//! attaches connector context (browser domain, editor/shell project) to timeline blocks,
//! and maps well-known domains to categories.

use chrono::{DateTime, Duration, Utc};

use crate::intelligence::ContextObservationV2;

/// App label stored and shown instead of an excluded app's real name.
pub const EXCLUDED_APP_LABEL: &str = "Aplicación excluida";

/// Connector observations are sent on tab/editor change and about once a minute, so the
/// context active at a block's start may have been observed up to this long before it.
pub const CONTEXT_LOOKBACK_SECONDS: i64 = 75;

/// A context observation stays in effect for at most this long without a newer one.
const CONTEXT_MAX_HOLD_SECONDS: i64 = 150;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockContext {
    pub domain: Option<String>,
    pub project: Option<String>,
}

/// Normalized app identity that also matches Windows executable basenames
/// (`aw-watcher-window` reports `chrome.exe`, `1Password.exe`, ...).
pub(crate) fn app_identity(app: &str) -> String {
    let normalized = crate::activitywatch::normalize_match_text(app);
    normalized
        .strip_suffix(".exe")
        .map(str::to_string)
        .unwrap_or(normalized)
}

/// Lock screen and screensaver processes: the person is away, not working.
const LOCK_SCREEN_APPS: [&str; 4] = ["loginwindow", "screensaverengine", "lockapp", "logonui"];

pub fn is_lock_screen(app: &str) -> bool {
    LOCK_SCREEN_APPS.contains(&app_identity(app).as_str())
}

pub fn is_excluded_app(app: &str, excluded_apps: &[String]) -> bool {
    let identity = app_identity(app);
    !identity.is_empty()
        && excluded_apps
            .iter()
            .any(|excluded| app_identity(excluded) == identity)
}

/// The only browser connector is the Chromium extension, and it reports the active tab
/// even while another browser is frontmost. Only Chromium-family windows may take its
/// domain; Safari and Firefox blocks must never inherit Chrome's background tab.
pub fn accepts_browser_context(app: &str) -> bool {
    [
        "google chrome",
        "chrome",
        "chromium",
        "microsoft edge",
        "msedge",
        "brave browser",
        "brave",
        "arc",
        "opera",
        "vivaldi",
    ]
    .contains(&app_identity(app).as_str())
}

/// Redacts emails, card-like numbers, API keys and phone numbers from a window title.
pub fn sanitize_window_title(title: &str) -> String {
    crate::intelligence::redact_sensitive_text(title)
        .replace('\n', " ")
        .trim()
        .to_string()
}

/// Matches the browser that sent an observation (its brand, from the extension) with the
/// frontmost window. Extension 0.1.0 sent the generic `Chromium`; that stays accepted for
/// any Chromium-family window until users update the extension.
fn browser_emitter_matches(emitter: Option<&str>, app: &str) -> bool {
    let identity = app_identity(app);
    let family: &[&str] = match emitter.map(app_identity).as_deref() {
        Some("google chrome") => &["google chrome", "chrome", "arc"],
        Some("microsoft edge") => &["microsoft edge", "msedge"],
        Some("brave") => &["brave browser", "brave"],
        Some("opera") => &["opera"],
        Some("vivaldi") => &["vivaldi"],
        _ => return true,
    };
    family.contains(&identity.as_str())
}

pub(crate) fn context_source_for_app(app: &str) -> Option<&'static str> {
    let identity = app_identity(app);
    if accepts_browser_context(app) {
        Some("browser")
    } else if ["visual studio code", "code", "cursor", "vscodium"].contains(&identity.as_str()) {
        Some("editor")
    } else if [
        "terminal",
        "iterm2",
        "iterm",
        "warp",
        "ghostty",
        "windows terminal",
        "windowsterminal",
        "powershell",
        "pwsh",
    ]
    .contains(&identity.as_str())
    {
        Some("shell")
    } else {
        None
    }
}

/// Picks the connector context that was in effect for the largest share of the block.
///
/// Observations must be sorted by `observed_at` ascending (as returned by storage).
pub fn dominant_context(
    app: &str,
    block_start: DateTime<Utc>,
    block_end: DateTime<Utc>,
    observations: &[ParsedObservation<'_>],
) -> BlockContext {
    context_totals(app, block_start, block_end, observations)
        .into_iter()
        .max_by_key(|(_, total)| *total)
        .map(|(observation, _)| BlockContext {
            domain: observation.domain.clone(),
            project: observation
                .project
                .clone()
                .or_else(|| observation.repository.clone()),
        })
        .unwrap_or_default()
}

/// Seconds of the block actually covered by a live connector observation (each one holds
/// until the next or for `CONTEXT_MAX_HOLD_SECONDS`), so one stale report does not count
/// for an hour-long block.
pub fn context_covered_seconds(
    app: &str,
    block_start: DateTime<Utc>,
    block_end: DateTime<Utc>,
    observations: &[ParsedObservation<'_>],
) -> i64 {
    context_totals(app, block_start, block_end, observations)
        .iter()
        .filter(|(observation, _)| observation.domain.is_some() || observation.project.is_some())
        .map(|(_, total)| *total)
        .sum::<i64>()
        / 1000
}

/// Milliseconds of the block covered by each distinct context.
fn context_totals<'a>(
    app: &str,
    block_start: DateTime<Utc>,
    block_end: DateTime<Utc>,
    observations: &[ParsedObservation<'a>],
) -> Vec<(&'a ContextObservationV2, i64)> {
    let Some(source) = context_source_for_app(app) else {
        return Vec::new();
    };
    if block_end <= block_start {
        return Vec::new();
    }
    let lookback = block_start - Duration::seconds(CONTEXT_LOOKBACK_SECONDS);
    let first = observations.partition_point(|item| item.observed_at < lookback);
    let last = observations.partition_point(|item| item.observed_at < block_end);
    let relevant = observations[first..last.max(first)]
        .iter()
        .filter(|item| item.observation.source == source)
        .filter(|item| {
            source != "browser" || browser_emitter_matches(item.observation.app.as_deref(), app)
        })
        .collect::<Vec<_>>();
    let mut totals: Vec<(&ContextObservationV2, i64)> = Vec::new();
    for (index, item) in relevant.iter().enumerate() {
        let hold_end = item.observed_at + Duration::seconds(CONTEXT_MAX_HOLD_SECONDS);
        let effective_end = relevant
            .get(index + 1)
            .map_or(hold_end, |next| next.observed_at.min(hold_end));
        let covered =
            (effective_end.min(block_end) - item.observed_at.max(block_start)).num_milliseconds();
        if covered <= 0 {
            continue;
        }
        let key = context_key(item.observation);
        match totals
            .iter_mut()
            .find(|(observation, _)| context_key(observation) == key)
        {
            Some((_, total)) => *total += covered,
            None => totals.push((item.observation, covered)),
        }
    }
    totals
}

fn context_key(observation: &ContextObservationV2) -> (Option<&str>, Option<&str>) {
    (
        observation.domain.as_deref(),
        observation
            .project
            .as_deref()
            .or(observation.repository.as_deref()),
    )
}

/// An observation with its timestamp parsed once.
pub struct ParsedObservation<'a> {
    pub observed_at: DateTime<Utc>,
    pub observation: &'a ContextObservationV2,
}

pub fn parse_observations(observations: &[ContextObservationV2]) -> Vec<ParsedObservation<'_>> {
    let mut parsed = observations
        .iter()
        .filter_map(|observation| {
            crate::models::parse_utc(&observation.observed_at)
                .ok()
                .map(|observed_at| ParsedObservation {
                    observed_at,
                    observation,
                })
        })
        .collect::<Vec<_>>();
    parsed.sort_by_key(|item| item.observed_at);
    parsed
}

/// True when `domain` is `rule_domain` or one of its subdomains.
pub fn domain_matches(domain: &str, rule_domain: &str) -> bool {
    let domain = domain.trim_end_matches('.').to_ascii_lowercase();
    let rule_domain = rule_domain
        .trim()
        .trim_start_matches("*.")
        .trim_end_matches('.')
        .to_ascii_lowercase();
    !rule_domain.is_empty()
        && (domain == rule_domain || domain.ends_with(&format!(".{rule_domain}")))
}

const BUILT_IN_DOMAINS: &[(&str, &str)] = &[
    ("meet.google.com", "meetings"),
    ("zoom.us", "meetings"),
    ("teams.microsoft.com", "meetings"),
    ("teams.live.com", "meetings"),
    ("mail.google.com", "communication"),
    ("outlook.office.com", "communication"),
    ("outlook.live.com", "communication"),
    ("app.slack.com", "communication"),
    ("web.whatsapp.com", "communication"),
    ("calendar.google.com", "admin"),
    ("sheets.google.com", "admin"),
    ("drive.google.com", "admin"),
    ("docs.google.com", "creation"),
    ("notion.so", "creation"),
    ("figma.com", "creation"),
    ("canva.com", "creation"),
    ("github.com", "creation"),
    ("gitlab.com", "creation"),
    ("app.hubspot.com", "admin"),
    ("lightning.force.com", "admin"),
    ("salesforce.com", "admin"),
    ("gohighlevel.com", "admin"),
    ("app.asana.com", "admin"),
    ("trello.com", "admin"),
    ("linear.app", "admin"),
    ("airtable.com", "admin"),
    ("quickbooks.intuit.com", "admin"),
    ("xero.com", "admin"),
    ("coursera.org", "learning"),
    ("udemy.com", "learning"),
];

pub fn built_in_domain_category(domain: &str) -> Option<&'static str> {
    BUILT_IN_DOMAINS
        .iter()
        .filter(|(known, _)| domain_matches(domain, known))
        .max_by_key(|(known, _)| known.len())
        .map(|(_, category)| *category)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(
        source: &str,
        at: &str,
        domain: Option<&str>,
        project: Option<&str>,
    ) -> ContextObservationV2 {
        ContextObservationV2 {
            schema_version: 2,
            id: format!("{source}-{at}"),
            source: source.into(),
            observed_at: at.into(),
            duration_seconds: 0,
            app: None,
            title: None,
            domain: domain.map(Into::into),
            project: project.map(Into::into),
            repository: None,
            branch: None,
            file_type: None,
            text_excerpt: None,
            ocr_terms: Vec::new(),
            sensitivity: "normal".into(),
            client_id: None,
            created_at: at.into(),
        }
    }

    fn at(value: &str) -> DateTime<Utc> {
        crate::models::parse_utc(value).unwrap()
    }

    #[test]
    fn excluded_apps_match_case_and_invisible_characters() {
        let excluded = vec!["1Password".to_string()];
        assert!(is_excluded_app("1password", &excluded));
        assert!(is_excluded_app("\u{200e}1Password", &excluded));
        assert!(!is_excluded_app("Google Chrome", &excluded));
        assert!(!is_excluded_app("", &excluded));
        assert!(is_excluded_app("1Password.exe", &excluded));
    }

    #[test]
    fn safari_and_firefox_never_inherit_the_chromium_extension_domain() {
        let observations = vec![observation(
            "browser",
            "2026-11-21T12:00:05Z",
            Some("mail.google.com"),
            None,
        )];
        let parsed = parse_observations(&observations);
        let (start, end) = (at("2026-11-21T12:00:00Z"), at("2026-11-21T12:02:00Z"));
        for app in ["Safari", "Firefox", "firefox.exe"] {
            assert_eq!(
                dominant_context(app, start, end, &parsed),
                BlockContext::default()
            );
        }
        for app in ["chrome.exe", "msedge.exe", "Brave Browser"] {
            assert_eq!(
                dominant_context(app, start, end, &parsed).domain.as_deref(),
                Some("mail.google.com")
            );
        }
    }

    #[test]
    fn titles_lose_emails_and_long_numbers_but_keep_short_identifiers() {
        assert_eq!(
            sanitize_window_title("Re: factura — ana@cliente.com — Gmail"),
            "Re: factura — [REDACTED] — Gmail"
        );
        assert_eq!(
            sanitize_window_title("Ticket #48213 – Zendesk"),
            "Ticket #48213 – Zendesk"
        );
        assert_eq!(
            sanitize_window_title("INV-2024-0042.pdf"),
            "INV-2024-0042.pdf"
        );
        assert_eq!(
            sanitize_window_title("Llamar +57 300 123 4567"),
            "Llamar +[REDACTED]"
        );
    }

    #[test]
    fn browser_block_takes_the_domain_that_covered_most_of_it() {
        let observations = vec![
            observation(
                "browser",
                "2026-11-21T12:00:10Z",
                Some("mail.google.com"),
                None,
            ),
            observation(
                "browser",
                "2026-11-21T12:01:00Z",
                Some("app.hubspot.com"),
                None,
            ),
            observation(
                "browser",
                "2026-11-21T12:02:00Z",
                Some("app.hubspot.com"),
                None,
            ),
            observation("editor", "2026-11-21T12:00:30Z", None, Some("enso")),
        ];
        let parsed = parse_observations(&observations);
        let context = dominant_context(
            "Google Chrome",
            at("2026-11-21T12:00:00Z"),
            at("2026-11-21T12:03:00Z"),
            &parsed,
        );
        assert_eq!(context.domain.as_deref(), Some("app.hubspot.com"));
        assert_eq!(context.project, None);
    }

    #[test]
    fn observation_just_before_the_block_counts_but_stale_ones_do_not() {
        let observations = vec![
            observation(
                "browser",
                "2026-11-21T11:50:00Z",
                Some("old.example.com"),
                None,
            ),
            observation(
                "browser",
                "2026-11-21T11:59:30Z",
                Some("docs.google.com"),
                None,
            ),
        ];
        let parsed = parse_observations(&observations);
        let context = dominant_context(
            "Arc",
            at("2026-11-21T12:00:00Z"),
            at("2026-11-21T12:00:40Z"),
            &parsed,
        );
        assert_eq!(context.domain.as_deref(), Some("docs.google.com"));

        let stale = vec![observation(
            "browser",
            "2026-11-21T11:50:00Z",
            Some("old.example.com"),
            None,
        )];
        let parsed = parse_observations(&stale);
        assert_eq!(
            dominant_context(
                "Arc",
                at("2026-11-21T12:00:00Z"),
                at("2026-11-21T12:01:00Z"),
                &parsed
            ),
            BlockContext::default()
        );
    }

    #[test]
    fn editor_and_shell_blocks_take_projects_and_other_apps_take_nothing() {
        let observations = vec![
            observation("editor", "2026-11-21T12:00:05Z", None, Some("enso-time")),
            observation("shell", "2026-11-21T12:00:05Z", None, Some("ops-scripts")),
        ];
        let parsed = parse_observations(&observations);
        let range = (at("2026-11-21T12:00:00Z"), at("2026-11-21T12:02:00Z"));
        assert_eq!(
            dominant_context("Cursor", range.0, range.1, &parsed)
                .project
                .as_deref(),
            Some("enso-time")
        );
        assert_eq!(
            dominant_context("Terminal", range.0, range.1, &parsed)
                .project
                .as_deref(),
            Some("ops-scripts")
        );
        assert_eq!(
            dominant_context("Slack", range.0, range.1, &parsed),
            BlockContext::default()
        );
    }

    #[test]
    fn an_edge_window_ignores_observations_sent_by_chrome() {
        let mut chrome = observation(
            "browser",
            "2026-11-21T12:00:05Z",
            Some("app.hubspot.com"),
            None,
        );
        chrome.app = Some("Google Chrome".into());
        let mut edge = observation(
            "browser",
            "2026-11-21T12:00:06Z",
            Some("mail.google.com"),
            None,
        );
        edge.app = Some("Microsoft Edge".into());
        let observations = vec![chrome, edge];
        let parsed = parse_observations(&observations);
        let (start, end) = (at("2026-11-21T12:00:00Z"), at("2026-11-21T12:02:00Z"));
        assert_eq!(
            dominant_context("msedge.exe", start, end, &parsed)
                .domain
                .as_deref(),
            Some("mail.google.com")
        );
        assert_eq!(
            dominant_context("Google Chrome", start, end, &parsed)
                .domain
                .as_deref(),
            Some("app.hubspot.com")
        );
    }

    #[test]
    fn an_empty_observation_ends_the_previous_project() {
        let observations = vec![
            observation("editor", "2026-11-21T12:00:00Z", None, Some("old-project")),
            observation("editor", "2026-11-21T12:00:20Z", None, None),
        ];
        let parsed = parse_observations(&observations);
        let context = dominant_context(
            "Cursor",
            at("2026-11-21T12:00:00Z"),
            at("2026-11-21T12:02:00Z"),
            &parsed,
        );
        assert_eq!(context, BlockContext::default());
    }

    #[test]
    fn domains_match_subdomains_and_map_to_built_in_categories() {
        assert!(domain_matches("eu1.app.hubspot.com", "app.hubspot.com"));
        assert!(domain_matches("hubspot.com", "*.hubspot.com"));
        assert!(!domain_matches("nothubspot.com", "hubspot.com"));
        assert_eq!(
            built_in_domain_category("mail.google.com"),
            Some("communication")
        );
        assert_eq!(
            built_in_domain_category("acme.lightning.force.com"),
            Some("admin")
        );
        assert_eq!(built_in_domain_category("www.notion.so"), Some("creation"));
        assert_eq!(built_in_domain_category("example.com"), None);
    }
}
