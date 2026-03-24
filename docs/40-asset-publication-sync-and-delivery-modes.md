# Asset Publication, Sync, and Delivery Modes

**Part:** Data and Storage  
**Chapter:** 40

The platform distinguishes sharply between deployment artifacts and managed assets because they move through the system differently and are governed by different rules. Compiled theme and site bundles are published by the deployment pipeline, hashed, always public once active, and resolved through a manifest. Managed assets are created or uploaded during application use, carry business meaning, and are governed by auth, storage policy, and publication state.

Sync behavior follows that distinction. Static deployment assets are published to the configured object store or CDN target during build or deploy and never "synced later" on the request path. Managed uploads use write-through semantics by default: if their policy allows object storage, the object store becomes the durable source of truth from the start. Background jobs may still handle replication, derivative generation, or publication propagation, but the runtime does not pretend that request-time copying is an acceptable operating model.

Delivery mode is a first-class policy choice. Public marketing media and other publishable assets can be served through `public_cdn`. Private but shared files can use `signed_url` or `app_proxy`, depending on whether access should be delegated directly to storage or mediated by application checks. `local_only` remains available for deliberately retained files. These modes are orthogonal to storage backend choice: the same object store may hold both publicly publishable and strictly private objects.

Auth matters here because managed-asset publication is a state transition, not a side effect of uploading bytes. The auth layer decides who may publish, unpublish, replace, or delete a managed asset, and those decisions gate whether a given object can move into a publicly cacheable delivery path. Build artifacts remain outside that fine-grained model because they are deployment outputs rather than user-managed resources.

Core owns the mechanics of publication, syncing, and URL generation. Official native modules declare the policies and workflows relevant to their resources. Customer apps choose the storage topology, CDN strategy, and per-path defaults that fit each installation. WASM extensions may request publication or delivery actions through host APIs, but they never own certificate management, CDN credentials, or direct object-store configuration.

This separation keeps the platform coherent. A new deploy swaps in a new asset manifest. A CMS author publishes a brochure and the system evaluates auth, storage, and delivery policy. A restricted document is uploaded and remains private despite living in object storage. These are different workflows and the architecture treats them that way on purpose.
