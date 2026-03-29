---
title: Shoppr Observability And Audit
---

Shoppr is the strongest public example of operator-facing trust signals in a real customer app.

Use it to study three concrete things:

- runtime observability toggles in app config
- audit visibility in the admin shell
- order/admin surfaces that stay truthful after checkout and webhook follow-up

## Start With The Config

Shoppr enables observability in the same file a developer already uses to boot the app:

```toml
[observability]
metrics = true
tracing = true

[jobs]
backend = "redis"
```

That lives in `apps/shoppr/platform.dev.toml`.

This is the first useful lesson: observability is part of the runtime contract the customer app
ships, not a hidden post-deploy tweak.

## The Audit Page Is A Real Surface

The Shoppr audit template is a good public example because it tells you exactly what operators can
expect:

```html
<p>
  Backend <code coil:text="${audit_backend}">local-sqlite</code> at
  <code coil:text="${audit_location}">/var/lib/coil/shared-state</code> with
  <strong coil:text="${audit_entry_count}">0</strong> recorded entries.
</p>
...
<tr coil:each="entry : ${audit_entries}">
  <td coil:text="${entry.when}">1764223200</td>
  <td coil:text="${entry.actor}">operator-live-1</td>
  <td coil:text="${entry.action}">Issue refund</td>
  <td coil:text="${entry.capability}">order.refund.issue</td>
</tr>
```

That teaches the right operator questions:

- who acted
- what they did
- which capability/resource changed
- whether the action succeeded

## Orders Are Part Of Observability Too

Shoppr’s observability story is not just `/admin/audit`.

The orders page is also teaching operator truth:

```html
<p>
  This queue is store-wide. Use it to confirm payment state, review checkout email and totals,
  and move into the per-order support detail view before escalating a checkout case.
</p>
```

And:

```html
<p>
  After a Stripe return, compare the customer account view and provider callback window before
  treating Pending Payment as a failed checkout.
</p>
```

That is observability in the product, not just in logs.

## What A New Developer Should Actually Open

Once Shoppr is running, inspect:

- `/ready`
- `/admin`
- `/admin/audit`
- `/admin/orders`

That sequence is more useful than starting in runtime internals, because it shows how operational
signals appear in the actual store.

## Linked Customer Hooks Still Fit The Same Boundary

Shoppr’s linked backend can record customer-owned evidence through the stable runtime facade. That
matters because customer logic should not invent a second audit pipeline beside the app.

The right public takeaway is:

- native admin actions and customer hooks should land in one operator-history lane
- the app surfaces that evidence through the shared admin shell

## Honest Limits

Shoppr does not yet ship:

- a public dashboard definition for metrics
- a trace backend walkthrough
- a second, non-commerce app with comparable audit visibility

So copy Shoppr for:

- audit UI shape
- truthful operator empty states
- post-checkout/admin support visibility

Do not claim the repo already contains a complete production monitoring stack tutorial.

## Read Next

- [Shoppr Jobs, Webhooks, And Background Work](./jobs-webhooks-and-background-work.md)
- [Observability, monitoring, and audit](../../operations/observability.md)
- [Environment Variables](../../reference/environment-variables.md)
