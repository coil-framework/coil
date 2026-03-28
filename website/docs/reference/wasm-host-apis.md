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
- how to build a real package without reverse-engineering core crates

## A Real End-To-End Host API Contract

Every WASM host interaction has four separate contracts:

1. the customer app exposes a slot
2. the package declares a handler for that slot
3. the package requests host grants
4. the customer app approves those grants during installation

That means the `.wasm` file alone is never enough.

The smallest real pair looks like this:

```toml
[[handlers]]
id = "community-pulse"
export = "exports.community_pulse"
point = "api"
target = "/api/github/pulse"
grants = []
```

and then:

```toml
[[extensions]]
id = "gitly-community-pulse"
package_version = "0.1.0"
artifact_sha256 = "..."
customer_app_id = "gitly"

[[extensions.handlers]]
id = "community-pulse"
grants = []
```

The second block is the approval boundary. It is where the customer app decides that the package is
actually installed and what grants it is allowed to keep.

## Start With A Full End-To-End Example

Gitly’s API extension is a complete real example:

```toml title="apps/gitly/extensions/gitly-community-pulse/package.toml"
publisher = "gitly-demo"
artifact = "artifacts/gitly-community-pulse.wasm"
artifact_sha256 = "ef2b0bc15aa0baf178df23d3671bf0a2914c618e394f985441e27a5fdd7c89d7"

[manifest]
id = "gitly-community-pulse"
display_name = "Gitly Community Pulse"
version = "0.1.0"
host_api_version = "1.0.0"

[[handlers]]
id = "community-pulse"
export = "exports.community_pulse"
point = "api"
target = "/api/github/pulse"
grants = []
```

And the customer app installs it like this:

```toml title="apps/gitly/app.toml"
[[extensions]]
id = "gitly-community-pulse"
package_version = "0.1.0"
artifact_sha256 = "ef2b0bc15aa0baf178df23d3671bf0a2914c618e394f985441e27a5fdd7c89d7"
customer_app_id = "gitly"

[[extensions.handlers]]
id = "community-pulse"
grants = []
```

That pair is the real contract.

## What The Host API Surface Is

The simplest mental model is:

1. package requests a host action
2. installation grants or denies that action
3. runtime host implementation enforces the decision

For a third-party extension author, the important operational split is:

- your package declares handlers and requested grants
- the customer app installs those handlers and approves grants
- the host enforces the runtime boundary

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

## Grant Families In Practice

The easiest way to understand the host API is to map each grant family to a real use case.

### `DataRead { resource = ... }`

Use this when the package needs read-only access to a named host-owned repository surface.

Typical use:

- read a repository summary
- read a CMS page record
- read a catalog item for a small UI fragment

### `DataWrite { resource = ... }`

Use this when the package needs to update a bounded repository-backed record through the host.

Typical use:

- write a computed status record
- store a small host-owned extension result

Do not treat this as arbitrary database ownership.

### `AuthCheck`, `AuthList`, `AuthLookup`, `AuthTupleWrite`

These let the package ask the host auth model questions.

Typical use:

- `AuthCheck`
  - can the current actor perform this action?
- `AuthLookup`
  - which tuples currently apply?
- `AuthTupleWrite`
  - add or remove a bounded auth relationship through the host

### `StorageRead` and `StorageWrite`

These are scoped by storage class, not raw filesystem access.

The important classes today are:

- `public_upload`
- `private_shared`
- `local_only_sensitive`
- `public_asset`

Use them when the package needs the host to inspect or publish a bounded file.

### `RenderFragment { slot = ... }`

This is the classic render-hook grant.

Use it for:

- banners
- badges
- sidebar panels
- contextual HTML fragments

### `MetadataWrite { kind = ... }`

This lets a package ask the host to store bounded metadata such as:

- `json_ld`
- `sitemap_entry`
- `translation`
- `seo_head`

Use it when a package contributes SEO or translation-adjacent metadata, not when it is trying to
own a whole persistence model.

### `CacheHintWrite`

This lets a package hint to the host how a response should be cached.

It is a hint, not raw cache ownership.

### `OutboundHttp { integration = ... }`

Use this when a package needs to call one approved named integration.

Example request:

```toml
grants = ["http.outbound:github_api"]
```

That does not mean “open the network”. It means “the host may allow calls through the configured
`github_api` integration”.

### `SecretRead { secret = ... }`

Use this when the package needs one specific runtime-bound secret.

Example request:

```toml
grants = ["secret.read:github_webhook_token"]
```

### `EnqueueJob { queue = ... }`

Use this when the package needs to ask the host job system to enqueue follow-up work.

Example request:

```toml
grants = ["job.enqueue:default"]
```

That gives the package bounded job submission. It does not give it scheduler ownership.

## What You Can Build Today

The checked-in demos already prove three concrete package shapes:

- a render hook
  - Shoppr waitlist banner
- an API handler
  - Gitly community pulse
- a scheduled job
  - Gitly actions refresh

If your package does not fit one of those patterns yet, you need to check whether the customer app
has exposed the right slot and whether the host surface exists.

## Outbound HTTP

The most important security boundary is outbound HTTP.

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

The metadata host surface gives packages a way to persist bounded runtime-owned state.

In practice, that means:

- local single-node metadata and audit persistence
- shared Postgres-backed metadata and audit persistence

The shared backend now also stores durable customer managed-asset records, which is the concrete
example of a WASM/host-adjacent API becoming production-grade instead of request-local state.

In practice, this means an extension can contribute durable metadata or managed assets without
owning the storage backend itself.

## Storage And Managed Assets

Asset publication and delivery planning are not implemented inside the WASM package itself.

This means:

- the guest requests a bounded asset operation
- the runtime plans storage according to configured policy
- public delivery remains tied to the configured asset delivery model

That keeps storage policy enforceable at the platform layer instead of inside extension code.

## Jobs

WASM packages can target scheduled jobs and other background work, but only through explicit host
contracts and installed handlers.

The important boundary is that packages do not start their own scheduler. They plug into a host
job system the customer app already composed.

## Lifecycle Expectations

Write packages as if each invocation is isolated and host-governed.

That means:

- do not depend on process-global mutable state
- do not assume one warm singleton stays alive forever
- do not treat the package as if it owns the whole request lifecycle

The stable contract is handler invocation plus grants, not a particular in-memory hosting detail.

## Render Hooks

Shoppr’s waitlist package is the clearest render-hook example:

```toml title="apps/shoppr/extensions/shoppr-waitlist-tools/package.toml"
[[handlers]]
id = "home.waitlist.banner"
export = "exports.home_waitlist_banner"
point = "render-hook"
target = "cms.page.render"
grants = []
```

This is useful because it shows the smallest possible bounded extension:

- one handler
- one render hook target
- no extra grants

Use this pattern when you want "small injected behaviour", not "customer-owned product policy".

## Runtime Configuration That Affects Host APIs

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

If you want to study a full working implementation after reading this page, start with:

- `apps/shoppr/extensions/shoppr-waitlist-tools/package.toml`
- `apps/gitly/extensions/gitly-community-pulse/package.toml`
- `apps/gitly/extensions/gitly-actions-scheduler/package.toml`
- `apps/shoppr/crates/shoppr-app/src/extensions.rs`
- `apps/gitly/crates/gitly-app/src/extensions.rs`

## Read Next

- [Extension Package Format](./extension-package-format.md)
- [Writing And Installing WASM Extensions](./wasm-writing-and-installing-extensions.md)
- [WASM Host Service Examples](./wasm-host-service-examples.md)
- [Linked Rust Hook APIs](./linked-rust-hook-apis.md)
- [Gitly Extensions And Host APIs](../use-cases/gitly/extensions-and-host-apis.md)
