---
title: WASM Host APIs
---

Davenda's WASM extensions run against a bounded host API surface.

That boundary is the point of the model. A runtime-installed extension should be powerful enough to
do useful work and constrained enough that customer apps can install it without turning the runtime
into an untyped plugin free-for-all.

## What This Page Covers

Use this page when you want to know:

- what services a WASM extension can access
- what it cannot access
- how extension handlers interact with the host runtime
- where to read the concrete host-service implementation

## Where The Host Surface Lives

The current host implementation lives in:

- `crates/davenda-runtime/src/wasm/host/mod.rs`
- `crates/davenda-runtime/src/wasm/host/context.rs`
- `crates/davenda-runtime/src/wasm/host/principal.rs`
- `crates/davenda-runtime/src/wasm/host/services/`

The main service families exposed today are:

- HTTP
- jobs
- metadata
- secrets
- webhooks

## Execution Model

A WASM extension is loaded by the runtime for a supported extension point.

At execution time the host provides:

- request and route context
- current principal information
- host services permitted for that extension point
- typed input and typed output boundaries

The extension does not become a second application runtime. It participates at a specific bounded
hook point.

## HTTP Service

The HTTP host service lets an extension make outbound requests through a controlled runtime facade.

Implementation:

- `crates/davenda-runtime/src/wasm/host/services/http/mod.rs`
- `crates/davenda-runtime/src/wasm/host/services/http/backend.rs`
- `crates/davenda-runtime/src/wasm/host/services/http/offload.rs`

Use it for:

- calling partner APIs
- enrichment lookups
- background integration work

Do not use it as an excuse to rebuild the whole customer backend in WASM.

## Jobs Service

The jobs host service lets an extension enqueue background work through the platform queue.

Implementation:

- `crates/davenda-runtime/src/wasm/host/services/jobs.rs`

Use it for:

- follow-up work
- asynchronous notifications
- bounded scheduled or event-driven extension tasks

## Metadata Service

The metadata service exposes typed metadata storage and sequencing helpers used by extensions.

Implementation:

- `crates/davenda-runtime/src/wasm/host/services/metadata/mod.rs`
- `crates/davenda-runtime/src/wasm/host/services/metadata/local.rs`
- `crates/davenda-runtime/src/wasm/host/services/metadata/shared.rs`
- `crates/davenda-runtime/src/wasm/host/services/metadata/sequence.rs`

Use it for:

- lightweight extension state
- cross-invocation sequencing
- host-managed metadata values

## Secrets Service

The secrets service lets extensions resolve approved secrets through the runtime instead of reading
host environment variables directly.

Implementation:

- `crates/davenda-runtime/src/wasm/host/services/secrets.rs`

That keeps secret access explicit and host-mediated.

## Webhook Services

The webhook host service handles extension participation in verified webhook flows.

Implementation:

- `crates/davenda-runtime/src/wasm/host/services/webhooks/mod.rs`
- `crates/davenda-runtime/src/wasm/host/services/webhooks/local.rs`
- `crates/davenda-runtime/src/wasm/host/services/webhooks/shared.rs`

Use it for:

- accepted runtime-installed webhook enrichments
- bounded processing that still fits inside extension trust limits

## Context And Principal

The host also provides execution context and principal-aware information:

- `crates/davenda-runtime/src/wasm/host/context.rs`
- `crates/davenda-runtime/src/wasm/host/principal.rs`

That is what lets the extension behave differently for:

- current site
- locale
- operator versus anonymous user
- current route and request context

## What Extensions Cannot Do

By design, runtime-installed WASM extensions should not assume they can:

- import arbitrary internal Davenda crates
- bypass auth and repository facades
- read the raw database directly
- become the primary customer-owned business logic layer

If you need that level of power, use linked Rust instead.

## Lifecycle Guidance

Think about the lifecycle like this:

1. package the extension
2. build the `.wasm` artifact
3. install it into the customer app
4. runtime validates and loads it for supported extension points
5. the extension executes against host APIs for each invocation

The important constraint is that the runtime can add or remove an installed extension without
recompiling the whole customer binary, which is exactly why this surface must stay narrower than
linked Rust.

## Shoppr And Gitly Examples

Read these alongside this page:

- `apps/shoppr/crates/shoppr-app/src/extensions.rs`
- `apps/shoppr/extensions/shoppr-waitlist-tools/package.toml`
- `apps/gitly/crates/gitly-app/src/extensions.rs`
- `apps/gitly/extensions/gitly-actions-scheduler/package.toml`
- `apps/gitly/extensions/gitly-community-pulse/package.toml`

## Common Mistakes

- Treating WASM as the default customization path for customer-owned logic.
- Assuming extensions can access arbitrary runtime internals.
- Shipping an artifact without documenting which host services it needs.
- Forgetting that the trust boundary is the entire reason this model exists.

## Read Next

- [Extension Package Format](./extension-package-format.md)
- [Customer Rust Vs Third-Party WASM](./customer-vs-wasm.md)
- [Gitly Extensions And Host APIs](../use-cases/gitly/extensions-and-host-apis.md)
