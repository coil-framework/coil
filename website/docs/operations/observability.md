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

## The Four Operational Signals

For production, assume you need all four:

- structured logs
- metrics
- traces
- audit records or operator evidence

Each answers a different question.

### Logs

Use logs to answer:

- what happened
- which request, job, or operation failed
- which dependency or provider returned the error
- whether the runtime failed closed or continued in a degraded mode

Logs should be structured and machine-searchable.

### Metrics

Use metrics to answer:

- is the service healthy right now
- are error rates increasing
- are caches effective
- are jobs backing up
- is latency or resource pressure changing over time

Metrics support alerting and capacity planning.

### Traces

Use traces to answer:

- where latency is actually being spent
- which hop or integration made a request slow
- how a request interacted with storage, auth, jobs, or provider calls

Traces are especially useful for multi-step user journeys and operational flows.

### Audit And Operator Evidence

Use audit evidence to answer:

- who changed or approved a release
- who executed a migration, publish, or cutover command
- what rollback or recovery action was performed
- what state the platform observed before and after a sensitive operation

This is not the same thing as general application logging.

## Minimum Production Dashboard Areas

At minimum, teams should track:

- request rate, latency, and error rate
- background job queue depth, retries, and dead letters
- cache hit rate and invalidation activity
- database connectivity and latency
- object-store errors and latency
- payment webhook failures or replay rejections
- TLS renewal and certificate health
- migration and cutover execution outcomes

If those are invisible, production operation becomes guesswork.

## Logging Guidance

Good operational logging in Davenda should include:

- request identifiers
- job identifiers
- correlation or causation identifiers for event-driven work
- site and locale context where relevant
- operator command names for CLI-initiated changes
- dependency/provider names when external services fail

Do not rely on free-form strings alone.

## Monitoring Guidance

Alert on symptoms that matter to customers and operators:

- sustained request failures
- elevated checkout or webhook failure rates
- scheduler leadership or promotion failures
- dead-letter growth
- asset publication failures
- TLS renewal failures
- storage or cache backend unavailability

Avoid noisy alerts that do not map to action.

## Audit Scope

Audit evidence should exist for:

- release and deployment approvals
- migration application
- asset publication
- module enable/disable/install operations
- cutover apply and rollback
- privileged administrative workflows

If these actions cannot be reconstructed after the fact, operational trust will suffer.

## Cutover And Incident Monitoring

During a migration or cutover, broaden monitoring temporarily:

- watch critical customer journeys explicitly
- inspect cache behavior and leakage
- verify webhook and callback behavior
- verify canonical hosts and media delivery
- confirm rollback triggers remain observable

A calm steady-state dashboard is not enough during live traffic transitions.

## Troubleshooting Posture

Observability should support safe debugging without requiring production shell access as the first
response.

Teams should be able to answer common questions through:

- logs
- traces
- metrics
- operator commands
- deployment records

If the first step in every incident is “ssh into a box and guess,” the observability model is not
finished.
