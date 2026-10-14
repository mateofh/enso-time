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

## Validation coverage

The suite now exercises integrated Blueprint UI contracts through a mocked Tauri
transport, plus the existing preview journeys. It does not run a native webview or
OS capture. Both desktop and mobile-width projects discover the same journeys.

Coverage includes Spanish/English, keyboard/focus, loading/retry, native errors,
stale source, save/refresh conflicts, weak evidence, and confirmed deletion.
The base app suite also verifies periodic/focus refresh, retained unsaved edits,
visible refresh failure/recovery, and delayed-response invalidation after a pause.

Run `npm run qa` for the complete application/Rust/packaging/release checks.
Native installer checks and permission-dependent acceptance are described in
[`docs/QUALITY.md`](../../docs/QUALITY.md). Record current results from the generated
QA report rather than relying on historical counts in this document.
