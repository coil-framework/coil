# Gitly Backend

Gitly uses the customer-root model directly:

- first-party customer logic lives in `crates/gitly-backend`
- the `gitly` binary links that crate into the runtime
- bounded third-party behavior lives under `extensions/`

This folder is documentation only. Gitly does not use a sidecar backend for the primary demo.

## Real Backend Path

Start here:

- `../crates/gitly-backend/src/lib.rs`

That crate owns:

- repository, pull request, workflow, organization, and user fixtures
- JSON payload helpers for the GitHub-style API
- the linked customer plugin descriptor
- CMS publish policy for README-style pages

Useful commands from `apps/gitly`:

```bash
./scripts/prepare-local-dev.sh
cargo run -p gitly -- describe
cargo run -p gitly -- linked-backend describe
cargo run -p gitly -- linked-backend repository
cargo run -p gitly -- linked-backend pulls
cargo run -p gitly -- linked-backend workflows
```

## Boundary

Use linked Rust when the behavior is first-party customer logic that should run inside the Gitly
runtime.

Use WASM in `../extensions/` when the behavior should stay runtime-installed, bounded, and
replaceable.
