# Gap Audit and Realignment

## Purpose

This document records the gap between the requirement set in documents 01 through 06 and the
current implementation state in the codebase and public docs.

The purpose is corrective. Earlier implementation work closed several important gaps, but it did
not complete the whole requirement set. This document makes the remaining work explicit so the
implementation program can be driven by real omissions rather than optimistic summaries.

## Current Overall Status

The current state is:

- partially aligned with 01
- partially aligned with 04
- partially aligned with 05
- misaligned with 06
- incomplete against 02 and 03

In short: the platform has moved forward, but the full requirement set has not yet been
implemented.

## What Is Already In Better Shape

The following areas have meaningful progress:

- render-model hooks now support explicit mount and merge behavior
- the template engine now supports a richer expression language and block-oriented rendering
- the CMS model now distinguishes schema from content instances more clearly than before
- structured page surfaces, shared blocks, scheduled publication, and rollback now exist in the
  platform
- public documentation includes several boundary pages that explain schema versus instances,
  dynamic blocks, and render-model composition

Those changes matter, but they do not complete the full program.

## Gap 1: Requirement 06 Is Not Yet Implemented

Requirement 06 defines a twenty-part end-to-end tutorial. The current getting-started section does
not satisfy that requirement.

### Current Tutorial Structure

The current getting-started section contains these pages:

- `quickstart`
- `what-you-are-building`
- `create-the-project`
- `understand-the-runtime-shape`
- `build-the-base-theme`
- `add-sites-markets-and-locales`
- `add-a-real-content-model`
- `build-reusable-blocks`
- `add-dynamic-blocks`
- `customer-project-layout`
- `linked-rust-backends`

That is not the twenty-step tutorial defined in requirement 06. It is still an early-stage
tutorial skeleton plus supporting reference-like pages.

### Missing Tutorial Chapters

The following required tutorial chapters do not exist yet:

1. `model-brands-categories-and-discovery`
2. `add-authentication-and-customer-accounts`
3. `add-memberships-and-audience-gating`
4. `add-events-and-timeslots`
5. `add-bookings-reservations-and-validation`
6. `add-passes-or-credits`
7. `add-admin-resources`
8. `add-one-reproducible-integration`
9. `add-jobs-notifications-and-scheduled-work`
10. `add-observability-and-troubleshooting`
11. `prepare-for-production`
12. `where-to-go-next`

The current chapters also do not yet follow the stricter tutorial-writing standard required by 06:

- full file contents when a file is introduced
- explicit explanation of what each file and section does
- visible checkpoints after each chapter
- one coherent product story all the way through

## Gap 2: The Tutorial Product Is Not Yet the Required Product

Requirement 06 does not call for a generic tutorial workspace. It calls for a realistic product
tutorial that proves Coil can support a demanding retail, editorial, events, and memberships site.

The current getting-started content still centers a generic `tutorial-app`. That is useful for
early framing, but it does not satisfy the requirement to prove the target platform shape.

### What Is Missing

The tutorial does not yet build:

- real brand and category discovery
- account state and profile flows
- membership-aware rendering
- events and timeslots
- transactional booking flows
- passes or credits
- meaningful admin resources
- a reproducible integration
- jobs and operational flows
- observability and production preparation

The demo story is therefore still too small relative to the agreed requirement.

## Gap 3: Requirement 01 Is Only Partially Implemented

Requirement 01 calls for a first-class page-builder and editorial model.

### Implemented or Partially Implemented

- structured block schema
- structured page content surfaces
- shared block references
- scheduled publication
- rollback
- explicit render-model shaping boundaries

### Still Missing or Not Yet Proven End to End

- global options surfaces as a fully coherent editor-facing system
- nested and repeatable editorial field groups proven end to end
- per-page enable and disable state for blocks through the authoring interface
- block duplication, ordering, and reusable-block usage inspection in the admin UI
- preview workflow that demonstrates the full composed page-builder flow in the tutorial
- page-level targeting and audience/editorial controls at the level described in 01

The platform has the start of the editorial model, but not the whole editorial operating model.

## Gap 4: Requirement 02 Is Only Partially Implemented

Requirement 02 covers events, bookings, memberships, passes, and entitlements as first-class
platform capability.

### What Exists in Some Form

- modules and docs for events, commerce, memberships, jobs, and webhook flows
- product examples in Shoppr
- some pass-oriented catalog and commerce examples

### What Is Missing Relative to the Requirement

- a fully documented and tutorial-backed event and timeslot flow
- transactional reservation or hold modeling clearly exposed as a first-class customer-facing path
- waitlist behavior proven in the official getting-started path
- check-in and attendance workflows surfaced as part of the product story
- pass-backed booking and entitlement behavior demonstrated end to end
- region, audience, and tier-aware event visibility taught through the tutorial

This area may have pieces in code, but it is not yet implemented as a coherent platform-and-demo
story.

## Gap 5: Requirement 03 Is Only Partially Implemented

Requirement 03 covers admin operations, exports, jobs, webhooks, audit, and integrations.

### What Exists in Some Form

- admin shell foundations
- jobs and webhook docs
- audit-related workflow improvements in CMS
- Shoppr examples of backend integrations

### What Is Missing Relative to the Requirement

- a clear operator-oriented admin resource story in the tutorial
- booking and event operations exposed as first-class admin resources in the demo
- export flows and bulk operational actions demonstrated in a realistic use case
- integration settings taught as editable business configuration instead of pure environment config
- end-to-end operator workflows tying audit, notifications, jobs, and admin actions together

The shell exists, but the product-level operator story is still incomplete.

## Gap 6: Requirement 04 Is Improved but Not Finished

Requirement 04 focuses on platform seams and public documentation clarity.

### Improved

- schema versus content instance distinctions are better documented
- render-model hooks now have a real public API
- mount versus merge is documented
- dynamic blocks have a clearer contract

### Still Missing

- the end-to-end tutorial does not yet serve as the bridging narrative required by 04 and 06
- several getting-started pages still explain files too abstractly instead of teaching ownership,
  effect, and sequence
- the docs still do not walk a user through a full realistic application that forces all the main
  seams to become visible

## Gap 7: Requirement 05 Needs a More Honest Program State

Requirement 05 defines the phased delivery plan. The current `07-implementation-program.md` is
useful, but it needs to be treated as an active execution plan rather than a mostly static design
note.

The missing correction is:

- explicit status per epic
- explicit dependencies between tutorial chapters and platform work
- a rule that the tutorial cannot claim a chapter until the underlying capability is real in code
  and demonstrated in the demo app

## Realignment Decisions

The implementation should now be realigned around these decisions.

### 1. The Twenty-Chapter Tutorial Is a Hard Requirement

The getting-started section must be expanded from the current early skeleton into the full
twenty-part structure defined in requirement 06.

### 2. The Tutorial Must Use One Coherent Product Story

The tutorial must stop reading like a generic sample and start building the more realistic Shoppr
shape agreed in requirement 06:

- editorial composition
- discovery
- accounts
- memberships
- events
- bookings
- passes
- admin operations
- one reproducible integration
- jobs and observability

### 3. Tutorial and Platform Work Must Move Together

A tutorial chapter should not be written as if the capability exists unless:

- the platform behavior exists in code
- the demo app exercises it
- the user can run the checkpoint and observe the promised result

### 4. The Next Missing Capabilities Should Be Implemented in Product Order

The next implementation tranches should be:

1. global options and editorial targeting completion
2. brands, categories, and discovery
3. authentication, accounts, and profile completeness
4. memberships and audience gating
5. events and timeslots
6. bookings, reservations, and validation
7. passes or credits
8. admin resources and operator flows
9. one reproducible integration
10. jobs, notifications, observability, and production preparation

That ordering is now expanded into the active dependency-ordered backlog in
[09-detailed-execution-plan.md](./09-detailed-execution-plan.md), which is the management document
used to drive execution from the requirement set.

## Acceptance Criteria for Realignment

The requirement set can be considered aligned only when:

- all twenty tutorial chapters exist
- each chapter is concrete, cumulative, and runnable
- the tutorial product proves the required platform shape
- the demo app covers the agreed editorial, account, event, booking, admin, and integration
  capabilities
- the implementation program tracks real remaining work rather than summarizing partial progress as
  completion
