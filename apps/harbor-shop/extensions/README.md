## Harbor Shop WASM Extension Examples

This folder demonstrates the bounded runtime-only extension path from
`docs/design/80-customer-extensions-and-integration-patterns.md`.

Use this folder for customization that should remain:

- runtime-installed rather than linked into the customer binary
- capability-scoped through host contracts
- replaceable or removable without changing Harbor Shop's native crates
- appropriate for a third-party, partner, or marketplace-style add-on

Do not use this folder for Harbor Shop's first-party store logic. That path is the linked Rust
customer workspace under `crates/harbor-shop-backend` and
`backend/harbor-loyalty-backend`, per chapter 96.

The checked-in example here is:

- `harbor-waitlist-tools/`
  - a bounded WASM package example for an events/admin waitlist widget and a scheduled
    reconciliation job
  - intentionally not installed by default in `app.toml`
  - meant to show package shape, capability boundaries, and installation guidance without
    pretending Harbor Shop ships a live marketplace package in this checkout

That distinction is intentional:

- linked Rust is the primary path for Harbor Shop-owned first-party logic
- WASM remains the bounded path for runtime-installed or third-party extensions
