---
title: Media Module
---

The media module owns managed assets, media-library workflows, storage-policy UI, and publication
state above the raw storage engine.

Primary implementation files:

- `crates/davenda-media/src/module/manifest.rs`
- `apps/shoppr/templates/admin/dashboard.html`

## Why It Exists

Raw object storage is not a usable product feature by itself. Real apps need:

- asset metadata
- revision history
- publication state
- derived files
- storage policy controls

The media module provides that domain layer.

## What It Provides

From `crates/davenda-media/src/module/manifest.rs`, media adds:

- migrations for libraries, assets, and derivatives
- admin routes for `/admin/media` and `/admin/media/storage`
- a managed delivery route at `/media/files/{asset_id}`
- follow-up jobs for derivative generation and storage sync
- admin resources and search contribution for media

## How To Enable It

```toml title="app.toml"
[modules]
enabled = ["media"]
```

```toml title="platform.dev.toml"
[modules]
enabled = ["media"]
```

Shoppr enables it in both files and then uses it from CMS, commerce, and admin surfaces.

## How To Disable It

Remove `media` from the enabled module lists and remove any customer surfaces that assume managed
asset workflows. CMS and commerce can still exist, but they lose the media-library integration
points declared in their optional module dependencies.

## Config Expectations

Media depends heavily on shared storage configuration. The important settings live under
`[storage]` in platform config, not under a large media-specific block.

Shoppr's concrete example is in `apps/shoppr/platform.dev.toml`:

- `default_class = "public_upload"`
- distributed object-store storage
- published asset manifest support through `[assets]`

## Routes And Surfaces

Important routes from the manifest:

- `/admin/media`
- `/admin/media/storage`
- `/media/files/{asset_id}`

In practice, the public delivery behavior is also shaped by storage policy and publication state.

## Required Auth Capabilities

Media requires:

- `asset.read`
- `asset.publish`
- `asset.replace`
- `asset.manage_storage`

Optional capabilities become relevant when the module integrates with admin, CMS, SEO, or i18n.

## How Customer Apps Extend It

Media exposes:

- admin widget slot: `media.asset.sidebar`
- render hook slot: `media.asset.metadata`

Customer apps usually extend media by:

- adding templates that display media metadata
- connecting media into CMS and commerce pages
- customizing storage policy defaults in platform config

## Where To See It

Shoppr uses media as part of the broader store workflow:

- CMS pages can reference managed assets
- commerce pages can render product media
- admin surfaces point operators toward media and storage workflows

## Common Mistakes

- Treating media as just “uploads” and ignoring publication state.
- Forgetting that media routes depend on auth and storage policy, not just template markup.
- Skipping storage-policy reasoning when moving from local dev to object-store deployments.

## Read Next

- [CMS](./cms.md)
- [Commerce](./commerce.md)
