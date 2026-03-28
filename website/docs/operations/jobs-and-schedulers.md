---
title: Jobs And Schedulers
---

Davenda treats background work as a first-class platform concern.

That includes:

- scheduled jobs
- retryable work
- domain-event-driven work
- queue inspection and recovery

## What Is This?

This page explains:

- what kinds of work belong in Davenda jobs
- how the checked-in apps demonstrate jobs and schedulers
- which operator commands and signals matter

## Why Does Davenda Care About Jobs So Much?

Because async work is where many products become operationally unsafe:

- side effects are retried blindly
- queues are opaque
- release rollouts ignore background work entirely
- dead letters accumulate without ownership

Davenda is trying to make jobs inspectable and recoverable instead.

## What Runs In The Job System

Typical uses include:

- webhook fan-out
- follow-up integration work
- scheduled refreshes
- exports and reports
- asset housekeeping
- retryable operational tasks

The goal is not "make everything async." The goal is to move the right work off the request path
without making it invisible.

## Concrete Repo Examples

### Gitly scheduled work

Gitly is the clearest checked-in public jobs example.

Relevant files:

- `apps/gitly/extensions/gitly-actions-scheduler/`
- `apps/gitly/app.toml`
- `apps/gitly/platform.dev.toml`

Gitly uses a bounded runtime-installed extension to simulate GitHub Actions refresh work. That
gives you a real scheduled-work example without turning the whole repo into a queue tutorial.

### Runtime config

Both checked-in apps currently use Redis-backed jobs:

```toml
[jobs]
backend = "redis"
```

Concrete files:

- `apps/shoppr/platform.dev.toml`
- `apps/shoppr/platform.toml`
- `apps/gitly/platform.dev.toml`
- `apps/gitly/platform.toml`

## How To Operate Jobs

The operator surface is the platform jobs command group.

Representative commands:

```bash
platform jobs status --config apps/shoppr/platform.toml
platform jobs run --config apps/shoppr/platform.toml --worker-id worker-a --limit 25
platform jobs ready --config apps/shoppr/platform.toml --queue jobs.work --limit 25
platform jobs dead-letters --config apps/shoppr/platform.toml --queue jobs.dead-letter --limit 25
platform jobs in-flight --config apps/shoppr/platform.toml --queue jobs.work --worker-id worker-a --limit 25
platform jobs retry dead-letter:job-retry --config apps/shoppr/platform.toml --dry-run
platform jobs promote --config apps/shoppr/platform.toml --dry-run
```

That is the control-plane shape operators should build around:

- inspect
- run
- observe ready work
- inspect dead letters
- inspect in-flight work
- retry safely
- promote safely

## How To Think About Job Types

### Scheduled jobs

Use these when work should be promoted on a schedule.

Current public example:

- Gitly Actions refresh simulation

### Retryable jobs

Use these when a side effect can legitimately fail and should be retried under a bounded policy.

### Domain-event-driven jobs

Use these when the request path should emit a durable event and let follow-up work happen
asynchronously.

The public repo demonstrates the runtime and CLI surfaces for this lane, but the public website
docs are still thinner than ideal on a single polished end-to-end domain-event example.

## What Operators Need To Know

At minimum, operators should be able to answer:

- which queues exist
- what is ready right now
- what is in flight
- what is retrying
- what is dead-lettered
- which worker is currently making progress

If those answers require database spelunking, the jobs surface is not being used properly.

## Release Guidance

Before deploying code that changes job behavior, verify:

- whether pending jobs were created by older code
- whether new jobs depend on new schema
- whether a rollback would replay external side effects
- whether workers should be paused, drained, or restarted during cutover

Jobs are part of release planning, not an afterthought.

## Current Example Coverage And Limits

Strong public example coverage exists for:

- job backend config
- worker/operator command surface
- scheduled work through Gitly

Public example coverage is still thinner for:

- a polished linked-Rust customer job definition walkthrough
- a public end-to-end domain-event job tutorial page

So use Gitly as the current canonical scheduler example, and treat the general jobs contract as
stable even where the public examples are still catching up.

## Common Mistakes

### Treating dead letters as an archive

Dead letters are an operational signal that needs ownership and follow-up.

### Running workers without queue inspection

If operators cannot see ready, in-flight, and dead-letter state, they are operating blind.

### Forgetting job impact during rollout

Background work can replay, stall, or corrupt expectations during a bad deploy just as easily as
request-path logic can.

## What To Read Next

- [Observability, monitoring, and audit](observability.md)
- [Troubleshooting](troubleshooting.md)
- [Build and deploy](build-and-deploy.md)
