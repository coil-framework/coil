---
title: Cache, TLS, Cutover, And Rollback
---

Coil exposes cache, TLS, cutover, and rollback as operator-visible concerns because they are
common sources of production risk.

Treat them as explicit release surfaces, not hidden implementation details.

## What Is This?

This page explains how to operate four areas that often determine whether a release is merely
"working" or actually safe:

- cache topology and invalidation
- TLS mode and certificate lifecycle
- cutover preparation and execution
- rollback preparation and execution

## Cache Topology

Coil supports a two-layer cache model:

- `l1`: in-process cache such as Moka
- `l2`: shared cache such as Redis or Valkey

Concrete checked-in example:

```toml
[cache]
l1 = "moka"
l2 = "redis"
```

This configuration is used by both Shoppr and Gitly.

### When To Use Only `l1`

Use only `l1` when:

- you are running a single node
- you want the simplest setup
- shared invalidation is not yet required

### When To Use `l1` And `l2`

Use both when:

- you have more than one runtime node
- you need shared invalidation behaviour
- cache correctness has to survive restarts or horizontal scaling better

### Operator Commands

Representative cache commands:

```bash
coil cache warm --config apps/shoppr/platform.toml --scope public --route /en-GB/shop
coil cache inspect --config apps/shoppr/platform.toml --route /en-GB/shop
coil cache invalidate --config apps/shoppr/platform.toml --tag route:events.list --tag locale:en-GB --yes
```

Use them to make caching legible, not magical.

## TLS Operations

TLS is part of the product surface, not just an infrastructure checkbox.

Shoppr production config demonstrates a real automated TLS lane:

```toml
[tls]
mode = "acme"
challenge = "dns-01"
provider = "cloudflare-dns"
```

Development configs for Shoppr and Gitly use:

```toml
[tls]
mode = "external"
```

That split is healthy:

- local stacks use externally terminated or dev-safe HTTP behaviour
- production expresses the real certificate lifecycle

### Operator Commands

Representative TLS commands:

```bash
coil tls status --config apps/shoppr/platform.toml
coil tls validate-challenge --config apps/shoppr/platform.toml
coil tls renew --config apps/shoppr/platform.toml --certificate cert-live --replacement cert-next --dry-run
```

If operators cannot tell whether challenge setup and renewal are healthy, the deployment is not
production-ready.

## Cutover Planning

Cutover is when the new release starts receiving real traffic.

Minimum cutover checklist:

1. config validated
2. migrations reviewed and applied as intended
3. assets published
4. `/ready` is healthy on the target release
5. critical product journeys are verified
6. rollback target is known
7. cache invalidation or warming plan is ready

## Rollback Planning

Rollback should be prepared before the switch, not improvised after the incident.

Operators should know:

- which binary and config are the previous known-good target
- whether migrations are backward-safe
- whether queues need draining or pausing
- whether caches need rewarming or invalidation
- which asset release should become active again

## A Practical Coil Cutover Sequence

1. Validate the target release.
2. Apply approved migrations.
3. Publish assets.
4. Start the new runtime beside the old one if your topology allows.
5. Check `/health` and `/ready`.
6. Verify critical journeys such as storefront, admin, and webhook paths.
7. Switch traffic.
8. Watch health, logs, queues, and audit surfaces closely.

## Production Checklist

Before you call a cutover safe, confirm:

- canonical hosts resolve correctly
- localised routes resolve correctly
- assets are serving from the intended origin or CDN
- payments and webhooks are still flowing
- admin and account surfaces still work
- caches are not leaking stale or cross-site state

## Common Mistakes

### Treating TLS as "set and forget"

Certificate lifecycle and challenge validation need operator visibility.

### Switching traffic before assets are confirmed

A healthy backend plus stale frontend assets is still a broken release.

### Forgetting cache state during rollback

Rollback can fail if the old runtime meets the new cache or asset state unexpectedly.

### Skipping post-cutover verification

The switch itself is not the end of the release. Clean post-switch verification is.

## What To Read Next

- [Asset publication and CDN delivery](../asset-publication-and-cdn-delivery/)
- [Health, readiness, and maintenance mode](../health-readiness-and-maintenance-mode/)
- [Troubleshooting](../troubleshooting/)
