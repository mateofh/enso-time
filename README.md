# Ensō Time

Ensō Time is a local-first desktop time tracker that helps you decide where to start
applying AI in your daily work. It records which applications receive your attention,
asks only when a 15/30/60-minute interval cannot be classified, and turns the resulting
evidence into a prioritized automation opportunity, a guided workflow intake, and a
diagnostic prompt you can paste into ChatGPT or Claude.

> **Public beta:** `v0.2.0-beta.1` is source-available, not open source. The beta is
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
  copyable AI prompt.
- Optional local context connectors for Chromium, VS Code/Cursor, and terminal. Each
  connector uses a one-use pairing code and a revocable per-client token, and stops
  recording while tracking is paused or outside the work schedule.
- A bilingual, editable workflow intake for the top candidate: trigger, tools, ordered
  steps, human decisions, inputs, desired output, frequency, time, and pain points.
- Local export and deletion controls. No account, telemetry, cloud sync, ads, or hidden
  updater.

The beta helps identify and document the first opportunity. It does not create or run
an automation in n8n, Make, Zapier, or any external system. See the [roadmap](ROADMAP.md)
for the planned Automation Blueprint stage.

## Install from GitHub Releases

Download the installer for your computer from this repository's **Releases** page:

- Apple Silicon Mac: arm64 DMG.
- Intel Mac: x86_64 DMG.
- Windows 10/11 x64: MSI or NSIS EXE.

Every release includes `SHA256SUMS`. Verify the downloaded installer before opening it:

```bash
shasum -a 256 "enso-time-0.2.0-beta.1-macos-arm64.dmg"
```

On Windows, use PowerShell:

```powershell
Get-FileHash .\enso-time-0.2.0-beta.1-windows-x86_64.msi -Algorithm SHA256
```

Compare the hash with the matching line in `SHA256SUMS` from the same release. A
checksum confirms that two files are identical; it does not prove who created a file or
that the file is safe.

Each release also includes a Sigstore bundle named
`enso-time-<version>-provenance.sigstore.json`. The tag workflow generates GitHub build
provenance for all four installers using a short-lived GitHub OIDC identity. With a
current GitHub CLI, replace `OWNER/REPOSITORY` with this repository and verify an
installer online:

```bash
gh attestation verify ./enso-time-0.2.0-beta.1-macos-arm64.dmg \
  --repo OWNER/REPOSITORY \
  --signer-workflow OWNER/REPOSITORY/.github/workflows/release.yml
```

To verify against the attached bundle instead of fetching the attestation from GitHub:

```bash
gh attestation verify ./enso-time-0.2.0-beta.1-macos-arm64.dmg \
  --repo OWNER/REPOSITORY \
  --bundle ./enso-time-0.2.0-beta.1-provenance.sigstore.json \
  --signer-workflow OWNER/REPOSITORY/.github/workflows/release.yml
```

Provenance links an artifact digest to this GitHub Actions workflow and source ref. It
does not review the source or replace Apple Developer ID signing/notarization or Windows
Authenticode.

### Installer trust

Release macOS applications are signed with Developer ID, notarized by Apple, and shipped
in a DMG with a stapled notarization ticket. The release workflow fails closed when any
required Apple credential is missing. Windows MSI and NSIS installers remain unsigned,
so SmartScreen may warn. In every case, verify the published checksum and provenance.

### First-run permissions

On macOS 12 or newer, Accessibility is optional and is used only for window titles.
Without it, Ensō Time continues recording active application names and omits titles.
Release builds never request Screen Recording. The app does not require Automation/Apple
Events access. On Windows,
administrator access should not be required for normal use. Disabling OS-level access
stops detailed window capture; Ensō Time should report the watcher as unavailable rather
than silently pretending to track.

### Optional context connectors

- Chromium: load [`connectors/chromium`](connectors/chromium) as an unpacked extension,
  enable **Browser** in Ensō Time, and pair it with the displayed eight-digit code.
- VS Code/Cursor: package or run [`connectors/vscode`](connectors/vscode) as an extension,
  enable **Editor**, and run `Ensō Time: Emparejar con la aplicación`.
- Terminal: follow [`connectors/shell/README.md`](connectors/shell/README.md), enable
  **Terminal**, and pair `enso-context.py` once.

Disabling a source in Ensō Time immediately rejects further observations even if a
connector still holds its local token. Browser visible text is accepted only for domains
also listed in the desktop allowlist.

## Privacy model

Timeline, check-in, connector, and diagnosis data stays in Ensō Time's local application-data
folder. The embedded server binds only to loopback (`127.0.0.1`) on a private port, and
the app does not expose the full ActivityWatch web UI.

Window titles can reveal document names, URLs, people, or client information. Turn off
title capture or add classification rules if that context is too sensitive. Exported
JSON is unencrypted and should be handled like any other private work log. Deleting app
data does not delete exported files or operating-system backups.

Release builds of v0.2 do not contact any AI service: the copyable diagnostic prompt is
only placed on the clipboard. See the full [privacy notice](PRIVACY.md).

### v0.3 preview: OpenRouter evaluation and screen OCR

The source also contains an OpenRouter analyst/evaluator and macOS screen OCR. Both are
disabled in every published installer and are only active in a development build made
with `cargo tauri build --features experimental-intelligence`. They remain under security
review and are tracked in [`ROADMAP.md`](ROADMAP.md); do not use the preview with
sensitive data.

## Coexistence with ActivityWatch

Ensō Time uses its own database, loopback port, and process lifecycle. On macOS, window
and idle capture run natively inside the Ensō Time process; no ActivityWatch watcher
processes are bundled or launched. Windows packages include the pinned ActivityWatch
`v0.14.0b3` window and AFK watchers. Until coexistence has been verified on a specific
Windows version, avoid running the official ActivityWatch desktop bundle and Ensō Time
simultaneously: upstream watcher queue/config behavior can create duplicate events or
send queued events to the wrong local server. Stopping one app does not delete the
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
`windows-x86_64`. Tauri/Rust triples are accepted as aliases. For Windows, the preparer
downloads a fixed upstream archive, rejects any SHA-256 mismatch, and extracts both
watchers with their runtime dependencies. For macOS, it writes an empty schema-v2
resource manifest because capture is native and in-process. Generated resources are
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
`SHA256SUMS`, and publish a prerelease when the version contains a hyphen. Apple
credentials are supplied only through GitHub Actions secrets; they are imported into a
temporary keychain during the job and are never stored in this repository. macOS
artifacts are signed and notarized; Windows artifacts remain unsigned.

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
