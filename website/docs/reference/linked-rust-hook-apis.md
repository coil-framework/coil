---
title: Linked Rust Hook APIs
---

Linked Rust backends are the first-party customer extension model in Davenda.

Start with the smallest useful plugin shape:

```rust
use davenda_customer_sdk::{
    CheckoutHooks, CustomerBackendPlugin, CustomerHookRegistry, PluginDescriptor,
};

struct ShopprBackend;

impl CustomerBackendPlugin for ShopprBackend {
    fn descriptor(&self) -> PluginDescriptor {
        PluginDescriptor::new("shoppr-backend", "Shoppr Linked Backend", "0.1.0")
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) {
        registry.register_checkout(Box::new(ShopprCheckoutHooks));
    }
}
```

That is the model:

- the customer binary links a plugin
- the plugin registers explicit hooks
- the runtime invokes those hooks through stable SDK traits and facades

Use this page when you want to answer:

- how a customer plugin registers hooks
- which hook kinds exist today
- which facades hooks can call
- where Shoppr and Gitly demonstrate the pattern

## The Core Plugin Contract

The top-level trait is `CustomerBackendPlugin`.

Each plugin provides:

- `descriptor()`
  - id, display name, version, docs URL
- `register(...)`
  - hook registration against a `CustomerHookRegistry`

That is the stable boundary a customer crate should target.

## How To Think About Linked Rust

Linked Rust is for first-party product policy, not for generic "run arbitrary code."

Good linked Rust use cases:

- checkout review rules
- CMS publish validation
- verified webhook handling
- customer-specific audit or CRM routing

Bad linked Rust use cases:

- replacing core services
- bypassing runtime policy through private internals
- re-implementing official modules in customer code

## Registered Hook Kinds

The registry currently supports four hook families:

- `Checkout`
- `CmsPagePublish`
- `VerifiedWebhook`
- `VerifiedWebhookAssets`

Those hook kinds are exposed as `RegisteredHookKind` in
`crates/davenda-customer-sdk/src/registry.rs`.

## Checkout Hooks

The checkout hook trait lives in `crates/davenda-customer-sdk/src/hooks.rs`:

- `CheckoutHooks::review_order(...)`

This is the first hook to read if you are building customer-specific pricing, membership, fraud,
or CRM routing logic.

Concrete examples:

- `apps/shoppr/backend/shoppr-loyalty-backend/src/lib.rs`
- `apps/shoppr/crates/shoppr-backend/src/lib.rs`

These show how Shoppr:

- inspects the draft order
- adds order notes through `CommerceFacade`
- returns approve, reject, or adjust decisions

Minimal mental model for checkout hooks:

1. runtime builds an order draft
2. hook receives the draft through the SDK
3. hook uses stable facades if needed
4. hook returns a bounded decision

That is much safer than letting customer code reach directly into runtime request internals.

## CMS Publish Hooks

The CMS hook trait is:

- `CmsHooks::validate_page_publish(...)`

Gitly is the clearer example here:

- `apps/gitly/crates/gitly-backend/src/lib.rs`

Gitly uses this hook to keep its README-style content honest by requiring accessibility guidance in
published content.

That is a good example of linked Rust doing first-party product policy, not generic platform work.

## Verified Webhook Hooks

Verified webhooks are the hook family that runs after the runtime has already authenticated and
normalized inbound webhook data.

The traits are:

- `VerifiedWebhookHooks::handle_verified_webhook(...)`
- `VerifiedWebhookAssetHooks::handle_verified_webhook(...)`

These hooks matter because they combine multiple host facades safely:

- outbound HTTP
- jobs
- repositories
- assets
- audit

The strongest current runtime coverage for these hooks lives in:

These hooks are where multiple facades come together:

- repository access
- jobs
- audit
- outbound HTTP
- managed assets

That makes them the best example of "customer-owned logic through stable runtime contracts."

## Available Facades

The facade traits live in `crates/davenda-customer-sdk/src/facade.rs`.

Current families:

- `CommerceFacade`
  - product lookup
  - add order note
- `JobsFacade`
  - enqueue runtime jobs
- `RepositoryFacade`
  - read and write stable repository surfaces
- `AuthFacade`
  - capability checks and denial explanations
- `AuditFacade`
  - operator or hook audit records
- `OutboundHttpFacade`
  - approved integration HTTP only
- `AssetsFacade`
  - publish and inspect managed assets

Simple example:

```rust
let product = commerce.product("harbor-cap").await?;
audit.record("checkout.reviewed", "customer draft inspected").await?;
jobs.enqueue("crm.sync.contact", payload).await?;
```

The exact facade methods vary by family, but the pattern stays the same:

- customer code uses typed SDK services
- the runtime decides how those services are actually implemented

The extension traits in `RepositoryFacadeExt` are also worth reading because they show the current
stable repository surfaces directly:

- CMS pages
- CMS navigation
- CMS redirects
- commerce catalog product and collection
- commerce order lookup by id or payment reference

## What A Good Hook API Feels Like

You should be able to explain linked Rust hooks in one sentence:

"Customer-owned Rust implements explicit hook traits and talks to Davenda only through stable SDK facades."

If a customization needs private runtime types or deep internal crates, it is crossing the wrong boundary.

## Try It Locally

Useful commands:

```bash
cargo run -p shoppr -- describe
cargo run -p shoppr -- linked-backend describe
cargo run -p shoppr -- linked-backend demo
```

## Constraints

- Linked Rust is first-party only.
  - it is compiled into the customer binary
  - it is not runtime-installed like WASM
- Hooks must use the facades the runtime exposes.
  - they do not get arbitrary process internals
- The hook registry is explicit.
  - if a plugin does not register a hook, the runtime will not discover it magically

## When To Use Linked Rust Instead Of WASM

Use linked Rust when the logic is:

- customer-owned
- first-party
- tightly tied to product policy
- likely to evolve with the customer app workspace

Use WASM when the logic is:

- runtime-installed
- bounded
- grant-scoped
- more operationally swappable

## Full Implementation

Core SDK boundary:

- `crates/davenda-customer-sdk/src/registry.rs`
- `crates/davenda-customer-sdk/src/hooks.rs`
- `crates/davenda-customer-sdk/src/facade.rs`

Canonical Shoppr implementation:

- `apps/shoppr/backend/shoppr-loyalty-backend/src/lib.rs`
- `apps/shoppr/crates/shoppr-backend/src/lib.rs`
- `apps/shoppr/crates/shoppr-app/src/lib.rs`
- `apps/shoppr/crates/shoppr-bin/src/main.rs`

Canonical Gitly implementation:

- `apps/gitly/crates/gitly-backend/src/lib.rs`
- `apps/gitly/crates/gitly-app/src/lib.rs`
- `apps/gitly/crates/gitly-bin/src/main.rs`

Runtime coverage:

- `crates/davenda-runtime/src/tests/server.rs`

## Read Next

- [Customer Rust Vs Third-Party WASM](./customer-vs-wasm.md)
- [WASM Host APIs](./wasm-host-apis.md)
- [Shoppr Linked Rust Backend](../use-cases/shoppr/linked-rust-backend.md)
