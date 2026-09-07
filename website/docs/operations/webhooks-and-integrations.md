---
title: Webhooks And Integrations
---

Coil treats webhook ingress as an operational boundary, not just "some HTTP endpoint."

## What Is This?

This page covers how to operate inbound webhooks and adjacent integration paths safely.

## Why Does This Matter?

Webhook failures are rarely isolated:

- retries pile up
- signatures drift
- releases change payload assumptions
- duplicate side effects appear quickly

A safe integration story needs host-owned verification, idempotent handling, and operator-visible
state.

## The Canonical Webhook Model

Treat webhook ingress as a four-stage flow:

1. accept the request on a dedicated route
2. verify the signature or shared secret before business logic runs
3. normalize the payload into a known event shape
4. record or route the result through the runtime and operator surfaces

That keeps webhook handling from becoming a pile of ad hoc controller code.

## What Operators Should Verify

Before trusting a webhook path in production, verify:

- the endpoint is reachable from the provider
- the correct secret is configured
- signature or shared-secret verification is enabled
- duplicate delivery is safe
- downstream dependencies are healthy
- retry behaviour is understood

## A Practical Local Test Flow

For provider callbacks, the minimal operator-safe test loop is:

1. run the app locally
2. configure the local webhook secret
3. forward provider traffic to the local callback route
4. confirm verification succeeds
5. confirm duplicate or invalid calls fail closed

A representative command shape is:

```bash
stripe listen --forward-to http://localhost:8080/webhooks/commerce/payment-provider
```

You can apply the same pattern to non-Stripe providers: forward, verify, observe, and replay
carefully.

## Linked Rust And Webhook Handling

If the webhook behaviour is truly customer-owned product logic, linked customer Rust is the right
place for the business rule.

That still does not mean the webhook should bypass the host. The runtime should own ingress and
verification before customer code handles the verified event.

## Sidecars And External Integrations

A separate process boundary can still be correct when:

- a provider integration is operationally independent
- the integration needs a different scaling or security posture
- the boundary is genuinely external-facing

Use that boundary intentionally, not because the framework lacks a first-party customization path.

## Supporting Repo Examples

The checked-in examples prove two useful variants:

- Shoppr payment-provider callbacks through the main app runtime
- Shoppr's optional CRM webhook sidecar with explicit shared-secret verification

Those examples are worth reading after this page if you want concrete implementation detail, but
the pattern above is the primary teaching model.

## Common Mistakes

### Treating webhook handling as just another controller

Ingress verification and replay safety are operational concerns, not just app code details.

### Testing only with unsigned local requests

That produces a fake green path and hides the real verification behaviour.

### Ignoring duplicate delivery

A webhook system that cannot tolerate retries is not production-ready.

## What To Read Next

- [Configuration and secrets](../configuration-and-secrets/)
- [Observability, monitoring, and audit](../observability/)
- [Troubleshooting](../troubleshooting/)
