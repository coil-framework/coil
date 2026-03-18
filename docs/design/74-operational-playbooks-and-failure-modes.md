# Operational Playbooks and Failure Modes

**Part:** Operations  
**Chapter:** 74

The platform needs explicit runbooks because several core services are shared across all customer apps: auth, cache, storage, TLS, queueing, and extension hosting. When one of those services degrades, the failure rarely stays local unless operators know which controls to use and what can be rolled back safely.

## Certificate Renewal Failures

If renewal fails, keep serving the existing valid certificate, raise an alert immediately, and inspect:

- provider status and challenge type
- DNS automation credentials and recent token changes
- hostname ownership changes in the customer app
- proxy or Cloudflare configuration that may block validation

Do not delete or overwrite the working certificate before replacement succeeds. Roll back only the hostname or certificate-policy change that caused the renewal path to fail.

## Object Storage and Asset Failures

If object-store writes or reads fail:

1. Determine whether the incident affects published build assets, managed uploads, or both.
2. Pause nonessential sync jobs and media processing so the backlog is bounded.
3. Switch affected delivery paths to their safest degraded mode, such as app-proxy delivery for private files when possible.
4. Avoid deploying new asset manifests until storage consistency is restored.

For `local_only_sensitive` workloads, verify whether the affected customer app depends on node affinity or a shared private volume before moving traffic.

## Auth Regressions

When access control changes unexpectedly, the first tools should be capability-level diagnostics:

- run explain on the denied or over-granted action
- confirm which auth model package and capability bindings are active
- check whether a module upgrade introduced a new capability requirement
- inspect cache invalidation around tuple or model changes

Do not patch around auth regressions with feature flags or route-level exceptions. Fix the capability binding or model state directly.

## Cache Stampedes and Stale Content

When cache hit rate collapses or stale content persists:

- verify recent invalidation events
- inspect whether a write path is invalidating too broadly or not broadly enough
- check distributed cache health and lock behavior
- confirm that personalized responses are not being treated as public cache entries

If needed, disable the problematic cache layer selectively rather than clearing every cache in production without a plan.

## Webhook Backlog and Retry Storms

Webhook incidents should be handled by protecting the ingest boundary first. Verify signature failures, replay rejection rates, worker concurrency, and dead-letter growth. If one customer-specific extension or integration is failing repeatedly, isolate that consumer rather than degrading unrelated customer apps.

## Rollback Criteria

A rollout should be rolled back when:

- readiness degrades persistently after deploy
- auth decisions change outside the intended capability delta
- queue backlog grows faster than workers can drain it
- certificate or storage errors affect live traffic
- the change cannot be isolated to one customer app or one optional extension

The broader rule is that runbooks should preserve platform boundaries. Core failures, official module failures, customer-app misconfiguration, and single-extension defects must be diagnosable and reversible as separate classes of incident.
