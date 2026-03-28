---
title: Jobs And Schedulers
---

Davenda treats background work as a first-class platform concern.

That includes:

- scheduled jobs
- retryable work
- domain-event-driven jobs
- queue inspection and recovery

## What Runs In The Job System

Typical uses include:

- email or notification delivery
- webhook fan-out
- report exports
- asset or content follow-up work
- integration refreshes
- operational backfills and recovery tasks

The point is not “do everything async.” The point is to make async work explicit, inspectable, and
recoverable.

## Operational Expectations

A production team should know:

- which queues exist
- which jobs are ready, in flight, failed, or dead-lettered
- how retries are bounded
- how scheduler leadership is coordinated
- how to re-run or promote work safely

If the queue is opaque, incidents will cascade.

## Scheduler Model

Schedulers should promote due work predictably and without duplicate execution under normal
conditions. In multi-node environments, leadership and coordination need to be explicit rather than
assumed.

From an operator perspective, that means:

- one node or coordinator should own promotion at a time
- queue lag should be visible
- promotion failures should be observable
- operator workflows should exist for inspect, promote, retry, and dead-letter handling

## Retry And Dead-Letter Policy

Retries are part of the product contract, not an implementation detail.

Teams should decide and document:

- which jobs are retry-safe
- how many attempts are allowed
- what counts as permanent failure
- when a job moves to dead letter
- who is responsible for retrying or abandoning dead-letter work

Blind infinite retry loops are not production behavior.

## Jobs In Release Operations

Before or during deployment, pay attention to:

- jobs that depend on schema changes
- jobs that depend on newly deployed code paths
- jobs that may replay work after cutover
- jobs that must be drained or paused before rollback

Release planning should include background work, not only HTTP traffic.

## Monitoring Guidance

Track at least:

- ready queue depth
- in-flight count
- retry count
- dead-letter count
- oldest pending job age
- scheduler leadership or promotion status

Those signals let operators distinguish “system healthy but busy” from “system stuck.”

## Safe Operator Commands

A serious jobs toolchain should support:

- status and queue inspection
- ready and in-flight views
- dead-letter inspection
- controlled retry
- controlled promotion
- bounded worker execution

Any destructive or replay-capable command should require clear confirmation and operator intent.

## Incident Questions To Answer Fast

When jobs misbehave, operators should be able to answer:

- is the queue progressing
- is a dependency failing
- did a deploy change job behavior
- are retries causing duplicate external side effects
- which jobs can be safely replayed

If those answers are not available quickly, jobs become an operational risk rather than a safety
tool.
