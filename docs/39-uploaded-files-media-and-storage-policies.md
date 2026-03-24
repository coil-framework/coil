# Uploaded Files, Media, and Storage Policies

**Part:** Data and Storage  
**Chapter:** 39

Managed uploads are first-class domain resources, not anonymous blobs. The platform therefore models them as media or asset records with identity, metadata, ownership, and policy. That is the only workable foundation for a system where publication state, auth, storage location, and delivery mode all matter independently.

The storage-policy model separates several concerns that are often collapsed into one flag. `delivery_mode` determines whether a file is exposed through `public_cdn`, `signed_url`, `app_proxy`, or `local_only`. `sync_mode` determines whether the bytes are expected in object storage or retained only on local storage. `sensitivity` captures whether the content is public, internal, restricted, or secret. Publication capability and read capability are then governed through the authorization layer, not by raw file placement alone.

This is why publishability is modeled through auth for managed assets. An uploaded image, brochure, membership document, or admin-generated export may live in object storage, but it is only eligible for public delivery if its auth-governed state and capability bindings allow that transition. Folder and path rules still exist, but they act as policy templates and sensible defaults rather than as the final authority.

Per-folder and per-upload overrides are supported because the platform has to handle mixed workloads. A media library may default a public marketing folder to `public_cdn` with object-store sync, while a restricted documents folder defaults to `signed_url` and stricter capability requirements. A one-off upload may opt out and be marked `local_only` when the operator deliberately accepts the resulting operational tradeoff.

That tradeoff is important enough to state plainly. `local_only_sensitive` files are supported, but they are operationally noisy by design because they break the simplest stateless multi-node story. If several nodes need access to the same sensitive data, private encrypted object storage is usually the cleaner answer. Local-only retention should be reserved for cases where avoiding sync is itself part of the security or compliance posture.

Official native modules such as CMS, commerce, and media-library tooling consume this policy model rather than inventing their own upload flags. Customer apps choose the defaults and path templates appropriate to each installation. WASM extensions can propose uploads or metadata changes through host APIs, but the host remains responsible for policy enforcement and publication decisions.
