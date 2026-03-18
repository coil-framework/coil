# Multi-Node Deployment Patterns

**Part:** Operations  
**Chapter:** 70

The default production shape is horizontally scaled and mostly stateless. Web nodes can come and go without carrying unique business state, while durable state lives in shared systems such as Postgres, distributed cache, queue infrastructure, and S3-compatible object storage. This is the baseline assumption for customer apps unless they deliberately opt into an exception.

## Shared Services

A healthy multi-node deployment shares at least:

- the primary relational database
- distributed cache and coordination services, using Redis or Valkey
- job queue state and scheduler coordination
- object storage for uploads and published assets
- shared configuration, feature flags, and certificate state

Moka remains useful as an in-process L1 cache, but it is strictly local to each node and never treated as a source of truth.

## Node Roles

The platform can run web, worker, and scheduler responsibilities together in small environments, but larger deployments should separate them operationally. The important rule is ownership, not a fixed topology:

- web nodes serve requests and short-lived rendering work
- worker nodes process jobs, webhooks, sync tasks, and retries
- scheduler responsibility runs once per logical deployment, either through leader election or an explicitly designated instance

This avoids duplicate scheduled work and keeps background pressure from destabilizing request latency.

## Session, Cache, and Storage Behavior

Sessions should be lean and shared rather than local-memory heavy. Cache strategy should combine:

- local in-process L1 for hot read reduction
- distributed L2 for shared invalidation, locks, rate limiting, and coordination
- CDN or reverse-proxy cache for public traffic

For storage, the object store is the source of truth by default. Uploads should use write-through behavior unless explicitly marked `local_only_sensitive`.

## Local-Only Exceptions

`local_only_sensitive` is permitted but intentionally noisy. It breaks the default stateless assumption and therefore requires one of:

- a single-node deployment
- a shared private volume
- deliberate routing affinity

Most sensitive shared files should still prefer private encrypted object storage over local-only placement. The local-only mode exists for edge cases, not as a normal customer-app storage strategy.

## Deployment Artifacts

Published frontend assets should be versioned at build or deploy time and distributed through their manifest, not synchronized lazily on request. The same principle applies to extension packages and other immutable runtime artifacts: they should be deployed consistently across nodes rather than discovered opportunistically.

## Operational Summary

If a feature requires per-node local state to function correctly, it should be treated as an exception and documented as such. The normal platform contract is that customer apps scale by adding interchangeable nodes around shared data, shared coordination, and shared storage.
