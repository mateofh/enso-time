# 0.2.0 stable candidate acceptance

Status: source prepared; signing, installer acceptance, and pilot pending. No stable
tag or release is created by preparing this source. The published beta remains
`v0.2.0-beta.1`.

## Candidate preparation

1. Keep package.json, package-lock.json, Cargo.toml, Cargo.lock, and tauri.conf.json
   aligned to `0.2.0`. Run `npm run qa` on the committed source and inspect its clean
   tree flag; verify CI on all three platforms and review the candidate changes.
2. Configure the secrets listed in [QUALITY.md](QUALITY.md) in `macos-signing` and
   `windows-signing`. Both environments require a reviewer and permit main/release
   tags. Keep certificates, private keys, and passwords out of Git and shared reports.
3. Integrate the approved source. Create `v0.2.0` only after signing setup is ready.
   Approve the signing build jobs, then download all four signed installer artifacts
   from that one run. Preserve their exact hashes and source commit.
4. Leave the protected publishing job unapproved during acceptance. Test those exact
   artifacts; record the full acceptance matrix from QUALITY.md. The installed-app
   smoke checks do not substitute for permission-dependent capture testing.
5. Complete the pilot below, resolve blocking incidents, and complete review evidence
   for the final source. Set `STABLE_QA_EVIDENCE` in the protected `release` environment
   using [qa-evidence.example.json](qa-evidence.example.json), actual hashes, and results.
6. Approve publication only after all requirements pass. A changed source or rebuilt
   installer invalidates previous acceptance evidence and requires retesting.

## Five-person, seven-day pilot

Assign consenting participants to these slots before starting. These are planned slots,
not enrolled participants or completed results. Record actual OS versions and the exact
installer hash privately with the release owner.

| Slot | Coverage | Enrollment | Acceptance |
| --- | --- | --- | --- |
| P01 | Apple Silicon Mac; declared minimum supported macOS | pending | pending |
| P02 | Apple Silicon Mac; participant's current macOS | pending | pending |
| P03 | Intel Mac; supported macOS | pending | pending |
| P04 | Windows 10 x64 | pending | pending |
| P05 | Windows 11 x64 | pending | pending |

Each participant uses the candidate for seven full days. Capture, diagnostic accuracy,
and recovery from interruptions are observed throughout the pilot.

| Day | Focus and evidence |
| --- | --- |
| Setup | Consent, OS/version/hash record, signature verification, clean installation, initial permissions, and synthetic test data. |
| 1 | Timed log of application changes and AFK; compare captured intervals and identify gaps or duplicates. |
| 2 | Pause/resume from window and tray; deny/revoke/regrant permissions and confirm visible recovery. |
| 3 | Diagnosis → intake → blueprint → copy/export; check evidence, assumptions, stale/conflict states, and drafts. |
| 4 | Synthetic export, retention, and deletion; inspect local storage and outbound network behavior. Keep the pilot's retained evidence separate. |
| 5 | Sleep/wake, quit/restart, reboot, scheduled pause expiry, and startup toggle. |
| 6 | Upgrade from published beta using synthetic saved settings/intakes/drafts; verify one app identity and preserved data. |
| 7 | ES/EN, theme, keyboard/focus, contrast, minimum window; reconcile every incident and complete the acceptance matrix. |

For each incident record: participant slot, timestamp, source/version/hash, OS, steps,
expected/actual result, severity, owner, evidence, fix, and retest result. Use synthetic
examples in shared screenshots. Zero blocking incidents, all required platform checks,
and code/security review without unresolved critical/high/medium findings are required.

## Current blockers

- Apple signing/notarization secrets are absent from the GitHub environment.
- Windows signing credentials are absent; its protected environment is configured.
- Signed candidate installers, real OS acceptance, and the pilot have not run.

Review and automated checks from the hardening PR are baseline evidence. The final
candidate must also be validated; a source-version change does not approve its binaries.
