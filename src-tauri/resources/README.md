# Generated ActivityWatch resources

`npm run prepare:activitywatch -- --target <target>` downloads and verifies the pinned
ActivityWatch release, then writes only `aw-watcher-window`, `aw-watcher-afk`, and a
generated `manifest.json` to `activitywatch/`.

The generated directory is intentionally ignored by Git. Do not commit these binaries.
