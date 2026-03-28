## Shoppr WASM Extension Examples

This folder demonstrates the bounded runtime-only extension path from
`docs/design/80-customer-extensions-and-integration-patterns.md`.

Use this folder for customization that should remain:

- runtime-installed rather than linked into the customer binary
- capability-scoped through host contracts
- replaceable or removable without changing Shoppr's native crates
- appropriate for a third-party, partner, or marketplace-style add-on

Do not use this folder for Shoppr's first-party store logic. That path is the linked Rust
customer workspace under `crates/shoppr-backend`, backed by the shared customer-owned rules
in `backend/shoppr-loyalty-backend`, per chapter 96.

The checked-in example here is:

- `shoppr-waitlist-tools/`
  - a bounded runtime-installed WASM extension that is now pinned in `app.toml`
  - loaded through `package.toml` and a checked-in WAT source, then compiled into the runtime
    artifact path during Shoppr bootstrap
  - meant to show a real installed extension boundary without pretending Shoppr's first-party
    business logic belongs in WASM

That distinction is intentional:

- linked Rust is the primary path for Shoppr-owned first-party logic
- WASM remains the bounded path for runtime-installed or third-party extensions
