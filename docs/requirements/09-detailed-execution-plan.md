# Detailed Execution Plan for Requirements 01 Through 06

## Purpose

Documents 01 through 06 define the target product, but they are still too high-level to manage day
to day implementation. This document is the active execution backlog for finishing that requirement
set.

The standard is strict:

- a requirement is not done because an API exists
- a workstream is not done because a demo page exists
- a tutorial chapter is not done because a concept is mentioned

A slice is done only when:

1. the platform capability exists in code
2. Shoppr uses it in a realistic flow
3. the getting-started guide teaches it with the actual files and code involved
4. tests prove the capability behaves correctly

## Working Rules

1. Keep the requirements honest. Do not describe a capability as complete unless the code, demo,
   tutorial, and verification all exist.
2. Build vertical slices. Prefer finishing one end-to-end capability over touching multiple systems
   without closing the loop.
3. Keep Shoppr aligned to the tutorial. The demo is not marketing collateral. It is the canonical
   proof that the platform shape is real.
4. Keep reference docs stable and modular. Use the getting-started tutorial as the narrative bridge.
5. Make the platform seams explicit. When a capability depends on customer code, the tutorial must
   show exactly where that code lives and why.

## Current State Summary

Current status against the requirement set:

- 01 is partial
- 02 is partial
- 03 is partial
- 04 is improved but incomplete
- 05 exists as planning intent but needs to stay coupled to real execution status
- 06 is materially incomplete

This backlog exists to close that gap.

## Requirement Coverage Matrix

### Requirement 01: Composable page builder and editorial model

Covered by workstreams:

- 1. Editorial operating model completion
- 2. Dynamic blocks and render-model composition
- 8. Admin resources for editorial and operations
- 12. Tutorial completion and reference alignment

### Requirement 02: Events, bookings, memberships, and passes

Covered by workstreams:

- 4. Memberships and audience gating
- 5. Events and timeslots
- 6. Bookings, reservations, and validation
- 7. Passes or credits
- 8. Admin resources for editorial and operations
- 9. Reproducible integration
- 10. Jobs, notifications, and scheduled work

### Requirement 03: Admin, operations, and integrations

Covered by workstreams:

- 6. Bookings, reservations, and validation
- 8. Admin resources for editorial and operations
- 9. Reproducible integration
- 10. Jobs, notifications, and scheduled work
- 11. Observability and production preparation

### Requirement 04: Platform boundaries and public documentation gaps

Covered by workstreams:

- 1. Editorial operating model completion
- 2. Dynamic blocks and render-model composition
- 12. Tutorial completion and reference alignment

### Requirement 05: Phased delivery plan

Covered by this execution backlog and by:

- [07-implementation-program.md](./07-implementation-program.md)
- [08-gap-audit-and-realignment.md](./08-gap-audit-and-realignment.md)

### Requirement 06: Getting started as an end-to-end product tutorial

Covered by:

- the chapter plan in this document
- the chapter obligations attached to every workstream
- the tutorial completion workstream

## Dependency Order

Execution order for the full program:

1. Editorial operating model completion
2. Dynamic blocks and render-model composition
3. Discovery model and browse journeys
4. Authentication and customer accounts
5. Memberships and audience gating
6. Events and timeslots
7. Bookings, reservations, and validation
8. Passes or credits
9. Admin resources for editorial and operations
10. Reproducible integration
11. Jobs, notifications, and scheduled work
12. Observability and production preparation
13. Tutorial completion and reference alignment

This order is not arbitrary. Each later workstream assumes earlier seams are explicit and usable.

## Tutorial Chapter Plan

Requirement 06 defines twenty tutorial chapters. They are part of the backlog, not an appendix.

### Chapter Status Legend

- `done`
- `partial`
- `not_started`

### Chapter 1: What You Are Building

- status: `partial`
- goal: explain the final product shape and why the tutorial is intentionally ambitious
- missing:
  - stronger explanation of the target retail plus memberships plus events shape
  - clearer statement that the tutorial is building one coherent product rather than separate demos

### Chapter 2: Create the Project

- status: `partial`
- goal: explain project generation, workspace layout, dev flow, and first boot
- missing:
  - full generated file contents with guided explanation
  - clearer separation of generated defaults versus later edits

### Chapter 3: Understand the Runtime Shape

- status: `partial`
- goal: explain `app.toml`, `platform.dev.toml`, sites, locales, modules, and customer crates
- missing:
  - full file walkthroughs with responsibility-based explanation
  - clearer explanation of config versus content versus render-time state

### Chapter 4: Build the Base Theme

- status: `partial`
- goal: explain layouts, fragments, assets, tokens, and shell rendering
- missing:
  - full file contents for the initial theme files
  - more explicit explanation of template ownership and composition

### Chapter 5: Add Sites, Markets, and Locales

- status: `partial`
- goal: explain multi-site and localized routing
- missing:
  - concrete code-first walkthrough using actual generated files
  - checkpoint with visible market and locale changes

### Chapter 6: Add a Real Content Model

- status: `partial`
- goal: explain schema, page instances, page settings, and global options
- missing:
  - actual authored page-instance flow in the tutorial
  - full file and admin walkthrough of schema versus instance boundaries

### Chapter 7: Build Reusable Blocks

- status: `partial`
- goal: explain shared blocks, ordering, enable/disable, duplication, and preview
- missing:
  - concrete admin workflow walkthrough with actual files and state changes
  - shared-block usage and preview guidance

### Chapter 8: Add Dynamic Blocks

- status: `partial`
- goal: explain render-model hooks, mount versus merge, and live-data sections
- missing:
  - end-to-end example showing schema, instance, hook, model, fragment, and output together

### Chapter 9: Model Brands, Categories, and Discovery

- status: `partial`
- goal: explain discovery data model and browse surfaces
- missing:
  - alignment with final Shoppr discovery story
  - stronger route and model explanation tied to concrete files

### Chapter 10: Add Authentication and Customer Accounts

- status: `partial`
- goal: explain sign-in, sessions, account pages, and profile state
- missing:
  - fuller walkthrough of actual templates and account data shape
  - clearer explanation of official-module behavior versus customer code

### Chapter 11: Add Memberships and Audience Gating

- status: `not_started`
- goal: explain entitlement-aware rendering and gated content

### Chapter 12: Add Events and Timeslots

- status: `not_started`
- goal: explain event content, venues, timeslots, visibility, and browse/detail pages

### Chapter 13: Add Bookings, Reservations, and Validation

- status: `not_started`
- goal: explain the booking lifecycle and its transactional boundary

### Chapter 14: Add Passes or Credits

- status: `not_started`
- goal: explain pass-backed access and how it differs from membership

### Chapter 15: Add Admin Resources

- status: `not_started`
- goal: explain operator-facing resources beyond CMS page editing

### Chapter 16: Add One Reproducible Integration

- status: `not_started`
- goal: explain one local, reproducible external integration end to end

### Chapter 17: Add Jobs, Notifications, and Scheduled Work

- status: `not_started`
- goal: explain deferred work as part of the product flow

### Chapter 18: Add Observability and Troubleshooting

- status: `not_started`
- goal: explain debugging, readiness, health, and common operational failure modes

### Chapter 19: Prepare for Production

- status: `not_started`
- goal: explain secrets, topology, migrations, assets, and deployment concerns

### Chapter 20: Where to Go Next

- status: `not_started`
- goal: map the finished tutorial back to the stable reference surface

## Workstream 1: Editorial Operating Model Completion

### Status

`partial`

### Requirement Coverage

- 01
- 04
- 05
- 06 chapters 6 through 8

### Dependencies

- existing CMS foundations
- existing render-model hook API

### Framework Tasks

1. Finish global options as a first-class editor-facing model.
2. Finish page-level settings and targeting:
   - layout variants
   - header and footer visibility
   - body classes
   - locale and site targeting
   - audience targeting hooks
   - redirect or upgrade behavior for gated pages
3. Finish page-builder authoring operations:
   - add
   - remove
   - reorder
   - enable and disable
   - duplicate
   - insert shared block reference
4. Support nested and repeatable block field groups with validation.
5. Add preview semantics for:
   - structured page drafts
   - shared blocks
   - scheduled pages
6. Add shared-block usage inspection so editors can see where a shared block is referenced.

### Shoppr Tasks

1. Use global options in the public storefront shell.
2. Use page settings to change layout or visibility on at least two pages.
3. Use shared blocks on multiple public pages.
4. Demonstrate disabled and duplicated blocks visibly in CMS admin and live rendering.

### Getting Started Tasks

1. Rewrite chapter 6 with full file contents and a responsibility-based explanation.
2. Rewrite chapter 7 around a real editor workflow:
   - create blocks
   - reorder blocks
   - disable a block
   - duplicate a block
   - create and insert a shared block
3. Rewrite chapter 8 around a real dynamic block contract.

### Verification

1. Model tests for schema and page-instance invariants.
2. Runtime tests for page-instance rendering, targeting, and preview.
3. Admin mutation tests for block and shared-block operations.
4. Tutorial checkpoint verification in the generated or checked-in demo.

### Definition of Done

- editors can compose, preview, schedule, and publish structured pages
- global options affect public rendering
- shared blocks are inspectable and reusable
- chapters 6 through 8 are concrete, runnable, and file-complete

## Workstream 2: Dynamic Blocks and Render-Model Composition

### Status

`partial`

### Requirement Coverage

- 01
- 04
- 05
- 06 chapter 8

### Dependencies

- Workstream 1

### Framework Tasks

1. Finalize the public dynamic block contract:
   - schema-owned configuration
   - request-time shaping
   - fragment dispatch
   - mounted or merged render model
2. Ensure block fragment dispatch works for:
   - static blocks
   - dynamic blocks
   - shared block references
3. Add clearer render-model composition around:
   - framework-owned models
   - customer namespaces
   - fail-closed merge behavior
4. Ensure docs and examples use the public hook API rather than runtime internals.

### Shoppr Tasks

1. Use dynamic blocks for real discovery or membership-aware sections.
2. Show at least one page where editorial config and live runtime data combine.

### Getting Started Tasks

1. Make chapter 8 a full end-to-end example:
   - schema
   - content instance
   - linked Rust hook
   - rendered template

### Verification

1. Template and runtime tests for fragment dispatch.
2. Hook tests for mount and merge behavior.
3. Shoppr route or page tests proving mixed static and dynamic block rendering.

### Definition of Done

- dynamic blocks are a first-class documented capability
- a developer can follow chapter 8 without guessing where the data handoff occurs

## Workstream 3: Discovery Model and Browse Journeys

### Status

`partial`

### Requirement Coverage

- 01
- 02
- 04
- 06 chapter 9

### Dependencies

- Workstream 1
- Workstream 2

### Framework Tasks

1. Expose stable browse-oriented route models for:
   - collections
   - brands
   - categories
   - discovery hubs
   - search and filtering
2. Clarify the difference between CMS page content and catalog or taxonomy data.
3. Ensure links and route data remain site- and locale-aware.

### Shoppr Tasks

1. Build brand and category browse journeys that read as discovery, not just catalog listing.
2. Add at least one landing surface that combines editorial content with browse data.

### Getting Started Tasks

1. Rewrite chapter 9 so it includes:
   - the actual model file changes
   - the relevant templates
   - the route-aware links
   - the final checkpoint

### Verification

1. Runtime model tests for discovery routes.
2. Template tests or route tests for brand/category/discovery rendering.

### Definition of Done

- the public storefront has real browse journeys
- chapter 9 is code-first and aligned to the live demo

## Workstream 4: Authentication and Customer Accounts

### Status

`partial`

### Requirement Coverage

- 02
- 04
- 06 chapter 10

### Dependencies

- Workstream 3

### Framework Tasks

1. Clarify account render-model surfaces and signed-in state contracts.
2. Expose profile completeness and structured account summary data where missing.
3. Keep auth/session behavior explicit in the tutorial and reference docs.

### Shoppr Tasks

1. Make sign-in, sign-up, sign-out, and account pages a coherent journey.
2. Show visible account-state rendering in the public UI.
3. Surface profile completeness or missing-profile guidance.

### Getting Started Tasks

1. Rewrite chapter 10 with full templates, account routes, and model explanation.
2. Show exactly which parts come from official modules versus customer code.

### Verification

1. Account and auth route tests.
2. Render-model tests for signed-in account surfaces.

### Definition of Done

- the demo has a coherent account story
- chapter 10 is concrete and runnable

## Workstream 5: Memberships and Audience Gating

### Status

`not_started`

### Requirement Coverage

- 01
- 02
- 04
- 06 chapter 11

### Dependencies

- Workstream 4

### Framework Tasks

1. Model entitlement and profile-completeness state clearly in official surfaces.
2. Add page and route gating contracts that can depend on:
   - auth
   - membership tier
   - audience state
   - profile completeness
3. Add membership-aware render-model values for:
   - CTA states
   - gated page explanation
   - account panels
   - upgrade prompts

### Shoppr Tasks

1. Add members-only and tier-aware pages.
2. Add visible upgrade or unlock prompts.
3. Show different public and account experiences by entitlement state.

### Getting Started Tasks

1. Write chapter 11 as a code-first walkthrough that includes:
   - membership state
   - gating rules
   - template changes
   - checkpoint behavior

### Verification

1. Route access tests.
2. Audience-aware render-model tests.
3. Template tests for gated content and upgrade prompts.

### Definition of Done

- at least one page and one component vary by audience state
- chapter 11 proves the full content-gating path

## Workstream 6: Events and Timeslots

### Status

`not_started`

### Requirement Coverage

- 02
- 06 chapter 12

### Dependencies

- Workstream 5

### Framework Tasks

1. Audit current events support against requirement 02.
2. Ensure first-class support for:
   - event content
   - venue data
   - timeslots
   - visibility
   - featured state
   - capacity-facing fields
3. Expose stable event and timeslot route models to templates.

### Shoppr Tasks

1. Add event listing and event detail pages.
2. Make venue and timeslot data visible in public pages.
3. Connect event discovery back to memberships or audience state where appropriate.

### Getting Started Tasks

1. Write chapter 12 as a full event and timeslot walkthrough.

### Verification

1. Event listing and detail model tests.
2. Route tests for event visibility and rendering.

### Definition of Done

- the demo includes real event browse and detail pages
- chapter 12 ends with a runnable public event checkpoint

## Workstream 7: Bookings, Reservations, and Validation

### Status

`not_started`

### Requirement Coverage

- 02
- 03
- 06 chapter 13

### Dependencies

- Workstream 6

### Framework Tasks

1. Make the booking lifecycle explicit:
   - discover
   - reserve or hold
   - validate
   - commit booking
   - cancel or change
2. Make validation boundaries explicit:
   - capacity
   - eligibility
   - pass ownership
   - payment state if applicable
3. Add concurrency-sensitive tests for booking paths.

### Shoppr Tasks

1. Add a real booking flow tied to an event or timeslot.
2. Show booking state in the account area.
3. Show cancellation or change where supported.

### Getting Started Tasks

1. Write chapter 13 around the actual booking path and validation boundary.

### Verification

1. Reservation and booking tests.
2. Cancellation tests.
3. Account-surface tests for booked state.

### Definition of Done

- the booking flow is more than static UI
- chapter 13 teaches the transactional boundary clearly

## Workstream 8: Passes or Credits

### Status

`not_started`

### Requirement Coverage

- 02
- 06 chapter 14

### Dependencies

- Workstream 7

### Framework Tasks

1. Clarify pass or credit domain model and lifecycle state.
2. Connect pass state to eligibility and booking validation.
3. Expose pass state in account and admin surfaces.

### Shoppr Tasks

1. Add a pass-backed experience that differs from ordinary membership.
2. Show pass state in account and booking UI.

### Getting Started Tasks

1. Write chapter 14 around a pass or credit flow.

### Verification

1. Pass ownership and redemption tests.
2. Eligibility tests that include pass state.

### Definition of Done

- passes affect real product behavior
- chapter 14 demonstrates that difference clearly

## Workstream 9: Admin Resources for Editorial and Operations

### Status

`partial`

### Requirement Coverage

- 01
- 03
- 06 chapter 15

### Dependencies

- Workstreams 1, 6, 7, and 8

### Framework Tasks

1. Extend admin resources beyond CMS editing:
   - events
   - bookings
   - memberships
   - passes
   - reports
2. Add search, filters, actions, detail screens, and bulk-action primitives where missing.
3. Add export contracts and audit behavior for operational actions.

### Shoppr Tasks

1. Show operator workflows for:
   - editorial content
   - event or booking operations
   - membership or pass management
2. Make the admin shell read as one shared back-office surface.

### Getting Started Tasks

1. Write chapter 15 as an operator-focused walkthrough using actual admin files and actions.

### Verification

1. Admin resource tests.
2. Action and audit tests.
3. Export and filter tests where available.

### Definition of Done

- Shoppr admin demonstrates both editorial and operational resources
- chapter 15 proves the shared admin shell story

## Workstream 10: Reproducible Integration

### Status

`not_started`

### Requirement Coverage

- 03
- 06 chapter 16

### Dependencies

- Workstreams 7 and 9

### Framework Tasks

1. Choose and standardize one integration path that is reproducible locally.
2. Keep the boundary explicit:
   - ingress
   - verification
   - routing
   - follow-up work
3. Separate environment-owned secrets from editable business configuration.

### Shoppr Tasks

1. Use the same reproducible integration path end to end.
2. Surface the integration in public and admin understanding.

### Getting Started Tasks

1. Write chapter 16 around the actual integration used in Shoppr.

### Verification

1. Local reproducibility test path.
2. Webhook or callback verification tests.
3. Follow-up job or state transition tests.

### Definition of Done

- developers can reproduce the integration locally
- the tutorial and demo use the same path

## Workstream 11: Jobs, Notifications, and Scheduled Work

### Status

`not_started`

### Requirement Coverage

- 02
- 03
- 06 chapter 17

### Dependencies

- Workstreams 7, 8, and 10

### Framework Tasks

1. Ensure deferred and scheduled work is part of the public product model.
2. Support reminder and follow-up workflows tied to bookings, memberships, or integration events.
3. Keep operator visibility and auditability explicit.

### Shoppr Tasks

1. Show at least one real reminder or follow-up flow.
2. Show the operator consequence of that job.

### Getting Started Tasks

1. Write chapter 17 as a jobs-and-scheduled-work walkthrough.

### Verification

1. Job scheduling and execution tests.
2. Notification or follow-up state tests.

### Definition of Done

- jobs are part of the actual product flow
- chapter 17 has a visible runnable checkpoint

## Workstream 12: Observability and Production Preparation

### Status

`not_started`

### Requirement Coverage

- 03
- 06 chapters 18 and 19

### Dependencies

- Workstreams 9, 10, and 11

### Framework Tasks

1. Ensure readiness, diagnostics, and troubleshooting surfaces are part of the public runtime model.
2. Align production guidance with the real runtime shape:
   - assets
   - storage
   - cache
   - jobs
   - secrets
   - TLS

### Shoppr Tasks

1. Make health and operational diagnostics visible in the app and docs.
2. Make production guidance match the same product built in the tutorial.

### Getting Started Tasks

1. Write chapter 18 around observability and troubleshooting.
2. Write chapter 19 around production shaping and deployment concerns.

### Verification

1. Health/readiness verification.
2. Operational-path tests where available.

### Definition of Done

- users can debug the app, not just run it
- production guidance is an extension of the same tutorial product

## Workstream 13: Tutorial Completion and Reference Alignment

### Status

`partial`

### Requirement Coverage

- 04
- 05
- 06

### Dependencies

- Workstreams 1 through 12

### Framework Tasks

1. Fill any remaining public seam gaps exposed by tutorial writing.
2. Remove contradictions between tutorial and reference pages.

### Shoppr Tasks

1. Keep the demo aligned with the final tutorial story.
2. Remove generic or placeholder explanations that do not teach ownership and effect.

### Getting Started Tasks

1. Finish all twenty chapters.
2. Apply the tutorial writing rule to every chapter:
   - show the actual file contents when a file is introduced
   - explain what the file owns
   - explain what each relevant section does
   - explain what changes in the running app
   - end with a visible checkpoint
3. Write chapter 20 as the map back to reference docs and next extension points.

### Verification

1. `website` build passes.
2. Tutorial cross-links match the actual reference pages.
3. Random chapter review confirms the tutorial is code-first rather than abstract.

### Definition of Done

- the twenty-chapter tutorial is complete
- the tutorial teaches the platform seams without guesswork
- the reference docs and tutorial agree

## Active Queue

The immediate execution queue is:

1. Workstream 5: Memberships and audience gating
2. Workstream 6: Events and timeslots
3. Workstream 7: Bookings, reservations, and validation
4. Workstream 13: rewrite chapters 11 through 13 in lockstep with code

These are the next dependencies required to move the requirement set from partial alignment toward
full implementation.
