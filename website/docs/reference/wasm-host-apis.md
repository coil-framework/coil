---
title: WASM Host APIs
---

Davenda’s WASM runtime is intentionally narrow. Packages do not get arbitrary process access.

Start with the key idea:

```toml
[[handlers]]
id = "community-pulse"
export = "exports.community_pulse"
point = "api"
target = "/api/github/pulse"
grants = []
```

Even after a package declares a handler like that, it still does not get unrestricted host access.

Everything remains bounded by:

- the extension point
- the granted host capabilities
- runtime config such as network policy

Use this page when you want to answer:

- what a WASM package can actually ask the host to do
- how grants gate those calls
- what the runtime already supports today
- where the hardened host backends live

## What The Host API Surface Is

The simplest mental model is:

1. package requests a host action
2. installation grants or denies that action
3. runtime host implementation enforces the decision

That split matters:

- `davenda-wasm` defines the contract and grant vocabulary
- `davenda-runtime` provides the concrete host implementation

## Current Host Capability Families

The main grant families are defined by `HostCapabilityGrant` in
`crates/davenda-wasm/src/grants.rs`.

Current families include:

- repository-style data read and write
- auth inspection and tuple writes
- storage read and write
- render fragment access
- metadata writes
- cache hints
- outbound HTTP by named integration
- secret reads
- job enqueue

If a package does not have the grant, the host should fail closed.

That is the main point of this model: a WASM package is not a second backend. It is a guest asking the host for a bounded operation.

## Outbound HTTP

The most important security boundary is outbound HTTP.

The hardened backend lives in:

- `crates/davenda-runtime/src/wasm/host/services/http/backend.rs`

Important behaviours already implemented there:

- network can be disabled entirely through runtime config
- extensions name an integration, not an arbitrary raw destination
- the backend resolves integrations through an approved target map
- `request.url` must match the approved endpoint
- response size is capped
- reserved headers such as `Host` and `Content-Length` are blocked

That is the same posture Davenda now uses for linked customer outbound HTTP too: approved
integration targets, not unrestricted guest-controlled egress.

Minimal example:

```text
allowed integration: github_api -> https://api.github.com
requested URL:       https://api.github.com/repos/acme/project
result:              allowed
```

```text
allowed integration: github_api -> https://api.github.com
requested URL:       https://evil.example.com/steal
result:              denied
```

## Metadata And Durable Shared State

The runtime metadata backends live under:

- `crates/davenda-runtime/src/wasm/host/services/metadata/local.rs`
- `crates/davenda-runtime/src/wasm/host/services/metadata/shared.rs`

These files are worth reading because they show two important host behaviours:

- local single-node metadata and audit persistence
- shared Postgres-backed metadata and audit persistence

The shared backend now also stores durable customer managed-asset records, which is the concrete
example of a WASM/host-adjacent API becoming production-grade instead of request-local state.

In practice, this means an extension can contribute durable metadata or managed assets without owning the storage backend itself.

## Storage And Managed Assets

Asset publication and delivery planning are not implemented inside the WASM package itself.

Relevant files:

- `crates/davenda-runtime/src/storage/host.rs`
- `crates/davenda-assets/src/delivery.rs`
- `crates/davenda-assets/src/release.rs`

This means:

- the guest requests a bounded asset operation
- the runtime plans storage according to configured policy
- public delivery remains tied to the configured asset delivery model

That keeps storage policy enforceable at the platform layer instead of inside extension code.

## Jobs

WASM packages can target scheduled jobs and other background work, but only through explicit host
contracts and installed handlers.

Concrete package example:

- `apps/gitly/extensions/gitly-actions-scheduler/package.toml`

Concrete app loader:

- `apps/gitly/crates/gitly-app/src/extensions.rs`

The important boundary is that packages do not start their own scheduler. They plug into a host
job system the customer app already composed.

## Render Hooks

Shoppr’s waitlist package is the clearest render-hook example:

- `apps/shoppr/extensions/shoppr-waitlist-tools/package.toml`
- `apps/shoppr/crates/shoppr-app/src/extensions.rs`

This is useful because it shows the smallest possible bounded extension:

- one handler
- one render hook target
- no extra grants

Use this pattern when you want "small injected behaviour", not "customer-owned product policy".

## Runtime Configuration That Affects Host APIs

The demo apps expose the main knobs in `platform.dev.toml`:

- `apps/shoppr/platform.dev.toml`
- `apps/gitly/platform.dev.toml`

Important settings:

- `[wasm].directory`
- `[wasm].default_time_limit_ms`
- `[wasm].allow_network`

If `allow_network = false`, outbound HTTP should fail closed even if a package is installed.

That means runtime config remains authoritative over package intent.

## What WASM Is Good For In Practice

The checked-in demos use WASM for:

- Shoppr: a bounded render-hook waitlist banner
- Gitly: a bounded API payload contributor
- Gitly: a bounded scheduled-job handler

Those are good examples because they keep the extension model honest:

- runtime-installed
- hash-pinned
- limited grants
- no special-casing in the app templates

## What A Good WASM Package Feels Like

A good WASM package is:

- small
- explicit
- easy to pin and review
- easy to revoke
- narrow in grants

If a package needs broad host powers or deep product context, it probably belongs in linked customer Rust instead.

## Common Mistakes

- Do not use WASM as a substitute for first-party customer Rust.
- Do not assume a package can reach the network without an approved integration mapping.
- Do not request broad grants when the package only needs one narrow action.
- Do not treat the host API as stable if you have not pinned `host_api_version`.

## Full Implementation

Contract and grant model:

- `crates/davenda-wasm/src/manifest/manifests.rs`
- `crates/davenda-wasm/src/grants.rs`

Runtime host implementations:

- `crates/davenda-runtime/src/wasm/host/services/`

Concrete package examples:

- `apps/shoppr/extensions/shoppr-waitlist-tools/package.toml`
- `apps/gitly/extensions/gitly-community-pulse/package.toml`
- `apps/gitly/extensions/gitly-actions-scheduler/package.toml`

Concrete app loaders:

- `apps/shoppr/crates/shoppr-app/src/extensions.rs`
- `apps/gitly/crates/gitly-app/src/extensions.rs`

Runtime settings examples:

- `apps/shoppr/platform.dev.toml`
- `apps/gitly/platform.dev.toml`

## Read Next

- [Extension Package Format](./extension-package-format.md)
- [Linked Rust Hook APIs](./linked-rust-hook-apis.md)
- [Gitly Extensions And Host APIs](../use-cases/gitly/extensions-and-host-apis.md)
