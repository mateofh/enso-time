# Third-party notices

Ensō Time is distributed under the Mozilla Public License 2.0 and includes or bundles
the following third-party software. Copyright remains with each upstream contributor.

## ActivityWatch

- Project: ActivityWatch
- Website: <https://activitywatch.net/>
- Source: <https://github.com/ActivityWatch/activitywatch>
- Watcher release: `v0.14.0b3`
- Bundled components: `aw-watcher-window` and `aw-watcher-afk`, including the runtime
  files contained in their official release directories
- License: Mozilla Public License 2.0

The release preparer downloads the official platform ZIP and validates one of these
fixed SHA-256 hashes before extraction:

| Target | Official ZIP SHA-256 |
| --- | --- |
| macOS arm64 | `70a7ba7e307abe3352b912411f09600075f427b5b5d139a238c051f813e96264` |
| macOS x86_64 | `eb9bf65f2a0e28489b4f89eb64159a15b82f8f07d6eb6cd3efe0dd3565e61b36` |
| Windows x86_64 | `45ce0ccc9e12ff60358878d3b8bef06da86957e53fc564ea2316cf2f190eb659` |

Ensō Time also vendors selected crates from
<https://github.com/ActivityWatch/aw-server-rust> at commit
`7c33810cd44393b3ecad64277b7e69bfeaee817b`. The production CORS policy is modified to
remove browser-extension origins, and a minimal embedded-static placeholder is added.
The exact changes are described in `vendor/activitywatch/README.md`. Corresponding
source is included under `vendor/activitywatch/` as required by MPL-2.0.

Ensō Time is not affiliated with or endorsed by the ActivityWatch project.

## Application libraries and fonts

- Tauri — Apache License 2.0 or MIT License.
- React — MIT License.
- Lucide — ISC License.
- Inter and Inter Tight — SIL Open Font License 1.1.
- Jost — SIL Open Font License 1.1.
- Quicksand — SIL Open Font License 1.1.

Additional transitive JavaScript and Rust packages retain their upstream copyright and
license terms. Exact versions are recorded in `package-lock.json` and the Rust lockfile
generated for a release build. This notice does not replace those upstream license
files or terms.
