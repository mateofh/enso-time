# Quality control and stable release

The current source targets the `0.2.0` stable candidate. It is not approved or published
as stable; the latest published version remains `0.2.0-beta.1`. Follow
[STABLE_CANDIDATE.md](STABLE_CANDIDATE.md) for the candidate acceptance plan.
Automation Blueprint is implemented. A passing browser suite proves the UI and
its bridge contracts; it does not prove that OS permissions or real capture work.

## Every change

Run `npm run qa` with Node 22 and stable Rust. The command checks TypeScript, unit
and browser journeys, the production frontend build, ActivityWatch packaging,
release workflow contracts, license boundaries (including CRLF), and Rust tests.
It fails at the first failing stage. A missing Rust toolchain is a failure, not a
skipped check. Live UI reads refresh every 15 seconds while visible and on focus;
a 10-second read timeout marks the status as unconfirmed and preserves drafts. `rustup which cargo` also supports machines whose toolchain is not
on PATH.

Reports and logs are in `node_modules/.cache/enso-time-qa/`. The report includes the
version, commit, dirty-worktree flag, OS/architecture, timestamps, and exit codes.
CI runs on macOS Apple Silicon, macOS Intel, and Windows x64 and uploads reports
and browser failure traces. Inspect the dirty flag: an uncommitted local run does
not certify the commit named in the report.

## Installed application

The tag workflow checks immutable source, creates the installers, inspects their
resources, and launches the installed application. macOS runs a copy from the DMG;
Windows installs, launches, and uninstalls both MSI and NSIS packages. Stable builds
also verify Developer ID/notarization and Authenticode signatures.

The executable supports:

```bash
node scripts/installer-smoke.mjs --app PATH_TO_EXECUTABLE --report PATH_TO_NEW_JSON
```

This opens a real webview and requires successful IPC, initial screen data loading,
local server readiness, and a SQLite write/read. Its data directory is a private,
random temporary directory, removed on exit. It never starts capture before consent
and bypasses single-instance activation so it does not wake a normal instance. The
runner fails on a crash, mismatched version/source, absent report, or a 45-second
timeout. It does **not** claim to validate permission-dependent window/AFK capture.

## Human acceptance and pilot

For each of `macos-arm64`, `macos-x86_64`, and `windows-x86_64`, record the OS version
and pass all these checks using the exact final installer:

| Check | Acceptance |
| --- | --- |
| `cleanInstall` | Install and open on a clean machine; no unexpected elevated permission |
| `upgradeFromBeta` | Upgrade the published beta with synthetic saved data; keep settings, intakes, drafts, and one app identity |
| `signatures` | Validate signature/notarization of the delivered files |
| `captureAndAfk` | Compare application changes and away periods with a timed activity log; no unexplained gaps or duplicate intervals |
| `permissionsDenied` | Deny/revoke capture access; visible degraded state; recover after granting it |
| `pauseResume` | Pause/resume from both window and tray; no false success on failures |
| `suspendRestart` | Sleep, wake, quit, relaunch, reboot, and automatic pause expiry preserve accurate state |
| `autostart` | Enable/disable startup and verify after login |
| `fullBlueprintJourney` | Activity → diagnosis → intake → blueprint → explicit copy/export; weak evidence/stale/conflict states block export |
| `exportRetentionDeletion` | Inspect JSON/CSV/Markdown; exercise retention and deletion; document clipboard/moved export/backup limits |
| `localPrivacy` | Inspect network activity during capture/diagnosis/export; no unexpected outbound requests; local server only on loopback |
| `languagesKeyboardContrast` | ES/EN, light/dark, keyboard/focus, contrast, and minimum native window; user-entered content remains unchanged |

Test the minimum declared OS versions as well as the actual pilot versions before
claiming support for them. Windows 10 and 11 both need acceptance evidence.
Use only synthetic sensitive examples in shared screenshots and failure reports.

Run a seven-day pilot with at least five participants covering all three targets.
Log incidents with version, installer hash, OS, reproduction steps, expected result,
severity, and evidence. Record zero blocking issues before release. Complete code
and security reviews with no unresolved critical, high, or medium findings.

## Signing and publication gate

Retain the protected `macos-signing` environment for Apple certificate access and
API-key notarization. Use a separate protected `windows-signing` environment for
stable Windows certificate access, and retain `release` for publication. Limit both to trusted
release tags/main backfill and require a reviewer. No credential belongs in Git or a
QA artifact.

The platform signing environments need these secrets:

- macOS: `APPLE_CERTIFICATE` (base64 Developer ID Application P12),
  `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_API_ISSUER`,
  `APPLE_API_KEY`, `APPLE_API_KEY_CONTENT` (P8 contents), `KEYCHAIN_PASSWORD`.
- Windows: `WINDOWS_CERTIFICATE` (base64 PFX),
  `WINDOWS_CERTIFICATE_PASSWORD`, `WINDOWS_CERTIFICATE_THUMBPRINT`.

Use a signing provider/certificate that permits CI signing. See the official
[Tauri macOS signing](https://v2.tauri.app/distribute/sign/macos/) and
[Windows signing](https://v2.tauri.app/distribute/sign/windows/) instructions.
macOS capture is native and does not package Python watcher runtimes. Apple
signing and notarization remain required for beta and stable releases. A missing
certificate or failed verification blocks the build. Windows beta installers remain
unsigned; stable installers require Authenticode signing.

Before approving the stable publishing job, set the `release` environment variable
`STABLE_QA_EVIDENCE` to the completed JSON based on
[`qa-evidence.example.json`](qa-evidence.example.json). Download the candidate build
artifacts, test those files, and copy their **actual** names/hashes and the full source
SHA into the record. Keep the supporting pilot log and review evidence with the
release owner. The example is deliberately pending and cannot pass the gate.

The publishing job compares this record with the four final installer checksums,
source commit, platform checks, reviews, and pilot. If anything changes, retest the
new files and replace the evidence. Rebuilding signed files can change their hashes;
prefer completing acceptance while the publishing job awaits review rather than
rebuilding a previously tested candidate.

The candidate's package.json, package-lock.json, Cargo.toml/Cargo.lock, and tauri.conf.json
are aligned to `0.2.0`. Verify QA on the final source, configure signing credentials,
and tag that exact source to build the installers. Its publishing job must wait for
the evidence above. The
**stable-version candidate must be built and retested**;
approval of beta binaries cannot approve stable binaries.

Windows keeps the existing UpgradeCode. Installer version fields are distinct from
application SemVer: MSI uses `major.minor.(patch*100 + ordinal)` (beta ordinal 1–98,
stable 99), and NSIS uses the base version with numeric prerelease ordinal (stable
65535). This orders upgrades from published betas and subsequent releases. Windows
bounds are validated before packaging. Application manifests retain their SemVer.

Calendar import and external automation execution remain outside this release.
