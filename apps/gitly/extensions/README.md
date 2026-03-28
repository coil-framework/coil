## Gitly WASM Extensions

This folder demonstrates the bounded runtime-only extension path from
`docs/design/80-customer-extensions-and-integration-patterns.md`.

Gitly uses WASM only for behavior that should stay:

- runtime-installed rather than linked into the customer binary
- capability-scoped through explicit host contracts
- replaceable without rebuilding the customer app
- appropriate for third-party or partner-style customization

The checked-in examples are:

- `gitly-community-pulse/`
  - API extension for `/api/github/pulse`
- `gitly-actions-scheduler/`
  - scheduled-job extension for `github.actions.refresh`

That boundary is deliberate:

- linked Rust in `../crates/gitly-backend` is the first-party customer path
- WASM in this folder is the bounded runtime-installed path
