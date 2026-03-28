# Anatomy of a Customer App

**Part:** Customer Apps  
**Chapter:** 75

A customer app is the unit of product assembly on top of the platform. It is not a thin theme over one shared mega-application. Each customer app is a separate composition of core services, selected official native modules, app-specific templates and content models, and any custom WASM extensions needed for that customer's business.

## What a Customer App Contains

At minimum, a customer app owns:

- application configuration, including one or more site definitions, storage policy, and feature flags
- the set of installed official modules
- frontend theme, templates, fragments, and design-token choices
- content-model definitions and customer-specific schemas
- translations, SEO content, and brand copy
- auth model extensions or replacements and capability bindings
- customer-specific WASM extensions and integration settings

This is where product variability belongs. Core and official modules provide the reusable machinery; the customer app binds it into a deployable product.

In the initial multi-site model, a customer app remains the deployment and composition root, while each site inside that app owns public host bindings, canonical-host policy, and locale policy. That keeps module installation and deployment batteries app-scoped while making routing and SEO policy site-scoped.

## What a Customer App Does Not Own

The customer app does not reimplement:

- the HTTP runtime
- the auth engine and tuple execution model
- storage, cache, queue, TLS, or certificate automation
- the template engine itself
- major reusable business batteries such as CMS, commerce, memberships, or events

Those remain in core or in official native modules so they can evolve once and stay supportable across customers.

## Typical Shape

For the current business context, the first real customer app is likely to combine:

- commerce
- memberships and subscriptions
- events, timeslots, reservations, and bookings
- branded CMS and admin behavior

On top of that, the app adds its own white-label presentation, region-aware policy, customer-specific integrations, and any narrow custom workflows.

## Upgradeability Rule

A customer app stays upgradeable by keeping each kind of variability in the right place:

- reuse official modules for reusable domain behavior
- use configuration and content models for sanctioned variability
- use WASM for bounded customer-specific logic
- avoid patching core or forking official modules unless the work is truly becoming a new supported module

That split is the difference between a maintainable product line and a new version of WordPress-style application sprawl.
