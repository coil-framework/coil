# Platform Requirements for Editorial, Booking, and Membership-Led Retail Sites

These documents capture the product requirements needed for Coil to support modern retail and membership-led businesses that combine editorial flexibility, live event booking, customer accounts, subscriptions, and operational tooling in one platform.

They are written as product requirements, not implementation detail notes. The goal is to describe what Coil must support, where the platform boundaries should sit, and which capabilities belong in core, in official modules, and in customer-owned code.

## Document Map

- [01-composable-page-builder-and-editorial-model.md](./01-composable-page-builder-and-editorial-model.md)
  Covers the content model, reusable blocks, page settings, global settings, rendering contracts, and editorial workflows.
- [02-events-bookings-memberships-and-passes.md](./02-events-bookings-memberships-and-passes.md)
  Covers the transactional domain for events, timeslots, reservations, bookings, subscriptions, account state, and event passes.
- [03-admin-operations-and-integrations.md](./03-admin-operations-and-integrations.md)
  Covers the admin shell, reporting, bulk actions, jobs, webhooks, notifications, and external system integration surfaces.
- [04-platform-boundaries-and-public-documentation-gaps.md](./04-platform-boundaries-and-public-documentation-gaps.md)
  Explains the platform seams that are currently easy to misunderstand and proposes the public documentation changes needed to make those seams clear.
- [05-phased-delivery-plan.md](./05-phased-delivery-plan.md)
  Breaks the work into staged platform deliveries so the capability set can be built without losing coherence.
- [06-getting-started-as-an-end-to-end-product-tutorial.md](./06-getting-started-as-an-end-to-end-product-tutorial.md)
  Defines the documentation strategy for turning Getting Started into a full step-by-step build of a realistic retail, events, and memberships site.
- [07-implementation-program.md](./07-implementation-program.md)
  Breaks the requirement set into executable epics, workstreams, task slices, and acceptance criteria.

## Cross-Cutting Principles

- Editorial flexibility must remain a first-class capability rather than a one-off customer customization.
- Structured content and structured business data must be modeled separately even when they render together on the same page.
- Dynamic rendering must be supported through explicit contracts rather than hidden global state.
- Operational tooling is part of the product, not an implementation afterthought.
- Customer code should extend the platform through clear APIs, not by taking over framework-owned contracts implicitly.

## Intended Use

These documents are for:

- product and architecture review
- scoping Coil module boundaries
- planning migration support from legacy CMS-driven retail stacks
- identifying missing official-module capabilities
- clarifying what public documentation must explain more concretely
