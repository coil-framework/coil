# Reference Storage Policy Model

**Part:** Appendices  
**Chapter:** 90

Storage policy is a core concern because the platform owns asset distribution, object storage, local-only exceptions, and publication behavior. The policy model answers where bytes live, how they are replicated, and how they may be delivered. It does not answer who is allowed to publish or view an asset. That remains an authorization concern and is bridged through capability bindings.

## Policy Fields

The reference model uses the following fields:

| Field | Purpose | Typical values |
| --- | --- | --- |
| `class` | Named storage-profile shorthand | `public_asset`, `public_upload`, `private_shared`, `local_only_sensitive` |
| `delivery_mode` | How the asset is served | `public_cdn`, `signed_url`, `app_proxy`, `local_only` |
| `sync_mode` | Whether bytes replicate to object storage | `object_store`, `local_only` |
| `sensitivity` | Handling and operational expectations | `public`, `internal`, `restricted`, `secret` |
| `cache_profile` | Default HTTP or CDN cache posture | `immutable_public`, `revalidating_public`, `private`, `uncacheable` |
| `encryption` | Required at-rest handling | `provider_default`, `managed_key`, `customer_key` |
| `retention` | Lifecycle or deletion rules | named retention profile |

These fields may be represented in config, database policy records, or module metadata, but their meaning should remain stable across those forms.

## Reference Classes

The baseline platform classes are:

| Class | Meaning |
| --- | --- |
| `public_asset` | Deployment artifact published at build or deploy time and intended for public CDN delivery |
| `public_upload` | Managed asset stored in object storage and eligible for public delivery when auth state permits publication |
| `private_shared` | Managed asset stored in private object storage and delivered through signed URLs or application proxying |
| `local_only_sensitive` | Managed asset kept on local or private attached storage and never replicated to object storage |

`local_only_sensitive` is intentionally noisy. It breaks the normal stateless multi-node story and should be used only when the operational tradeoff is understood.

## Resolution Order

Policy resolution follows this order:

1. explicit per-upload override
2. folder or path rule
3. module default
4. platform default

The resolved policy is stored with the managed asset so later rule changes do not retroactively and invisibly change already-imported content.

## Publication Bridge

A managed asset may be stored in object storage and still remain non-public. Public delivery requires both a storage policy that permits public delivery and an auth state that permits publication. In practice:

- `asset.publish` governs the transition into a public state
- `asset.read_public` governs whether anonymous or public delivery is valid
- `delivery_mode = public_cdn` is only effective when those capability checks permit it

This is why publication is treated as a state transition rather than a storage flag.

## Build Assets Versus Managed Assets

Build assets and managed assets are intentionally separate.

- Build assets are hashed theme or site artifacts such as CSS, JS, and compiled bundles. They are published by the deployment pipeline and treated as public once activated.
- Managed assets are business-relevant files such as uploads, documents, event images, product photography, and downloadable member content. They are subject to auth, publication state, and policy resolution.

Only managed assets belong in the auth model at fine granularity.

## Example Policy Document

The examples below use TOML, but the model is format-agnostic.

```toml
[[rules]]
match = "assets/**"
class = "public_asset"
delivery_mode = "public_cdn"
sync_mode = "object_store"

[[rules]]
match = "uploads/products/**"
class = "public_upload"
delivery_mode = "public_cdn"
sync_mode = "object_store"

[[rules]]
match = "uploads/invoices/**"
class = "private_shared"
delivery_mode = "signed_url"
sync_mode = "object_store"

[[rules]]
match = "uploads/legal/**"
class = "local_only_sensitive"
delivery_mode = "local_only"
sync_mode = "local_only"
```

The per-upload override remains available for exceptional cases, but broad policy should be path-driven so it can be reasoned about operationally.
