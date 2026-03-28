---
title: Theme Asset Delivery
---

Davenda treats theme assets as publishable runtime artifacts, not loose files the browser reads
directly from your repository.

That distinction is why Shoppr and Gitly can use hashed CSS and JavaScript assets in local and
production builds without changing template structure.

## What This Page Covers

Use this page when you need to know:

- where theme assets live
- how they are hashed and published
- how they are resolved in templates
- when to use your own domain versus a CDN or object-store host

## Source Files Versus Published Assets

The customer app keeps human-edited assets in the theme:

- `apps/shoppr/theme/assets/`
- `apps/gitly/theme/assets/`

Those source files are not the final URLs browsers should request.

Davenda publishes them into managed storage and serves them through the runtime's asset-delivery
contract. The published object typically has:

- a hashed filename
- a content-type preserved from the source asset
- stable cache semantics
- a logical relationship back to the theme asset path

## How Templates Should Reference Assets

Always use the asset helper exposed by the template layer.

Do not hard-code:

- `site.css`
- raw object-store bucket URLs
- guessed hashed filenames

That rule matters because the publish step owns:

- hashing
- storage location
- MIME type
- public URL generation

## The Shoppr And Gitly Pattern

The canonical examples are:

- `apps/shoppr/templates/layouts/base.html`
- `apps/gitly/templates/layouts/base.html`

Those layouts reference the logical theme assets and let Davenda resolve the published URLs.

That is the supported pattern for:

- CSS
- JavaScript
- images shipped with the theme
- favicons and manifest files

## Local Development

In local development, the runtime still resolves published assets rather than pretending the
browser can read files from disk.

That is deliberate. Local should be close enough to production to catch real mistakes such as:

- missing publish steps
- broken MIME types
- bad asset helper usage
- object-store configuration errors

## Production Serving Options

You have two normal production options.

### Same-Domain Asset Serving

Use the main application domain for both HTML and published assets when:

- you want the simplest deployment
- you do not yet need edge caching beyond the app boundary
- you want cookies, CSP, and observability to stay straightforward

This is a perfectly valid production shape.

### CDN Or Dedicated Asset Host

Use `cdn_base_url` or an equivalent dedicated asset host when:

- you want edge caching for large global traffic
- you need stronger cache isolation between HTML and assets
- your operations model already includes a CDN

You do not need a CDN to run Davenda correctly. It is an optimisation and topology choice, not a
mandatory platform requirement.

## Object Store And Public Reachability

Davenda stores published assets in managed object storage, but customer templates should still
think in terms of the runtime delivery contract.

In practice that means:

- object storage is part of the platform plumbing
- the public URL may be same-domain, CDN-fronted, or object-store-backed
- templates should not care which one is active

If the object store is exposed directly in local development, that is a delivery detail, not the
authoring model.

## MIME Types

Davenda preserves source content types when publishing assets.

That matters because browsers will reject CSS and JavaScript if the asset store serves them as
generic binary blobs. If you see browser errors about unsupported stylesheet or script MIME types,
check the publication path first.

## Cache Behaviour

Hashed assets are designed for long-lived caching because the content hash changes when the file
changes.

Typical operational pattern:

- HTML remains more dynamic
- assets remain aggressively cacheable
- a new publish creates new URLs rather than mutating old ones in place

## Concrete Files To Read

Start here:

- `apps/shoppr/theme/assets/`
- `apps/gitly/theme/assets/`
- `apps/shoppr/platform.toml`
- `apps/shoppr/platform.dev.toml`
- `crates/davenda-cli/src/cli/app.rs`
- `crates/davenda-runtime/src/server/backend.rs`

## Common Mistakes

- Hard-coding MinIO, S3, or bucket URLs in templates.
- Referencing unhashed source filenames directly from HTML.
- Assuming `cdn_base_url` is required for production.
- Treating local asset serving as a separate product surface from production.

## Read Next

- [Theme Structure](./theme-structure.md)
- [Platform Config](./platform-config.md)
- [Asset Publication And CDN Delivery](../operations/asset-publication-and-cdn-delivery.md)
