# Site-Scoped Inventory And Catalog Availability

**Part:** Commerce  
**Chapter:** 98

## Status

Accepted.

## Decision

Coil models site-scoped commerce availability directly in the customer app storefront catalog.

The first production slice supports:

- site-scoped collection visibility
- site-scoped product visibility
- declarative inventory location metadata on products

This gives customer apps a practical way to express:

- products available only in one site
- region-specific assortment differences
- location metadata that can later back richer fulfillment rules

## Why

For many real stores, the first multi-site problem is not full warehouse optimization. It is simpler and more immediate:

- UK sells one subset
- DE sells another subset
- US runs event-linked drops unavailable elsewhere

Customers need that modeled before more advanced inventory allocation or fulfillment routing exists.

## Model

### Collections

Storefront collections may declare `site_ids`.

If omitted, the collection is visible to all sites.

If present, the collection is visible only when the current request site is included.

### Products

Storefront products may declare:

- `site_ids`
- `inventory_locations`

`site_ids` controls where the product is purchasable or publicly visible.

`inventory_locations` is metadata describing the stock pool or origin locations that back the offer. In this slice it is surfaced for rendering, auditability, and future fulfillment logic, but it does not yet implement full reservation or split-shipment planning.

### Runtime Behavior

The runtime must:

- filter visible collections and products by current site
- reject add-to-cart flows for products not available on the current site
- preserve site-aware visibility in product detail, collection detail, and catalog surfaces

## Why This Boundary

This is intentionally narrower than a full inventory engine.

It solves the first real customer need:

- one app
- multiple sites
- different products and campaign surfaces per site

without blocking on:

- distributed stock reservation
- advanced warehouse routing
- multi-origin shipping calculations

## Follow-On Work

Later work may extend this into:

- site-specific price lists
- per-site payment and shipping policy
- fulfillment origin selection
- stock reservation by location
- event capacity inventory tied to site and venue

Those extensions must build on the same site-scoped availability model instead of replacing it.
