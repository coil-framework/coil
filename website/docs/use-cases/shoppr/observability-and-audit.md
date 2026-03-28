---
title: Shoppr Observability And Audit
---

Shoppr is the best place in the repo to understand how Davenda surfaces operator trust signals in a
real customer app.

Use this page when you want to answer:

- where metrics, tracing, and audit are enabled
- which admin pages expose the resulting state
- how linked customer hooks participate in audit

## Start With Runtime Config

Shoppr’s local observability settings live in `apps/shoppr/platform.dev.toml`.

The relevant blocks are:

- `[observability]`
  - `metrics = true`
  - `tracing = true`
- `[jobs]`
  - background-work backend
- `[cache]`
  - L1 and L2 cache setup

That file is the operational contract. The templates are just the UI on top.

## The Main Operator Surfaces

Read these templates together:

- `apps/shoppr/templates/admin/dashboard.html`
- `apps/shoppr/templates/admin/audit.html`
- `apps/shoppr/templates/commerce/orders.html`
- `apps/shoppr/templates/commerce/order-detail.html`

These pages are intentionally honest about what the app can and cannot yet do.

## Audit Is A Real Runtime Surface

The Shoppr audit page is not just placeholder copy anymore. The current template in
`apps/shoppr/templates/admin/audit.html` expects:

- `auditBackend`
- `auditLocation`
- `auditEntryCount`
- `hasAuditEntries`
- `auditEntries`

Those are runtime-backed fields shaped in `crates/davenda-runtime/src/render/model.rs`.

This is important because it means the product is teaching a real audit boundary:

- who acted
- what they did
- what capability it mapped to
- what resource changed
- whether the action succeeded

## Where Audit Records Come From

There are two main sources in the current design:

- native admin/operator actions
- linked customer hook audit calls through `AuditFacade`

The linked-customer audit facade lives in:

- `crates/davenda-runtime/src/render/model.rs`
- `crates/davenda-customer-sdk/src/facade.rs`

The shared metadata audit persistence path lives in:

- `crates/davenda-runtime/src/wasm/host/services/metadata/shared.rs`

That means the same system can record:

- CMS/admin actions
- order operations
- customer hook side effects

## Shoppr Examples To Read

### Admin audit UI

Read:

- `apps/shoppr/templates/admin/audit.html`

This template is a good example because it teaches both states:

- truthful empty state
- real audit table once entries exist

### Order operations

Read:

- `apps/shoppr/templates/commerce/orders.html`
- `apps/shoppr/templates/commerce/order-detail.html`

These templates show how audit and operator history matter in support work, not just in an abstract
"audit subsystem."

### Linked customer backend

Read:

- `apps/shoppr/backend/shoppr-loyalty-backend/src/lib.rs`
- `apps/shoppr/crates/shoppr-backend/src/lib.rs`

These files show where customer-owned Rust can add audit-aware logic without escaping the stable
host boundary.

## Tests That Prove The Boundary

The strongest runtime coverage lives in:

- `crates/davenda-runtime/src/tests/server.rs`

Relevant test areas in that file cover:

- diagnostics probe access control
- metadata audit backend selection
- verified webhook hook execution
- linked customer asset and repository behaviour

If you want proof the observability boundary is real, not just documented, start there.

## What A New Developer Should Copy

From Shoppr, copy this pattern:

1. enable observability in `platform.dev.toml`
2. expose honest admin surfaces in templates
3. shape runtime audit and operator fields in the render model
4. let linked hooks record audit entries through the facade instead of inventing side channels

## Common Mistakes

- Do not describe an audit page as available if the template only has placeholder prose.
- Do not let customer hooks write their own parallel audit log outside the stable facade.
- Do not hide observability expectations only in operator docs.
  - surface them in the customer app config and templates too

## Read Next

- [Jobs, Webhooks, And Background Work](./jobs-webhooks-and-background-work.md)
- [Shoppr Checkout And Operations](./checkout-and-operations.md)
- [Environment Variables](../../reference/environment-variables.md)
