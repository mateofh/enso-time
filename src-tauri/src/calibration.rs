//! Threshold calibration against a real local profile. Ignored by default; it only runs
//! when pointed at a copy of the data:
//!
//! ```sh
//! ENSO_CALIBRATION_DIR=/path/to/copy cargo test --release calibration -- --ignored --nocapture
//! ```
//!
//! The directory holds `enso-time.sqlite` and `activitywatch/activitywatch.sqlite`. Output
//! is aggregate only: no titles, no identifier values. App names appear only in the
//! `unknown` breakdown, which stays on the local terminal.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

use chrono::{Duration, Local, Utc};

use crate::episodes::{self, Episode, EpisodeParams};
use crate::models::TimelineBlockV1;

const CALIBRATION_DAYS: i64 = 120;
const TOP_UNKNOWN_APPS: usize = 10;

struct Summary {
    episodes: usize,
    per_day: f64,
    median_minutes: f64,
    labeled_share: f64,
    categorized_share: f64,
    switches_per_hour: f64,
    candidates_with_variants: usize,
    dropped_share: f64,
    suggestions_per_day: f64,
}

fn active_seconds(blocks: &[TimelineBlockV1]) -> i64 {
    blocks
        .iter()
        .filter(|block| !block.afk && !block.id.starts_with("manual-"))
        .map(|block| block.duration_seconds)
        .sum()
}

fn active_days(blocks: &[TimelineBlockV1]) -> usize {
    blocks
        .iter()
        .filter(|block| !block.afk)
        .filter_map(|block| crate::models::parse_utc(&block.start).ok())
        .map(|start| start.with_timezone(&Local).date_naive())
        .collect::<HashSet<_>>()
        .len()
}

fn median(mut values: Vec<f64>) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|left, right| left.total_cmp(right));
    let middle = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}

fn summarize(
    blocks: &[TimelineBlockV1],
    items: &[Episode],
    days: usize,
    params: &EpisodeParams,
) -> Summary {
    let days = days.max(1) as f64;
    let touch: i64 = items.iter().map(|episode| episode.touch_seconds).sum();
    let touch_f = touch.max(1) as f64;
    let labeled: i64 = items
        .iter()
        .filter(|episode| episode.candidate_key.is_some())
        .map(|episode| episode.touch_seconds)
        .sum();
    let categorized: i64 = items
        .iter()
        .filter(|episode| episode.task_key.is_some())
        .map(|episode| episode.touch_seconds)
        .sum();
    let active = active_seconds(blocks).max(1) as f64;
    Summary {
        episodes: items.len(),
        per_day: items.len() as f64 / days,
        median_minutes: median(
            items
                .iter()
                .map(|episode| episode.touch_seconds as f64 / 60.0)
                .collect(),
        ),
        labeled_share: labeled as f64 / touch_f,
        categorized_share: categorized as f64 / touch_f,
        switches_per_hour: episodes::task_switches_with(items, params) as f64 / (touch_f / 3600.0),
        candidates_with_variants: episodes::observed_evidence(items)
            .values()
            .filter(|evidence| evidence.variants.is_some())
            .count(),
        dropped_share: (1.0 - touch as f64 / active).max(0.0),
        suggestions_per_day: items
            .iter()
            .filter(|episode| {
                episode.candidate_key.is_none()
                    && episode.touch_seconds >= episodes::MIN_LABEL_SUGGESTION_SECONDS
            })
            .count() as f64
            / days,
    }
}

fn print_row(label: &str, summary: &Summary) {
    println!(
        "| {label:<26} | {:>5} | {:>5.1} | {:>6.1} | {:>5.0}% | {:>5.0}% | {:>5.1} | {:>3} | {:>5.1}% | {:>5.1} |",
        summary.episodes,
        summary.per_day,
        summary.median_minutes,
        summary.labeled_share * 100.0,
        summary.categorized_share * 100.0,
        summary.switches_per_hour,
        summary.candidates_with_variants,
        summary.dropped_share * 100.0,
        summary.suggestions_per_day,
    );
}

fn print_unknown(blocks: &[TimelineBlockV1]) {
    let mut by_category: BTreeMap<String, i64> = BTreeMap::new();
    let mut unknown_apps: HashMap<String, i64> = HashMap::new();
    let mut with_domain = 0;
    let mut browser_seconds = 0;
    for block in blocks
        .iter()
        .filter(|block| !block.afk && !block.id.starts_with("manual-"))
    {
        *by_category.entry(block.category.clone()).or_default() += block.duration_seconds;
        let identity = crate::process_context::app_identity(&block.app);
        if crate::activitywatch::is_browser(&crate::activitywatch::normalize_match_text(&block.app))
        {
            browser_seconds += block.duration_seconds;
            if block.context_domain.is_some() {
                with_domain += block.duration_seconds;
            }
        }
        if block.category == "unknown" {
            *unknown_apps.entry(identity).or_default() += block.duration_seconds;
        }
    }
    let active = active_seconds(blocks).max(1) as f64;
    println!("\n## Time by category (active, window blocks)");
    for (category, seconds) in &by_category {
        println!(
            "- {category}: {:.1} h ({:.0}%)",
            *seconds as f64 / 3600.0,
            *seconds as f64 / active * 100.0
        );
    }
    println!(
        "- browser time with connector domain: {:.0}% of {:.1} h",
        with_domain as f64 / browser_seconds.max(1) as f64 * 100.0,
        browser_seconds as f64 / 3600.0
    );
    let mut apps = unknown_apps.into_iter().collect::<Vec<_>>();
    apps.sort_by_key(|(_, seconds)| -seconds);
    println!("\n## Top apps in `unknown`");
    for (app, seconds) in apps.into_iter().take(TOP_UNKNOWN_APPS) {
        println!(
            "- {app}: {:.1} h ({:.0}% of active)",
            seconds as f64 / 3600.0,
            seconds as f64 / active * 100.0
        );
    }
}

fn print_case_refs(blocks: &[TimelineBlockV1]) {
    let titled = blocks.iter().filter(|block| block.title.is_some()).count();
    let mut kinds: BTreeMap<String, (usize, HashSet<String>)> = BTreeMap::new();
    for block in blocks {
        let Some(title) = &block.title else { continue };
        for reference in crate::case_refs::extract(title, "calibration-only-salt") {
            let kind = reference.split(':').next().unwrap_or_default().to_string();
            let entry = kinds.entry(kind).or_default();
            entry.0 += 1;
            entry.1.insert(reference);
        }
    }
    println!(
        "\n## Case identifiers (titles present in {titled} of {} blocks)",
        blocks.len()
    );
    if kinds.is_empty() {
        println!("- none detected");
    }
    for (kind, (hits, distinct)) in kinds {
        println!("- {kind}: {hits} blocks, {} distinct", distinct.len());
    }
}

#[test]
fn median_averages_the_two_middle_values() {
    assert_eq!(median(vec![9.0, 1.0]), 5.0);
    assert_eq!(median(vec![3.0, 1.0, 2.0]), 2.0);
    assert_eq!(median(Vec::new()), 0.0);
}

#[test]
#[ignore = "needs ENSO_CALIBRATION_DIR pointing at a copy of a real profile"]
fn calibration_against_local_profile() {
    let dir = std::env::var_os("ENSO_CALIBRATION_DIR")
        .map(PathBuf::from)
        .expect("set ENSO_CALIBRATION_DIR to a copy of a real profile");
    for required in ["enso-time.sqlite", "activitywatch/activitywatch.sqlite"] {
        assert!(
            dir.join(required).is_file(),
            "{required} missing in {}",
            dir.display()
        );
    }
    let store = crate::storage::LocalStore::open(&dir.join("enso-time.sqlite")).unwrap();
    let activity = crate::activitywatch::ActivityWatchRuntime::start(
        &dir.join("activitywatch"),
        dir.join("no-resources"),
    )
    .unwrap();
    let to = Utc::now();
    let from = to - Duration::days(CALIBRATION_DAYS);
    let (from_s, to_s) = (from.to_rfc3339(), to.to_rfc3339());
    let settings = store.settings().unwrap();
    let titles_setting = settings.capture_window_titles;
    let checkins = store.checkins_between(&from_s, &to_s).unwrap();
    let rules = store.classification_rules().unwrap();
    let overrides = store
        .classification_overrides_between(&from_s, &to_s)
        .unwrap();
    let context = crate::intelligence::timeline_context_between(&store, &from_s, &to_s).unwrap();
    let blocks = activity
        .timeline_with_context(from, to, &settings, &checkins, &rules, &overrides, &context)
        .unwrap();
    assert!(
        active_seconds(&blocks) > 0,
        "no captured activity in the last {CALIBRATION_DAYS} days"
    );
    // Identifier patterns are counted on a separate titled read so the profile's own
    // settings (and title-based rules) still drive classification and episodes.
    let titled = if titles_setting {
        blocks.clone()
    } else {
        let mut with_titles = settings.clone();
        with_titles.capture_window_titles = true;
        activity
            .timeline_with_context(
                from,
                to,
                &with_titles,
                &checkins,
                &rules,
                &overrides,
                &context,
            )
            .unwrap()
    };
    let days = active_days(&blocks);
    println!(
        "# Calibration: {} blocks, {days} active days, {:.1} active h, {} rules, {} overrides, {} check-ins, {} context observations, titles setting {titles_setting}",
        blocks.len(),
        active_seconds(&blocks) as f64 / 3600.0,
        rules.len(),
        overrides.len(),
        checkins.len(),
        context.len(),
    );
    print_unknown(&blocks);
    print_case_refs(&titled);

    println!("\n## Sensitivity (one threshold at a time; default first)");
    println!("| params                     |   eps | eps/d | med m  | label | categ | sw/h  | var | drop   | sug/d |");
    println!("|---|---|---|---|---|---|---|---|---|---|");
    let base = EpisodeParams::default();
    let mut variants: Vec<(String, EpisodeParams)> = vec![("default".into(), base)];
    for dwell in [2, 8, 15] {
        variants.push((
            format!("dwell {dwell}s"),
            EpisodeParams {
                min_dwell_seconds: dwell,
                ..base
            },
        ));
    }
    for gap in [60, 300, 600] {
        variants.push((
            format!("gap {gap}s"),
            EpisodeParams {
                max_gap_seconds: gap,
                ..base
            },
        ));
    }
    for sustained in [60, 240] {
        variants.push((
            format!("excursion {sustained}s"),
            EpisodeParams {
                sustained_switch_seconds: sustained,
                ..base
            },
        ));
    }
    for share in [0.1, 0.35] {
        variants.push((
            format!("excursion share {share}"),
            EpisodeParams {
                max_excursion_share: share,
                ..base
            },
        ));
    }
    for (label, params) in variants {
        let items = episodes::build_episodes_with(&blocks, &params);
        print_row(&label, &summarize(&blocks, &items, days, &params));
    }

    if let Some(path) = std::env::var_os("ENSO_CALIBRATION_OCEL").map(PathBuf::from) {
        let items = episodes::build_episodes(&blocks);
        let ocel = crate::ocel::ocel_export(&items);
        std::fs::write(&path, serde_json::to_string_pretty(&ocel).unwrap()).unwrap();
        println!(
            "\nOCEL written: {} events",
            ocel["events"].as_array().unwrap().len()
        );
    }
    activity.shutdown().unwrap();
}
