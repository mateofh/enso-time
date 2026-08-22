# Automation Blueprint v0.2 E2E contract

`automation-blueprint.spec.ts` is the frontend hand-off for the contract in
`docs/AUTOMATION_BLUEPRINT_SPEC.md`. It deliberately uses accessible labels and
the following stable `data-testid` hooks:

| Hook | Contract |
| --- | --- |
| `blueprint-entry` | Candidate with a completed intake opens the blueprint |
| `blueprint-no-candidate` | Candidate/intake/evidence is insufficient; no blueprint can be created |
| `blueprint-panel` | Blueprint editor/preview landmark |
| `blueprint-status`, `blueprint-completeness`, `blueprint-evidence-count` | Status and deterministic assessment |
| `blueprint-source-map`, `blueprint-boundary`, `blueprint-assumptions` | Read-only evidence, boundaries, and visible estimate assumptions |
| `blueprint-save`, `blueprint-refresh`, `blueprint-refresh-confirmation` | Draft save and explicit stale-source refresh |
| `blueprint-copy`, `blueprint-export` | Enabled only for `ready`; visible gesture / local export |
| `blueprint-loading`, `blueprint-error`, `blueprint-retry`, `blueprint-conflict` | Loading, backend failure, retry, and optimistic conflict states |
| `blueprint-delete-preview` | Delete confirmation preview; must state the deletion boundary |

The fixture `tests/fixtures/automation-blueprint-vectors.json` is the shared
deterministic input/output set for completeness and estimates. Its effort,
risk, and confidence values are explicit golden expectations until the Rust
source-of-truth formula is wired into the frontend parity tests.

## Validation status

- `npx playwright test --list`: passes; 26 tests are discovered across desktop and mobile projects.
- `npx playwright test tests/e2e/app.spec.ts`: passes; 14 existing tests pass.
- `npx playwright test tests/e2e/automation-blueprint.spec.ts --project=desktop-1280x800`: 1 fixture test passes; 5 journey tests fail at the expected missing v0.2 hooks because the blueprint frontend/backend integration is being implemented in another worktree. No test is skipped or marked as fixme.

Run the complete matrix with:

```bash
npx playwright test tests/e2e/automation-blueprint.spec.ts
```

The journeys assert that English translates interface chrome only, preserves
Spanish user content, works at mobile width and with keyboard focus, exposes
loading/error/stale/conflict states, blocks copy/export before `ready`, and
does not expose vendor links, OAuth/token inputs, credentials, or execution
controls.
