---
title: Jobs, Webhooks, And Background Work
---

Shoppr demonstrates the practical Davenda pattern for background work:

- the storefront stays HTML-first
- mutating workflows remain explicit
- long-running or integration-heavy follow-up work is pushed into jobs and verified webhooks

## Repo Areas To Read

Start here:

- `apps/shoppr/backend/shoppr-loyalty-backend/src/lib.rs`
- `apps/shoppr/backend/shoppr-loyalty-backend/src/http.rs`
- `apps/shoppr/crates/shoppr-bin/src/main.rs`
- `apps/shoppr/platform.toml`
- `apps/shoppr/platform.dev.toml`

## Verified Webhooks

Shoppr's linked Rust backend implements verified webhook hooks.

That gives it a supported way to:

- react to trusted external events
- call outbound services
- enqueue background jobs
- update repository-backed records
- record audit evidence

This is the practical answer to "how do I add integration logic without inventing a second
application?"

## Background Jobs

The public job surface comes through `JobsFacade`.

In Shoppr, that means customer logic can:

- accept a checkout or webhook path quickly
- enqueue follow-up work
- keep the browser-facing request bounded

Use that pattern for:

- CRM sync
- warehouse notifications
- fulfilment handoff
- waitlist or membership follow-up

## Why This Matters

Many platforms talk about jobs and webhooks as if they are separate subsystems. In Davenda they are
part of one operational story:

- the runtime verifies and normalises the trigger
- customer logic decides what should happen
- jobs absorb slow follow-up work
- audit records the decision trail

## How To Copy This Pattern

1. implement a linked Rust plugin
2. register verified webhook hooks
3. use `JobsFacade` for deferred work
4. use `AuditFacade` for evidence
5. use the jobs CLI to inspect queue state during development and operations

## Read Next

- [Jobs And Schedulers](../../operations/jobs-and-schedulers.md)
- [Webhooks And Integrations](../../operations/webhooks-and-integrations.md)
- [Observability And Audit](./observability-and-audit.md)
