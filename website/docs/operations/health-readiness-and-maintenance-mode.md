---
title: Health, Readiness, And Maintenance Mode
---

Coil separates health, readiness, and maintenance because they answer different operator
questions.

## What Is This?

This page explains the runtime's service-state controls:

- `/health`
- `/ready`
- maintenance mode
- operator bypass behaviour

## Why Does This Matter?

Without this separation, teams end up using one endpoint for everything:

- "is the process alive?"
- "is it safe to send traffic?"
- "is the deployment intentionally blocking customer traffic?"

Those are not the same question.

## The Canonical Model

### `/health`

Use `/health` to answer:

- is the runtime alive
- what does it think its current dependency and maintenance state is

### `/ready`

Use `/ready` to answer:

- is this instance ready to receive traffic right now

### maintenance mode

Use maintenance mode to answer:

- should some or all traffic be deliberately blocked during deployment or incident handling

## How Operators Should Use Them

### During startup

Wait for `/ready`, not just `/health`, before putting a node behind live traffic.

### During deployment

Check both:

- `/health` for broad service state
- `/ready` for cutover safety

### During incident containment

Use maintenance mode when you need to stop customer-facing churn while keeping operator control and
visibility.

## Maintenance Mode In Practice

The practical runtime contract is:

- health output should expose maintenance state
- affected requests should fail in a predictable way
- operator bypass should be explicit
- maintenance should be deliberate, not accidental

The important operator rule is to use maintenance as a controlled state, not as a substitute for
readiness or deploy discipline.

## A Practical Rollout Checklist

Before cutover:

- `/health` is sane
- `/ready` is passing
- maintenance is not unexpectedly enabled

If maintenance is intentionally enabled:

- confirm which traffic is blocked
- confirm bypass behaviour is limited to the right operators

After maintenance:

- verify `/health`
- verify `/ready`
- re-run critical journeys

## Supporting Repo Examples

The checked-in apps already prove these surfaces operationally:

- main app containers use `/ready` in health checks
- the optional Shoppr sidecar exposes `/health`
- the runtime implements maintenance and bypass semantics

Those examples support this page, but the operator model above is what matters first.

## Common Mistakes

### Using `/health` as the load-balancer gate

That is what readiness is for.

### Enabling maintenance without observability

If you cannot see maintenance state from the service, it becomes guesswork.

### Treating bypass as a convenience header

Bypass should be narrow, explicit, and operationally controlled.

## What To Read Next

- [Observability, monitoring, and audit](observability.md)
- [Cache, TLS, cutover, and rollback](cache-tls-cutover-and-rollback.md)
- [Troubleshooting](troubleshooting.md)
