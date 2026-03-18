# Search, Reporting, and Bulk Operations

**Part:** Native Batteries  
**Chapter:** 59

Search, reporting, and bulk operations are not glamorous, but they are the difference between a demo system and an operable product. They belong in native batteries because they are cross-domain product features built on top of core jobs, storage, auth, and admin primitives.

## Search
The platform should support indexing and search across CMS content, catalog data, media, memberships, and event records through explicit module contributions. Search adapters are valid extension points, and the chat calls them out as a natural place for customization. The important rule is that indexing remains declarative: modules publish what should be indexed and when it must be invalidated or rebuilt, while core supplies the jobs, storage, and cache coordination.

Search also has to respect authorization and publication state. Public search indexes should not leak restricted assets or unpublished content. Admin and operator search may surface more, but only through capability-checked back-office interfaces.

## Reporting
Reporting modules should be designed as asynchronous workloads. Large exports, operational dashboards, and scheduled summaries rely on the job system, object storage, and admin shell rather than on request-time query explosions. This is especially important for the reference customer, where memberships, bookings, check-ins, and orders all produce operational reporting needs.

Generated reports are managed outputs with their own storage and delivery policy. A public export and a restricted finance report should not share the same delivery mode simply because both happen to be files.

## Bulk Operations
Bulk actions are really controlled workflows. Bulk publish, bulk cancel, bulk check-in, or bulk metadata edits should run through capability checks, audit trails, and idempotent job execution. The admin shell provides the operator surface, the auth engine decides what is allowed, and the underlying modules implement the domain-specific mutation rules.

This chapter is intentionally about disciplined scale. The platform is not trying to guess every report a customer will ever want. It is providing a first-party operational substrate that lets search, reporting, and bulk actions behave consistently across the supported module set.
