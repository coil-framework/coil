---
title: WASM Host Service Examples
---

This page is the practical companion to [WASM Host APIs](./wasm-host-apis/).

The goal is simple: show what the host surfaces mean in use, not just list them.

## Mental Model

Every host service follows the same pattern:

1. the package declares a handler and requests grants
2. the customer app installs that handler and approves grants
3. the host validates the request against runtime policy
4. the host performs the bounded action or fails closed

## API Handler Example

Gitly’s community pulse package is the simplest API example:

```toml title="apps/gitly/extensions/gitly-community-pulse/package.toml"
[[handlers]]
id = "community-pulse"
export = "exports.community_pulse"
point = "api"
target = "/api/github/pulse"
grants = []
```

What this means in practice:

- the handler may only run for `/api/github/pulse`
- it does not get database, network, secret, or job powers by default
- the host still decides how the response is mounted into the request lifecycle

This is a good model for small payload contributors or diagnostics endpoints.

## Render-Hook Example

Shoppr’s waitlist banner package is the clearest render-hook example:

```toml title="apps/shoppr/extensions/shoppr-waitlist-tools/package.toml"
[[handlers]]
id = "home.waitlist.banner"
export = "exports.home_waitlist_banner"
point = "render-hook"
target = "cms.page.render"
grants = []
```

What this means:

- the package can contribute to the named render slot
- it cannot bypass the page model
- it cannot silently fetch network data or mutate storage because no grants were approved

This is the right pattern for:

- banners
- badges
- extra panels
- small contextual HTML fragments

## Scheduled-Job Example

Gitly’s actions scheduler package demonstrates the scheduled-job point:

```toml title="apps/gitly/extensions/gitly-actions-scheduler/package.toml"
[[handlers]]
id = "nightly-refresh"
export = "exports.nightly_refresh"
point = "scheduled-job"
target = "github.actions.refresh"
grants = []
```

This means:

- the package does not create a scheduler
- the host scheduler already exists
- the package only contributes the work unit for the named job target

That is why Coil’s extension model stays operationally coherent. Jobs remain host-owned.

## Outbound HTTP Example

Outbound HTTP is the most sensitive host surface.

The safe pattern is:

```text
approved integration: github_api -> https://api.github.com
extension request:    https://api.github.com/repos/acme/project
result:               allowed
```

Unsafe:

```text
approved integration: github_api -> https://api.github.com
extension request:    https://evil.example.com/steal
result:               denied
```

Operationally, this means:

- the package should ask for a named integration, not a free-form destination
- runtime config can still disable network access entirely
- request headers and response size stay bounded by host policy

## Metadata Write Example

The host metadata layer is for durable, bounded state the package can attach through the host.

A practical example would be:

- a scheduled-job extension records its last successful run timestamp
- an API extension records a refresh watermark
- a render hook records a small audit breadcrumb for operator visibility

The important point is not the exact shape of the value. The important point is ownership:

- the package does not open its own database connection
- the host owns the persistence boundary
- the package only uses the bounded metadata contract

## Asset Example

Managed assets follow the same pattern:

- the package asks the host to handle a managed asset
- the host decides storage class, publication, and delivery plan
- public URLs still come from the configured delivery model

That means a package can contribute an asset without smuggling in its own storage backend.

## Job Enqueue Example

If a package has a job-enqueue grant, the safe mental model is:

- the package asks the host to enqueue a named job
- the host validates the queue and payload boundary
- the package does not get arbitrary queue ownership

Use this when a bounded extension needs follow-up work, not when it is trying to become a second job system.

## When To Stop And Use Linked Rust Instead

Move to linked customer Rust when the package needs:

- broad product context
- deep transactional logic
- richer typed access to runtime facades
- customer-owned release cadence rather than install/uninstall semantics

That is the line between bounded extension and first-party product code.

## Read Next

- [WASM Host APIs](./wasm-host-apis/)
- [Writing And Installing WASM Extensions](./wasm-writing-and-installing-extensions/)
- [Customer Rust Vs Third-Party WASM](./customer-vs-wasm/)
