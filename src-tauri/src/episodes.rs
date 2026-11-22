//! Groups timeline blocks into work episodes (one sitting on one task) and derives the
//! observed signal process mapping needs: steps, handoffs between apps/sites, and variants.
//!
//! Pure functions over `TimelineBlockV1`. Thresholds are hypotheses to calibrate with
//! real use; each one is a named constant.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};

use crate::models::{canonicalize_candidate_label, TimelineBlockV1};

/// Shorter visits are pass-throughs (Cmd+Tab, a glance) and are not steps.
pub const MIN_DWELL_SECONDS: i64 = 4;
/// A pause longer than this without capture ends the episode.
pub const MAX_GAP_SECONDS: i64 = 120;
/// Work on a different task must last this long to start a new episode; shorter
/// excursions (checking a message mid-task) stay inside the current one.
pub const SUSTAINED_SWITCH_SECONDS: i64 = 120;
/// Absorbed excursions may make up at most this share of an episode's touch time;
/// beyond it the person is interleaving two tasks, not taking a detour.
pub const MAX_EXCURSION_SHARE: f64 = 0.2;
/// Variants are only reported once a task has this many observed episodes.
pub const MIN_EPISODES_FOR_VARIANTS: usize = 3;
/// Steps kept per variant signature.
pub const MAX_VARIANT_STEPS: usize = 8;
/// Handoffs reported in the diagnostic, and the minimum repetitions for one to count.
pub const MAX_HANDOFFS: usize = 5;
pub const MIN_HANDOFF_COUNT: usize = 2;
/// End-of-day labeling offers only episodes at least this long, and at most this many.
pub const MIN_LABEL_SUGGESTION_SECONDS: i64 = 300;
pub const MAX_LABEL_SUGGESTIONS: usize = 5;
/// Most window blocks one labeling request may carry; larger episodes are not offered.
pub const MAX_EPISODE_LABEL_BLOCKS: usize = 5_000;

#[derive(Debug, Clone, PartialEq)]
pub struct Episode {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    /// Canonical candidate label when the episode holds confirmed evidence.
    pub candidate_key: Option<String>,
    /// Confirmed label, else `category:<id>`; `None` when every block was unknown.
    pub task_key: Option<String>,
    /// Started after AFK, a declared break, an excluded app or a long gap.
    pub after_break: bool,
    /// Window blocks that make up the episode (manual check-in blocks excluded).
    pub block_ids: Vec<String>,
    /// Distinct hashed case identifiers seen in the episode's blocks.
    pub case_refs: Vec<String>,
    /// Consecutive distinct steps (domain, else app), e.g. `[mail.google.com, excel, app.hubspot.com]`.
    pub steps: Vec<String>,
    pub touch_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffV1 {
    pub from: String,
    pub to: String,
    pub count: usize,
}

/// What the capture observed for one candidate, kept apart from what the person declared.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservedEvidenceV1 {
    pub episodes: usize,
    pub observed_days: usize,
    pub median_minutes: f64,
    /// Present only from `MIN_EPISODES_FOR_VARIANTS` episodes on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variants: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_variant_share: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub top_variant_steps: Vec<String>,
    /// Distinct cases (hashed ticket/invoice/order ids) worked on in this task's
    /// episodes. Present only when window titles are kept and identifiers appear.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cases: Option<usize>,
}

/// A contiguous run of blocks with the same step and task key.
struct Run {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    block_ids: Vec<String>,
    case_refs: Vec<String>,
    step: Option<String>,
    candidate_key: Option<String>,
    /// Label if confirmed, else category; `None` for unknown blocks, which join any task.
    task_key: Option<String>,
}

impl Run {
    fn seconds(&self) -> i64 {
        (self.end - self.start).num_seconds()
    }

    /// AFK, declared breaks and excluded apps end an episode and are never steps.
    fn is_break(&self) -> bool {
        self.step.is_none()
    }
}

struct Draft {
    episode: Episode,
    task_key: Option<String>,
    after_break: bool,
    /// Touch seconds that belong to absorbed excursions, not to the episode's task.
    excursion_seconds: i64,
}

fn step_identity(block: &TimelineBlockV1) -> Option<String> {
    // A break declared by the person (check-in or correction) ends work like AFK does.
    if block.afk
        || block.category == "break"
        || block.app == crate::process_context::EXCLUDED_APP_LABEL
    {
        return None;
    }
    block
        .context_domain
        .clone()
        .or_else(|| Some(crate::process_context::app_identity(&block.app)))
        .filter(|step| !step.is_empty())
}

fn runs_from_blocks(blocks: &[TimelineBlockV1]) -> Vec<Run> {
    let mut timed = blocks
        .iter()
        // Manual check-in blocks overlap window blocks in time; they are not a sequence.
        .filter(|block| !block.id.starts_with("manual-"))
        .filter_map(|block| {
            let start = crate::models::parse_utc(&block.start).ok()?;
            let end = crate::models::parse_utc(&block.end).ok()?;
            (end > start).then_some((start, end, block))
        })
        .collect::<Vec<_>>();
    timed.sort_by_key(|(start, _, _)| *start);

    let mut runs: Vec<Run> = Vec::new();
    for (start, end, block) in timed {
        let candidate_key = crate::diagnostics::candidate_label_for_block(block)
            .map(|label| canonicalize_candidate_label(&label));
        let task_key = candidate_key.clone().or_else(|| {
            (!matches!(block.category.as_str(), "unknown" | "break"))
                .then(|| format!("category:{}", block.category))
        });
        let step = step_identity(block);
        // Segments split at check-in/override/AFK boundaries are merged back before the
        // dwell filter, so fragments of one long visit are not dropped.
        if let Some(last) = runs.last_mut() {
            let contiguous = (start - last.end).num_seconds() <= 1;
            if contiguous && last.step == step && last.task_key == task_key {
                last.end = last.end.max(end);
                last.block_ids.push(block.id.clone());
                add_unique(&mut last.case_refs, &block.case_refs);
                continue;
            }
        }
        runs.push(Run {
            start,
            end,
            block_ids: vec![block.id.clone()],
            case_refs: block.case_refs.clone(),
            step,
            candidate_key,
            task_key,
        });
    }
    runs.retain(|run| run.is_break() || run.seconds() >= MIN_DWELL_SECONDS);
    runs
}

fn add_unique(target: &mut Vec<String>, items: &[String]) {
    for item in items {
        if !target.contains(item) {
            target.push(item.clone());
        }
    }
}

fn push_step(episode: &mut Episode, run: &Run) {
    if let Some(step) = &run.step {
        if episode.steps.last() != Some(step) {
            episode.steps.push(step.clone());
        }
    }
    episode.end = episode.end.max(run.end);
    episode.touch_seconds += run.seconds();
    episode.block_ids.extend(run.block_ids.iter().cloned());
    add_unique(&mut episode.case_refs, &run.case_refs);
    if episode.candidate_key.is_none() {
        episode.candidate_key = run.candidate_key.clone();
    }
}

fn merge_into(target: &mut Draft, next: Draft) {
    for step in next.episode.steps {
        if target.episode.steps.last() != Some(&step) {
            target.episode.steps.push(step);
        }
    }
    target.episode.end = target.episode.end.max(next.episode.end);
    target.episode.touch_seconds += next.episode.touch_seconds;
    target.episode.block_ids.extend(next.episode.block_ids);
    add_unique(&mut target.episode.case_refs, &next.episode.case_refs);
    target.excursion_seconds += next.excursion_seconds;
    if target.task_key.is_none() {
        target.task_key = next.task_key;
        target.episode.candidate_key = next.episode.candidate_key;
    }
}

pub fn build_episodes(blocks: &[TimelineBlockV1]) -> Vec<Episode> {
    let mut drafts: Vec<Draft> = Vec::new();
    let mut broken = true;
    for run in runs_from_blocks(blocks) {
        if run.is_break() {
            broken = true;
            continue;
        }
        let continues = drafts.last().is_some_and(|current| {
            !broken
                && (run.start - current.episode.end).num_seconds() <= MAX_GAP_SECONDS
                && (run.task_key.is_none()
                    || current.task_key.is_none()
                    || current.task_key == run.task_key)
        });
        let gap_break = drafts.last().is_none_or(|current| {
            (run.start - current.episode.end).num_seconds() > MAX_GAP_SECONDS
        });
        if continues {
            let current = drafts.last_mut().expect("checked above");
            if current.task_key.is_none() {
                current.task_key = run.task_key.clone();
            }
            push_step(&mut current.episode, &run);
        } else {
            let mut episode = Episode {
                start: run.start,
                end: run.start,
                candidate_key: None,
                task_key: None,
                after_break: false,
                block_ids: Vec::new(),
                case_refs: Vec::new(),
                steps: Vec::new(),
                touch_seconds: 0,
            };
            push_step(&mut episode, &run);
            drafts.push(Draft {
                episode,
                task_key: run.task_key.clone(),
                after_break: broken || gap_break,
                excursion_seconds: 0,
            });
        }
        broken = false;
    }
    absorb_short_excursions(drafts)
        .into_iter()
        .map(|draft| Episode {
            task_key: draft.task_key,
            after_break: draft.after_break,
            ..draft.episode
        })
        .collect()
}

/// A different task shorter than `SUSTAINED_SWITCH_SECONDS` between two stretches of the
/// same task is an excursion (A, B, A becomes one A episode). Only a true sandwich is
/// absorbed: otherwise rapid switching would chain into one endless episode.
fn absorb_short_excursions(drafts: Vec<Draft>) -> Vec<Draft> {
    let mut merged: Vec<Draft> = Vec::new();
    for draft in drafts {
        if !draft.after_break && merged.len() >= 2 {
            let excursion = &merged[merged.len() - 1];
            let resumed = &merged[merged.len() - 2];
            let short = (excursion.episode.end - excursion.episode.start).num_seconds()
                < SUSTAINED_SWITCH_SECONDS;
            let foreign = resumed.excursion_seconds + excursion.episode.touch_seconds;
            let total = resumed.episode.touch_seconds
                + excursion.episode.touch_seconds
                + draft.episode.touch_seconds;
            let minor = (foreign as f64) < total as f64 * MAX_EXCURSION_SHARE;
            if short && minor && !excursion.after_break && resumed.task_key == draft.task_key {
                let excursion = merged.pop().expect("length checked");
                let excursion_touch = excursion.episode.touch_seconds;
                let resumed = merged.last_mut().expect("length checked");
                merge_into(resumed, excursion);
                resumed.excursion_seconds += excursion_touch;
                merge_into(resumed, draft);
                continue;
            }
        }
        match merged.last_mut() {
            Some(previous) if !draft.after_break && previous.task_key == draft.task_key => {
                merge_into(previous, draft)
            }
            _ => merged.push(draft),
        }
    }
    merged
}

/// Moves from one task to a different one without a pause in between. Window changes
/// inside a task (Gmail → Sheets for the same invoice) are not switches.
pub fn task_switches(episodes: &[Episode]) -> usize {
    episodes
        .windows(2)
        .filter(|pair| {
            let continuous = !pair[1].after_break
                && (pair[1].start - pair[0].end).num_seconds() <= MAX_GAP_SECONDS;
            continuous
                && pair[0].task_key.is_some()
                && pair[1].task_key.is_some()
                && pair[0].task_key != pair[1].task_key
        })
        .count()
}

/// Longest episodes with no confirmed label, offered for one-click labeling at day's end.
pub fn unlabeled_suggestions(episodes: &[Episode]) -> Vec<&Episode> {
    let mut suggestions = episodes
        .iter()
        .filter(|episode| {
            episode.candidate_key.is_none()
                && !episode.steps.is_empty()
                && episode.touch_seconds >= MIN_LABEL_SUGGESTION_SECONDS
                && episode.block_ids.len() <= MAX_EPISODE_LABEL_BLOCKS
        })
        .collect::<Vec<_>>();
    suggestions.sort_by(|left, right| {
        right
            .touch_seconds
            .cmp(&left.touch_seconds)
            .then_with(|| left.start.cmp(&right.start))
    });
    suggestions.truncate(MAX_LABEL_SUGGESTIONS);
    suggestions
}

/// Most repeated step-to-step handoffs inside episodes (swivel-chair candidates).
pub fn top_handoffs(episodes: &[Episode]) -> Vec<HandoffV1> {
    let mut counts: HashMap<(&str, &str), usize> = HashMap::new();
    for episode in episodes {
        for pair in episode.steps.windows(2) {
            *counts.entry((&pair[0], &pair[1])).or_default() += 1;
        }
    }
    let mut handoffs = counts
        .into_iter()
        .filter(|(_, count)| *count >= MIN_HANDOFF_COUNT)
        .map(|((from, to), count)| HandoffV1 {
            from: from.into(),
            to: to.into(),
            count,
        })
        .collect::<Vec<_>>();
    handoffs.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.from.cmp(&right.from))
            .then_with(|| left.to.cmp(&right.to))
    });
    handoffs.truncate(MAX_HANDOFFS);
    handoffs
}

/// Distinct steps in order of first use: alternations (A, B, A) do not create variants.
fn variant_signature(steps: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    steps
        .iter()
        .filter(|step| seen.insert(step.as_str()))
        .take(MAX_VARIANT_STEPS)
        .cloned()
        .collect()
}

/// Observed evidence per canonical candidate label.
pub fn observed_evidence(episodes: &[Episode]) -> HashMap<String, ObservedEvidenceV1> {
    let mut by_key: HashMap<&str, Vec<&Episode>> = HashMap::new();
    for episode in episodes {
        if let Some(key) = episode.candidate_key.as_deref() {
            by_key.entry(key).or_default().push(episode);
        }
    }
    by_key
        .into_iter()
        .map(|(key, items)| (key.to_string(), evidence_for(&items)))
        .collect()
}

fn evidence_for(items: &[&Episode]) -> ObservedEvidenceV1 {
    let days = items
        .iter()
        .map(|episode| episode.start.with_timezone(&Local).date_naive())
        .collect::<HashSet<_>>()
        .len();
    let mut minutes = items
        .iter()
        .map(|episode| episode.touch_seconds as f64 / 60.0)
        .collect::<Vec<_>>();
    minutes.sort_by(f64::total_cmp);
    let middle = minutes.len() / 2;
    let median = if minutes.len() % 2 == 0 {
        (minutes[middle - 1] + minutes[middle]) / 2.0
    } else {
        minutes[middle]
    };
    let mut evidence = ObservedEvidenceV1 {
        episodes: items.len(),
        observed_days: days,
        median_minutes: (median * 10.0).round() / 10.0,
        variants: None,
        top_variant_share: None,
        top_variant_steps: Vec::new(),
        cases: Some(
            items
                .iter()
                .flat_map(|episode| episode.case_refs.iter())
                .collect::<HashSet<_>>()
                .len(),
        )
        .filter(|cases| *cases > 0),
    };
    if items.len() >= MIN_EPISODES_FOR_VARIANTS {
        let mut variants: HashMap<Vec<String>, usize> = HashMap::new();
        for episode in items {
            *variants
                .entry(variant_signature(&episode.steps))
                .or_default() += 1;
        }
        let (top_steps, top_count) = variants
            .iter()
            .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))
            .map(|(steps, count)| (steps.clone(), *count))
            .unwrap_or_default();
        evidence.variants = Some(variants.len());
        evidence.top_variant_share =
            Some((top_count as f64 / items.len() as f64 * 100.0).round() / 100.0);
        evidence.top_variant_steps = top_steps;
    }
    evidence
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "2026-11-21T12:00:00Z";

    fn at(seconds: i64) -> String {
        (crate::models::parse_utc(BASE).unwrap() + chrono::Duration::seconds(seconds)).to_rfc3339()
    }

    struct B<'a> {
        app: &'a str,
        domain: Option<&'a str>,
        from: i64,
        to: i64,
        label: Option<&'a str>,
        category: &'a str,
        source: &'a str,
        afk: bool,
    }

    fn b(app: &'static str, from: i64, to: i64) -> B<'static> {
        B {
            app,
            domain: None,
            from,
            to,
            label: None,
            category: "unknown",
            source: "unknown",
            afk: false,
        }
    }

    impl<'a> B<'a> {
        fn label(mut self, label: &'a str) -> Self {
            self.label = Some(label);
            self.category = "admin";
            self.source = "rule";
            self
        }
        fn domain(mut self, domain: &'a str) -> Self {
            self.domain = Some(domain);
            self
        }
        fn category(mut self, category: &'a str) -> Self {
            self.category = category;
            self.source = "builtin";
            self
        }
        fn afk(mut self) -> Self {
            self.afk = true;
            self.category = "break";
            self.source = "afk";
            self
        }
        fn block(&self, id: &str) -> TimelineBlockV1 {
            TimelineBlockV1 {
                id: id.into(),
                start: at(self.from),
                end: at(self.to),
                duration_seconds: self.to - self.from,
                app: self.app.into(),
                title: None,
                activity_label: self.label.map(Into::into),
                category: self.category.into(),
                confidence: 1.0,
                afk: self.afk,
                source: "window".into(),
                check_in_id: None,
                classification_source: self.source.into(),
                rule_id: None,
                correctable: true,
                context_domain: self.domain.map(Into::into),
                context_project: None,
                case_refs: Vec::new(),
            }
        }
    }

    fn blocks(items: &[B<'_>]) -> Vec<TimelineBlockV1> {
        items
            .iter()
            .enumerate()
            .map(|(index, item)| item.block(&format!("b{index}")))
            .collect()
    }

    #[test]
    fn alternating_between_two_apps_for_one_task_is_one_episode() {
        let episodes = build_episodes(&blocks(&[
            b("Google Chrome", 0, 60)
                .domain("mail.google.com")
                .label("Facturar"),
            b("Microsoft Excel", 60, 180).label("Facturar"),
            b("Google Chrome", 180, 200)
                .domain("mail.google.com")
                .label("Facturar"),
            b("Microsoft Excel", 200, 400).label("Facturar"),
        ]));
        assert_eq!(episodes.len(), 1);
        assert_eq!(
            episodes[0].steps,
            vec![
                "mail.google.com",
                "microsoft excel",
                "mail.google.com",
                "microsoft excel"
            ]
        );
        assert_eq!(episodes[0].candidate_key.as_deref(), Some("facturar"));
        assert_eq!(episodes[0].touch_seconds, 400);
    }

    #[test]
    fn a_short_excursion_stays_inside_but_a_sustained_switch_starts_a_new_episode() {
        let episodes = build_episodes(&blocks(&[
            b("Excel", 0, 300).label("Facturar"),
            b("Slack", 300, 360).category("communication"),
            b("Excel", 360, 600).label("Facturar"),
            b("Figma", 600, 1200).label("Diseñar"),
        ]));
        assert_eq!(episodes.len(), 2);
        assert_eq!(episodes[0].steps, vec!["excel", "slack", "excel"]);
        assert_eq!(episodes[1].candidate_key.as_deref(), Some("diseñar"));
    }

    #[test]
    fn rapid_switching_between_tasks_does_not_chain_into_one_endless_episode() {
        let mut items = Vec::new();
        for index in 0..30 {
            let start = index * 60;
            items.push(if index % 2 == 0 {
                b("Excel", start, start + 60).label("Facturar")
            } else {
                b("Figma", start, start + 60).label("Diseñar")
            });
        }
        let episodes = build_episodes(&blocks(&items));
        // Equal interleaving is two tasks, not one task with detours.
        assert_eq!(episodes.len(), 30, "got {} episodes", episodes.len());
    }

    #[test]
    fn task_switches_ignore_window_changes_inside_a_task_and_pauses() {
        let episodes = build_episodes(&blocks(&[
            b("Google Chrome", 0, 300)
                .domain("mail.google.com")
                .label("Facturar"),
            b("Excel", 300, 600).label("Facturar"),
            b("Figma", 600, 1200).label("Diseñar"),
            b("Excel", 1200, 1500).label("Facturar"),
            b("Excel", 3000, 3300).label("Diseñar"),
        ]));
        assert_eq!(episodes.len(), 4);
        // Facturar → Diseñar → Facturar are switches; the move after a 25 min pause is not.
        assert_eq!(task_switches(&episodes), 2);

        let mut items = blocks(&[
            b("Excel", 0, 300).label("Facturar"),
            b("Excel", 300, 330),
            b("Figma", 330, 900).label("Diseñar"),
        ]);
        items[1].category = "break".into();
        items[1].classification_source = "checkin".into();
        // A short declared break is an interruption, not a task switch.
        assert_eq!(task_switches(&build_episodes(&items)), 0);
    }

    #[test]
    fn unlabeled_suggestions_offer_the_longest_unconfirmed_episodes_only() {
        let mut items = vec![
            b("Excel", 0, 900).label("Facturar"),
            b("Google Chrome", 3_000, 3_600)
                .domain("app.hubspot.com")
                .category("admin"),
            b("Slack", 3_600, 3_700),
            b("Google Chrome", 3_700, 4_200)
                .domain("app.hubspot.com")
                .category("admin"),
            b("Notes", 9_000, 9_100),
        ];
        for index in 0..8 {
            let start = 20_000 + index * 2_000;
            items.push(b("Figma", start, start + 400 + index * 10).category("creation"));
        }
        let episodes = build_episodes(&blocks(&items));
        let suggestions = unlabeled_suggestions(&episodes);
        assert_eq!(suggestions.len(), MAX_LABEL_SUGGESTIONS);
        // The CRM episode (1,200 s across 3 blocks) comes first; the labeled task and
        // the 100 s note never appear.
        assert_eq!(suggestions[0].touch_seconds, 1_200);
        assert_eq!(suggestions[0].block_ids, vec!["b1", "b2", "b3"]);
        assert!(suggestions
            .iter()
            .all(|episode| episode.candidate_key.is_none() && episode.touch_seconds >= 300));
    }

    #[test]
    fn cases_count_distinct_hashed_identifiers_across_a_tasks_episodes() {
        let mut items = blocks(&[
            b("Google Chrome", 0, 300)
                .domain("mail.google.com")
                .label("Facturar"),
            b("Excel", 300, 600).label("Facturar"),
            b("Excel", 5_000, 5_300).label("Facturar"),
        ]);
        items[0].case_refs = vec!["invoice:aaa".into()];
        items[1].case_refs = vec!["invoice:aaa".into()];
        items[2].case_refs = vec!["invoice:bbb".into()];
        let episodes = build_episodes(&items);
        assert_eq!(episodes.len(), 2);
        assert_eq!(episodes[0].case_refs, vec!["invoice:aaa"]);
        assert_eq!(observed_evidence(&episodes)["facturar"].cases, Some(2));

        let without = blocks(&[b("Excel", 0, 300).label("Facturar")]);
        assert_eq!(
            observed_evidence(&build_episodes(&without))["facturar"].cases,
            None
        );
    }

    #[test]
    fn dwell_filter_runs_after_merging_split_segments() {
        // One visit split into 3 s fragments by check-in boundaries must survive,
        // while a genuine 2 s pass-through is dropped.
        let episodes = build_episodes(&blocks(&[
            b("Excel", 0, 3).label("Facturar"),
            b("Excel", 3, 6).label("Facturar"),
            b("Finder", 6, 8).label("Facturar"),
            b("Excel", 8, 200).label("Facturar"),
        ]));
        assert_eq!(episodes.len(), 1);
        assert_eq!(episodes[0].steps, vec!["excel"]);
        // The merged 6 s visit survived: the episode starts at 0, not at the long block.
        assert_eq!(episodes[0].start, crate::models::parse_utc(&at(0)).unwrap());
        assert_eq!(episodes[0].touch_seconds, 198);
    }

    #[test]
    fn a_declared_break_splits_one_task_into_two_runs() {
        let mut items = blocks(&[
            b("Excel", 0, 600).label("Facturar"),
            b("Excel", 600, 1200),
            b("Excel", 1200, 1800).label("Facturar"),
        ]);
        items[1].category = "break".into();
        items[1].classification_source = "checkin".into();
        let episodes = build_episodes(&items);
        assert_eq!(episodes.len(), 2);
        assert!(episodes.iter().all(|episode| episode.touch_seconds == 600));
    }

    #[test]
    fn afk_excluded_apps_and_long_gaps_cut_episodes() {
        let episodes = build_episodes(&blocks(&[
            b("Excel", 0, 300).label("Facturar"),
            b("Away", 300, 900).afk(),
            b("Excel", 900, 1200).label("Facturar"),
            b(crate::process_context::EXCLUDED_APP_LABEL, 1200, 1300),
            b("Excel", 1300, 1600).label("Facturar"),
            b("Excel", 2000, 2300).label("Facturar"),
        ]));
        assert_eq!(episodes.len(), 4);
        assert!(episodes
            .iter()
            .all(|episode| episode.steps == vec!["excel".to_string()]));
    }

    #[test]
    fn manual_check_in_blocks_are_not_part_of_sequences() {
        let mut items = blocks(&[b("Excel", 0, 300).label("Facturar")]);
        let mut manual = b("Contexto manual", 0, 900)
            .label("Facturar")
            .block("manual-c1");
        manual.source = "checkin".into();
        items.push(manual);
        let episodes = build_episodes(&items);
        assert_eq!(episodes.len(), 1);
        assert_eq!(episodes[0].steps, vec!["excel"]);
    }

    #[test]
    fn episode_key_matches_the_candidate_id_used_by_scoring() {
        let block = b("Excel", 0, 300).label("  Conciliar Pagos ").block("x");
        let episodes = build_episodes(std::slice::from_ref(&block));
        let label = crate::diagnostics::candidate_label_for_block(&block).unwrap();
        assert_eq!(
            crate::models::automation_candidate_id(episodes[0].candidate_key.as_deref().unwrap()),
            crate::models::automation_candidate_id(&label)
        );
    }

    #[test]
    fn handoffs_count_repeated_pairs_and_variants_need_three_episodes() {
        let mut items = Vec::new();
        for day in 0..4 {
            let offset = day * 86_400;
            let crm = if day == 3 {
                "airtable.com"
            } else {
                "app.hubspot.com"
            };
            items.extend([
                b("Google Chrome", offset, offset + 120)
                    .domain("mail.google.com")
                    .label("Alta cliente"),
                b("Excel", offset + 120, offset + 300).label("Alta cliente"),
                b("Google Chrome", offset + 300, offset + 420)
                    .domain(crm)
                    .label("Alta cliente"),
            ]);
        }
        let episodes = build_episodes(&blocks(&items));
        assert_eq!(episodes.len(), 4);
        let handoffs = top_handoffs(&episodes);
        assert_eq!(
            handoffs,
            vec![
                HandoffV1 {
                    from: "mail.google.com".into(),
                    to: "excel".into(),
                    count: 4,
                },
                HandoffV1 {
                    from: "excel".into(),
                    to: "app.hubspot.com".into(),
                    count: 3,
                },
            ]
        );
        let evidence = observed_evidence(&episodes);
        let alta = &evidence["alta cliente"];
        assert_eq!(alta.episodes, 4);
        assert_eq!(alta.median_minutes, 7.0);
        assert_eq!(alta.variants, Some(2));
        assert_eq!(alta.top_variant_share, Some(0.75));
        assert_eq!(
            alta.top_variant_steps,
            vec!["mail.google.com", "excel", "app.hubspot.com"]
        );
        let two = observed_evidence(&episodes[..2]);
        assert_eq!(two["alta cliente"].variants, None);
    }
}
