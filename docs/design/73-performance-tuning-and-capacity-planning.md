# Performance Tuning and Capacity Planning

**Part:** Operations  
**Chapter:** 73

The platform is optimized around server-rendered HTML with progressive enhancement, not around an API-everywhere architecture. Capacity planning should therefore start with the request classes the system actually serves: public pages, personalized account views, admin workflows, checkout, webhook ingestion, and background jobs. The right goal is predictable throughput with low memory overhead, not just raw benchmark numbers.

## The Main Performance Levers

The chat established several defaults that matter more than micro-optimizations:

- page and fragment caching
- async job offloading by default
- image and heavy media processing outside the request path
- lean session handling
- explicit N+1 detection in development
- first-class observability

This means most tuning work focuses on data access, auth batching, cache strategy, and queue pressure rather than on template syntax alone.

## Cache Hierarchy

The intended cache stack is:

- moka as local in-process L1
- Redis or Valkey as distributed L2 for shared invalidation, locks, and coordination
- reverse proxy or CDN cache for public responses and published assets

Every cached response needs the correct scope. Public pages can ride the full stack. Locale-scoped or tenant-scoped responses can still use shared caches with proper keys. User-scoped and session-scoped responses must stay out of shared public caches.

## Query and Auth Pressure

Auth is built into the core architecture, so capacity planning must treat authorization cost as a first-class input. Poorly batched capability checks or recursive CTE plans will hurt sooner than most template rendering choices. The same is true for module-level data access on event listings, timeslot availability, and personalized account views.

The important tuning questions are:

- are auth checks batched
- are module queries avoiding N+1 behavior
- are cache invalidation rules precise enough to preserve hit rate
- is background work stealing resources from web request handling

## Workload Classes

The platform should benchmark and size around at least:

- public CMS and catalog pages
- event and booking availability pages
- checkout and authenticated account flows
- admin list and edit screens
- webhook bursts
- scheduled or batch jobs

The current business context makes events, memberships, and bookings just as important as generic storefront traffic.

## Capacity Planning Guidance

Add nodes when request concurrency or worker backlog demands it, but only after checking whether the real bottleneck is database shape, auth query cost, cache misses, or expensive sync work in the request path. The platform's architecture is designed so those problems are visible. Capacity planning should use that visibility rather than treating hardware growth as the first answer.
