---
title: Jobs And Schedulers
---

Coil treats background work as a first-class operator surface, but the checked-in public apps
demonstrate different parts of that story.

- Gitly is the clearest public example of a scheduled-job contract plus a bounded runtime-installed
  extension.
- Shoppr is the clearest public example of why jobs and webhooks matter to a real product flow:
  checkout return, payment settlement, order visibility, and operator follow-up.

## Start With The Runtime Contract

Every checked-in app declares a shared jobs backend in platform config:

```toml
[jobs]
backend = "redis"
```

You can see that in:

- `apps/shoppr/platform.dev.toml`
- `apps/gitly/platform.dev.toml`

That is the first practical point to copy: background work is part of runtime configuration, not a
theme or frontend concern.

## What The Public Examples Actually Demonstrate

### Gitly: a bounded scheduled-job contract

Gitly declares a real runtime-installed scheduled-job extension in `app.toml`:

```toml
[[extensions]]
id = "gitly-actions-scheduler"

[[extensions.handlers]]
id = "nightly-refresh"
```

And the Actions page makes the contract explicit in the product itself:

```html
<p data-i18n="actions.scheduleBody">
  The `github.actions.refresh` surface is declared by the Gitly customer module and can be
  fulfilled by a runtime-installed scheduled-job extension.
</p>
<p data-i18n="actions.mockBody">
  This browser-side loop simulates a scheduled refresh so the Actions demo shows visible cadence
  instead of static counters only.
</p>
```

That is an honest public example:

- the extension contract is real
- the runtime-installed package is real
- the visible heartbeat on the page is still a browser-side simulation, not a polished production
  worker demo

So use Gitly to learn the extension and operator contract, not to study a full end-to-end
scheduler product.

### Shoppr: a real product consequence of jobs and webhooks

Shoppr is where background work becomes operationally meaningful:

```toml
[jobs]
backend = "redis"

[modules."commerce-payments-stripe"]
provider = "stripe"
checkout_mode = "hosted-checkout"
webhook_secret = { kind = "env", var = "STRIPE_WEBHOOK_SECRET" }
```

And the operator-facing orders page teaches the support consequence directly:

```html
<p>
  This queue is store-wide. Use it to confirm payment state, review checkout email and totals,
  and move into the per-order support detail view before escalating a checkout case.
</p>
```

That is the right way to read Shoppr:

- request-path checkout is not the whole story
- settlement and follow-up work continue after browser return
- the operator must be able to inspect resulting order state

## Operator Commands To Learn First

The customer binaries do not own queue control. The operator surface is still the platform CLI.

Start with:

```bash
cargo run -p coil-cli -- jobs status --config apps/shoppr/platform.dev.toml
cargo run -p coil-cli -- jobs ready --config apps/shoppr/platform.dev.toml --queue jobs.work --limit 25
cargo run -p coil-cli -- jobs dead-letters --config apps/shoppr/platform.dev.toml --queue jobs.dead-letter --limit 25
cargo run -p coil-cli -- jobs run --config apps/shoppr/platform.dev.toml --worker-id worker-a --limit 25
```

Those four commands cover the minimum real workflow:

1. inspect the queues
2. inspect ready work
3. inspect failures
4. run a worker

Only then should you move on to `jobs in-flight`, `jobs retry`, and `jobs promote`.

## Local Product Examples To Copy

### For webhook-driven commerce work

Use Shoppr’s local Stripe forwarding path:

```bash
stripe listen --forward-to http://uk.localhost:8080/webhooks/commerce/payment-provider
```

That gives you a concrete local story for:

- browser redirect out to a provider
- webhook return into the runtime
- resulting order/admin visibility

### For scheduled extension work

Use Gitly’s Actions surface:

- `/forgeflow/platform-ui/actions`
- `apps/gitly/extensions/gitly-actions-scheduler/package.toml`
- `apps/gitly/templates/gitly/actions.html`

That shows how to expose a bounded scheduled-job slot in a customer app without pretending the demo
already ships a full production automation product.

## What Belongs In Jobs

Good candidates:

- verified webhook follow-up
- scheduled refresh or reconciliation work
- retryable integration side effects
- exports and bulk operations
- operational recovery tasks

Bad candidates:

- request-path work that must complete before the user gets a truthful answer
- frontend timers pretending to be durable background processing

Gitly intentionally uses the second pattern only for visible demo cadence. The docs should not
confuse that with durable queue execution.

## Honest Limits In The Public Examples

The public repo is strong on:

- operator queue inspection and worker commands
- a real scheduled-job extension slot in Gitly
- a real webhook-driven product consequence in Shoppr

The public repo is still thinner on:

- a polished linked-Rust customer job-definition tutorial
- a single public example app whose visible scheduled work is entirely runtime-driven rather than
  partly browser-simulated

That means the jobs operator model is real today, but the demos still split the story across two
apps for clarity.

## Read Next

- [Observability, monitoring, and audit](../observability/)
- [Troubleshooting](../troubleshooting/)
- [Shoppr Jobs, Webhooks, And Background Work](../use-cases/shoppr/jobs-webhooks-and-background-work/)
