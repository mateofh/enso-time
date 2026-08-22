# Ensō Time

Ensō Time is a local-first desktop time tracker for personal insight. It records which
applications receive your attention, asks only when a 15/30/60-minute interval cannot
be classified, and turns the resulting evidence into productivity guidance and an
automation-diagnosis prompt you can paste into ChatGPT or Claude.

> **Public beta:** `v0.1.0-beta.2` is source-available, not open source. The beta is
> free only for uses permitted by the
> [PolyForm Noncommercial License 1.0.0](LICENSE). Commercial use requires a separate
> written agreement; see [Commercial licensing](COMMERCIAL.md).

The product is an independent Ensō application. It uses selected ActivityWatch capture
and server components under MPL-2.0; it is not an official ActivityWatch distribution.

## What the beta includes

- Automatic active-window and AFK capture on macOS and Windows.
- A local timeline with application categories and optional window titles.
- Context check-ins for AFK, unknown, or ambiguous periods instead of recurring
  interruptions when the evidence is already clear.
- 7, 14, or 28-day studies, productivity observations, automation candidates, and a
  copyable AI prompt. Ensō Time does not send that prompt or your timeline to an AI.
- Local export and deletion controls. No account, telemetry, cloud sync, ads, or hidden
  updater.

## Install from GitHub Releases

Download the installer for your computer from this repository's **Releases** page:

- Apple Silicon Mac: arm64 DMG.
- Intel Mac: x86_64 DMG.
- Windows 10/11 x64: MSI or NSIS EXE.

Every release includes `SHA256SUMS`. Verify the downloaded installer before opening it:

```bash
shasum -a 256 "enso-time-0.1.0-beta.2-macos-arm64.dmg"
```

On Windows, use PowerShell:

```powershell
Get-FileHash .\enso-time-0.1.0-beta.2-windows-x86_64.msi -Algorithm SHA256
```

Compare the hash with the matching line in `SHA256SUMS` from the same release. A
checksum confirms that two files are identical; it does not prove who created a file or
that the file is safe.

### Unsigned beta installers

This beta is built without paid signing or notarization certificates. macOS application
contents are ad-hoc signed for DMG packaging; the DMG is not Apple-notarized. Windows
MSI and NSIS installers are unsigned. Gatekeeper or SmartScreen warnings are expected.
If you are not comfortable reviewing the source, release workflow, and checksum, do not
install the beta.

### First-run permissions

On macOS, grant only the Accessibility and/or Screen Recording permissions requested by
the active-window watcher. The exact prompt depends on the macOS version. On Windows,
administrator access should not be required for normal use. Disabling OS-level access
stops detailed window capture; Ensō Time should report the watcher as unavailable rather
than silently pretending to track.

## Privacy model

Timeline, check-in, and diagnosis data stays in Ensō Time's local application-data
folder. The embedded server binds only to loopback (`127.0.0.1`) on a private port, and
the app does not expose the full ActivityWatch web UI.

Window titles can reveal document names, URLs, people, or client information. Turn off
title capture or add classification rules if that context is too sensitive. Exported
JSON is unencrypted and should be handled like any other private work log. Deleting app
data does not delete exported files or operating-system backups.

Ensō Time only opens ChatGPT or Claude after an explicit action. Copying a diagnosis
prompt puts that text on the system clipboard; pasting it into a third-party service is
your choice and is governed by that service's terms. See the full [privacy
notice](PRIVACY.md).

## Coexistence with ActivityWatch

Ensō Time uses its own database, loopback port, and process lifecycle. The bundled
watchers come from ActivityWatch `v0.14.0b3`. Until coexistence has been verified on a
specific OS version, avoid running the official ActivityWatch desktop bundle and Ensō
Time simultaneously: upstream watcher queue/config behavior can create duplicate events
or send queued events to the wrong local server. Stopping one app does not delete the
other's database.

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
node scripts/license-check.mjs
node scripts/prepare-activitywatch.mjs --target windows-x86_64 --dry-run
```

Pull requests and pushes to `main` run checks on macOS and Windows. Tags matching `v*`
run the release workflow for macOS arm64, macOS x86_64, and Windows x64, generate
`SHA256SUMS`, and publish a prerelease when the version contains a hyphen. The workflow
contains no certificate secrets and deliberately produces ad-hoc/unsigned artifacts.

## License, contributions, and marks

Ensō Time is **source-available, not open source**. Original project code is licensed
under PolyForm Noncommercial 1.0.0 with this required notice:

```text
Required Notice: Copyright 2026 Mateo (@mateofh).
```

ActivityWatch code under `vendor/activitywatch/**` and modifications to it remain under
MPL-2.0. Other dependencies keep their original terms. The Ensō/Ensō Time names,
isotype, app icons, and logos are not licensed with the code. See the [license
map](LICENSES/README.md), [third-party notices](THIRD_PARTY_NOTICES.md), [trademark
policy](TRADEMARKS.md), and [contribution policy](CONTRIBUTING.md).
