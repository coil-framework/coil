---
title: Linked Rust Hook APIs
---

Linked Rust is Davenda's primary customization model for customer-owned backend logic.

This page documents the public hook surface exposed by `davenda-customer-sdk`.

## What This Page Covers

Use this page when you want to know:

- which traits a customer backend can implement
- how plugins register themselves
- which facades customer hooks receive
- what the supported integration boundary looks like today

For the architectural rationale, read [Customer Rust Vs Third-Party WASM](./customer-vs-wasm.md).
For a complete working example, read [Shoppr Linked Rust Backend](../use-cases/shoppr/linked-rust-backend.md).

## Where The Public SDK Lives

The stable surface is provided by:

- `crates/davenda-customer-sdk/src/lib.rs`
- `crates/davenda-customer-sdk/src/registry.rs`
- `crates/davenda-customer-sdk/src/hooks.rs`
- `crates/davenda-customer-sdk/src/facade.rs`

Customer apps should depend on this SDK instead of importing arbitrary runtime internals.

## Plugin Registration

Every linked backend starts by implementing `CustomerBackendPlugin`.

Core methods:

- `descriptor()`
  - identifies the plugin with id, name, and version
- `register()`
  - registers one or more hook implementations with the runtime

The runtime builder accepts linked plugins through:

- `register_customer_plugin(...)`
- `with_customer_plugin(...)`

The Shoppr and Gitly customer binaries use this pattern directly.

## Hook Registry

The runtime exposes a `CustomerHookRegistry` during plugin registration.

Current supported registration points are:

- checkout hooks
- CMS publish hooks
- verified webhook hooks
- verified webhook asset hooks

These are intentionally explicit. A plugin declares exactly which hook families it participates in.

## Hook Traits

### `CheckoutHooks`

Entry point:

- `review_order(...) -> Result<OrderReviewDecision, BackendError>`

Use it for:

- order review policy
- loyalty or membership gating
- metadata enrichment before order acceptance

Facades available:

- `CommerceFacade`
- `AuthFacade`
- `AuditFacade`

### `CmsHooks`

Entry point:

- `validate_page_publish(...) -> Result<CmsPublishDecision, BackendError>`

Use it for:

- editorial policy
- custom publish gates
- content validation and rewrite flows

Facades available:

- `RepositoryFacade`
- `AuditFacade`

### `VerifiedWebhookHooks`

Entry point:

- `handle_verified_webhook(...) -> Result<WebhookHandlingResult, BackendError>`

Use it for:

- reacting to verified payment or integration webhooks
- enqueueing follow-up jobs
- updating repository-backed records

Facades available:

- `OutboundHttpFacade`
- `JobsFacade`
- `RepositoryFacade`
- `AuditFacade`

### `VerifiedWebhookAssetHooks`

Entry point:

- `handle_verified_webhook(...)` with asset publication access

Use it when verified-webhook processing must also publish or inspect managed assets.

Additional facade:

- `AssetsFacade`

## Facades

The public facades are the supported way for customer code to interact with the platform.

### `CommerceFacade`

Current responsibilities:

- inspect products
- add order notes

### `JobsFacade`

Current responsibility:

- enqueue runtime jobs

### `RepositoryFacade`

Current responsibilities:

- read repository records
- write repository records

The SDK also provides `RepositoryFacadeExt` helpers for common typed records such as:

- CMS pages
- CMS navigation
- CMS redirects
- catalogue products
- catalogue collections
- orders

### `AuthFacade`

Current responsibilities:

- capability checks
- denial explanations

### `AuditFacade`

Current responsibility:

- record audit entries

### `OutboundHttpFacade`

Current responsibility:

- send outbound HTTP requests through the runtime boundary

### `AssetsFacade`

Current responsibilities:

- publish managed assets
- inspect managed assets

## What Linked Rust Can Do That WASM Should Not

Linked Rust is the right path when you need:

- richer typing
- first-party build participation
- tighter integration with customer-owned policy
- broader public facades than a runtime-installed extension should receive

If the code should be installable by third parties at runtime, that is exactly when you should not
use this surface.

## Canonical Examples

Read these end to end:

- `apps/shoppr/backend/shoppr-loyalty-backend/src/lib.rs`
- `apps/shoppr/crates/shoppr-backend/src/lib.rs`
- `apps/shoppr/crates/shoppr-app/src/lib.rs`
- `apps/gitly/crates/gitly-backend/src/lib.rs`
- `apps/gitly/crates/gitly-app/src/lib.rs`

They show:

- plugin descriptor registration
- hook registration
- typed hook implementation
- customer binary composition

## Common Mistakes

- Importing runtime internals instead of the customer SDK.
- Treating linked Rust as a separate sidecar API.
- Putting one plugin id behind multiple unrelated behaviours without a clear boundary.
- Using linked Rust for reusable marketplace-style extensions that should remain runtime-installed.

## Read Next

- [Customer Rust Vs Third-Party WASM](./customer-vs-wasm.md)
- [Shoppr Linked Rust Backend](../use-cases/shoppr/linked-rust-backend.md)
- [Jobs And Schedulers](../operations/jobs-and-schedulers.md)
- [Observability](../operations/observability.md)
