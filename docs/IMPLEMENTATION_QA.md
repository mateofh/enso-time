# Implementation validation

This historical record covers the release hardening work validated as `0.2.0-beta.1`.
It is not approval to publish a stable release. The subsequent `0.2.0` source candidate
and its acceptance requirements are tracked in [STABLE_CANDIDATE.md](STABLE_CANDIDATE.md).
Validation below ran on the integrated working tree before the merge commit;
the generated report records that tree as dirty. CI validates the pushed commit separately.

## Completed checks

- `npm run qa`: passed, with 23 Vitest tests, 58 browser journeys, 10 ActivityWatch
  packaging tests, 14 release/gate tests, and 94 Rust tests (199 total).
- TypeScript, frontend production build, strict license boundaries, and CRLF checks:
  passed as part of QA.
- GitHub workflows: parsed successfully and checked with actionlint.
- `git diff --check`: passed.
- macOS Apple Silicon debug application bundle: built successfully and launched through
  `installer-smoke.mjs`; real WebView initialization, Tauri IPC, local server, and SQLite
  write/read passed in an isolated temporary profile.

Local reports are generated in `node_modules/.cache/enso-time-qa/`, including
`report.json`, `checks.log`, and native smoke reports. These files are ignored by
Git; CI uploads its own reports for its exact source and platform.

## Code and failure-path review

Reviewed the UI refresh lifecycle, settings/intake preservation, candidate evidence,
tray command parity, deletion coordination, smoke isolation, release versioning,
certificate handling, immutable source checks, and publication gates.

Resolved during implementation:

- Late UI reads could overwrite newer pause/save/delete state: generation guards now
  invalidate background and initial-load responses around mutations.
- A hung background read could indefinitely display healthy capture: timeout now exposes
  unconfirmed state and allows retries without resetting forms.
- Preview save/refresh/conflict assessment could use synthetic default observations:
  all bridge assessment paths now derive evidence from actual preview check-ins.
- Delayed tray failure recording could write after deletion: generation checks under the
  capture gate prevent that write.
- Windows installer fields could compare a stable release incorrectly against beta:
  bounded numeric ordering and the existing UpgradeCode are tested explicitly.
- Integrated the current native macOS capture and protected Apple API-key signing
  flow from main; obsolete Python-runtime signing is not used.
- Rust PATH discovery must preserve Windows's `Path` casing and existing tool paths:
  the wrapper updates the existing environment key.
- Concurrent pause/resume could mix the persisted pause state and active watchers:
  tracking transitions now serialize, with deletion generation checked after queuing.
  Regression tests exercise both ordering and cancellation.
- Smoke scheduler maintenance could prune backups in the normal user profile:
  the scheduler now receives the chosen application data directory explicitly.

An independent read-only code/security review approved the integrated tree with no
confirmed critical, high, medium, or low findings remaining. The reviewer independently
ran all 14 release-contract tests. See [CODE_REVIEW.md](CODE_REVIEW.md) for scope and limits.

## Still required before stable publication

The local native check used an ad-hoc debug bundle. Paid signing/notarization and
Authenticode execution are not verified here because signing credentials were not
provided. Windows MSI/NSIS installation and Mac Intel runtime checks are implemented
in the release workflow but have not been executed from this checkout.

Complete real OS capture/permission, upgrade, privacy, accessibility, and retention
acceptance on the supported platforms; run the five-person, seven-day pilot. The stable
candidate needs review evidence matching its final source. Record its actual installer
hashes and source in the protected release environment's QA evidence. The publisher
rejects missing or mismatched evidence rather than declaring these checks complete.

Follow [QUALITY.md](QUALITY.md) for the full acceptance matrix and credential setup.
