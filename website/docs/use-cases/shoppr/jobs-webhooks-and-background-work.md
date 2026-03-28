---
title: Shoppr Jobs, Webhooks, And Background Work
---

Shoppr is not the scheduler showcase app. It is the best public example of why background work
matters to a real store.

Use it to learn:

- how checkout hands work off to webhook and queue-driven follow-up
- what the operator needs to inspect after browser return
- where the customer app ends and the platform jobs surface begins

## Start With The Live Runtime Contract

Shoppr’s development config already says the important part out loud:

```toml
[jobs]
backend = "redis"

[modules."commerce-payments-stripe"]
provider = "stripe"
checkout_mode = "hosted-checkout"
webhook_secret = { kind = "env", var = "STRIPE_WEBHOOK_SECRET" }
```

That gives you the real product boundary:

- requests are not the whole payment flow
- settlement depends on webhook handling
- background work must be operationally visible

## What Developers Can Actually Exercise Locally

Without real Stripe credentials, Shoppr still runs through the built-in local hosted-checkout stub.

With real Stripe test credentials, use:

```bash
stripe listen --forward-to http://uk.localhost:8080/webhooks/commerce/payment-provider
```

That is the shortest honest path to see:

1. request-path checkout
2. provider return
3. webhook callback
4. resulting order/operator state

## The Product-Side Operator Story

The admin orders screen is the main product surface to study:

```html
<p>
  This queue is store-wide. Use it to confirm payment state, review checkout email and totals,
  and move into the per-order support detail view before escalating a checkout case.
</p>
```

And the same template teaches the real support boundary:

```html
<p>
  After a Stripe return, compare the customer account view and provider callback window before
  treating Pending Payment as a failed checkout.
</p>
```

That is why Shoppr matters here. It does not just say “jobs exist.” It shows the operator
consequence of async settlement in the product itself.

## Where The Customer Binary Stops

The Shoppr customer binary owns:

- `validate`
- `migrate apply`
- `assets publish`
- `up`

The customer binary does **not** own queue control. Jobs remain an operator/platform concern.

Use the platform CLI for that:

```bash
cargo run -p davenda-cli -- jobs status --config apps/shoppr/platform.dev.toml
cargo run -p davenda-cli -- jobs ready --config apps/shoppr/platform.dev.toml --queue jobs.work --limit 25
cargo run -p davenda-cli -- jobs dead-letters --config apps/shoppr/platform.dev.toml --queue jobs.dead-letter --limit 25
cargo run -p davenda-cli -- jobs run --config apps/shoppr/platform.dev.toml --worker-id worker-a --limit 25
```

That split is intentional:

- customer binaries compose the product
- the platform CLI operates the shared job system

## Linked Customer Hooks In This Flow

Shoppr’s linked backend is relevant because webhook handling is not purely native module code.

The checked-in backend demonstrates:

- checkout review hooks
- verified webhook hooks
- customer-owned logic running through the stable runtime boundary

Read those after you understand the product flow:

- `apps/shoppr/crates/shoppr-backend/src/lib.rs`
- `apps/shoppr/backend/shoppr-loyalty-backend/src/lib.rs`

But start with the product pages and config above. That is where the behavior becomes real for a
new developer.

## What Shoppr Teaches Better Than Gitly

Gitly is the clearer scheduler-slot demo.

Shoppr is the clearer example for:

- webhook-driven follow-up
- customer-visible payment state
- order-support consequences of async work
- audit/operator traces around privileged actions

So if you are building commerce, learn the operator meaning of async work from Shoppr first.

## Honest Limits

Shoppr is still not a public “queue tutorial app.” It does not ship a polished in-app worker
dashboard or a demo page dedicated to ready/dead-letter queue internals.

That is why the public jobs operator learning path still spans:

- Shoppr for product and webhook consequences
- the platform CLI for queue control
- Gitly for a bounded scheduled-job extension example

## Read Next

- [Shoppr Observability And Audit](./observability-and-audit.md)
- [Jobs and schedulers](../../operations/jobs-and-schedulers.md)
- [CLI Commands](../../reference/cli-commands.md)
