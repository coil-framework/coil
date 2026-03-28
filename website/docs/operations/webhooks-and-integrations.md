---
title: Webhooks And Integrations
---

Davenda treats webhook ingress as an operational boundary, not just "some HTTP endpoint."

## What Is This?

This page covers how to operate inbound webhook and external integration paths in Davenda.

## Why Does This Matter?

Webhook failures are rarely isolated:

- retries can pile up
- signatures can drift
- releases can change payload assumptions
- duplicate side effects can appear quickly

A safe integration story requires host-owned verification and operator-visible state.

## Concrete Repo Examples

### Shoppr payment provider callback

Shoppr's public ecommerce flow includes the payment provider callback path:

- `/webhooks/commerce/payment-provider`

Relevant config:

- `apps/shoppr/platform.dev.toml`
- `apps/shoppr/platform.toml`
- `STRIPE_WEBHOOK_SECRET` in `apps/shoppr/.env.example`

### Shoppr sidecar CRM webhook example

The optional Shoppr loyalty sidecar gives a second checked-in webhook example:

- `POST http://localhost:8081/webhooks/crm/contact-updated`

Relevant files:

- `apps/shoppr/backend/shoppr-loyalty-backend/src/http.rs`
- `apps/shoppr/backend/README.md`
- `HARBOR_BACKEND_WEBHOOK_SECRET` in `apps/shoppr/.env.example`

## How To Test Locally

For Stripe-style local forwarding, Shoppr already documents:

```bash
stripe listen --forward-to http://localhost:8080/webhooks/commerce/payment-provider
```

For the CRM sidecar example, the checked-in README documents a direct `curl` flow against
`/webhooks/crm/contact-updated`.

## What Operators Should Verify

Before trusting a webhook path in production, verify:

- the endpoint is reachable
- the right secret is configured
- signature or shared-secret verification is enabled
- retries are understood
- duplicate delivery is safe or rejected correctly
- downstream dependencies are healthy

## Linked Rust And Webhooks

Shoppr also demonstrates verified webhook handling through linked customer Rust. That matters
because it shows the first-party customer path without exposing unstable runtime internals.

Current public example:

- `apps/shoppr/crates/shoppr-backend/`

## Common Mistakes

### Treating webhook handling as a pure app concern

Ingress verification, retries, and operator visibility are operational concerns too.

### Forgetting local secret parity

If the local secret path is undocumented, developers will end up "testing" with unsigned payloads
that bypass the real behavior.

### Ignoring duplicate delivery

A webhook system that cannot tolerate retries is not production-ready.

## What To Read Next

- [Configuration and secrets](configuration-and-secrets.md)
- [Troubleshooting](troubleshooting.md)
- [Observability, monitoring, and audit](observability.md)
