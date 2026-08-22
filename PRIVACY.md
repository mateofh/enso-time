# Privacy

Ensō Time is designed as a local-first desktop application. The public beta does not
require an account and does not include product telemetry, advertising, cloud sync, or
an automatic updater.

## Data processed on your device

Depending on the permissions and settings you enable, Ensō Time can process active
application names, window titles, AFK status, timestamps, classifications, check-in
answers, productivity observations, and automation notes. Window titles may contain
sensitive document names, URLs, people, or client information.

This data is stored in the application's local data directory. The embedded
ActivityWatch-derived server binds to loopback (`127.0.0.1`) and is not intended to be
reachable from other devices. Operating-system backups, endpoint-management products,
antivirus tools, or anyone with access to your device may still copy or inspect local
files outside Ensō Time's control.

## AI and external services

Ensō Time can prepare text for use with third-party AI services, but it does not send
your timeline or prompt to them automatically. Copying a prompt places it on the system
clipboard. Opening, pasting into, or otherwise using a third-party service is an
explicit user action and is governed by that service's terms and privacy practices.

## Your controls

You can limit operating-system capture permissions, disable sensitive title capture
where the application supports it, export local data, and request deletion from within
the app. Exported files are unencrypted. Deletion from Ensō Time does not erase copies
already exported, backed up, synced, or captured by other software.

## Distribution infrastructure

Source and beta installers may be distributed through GitHub. GitHub may process
ordinary website and download metadata under its own privacy terms. Ensō Time does not
receive that data through application telemetry.

Because this is beta software, review the source, permissions, and release checksum
before using it with sensitive information. Privacy or security concerns can be raised
through the channel in [`SECURITY.md`](SECURITY.md).
