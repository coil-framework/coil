---
title: Build And Deploy
---

Coil expects production teams to treat build, migration, asset publication, runtime startup,
and cutover as separate operational steps with explicit verification in between.

That separation is deliberate. A release is not just a compiled binary. It is the coordinated
bundle of:

- customer binary
- app manifest
- platform config
- auth package files
- templates and theme assets
- linked customer Rust code
- optional runtime-installed extensions

## What Is This?

This page describes the operational release flow for a Coil customer app:

- what to build
- which commands to run
- when to migrate
- when to publish assets
- how to start the runtime
- how to think about production rollout

## Why Does Coil Separate These Steps?

Because they fail differently.

- build failures are code or dependency problems
- migration failures are data-plane problems
- asset publication failures are delivery problems
- startup failures are config, dependency, or composition problems
- cutover failures are live traffic problems

If you collapse all of those into one "deploy" button, incident recovery gets much harder.

## A Practical Coil Release Sequence

For a serious deployment, use the same high-level order everywhere:

1. validate the app and config
2. build the customer binary
3. run tests and smoke checks
4. plan migrations
5. apply migrations deliberately
6. publish assets deliberately
7. start the new runtime
8. verify health and critical journeys
9. cut traffic over
10. keep rollback inputs available until the release is proven stable

## Concrete Local And CI Commands

Shoppr exposes the relevant lifecycle from the customer binary:

```bash
cd apps/shoppr
./scripts/prepare-local-dev.sh
cargo run -p shoppr -- describe
cargo run -p shoppr -- validate
cargo run -p shoppr -- migrate apply --dry-run
cargo run -p shoppr -- assets publish
cargo run -p shoppr -- up --config platform.dev.toml
```

Gitly exposes the same shape:

```bash
cd apps/gitly
./scripts/prepare-local-dev.sh
cargo run -p gitly -- validate
cargo run -p gitly -- migrate apply --dry-run
cargo run -p gitly -- up
```

Those commands are not throwaway examples. They are the checked-in operator surface for the demo
apps.

## Generic Platform Commands

At the platform level, the CLI contract is the same even if the binary name differs:

```bash
coil migrate plan --config apps/shoppr/platform.toml
coil migrate apply --config apps/shoppr/platform.toml --dry-run
coil assets publish --config apps/shoppr/platform.toml --dry-run
coil jobs status --config apps/shoppr/platform.toml
coil tls status --config apps/shoppr/platform.toml
```

Use the customer binary when the app re-exports that control plane. Use `coil` when you are
operating the generic CLI directly.

## What To Build

A production release should version and promote at least:

- the customer binary
- the exact `app.toml`
- the exact environment-specific `platform.toml`
- the auth package files
- templates and theme assets
- published asset manifest output when asset publication is enabled

Do not treat the binary alone as "the release."

## Migration Execution

Migration execution is a separate step because it changes live state.

Minimum safe pattern:

1. run a dry plan
2. review executable and manual migration entries
3. apply when approved
4. record what happened

Shoppr and Gitly both already expose migration reporting from the customer binary. Their validate
and migrate output includes the count of migration contracts and any manual customer migration
entries.

Important current limitation:

- the repo currently proves the migration reporting surface
- but it does not yet ship a polished public example of a custom customer-owned SQL table
  migration beyond the built-in reporting path

So document and operate customer-specific schema changes carefully, but do not assume there is a
fully finished example app walkthrough for that lane yet.

For more detail, read [Database migrations](../database-migrations/).

## Asset Publication

Asset publication is a distinct release step because a healthy binary can still serve the wrong
frontend assets if the manifest or CDN state is stale.

Concrete examples:

- Shoppr local dev publishes assets through `cargo run -p shoppr -- assets publish`
- Shoppr and Gitly both set `cdn_base_url` in `platform.dev.toml` and `platform.toml`

Read [Asset publication and CDN delivery](../asset-publication-and-cdn-delivery/) before treating
frontend delivery as part of startup.

## Production Deployment Example: Shoppr

A practical Shoppr deployment flow looks like this:

```bash
cd apps/shoppr
./scripts/prepare-local-dev.sh
cargo run -p shoppr -- validate
cargo run -p shoppr -- migrate apply --dry-run
cargo run -p shoppr -- assets publish
cargo run -p shoppr -- up --config platform.toml
```

During local development, the repo-maintainer override path is:

```bash
docker compose -f docker-compose.yml -f docker-compose.repo.yml up --build
```

That override is for working against the monorepo before upstream crates are published. It is not
the public deployment model.

## Production Deployment Example: Gitly

Gitly is the non-commerce example, but the operational contract is the same:

```bash
cd apps/gitly
./scripts/prepare-local-dev.sh
cargo run -p gitly -- validate
cargo run -p gitly -- migrate apply --dry-run
cargo run -p gitly -- up
```

Gitly is especially useful when you want to confirm that these operational rules are about the
platform rather than Shoppr-specific commerce behaviour.

## Same-Domain Versus CDN Asset Delivery

Coil supports both same-origin and CDN-style asset delivery, but the choice should be explicit.

Use same-domain delivery when:

- you want the simplest deployment shape
- you do not yet need CDN behaviour
- cache behaviour is easy to reason about without an extra edge layer

Use a CDN when:

- you want aggressive asset caching
- you need better geographic delivery
- you want runtime traffic and asset traffic to scale independently

Concrete examples:

- Shoppr dev: `cdn_base_url = "http://localhost:9000/shoppr"`
- Gitly dev: `cdn_base_url = "http://localhost:9002/gitly"`
- Shoppr prod: `cdn_base_url = "https://cdn.example.com"`

If the CDN path is used, asset publication must be treated as part of release promotion, not as a
background detail.

## Common Mistakes

### Treating `up` as the whole release flow

`up` is runtime startup. It is not a substitute for validation, migration review, and asset
publication.

### Applying migrations implicitly during live cutover

Migration execution should be deliberate and recorded.

### Publishing assets too late

If the new runtime starts before assets are in place, public pages can break even when the backend
is healthy.

### Forgetting customer-specific files in the release record

If you cannot answer which manifest, config, and auth package were deployed, rollback confidence
will be poor.

## What To Read Next

- [Database migrations](../database-migrations/)
- [Asset publication and CDN delivery](../asset-publication-and-cdn-delivery/)
- [Cache, TLS, cutover, and rollback](../cache-tls-cutover-and-rollback/)
