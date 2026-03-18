# Operational Cutover and Rollback Planning

**Part:** Migration and Evolution  
**Chapter:** 85

Cutover is where architecture becomes operations. The platform already assumes strong cache behavior, explicit TLS handling, object storage, managed assets, and auth-aware request handling. A launch plan therefore has to coordinate runtime, data, certificates, routing, and support procedures together. A deployment is not “live” because the binary starts. It is live when traffic, editors, admins, assets, background jobs, and rollback paths are all coherent.

## Preconditions

Before traffic moves, the following should already be true:

- the target customer app has passed staging tests on the exact core and module versions intended for production
- TLS mode is selected and working, whether that is ACME issuance, Cloudflare Origin CA, or external termination
- build assets have been published with a versioned manifest
- object storage and local-only exceptions have been validated against the chosen storage policy
- cache backends and invalidation channels are live
- redirects, canonical URLs, and sitemap outputs are ready
- support and editorial teams know which admin surfaces change at cutover

If any of those are unknown on launch day, the launch is not ready.

## Recommended Cutover Sequence

The default production sequence is:

1. Freeze content and operationally sensitive writes in the legacy system for the shortest practical window.
2. Run the final delta import for content, users, memberships, events, and managed assets.
3. Apply any outstanding schema and auth-model migrations in the target system.
4. Warm critical caches and verify storage reachability, fragment rendering, and login or session creation.
5. Activate routing changes, DNS changes, load-balancer changes, or CDN origin changes.
6. Watch live diagnostics for auth failures, cache leaks, media misses, webhook failures, and transactional journey errors.

The exact edge mechanism can vary, but the sequence should not. Data convergence must happen before traffic moves.

## TLS, CDN, and Edge Concerns

The core TLS layer supports ACME, Cloudflare-assisted DNS validation, Cloudflare Origin CA, and manual certificate install. The operational plan should state which mode is used and who owns failure response. For example:

- DNS-01 ACME is preferred for wildcard certificates, multi-node deployments, and CDN-fronted sites.
- Cloudflare Origin CA is appropriate only when the origin is intentionally private behind Cloudflare.
- External termination is valid, but then certificate lifecycle and renewal are outside the platform and must be documented elsewhere.

Cutover plans should include certificate issuance timing, DNS propagation expectations, and rollback behavior if certificate provisioning or origin validation fails.

## Rollback Semantics

Rollback must be defined per capability, not as “put the old code back.” Some parts of the platform are easy to reverse, and some are not.

- DNS or load-balancer changes are reversible.
- Core or module artifact versions are reversible if no irreversible data migration has taken place.
- Content imports may be reversible if the new platform has not become the source of truth.
- Orders, bookings, reservations, or account changes created after cutover require reconciliation, not denial.

That is why rollback triggers need to be explicit before launch. If a customer app begins writing bookings or orders in the new system, rollback may mean routing users back to legacy while preserving those new writes for support handling, not erasing them.

## Typical Rollback Triggers

The launch plan should declare hard rollback triggers such as:

- systemic auth failure in admin or customer account areas
- incorrect cache scoping that exposes personalized content
- checkout, booking, or membership activation failures above an agreed threshold
- media delivery failures for core customer journeys
- TLS or origin validation errors that cannot be corrected quickly

These are not “monitor and see” situations. They are predefined decision points.

## Operational Communication

Cutover affects more than request traffic. Editors, marketers, support staff, and finance or operations teams may all be working in changed interfaces. The plan should therefore assign:

- who announces freeze start and freeze end
- who validates admin and editorial workflows immediately after launch
- who monitors background jobs, webhooks, and import reconciliation
- who authorizes rollback

Strong software architecture reduces cutover risk, but it does not remove the need for explicit human coordination.

## Preferred Launch Pattern

For most customer apps, the preferred pattern is a short freeze window, a final import, a controlled switch, and a time-boxed observation period with rollback still available. The longer a launch remains half-switched, the more likely it is to create data ambiguity. A platform built around clear ownership should launch the same way.
