---
title: Writing And Installing WASM Extensions
---

This page shows the full path for a Coil WASM extension:

1. define the product slot in the customer app
2. describe the package in `extensions/<id>/package.toml`
3. install it in `app.toml`
4. let the runtime load only the approved handlers and grants

If you cannot follow those four steps from the docs alone, the docs are failing. This page exists to
make the process concrete.

## The Smallest Real Extension

Shoppr ships the smallest useful example:

```toml title="apps/shoppr/extensions/shoppr-waitlist-tools/package.toml"
publisher = "harbor-marketplace"
artifact = "shoppr-waitlist-tools/shoppr-waitlist-tools.wasm"
artifact_sha256 = "3ad7b44218d04a3eba602051cbcb991bdd1ab69fd55ad995cd688af26ca6d067"
source_wat = "shoppr-waitlist-tools.wat"

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

And the customer app installs it like this:

```toml title="apps/shoppr/app.toml"
[[extensions]]
id = "shoppr-waitlist-tools"
package_version = "0.1.0"
artifact_sha256 = "3ad7b44218d04a3eba602051cbcb991bdd1ab69fd55ad995cd688af26ca6d067"
customer_app_id = "shoppr"

[[extensions.handlers]]
id = "home.waitlist.banner"
grants = []
```

That is the full minimum:

- one package
- one handler
- one target
- no host grants

## Step 1: Define A Product Slot

The extension point should come from product vocabulary, not from generic framework jargon.

Gitly shows this clearly:

```rust title="apps/gitly/crates/gitly-app/src/lib.rs"
ExtensionSlotDescriptor::new(
    ExtensionSlotKind::Api,
    "/api/github/pulse",
    "Allows bounded third-party extensions to contribute GitHub-style community pulse API data",
)
```

and:

```rust title="apps/gitly/crates/gitly-app/src/lib.rs"
ExtensionSlotDescriptor::new(
    ExtensionSlotKind::ScheduledJob,
    "github.actions.refresh",
    "Allows bounded third-party scheduled jobs to simulate GitHub Actions refresh cycles",
)
```

Your customer app owns those names. A package can only target what the app chose to expose.

## Step 2: Write `package.toml`

Every package needs:

- publisher metadata
- a pinned artifact path and hash
- a manifest identity
- at least one handler

Gitly’s API package is a good example:

```toml title="apps/gitly/extensions/gitly-community-pulse/package.toml"
publisher = "gitly-demo"
artifact = "artifacts/gitly-community-pulse.wasm"
artifact_sha256 = "ef2b0bc15aa0baf178df23d3671bf0a2914c618e394f985441e27a5fdd7c89d7"
source_wat = "gitly-community-pulse.wat"

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

Interpret it literally:

- `id` is the package identity
- `export` is the function the host calls in the WASM artifact
- `point` decides the runtime contract
- `target` must match the customer-app slot
- `grants` is the package request, not the final approval

## Step 3: Install The Package In `app.toml`

The customer app chooses whether a package is installed at all.

Gitly installs its scheduled job package like this:

```toml title="apps/gitly/app.toml"
[[extensions]]
id = "gitly-actions-scheduler"
package_version = "0.1.0"
artifact_sha256 = "eadcc0e65bf8059fa9411be20e885a976d415e7747e505125c2f15ef662e333f"
customer_app_id = "gitly"

[[extensions.handlers]]
id = "nightly-refresh"
grants = []
```

This is the runtime installation contract. It says:

- the app accepts exactly this package id
- the app expects exactly this artifact hash
- the app enables exactly this handler
- the app approves exactly this handler grant set

## Step 4: Let The Customer Loader Resolve It

Gitly’s loader path is straightforward:

```rust title="apps/gitly/crates/gitly-app/src/extensions.rs"
for extension in load_declared_extensions(manifest_path)? {
    manifest = manifest.with_extension(extension);
}
```

and later:

```rust title="apps/gitly/crates/gitly-app/src/extensions.rs"
document
    .extensions
    .into_iter()
    .map(|extension| load_extension_package(app_root, extension_directory, &extension.id))
    .collect()
```

That means the customer app:

1. reads installed extensions from `app.toml`
2. loads each package from `extensions/<id>/package.toml`
3. verifies the package shape
4. builds `ExtensionPackage`
5. hands it to the runtime

## A Second Real Example: Scheduled Jobs

Gitly’s scheduled-job package shows the same model with a different point kind:

```toml title="apps/gitly/extensions/gitly-actions-scheduler/package.toml"
[[handlers]]
id = "nightly-refresh"
export = "exports.nightly_refresh"
point = "scheduled-job"
target = "github.actions.refresh"
grants = []
```

Nothing magical changed. Only the target and contract changed:

- API handlers target a route
- scheduled-job handlers target a named job surface
- render-hook handlers target a named render slot

## What You Need To Ship

For a real extension package, commit:

- `extensions/<id>/package.toml`
- the `.wasm` artifact or a reproducible build input
- the install block in `app.toml`
- any docs for required grants or integration wiring

For the checked-in demos, the `.wat` source is also committed because the package compiler builds the
demo artifacts locally.

## What The Runtime Will Reject

The runtime or customer loader should reject:

- a package id that does not match the installed id
- a mismatched artifact hash
- an unknown target
- an unsupported host API version
- a handler installation that requests grants the app did not approve

## Practical Workflow

Use this sequence when creating a new extension:

```bash
cd apps/gitly
cargo run -p gitly -- extension-checksums
cargo run -p gitly -- validate
```

For Shoppr:

```bash
cd apps/shoppr
cargo run -p shoppr -- validate
```

That validates the customer app and installed extension configuration before you try to serve it.

## Read Next

- [Extension Package Format](./extension-package-format/)
- [WASM Host APIs](./wasm-host-apis/)
- [WASM Host Service Examples](./wasm-host-service-examples/)
