//! What kind of answer Ensō can honestly give today, and what unlocks the next one.
//!
//! 1. Time map: where time goes, by app and category.
//! 2. Friction: repeated jumps between tools, as hypotheses to validate.
//! 3. Workflow candidates: a named task repeated on several days, with steps that say
//!    which site or project (not just "Chrome" or "Terminal").
//! 4. Use case: a declared workflow whose blueprint passed the `ready` gate.

use serde::Serialize;

const MIN_CAPTURED_DAYS: f64 = 3.0;
const MIN_ACTIVE_HOURS: f64 = 2.0;
const MIN_LABELED_EPISODES: f64 = 3.0;
const MIN_LABELED_DAYS: f64 = 2.0;
/// Share of browser/editor/terminal time whose site or project is known.
const MIN_STEP_CONTEXT_SHARE: f64 = 0.5;
/// Less browser/editor/terminal time than this needs no enrichment.
const MIN_CONTEXT_TOOL_SECONDS: i64 = 30 * 60;

#[derive(Debug, Clone, Default)]
pub struct ReadinessInputs {
    pub days_observed: f64,
    pub active_seconds: i64,
    /// Handoffs already filtered to the repeated ones.
    pub repeated_handoffs: usize,
    /// Best named task (not app usage): labeled episodes and the days they span.
    pub best_task_episodes: usize,
    pub best_task_days: usize,
    pub browser_seconds: i64,
    pub browser_domain_share: f64,
    pub project_seconds: i64,
    pub project_share: f64,
    /// A named task has a declared workflow intake.
    pub has_intake: bool,
    /// A stored blueprint passed the existing `ready` gate.
    pub blueprint_ready: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessStepV1 {
    /// `captured-days`, `active-hours`, `repeated-handoff`, `labeled-episodes`,
    /// `labeled-days`, `step-context`, `workflow-intake`, `blueprint-ready`.
    pub requirement: String,
    pub current: f64,
    pub target: f64,
    pub done: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessV1 {
    /// Highest level whose requirements (and all below) are met; 0 before any.
    pub level: u8,
    /// Requirements of the next level, with progress. Empty at level 4.
    pub next: Vec<ReadinessStepV1>,
}

fn step(requirement: &str, current: f64, target: f64) -> ReadinessStepV1 {
    ReadinessStepV1 {
        requirement: requirement.into(),
        current,
        target,
        done: current >= target,
    }
}

/// Weighted share of browser + editor/terminal time with a known site or project, as a
/// percentage. Full when there is too little of that time to matter.
fn step_context_percent(inputs: &ReadinessInputs) -> f64 {
    let tools = inputs.browser_seconds + inputs.project_seconds;
    if tools < MIN_CONTEXT_TOOL_SECONDS {
        return 100.0;
    }
    let covered = inputs.browser_seconds as f64 * inputs.browser_domain_share
        + inputs.project_seconds as f64 * inputs.project_share;
    (covered / tools as f64 * 100.0).round()
}

fn requirements(level: u8, inputs: &ReadinessInputs) -> Vec<ReadinessStepV1> {
    match level {
        1 => vec![step(
            "captured-days",
            (inputs.days_observed * 10.0).floor() / 10.0,
            MIN_CAPTURED_DAYS,
        )],
        2 => vec![
            step(
                "active-hours",
                (inputs.active_seconds as f64 / 360.0).floor() / 10.0,
                MIN_ACTIVE_HOURS,
            ),
            step(
                "repeated-handoff",
                inputs.repeated_handoffs.min(1) as f64,
                1.0,
            ),
        ],
        3 => vec![
            step(
                "labeled-episodes",
                inputs.best_task_episodes as f64,
                MIN_LABELED_EPISODES,
            ),
            step(
                "labeled-days",
                inputs.best_task_days as f64,
                MIN_LABELED_DAYS,
            ),
            step(
                "step-context",
                step_context_percent(inputs),
                MIN_STEP_CONTEXT_SHARE * 100.0,
            ),
        ],
        4 => vec![
            step(
                "workflow-intake",
                f64::from(u8::from(inputs.has_intake)),
                1.0,
            ),
            step(
                "blueprint-ready",
                f64::from(u8::from(inputs.blueprint_ready)),
                1.0,
            ),
        ],
        _ => Vec::new(),
    }
}

pub fn assess(inputs: &ReadinessInputs) -> ReadinessV1 {
    let mut level = 0;
    for candidate in 1..=4 {
        if requirements(candidate, inputs).iter().all(|item| item.done) {
            level = candidate;
        } else {
            break;
        }
    }
    ReadinessV1 {
        level,
        next: requirements(level + 1, inputs),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape of the real profile analyzed on 2026-11-22: lots of time and friction,
    /// no named tasks, no site or project context.
    fn friction_only() -> ReadinessInputs {
        ReadinessInputs {
            days_observed: 26.0,
            active_seconds: 100 * 3600,
            repeated_handoffs: 5,
            browser_seconds: 70 * 3600,
            project_seconds: 70 * 3600,
            ..ReadinessInputs::default()
        }
    }

    #[test]
    fn app_usage_without_named_tasks_stops_at_friction() {
        let readiness = assess(&friction_only());
        assert_eq!(readiness.level, 2);
        let pending = readiness
            .next
            .iter()
            .filter(|item| !item.done)
            .map(|item| item.requirement.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            pending,
            vec!["labeled-episodes", "labeled-days", "step-context"]
        );
    }

    #[test]
    fn named_repeated_task_with_context_reaches_workflow_candidates() {
        let inputs = ReadinessInputs {
            best_task_episodes: 4,
            best_task_days: 3,
            browser_domain_share: 0.8,
            project_share: 0.4,
            ..friction_only()
        };
        let readiness = assess(&inputs);
        assert_eq!(readiness.level, 3);
        assert_eq!(readiness.next[0].requirement, "workflow-intake");
        // Labels alone are not enough when steps are opaque.
        let opaque = ReadinessInputs {
            browser_domain_share: 0.0,
            project_share: 0.0,
            ..inputs
        };
        assert_eq!(assess(&opaque).level, 2);
    }

    #[test]
    fn use_case_needs_the_blueprint_gate_not_just_an_intake() {
        let inputs = ReadinessInputs {
            best_task_episodes: 4,
            best_task_days: 3,
            browser_domain_share: 0.8,
            project_share: 0.8,
            has_intake: true,
            ..friction_only()
        };
        assert_eq!(assess(&inputs).level, 3);
        let ready = ReadinessInputs {
            blueprint_ready: true,
            ..inputs
        };
        let readiness = assess(&ready);
        assert_eq!(readiness.level, 4);
        assert!(readiness.next.is_empty());
    }

    #[test]
    fn early_days_start_at_zero_and_little_tool_time_needs_no_context() {
        let early = assess(&ReadinessInputs {
            days_observed: 1.0,
            ..ReadinessInputs::default()
        });
        assert_eq!(early.level, 0);
        assert_eq!(early.next[0].requirement, "captured-days");
        assert_eq!(
            step_context_percent(&ReadinessInputs {
                browser_seconds: 600,
                ..ReadinessInputs::default()
            }),
            100.0
        );
    }
}
