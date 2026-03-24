# Membership and Subscription Modules

**Part:** Native Batteries  
**Chapter:** 55

Memberships and subscriptions are first-party native modules because they combine account state, billing lifecycle, entitlement checks, and domain-specific access rules. They are central to the reference customer and are likely to recur across similar deployments, but they still do not belong in core because they are product features, not universal runtime primitives.

## Scope
The membership distribution should cover:

- plans or tiers
- subscription lifecycle hooks
- entitlement assignment and revocation
- upgrade, downgrade, cancellation, and renewal flows
- account gating and member-specific customer experience
- integration points into commerce, content visibility, and event eligibility

The commerce modules remain responsible for the money movement and order substrate. The membership modules interpret those commercial events into ongoing access rights and member state.

## Auth and Entitlements
Membership behavior should integrate with the capability-based auth system rather than bypass it. A plan purchase may create or update relations that satisfy member capabilities. A downgrade or expiry revokes them. This keeps content access, admin permissions, event booking eligibility, and member-only media publication aligned with the same authorization model used elsewhere in the platform.

The reference customer makes this especially important. Membership state is not an isolated billing concern; it affects which pages a user may see, which prices they receive, which bookings they may create, and what the account dashboard presents to them.

## Operational Model
Subscriptions are long-lived workflows, so the module set depends on the job system, notification infrastructure, audit trails, and admin shell. Renewals, grace periods, retries, and entitlement updates should happen through explicit domain events and scheduled work rather than hidden cron logic embedded in templates or controllers.

This module family is a good example of why the platform split matters. Core provides durable primitives such as auth, jobs, transactions, and storage. Commerce provides billing and order mechanics. Membership modules turn those into a reusable product capability centered on ongoing access and customer state.
