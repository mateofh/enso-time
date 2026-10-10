# Generated ActivityWatch resources

`npm run prepare:activitywatch -- --target <target>` writes a generated
`manifest.json` to `activitywatch/`. Windows targets download and verify the pinned
ActivityWatch release and include both upstream watchers. macOS targets include no
watcher resources because window and idle capture run natively inside the Ensō Time
process.

The generated directory is intentionally ignored by Git. Do not commit these binaries.
