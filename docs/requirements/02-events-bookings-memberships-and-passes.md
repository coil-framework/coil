# Events, Bookings, Memberships, and Passes

## Purpose

Coil must support sites where event discovery, booking, memberships, subscriptions, and passes are part of the core business model rather than a plugin attached to a content site. These capabilities should be first-class official modules with strong transactional guarantees and clear integration points.

## Capability Summary

The platform must support:

- events and venues
- timeslots or sessions
- capacity-aware reservations
- bookings and cancellations
- waitlists
- check-in and attendance workflows
- event passes or booking credits
- subscriptions and tier-based eligibility
- customer account state and profile completeness
- region, brand, and audience-aware visibility

## Domain Model

The domain model should at minimum distinguish:

- event content
- venue data
- event sessions or timeslots
- reservations
- bookings
- booking attendance state
- event passes
- subscriptions
- membership tiers
- customer visibility and eligibility state

These concerns change at different rates and must not be flattened into a single content type.

## Events and Timeslots

Events need:

- editorial content and SEO metadata
- associated brands or categories
- venue assignment
- online, in-store, virtual, and collection modes
- visibility and audience restrictions
- featured state
- related-events support

Timeslots need:

- start and end time
- timezone handling
- capacity
- tier restrictions
- virtual flags
- parent event linkage
- operational status and derived availability

The event page and the timeslot capacity system are related but not the same subsystem.

## Reservation and Booking Flow

The platform must model booking in explicit stages:

1. discover event
2. inspect eligible timeslots
3. create reservation or hold if required
4. validate user eligibility
5. validate pass or payment state
6. create booking transactionally
7. trigger follow-up notifications and state updates

This flow must be concurrency-safe. Capacity, reservations, and cancellations require native transactional behavior.

## Eligibility and Access

Booking eligibility must be able to depend on:

- authentication state
- customer profile completeness
- membership tier
- event visibility rules
- restricted email or audience lists
- event-pass ownership
- region or brand context

This is not just route protection. It is domain-level authorization and validation.

## Event Passes

The platform must support event-pass style access products that are separate from ordinary membership.

Capabilities needed:

- pass definitions and pricing
- pass ownership
- pass tier compatibility
- pass redemption
- pass status
- integration with booking validation
- event-specific pass overrides

These passes may be sold or granted and may coexist with subscription tiers.

## Memberships and Subscriptions

Membership and subscription flows need:

- free and paid tiers
- subscription lifecycle state
- complimentary subscriptions
- renewals
- cancellation and resume flows
- payment method update flows
- scheduled subscription reminders
- user-facing account views
- staff-facing visibility into current and previous state

The platform should treat membership state as shared platform data that can influence content, event visibility, pricing, and admin tooling.

## Customer Profile and Preferences

Customer accounts need:

- structured profile fields
- profile completeness checks
- consent and preference management
- marketing preferences
- region and address data
- stored delivery or collection information
- auditability for sensitive changes

Some of this belongs in shared account modules. Some belongs in customer-owned extensions. The boundaries must stay explicit.

## White-Label, Region, and Audience Variants

Many retail businesses operate under multiple brand, region, or audience contexts. Coil needs support for:

- region-aware customer journeys
- region-specific integrations and payment settings
- partner or white-label branding
- brand-specific consent or messaging
- content and event visibility scoped by audience or brand

This cannot be solved by template theming alone. The domain and admin model need to understand it.

## Operational Requirements

The event and booking stack must support:

- booking confirmation emails
- cancellation emails
- reminder emails
- attendance confirmation
- check-in operations
- booking exports
- audit trail for operational changes
- customer self-service where allowed
- admin override flows where required

## Required Official Module Boundaries

The recommended split is:

- `coil-events`
  owns events, venues, timeslots, discoverability, and visibility rules
- `coil-memberships`
  owns subscriptions, tiers, account status, and renewal state
- `coil-commerce`
  owns payment and checkout primitives where booking or pass purchase is paid
- `coil-admin`
  owns operational back-office screens
- `coil-jobs`
  owns reminders, follow-up tasks, and scheduled workflows
- customer linked Rust
  owns customer-specific policy and integration details that are not widely reusable

## Immediate Implications for Coil

Coil should not frame this area as a small add-on module. It is one of the core reasons the platform exists for this class of customer. That means:

- transaction safety matters
- admin workflows matter
- dynamic render-model integration matters
- jobs and webhooks matter

If these capabilities remain partially implicit or scattered across customer code, the platform will not reproduce the product shape it is supposed to enable.
