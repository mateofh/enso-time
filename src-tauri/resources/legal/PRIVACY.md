# Privacy

Ensō Time is designed as a local-first desktop application. The public beta does not
require an account and does not include product telemetry, advertising, cloud sync, or
an automatic updater.

## Data processed on your device

Depending on the permissions and settings you enable, Ensō Time can process active
application names, window titles, AFK status, timestamps, classifications, check-in
answers, productivity observations, automation notes, browser domains, and
editor/terminal project metadata. Window titles and optional visible
browser/editor text may contain sensitive document names, URLs, people, client
information, or source code.

Deep context sources are disabled by default and controlled independently. Connectors
are rejected while tracking is paused or outside the configured work schedule. Context
observations follow the same retention period as the timeline.

This data is stored in the application's local data directory. The embedded
ActivityWatch-derived server binds to loopback (`127.0.0.1`) and is not intended to be
reachable from other devices. Operating-system backups, endpoint-management products,
antivirus tools, or anyone with access to your device may still copy or inspect local
files outside Ensō Time's control.

## AI and external services

Ensō Time can prepare text for use with third-party AI services. Copying the diagnostic
prompt places it only on the system clipboard. Published v0.2 installers do not send
activity data to any AI service and do not capture the screen.

### v0.3 preview builds only

The following applies only to development builds made with the
`experimental-intelligence` feature. Those features are still under security review.

The macOS OCR path uses Apple Vision locally: screenshot pixels and full OCR output are
held only in memory and are not written to disk. Up to 30 normalized terms may be stored
for an observation. Apps on the exclusion list are skipped.

The optional OpenRouter evaluator sends a minimized evidence bundle only after you:

1. enable AI and choose two distinct models;
2. set a monthly hard budget;
3. inspect the exact JSON payload; and
4. authorize the fingerprint for that source/model policy.

Changing a model or cloud-allowed source invalidates the consent. Requests ask
OpenRouter for zero-data-retention routing, deny provider data collection, prohibit
provider fallbacks, and require structured output. Provider/model availability can
prevent a request; these request flags do not replace the third party's legal terms or
privacy policy. A weekly evaluation runs automatically only when its separate toggle is
enabled and the current policy remains authorized. The local database records model
names, token counts, cost, results, and sanitized failures, but never stores the API key;
the key is kept in the operating-system credential store.

Only sources marked **Allow in approved cloud summaries** enter the deep-context portion
of the payload. Window titles are not included by default. Before every manual run, the
app displays the payload and a conservative maximum cost estimate.

## Your controls

You can limit operating-system capture permissions, disable or revoke each connector,
disable sensitive title/OCR capture, revoke cloud consent, remove the OpenRouter key,
export local data, and request deletion from within the app. Deleting all app data also
removes the OpenRouter credential from the operating-system store. Exported files are
unencrypted. Deletion from Ensō Time does not erase copies already exported, backed up,
synced, or previously processed by a third party.

## Distribution infrastructure

Source and beta installers may be distributed through GitHub. GitHub may process
ordinary website and download metadata under its own privacy terms. Ensō Time does not
receive that data through application telemetry.

Because this is beta software, review the source, permissions, and release checksum
before using it with sensitive information. Privacy or security concerns can be raised
through the channel in [`SECURITY.md`](SECURITY.md).
