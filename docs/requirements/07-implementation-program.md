# Implementation Program

## Purpose

The six requirement documents describe the target product shape. This document turns that shape into executable work. It is the management layer between requirements and code.

The goal is to make progress without allowing the work to dissolve into disconnected local improvements. Each epic below has:

- a purpose
- explicit scope
- dependencies
- concrete task slices
- acceptance criteria

## Program Structure

The implementation program is organized into seven epics:

1. CMS editorial foundation
2. Dynamic render-model and block composition
3. Events, bookings, and passes hardening
4. Memberships, accounts, and audience gating
5. Admin shell and operational tooling
6. Integrations, jobs, and observability
7. Documentation tutorial and demo rewrite

## Epic 1: CMS Editorial Foundation

### Purpose

Create the missing foundation for structured page-builder content so Coil has a real distinction between schema, content instances, reusable content, and rendering.

### Scope

- block type schemas
- structured block instances
- shared block definitions and references
- page settings
- global options surfaces
- page-instance domain model
- preview and publication implications at the model level

### Dependencies

- none beyond existing CMS module foundations

### Task Slices

1. Extend `coil-cms` domain model with:
   - block schema types
   - block instance types
   - shared block types
   - page settings types
2. Define invariants and validation rules:
   - schema ids and field ids
   - block reference rules
   - enabled and disabled semantics
   - nested field constraints
3. Add tests covering:
   - schema creation
   - page composition
   - shared-block reference resolution contracts
   - invalid combinations
4. Define storage-facing contracts for later persistence work:
   - serializable page-instance representation
   - serializable shared-block representation

### Acceptance Criteria

- `coil-cms` has first-class model types for schema and content instances
- the difference between schema and content instances is explicit in code
- reusable/shared blocks are represented distinctly from page-owned blocks
- model tests cover valid and invalid editorial states

## Epic 2: Dynamic Render-Model and Block Composition

### Purpose

Allow editorial content to combine with live data through explicit, stable platform contracts.

### Scope

- dynamic block contracts
- render-model contributions for structured pages
- page/block fragment dispatch
- customer namespace versus framework merge guidance reflected in code/tests where applicable

### Dependencies

- Epic 1 domain types
- existing render-model hook foundation

### Task Slices

1. Define how page-instance blocks reach templates:
   - canonical `page.blocks`
   - customer namespace mounting guidance
2. Define dynamic block resolution contract:
   - editorial config in CMS
   - live data shaping in code
   - final render fragment or render model output
3. Add tests for:
   - page blocks exposed to templates
   - shared block expansion
   - dynamic block dispatch
   - mixed static and dynamic block pages
4. Update reference docs:
   - `app.toml`
   - render-model hooks
   - template models

### Acceptance Criteria

- a realistic editorial page can expose structured blocks to templates without undocumented glue
- dynamic blocks have an explicit contract rather than implied behavior
- docs state clearly where render-time shaping happens

## Epic 3: Events, Bookings, and Passes Hardening

### Purpose

Bring the event and booking stack up to the level required by editorially rich retail and membership-led sites.

### Scope

- events
- venues
- timeslots
- reservations
- bookings
- passes
- waitlists
- booking validation
- booking operations support

### Dependencies

- existing events and memberships modules
- admin shell foundations for operator workflows

### Task Slices

1. Audit current modules against requirements:
   - explicit gap list
2. Fill the highest-risk transactional gaps:
   - reservation handling
   - pass compatibility
   - cancellation and check-in state
3. Add operator-facing hooks for:
   - search/filter/export
   - manual operations
4. Add scenario tests:
   - capacity-safe booking
   - cancellation
   - pass-backed booking
   - reminder and attendance state transitions

### Acceptance Criteria

- the domain covers the required event and booking lifecycle explicitly
- transaction-sensitive flows have tests
- operator workflows are not an afterthought

## Epic 4: Memberships, Accounts, and Audience Gating

### Purpose

Make account state and audience targeting first-class so content, events, and bookings can all depend on them consistently.

### Scope

- profile completeness
- memberships and subscriptions
- preference and consent state
- page and route gating
- audience-aware rendering

### Dependencies

- auth and memberships modules
- dynamic render-model composition

### Task Slices

1. Audit current account and membership flows against the requirement set
2. Add or refine:
   - profile completeness contracts
   - gating rules
   - audience-aware page visibility
3. Add tests for:
   - members-only page access
   - tier-aware rendering
   - account-state dependent CTAs or page behavior

### Acceptance Criteria

- content and event visibility can depend on membership and audience state
- the account and membership story is coherent across frontend and admin behavior

## Epic 5: Admin Shell and Operational Tooling

### Purpose

Ensure Coil can support both editorial users and operations users through a common admin shell.

### Scope

- resource registration
- tables
- filters
- detail views
- actions
- bulk actions
- export contracts
- audit trail

### Dependencies

- Epic 1 and 3 domain models

### Task Slices

1. Identify missing admin primitives needed for:
   - pages and shared blocks
   - events and bookings
   - passes and memberships
2. Add or refine:
   - resource list contracts
   - filters
   - bulk action primitives
   - export hooks
3. Add tests for:
   - admin resource registration
   - action gating
   - list/detail rendering expectations where applicable

### Acceptance Criteria

- admin supports more than page editing
- editorial and operational workflows fit within one coherent shell

## Epic 6: Integrations, Jobs, and Observability

### Purpose

Support the operational perimeter that real businesses need around the core site.

### Scope

- webhook ingestion
- scheduled work
- follow-up notifications
- integration settings
- operational observability

### Dependencies

- jobs, observability, and relevant domain modules

### Task Slices

1. Define the supported reproducible integration story for the tutorial/demo
2. Ensure jobs and webhooks cover:
   - reminder flows
   - payment-backed flows
   - integration auditability
3. Add tests and operational docs for:
   - webhook verification and replay safety
   - scheduled job execution
   - local development of integrations

### Acceptance Criteria

- at least one representative integration path is coherent end to end
- jobs and webhook behavior are testable and observable

## Epic 7: Documentation Tutorial and Demo Rewrite

### Purpose

Turn the public onboarding story into a realistic product-building tutorial and align the demo with the target customer shape.

### Scope

- getting-started restructuring
- tutorial chapters
- demo application evolution
- reference-doc cross-linking

### Dependencies

- enough implementation from Epics 1 through 6 that the tutorial is honest

### Task Slices

1. Restructure `Getting Started` into a tutorial sequence
2. Build the early tutorial chapters:
   - project creation
   - runtime shape
   - theme
   - sites/locales
3. Evolve the demo to exercise:
   - structured CMS
   - dynamic blocks
   - bookings/memberships/admin
   - one reproducible integration
4. Map every tutorial chapter back to stable reference docs

### Acceptance Criteria

- the public tutorial teaches the platform seams through a real build
- the demo proves the platform can support the target product shape
- reference docs remain lookup-oriented while the tutorial provides the narrative bridge

## Current Execution Order

The recommended execution order is:

1. Epic 1
2. Epic 2
3. Epic 7 early docs restructuring in parallel
4. Epic 3
5. Epic 4
6. Epic 5
7. Epic 6
8. Epic 7 full demo/tutorial completion

## Immediate Tranche

The first execution tranche should be:

- Epic 1 foundational `coil-cms` domain model work
- Epic 7 tutorial restructuring for early chapters
- public-doc clarification driven by Epic 2 boundary needs

This tranche has the highest leverage because it fixes the two biggest present failures:

- lack of structured editorial foundations
- lack of a coherent onboarding narrative
