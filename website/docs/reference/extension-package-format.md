---
title: Extension Package Format
---

Davenda runtime-installed extensions are declared in two places:

- the customer app manifest installs them
- each extension package describes its own artifact, manifest, handlers, and grants

Start with the minimal pair:

```toml
# app.toml
[[extensions]]
id = "shoppr-waitlist-tools"
package_version = "0.1.0"
artifact_sha256 = "a86d1a1646e818bf600653ba6cf3585162157517acf74d76c20ecba3328694dc"
customer_app_id = "shoppr"

[[extensions.handlers]]
id = "home.waitlist.banner"
grants = []
```

```toml
# package.toml
[manifest]
id = "shoppr-waitlist-tools"
display_name = "Shoppr Waitlist Tools"
version = "0.1.0"
host_api_version = "1.0.0"

[[handlers]]
id = "home.waitlist.banner"
export = "exports.home_waitlist_banner"
point = "render-hook"
target = "cms.page.render"
grants = []
```

That pair tells Davenda:

- which package is installed
- which handler is activated
- which artifact hash is expected
- which host API contract the package expects
- which grants are requested and approved

Use this page when you want to answer:

- what goes in `package.toml`
- what goes in `app.toml`
- how handlers map to extension points
- how grants are declared and installed
- where Shoppr and Gitly provide concrete examples

## The Two Files You Always Need

### 1. The customer app install entry

Customer apps install extensions through `[[extensions]]` blocks in `app.toml`.

Example from Gitly:

```toml
[[extensions]]
id = "gitly-community-pulse"
package_version = "0.1.0"
artifact_sha256 = "ef2b0bc15aa0baf178df23d3671bf0a2914c618e394f985441e27a5fdd7c89d7"
customer_app_id = "gitly"

[[extensions.handlers]]
id = "community-pulse"
grants = []
```

This file says:

- which package is installed
- which artifact hash is expected
- which customer app owns the installation
- which handlers are activated
- which grants the installation approves for each handler

### 2. The extension package manifest

Each package has its own `package.toml` under the app’s `extensions/` directory.

The package manifest is the extension's own contract. The app manifest is the installation contract. You need both.

## What `package.toml` Contains

Important top-level fields:

- `publisher`
  - human and operational provenance for the package
- `artifact`
  - relative path to the `.wasm` artifact
- `artifact_sha256`
  - content hash pinned by the installer
- `source_wat`
  - demo-only source hint used by the checked-in sample apps
- `[manifest]`
  - extension identity and contract versioning
- `[[handlers]]`
  - each exported handler inside the package

## Manifest Fields

The manifest layer maps to `ExtensionManifest` in
`crates/davenda-wasm/src/manifest/manifests.rs`.

The main fields are:

- `id`
  - package identity
- `display_name`
  - human-facing name
- `version`
  - package version
- `host_api_version`
  - the host API contract the package expects

Gitly example:

```toml
[manifest]
id = "gitly-actions-scheduler"
display_name = "Gitly Actions Scheduler"
version = "0.1.0"
host_api_version = "1.0.0"
```

This is the part that makes compatibility explicit. A package should not assume the host ABI by guesswork.

## Handler Fields

Each `[[handlers]]` block maps to `HandlerManifest`.

Important fields:

- `id`
  - handler identity inside the package
- `export`
  - exported function name in the artifact
- `point`
  - extension point kind
- `target`
  - route, slot, or scheduled job target depending on point kind
- `grants`
  - requested host grants

Examples:

### Shoppr render hook

`apps/shoppr/extensions/shoppr-waitlist-tools/package.toml`

```toml
[[handlers]]
id = "home.waitlist.banner"
export = "exports.home_waitlist_banner"
point = "render-hook"
target = "cms.page.render"
grants = []
```

### Gitly API handler

`apps/gitly/extensions/gitly-community-pulse/package.toml`

```toml
[[handlers]]
id = "community-pulse"
export = "exports.community_pulse"
point = "api"
target = "/api/github/pulse"
grants = []
```

### Gitly scheduled job

`apps/gitly/extensions/gitly-actions-scheduler/package.toml`

```toml
[[handlers]]
id = "nightly-refresh"
export = "exports.nightly_refresh"
point = "scheduled-job"
target = "github.actions.refresh"
grants = []
```

## Installation Grants

Packages request grants, but the customer app installation still decides which grants are actually
approved for the activated handler.

That boundary is enforced through:

- `crates/davenda-wasm/src/grants.rs`
- `apps/shoppr/crates/shoppr-app/src/extensions.rs`
- `apps/gitly/crates/gitly-app/src/extensions.rs`

Current grant kinds include:

- data read/write
- auth checks and tuple writes
- storage read/write
- render fragment access
- metadata write
- cache hint write
- outbound HTTP by named integration
- secret read
- job enqueue

In the checked-in demo packages, the handlers intentionally use empty grant sets so the package
format is easy to understand first.

## How To Read The Format

Read it in this order:

1. installation block in `app.toml`
2. package `[manifest]`
3. `[[handlers]]`
4. requested and approved grants

That sequence tells you:

- whether the package is installed
- what the package claims to be
- what code paths it exposes
- what the runtime will actually let it do

## Defaults And Constraints

- The customer app must pin `artifact_sha256`.
- The extension package `manifest.id` must match the installed `id`.
- Each handler id must be valid and unique within the package.
- `host_api_version` is required.
- Resource limits are assigned from `ResourceLimits::baseline_for(...)` in the app loaders today.
- Demo apps currently compile simple WAT examples into the runtime extension directory from
  `apps/.../crates/*-app/src/extensions.rs`.

## What A Good Extension Package Looks Like

A good package is:

- easy to identify
- easy to pin
- explicit about handlers
- narrow in grants
- tied to one bounded use case

If a package starts looking like a replacement application backend, it is crossing the wrong boundary.

## Common Mistakes

- Do not describe a handler in `package.toml` and forget to install it in `app.toml`.
- Do not request grants in a package and assume they are active automatically.
- Do not treat `target` as a free-form comment.
  - it must match the semantics of the selected extension point
- Do not use WASM for first-party customer logic that should be linked Rust.
  - use [Customer Rust Vs Third-Party WASM](./customer-vs-wasm.md) to choose the boundary

## Full Implementation

Customer app install examples:

- `apps/shoppr/app.toml`
- `apps/gitly/app.toml`

Package examples:

- `apps/shoppr/extensions/shoppr-waitlist-tools/package.toml`
- `apps/gitly/extensions/gitly-community-pulse/package.toml`
- `apps/gitly/extensions/gitly-actions-scheduler/package.toml`

Runtime model code:

- `crates/davenda-wasm/src/manifest/package.rs`
- `crates/davenda-wasm/src/manifest/manifests.rs`

App loaders:

- `apps/shoppr/crates/shoppr-app/src/extensions.rs`
- `apps/gitly/crates/gitly-app/src/extensions.rs`

## Read Next

- [WASM Host APIs](./wasm-host-apis.md)
- [Customer Rust Vs Third-Party WASM](./customer-vs-wasm.md)
- [Gitly Extensions And Host APIs](../use-cases/gitly/extensions-and-host-apis.md)
- [Shoppr WASM Extensions](../use-cases/shoppr/wasm-extensions.md)
