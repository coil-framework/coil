# Media Library and Asset Management

**Part:** Native Batteries  
**Chapter:** 58

The platform distinguishes sharply between deployment artifacts and managed assets. Hashed theme bundles are published by the build and deploy pipeline and become always-public once deployed. Managed assets are business objects: uploads, media items, downloadable documents, and other customer-controlled files that need workflow, metadata, and authorization. The media library exists to manage the second category.

## Asset Model
Managed assets should participate in the auth model. The conversation explicitly calls out resource types such as `asset`, `asset_folder`, `theme_asset_bundle`, and `media_library`, with capabilities including `asset.read`, `asset.read_public`, `asset.publish`, `asset.unpublish`, `asset.replace`, `asset.delete`, and `asset.manage_storage`. Publication is therefore a real state transition, not a side effect of copying a file to a public path.

Storage policy remains separate from auth. The important dimensions are:

- `delivery_mode`: `public_cdn`, `signed_url`, `app_proxy`, or `local_only`
- `sync_mode`: `object_store` or `local_only`
- `sensitivity`: public, internal, restricted, or secret

An asset may be stored in object storage and still require signed delivery. Public CDN delivery is only valid when the asset's capability state permits publication.

## Authoring Workflow
The media library module should provide foldering, metadata capture, derived metadata and image handling, replacement workflows, and reuse across CMS, commerce, and events. Per-folder or path-based defaults are useful, but they are policy templates, not the source of truth. Per-upload overrides remain possible when the app needs to keep a particular file local-only or route it through signed delivery.

## Operational Consequences
By default, uploads should use write-through object storage so the distributed store is the source of truth. `local_only_sensitive` remains supported for exceptional cases, but it is operationally noisy because it breaks the normal stateless deployment model. The media library should make that tradeoff visible to operators rather than hiding it.

This chapter is where the platform's storage, auth, and publishing ideas meet. The media module gives editors and operators a usable workflow. Core continues to own the policy engine and the actual storage machinery underneath.
