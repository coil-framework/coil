---
title: Ops Module
---

The ops module owns search, reports, recovery, and audited bulk operations above the raw runtime
primitives.

Primary implementation files:

- `crates/davenda-ops/src/module/manifest.rs`
- `apps/shoppr/templates/admin/dashboard.html`

## Why It Exists

Storage, jobs, and cache primitives are not enough for a real operator experience. Product teams
also need:

- search rebuild workflows
- report exports
- recovery guidance
- bulk actions with audit and idempotency

That is the layer the ops module provides.

## What It Provides

From `crates/davenda-ops/src/module/manifest.rs`, ops adds:

- migrations for search, reports, and bulk operation state
- admin routes for `/admin/search`, `/admin/reports`, and `/admin/recovery`
- operator jobs for search rebuild, report export, bulk execute, and recovery rehydrate
- admin-widget and job extension slots

## How To Enable It

```toml title="app.toml"
[modules]
enabled = ["admin", "ops"]
```

Ops requires the admin shell, so enabling it without `admin` is not the right shape.

## How To Disable It

Remove `ops` from the enabled lists and remove any customer guidance that assumes search, reports,
recovery, or bulk routes exist.

## Config Expectations

Ops depends mainly on shared runtime services:

- jobs
- storage
- cache
- data
- auth

The concrete module behaviour changes depending on which other modules are installed, because ops
has optional dependencies on CMS, commerce, memberships, events, and media.

## Routes And Surfaces

Important routes:

- `/admin/search`
- `/admin/reports`
- `/admin/recovery`
- `/admin/bulk`

## Required Auth Capabilities

Ops requires:

- `admin.shell.access`
- `admin.audit.read`
- `system.module.manage`

It can also light up richer workflows when optional CMS, commerce, memberships, events, or media
capabilities are present.

## How Customer Apps Extend It

Ops exposes:

- admin widget slot: `ops.report.dashboard`
- job slot: `ops.search.adapter`

Customer apps typically extend ops by:

- adding customer dashboards or report widgets
- integrating search adapters
- wiring customer-specific recovery guidance into the operator story

Concrete example:

```html title="templates/admin/dashboard.html"
<section xmlns:dv="https://davenda.dev">
  <h3>Recovery</h3>
  <p>Last catalogue rebuild: <span dv:text="${ops.lastRebuildAt}">never</span></p>
</section>
```

Ops still owns the operator workflows and background jobs. The customer app owns the domain-specific
guidance and summary panels that make those workflows usable.

## Where To See It

Shoppr enables `ops` in both `apps/shoppr/app.toml` and `apps/shoppr/platform.dev.toml` and uses
the resulting admin surfaces as part of its operator walkthrough.

## Common Mistakes

- Confusing the ops module with the root CLI. The module owns product-level operator workloads, not
  every deployment command.
- Enabling ops without thinking through admin capabilities and audit.

## Read Next

- [Admin](./admin.md)
- [Shoppr Checkout And Operations](../../use-cases/shoppr/checkout-and-operations.md)
