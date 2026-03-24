# Installing and Composing Official Modules

**Part:** Customer Apps  
**Chapter:** 76

Official modules are the supported batteries shipped on top of core. They are native first-party modules, versioned separately from core, and installed per customer app. This is where most of the "batteries included" story lives. Core stays lean; customer apps opt into the domain packages they actually need.

## Module Installation Model

Installing a module into a customer app should be an explicit composition step, not an ambient side effect of dropping code into a directory. A module installation normally includes:

- registering the module in the customer app
- applying its migrations
- enabling its routes, admin contributions, and background workers
- binding its required capabilities into the active auth model
- validating its configuration and integration secrets

This makes module adoption observable and reversible.

## Composition Rules

Modules must compose through stable platform contracts:

- core services such as auth, cache, storage, SEO, i18n, and observability
- capability contracts instead of hard-coded relation names
- documented extension points rather than hidden internal hooks

That rule is especially important for auth. A CMS or events module must ask for capabilities like `cms.page.publish` or `events.booking.manage`, not assume the default auth model's relation names. Otherwise customer apps cannot genuinely extend or replace the shipped model.

## Typical Module Sets

Different customer apps will take different subsets. A commerce-heavy customer may install catalog, checkout, orders, payments, and CMS. The current business context likely adds:

- memberships or subscriptions
- events
- timeslots, reservations, and bookings
- branded admin and reporting modules

The point of modularity is not unlimited combinatorics. It is controlled choice among supportable packages.

## Supportability Boundaries

To stay supportable, official modules should declare:

- their module dependencies
- the core services they require
- migration order
- capability bindings they expect
- any extension slots they expose

Customer apps can mix modules, but only along those documented contracts. If a combination requires internal patching or undocumented override behavior, it is no longer a clean composition.

## Upgrade Path

Because modules are native and versioned separately from core, upgrades should be pinned and deliberate. A customer app should be able to move core, official modules, and WASM extensions on different cadences, provided compatibility checks and migration requirements are satisfied. That separation is what lets the platform ship real batteries without recreating monolithic bloat.
