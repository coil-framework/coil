---
title: Extensions And Host APIs
---

Gitly is the clearest non-commerce demonstration of Davenda's runtime-installed WASM model.

Use this page when you want to see the abstract extension docs tied to a real app.

## Repo Areas To Read

Start here:

- `apps/gitly/crates/gitly-app/src/extensions.rs`
- `apps/gitly/extensions/gitly-actions-scheduler/package.toml`
- `apps/gitly/extensions/gitly-community-pulse/package.toml`
- `apps/gitly/extensions/artifacts/gitly-actions-scheduler.wasm`
- `apps/gitly/extensions/artifacts/gitly-community-pulse.wasm`

## What Gitly Demonstrates

Gitly shows the right responsibilities for WASM:

- bounded runtime-installed behaviour
- host-mediated metadata and background work
- app-visible enhancements that do not need first-party compile-time access

That is exactly the case where linked Rust would be the wrong default.

## Why Gitly Is Better Than Shoppr For This Topic

Shoppr proves the commerce story, but Gitly makes the extension boundary easier to see because the
product is not dominated by checkout or catalogue concerns.

Here the extensions are clearly:

- product enrichments
- scheduler-style helper behaviour
- installable features the host can reason about explicitly

## Host API Shape

Gitly's extension runtime uses the same public host categories documented in
[WASM Host APIs](../../reference/wasm-host-apis.md):

- HTTP
- jobs
- metadata
- secrets
- webhooks where applicable

The app-side extension loader in `apps/gitly/crates/gitly-app/src/extensions.rs` is the canonical
place to see how the customer app wires installed artifacts into the runtime.

## Practical Guidance

Use the Gitly pattern when:

- you want a third party to ship a runtime-installed feature
- the feature should stay bounded to documented host APIs
- the customer should be able to add or remove it without rebuilding core Davenda crates

Do not use this pattern when:

- the logic is customer-owned first-party policy
- the feature needs linked Rust hook facades
- the behaviour is central enough that it should compile into the product binary

## Read Next

- [WASM Host APIs](../../reference/wasm-host-apis.md)
- [Extension Package Format](../../reference/extension-package-format.md)
- [Customer Rust Vs Third-Party WASM](../../reference/customer-vs-wasm.md)
