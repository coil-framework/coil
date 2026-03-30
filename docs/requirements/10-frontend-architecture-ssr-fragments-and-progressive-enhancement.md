# Frontend Architecture Requirements for SSR Fragments and Progressive Enhancement

## Purpose

Coil already has an HTML-first rendering model, fragment rendering, template composition, and an
asset pipeline. What it does not yet have in one place is a clear frontend requirement set for how
customer apps, official modules, and extensions are supposed to contribute real browser behavior
without collapsing the system into an accidental SPA.

This document defines that requirement set for a frontend architecture built around:

- server-rendered HTML documents and fragments
- Turbo for navigation and HTML-over-the-wire replacement
- Stimulus for small client-side controllers
- PostCSS for CSS compilation
- esbuild for JavaScript and CSS entrypoint bundling

The goal is not to make those tools the product. The goal is to define the platform contract that
lets Coil use them coherently.

## The Core Product Requirement

The frontend architecture must preserve Coil's SSR-first operating model while making real
interactive applications practical for storefronts, account areas, CMS pages, and admin/editor
surfaces.

That means:

- HTML documents remain the default response artifact
- HTML fragments remain the default enhanced interaction artifact
- JavaScript enhances server-owned UI instead of replacing it
- customer apps can supply branded frontend behavior without taking over module semantics
- official modules can declare frontend contributions without hard-wiring themselves to one global
  bundle
- extensions can contribute to approved slots without mutating the shell arbitrarily at runtime

## Required Frontend Contribution Model

The platform must support declared frontend contributions rather than implicit asset inclusion.

At minimum, the contribution model must support:

- JavaScript controller entrypoints
- CSS entrypoints
- route-scoped or surface-scoped asset declarations
- fragment-scoped enhancement declarations where appropriate
- admin/editor-only bundles distinct from public-site bundles
- customer-app composition of official-module contributions into final bundles

The important boundary is that modules and extensions declare what they contribute, while the
customer app decides what becomes part of the final storefront or admin build.

### Required Rules

1. A route or surface must be able to declare the frontend contributions it needs.
2. A fragment must be able to declare enhancement requirements without assuming a global script is
   always present.
3. An official module must be able to ship frontend behavior without forcing a customer app to
   accept the module's entire visual shell.
4. A customer app must be able to override presentation and enhancement wiring at supported seams.
5. An extension must only contribute through explicit slots or declared surface hooks.

## Required Fragment and Component Contract

The platform must define a first-class fragment or component contract for SSR-first UI.

That contract must include:

- a stable fragment identifier
- the fragment's server-owned input model
- allowed slot inputs or nested content areas
- the fragment's enhancement hooks, if any
- cache and auth scope
- whether the fragment is valid as:
  - full-page content
  - an inline partial render
  - a Turbo frame or stream target

The key requirement is explicitness. A fragment must not depend on ambient global state or hidden
selector conventions.

### Required Outcomes

- A fragment rendered inside a full page and the same fragment rendered as a partial response must
  have the same semantics.
- Stimulus controllers must attach through stable, documented attributes, not accidental CSS hooks.
- Turbo targets and fragment boundaries must line up with the server's rendering boundaries.

## Required Route and Surface Asset Loading

The runtime must support route-aware and surface-aware asset loading.

The platform must be able to distinguish at least:

- public storefront shell assets
- account and membership assets
- admin/editor shell assets
- fragment-specific enhancements that are not needed on every page

This requirement exists for two reasons:

- performance: admin/editor bundles should not inflate the public storefront
- correctness: admin/editor behavior often needs richer controllers than public pages

### Required Behavior

1. A rendered route must be able to declare which logical bundles it depends on.
2. A rendered admin/editor surface must be able to load a different bundle set from the public site.
3. A fragment response must be able to rely on the same declared asset contract as the full page
   that hosts it.
4. Development mode and production mode must share the same logical bundle names even if the
   delivery mechanism differs.

## Required Customer Override Model

Customer apps must be able to override and extend frontend behavior without forking official-module
business semantics.

The platform must support:

- layout overrides
- fragment presentation overrides
- customer-owned Stimulus controllers
- customer-owned stylesheet layers and tokens
- customer-owned bundle composition

The platform must not require a customer app to copy an entire module screen just to:

- restyle it
- change shell placement
- add a small interaction
- add a branded fragment wrapper

If a module's frontend can only be changed by copying the whole template and controller surface, the
module boundary is wrong.

## Required Extension Slot Model

Extensions must be able to contribute frontend behavior only through declared slots and surface
hooks.

The platform must support extension contributions such as:

- adding a fragment to a declared slot
- adding a small controller or stylesheet contribution to a declared admin or storefront surface
- registering a widget for a documented admin/editor region

The platform must not support:

- arbitrary runtime injection into the global shell
- undeclared mutation of the page head or asset graph
- arbitrary cross-surface JavaScript execution

The extension boundary must stay reviewable and deterministic.

## Required Development and Build Pipeline

The frontend toolchain must be explicit and reproducible.

The required pipeline is:

- PostCSS for stylesheet compilation and transforms
- esbuild for JavaScript bundling and asset graph assembly
- a development watcher or dev server that still respects the asset manifest contract
- production builds that emit hashed assets and a manifest

### Required Development Experience

Developers must be able to:

- run a local dev watcher
- edit templates, CSS, and Stimulus controllers with fast feedback
- use the same logical asset names in templates in development and production
- understand where official-module, customer-app, and extension contributions end up in the final
  bundle graph

### Required Build Outcomes

Production builds must:

- emit immutable hashed assets
- emit a manifest keyed by logical entrypoint names
- support separate public and admin/editor entrypoints
- support sourcemaps with an explicit publication policy

## Required Admin and Editorial Behavior

The admin/editor frontend has stronger requirements than the public storefront.

The architecture must support:

- inline validation and form feedback that still works as normal HTML
- partial updates for editor sidebars, page-builder panels, previews, and resource tables
- predictable focus management and accessibility after fragment replacement
- editor-specific bundles and controllers without shipping them to public pages

The admin/editor shell may use richer enhancement than the public storefront, but it must still
honor the same rules:

- server owns business state
- forms still post to real handlers
- fragment refresh remains HTML-first
- customer apps can brand within supported admin boundaries

## Documentation and Public-Contract Obligations

The public docs must explain:

- how frontend contributions are declared
- how a fragment advertises its controller and slot contract
- how route-level or surface-level asset selection works
- how customer overrides differ from extension contributions
- how Turbo and Stimulus fit into the server-owned rendering model
- how admin/editor surfaces differ from storefront surfaces operationally

Examples must show the full chain:

- declared contribution
- route or fragment using it
- rendered HTML boundary
- resulting asset load
- fallback behavior when JavaScript is absent

## Verification Obligations

The platform is not done when the build works. It is done when the architecture is testable and
auditable.

At minimum, verification must cover:

- document render without JavaScript
- fragment refresh through Turbo or ordinary HTTP fallback
- route-level asset inclusion
- admin/editor bundle separation from public bundles
- customer override behavior
- extension slot rendering and isolation
- accessibility after partial updates

## Acceptance Criteria

This requirement set is satisfied when:

1. Coil has a documented frontend contribution model for official modules, customer apps, and
   extensions.
2. Fragments and components have an explicit SSR contract, not just template conventions.
3. Public routes and admin/editor routes can load different declared asset sets through the same
   manifest model.
4. Customer apps can override and extend the frontend at supported seams without copying module
   business logic wholesale.
5. The development and production asset pipeline is explicit, repeatable, and consistent with the
   runtime's asset manifest contract.
6. The admin/editor UI can support richer progressive enhancement without breaking the HTML-first
   operating model.
