---
title: Render Model Hooks
---

Render model hooks are the linked Rust API for customer-owned page shaping.

Use this hook family when the runtime should still own route resolution and template rendering, but
customer code needs to contribute additional model data before the final HTML is rendered.

This is the supported answer to both of these needs:

- mount a customer-owned namespace such as `crm_page` or `customer_extension`
- merge customer-owned fields into a shared runtime object such as `page`

If you need the full request-time context first, read
[Render pipeline and model composition](../core-concepts/render-pipeline-and-model-composition.md).

## What Problem This Solves

Without this hook, a customer can read the `RenderModel` type in the docs, but there is no public
handoff path that answers:

- where do I construct my own model?
- how do I give it to Coil?
- how do I add a top-level namespace safely?
- how do I intentionally participate in the existing `page` contract?

This hook makes that handoff explicit.

It does not replace route ownership, CMS storage, or the framework base model. It is the customer
contribution step in the existing render pipeline.

## The Hook Trait

Customer code registers `RenderModelHooks` through `CustomerHookRegistry`:

```rust
use coil_customer_sdk::{
    AuditFacade, BackendError, CustomerBackendPlugin, CustomerHookRegistry,
    CustomerPluginDescriptor, MergePolicy, RenderModelContribution, RenderModelHooks,
    RenderTarget, RepositoryFacade, RequestContext,
};
use coil_template::{RenderModel, RenderValue};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Default)]
pub struct CustomerPagesBackend;

impl CustomerBackendPlugin for CustomerPagesBackend {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        CustomerPluginDescriptor::new(
            "customer-pages-backend",
            "Customer Pages Backend",
            env!("CARGO_PKG_VERSION"),
        )
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        registry.register_render_model_hooks(Arc::new(*self))
    }
}
```

The hook itself returns explicit contributions:

```rust
impl RenderModelHooks for CustomerPagesBackend {
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

        let customer_namespace = RenderModel::new()
            .with_value("page_kind", RenderValue::text("crm-home"))
            .and_then(|model| {
                model.with_value("hero_title", RenderValue::text("Customer-managed homepage"))
            })
            .map_err(|error| {
                BackendError::new(
                    coil_customer_sdk::BackendErrorKind::Internal,
                    "render_model.invalid",
                    error.to_string(),
                )
            })?;

        let page_overlay = RenderModel::new()
            .with_value("render_source", RenderValue::text("linked-rust"))
            .map_err(|error| {
                BackendError::new(
                    coil_customer_sdk::BackendErrorKind::Internal,
                    "render_model.invalid",
                    error.to_string(),
                )
            })?;

        Ok(vec![
            RenderModelContribution::mount("customer_extension", customer_namespace)?,
            RenderModelContribution::merge("page", page_overlay, MergePolicy::FailOnConflict)?,
        ])
    }
}
```

That is the full handoff path:

1. your plugin registers a render-model hook
2. Coil builds the standard request model
3. Coil invokes your hook with request and route metadata
4. your hook returns one or more mount or merge contributions
5. Coil applies them deterministically
6. the final template render reads the combined model

What this hook does **not** do automatically:

- create routes
- create CMS page or block instances
- turn schema declarations into live `page.blocks`
- let customer code silently overwrite framework contracts unless merge policy allows it

## The Render Target

The hook receives a `RenderTarget` that tells you what is being rendered:

- `route_name`
- `template_name`
- `fragment_id`
- `site_id`
- `locale`
- `route_params`
- `query_params`

That means customer code can branch on the actual render target instead of guessing from raw paths.

Example:

```rust
if target.route_name == "commerce.product-detail" {
    let slug = target.route_params.get("product_slug");
}
```

## Mount Vs Merge

The API exposes two contribution kinds because they solve different problems.

### Mount

Use `RenderModelContribution::mount(...)` when the customer model should live under its own
namespace.

Example:

```rust
RenderModelContribution::mount("crm_page", crm_page_model)?
```

Templates then read:

```html
<h1 coil:text="${crm_page.hero_title}">Hero</h1>
```

Use mount when:

- the model is customer-owned
- the data should not collide with runtime-owned keys
- you want clear namespacing

Mount is the default choice for customer-specific request data.

### Merge

Use `RenderModelContribution::merge(...)` when customer code should intentionally participate in a
shared runtime object such as `page`.

Example:

```rust
RenderModelContribution::merge(
    "page",
    RenderModel::new()
        .with_value("render_source", RenderValue::text("linked-rust"))?,
    MergePolicy::FailOnConflict,
)?
```

Templates then read:

```html
<p coil:text="${page.render_source}">linked-rust</p>
```

Use merge when:

- the field belongs in an existing public contract
- the customer model should feel like part of the page model
- you want conflicts to be checked explicitly instead of silently overwritten

Merge is a narrower tool. Use it when the shared object really is the right public boundary.

## Conflict Policy

Merge behaviour is explicit through `MergePolicy`.

### `FailOnConflict`

This is the safe default.

Rules:

- missing keys are inserted
- nested object fields merge recursively
- identical scalar values are accepted
- conflicting scalar values fail
- list collisions fail

Use this when you want predictability and clear errors.

### `ReplaceExisting`

Customer values replace existing values on conflict.

Rules:

- missing keys are inserted
- nested object fields merge recursively
- conflicting scalar values are replaced
- conflicting lists are replaced

Use this only when customer code is intentionally taking ownership of part of a shared contract.

### `AppendLists`

List collisions append rather than fail.

Rules:

- missing keys are inserted
- nested object fields merge recursively
- list collisions append
- conflicting scalar values still fail

Use this for list-shaped page composition such as block collections.

## Shared Contract Vs Customer Namespace

Use this rule of thumb:

- if the data is customer-specific and not already part of a framework contract, mount it
- if the data belongs inside a shared object such as `page`, merge it explicitly

Do not treat `page` as a general customer scratch space. That makes upgrades harder and blurs the
public contract.

## Schema And Dynamic Block Reminder

Render-model hooks work on the request-time model. They do not replace the distinction between:

- content schema
- content instances
- request-time shaping

If your hook is contributing dynamic blocks or page-builder data, keep those layers explicit.

## End-To-End Example

This example shows both namespaced mounting and shared-page merging for a CRM-driven homepage.

### Hook implementation

```rust
use coil_customer_sdk::{
    AuditFacade, BackendError, BackendErrorKind, CustomerBackendPlugin,
    CustomerHookRegistry, CustomerPluginDescriptor, MergePolicy,
    RenderModelContribution, RenderModelHooks, RenderTarget, RepositoryFacade,
    RequestContext,
};
use coil_template::{RenderModel, RenderValue};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Default)]
pub struct CustomerPagesBackend;

impl CustomerBackendPlugin for CustomerPagesBackend {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        CustomerPluginDescriptor::new(
            "customer-pages-backend",
            "Customer Pages Backend",
            env!("CARGO_PKG_VERSION"),
        )
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        registry.register_render_model_hooks(Arc::new(*self))
    }
}

impl RenderModelHooks for CustomerPagesBackend {
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

        let blocks = vec![
            RenderModel::new()
                .with_value("type", RenderValue::text("hero_section"))
                .and_then(|model| {
                    model.with_value(
                        "title",
                        RenderValue::text("Customer-managed homepage"),
                    )
                })
                .map_err(|error| {
                    BackendError::new(
                        BackendErrorKind::Internal,
                        "render_model.invalid_block",
                        error.to_string(),
                    )
                })?,
            RenderModel::new()
                .with_value("type", RenderValue::text("text_band"))
                .and_then(|model| {
                    model.with_value(
                        "body",
                        RenderValue::text("This section came from linked Rust."),
                    )
                })
                .map_err(|error| {
                    BackendError::new(
                        BackendErrorKind::Internal,
                        "render_model.invalid_block",
                        error.to_string(),
                    )
                })?,
        ];

        let crm_page = RenderModel::new()
            .with_value("page_kind", RenderValue::text("crm-home"))
            .and_then(|model| model.with_list("blocks", blocks.clone()))
            .map_err(|error| {
                BackendError::new(
                    BackendErrorKind::Internal,
                    "render_model.invalid_crm_page",
                    error.to_string(),
                )
            })?;

        let page_overlay = RenderModel::new()
            .with_value("render_source", RenderValue::text("linked-rust"))
            .and_then(|model| model.with_list("blocks", blocks))
            .map_err(|error| {
                BackendError::new(
                    BackendErrorKind::Internal,
                    "render_model.invalid_page_overlay",
                    error.to_string(),
                )
            })?;

        Ok(vec![
            RenderModelContribution::mount("crm_page", crm_page)?,
            RenderModelContribution::merge("page", page_overlay, MergePolicy::FailOnConflict)?,
        ])
    }
}
```

### Template usage

```html
<main xmlns:coil="https://coil.rs">
  <p class="source" coil:text="${page.render_source}">linked-rust</p>

  <div coil:each="block : ${page.blocks}">
    <div coil:switch="${block.type}">
      <section coil:case="'hero_section'">
        <h1 coil:text="${block.title}">Hero</h1>
      </section>
      <section coil:case="'text_band'">
        <p coil:text="${block.body}">Text</p>
      </section>
      <section coil:default>
        Unsupported block type.
      </section>
    </div>
  </div>
</main>
```

That is the intended pattern:

- customer code shapes page data in Rust
- the hook mounts any private namespace it needs
- the hook merges selected fields into `page`
- the template consumes the shaped model without inventing its own data access path

## Fragment Dispatch By Block Type

If your page is block-driven, you do not need to hardcode every branch inline.

Coil now exposes runtime block dispatch fields inside `coil:each` loops over objects with a `type`
field:

- `block.is_<type>`
- `block.render_fragment`
- `block.render_fragment_shared`

That means a hook can merge blocks into `page.blocks`, and the template can dispatch fragments by
type:

```html
<div coil:each="block : ${page.blocks}">
  <coil:block coil:replace-fragment="${block.render_fragment}"></coil:block>
</div>
```

This works well when customer code is constructing page blocks dynamically and the theme already has
fragment templates for the block types.

## Repository Access During Render

Render model hooks receive `RepositoryFacade`, but the render-time surface is intentionally narrower
than request-mutation hooks.

Currently available reads:

- `cms.pages`
- `cms.navigation`
- `cms.redirects`
- `commerce.catalog.products`
- `commerce.catalog.collections`

Important current limits:

- render model hooks are read-only
- `RepositoryFacade::write(...)` is rejected during render
- `commerce.orders` is not exposed during render

That boundary is intentional. The render path should stay deterministic and side-effect-light.

## Audit Access During Render

The hook also receives `AuditFacade`.

Use it when the render contribution itself is operationally meaningful, for example:

- recording that a regulated customer page variant was selected
- recording which CRM policy branch shaped a page
- recording that a customer-owned personalization rule ran

Do not use audit writes as a substitute for model data. The hook should still return explicit
render-model contributions.

## Error Behaviour

Coil fails closed when a contribution is invalid.

Examples:

- mount path already exists
- merge target is not an object
- `FailOnConflict` sees a conflicting scalar value
- `AppendLists` is used against scalar conflicts

A conflicting merge reports the exact path, for example:

```text
render model conflict at `page.title`: existing value differs from contribution
```

That is deliberate. Silent overwrite is worse than a clear render failure.

## When To Mount And When To Merge

Use `mount(...)` when:

- the namespace is customer-owned
- the data is not part of the standard page contract
- you want to avoid collisions entirely

Use `merge(...)` when:

- the template should read the data from `page`, `navigation`, or another shared object
- the data belongs to an existing public contract
- you want conflicts checked explicitly

## What To Read Next

- [Linked Rust Hook APIs](./linked-rust-hook-apis.md)
- [Template Models](./template-models.md)
- [Template Language](./template-language.md)
- [Request And Render Lifecycle](../core-concepts/request-and-render-lifecycle.md)
- [Content schema vs content instances](../core-concepts/content-schema-vs-content-instances.md)
- [Dynamic blocks and live-data sections](../core-concepts/dynamic-blocks-and-live-data-sections.md)
- [CMS page builder model](./cms-page-builder-model.md)
- [Getting Started: Add Dynamic Blocks](../getting-started/add-dynamic-blocks.md)
