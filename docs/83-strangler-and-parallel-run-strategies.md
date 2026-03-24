# Strangler and Parallel-Run Strategies

**Part:** Migration and Evolution  
**Chapter:** 83

Incremental migration is often safer than a single cutover, but only if the boundaries are explicit. The platform should support strangler and parallel-run strategies without normalizing long-term dual ownership. The target state is always one system of record per capability. Incremental techniques exist to reduce launch risk, not to create a permanent hybrid architecture.

## When To Use A Strangler Pattern

A strangler strategy makes sense when:

- the legacy system has too much traffic or too many business-critical flows for a one-step replacement
- public content pages can move independently of account, checkout, or booking flows
- legacy editorial tooling must remain live while the new runtime proves itself
- high-value integrations such as payments, memberships, or event booking need phased confidence

The new platform is particularly well suited to this pattern because customer apps can adopt official modules incrementally. A site may begin with CMS pages and media on the new platform, then move account areas, then move commerce or events.

## Safe Boundary Choices

The best transition boundaries are coarse and observable:

- path or hostname ownership, such as `/events` or `shop.example.com`
- module ownership, such as media delivery moving first while checkout remains legacy
- back-office ownership, such as new admin for content while legacy admin still runs orders

The worst boundary is hidden dual-write logic across multiple overlapping systems. If a flow is migrated, the new platform should become the source of truth for that flow as quickly as practical.

## Identity and Session Strategy

Shared identity is usually the first place a parallel run becomes fragile. The preferred order is:

1. Use a shared identity provider or a temporary authentication bridge so users are not forced through conflicting account systems.
2. Normalize authorization in the new platform through capability checks, even if the legacy side still uses older role models.
3. Avoid sharing session storage unless both runtimes can do so safely and intentionally.

For customer-facing flows, explicit re-authentication at a controlled boundary is often safer than pretending two unrelated session models are interchangeable.

## Data Synchronization Rules

The platform should support staged import, delta import, and reconciliation, but it should resist full bidirectional sync. During a parallel run each important entity should have a declared source of truth:

- pages and media may move first to the new platform
- orders may remain in the legacy stack until checkout is migrated
- bookings may move only when availability, reservations, and confirmation flows can be handled entirely in the new runtime

Read shadowing is safer than write shadowing. It is reasonable to replay page requests, event availability lookups, or search queries against the new system and compare results. It is far less safe to accept writes in two systems and reconcile them later.

## Parallel-Run Confidence Building

A parallel run should have explicit success criteria before it starts. Useful measures include:

- route-level response correctness for public pages and fragments
- cache correctness across locale, tenant, and auth-aware scopes
- parity on redirects, canonical URLs, and JSON-LD output
- auth-decision parity for equivalent admin actions
- booking, reservation, checkout, or membership flow success rates
- import lag and reconciliation error counts

The point is not to prove the systems are identical in every internal detail. The point is to prove the customer experience and operational semantics are good enough to switch ownership.

## Example Transition Shape

For an events-and-memberships customer app, a credible migration sequence is:

1. Move brochure pages, navigation, redirects, and media delivery to the new CMS and storage stack.
2. Move public event catalogue pages next, using the new SEO, i18n, and cache layers while legacy booking remains linked or proxied.
3. Move account and membership flows once auth and profile migration are complete.
4. Move booking, capacity, waitlist, and check-in only when the new events module can own reservations and final confirmation atomically.

This preserves user-facing continuity while avoiding the mistake of splitting a single transactional workflow across two runtimes.

## Exit Criteria

A strangler phase ends when:

- all routes for the target capability are served by the new customer app
- the new system is the only writer for the associated data
- operational dashboards, support tooling, and admin procedures have moved with it
- rollback is no longer needed for that capability

Until those conditions are met, the migration is still transitional and should be treated as such in release planning and on-call procedures.
