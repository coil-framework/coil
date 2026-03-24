# Commerce Modules

**Part:** Native Batteries  
**Chapter:** 54

Commerce is shipped as a set of native modules rather than as a single inseparable product blob. That keeps the framework from becoming "the store" while still giving customer apps a supported first-party commerce distribution when they need one.

## Commerce Distribution
The baseline commerce modules cover:

- catalog and collection management
- pricing, promotions, vouchers, bundles, taxes, and currencies
- cart and checkout flows
- orders, refunds, and returns
- payment and shipping integration points
- customer account and address flows that belong to the buying journey

This distribution is designed to compose with CMS, memberships, events, and admin rather than sitting beside them awkwardly. A customer app may use all of it or only selected pieces.

## Integration with Core
Commerce depends heavily on core guarantees. Transactions, idempotent job handling, auth capabilities, storage, i18n, SEO primitives, and cache invalidation are all host concerns. Product content should inherit locale-aware routing and localized metadata. Checkout and order flows rely on the data layer and job system rather than custom module-local infrastructure. Payment logic may be extended, but settlement-critical behavior is not delegated to arbitrary sandbox code.

## Extension and Customization
The platform explicitly allows extension points around the edges of commerce:

- payment or shipping adapters
- pricing and promotion calculators
- storefront fragments and render hooks
- search and reporting integrations

Those are good places for customer-specific logic or WASM-based integrations. The core order and checkout model, however, stays native because it needs strong transactional semantics and debuggability.

A practical example is the reference customer that combines commerce with memberships and events. The commerce modules handle products, pricing, checkout, and orders. Membership and events modules layer on top of that foundation to express recurring entitlements, bookings, and event-specific rules. The result is a modular product stack, not one monolith pretending to be a framework.
