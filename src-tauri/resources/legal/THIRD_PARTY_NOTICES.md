# Third-party notices

Ensō Time's original code is made available under PolyForm Noncommercial 1.0.0. This
distribution also includes third-party software under its own terms. Copyright remains
with each upstream contributor.

## ActivityWatch

- Project: ActivityWatch
- Website: <https://activitywatch.net/>
- Source: <https://github.com/ActivityWatch/activitywatch>
- Watcher release: `v0.14.0b3`
- Bundled components: `aw-watcher-window` and `aw-watcher-afk`, including runtime files
  from their official release directories
- License: Mozilla Public License 2.0, reproduced in [`LICENSES/MPL-2.0.txt`](LICENSES/MPL-2.0.txt)

The release preparer downloads official platform archives and rejects any archive that
does not match these fixed SHA-256 values:

| Target | Official ZIP SHA-256 |
| --- | --- |
| `macos-arm64` | `e70639b63df662e1c72b4277e073bd80f7b94c516f12a8d01404f4aa526e8731` |
| `macos-x86_64` | `eb9bf65f2a0e28489b4f89eb64159a15b82f8f07d6eb6cd3efe0dd3565e61b36` |
| `windows-x86_64` | `45ce0ccc9e12ff60358878d3b8bef06da86957e53fc564ea2316cf2f190eb659` |

Selected crates from <https://github.com/ActivityWatch/aw-server-rust> are vendored at
commit `7c33810cd44393b3ecad64277b7e69bfeaee817b`. The production CORS allow-list is
modified to remove browser-extension origins, and a minimal embedded-static
placeholder replaces the web UI. Details and corresponding source are included under
`vendor/activitywatch/`.

All files under `vendor/activitywatch/**`, including Ensō Time's modifications to those
files, remain covered by MPL-2.0. Generated watcher bundles under
`src-tauri/resources/activitywatch/**` retain their upstream terms and notices. The
PolyForm license in the repository root does not apply to that ActivityWatch material.

Ensō Time is independent and is not affiliated with or endorsed by ActivityWatch. The
ActivityWatch name and marks belong to their respective owners.

## Application libraries and fonts

- Tauri — Apache License 2.0 or MIT License.
- React and React DOM — MIT License.
- Lucide — ISC License.
- Inter and Inter Tight — SIL Open Font License 1.1.
- Jost — SIL Open Font License 1.1.
- Quicksand — SIL Open Font License 1.1.

Additional direct and transitive JavaScript and Rust packages retain their upstream
copyright and license terms. Exact versions are recorded in `package-lock.json` and
`src-tauri/Cargo.lock`. Their licenses are not replaced by PolyForm, this notice, or
the trademark policy.
