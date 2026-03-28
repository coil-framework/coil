---
title: Observability, Monitoring, And Audit
---

Davenda treats observability as part of the runtime contract, not as optional glue added after the
product already works.

That matters because teams need to operate:

- storefront requests
- account and admin surfaces
- jobs and schedulers
- imports and migrations
- cutover and rollback flows
- auth, payments, storage, and extension boundaries

## What Is This?

This page covers the built-in operational signals Davenda exposes today and how the checked-in
apps use them:

- logs
- metrics
- traces
- health and readiness probes
- audit evidence

## Why Does Observability Matter Here?

Davenda is intentionally opinionated about runtime composition and operations. That only works if
operators can see what the runtime is doing without resorting to shell access as the first step in
every incident.

## The Four Main Signal Types

### Logs

Use logs to answer:

- what happened
- which request or job failed
- which dependency failed
- whether the runtime failed closed or degraded

### Metrics

Use metrics to answer:

- whether the system is healthy right now
- whether latency or error rates are drifting
- whether queues or dependencies are backing up

### Traces

Use traces to answer:

- where latency is really being spent
- which dependency or runtime phase is slow
- how a request or workflow crossed subsystem boundaries

### Audit evidence

Use audit evidence to answer:

- who performed a privileged action
- what administrative workflow was executed
- what changed during release or recovery operations

Audit is not a replacement for logs. It is the durable operator-history lane.

## Concrete Davenda Surfaces

### `/health` and `/ready`

Davenda exposes health and readiness probes as first-class runtime endpoints.

The checked-in Docker stacks already use them:

- Shoppr app healthcheck hits `/ready`
- Gitly app healthcheck hits `/ready`
- Shoppr sidecar backend exposes `/health`

That makes health an operator-visible contract, not an undocumented implementation detail.

### Metrics and tracing switches

Both checked-in apps currently enable observability in platform config:

- `apps/shoppr/platform.dev.toml`
- `apps/shoppr/platform.toml`
- `apps/gitly/platform.dev.toml`
- `apps/gitly/platform.toml`

Each uses:

```toml
[observability]
metrics = true
tracing = true
```

### Audit in Shoppr

Shoppr is the strongest checked-in audit example.

Concrete operator surface:

- `/admin/audit`

That page is meant to prove that administrative and privileged actions can be inspected as durable
operator history rather than inferred from general request logs.

### Customer-owned audit hooks

Shoppr's linked backend also uses the audit facade in customer-owned Rust. That matters because it
shows the supported path for customer-specific operator evidence without exposing unstable runtime
internals.

## Suggested Operator Dashboard Areas

At minimum, build dashboards for:

- request rate, error rate, and latency
- queue depth, retries, and dead letters
- database and cache health
- object-store errors and latency
- webhook failure rate
- readiness and health status
- audit volume for privileged workflows

If these areas are invisible, the runtime may still work, but the operational model is incomplete.

## How To Use These Surfaces In Practice

### For local development

Use Docker Compose health output and app logs first:

```bash
docker compose logs app
docker compose ps
```

Check:

- `/ready` for the main app
- `/health` for sidecars or integration adapters where present

### For deployed environments

Expose and monitor:

- `/health`
- `/ready`
- structured logs
- metrics collection
- trace export
- audit UI or audit-store access

## Current Example Coverage And Limits

The public repo already gives you strong examples for:

- readiness and health endpoints
- audit evidence in Shoppr
- linked-backend audit recording in Shoppr
- observability config toggles in both apps

The public examples are still thinner for:

- custom application metrics
- custom tracing spans documented end to end
- custom audit dashboards outside the Shoppr admin surface

So this page can document the current operator surfaces honestly, but it should not pretend the
repo already contains a complete public cookbook for every custom observability lane.

## Common Mistakes

### Treating `/ready` as a nicety

Readiness is part of deployment control and rollback safety, not just a convenience ping.

### Relying only on logs

Logs alone will not answer queue health, latency trends, or operator-history questions.

### Forgetting audit for privileged actions

If refunds, publishes, redirects, or cutover actions are not reconstructible, operational trust
degrades quickly.

## What To Read Next

- [Health, readiness, and maintenance mode](health-readiness-and-maintenance-mode.md)
- [Jobs and schedulers](jobs-and-schedulers.md)
- [Troubleshooting](troubleshooting.md)
