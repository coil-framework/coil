---
title: Linked Rust Hook APIs
---

Linked Rust backends are the first-party customer extension model in Coil.

This page explains the public hook surface as it exists today:

- how a customer plugin registers hooks
- which hook families are supported
- what each family is for
- how render-model hooks fit into page rendering

## The Plugin Boundary

Every linked backend starts the same way:

```rust
use coil_customer_sdk::{
    BackendError, CustomerBackendPlugin, CustomerHookRegistry, CustomerPluginDescriptor,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct CustomerBackend;

impl CustomerBackendPlugin for CustomerBackend {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        CustomerPluginDescriptor::new(
            "customer-backend",
            "Customer Backend",
            env!("CARGO_PKG_VERSION"),
        )
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        Ok(())
    }
}
```

That trait is the top-level contract.

The plugin declares:

- its stable identity
- its display name
- its version
- which hook families it wants to register

Coil does not scan random customer code for magic functions. Registration is explicit.

## Hook Families

The registry currently supports five hook families:

- `register_checkout_hooks(...)`
- `register_cms_hooks(...)`
- `register_render_model_hooks(...)`
- `register_verified_webhook_hooks(...)`
- `register_verified_webhook_asset_hooks(...)`

Each one is a different runtime boundary.

## A Real Registration Example

This is the normal shape for a plugin that participates in checkout and render-model shaping:

```rust
use coil_customer_sdk::{
    BackendError, CheckoutHooks, CustomerBackendPlugin, CustomerHookRegistry,
    CustomerPluginDescriptor, RenderModelHooks,
};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Default)]
pub struct CustomerBackend;

impl CustomerBackendPlugin for CustomerBackend {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        CustomerPluginDescriptor::new(
            "customer-backend",
            "Customer Backend",
            env!("CARGO_PKG_VERSION"),
        )
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        let hooks = Arc::new(*self);
        registry.register_checkout_hooks(hooks.clone())?;
        registry.register_render_model_hooks(hooks)?;
        Ok(())
    }
}
```

That is the public model:

- one plugin can implement multiple hook families
- registration order is explicit
- only registered hooks are invoked

## Checkout Hooks

Trait:

```rust
CheckoutHooks::review_order(...)
```

Use checkout hooks for:

- customer-specific checkout rules
- CRM routing at checkout time
- fraud or fulfilment annotations
- first-party product policy around order approval

The hook receives:

- `RequestContext`
- `OrderDraft`
- `CommerceFacade`
- `AuthFacade`
- `AuditFacade`

That gives customer code a controlled way to inspect the draft order and return an explicit review
decision without reaching into runtime internals.

## CMS Publish Hooks

Trait:

```rust
CmsHooks::validate_page_publish(...)
```

Use CMS hooks for:

- editorial validation
- content policy enforcement
- customer-specific workflow rules before publication

The hook receives:

- `RequestContext`
- `CmsPageDraft`
- `RepositoryFacade`
- `AuditFacade`

That lets customer code reject invalid drafts through a stable, bounded contract.

## Render Model Hooks

Trait:

```rust
RenderModelHooks::contribute_render_model(...)
```

This is the hook family for customer-owned page shaping.

Use it when:

- the runtime should still own route resolution and template rendering
- customer code needs to mount a top-level namespace such as `crm_page`
- customer code needs to merge fields into a shared object such as `page`
- page structure is derived from customer-owned data at request time

The hook receives:

- `RequestContext`
- `RenderTarget`
- `RepositoryFacade`
- `AuditFacade`

And it returns:

- `Vec<RenderModelContribution>`

That is the explicit handoff path for customer-owned render-model data.

Simple example:

```rust
use coil_customer_sdk::{
    AuditFacade, BackendError, BackendErrorKind, MergePolicy, RenderModelContribution,
    RenderModelHooks, RenderTarget, RepositoryFacade, RequestContext,
};
use coil_template::{RenderModel, RenderValue};

impl RenderModelHooks for CustomerBackend {
    fn contribute_render_model(
        &self,
        _ctx: &RequestContext,
        target: &RenderTarget,
        _repositories: &dyn RepositoryFacade,
        _audit: &dyn AuditFacade,
    ) -> Result<Vec<RenderModelContribution>, BackendError> {
        if target.route_name != "home" {
            return Ok(Vec::new());
        }

        let mounted = RenderModel::new()
            .with_value("page_kind", RenderValue::text("crm-home"))
            .map_err(|error| {
                BackendError::new(
                    BackendErrorKind::Internal,
                    "render_model.invalid",
                    error.to_string(),
                )
            })?;

        let overlay = RenderModel::new()
            .with_value("render_source", RenderValue::text("linked-rust"))
            .map_err(|error| {
                BackendError::new(
                    BackendErrorKind::Internal,
                    "render_model.invalid",
                    error.to_string(),
                )
            })?;

        Ok(vec![
            RenderModelContribution::mount("customer_extension", mounted)?,
            RenderModelContribution::merge("page", overlay, MergePolicy::FailOnConflict)?,
        ])
    }
}
```

Read the full detailed contract in [Render Model Hooks](./render-model-hooks.md).

## Verified Webhook Hooks

Traits:

- `VerifiedWebhookHooks::handle_verified_webhook(...)`
- `VerifiedWebhookAssetHooks::handle_verified_webhook(...)`

Use these for:

- customer-owned webhook policy after verification
- integration workflows
- bounded asset publication or inspection from trusted webhook flows

These hooks are intentionally later in the lifecycle. The runtime verifies and normalizes the
incoming webhook first, then invokes customer code through a stable SDK surface.

## Facade Philosophy

Linked Rust hooks do not receive the whole runtime.

They receive stable facades instead:

- commerce
- auth
- repository
- audit
- jobs
- outbound HTTP
- assets

That is deliberate. It keeps the platform boundary explicit and keeps customer code out of private
runtime internals.

## What Linked Rust Is Good For

Good linked Rust use cases:

- checkout policy
- customer-owned content validation
- request-time page shaping
- verified integration policy
- first-party audit and product workflow rules

Bad linked Rust use cases:

- replacing core runtime services wholesale
- depending on private runtime modules instead of the SDK
- treating customer hooks as a back door into arbitrary framework internals

## The Most Important Distinction

Use linked Rust when the code is:

- customer-owned
- trusted as first-party product code
- close to runtime decisions such as rendering, checkout, or verified integrations

Use WASM when the code is:

- lower-trust
- third-party
- bounded to marketplace-style extension points

Those are intentionally different trust boundaries.

## What To Read Next

- [Render Model Hooks](./render-model-hooks.md)
- [Template Models](./template-models.md)
- [Template Language](./template-language.md)
- [Customer Rust vs third-party WASM](./customer-vs-wasm.md)
- [Request And Render Lifecycle](../core-concepts/request-and-render-lifecycle.md)
