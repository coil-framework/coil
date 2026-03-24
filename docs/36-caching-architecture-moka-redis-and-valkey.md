# Caching Architecture: Moka, Redis, and Valkey

**Part:** Data and Storage  
**Chapter:** 36

Caching is a core service because the platform is expected to serve public pages efficiently while still handling personalized account, booking, and admin traffic safely. Core therefore provides a single cache abstraction with explicit scopes, TTLs, invalidation hooks, stale-while-revalidate behavior, request coalescing, and metrics. Modules declare cache intent through that abstraction instead of talking directly to whichever backend happens to be configured.

The cache stack is deliberately layered. `moka` is the in-process L1 for fast local reuse inside a single application instance. Redis and Valkey provide the distributed layer used for shared data, tags or surrogate-style invalidation, locks, rate-limiting coordination, and other workloads that require cross-node visibility. The platform supports both Redis and Valkey because they satisfy the same role in the architecture. Memcached was intentionally dropped because it does not align well with the richer invalidation and coordination semantics the framework expects.

Scope is the most important design feature. Cached data is never just "cached" in the abstract; it is `public`, tenant-scoped, locale-scoped, user-scoped, session-scoped, or explicitly uncacheable. That classification is necessary because the system mixes public catalog and content pages with highly personalized account and booking flows. Auth-sensitive data must not leak through careless reuse, and locale-sensitive content must not be served under the wrong variation key.

Official native modules are responsible for declaring useful invalidation behavior. Publishing a CMS page should invalidate the page itself, affected navigation fragments, related sitemap output, and any cached JSON-LD or metadata derived from that resource. A booking or reservation change should invalidate the relevant availability fragments and summaries. The platform cache is only as good as the module-level invalidation rules built on top of it.

WASM extensions can supply cache hints or request cached host operations, but they do not own raw connections to the backing stores. That keeps invalidation, observability, and safety in one place. It also means a customer-specific extension cannot silently depend on Redis-only semantics if the host contract never promised them.

The design goal is not to hide caching from developers. It is to make the rules visible enough that teams can reason about performance and correctness at the same time. A cache hit is useful only if the platform can still explain why the result was safe to reuse.
