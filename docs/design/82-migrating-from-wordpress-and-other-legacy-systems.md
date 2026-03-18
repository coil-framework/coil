# Migrating From WordPress and Other Legacy Systems

**Part:** Migration and Evolution  
**Chapter:** 82

Migration is one of the core reasons this platform exists. The target is not a simple rehosting of WordPress behavior. The target is a clean separation between reusable platform capabilities, installable official modules, and a customer-specific app. That means a migration project starts by deciding what belongs in each layer, not by copying every table and plugin into a new stack.

## Migration As Capability Mapping

A legacy system contains at least four kinds of things:

- content and assets
- business workflows
- identity and authorization rules
- operational assumptions, including URLs, redirects, jobs, integrations, and editorial process

The migration process maps those into the platform shape:

- reusable concerns move into core only if they are true platform primitives such as caching, storage, TLS, auth, i18n, or SEO infrastructure
- repeatable business capability moves into official modules such as CMS, commerce, memberships, events, media, and admin
- customer-specific presentation, field choices, content types, and business rules move into the customer app or its custom extensions

This prevents the new system from becoming a second WordPress, where one customer’s workaround quietly becomes everyone’s runtime burden.

## WordPress-Specific Interpretation

For a WordPress-origin system, the first pass is usually straightforward:

- posts, pages, redirects, menus, and editorial workflow map to CMS and admin modules
- uploads and media-library records map to managed assets governed by storage policy and asset capabilities
- custom post types such as events or brands map to official modules or customer-owned domain types
- user accounts, memberships, subscriptions, and account workflows map to auth, profile, and subscription modules
- bespoke plugin behavior gets split into either a first-party module, a customer app concern, or a WASM extension

The important discipline is that plugin parity is not the goal. Capability parity is the goal. If a WordPress site has fifteen plugins but only six actual business capabilities, the migration should reproduce the six capabilities and retire the accidental complexity.

## Identity, Auth, and Roles

Legacy roles should not be copied as fixed role names. They should be translated into the platform’s capability model and bound through an auth model package. In practice that means:

- importing users, groups, and key organizational relationships
- translating role intent into capabilities such as `cms.page.publish`, `asset.publish`, `catalog.product.edit`, or `events.booking.manage`
- deciding whether the default auth model is sufficient, needs extension, or should be replaced entirely for that customer

This is especially important when moving from systems that used ad hoc admin flags or plugin-specific permissions. The migration should normalize those into capability checks instead of preserving fragmented authorization logic.

## Content and URL Preservation

Public URL stability is a migration requirement, not an afterthought. The CMS and SEO layers must preserve:

- canonical URLs where they can remain unchanged
- redirect mappings where the content model changes
- locale-aware routes and `hreflang` data where the customer operates in multiple locales
- metadata needed for search continuity, including titles, descriptions, robots settings, and structured-data inputs

If the legacy system mixed content, commerce, and membership pages under one URL scheme, the migration plan should still make each route family explicit. That simplifies ownership in the new platform and reduces future routing collisions.

## Legacy System Categories

Not every source system is WordPress, but the same rules apply to other legacy stacks:

- from a monolithic CMS, separate editing concerns from runtime and move only reusable primitives into core
- from a bespoke app, identify which domain flows deserve official-module status because multiple customer apps will need them
- from a plugin-heavy ecommerce platform, separate catalog, checkout, orders, and content into explicit modules instead of one opaque application blob

In all cases, the migration should end with fewer hidden extension points, fewer global side effects, and clearer ownership of runtime behavior.

## Migration Deliverables

A credible migration plan produces more than an import script. It should produce:

- a capability map from legacy features to core, official modules, customer app code, and retired behavior
- an auth mapping document from legacy roles or ACLs to capability bindings
- a redirect and SEO preservation plan
- a content and media extraction specification
- a cutover and rollback plan tied to operational reality

The output of migration is a maintainable customer app on a reusable platform, not a historical copy of a legacy implementation.
