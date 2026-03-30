---
title: Add Bookings, Reservations, and Validation
---

This chapter turns the event pages into the first honest booking surface in the tutorial app.
The goal is not to fake a checkout form. The goal is to make the event detail page and account
area express the states a real booking system needs to carry:

- a timeslot can still accept a reservation
- a timeslot can be waitlist-only
- the customer account must surface confirmed bookings, held reservations, and waitlist state

The checked-in Shoppr app now does that with the event detail page and the account dashboard.

## What Changes In This Chapter

Three files carry the entire customer-facing surface for this step:

- `apps/shoppr/templates/events/detail.html`
- `apps/shoppr/templates/account/dashboard.html`
- `apps/shoppr/templates/account/summary-panels.html`

The runtime provides the model values these templates read:

- `event.*`
- `event.timeslots.*`
- `account.has_event_bookings`
- `account.event_bookings_*`
- `event_bookings`

The important boundary is:

- the template decides how booking state is presented
- the runtime decides which booking or reservation state the current request exposes

## Event Detail Template

The event detail page now shows each timeslot as something the customer can act on rather than
just a descriptive schedule.

```html title="apps/shoppr/templates/events/detail.html"
<!doctype html>
<html xmlns:coil="https://coil.rs" coil:attr="lang=${locale}" lang="en-GB">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Event detail · Shoppr</title>
    <link rel="stylesheet" href="/theme/assets/site.css" coil:href="asset('theme/assets/site.css')" />
  </head>
  <body class="harbor events">
    <header class="site-header">
      <a href="/" class="brand">Shoppr</a>
      <nav coil:replace="~{navigation/primary}"></nav>
    </header>
    <main class="site-main">
      <section class="home-page events-page" coil:if="${has_event}">
        <article class="catalog-section">
          <p class="catalog-section__eyebrow" coil:text="${event.eyebrow}">Event</p>
          <h1 coil:text="${event.title}">Spring Tasting Evening</h1>
          <p coil:text="${event.summary}">
            Event summary.
          </p>
          <p>
            <strong coil:text="${event.day_label}">Thursday 11 April</strong>
            ·
            <span coil:text="${event.time_range_label}">18:30 to 20:30</span>
          </p>
          <p>
            <span coil:text="${event.venue_name}">Shoppr Townhouse</span>
            ·
            <span coil:text="${event.venue_city}">London</span>
            ·
            <span coil:text="${event.venue_mode}">In store</span>
          </p>
          <p coil:text="${event.availability_label}">
            Priority booking window open
          </p>
          <p coil:text="${event.audience_label}">
            Gold members book first
          </p>
          <p coil:text="${event.priority_note}">
            Priority note.
          </p>
          <div class="checkout-actions">
            <a class="button" href="/en-GB/events" coil:attr="href=${links.events}">Back to events</a>
            <a class="button button--secondary" href="/account" coil:attr="href=${links.account}">
              Open account
            </a>
            <a class="button button--secondary" href="/account/memberships" coil:attr="href=${links.memberships}">
              Review memberships
            </a>
          </div>
        </article>

        <article class="catalog-section">
          <p class="catalog-section__eyebrow">Timeslots</p>
          <h2>Choose the right session before booking starts</h2>
          <ol class="account-panel__list" coil:if="${event.has_timeslots}">
            <li coil:each="timeslot : ${event.timeslots}">
              <div>
                <strong coil:text="${timeslot.label}">Early tasting</strong>
                <span coil:text="${timeslot.starts_at_label}">18:30</span>
                to
                <span coil:text="${timeslot.ends_at_label}">19:15</span>
              </div>
              <p coil:text="${timeslot.availability_label}">
                4 seats remaining
              </p>
              <p coil:text="${timeslot.booking_status_label}">
                Priority reservation available
              </p>
              <p coil:text="${timeslot.audience_label}">
                Gold members
              </p>
              <p coil:text="${timeslot.capacity_note}">
                Capacity note.
              </p>
              <div class="checkout-actions">
                <a class="button" href="/account" coil:attr="href=${links.account}" coil:text="${timeslot.booking_cta_label}">
                  Reserve seat
                </a>
                <a class="button button--secondary" href="/account/memberships" coil:attr="href=${links.memberships}">
                  Check eligibility
                </a>
              </div>
            </li>
          </ol>
        </article>
      </section>

      <section class="home-page events-page" coil:unless="${has_event}">
        <article class="catalog-section">
          <p class="catalog-section__eyebrow">Event detail</p>
          <h1>Event unavailable</h1>
          <p>
            The requested event slug is not currently available for this site or locale.
          </p>
          <p>
            Missing slug:
            <strong coil:text="${missing_event_slug}">unknown-event</strong>
          </p>
          <div class="checkout-actions">
            <a class="button" href="/en-GB/events" coil:attr="href=${links.events}">Back to events</a>
            <a class="button button--secondary" href="/en-GB/shop/collections/events" coil:attr="href=${links.events_collection}">
              Browse event-linked offers
            </a>
          </div>
        </article>
      </section>
    </main>
  </body>
</html>
```

This file does four important things:

- it treats each timeslot as a booking surface, not just metadata
- it exposes reservation state with `timeslot.booking_status_label`
- it gives the user a direct next step with `timeslot.booking_cta_label`
- it keeps the surrounding event context visible so the booking decision stays grounded in the
  actual event, venue, and audience rules

In a real production flow, the primary button would submit to a booking endpoint. At this stage of
the tutorial, it routes the customer back into the account and membership surfaces that already
exist so you can see the state transitions in the same product.

## Account Dashboard

The account dashboard needs to carry booking state alongside memberships and commerce orders.
If the account page only shows product orders, the event flow remains disconnected from the rest of
the app.

```html title="apps/shoppr/templates/account/dashboard.html"
<!doctype html>
<html
  xmlns:coil="https://coil.rs"
  coil:with="page_title='Your account'"
  lang="en-GB"
  coil:attr="lang=${locale}"
>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${page_title}">Your account</title>
    <link rel="stylesheet" href="/theme/assets/site.css" coil:href="asset('theme/assets/site.css')" />
  </head>
  <body class="harbor">
    <header class="site-header">
      <a href="/" class="brand">Shoppr</a>
      <nav coil:replace="~{navigation/primary}"></nav>
    </header>
    <main class="site-main">
      <section class="account-page">
        <section class="storefront-flash" coil:if="${has_flash_messages}">
          <article class="storefront-flash__message" coil:each="message : ${flash_messages}">
            <p coil:text="${message.text}">Account session updated.</p>
          </article>
        </section>
        <div class="account-page__intro">
          <p class="account-page__eyebrow">Customer account</p>
          <h1>Your account</h1>
          <p coil:text="${account.state_summary}">
            Review your membership status, recent orders, and the quickest route back into the
            storefront when you are ready to browse again.
          </p>
          <p coil:if="${account.has_principal}">
            Signed in as <strong coil:text="${customer.display_name}">Member Live</strong>.
          </p>
          <p coil:unless="${account.has_principal}">
            This account currently follows the browser session you are using right now. Keep this
            browser active if you want the same order history and membership state to remain
            visible.
          </p>
          <p coil:if="${account.has_latest_order}">
            Latest order
            <strong coil:text="${account.latest_order_reference}">ORD-10042</strong>
            is
            <span coil:text="${account.latest_order_status}">Paid</span>.
          </p>
          <p coil:if="${account.has_latest_order}">
            If you have just returned from the payment provider, this dashboard can show
            <strong>Pending Payment</strong>
            until the settlement callback updates the same browser session.
          </p>
          <div coil:if="${account.has_customer_email}">
            <p coil:if="${account.has_principal}">
              Account email: <strong coil:text="${customer.email}">member@example.com</strong>
            </p>
            <p coil:unless="${account.has_principal}">
              Latest receipt email for this browser session:
              <strong coil:text="${customer.email}">member@example.com</strong>
            </p>
          </div>
        </div>
        <nav class="account-page__nav" coil:replace="~{account/nav :: nav}"></nav>
        <div class="account-page__cards">
          <article class="account-card">
            <h2>Memberships</h2>
            <p coil:if="${account.has_membership}">
              Check your active tiers, renewals, and entitlement status.
            </p>
            <p coil:if="${account.has_pending_membership_order}">
              Latest order
              <strong coil:text="${account.latest_order_reference}">ORD-10042</strong>
              is
              <span coil:text="${account.latest_order_status}">Pending Payment</span>.
              Membership access moves into this account area after a qualifying membership purchase
              is captured for this browser session. Returning from Stripe does not make that access
              live until settlement completes.
            </p>
            <p coil:if="${account.needs_membership_purchase}">
              No active membership is attached to this account view yet. Open the membership area
              to check qualifying orders or return to the storefront collection to join.
            </p>
            <a class="button" href="/account/memberships">View memberships</a>
            <a class="button button--secondary" href="/account/orders" coil:if="${account.has_pending_membership_order}">
              View order history
            </a>
            <a
              class="button button--secondary"
              href="/en-GB/shop/collections/memberships"
              coil:if="${account.needs_membership_purchase}"
              coil:attr="href=${account.membership_cta_url}"
            >
              Explore memberships
            </a>
          </article>
          <article class="account-card">
            <h2>Orders and storefront</h2>
            <p coil:if="${account.has_recent_orders}">
              Review payment status, receipt details, and post-checkout next steps before heading
              back into the storefront. If the latest order still says Pending Payment, wait for
              the provider callback before retrying checkout.
            </p>
            <p coil:unless="${account.has_recent_orders}" coil:text="${account.orders_empty_text}">
              Continue browsing the public catalog and landing content.
            </p>
            <a class="button" href="/account/orders">
              View order history
            </a>
            <a
              class="button button--secondary"
              href="/en-GB/shop"
              coil:attr="href=${account.orders_cta_url}"
              coil:text="${account.orders_cta_label}"
            >
              Browse storefront
            </a>
          </article>
          <article class="account-card">
            <h2>Event bookings</h2>
            <p coil:if="${account.has_event_bookings}">
              Review confirmed seats, held reservations, and waitlist movement without leaving the
              same account surface.
            </p>
            <p coil:unless="${account.has_event_bookings}" coil:text="${account.event_bookings_empty_text}">
              Event reservations and confirmed bookings will appear here once the customer starts
              booking timed experiences.
            </p>
            <a
              class="button"
              href="/en-GB/events"
              coil:attr="href=${account.event_bookings_cta_url}"
              coil:text="${account.event_bookings_cta_label}"
            >
              Browse event calendar
            </a>
            <a class="button button--secondary" href="/account/memberships">
              Check membership access
            </a>
          </article>
        </div>
        <div coil:replace="~{account/summary-panels :: panels}"></div>
      </section>
    </main>
    <footer class="site-footer">
      <small>Shoppr</small>
    </footer>
  </body>
</html>
```

The new part is the third card:

- `account.has_event_bookings` decides whether the account already has booking state to show
- `account.event_bookings_empty_text` gives the empty-state copy
- `account.event_bookings_cta_url` and `account.event_bookings_cta_label` route the customer back
  to the event calendar when there is nothing booked yet

This keeps the event journey inside the account rather than making it feel like a separate product.

## Account Summary Panels

The summary fragment now exposes the actual booking list.

```html title="apps/shoppr/templates/account/summary-panels.html"
<section class="account-panels" xmlns:coil="https://coil.rs" coil:fragment="panels">
  <div class="account-panels__grid">
    <article class="account-panel">
      <p class="account-panel__eyebrow">Orders</p>
      <h2>Recent purchases</h2>
      <ul class="account-panel__list" coil:if="${account.has_recent_orders}">
        <li coil:each="order : ${recent_orders}">
          <strong coil:text="${order.reference}">HS-1048</strong>
          <span coil:text="${order.status}">Packed</span>
          <span coil:text="${order.total}">GBP 84</span>
          <span coil:text="${order.line_count}">1</span>
          <p coil:if="${order.has_payment_summary}" coil:text="${order.payment_summary}">
            Card ending 4242, reference PAY-50001
          </p>
        </li>
      </ul>
      <p coil:unless="${account.has_recent_orders}" coil:text="${account.orders_empty_text}">
        Recent orders will appear here once the customer has completed checkout.
      </p>
      <a
        class="button button--secondary"
        href="/account/orders"
        coil:attr="href=${account.orders_cta_url}"
        coil:text="${account.orders_cta_label}"
      >
        View order history
      </a>
    </article>

    <article class="account-panel">
      <p class="account-panel__eyebrow">Membership</p>
      <h2>Access and renewals</h2>
      <div coil:if="${account.has_membership}">
        <p>
          <strong coil:text="${membership_summary.tier_name}">Harbor Circle</strong>
          <span coil:text="${membership_summary.status}">Active</span>
        </p>
        <p coil:text="${membership_summary.renewal_text}">
          Renewing on 18 April with market-day priority access.
        </p>
        <a class="button" href="/account/memberships">View membership details</a>
      </div>
      <div coil:if="${account.has_pending_membership_order}">
        <p>
          <strong coil:text="${account.latest_order_reference}">ORD-10042</strong>
          <span coil:text="${account.latest_order_status}">Pending Payment</span>
        </p>
        <p>
          Membership access appears here only after a qualifying membership purchase is captured for
          this account view. After returning from the payment provider, review order history first,
          then return here once the payment state settles for this browser session.
        </p>
        <a class="button" href="/account/memberships">
          View memberships
        </a>
        <a class="button button--secondary" href="/account/orders">
          View order history
        </a>
      </div>
      <div coil:if="${account.needs_membership_purchase}">
        <p coil:text="${account.membership_empty_text}">
          No membership is attached yet. Join to unlock early-access drops and concierge support.
        </p>
        <a
          class="button"
          href="/en-GB/shop/collections/memberships"
          coil:attr="href=${account.membership_cta_url}"
        >
          Explore memberships
        </a>
      </div>
    </article>

    <article class="account-panel">
      <p class="account-panel__eyebrow">Support</p>
      <h2>Continue your account journey</h2>
      <p coil:if="${account.has_customer_email}">
        Use this account area to review orders, retry checkout, or check membership access for
        <strong coil:text="${customer.email}">member@example.com</strong>.
      </p>
      <p coil:if="${account.has_latest_order}">
        If the latest order still shows
        <strong>Pending Payment</strong>
        after you return from the provider, keep this browser session and refresh the account
        surfaces before contacting support.
      </p>
      <p coil:if="${account.has_principal}">
        This browser is already attached to a live customer identity for this account view.
      </p>
      <p coil:unless="${account.has_principal}">
        This customer account currently follows the browser session you are using. Keep this
        browser active if you need to revisit the same order history or membership state. Orders
        and membership changes from another browser will not appear here yet.
      </p>
      <ul class="account-panel__list">
        <li><a href="/account/orders">Review order history</a></li>
        <li><a href="/account/memberships">Check membership status</a></li>
        <li><a href="/checkout">Open checkout</a></li>
        <li><a href="/en-GB/shop" coil:attr="href=${links.catalog}">Continue shopping</a></li>
      </ul>
    </article>

    <article class="account-panel">
      <p class="account-panel__eyebrow">Event bookings</p>
      <h2>Timed experiences and reservation state</h2>
      <ul class="account-panel__list" coil:if="${account.has_event_bookings}">
        <li coil:each="booking : ${event_bookings}">
          <a href="/en-GB/events/spring-tasting" coil:attr="href=${booking.href}">
            <strong coil:text="${booking.event_title}">Spring Tasting Evening</strong>
          </a>
          <span coil:text="${booking.status_label}">Confirmed</span>
          <span coil:text="${booking.day_label}">Thursday 11 April</span>
          <p>
            <strong coil:text="${booking.slot_label}">Early tasting</strong>
          </p>
          <p coil:text="${booking.summary}">
            Seat confirmed. Arrive ten minutes early for the guided tasting.
          </p>
        </li>
      </ul>
      <div coil:unless="${account.has_event_bookings}">
        <p coil:text="${account.event_bookings_empty_text}">
          Event reservations and confirmed bookings will appear here once the customer starts booking
          timed experiences.
        </p>
        <a
          class="button"
          href="/en-GB/events"
          coil:attr="href=${account.event_bookings_cta_url}"
          coil:text="${account.event_bookings_cta_label}"
        >
          Browse event calendar
        </a>
      </div>
    </article>
  </div>
</section>
```

This fragment is where reservation state becomes concrete:

- `event_bookings` is a list, not a single flag
- each booking carries event, slot, status, date, and summary
- the account page can now show confirmed, held, or waitlisted event progress the same way it shows
  order history

That is the key conceptual step in this chapter. A booking is not just a button click on an event
page. It becomes customer-visible state that must be revisitable in the account area.

## What The Runtime Is Providing

For this chapter, the runtime now exposes:

- `event.timeslots[].booking_status_label`
- `event.timeslots[].booking_cta_label`
- `account.has_event_bookings`
- `account.event_bookings_empty_text`
- `account.event_bookings_cta_url`
- `account.event_bookings_cta_label`
- `event_bookings[]`

Those values are enough to demonstrate the booking seam honestly:

- event detail pages can tell the customer whether a slot is reservable or waitlist-only
- the account can reflect that booking state back to the customer
- memberships still remain part of the eligibility story rather than a separate subsystem

## Runnable Checkpoint

Run the app:

```bash
docker compose up
```

Then verify:

1. Open `/en-GB/events/spring-tasting`.
   The page should show:
   - `Priority reservation available`
   - `Reserve seat`
   - `Check eligibility`

2. Open `/account`.
   The dashboard should show:
   - `Event bookings`
   - `Browse event calendar`

3. Open the same account surface after exercising the existing membership flow.
   The account panels should show event-booking state alongside membership and order state.

At this point the tutorial app has a coherent booking surface:

- the event page explains the timeslot and reservation state
- the account page explains the resulting booking state
- memberships still gate eligibility where needed

The next chapter will build on that surface by adding the entitlement layer that sits between
memberships and one-off event access.
