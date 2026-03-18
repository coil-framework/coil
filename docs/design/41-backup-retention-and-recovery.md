# Backup, Retention, and Recovery

**Part:** Data and Storage  
**Chapter:** 41

Backups are defined around source-of-truth data, not around every byte the platform can regenerate. The critical state for a customer app is the Postgres database, including auth tuples and module data, plus managed objects stored through the platform storage layer. Caches, derived search indexes, generated JSON-LD fragments, and in-process state are disposable. Hashed frontend bundles are deployment artifacts and are reproduced from source and the build pipeline rather than treated as irreplaceable runtime data.

## Data Classes
The storage policy introduced earlier in the book drives durability expectations:

- `public_asset` is rebuildable from the deploy pipeline and published through a versioned manifest.
- `public_upload` and `private_shared` are managed objects and must be recoverable from the object store plus database metadata.
- `local_only_sensitive` is an exception path for files that must never sync away from the server. It is supported, but it intentionally breaks the normal stateless multi-node assumption and therefore requires explicit operator acceptance.

Retention policy is a deployment concern, but the platform architecture assumes point-in-time recovery for Postgres, versioned or otherwise durable object storage for managed blobs, and separate treatment of local-only files. A restore is only complete when database state, auth state, and object metadata agree on what exists and what is published.

## Recovery Model
Single-app operational recovery restores the customer app to a consistent model version, then rehydrates derived state. The expected order is:

1. Restore Postgres, including auth tuples and model package metadata.
2. Restore or reattach the managed object store.
3. Rebuild disposable layers such as caches, search indexes, and reporting snapshots.
4. Redeploy static theme assets from the build artifact manifest rather than from ad hoc file copies.

Platform-level disaster recovery is broader. Core services must come back first, but customer apps remain isolated units of recovery because each app chooses its own modules, locales, auth model, and storage rules. Backup tooling therefore treats the app boundary, not the full platform, as the main operational unit.

## Responsibilities and Caveats
Core owns backup-aware primitives: transactional persistence, object metadata, storage classes, and the distinction between managed assets and deployment artifacts. Official modules must store business state through those primitives and must be able to rebuild projections after restore. Customer apps own retention policy, legal hold rules, and whether they accept the operational tradeoffs of `local_only_sensitive`.

The main architectural caveat is that not every storage policy implies the same recovery shape. A media library item stored as `private_shared` fits the distributed model and can be restored on any node. A sensitive export forced to `local_only_sensitive` requires host-local backup and deliberate placement during restore. The platform allows that choice, but it keeps it noisy so operators understand they are stepping outside the default recovery model.
