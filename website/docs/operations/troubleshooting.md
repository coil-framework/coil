---
title: Troubleshooting
---

Troubleshooting in Coil should start with explicit evidence, not guesswork.

Use this page to classify a problem by subsystem first, then inspect the right operator surface.

## What Is This?

This page is the symptom-driven troubleshooting entry point for:

- startup failures
- readiness failures
- site and locale routing bugs
- asset and CDN problems
- session and CSRF problems
- jobs and queue failures
- migration and release mistakes
- webhook and extension failures

## Why Start Here?

Coil has explicit subsystems. That is an advantage only if operators and developers know which
one to inspect first.

The fastest first question is:

"Is this request-path, job-path, provider-path, or deployment-path?"

## Startup Failures

If the runtime will not start, check:

- app manifest and platform config alignment
- auth package loading
- required secrets
- database, cache, and storage configuration
- extension resolution
- linked customer plugin registration

Concrete inspection commands:

```bash
cargo run -p shoppr -- validate
cargo run -p gitly -- validate
```

Most startup failures should fail closed before traffic is accepted.

## Readiness Failures

If the service starts but is not healthy for traffic:

- query `/ready`
- query `/health`
- inspect container health output
- inspect dependency readiness such as Postgres, Redis, and object storage

Concrete checked-in healthchecks:

- `apps/shoppr/docker-compose.yml`
- `apps/gitly/docker-compose.yml`

## Site Or Locale Routing Problems

Symptoms:

- wrong site content under the right host
- locale prefixes resolving incorrectly
- canonical URL mismatches

Check:

- `app.toml` site definitions
- `platform.dev.toml` or `platform.toml` site blocks
- request host
- localised route expectations

Concrete files:

- `apps/shoppr/app.toml`
- `apps/shoppr/platform.dev.toml`
- `apps/gitly/app.toml`
- `apps/gitly/platform.dev.toml`

## Asset Problems

Symptoms:

- CSS or JS 404s
- stale frontend after deploy
- wrong asset origin

Check:

- whether `assets publish` ran
- the configured `cdn_base_url`
- whether the published asset manifest matches the release
- object-store or CDN health

Concrete commands:

```bash
cargo run -p shoppr -- assets publish
coil assets publish --config apps/shoppr/platform.toml --dry-run
```

## Session, Cookie, And CSRF Problems

Symptoms:

- browser loops between authenticated and unauthenticated state
- POSTs fail unexpectedly
- forms break only in one environment

Check:

- cookie `secure` and `same_site` settings
- whether TLS and proxy headers match the environment
- CSRF config in platform config
- session backend health

Concrete files:

- `apps/shoppr/platform.dev.toml`
- `apps/shoppr/platform.toml`
- `apps/gitly/platform.dev.toml`

## Jobs Problems

Symptoms:

- async work does not complete
- retries loop
- queues never drain

Check:

- ready queue depth
- in-flight jobs
- dead letters
- scheduler leadership or worker execution

Concrete commands:

```bash
coil jobs status --config apps/shoppr/platform.toml
coil jobs ready --config apps/shoppr/platform.toml --queue jobs.work --limit 25
coil jobs dead-letters --config apps/shoppr/platform.toml --queue jobs.dead-letter --limit 25
```

## Migration Problems

Symptoms:

- new release starts but behaves like old schema
- deploy blocks on startup
- data-plane behaviour differs between nodes

Check:

- whether migrations were planned
- whether they were actually applied
- whether manual customer migration entries were present
- whether the target binary and config match the migration run

Concrete commands:

```bash
cargo run -p shoppr -- migrate apply --dry-run
cargo run -p gitly -- migrate apply --dry-run
```

## Webhook Problems

Symptoms:

- payment callbacks do not settle
- provider retries repeat forever
- signed webhooks fail verification

Check:

- webhook secret configuration
- local forwarding tool or provider dashboard
- callback endpoint path
- request logs and health of downstream dependencies

Concrete Shoppr examples:

- payment callback: `/webhooks/commerce/payment-provider`
- sidecar CRM example: `POST http://localhost:8091/webhooks/crm/contact-updated`
- secrets in `apps/shoppr/.env.example`

## Extension Problems

Symptoms:

- an expected route or scheduled behaviour does not appear
- extension behaviour differs from linked Rust behaviour
- startup fails only when extensions are enabled

Check:

- extension directory
- manifest registration
- hash-pinned package entry in `app.toml`
- runtime limits and secret bindings in platform config

Concrete examples:

- `apps/shoppr/extensions/shoppr-waitlist-tools/`
- `apps/gitly/extensions/gitly-community-pulse/`
- `apps/gitly/extensions/gitly-actions-scheduler/`

## Release And Cutover Problems

Symptoms:

- new binary is live but old assets appear
- wrong hosts serve the new release
- rollback does not restore expected behaviour

Check:

- release inputs
- asset publication
- cache state
- `/ready` on the target
- traffic routing target

Do not debug a cutover issue until you verify what was actually deployed.

## Safe Response Rules

During troubleshooting:

- prefer inspection before mutation
- prefer reversible actions before destructive ones
- record what changed and why
- avoid bypassing the documented operator path unless the incident requires it

## What To Read Next

- [Health, readiness, and maintenance mode](../health-readiness-and-maintenance-mode/)
- [Webhooks and integrations](../webhooks-and-integrations/)
- [Database migrations](../database-migrations/)
