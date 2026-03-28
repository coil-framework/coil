---
title: Theme Asset Delivery
---

Coil theme assets are published artifacts with a manifest-backed delivery plan, not raw files
that templates reference directly forever.

## Start With The Template Call Site

A normal template should look like this:

```html
<link rel="stylesheet" href="/theme/assets/site.css" coil:href="asset('theme/assets/site.css')" />
<script src="/theme/assets/site.js" coil:src="asset('theme/assets/site.js')" defer="defer"></script>
```

Annotated:

- the literal `href` and `src` are readable development fallbacks
- `asset('theme/assets/...')` is the real contract
- the runtime replaces the logical path with the current published URL

That one example explains the whole subsystem better than a list of crates does.

## What Problem Is This Solving?

Asset delivery needs to satisfy all of these at once:

- customer apps should use stable logical asset names
- production should serve hashed assets
- local and production templates should not diverge
- the runtime should know which published URL belongs to which logical asset

That is why Coil uses:

- asset roots in the app manifest
- publication plans
- hashed artifact paths
- an active asset manifest during rendering

## How Publication Starts

Customer apps opt assets into publication with `[theme].asset_roots`:

```toml
[theme]
asset_roots = ["theme/assets"]
```

That tells Coil where the logical asset tree begins.

## How The Delivery Flow Works

The current delivery flow is:

1. the customer app declares asset roots
2. those files are turned into deployment artifacts
3. each artifact gets a hashed path and fingerprint
4. publication produces an active asset manifest
5. request rendering injects logical-path to public-URL mappings into the template model
6. `asset('...')` resolves through that manifest

The important design choice is step 5: templates do not need to know the hash.

## What The Runtime Actually Injects

At render time, the runtime loops through the active asset manifest and adds asset-path bindings to
the `RenderModel`.

Conceptually, it is doing this:

```rust
model = model.with_asset_path("theme/assets/site.css", "https://cdn.example.com/...hashed.css")?;
```

That is why template code stays small and stable while the published URL can change on every build.

## What Delivery Targets Exist?

Coil’s asset model includes these target shapes:

- `Cdn`
- `SignedObject`
- `AppProxy`
- `LocalPath`

That is the general asset capability.

For theme assets specifically, the currently checked-in publication flow is effectively CDN/object
store first. In other words:

- theme publication expects a `cdn_base_url`
- the checked-in demos publish public theme assets as CDN-style URLs

That is an implementation fact the docs should state plainly.

## Local Example

If you picture the flow with one CSS file, it looks like this:

```text
logical asset path:
  theme/assets/site.css

published artifact path:
  theme/assets/site.<fingerprint>.css

render-time resolved URL:
  http://localhost:9002/gitly/theme/assets/site.<fingerprint>.css
```

The template still only says:

```html
coil:href="asset('theme/assets/site.css')"
```

## What Config Is Involved?

App-level:

- `[theme].asset_roots`

Runtime-level:

- `[assets].publish_manifest = true`
- `[assets].cdn_base_url = "..."`

Practical consequence:

- if you remove `cdn_base_url` today without changing the implementation, theme publication breaks

## Common Mistakes

### Assuming `asset('...')` means “served by the app process”

It means “resolve this logical asset through the current publication manifest.”

### Hardcoding `/theme/assets/...` as the final production URL

That defeats hashing and manifest resolution.

### Treating theme assets as an afterthought compared to managed assets

Theme assets are already part of the deployment contract.

## Supporting Implementation And Repo Examples

Concrete supporting files:

- `apps/shoppr/app.toml`
- `apps/shoppr/platform.dev.toml`
- `apps/gitly/app.toml`
- `apps/gitly/platform.dev.toml`
- `crates/coil-assets/src/release.rs`
- `crates/coil-assets/src/delivery.rs`
- `crates/coil-runtime/src/storage/host.rs`
- `crates/coil-runtime/src/render/model.rs`

## What Should I Read Next?

- [Theme Structure](./theme-structure.md)
- [Template Models](./template-models.md)
- [Themes, Rendering, And Assets](../core-concepts/themes-rendering-and-assets.md)
