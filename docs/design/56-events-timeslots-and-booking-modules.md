# Events, Timeslots, and Booking Modules

**Part:** Native Batteries  
**Chapter:** 56

Events, timeslots, and bookings are a first-party native vertical because they are central to the reference customer and require strong coordination across content, commerce, memberships, auth, and operations. The platform should treat them as supported modules, not as one-off app code, while still keeping them outside core.

## Domain Model
The event stack should distinguish at least these concerns:

- event content and discoverability
- scheduled timeslots or sessions with capacity rules
- reservations and booking state
- waitlists and cancellation handling
- check-in and operator workflows

This split matters because the public event page, the seat-capacity rules, and the back-office check-in tools all evolve at different rates. The event catalog can integrate closely with CMS and SEO. The booking workflow needs transactional correctness and background work. The check-in flow belongs in the admin shell and often depends on staff-specific capabilities.

## Integration with Other Modules
Events sit on top of shared platform primitives:

- CMS provides content, routing, and publishing workflows for event pages.
- Commerce provides payment, checkout, and order integration when bookings are paid.
- Membership modules influence eligibility, pricing, or access to specific timeslots.
- Auth capabilities govern who may create, manage, cancel, or check in bookings.
- Jobs and notifications handle reminders, confirmations, cancellations, and waitlist promotion.

This layered design keeps event logic reusable without forcing every customer app to adopt it.

## Operational Expectations
Bookings are concurrency-sensitive. Capacity, reservation expiry, and cancellation workflows should be modeled explicitly and executed through native transactional code. Bulk staff screens such as check-in must rely on batched authorization and searchable admin resources rather than per-row ad hoc logic.

For the reference customer, this module family is not a sidecar. It is one of the core reasons the product exists. That is why it ships as a supported native battery rather than as a thin extension example.
