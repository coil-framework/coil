---
title: API And Background Work
---

Gitly is the clearest non-commerce demo for two Davenda patterns:

- API-style endpoints inside a customer app
- bounded scheduled/background work through extensions and jobs

## API Routes In The Customer App

Read `apps/gitly/crates/gitly-app/src/lib.rs`.

That file mounts API-shaped endpoints for Gitly's product shell, including repository, pull, org,
user, and workflow data surfaces.

The linked backend in `apps/gitly/crates/gitly-backend/src/lib.rs` then provides the payload
builders used by those routes:

- `repo_api_payload()`
- `pulls_api_payload()`
- `workflow_api_payload()`
- `organization_api_payload()`
- `user_api_payload()`

This is the practical lesson: Davenda can support API-style product surfaces without making the
whole app API-first.

## The Community Pulse Extension

Gitly's API extension example lives in:

- `apps/gitly/extensions/gitly-community-pulse/package.toml`
- `apps/gitly/extensions/gitly-community-pulse/gitly-community-pulse.wat`

The customer app loads it in `apps/gitly/crates/gitly-app/src/extensions.rs`.

That package targets the API extension point and gives Gitly a bounded “community pulse” surface.

## Scheduled Job Demo

Gitly's scheduled-job example lives in:

- `apps/gitly/extensions/gitly-actions-scheduler/package.toml`
- `apps/gitly/extensions/gitly-actions-scheduler/gitly-actions-scheduler.wat`

The product surface that talks about it is:

- `apps/gitly/templates/gitly/actions.html`

This is intentionally a bounded demo. It shows the platform shape for scheduled work without
pretending Gitly is a full CI system.

## Where The Runtime Wiring Lives

The two important files are:

- `apps/gitly/crates/gitly-app/src/lib.rs`
- `apps/gitly/crates/gitly-app/src/extensions.rs`

Together they show:

- which extension slots Gitly exposes
- how installed packages are loaded
- how the customer app keeps API and scheduled-job behavior explicit

## Adapt This For Your Product

Copy this approach when you need:

- a small number of product-shaped JSON endpoints
- bounded scheduled work that should stay behind explicit contracts
- a non-commerce example of host APIs and extension slots

Do not copy it as a reason to move your whole product into client-side hydration. Gitly works
because the page shell stays server-rendered.

## Read Next

- [Non-Commerce Product Shape](./non-commerce-product-shape.md)
- [Customer Rust Vs Third-Party WASM](../../reference/customer-vs-wasm.md)
- [Ops Module Reference](../../reference/modules/ops.md)
