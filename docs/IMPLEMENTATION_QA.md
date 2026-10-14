# Implementation validation

This record covers the modified local workspace for the stable-release preparation.
The application remains `0.2.0-beta.1`; this record is not approval to publish a stable
release. The working tree is uncommitted, which the generated QA report marks explicitly.

## Completed checks

- `npm run qa`: passed, with 23 Vitest tests, 50 browser journeys, 9 ActivityWatch
  packaging tests, 13 release/gate tests, and 63 Rust tests (158 total).
- TypeScript, frontend production build, strict license boundaries, and CRLF checks:
  passed as part of QA.
- GitHub workflows: parsed successfully and checked with actionlint.
- `git diff --check`: passed.
- macOS Apple Silicon debug application bundle: built successfully and launched through
  `installer-smoke.mjs`; real WebView initialization, Tauri IPC, local server, and SQLite
  write/read passed in an isolated temporary profile.

Local reports are generated in `node_modules/.cache/enso-time-qa/`, including
`report.json`, `checks.log`, and `native-local-final.json`. These files are ignored by
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
- Signing bundled macOS resources changes integrity hashes: signing refreshes the
  per-file manifest while preserving verified upstream archive provenance.
- Rust PATH discovery must preserve Windows's `Path` casing and existing tool paths:
  the wrapper updates the existing environment key.

## Still required before stable publication

The local native check used an ad-hoc debug bundle. Paid signing/notarization and
Authenticode execution are not verified here because signing credentials were not
provided. Windows MSI/NSIS installation and Mac Intel runtime checks are implemented
in the release workflow but have not been executed from this checkout.

Complete real OS capture/permission, upgrade, privacy, accessibility, and retention
acceptance on the supported platforms; run the five-person, seven-day pilot; obtain
independent code/security review. Record the final stable candidate's actual installer
hashes and source in the protected release environment's QA evidence. The publisher
rejects missing or mismatched evidence rather than declaring these checks complete.

Follow [QUALITY.md](QUALITY.md) for the full acceptance matrix and credential setup.
