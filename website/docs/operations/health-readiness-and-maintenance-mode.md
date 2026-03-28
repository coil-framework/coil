---
title: Health, Readiness, And Maintenance Mode
---

Davenda separates health, readiness, and maintenance because they answer different operator
questions.

## What Is This?

This page explains the runtime's operator-facing service-state controls:

- `/health`
- `/ready`
- maintenance mode
- operator bypass behavior

## Why Does This Matter?

Without this separation, teams end up using one endpoint for everything:

- "is the process alive?"
- "is it safe to send traffic?"
- "is the deployment intentionally in maintenance?"

Those are not the same question.

## Health Versus Readiness

### `/health`

Use `/health` to ask whether the runtime is alive and able to describe its current state.

### `/ready`

Use `/ready` to ask whether the runtime is actually ready for live traffic.

The checked-in Docker stacks already use `/ready` in healthchecks for the main app containers.

## Concrete Checked-In Examples

- `apps/shoppr/docker-compose.yml`
- `apps/gitly/docker-compose.yml`
- `apps/shoppr/backend/shoppr-loyalty-backend/src/http.rs` for the optional sidecar `/health`

## Maintenance Mode

Davenda has a real maintenance-mode concept in the runtime and observability model. It is not just
an edge proxy convention.

Current runtime behavior includes:

- maintenance state appearing in health output
- request blocking for affected traffic
- an operator bypass token/header path

That gives the platform a real maintenance contract even though the checked-in public apps do not
yet ship a polished branded maintenance-page walkthrough in the public docs tree.

## Practical Operator Guidance Today

Use maintenance mode when you need to reduce customer-facing churn during:

- schema or release transitions
- incident containment
- controlled rollback

Until the public app docs include a stronger end-to-end maintenance example, the practical safe
posture is:

- use readiness and cutover controls first
- enable maintenance intentionally when needed
- reserve bypass behavior for operators
- verify maintenance state through `/health`

## What To Check During A Rollout

Before cutover:

- `/health` reports sane dependency state
- `/ready` is passing
- maintenance is not unexpectedly enabled

During maintenance:

- operators confirm which traffic is blocked
- only approved bypass paths remain open

After maintenance:

- `/health` and `/ready` return to normal
- critical journeys are reverified

## Common Mistakes

### Using `/health` as the load-balancer gate

That is what readiness is for.

### Enabling maintenance without operator observability

If you cannot see maintenance state from the service, it becomes guesswork.

### Forgetting bypass discipline

Bypass should be explicit and limited to the right operators and workflows.

## What To Read Next

- [Observability, monitoring, and audit](observability.md)
- [Cache, TLS, cutover, and rollback](cache-tls-cutover-and-rollback.md)
- [Troubleshooting](troubleshooting.md)
