---
title: Build And Deploy
---

Davenda expects production teams to treat build, deploy, migrate, asset publication, and cutover as
separate operational steps with explicit verification between them.

That separation is deliberate. A customer app is not only a crate that compiles. It is a product
bundle made of:

- a customer binary
- a customer app manifest
- platform configuration
- auth package data
- templates and theme assets
- linked Rust backend code
- optional runtime-installed WASM packages

## Build Outputs

For a serious deployment, produce and version these artifacts together:

- the customer binary
- the exact app manifest used for the release
- the exact platform config used for the target environment
- the auth package and customer content/config committed with that release
- published asset metadata or release manifest when asset publication is enabled
- the release note or deployment record tying those inputs together

Do not treat the binary alone as the release.

## Recommended Promotion Flow

Use the same flow in CI, staging, and production so operational behavior stays consistent:

1. Resolve the customer workspace and configuration for the target environment.
2. Run config and composition validation.
3. Build the customer binary.
4. Run tests and any environment-specific smoke checks.
5. Plan and, when approved, apply executable migrations.
6. Publish theme and managed assets.
7. Start the new runtime alongside existing infrastructure or within the target rollout strategy.
8. Verify health, logs, metrics, jobs, and critical product journeys.
9. Execute cutover only after readiness gates pass.
10. Keep rollback inputs available until the new release is proven stable.

## CI Expectations

At minimum, CI should prove:

- the workspace compiles
- the documented customer apps still build
- critical tests pass
- configuration validation still succeeds for checked-in examples
- docs and reference material still match the current command and product shape

For production-oriented teams, CI should also emit:

- a versioned binary artifact
- a software bill of materials or equivalent dependency inventory
- a release record tied to the git revision and customer app revision

## Packaging Strategy

Davenda supports multiple packaging approaches, but they should all preserve the same operator
contract.

### Customer Workspace Binary

This is the primary model:

- the customer app owns the binary
- the binary links the selected official modules
- linked customer Rust logic is compiled into the same binary
- operators run customer-owned commands for validate, migrate, publish, and serve

This is the cleanest path for controlled deployments.

### Container Delivery

Containers are appropriate when teams want:

- a single immutable runtime image
- predictable runtime dependencies
- consistent local, staging, and production behavior

When using containers, keep the lifecycle explicit. A good deployment still validates config,
applies migrations deliberately, publishes assets deliberately, and records cutover state.

### Repo-Maintainer Overrides

Examples like `docker-compose.repo.yml` or local Cargo patching are maintainer conveniences for
developing against the monorepo before upstream publication. They are not the public deployment
model and should be documented as such.

## Environment Separation

Keep at least three distinct operational environments:

- local development
- pre-production or staging
- production

The goal is not identical infrastructure. The goal is behaviorally honest infrastructure:

- the same modules enabled
- the same auth package shape
- the same route and template tree
- the same jobs and webhook surfaces
- the same storage and asset publication model, as far as practical

Avoid “toy staging” that removes the exact components most likely to fail in production.

## Deployment Roles And Responsibilities

Separate the operational concerns clearly:

- product teams own the customer app, templates, linked Rust logic, and release intent
- platform operators own infrastructure, secrets distribution, runtime health, and cutover safety
- CI or release automation owns artifact assembly, validation, and evidence capture

That makes it possible to approve releases without hiding who changed what.

## Pre-Deploy Checks

Before any production rollout, require:

- config validation
- release doctor or compatibility checks
- migration plan review
- asset publication readiness
- provider credential validation for payments, TLS, object storage, or external integrations
- health and dependency readiness for database, cache, jobs, and object storage

If any of those fail, stop the rollout. Do not rely on the runtime to “figure it out later.”

## Runtime Startup Contract

A production startup should be predictable and boring:

- config loads successfully
- auth package loads successfully
- official modules compose successfully
- runtime-installed extensions resolve successfully
- linked customer plugins register successfully
- migrations are already in the expected state
- assets are already published or the release intentionally allows draft state

If the runtime cannot satisfy that contract, fail closed and keep the previous release in service.

## Artifact And Release Traceability

Every production deployment should answer:

- which git revision produced this binary
- which customer app manifest and config were used
- which migrations were pending, applied, or intentionally deferred
- which asset release was published
- which operator approved the rollout
- which cutover execution switched traffic

If those answers are not easy to retrieve, release confidence will degrade quickly.

## Production Readiness Checklist

Before you call a Davenda customer app production-ready, make sure the delivery path proves:

- reproducible build inputs
- environment-specific config validation
- explicit migration execution
- explicit asset publication
- observable startup and health signals
- rollback readiness
- documented operator workflows for common incidents

The framework is designed to support that model. Teams still need to operate it with discipline.
