---
title: Admin Module
---

The admin module provides the shared back-office shell and audit entry surfaces that other modules
plug into.

Primary implementation files:

- `crates/coil-admin/src/module/manifest.rs`
- `apps/shoppr/templates/admin/dashboard.html`
- `apps/shoppr/templates/admin/audit.html`

## Why It Exists

Coil does not want every domain module to invent its own operator shell. The admin module gives
the platform one shared place for:

- admin entry
- shared navigation
- audit visibility
- customer widgets inside the shell

## What It Provides

From `crates/coil-admin/src/module/manifest.rs`, admin adds:

- an audit-log migration
- `/admin`
- `/admin/audit`
- an audit export operator job
- an admin dashboard widget slot

## How To Enable It

```toml title="app.toml"
[modules]
enabled = ["admin"]
```

Shoppr and Gitly both enable it.

## How To Disable It

Remove `admin` from the enabled module lists. Be aware that other modules frequently declare
optional or required relationships to the shared admin shell for operator navigation.

## Config Expectations

Admin does not need a large module-specific config block in the demos. The important dependencies
are:

- auth
- template loading
- observability
- HTTP runtime

## Routes And Surfaces

Important routes:

- `/admin`
- `/admin/audit`

## Required Auth Capabilities

Admin requires:

- `admin.shell.access`
- `admin.audit.read`

Optional system capabilities add richer config and module management surfaces when available.

## How Customer Apps Extend It

Admin exposes:

- admin widget slot: `admin.dashboard.summary`

Customer apps usually extend it by:

- shipping their own admin templates
- letting official modules contribute resources into the shared shell
- adding customer summary widgets

Concrete example:

```html title="templates/admin/dashboard.html"
<section xmlns:coil="https://coil.rs">
  <h2>Shoppr operator overview</h2>
  <div coil:insert="~{admin/widgets/revenue-summary}"></div>
</section>
```

The admin module still owns shell access, audit routes, and shared operator navigation. The
customer app owns the dashboard content that appears inside that shell.

The practical sequence is:

1. enable `admin`
2. provide `templates/admin/dashboard.html`
3. add customer-owned summary widgets or dashboard fragments
4. keep audit and operator access inside the shared admin shell rather than inventing a second one

## Where To See It

- `apps/shoppr/templates/admin/dashboard.html`
- `apps/shoppr/templates/admin/audit.html`
- `apps/gitly/templates/...` through the shared admin shell when enabled

## Common Mistakes

- Treating admin as “just templates.” The shared capability and audit model matters.
- Forgetting that audit visibility is its own route and capability.

## Read Next

- [Ops](./ops/)
- [Shoppr Checkout And Operations](../../use-cases/shoppr/checkout-and-operations/)
