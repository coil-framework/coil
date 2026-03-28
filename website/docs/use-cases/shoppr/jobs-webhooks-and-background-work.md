---
title: Shoppr Jobs, Webhooks, And Background Work
---

Shoppr is the best checked-in example for Davenda’s production-style background-work story because
it combines:

- checkout
- Stripe handoff and webhook verification
- linked customer webhook hooks
- admin/operator visibility

## Where The Runtime Contract Starts

The shortest useful example is the runtime config shape:

```toml
[jobs]
backend = "redis"

[modules."commerce-payments-stripe"]
provider = "stripe"
checkout_mode = "hosted-checkout"
webhook_secret = { kind = "env", var = "STRIPE_WEBHOOK_SECRET" }
```

That tells you the real product story immediately:

- jobs are part of the live runtime, not a demo fiction
- payment-provider webhook handling is part of the checked-in config contract
- background work and settlement are first-class operational concerns

## Customer App Composition

Shoppr’s customer app and customer binary own:

- validate
- migrate
- asset publish
- serve/up

from the customer binary, while the runtime plan composes modules, auth, extensions, and linked
customer plugins.

## Linked Webhook Hooks

Shoppr’s linked backend example lives in:

- `apps/shoppr/backend/shoppr-loyalty-backend/src/lib.rs`
- `apps/shoppr/crates/shoppr-backend/src/lib.rs`

These are the right files to read when you want to understand:

- checkout review hooks
- verified webhook hooks
- verified webhook asset hooks
- customer-owned logic that still runs inside stable runtime boundaries

The hook traits themselves are in:

- `crates/davenda-customer-sdk/src/hooks.rs`

## Runtime Proof

The decisive runtime proof is not the templates. It is that the server contract already exercises:

- payment webhook signature validation
- Stripe-specific webhook verification
- replay protection across server reopen
- linked verified webhook hooks
- linked webhook repository access
- linked webhook jobs enqueue
- linked webhook managed-asset publication and inspection

That is why this page talks about webhook handling as a real platform story, not as documentation
wishful thinking.

## Operator Surfaces

Once background work exists, the app also needs operator visibility.

Read these templates:

- `apps/shoppr/templates/commerce/orders.html`
- `apps/shoppr/templates/commerce/order-detail.html`
- `apps/shoppr/templates/admin/audit.html`

Together they show:

- post-checkout status visibility
- provider-pending versus settled order states
- refund and support visibility
- audit traceability for privileged actions

## What Shoppr Teaches About Jobs

Shoppr is not a generic jobs demo app, but it still teaches the practical background-work split:

- the runtime owns queueing, leases, retries, and dead letters
- the app owns the product meaning of checkout, webhook side effects, and support visibility

For operator commands, the root CLI is still the main surface:

- `jobs status`
- `jobs ready`
- `jobs in-flight`
- `jobs dead-letters`
- `jobs retry`
- `jobs promote`
- `jobs run`

See `crates/davenda-cli/src/command.rs` and `crates/davenda-cli/src/cli/args.rs`.

## Full Implementation

If you want the full Shoppr implementation after learning the pattern:

- `apps/shoppr/platform.dev.toml`
- `apps/shoppr/crates/shoppr-app/src/lib.rs`
- `apps/shoppr/crates/shoppr-bin/src/main.rs`
- `apps/shoppr/backend/shoppr-loyalty-backend/src/lib.rs`
- `apps/shoppr/crates/shoppr-backend/src/lib.rs`
- `apps/shoppr/templates/commerce/checkout.html`
- `apps/shoppr/templates/commerce/checkout-confirmation.html`
- `apps/shoppr/templates/commerce/orders.html`
- `apps/shoppr/templates/admin/audit.html`
- `crates/davenda-runtime/src/tests/server.rs`

## Common Mistakes

- Do not document jobs as if the customer binary owns queue control.
  - that remains the platform/operator CLI surface
- Do not trust browser return from Stripe as payment truth.
  - Shoppr’s runtime and docs now treat webhook settlement as the real payment boundary
- Do not describe verified webhooks without pointing to replay protection and signature checks

## Read Next

- [Shoppr Checkout And Operations](./checkout-and-operations.md)
- [Shoppr Observability And Audit](./observability-and-audit.md)
- [CLI Commands](../../reference/cli-commands.md)
