---
title: Events Module
---

The events module owns event catalog pages, bookings, waitlists, reminders, and check-in.

Primary implementation files:

- `crates/coil-events/src/module/platform/manifest.rs`
- `crates/coil-events/src/module/platform/surfaces.rs`
- `crates/coil-events/src/module/platform/operations.rs`

## Why It Exists

Events need public discovery, booking workflows, capacity rules, reminders, and operator check-in.
Those are reusable product capabilities, not one customer's custom page set.

## What It Provides

The events module adds:

- migrations for event catalog, slots, and bookings
- public routes for `/events` and `/events/{event_slug}`
- booking action route `/events/{event_slug}/book`
- admin routes for event management, booking review, and check-in
- scheduled jobs for reservation expiry and reminders
- domain-event jobs for waitlist promotion

## How To Enable It

```toml title="app.toml"
[modules]
enabled = ["events"]
```

In a commerce-heavy app, events often sit alongside commerce and memberships. Shoppr uses that
broader combination.

## How To Disable It

Remove `events` from the enabled lists and remove or replace customer templates, navigation, and
admin surfaces that assume event routes exist.

## Config Expectations

Like CMS and memberships, the current events module relies mostly on shared platform config:

- database
- jobs
- i18n
- SEO
- templates
- auth

## Routes And Surfaces

Important routes from `surfaces.rs`:

- `/events`
- `/events/{event_slug}`
- `/events/{event_slug}/book`
- `/admin/events`
- `/admin/events/bookings`
- `/admin/events/check-in`

## Required Auth Capabilities

The exact capability set is declared in `crates/coil-events/src/module/platform/capabilities.rs`,
and the public/admin routes are gated by event publish, booking create, and booking check-in
capabilities.

## How Customer Apps Extend It

Events exposes:

- admin widget slot: `events.booking.summary`
- render hook slot: `events.page.render`

Customer apps usually extend events by:

- changing the public event templates
- integrating event booking with commerce or memberships
- adding customer widgets around booking and attendance views

Concrete example:

```html title="templates/events/event-detail.html"
<form method="post" coil:attr="action=${event.booking_action}">
  <input type="hidden" name="event_id" coil:attr="value=${event.id}" />
  <button type="submit">Reserve a place</button>
</form>
```

The events module still owns booking lifecycle, waitlist promotion, and reminders. The customer app
owns the public event presentation and the surrounding product story.

The practical sequence is:

1. enable `events`
2. provide public and admin event templates
3. connect events to commerce or memberships if bookings need payment or entitlement logic
4. use linked Rust or bounded render hooks for customer-specific booking policy and presentation

## Where To See It

Shoppr is the main example because it enables `events` alongside commerce and memberships in
`apps/shoppr/app.toml`.

## Common Mistakes

- Treating bookings as static forms instead of a lifecycle with capacity and waitlist behaviour.
- Forgetting that reminders and reservation expiry are scheduled jobs.
- Skipping auth capability design for check-in surfaces.

## Read Next

- [Memberships](./memberships/)
- [Ops](./ops/)
