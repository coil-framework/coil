# Example Events and Memberships Installation

**Part:** Appendices  
**Chapter:** 95

This is the vertical example closest to the current target customer. It combines commerce-style account and payment flows with memberships, subscriptions, event scheduling, capacity management, bookings, content, media, and branded editorial surfaces. In practical terms it is not “just ecommerce.” It is commerce plus memberships plus events plus branded CMS and admin.

## Module Composition

A representative installation enables:

- `cms-pages`
- `media-library`
- `memberships`
- `subscriptions`
- `events`
- `bookings`
- `notifications`
- `admin-shell`
- `admin-content`
- `admin-events`

Some customers may also install `commerce-checkout`, `commerce-orders`, and `commerce-payments-stripe` for paid bookings, passes, or merchandise, but the core shape is the memberships and events stack.

## Domain Model

The central managed resources are:

- members and member profiles
- membership tiers and subscription state
- events and event slots
- reservations, holds, waitlists, and bookings
- branded pages, landing content, and managed assets

The events module owns scheduling and capacity. The memberships and subscriptions modules own entitlement. Booking creation depends on both capability checks and business-rule checks such as slot availability or active membership state.

## End-To-End User Flow

A typical user journey looks like this:

1. The customer browses localized event pages rendered through the CMS and events modules.
2. The app checks eligibility, which may depend on membership state, region, or event visibility rules.
3. The booking flow creates a time-bounded reservation or hold before final confirmation.
4. Payment or membership validation completes.
5. The system confirms the booking, updates capacity, and emits notifications or pass artifacts as needed.

That flow is precisely why the platform keeps auth, jobs, caching, and domain modules in native code rather than forcing major transactional paths through WASM.

## Auth And Capability Mapping

The default auth package is usually extended for this installation. Typical additions include:

- venue or brand-specific admin groups
- support roles that can move or cancel bookings without editing event definitions
- membership operators who can manage tiers and subscription state
- check-in staff with `events.booking.check_in` but limited broader access

Managed assets such as event images, downloadable passes, and member documents also participate in auth. Public hero images may be publishable, while member-only downloads remain private even if stored in object storage.

## Cache, Storage, And Delivery

Caching needs careful scoping because the installation mixes public and personalized views.

- public event pages can be cached by locale, site, and event state
- account pages, active membership panels, and booking history are private
- availability fragments may be short-lived or uncacheable depending on load and consistency requirements

Storage policy typically looks like this:

- build assets as `public_asset`
- event imagery and public editorial media as `public_upload`
- downloadable passes, exported attendee lists, and sensitive documents as `private_shared`
- exceptional on-server sensitive files as `local_only_sensitive` only when operations explicitly accept the tradeoff

## Notifications And Jobs

This installation relies heavily on background work. Core jobs, mail, and observability services are used for:

- booking confirmations and reminders
- waitlist promotion
- membership renewal notices
- pass or attachment generation
- third-party webhook delivery and retry

Those workflows belong in native modules and host services because they interact with auth, capacity, payment state, and operational tracing.

## Editorial And Admin Workflow

Editors manage pages, event descriptions, SEO metadata, and media through CMS and admin modules. Operations staff manage slots, capacity, waitlists, check-in, and cancellation handling through events and admin modules. Finance or customer support may need membership and booking visibility without full publishing rights. That is exactly the sort of differentiated access pattern the capability-driven auth model is meant to support.

## White-Label And Regional Behavior

This installation often needs region-aware and brand-aware behavior. Core therefore provides locale, timezone, site, and brand primitives, while the customer app supplies:

- templates and brand tokens
- per-brand content and messaging
- locale policy and translated copy
- region-specific extensions for eligibility, passes, or notifications

Those concerns stay in the customer app unless they prove broadly reusable enough to graduate into an official module.

## Operational Emphasis

For this vertical, launch and upgrade validation should pay particular attention to:

- membership status evaluation
- booking atomicity and capacity correctness
- waitlist promotion and cancellation behavior
- notification and pass delivery
- support and check-in tooling in admin

This is the most demanding reference installation in the platform because it exercises personalization, transactional workflows, editorial tooling, and operational administration together. If this installation upgrades cleanly, the architecture boundary is doing its job.
