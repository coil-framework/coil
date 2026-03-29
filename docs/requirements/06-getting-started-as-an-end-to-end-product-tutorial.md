# Getting Started as an End-to-End Product Tutorial

## Purpose

Coil's public documentation currently has a strong reference layer but an insufficient narrative layer. The reference pages explain individual concepts and APIs, but they do not yet give customer developers a reliable end-to-end mental model for how those pieces are meant to compose into a real product.

This document defines a documentation requirement rather than a framework requirement. The requirement is that `Getting Started` must become a step-by-step product-building tutorial that teaches Coil by constructing a realistic site from scratch.

## Why This Is Needed

The current documentation problem is not simply that some reference pages are missing. The larger issue is that the user can read technically correct documentation and still be left guessing where the platform seams actually are.

Examples of the current gap:

- understanding block schema but not understanding content instances
- understanding render-model hooks but not understanding when to mount versus merge
- understanding templates but not understanding where request-time model data comes from
- understanding modules individually but not understanding how they cooperate in a real app

This is exactly the kind of problem that a reference section is bad at solving and a guided tutorial is good at solving.

## Documentation Strategy

The public docs should keep the current two-layer structure, but the role of each layer must be clearer.

### Reference Docs

The reference docs should remain:

- modular
- concept-focused
- API-focused
- stable lookup material

They do not need to be rewritten into one giant narrative if the tutorial layer is strong enough.

### Getting Started

`Getting Started` should become the bridging narrative.

It should:

- build one realistic product step by step
- introduce Coil concepts only when they become necessary
- end each section with a working checkpoint
- show the full chain from configuration to data model to render model to UI to admin to operations

This means the user learns the joints by building something that forces those joints to become visible.

## Tutorial Product Shape

The tutorial should build a realistic retail, events, and memberships site rather than a toy storefront.

The rewritten demo should prove that Coil can support:

- editorial flexibility
- reusable content blocks
- live event and booking flows
- memberships and audience gating
- admin tooling
- background jobs
- at least one reproducible external integration

The tutorial product should remain generic in the docs, but its shape should intentionally reflect the needs of high-end retail and membership-led businesses with strong editorial requirements.

## Core Documentation Principle

Every tutorial section must be:

- conceptually focused
- somewhat self-contained
- cumulative
- runnable at the end

The user should be able to stop after each section, run the app, and clearly see what changed.

That visible feedback loop is essential. Without it, the tutorial becomes another reference document in disguise.

## Proposed Table of Contents

### 1. What You Are Building

Explain the final product shape:

- a realistic multi-market retail, events, and memberships site
- editorially composed pages
- customer accounts and gated content
- live event discovery and bookings
- admin and operational workflows

This chapter should set expectations and explain why the tutorial is intentionally more ambitious than a typical quickstart.

### 2. Create the Project

Use `cargo coil new` and explain:

- generated workspace structure
- `cargo coil dev`
- Docker services
- the customer binary
- linked Rust backend

Checkpoint:

- the generated app boots locally

### 3. Understand the Runtime Shape

Explain:

- `app.toml`
- `platform.dev.toml`
- sites and locales
- official modules
- customer-owned crates

Checkpoint:

- the starter app runs and the reader understands which files own which concerns

### 4. Build the Base Theme

Introduce:

- theme structure
- layouts
- fragments
- assets
- design tokens

Checkpoint:

- the app renders a branded shell with header, footer, and static layout

### 5. Add Sites, Markets, and Locales

Introduce:

- multi-site routing
- locale-aware routing
- site and locale switching
- localized content structure

Checkpoint:

- site and locale variations render correctly

### 6. Add a Real Content Model

Introduce:

- CMS schema
- page instances
- page settings
- global options

This chapter must explicitly teach the difference between schema, content instance, and render model.

Checkpoint:

- editors can define structured pages and settings

### 7. Build Reusable Blocks

Introduce:

- block types
- repeaters and nested fields
- reusable/shared blocks
- ordering
- enable/disable
- preview

Checkpoint:

- the home page is composed from reusable editorial blocks

### 8. Add Dynamic Blocks

Introduce:

- render-model hooks
- customer namespaces
- mount versus merge
- dynamic block fragment dispatch

Examples should include things like:

- featured events
- featured brands
- membership callouts

Checkpoint:

- editorial blocks can mix stored content and live data

### 9. Model Brands, Categories, and Discovery

Introduce:

- customer domain models
- listings
- search and filtering
- route-aware discovery pages

Checkpoint:

- branded discovery pages and taxonomy-driven browsing work

### 10. Add Authentication and Customer Accounts

Introduce:

- registration
- login
- sessions
- CSRF
- account pages
- profile completeness

Checkpoint:

- a customer can sign up, sign in, and edit their profile

### 11. Add Memberships and Audience Gating

Introduce:

- membership tiers
- gated pages
- upgrade prompts
- audience-aware rendering
- route and page access rules

Checkpoint:

- members-only and tier-aware content works

### 12. Add Events and Timeslots

Introduce:

- events
- venues
- timeslots
- capacity
- visibility
- event detail pages

Checkpoint:

- editors can publish events and customers can browse them

### 13. Add Bookings, Reservations, and Validation

Introduce:

- reservation or hold model
- booking validation
- transactional booking flow
- cancellation
- confirmation

Checkpoint:

- a customer can make and manage a booking

### 14. Add Passes or Credits

Introduce:

- event passes or credits
- entitlement checks
- pass compatibility with bookings
- admin visibility into pass state

Checkpoint:

- pass-backed access works alongside memberships

### 15. Add Admin Resources

Introduce:

- admin shell
- resource registration
- tables
- filters
- actions
- detail views
- audit trail

Checkpoint:

- operators can manage content and bookings

### 16. Add One Reproducible Integration

Pick one integration that is:

- important
- easy for most developers to reproduce locally
- representative of the wider integration story

The recommended default is Stripe.

Introduce:

- configuration
- webhooks
- local testing
- payment-backed membership or booking flow

Checkpoint:

- the integration works end to end in development

### 17. Add Jobs, Notifications, and Scheduled Work

Introduce:

- jobs
- scheduled tasks
- reminder flows
- follow-up operational work

Checkpoint:

- reminder or follow-up jobs run correctly in local development

### 18. Add Observability and Troubleshooting

Introduce:

- logs
- readiness and health
- diagnostics
- common failure modes

Checkpoint:

- the reader can debug the app rather than only run it

### 19. Prepare for Production

Introduce:

- secrets
- migrations
- assets
- production topology
- deployment shape

Checkpoint:

- the app is production-shaped, not only development-shaped

### 20. Where to Go Next

Map the completed tutorial back to:

- reference docs
- official modules
- extension points
- next-level topics

This chapter should explicitly convert the tutorial mental model into a reference-doc navigation model.

## Requirements for the Tutorial Itself

The new tutorial must:

- use one coherent product throughout
- avoid toy examples that do not resemble real customer needs
- explain the ownership boundary for each new concept
- explain what the framework provides automatically
- explain what the customer app contributes explicitly
- end every chapter with a runnable checkpoint
- show visible outcome after each step

## Implications for the Demo Application

The demo application used by the tutorial should evolve from being a narrow storefront example into a more realistic proof that Coil can support:

- rich editorial composition
- account-aware rendering
- event and booking workflows
- admin operations
- one reproducible integration

The point is not to recreate every customer-specific edge case in the demo. The point is to make the demo representative enough that success in the tutorial genuinely proves the platform shape.

## Relationship to the Existing Public Docs

If this tutorial is done well, the existing reference material can remain largely modular and lookup-oriented. The tutorial will provide the bridging knowledge that is currently missing by:

- introducing concepts in the order a real product needs them
- showing how the pieces compose
- revealing the platform seams at the moment they matter

That is a better fix than trying to turn every reference page into a mini-tutorial.

## Expected Outcome

After completing the new `Getting Started`, a developer should be able to answer confidently:

- where schema lives
- where content instances live
- where request-time model shaping happens
- when to use official modules versus customer code
- how templates, hooks, admin, jobs, and integrations connect

If the tutorial achieves that, the public documentation will stop feeling like a collection of correct fragments and start feeling like a coherent product manual.
