# Vendored ActivityWatch server

This directory contains the minimum Rust server workspace required by Ensō Time:
`aw-server`, `aw-client-rust`, `aw-datastore`, `aw-models`, `aw-query`, and
`aw-transform`. The client crate is an Android-target dependency in the upstream server
manifest; Cargo resolves its path on desktop targets even though it is not linked into
the desktop binary.

- Upstream: <https://github.com/ActivityWatch/aw-server-rust>
- Pinned commit: `7c33810cd44393b3ecad64277b7e69bfeaee817b`
- License: Mozilla Public License 2.0 (see the repository root `LICENSE`)

## Local modifications

1. `aw-server/src/endpoints/cors.rs` removes the production allow-list for browser
   extensions. Ensō Time's server is loopback-only and accepts only its own local
   origins (plus explicit configuration). Test mode retains a test-only extension
   origin.
2. `aw-server/server-static/index.html` is a minimal compile-time placeholder for
   `rust-embed`. Ensō Time does not ship or expose ActivityWatch's web UI.

No other functional upstream source changes are intended; insignificant whitespace may
be normalized so repository-wide checks remain clean. When updating, compare every vendored
directory against the new pinned commit, reapply the two changes above, run the Rust
tests, and update `UPSTREAM_COMMIT` and `THIRD_PARTY_NOTICES.md` in the same commit.
