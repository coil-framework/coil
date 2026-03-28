# OctoHub Backend

OctoHub uses the customer-root model directly:

- first-party customer logic lives in `crates/octohub-backend`
- the `octohub` binary links that crate into the runtime
- bounded third-party behavior lives under `extensions/`

This folder is documentation only. OctoHub does not use a sidecar backend for the primary demo.

## Real Backend Path

Start here:

- `../crates/octohub-backend/src/lib.rs`

That crate owns:

- repository, pull request, workflow, organization, and user fixtures
- JSON payload helpers for the GitHub-style API
- the linked customer plugin descriptor
- CMS publish policy for README-style pages

Useful commands from `apps/octohub`:

```bash
./scripts/prepare-local-dev.sh
cargo run -p octohub -- describe
cargo run -p octohub -- linked-backend describe
cargo run -p octohub -- linked-backend repository
cargo run -p octohub -- linked-backend pulls
cargo run -p octohub -- linked-backend workflows
```

## Boundary

Use linked Rust when the behavior is first-party customer logic that should run inside the OctoHub
runtime.

Use WASM in `../extensions/` when the behavior should stay runtime-installed, bounded, and
replaceable.
