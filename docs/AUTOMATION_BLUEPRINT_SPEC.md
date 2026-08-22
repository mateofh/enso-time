# Automation Blueprint v0.2 contract

Status: implementation contract for `v0.2.0-beta.1`
Tracking: [GitHub milestone](https://github.com/mateofh/enso-time/milestone/1)

## Outcome

Turn a completed `WorkflowIntakeV1` into one evidence-backed, vendor-neutral,
build-ready automation specification. The user remains in control: Ensō Time does not
connect accounts, collect credentials, call an LLM, send activity data, or execute an
external workflow.

`WorkflowIntakeV1` remains the editable source of truth. A blueprint references its
source intake and becomes stale when that intake changes.

## Wire contracts

Rust and TypeScript expose the same camelCase JSON contract.

```ts
type StepDispositionV1 = "unassessed" | "automatable" | "human-required";
type DataSensitivityV1 = "unassessed" | "low" | "moderate" | "high";
type SideEffectLevelV1 = "unassessed" | "none" | "reversible" | "irreversible";
type VerificationModeV1 = "unassessed" | "none" | "sample" | "every-run";

interface StepBoundaryV1 {
  stepId: string;
  disposition: StepDispositionV1;
  rationale?: string;
}

interface AutomationBlueprintDraftV1 {
  schemaVersion: 1;
  id: string;
  candidateId: string;
  workflowIntakeId: string;
  sourceIntakeUpdatedAt: string;
  revision: number;
  runsPerWeek?: number;
  stepBoundaries: StepBoundaryV1[];
  dataSensitivity: DataSensitivityV1;
  sideEffectLevel: SideEffectLevelV1;
  verificationMode: VerificationModeV1;
  verificationMinutesPerRun?: number;
  recoveryPlan?: string;
  createdAt: string;
  updatedAt: string;
}
```

The derived `AutomationBlueprintV1` contains the draft, a read-only source map,
aggregated evidence, `draft | ready | stale | blocked` status, completeness, estimates,
and locale-specific rendered Markdown/copy text. Rendered text is derived and is not
stored. Unknown schema versions are rejected.

The backend always rebuilds source and observed evidence. The frontend cannot alter
observed data or the source intake through blueprint commands.

## Deterministic assessment

Completeness is scored from 0 to 100:

| Input | Points |
| --- | ---: |
| Trigger, inputs, output, tools, steps | 30 (6 each) |
| Human decisions | 10 |
| Minutes/run and runs/week | 10 (5 each) |
| Classified step boundaries with rationale | 20 proportional |
| Data sensitivity and side effects | 10 (5 each) |
| Verification mode and minutes | 10 (5 each) |
| Recovery/exception plan | 10 |

`ready` requires all of the following:

- completeness 100;
- source intake is current;
- at least two repeated observations;
- at least one automatable step.

Let `M` be minutes per manual run, `W` declared runs/week, `N` total steps, `A`
automatable steps, `H = N - A`, and `V` verification minutes/run.

- weekly baseline: `W * M`;
- weekly residual: `W * ((M * H/N) + V)`;
- estimated weekly savings: `max(0, round(baseline - residual))`.

The equal-time-per-step assumption and verification overhead must always be visible.
Savings are planning estimates, never observed or guaranteed outcomes.

Effort, operational risk, and confidence use bounded `0..100` scores with visible
components. Their golden vectors are shared between Rust and TypeScript tests. Rust is
the source of truth; preview mode is explicitly labelled as a simulation.

## Persistence and concurrency

Create a separate table; do not rewrite `workflow_intakes`:

```sql
CREATE TABLE automation_blueprints (
  candidate_id TEXT PRIMARY KEY,
  id TEXT NOT NULL UNIQUE,
  schema_version INTEGER NOT NULL CHECK (schema_version = 1),
  workflow_intake_id TEXT NOT NULL REFERENCES workflow_intakes(id) ON DELETE CASCADE,
  source_intake_updated_at TEXT NOT NULL,
  revision INTEGER NOT NULL CHECK (revision >= 1),
  draft_json TEXT NOT NULL CHECK (length(draft_json) <= 40000),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX automation_blueprints_updated_idx
  ON automation_blueprints(updated_at DESC);
```

The migration is transactional and idempotent. It preserves all v0.1 state. Blueprint
writes use optimistic concurrency (`expectedRevision`); conflicts return the current
state and never silently overwrite a draft.

Every blueprint read/write/export/delete command takes `CaptureGate.operation()`
before `LocalStore`. `delete_all` remains exclusive and removes blueprint rows,
SQLite/WAL/SHM traces, and app-managed exports before reporting success. Clipboard
contents, moved exports, backups, and synced copies remain outside Ensō Time's deletion
boundary and must be documented.

## Commands

```text
get_automation_blueprint_v1(candidateId, locale)
save_automation_blueprint_v1(input, locale)
refresh_automation_blueprint_source_v1(candidateId, expectedRevision, locale)
export_automation_blueprint_v1(candidateId, expectedRevision, format, locale)
delete_automation_blueprint_v1(candidateId, expectedRevision, confirmation)
```

`get` may return an ephemeral draft but does not create a row. If the intake timestamp
changes, status is `stale`; save, copy, and export remain disabled until explicit
refresh. Refresh preserves risk/frequency inputs but resets step classifications.

Export is local Markdown or JSON and is enabled only for `ready`. Files use generated
names, private permissions, `create_new`, and app-controlled directories. Copy is a
visible user gesture using the clipboard; it is not a backend side effect.

## Product journey

1. The existing recommendation remains the entry point.
2. A candidate without an intake continues to “Map workflow”.
3. A candidate with an intake can create or continue its blueprint.
4. The blueprint shows the current process as read-only evidence and asks only for
   missing frequency, step boundaries/rationales, sensitivity, side effects,
   verification, and recovery information.
5. A ready blueprint shows the human/automation boundary, estimates, assumptions,
   risk, confidence, copy, and export actions.

The journey includes Spanish (`es-419`) and English, desktop and mobile-width layouts,
keyboard/focus support, and loading, empty, stale, conflict, clipboard, and export
failures. User-entered content is never translated or rewritten.

There are no vendor links, OAuth fields, token inputs, execution buttons, or automatic
LLM calls in the blueprint journey.

## Release gates

- Golden-vector unit tests for completeness, estimates, risk, and confidence.
- Validation for bounds, enums, identifiers, RFC3339, Unicode controls, and size.
- SQLite migration, stale source, optimistic conflict, export, secure deletion, and
  delete/write-race tests.
- Vitest parity for wire models, renderer, preview persistence, stale/conflict, and
  copy/export guards.
- Playwright journeys in Spanish and English, including mobile, keyboard, failures,
  insufficient evidence, and deletion.
- macOS and Windows CI plus installer smoke tests.
- No unresolved critical, high, or medium code/security findings before release.
