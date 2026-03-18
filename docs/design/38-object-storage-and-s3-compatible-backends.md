# Object Storage and S3-Compatible Backends

**Part:** Data and Storage  
**Chapter:** 38

Core provides a storage abstraction that treats local disk and S3-compatible object storage as explicit backends with policy-driven usage rather than as interchangeable implementation details. The platform needs this because it serves both deployment artifacts and managed files, some of which are public, some private, and some intentionally retained only on the application side.

The default model is object-storage friendly. Public uploads and other shared files are written through to the configured S3-compatible backend and treated as the durable source of truth unless policy says otherwise. Private shared files also belong in object storage by default, but they are delivered through private access patterns rather than through a CDN. Local storage still exists, but it is no longer assumed to be the primary production filesystem for everything.

The abstraction includes object keys, metadata, content types, integrity information, and access strategy. Signed access is supported where files should remain private but still be retrievable by authorized users or downstream services. Customer apps choose the actual provider and credentials, while official native modules request storage behavior through the core API. That keeps media, exports, and document flows consistent even when different customers deploy against different S3-compatible systems.

The storage layer also has to coexist with the platform's auth and policy model. A file being stored in object storage does not imply that it is public. Delivery mode and publication state are separate concerns. The storage backend knows where the bytes live and how they are addressed. The publication system decides whether those bytes can be exposed through a CDN, a signed URL, an application proxy, or not at all.

WASM extensions can request storage operations through host APIs, but they do not receive raw object-store credentials. This preserves auditability and allows the host to enforce capability checks, per-path policy, and upload metadata validation. It also means extensions can remain portable across deployments that use different vendors behind the same S3-compatible contract.

This storage model is intentionally biased toward stateless deployment for shared data. When a file needs to be available across nodes, object storage is the normal answer. Local-only retention is supported, but it is treated as an explicit exception with operational consequences rather than as the quiet default.
