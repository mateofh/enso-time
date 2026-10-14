# Independent review — release quality hardening

Reviewed on 2026-10-14 by an independent read-only code-review agent. The review covers
the integrated changes relative to `origin/main` at `4e0ac4f`, before the merge commit.

Verdict: **APPROVE**. No confirmed critical, high, medium, or low findings remain.
This is code review evidence for these changes, not stable-release acceptance.

## Scope

- Tracking pause/resume, tray actions, capture cancellation, and deletion coordination.
- UI refresh lifecycle, stale responses, error visibility, and draft preservation.
- Installer smoke isolation, scheduler maintenance, local connectors, and feature gates.
- Signing, notarization, installer version ordering, immutable source provenance,
  publication evidence, and automated QA.

## Corrected findings

| Finding | Correction | Verification |
| --- | --- | --- |
| Concurrent pause/resume could leave capture active while settings indicated paused. | Serialize tracking transitions and revalidate deletion generation after queuing; dispatch tray transitions off the UI thread. | Controlled concurrent pause/resume and queued-transition cancellation regression tests; independent inspection. |
| Smoke scheduler backup cleanup could access the normal profile. | Pass the selected application data root into the scheduler and use it for retention maintenance. | Independent inspection of setup and scheduler paths; integrated Rust checks. |

The reviewer independently ran `npm run test:release`: 14/14 passed. The responsible
agent ran full integrated QA: 199 tests passed, plus type checking, production build,
license checks, and workflow validation. Local smoke uses an ad-hoc debug bundle.

## Limits

Signed installers, notarization execution, real OS capture permissions, cross-platform
installer acceptance, and the five-person seven-day pilot were not certified by this
review. Those require their own evidence for the final stable source and installer hashes,
as defined in [QUALITY.md](QUALITY.md).
