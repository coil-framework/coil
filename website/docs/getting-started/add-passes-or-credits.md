---
title: Add Passes or Credits
---

This chapter adds finite entitlements that sit between recurring memberships and one-off
bookings.

For Shoppr, this is the slice that answers three practical questions:

- which customers still have redeemable pass-backed balance
- which bookings or events already consumed that balance
- which operator queue should handle expiry, redemption mismatch, or manual adjustment work

Do not invent a separate product architecture for passes. Add them to the same customer account
and operator loops the app already uses for memberships, orders, and event bookings.

## Start From The Existing Shoppr Surfaces

These are the first files to extend:

- `apps/shoppr/templates/account/nav.html`
  This is where a new customer account area becomes discoverable.
- `apps/shoppr/templates/account/dashboard.html`
  This is the existing summary-card hub for orders, memberships, and bookings.
- `apps/shoppr/templates/account/summary-panels.html`
  This is the reusable account-detail strip where balance summaries belong.
- `apps/shoppr/templates/account/passes.html`
  This is the dedicated customer-facing pass and credit page.
- `apps/shoppr/templates/memberships/account.html`
  This is where the recurring-membership story and the finite pass story need to meet without
  being conflated.
- `apps/shoppr/templates/admin/nav.html`
  This is where operator navigation grows.
- `apps/shoppr/templates/admin/dashboard.html`
  This is the control-room surface where new operator queues first surface.
- `apps/shoppr/templates/memberships/passes.html`
  This is the dedicated operator lane for pass-backed customers.
- `crates/coil-memberships/src/module/manifest.rs`
  This is where the new account and admin routes become real module surfaces.
- `crates/coil-runtime/src/render/model.rs`
  This is where the customer and operator pass projections are shaped from live storefront state.

The important design rule is simple: passes and credits should fit into the same account and admin
loops instead of becoming a one-off microsystem.

## The Smallest Useful Shoppr Slice

The first coherent implementation is not a full entitlement engine. It is:

- one customer-facing passes and credits page
- one operator-facing passes and credits page
- summary cards in the existing account and admin dashboards

That already teaches the product shape before you wire capture, redemption, and expiry logic in
Rust.

## Customer Model To Bind

The checked-in Shoppr slice stays narrower than a full entitlement engine. The current customer
page binds:

```json
{
  "account": {
    "has_pass_programs": true,
    "pass_programs_empty_text": "Pass-backed access and remaining credits will appear here once the customer completes an event-pass checkout.",
    "passes_cta_url": "/en-GB/shop/collections/events",
    "passes_cta_label": "Browse event passes"
  },
  "pass_wallet": {
    "available": "1",
    "pending": "0",
    "has_pending": false,
    "summary": "1 pass currently available for event-linked bookings."
  },
  "pass_programs": [
    {
      "title": "Spring Tasting Pass",
      "sku": "tasting-pass",
      "state_label": "Available",
      "balance_label": "1 pass available",
      "usage_summary": "Use this pass when reserving tasting sessions or member-priority event slots.",
      "product_href": "/en-GB/shop/products/tasting-pass",
      "order_href": "/account/orders"
    }
  ]
}
```

That is enough to render:

- whether the customer has any pass-backed access at all
- the current wallet summary
- each purchased pass product and the route back to the originating order

## Operator Model To Bind

The checked-in operator lane is also narrow on purpose:

```json
{
  "membership_pass_stats": {
    "total": "1",
    "available": "1",
    "pending": "0",
    "follow_up": "0"
  },
  "has_membership_pass_rows": true,
  "membership_pass_rows": [
    {
      "display_name": "Alex Mariner",
      "has_customer_email": true,
      "customer_email": "alex@example.com",
      "pass_state_label": "Passes available",
      "pass_summary": "1 pass available across Spring Tasting Pass.",
      "pass_titles": "Spring Tasting Pass",
      "pass_count": "1",
      "support_state_label": "Needs review",
      "latest_order_reference": "ORD-10042",
      "latest_order_status": "Paid",
      "latest_order_total": "£45.00",
      "latest_order_href": "/admin/orders/ORD-10042"
    }
  ]
}
```

This gives operators a single place to inspect:

- current pass-backed access
- which pass titles a customer actually bought
- support state
- drill-through into the latest order

## Files To Add Or Update

In Shoppr, the practical tutorial slice maps to files like these:

```text
apps/shoppr/templates/account/nav.html
apps/shoppr/templates/account/dashboard.html
apps/shoppr/templates/account/summary-panels.html
apps/shoppr/templates/account/passes.html
apps/shoppr/templates/memberships/account.html
apps/shoppr/templates/admin/nav.html
apps/shoppr/templates/admin/dashboard.html
apps/shoppr/templates/memberships/passes.html
crates/coil-memberships/src/module/manifest.rs
crates/coil-runtime/src/render/model.rs
```

That keeps the chapter grounded in the same app shell a reader can open locally.

## Where The Booking Rule Lives

The template should present balance and redemption state. It should not decide whether a booking
may consume a credit.

That rule still belongs in customer-owned Rust:

```rust
pub fn validate_booking_with_pass(
    customer_id: &str,
    slot_id: &str,
) -> Result<BookingEntitlementDecision, BackendError> {
    // 1. Load the customer pass or credit balance.
    // 2. Check whether the selected slot can consume that entitlement.
    // 3. Return an explicit decision for the booking flow.
}
```

What this function owns:

- checking whether the customer has a valid pass
- checking whether the pass is compatible with the requested booking
- deciding whether the flow should consume one credit or reject the booking

## What This Chapter Should Prove

- passes use the same account and admin seams as memberships
- finite entitlement balance belongs next to bookings and events, not in an isolated subsystem
- customer account pages explain remaining balance and redeemed usage
- operator pages explain follow-up queues, expiry, and drill-through into orders/customers
- templates present pass state, but linked customer Rust still decides entitlement validity

## Runnable Checkpoint

Run:

```bash
cd apps/shoppr
docker compose up --build
```

Verify:

1. `/account`
   The overview shows a pass or credit summary card.
2. `/account/passes`
   The customer can see active pass programs, remaining credits, and redeemed events.
3. `/account/memberships`
   Membership access and pass-backed access are explained as complementary, not the same thing.
4. `/admin`
   The operator dashboard links into pass and credit work.
5. `/admin/memberships/passes`
   An operator can inspect pass-backed customer records, purchased pass titles, follow-up state,
   and the latest order that established the entitlement.
