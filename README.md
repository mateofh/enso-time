# Ensō Time

Ensō Time is a free, local-first desktop time tracker for personal use. It records
which applications receive your attention, asks only when a 15/30/60-minute interval
cannot be classified, and turns the resulting evidence into productivity guidance and
an automation-diagnosis prompt you can paste into ChatGPT or Claude.

The product is an independent Ensō application. It uses selected ActivityWatch capture
components under the Mozilla Public License 2.0; it is not an official ActivityWatch
distribution.

## What the beta includes

- Automatic active-window and AFK capture on macOS and Windows.
- A local timeline with application categories and optional window titles.
- Context check-ins for AFK, unknown, or ambiguous periods—not a recurring interruption
  when the evidence is already clear.
- 7, 14, or 28-day studies, productivity observations, automation candidates, and a
  copyable AI prompt. Ensō Time does not send that prompt or your timeline to an AI.
- Local export and deletion controls. No account, telemetry, cloud sync, ads, or hidden
  updater.

## Install from GitHub Releases

Download the installer that matches your computer from the repository's **Releases**
page:

- Apple Silicon Mac (M1 or newer): `macos-arm64` DMG.
- Intel Mac: `macos-x86_64` DMG.
- Windows 10/11 x64: MSI or NSIS EXE.

Each release includes `SHA256SUMS`. Verify the downloaded installer before opening it:

```bash
shasum -a 256 "Ensō Time.dmg"
```

Compare the result with the matching line in `SHA256SUMS`. Signed/notarized installers
are produced when the repository's release credentials are configured. Development or
unsigned builds can trigger Gatekeeper or SmartScreen warnings; never bypass a warning
for an installer whose checksum you have not verified.

### First-run permissions

On macOS, grant only the Accessibility and/or Screen Recording permissions requested by
the active-window watcher. The exact prompt depends on the macOS version. On Windows,
no administrator access should be required for normal use. Disabling OS-level access
stops detailed window capture; Ensō Time should report the watcher as unavailable rather
than silently pretending to track.

## Privacy model

All timeline, check-in, and diagnosis data stays in Ensō Time's application-data folder.
The embedded server binds only to loopback (`127.0.0.1`) on a private port and its CORS
policy does not allow production browser extensions. The app does not expose the full
ActivityWatch web UI.

Window titles can reveal document names, URLs, people, or client information. Turn off
title capture or add classification rules if that context is too sensitive. Exported
JSON is unencrypted and should be handled like any other private work log. “Delete all
data” removes Ensō Time's local records; backups made by the operating system remain
governed by the user's backup policy.

Ensō Time only opens ChatGPT or Claude after an explicit action. Copying a diagnosis
prompt puts that text on the system clipboard; pasting it into a third-party service is
the user's choice and is governed by that service's privacy terms.

## Coexistence with ActivityWatch

Ensō Time uses its own database, loopback port, and process lifecycle. The bundled
watchers come from ActivityWatch `v0.14.0b3`. Until coexistence has been verified on a
specific OS version, avoid running the official ActivityWatch desktop bundle and Ensō
Time simultaneously: upstream watcher queue/config behavior can otherwise create
duplicate events or send queued events to the wrong local server. Stopping one app does
not delete the other's database.

## Development

Prerequisites: Node.js 22, the stable Rust toolchain, and the platform requirements for
Tauri 2.

```bash
npm ci
npm run test:activitywatch
npm run prepare:activitywatch -- --target macos-arm64
npm run tauri:dev
```

Supported preparer targets are `macos-arm64`, `macos-x86_64`, and
`windows-x86_64`. Tauri/Rust triples are accepted as aliases. The preparer downloads a
fixed upstream archive, rejects any SHA-256 mismatch, extracts only
`aw-watcher-window` and `aw-watcher-afk` with their runtime dependencies, and writes a
per-file manifest to `src-tauri/resources/activitywatch/`. Generated binaries are
gitignored.

Useful checks:

```bash
npm run check
npm run test:activitywatch
node scripts/prepare-activitywatch.mjs --target windows-x86_64 --dry-run
```

GitHub Releases are built from `.github/workflows/release.yml` for macOS arm64, macOS
x64, and Windows x64. The workflow never checks for or installs updates in the
background. Apple signing/notarization uses `APPLE_CERTIFICATE`,
`APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`,
and `APPLE_TEAM_ID` when configured as repository secrets. Windows Authenticode signing
uses a base64 PFX in `WINDOWS_CERTIFICATE` plus `WINDOWS_CERTIFICATE_PASSWORD`. When
those secrets are absent, the workflow deliberately emits unsigned installers and the
release checksums still identify the exact outputs.

## License and attribution

Ensō Time source code is licensed under the Mozilla Public License 2.0. See `LICENSE`.
ActivityWatch provenance, the pinned server commit, local modifications, and additional
third-party notices are documented in `THIRD_PARTY_NOTICES.md` and
`vendor/activitywatch/README.md`.
