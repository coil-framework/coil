# Phased Delivery Plan

## Purpose

These requirements are too broad for a single implementation pass. Coil needs a staged plan that preserves architectural coherence while delivering useful capability increments.

## Phase 1: Clarify the Editorial and Render Boundary

Goal:

- make the current platform seams explicit
- remove ambiguity around schema, content, and render-model composition

Deliverables:

- public documentation updates described in [04-platform-boundaries-and-public-documentation-gaps.md](./04-platform-boundaries-and-public-documentation-gaps.md)
- stable render-model hook APIs
- documented mount and merge behavior
- dynamic block guidance

This phase is primarily about developer correctness and reducing accidental misuse.

## Phase 2: Structured Page Instances and Shared Blocks

Goal:

- move beyond block schema only
- support real page-builder content authoring

Deliverables:

- structured page-instance storage
- shared and reusable block model
- page settings model
- publication and preview support for structured pages
- admin authoring UI for add, reorder, enable, disable, duplicate, and reference

This is the first phase that closes the biggest editorial gap.

## Phase 3: Dynamic Blocks and Live Content Integration

Goal:

- support mixed editorial and live-data pages without hidden framework behavior

Deliverables:

- official contract for dynamic blocks
- block fragment dispatch support as an editor-facing capability
- customer-facing examples combining page-builder content with live queries
- stable admin and preview behavior for dynamic blocks

This phase makes the CMS viable for content-heavy retail and membership-led sites.

## Phase 4: Events, Timeslots, Bookings, and Passes

Goal:

- establish the first-class transactional event and booking stack

Deliverables:

- official events module
- timeslots with capacity management
- reservations and booking transactions
- event-pass support
- customer eligibility hooks
- event discoverability and content integration

This phase should be treated as a primary product capability, not a peripheral add-on.

## Phase 5: Memberships, Accounts, and Subscriptions

Goal:

- provide the account and entitlement model required by the booking and content systems

Deliverables:

- profile completeness model
- preferences and consent model
- free and paid tiers
- subscription lifecycle handling
- payment method update flows
- customer-facing account surfaces
- admin-facing membership visibility

This phase enables audience-aware and entitlement-aware experiences across the platform.

## Phase 6: Admin Operations and Reporting

Goal:

- make Coil operationally usable for real teams

Deliverables:

- bookings search and detail screens
- event operations tools
- bulk actions
- exports
- audit trail
- reports
- operator-facing notifications

Without this phase, the platform may render the site correctly but still fail to replace the working operating model.

## Phase 7: Webhooks, Jobs, and Integrations

Goal:

- complete the operational perimeter

Deliverables:

- webhook ingestion and verification
- scheduled reminder workflows
- integration settings surfaces
- connector contracts for external systems
- retry and idempotency rules
- operational observability for background and integration work

This phase is necessary for production parity with legacy business workflows.

## Delivery Guidance

- Do not try to hide missing phases behind customer-specific glue if the same capability will be needed repeatedly.
- Do not move customer-specific business policies into core merely because one customer currently depends on them.
- Prefer official-module capability when the workflow is repeatable across multi-site retail, membership, and event-driven businesses.
- Keep namespacing and extension points explicit so customer code does not become another hidden framework layer.

## Recommended Review Order

1. approve the capability boundaries in [01-composable-page-builder-and-editorial-model.md](./01-composable-page-builder-and-editorial-model.md)
2. approve the transactional domain in [02-events-bookings-memberships-and-passes.md](./02-events-bookings-memberships-and-passes.md)
3. approve the operator and integration model in [03-admin-operations-and-integrations.md](./03-admin-operations-and-integrations.md)
4. approve the public-docs correction plan in [04-platform-boundaries-and-public-documentation-gaps.md](./04-platform-boundaries-and-public-documentation-gaps.md)
5. refine delivery priorities and dependencies from there
