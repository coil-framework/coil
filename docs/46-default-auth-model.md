# Default Auth Model

**Part:** Authorization and Security  
**Chapter:** 46

The framework ships with a default auth model because most installations need a usable starting point, not a blank policy engine. That model is intentionally broad enough to cover the platform's common objects while still remaining a package layered on top of core rather than an unchangeable part of it.

## Shape of the Shipped Model
The baseline model covers platform scope, actors, and the resource families that first-party modules need most often:

- scope resources such as tenant, site, brand, and storefront
- actor resources such as user, group, team, and service account
- content and commerce resources such as page, product, collection, order, media, asset, and admin module
- first-party vertical resources for events, bookings, memberships, and related operational objects where the reference product requires them

The default relation vocabulary stays intentionally small: owner, admin, editor, viewer, support, and member are the common building blocks. Derived permissions then express platform actions such as view, edit, publish, manage, checkout, refund, book, check in, and manage storage. Modules consume those semantics through capability bindings, not through direct relation names.

## Capability Coverage
Out of the box, the default model is expected to satisfy the stable capability contracts exposed by first-party modules. Examples include:

- `cms.page.read` and `cms.page.publish`
- `catalog.product.edit`
- `admin.users.manage`
- `asset.read`, `asset.publish`, and `asset.manage_storage`
- membership and booking capabilities used by the subscriptions and events modules

This gives the official module set a coherent baseline without forcing customer apps to adopt one fixed org chart forever. A site with a simple editorial team can use the defaults directly. A more complex operator can import the model, add resource types and relations, and still keep the same capability surface for modules.

## What the Default Model Is Not
The shipped model is not a statement that every deployment has the same tenant hierarchy, approval flow, or team structure. It is the reference package that proves the engine is usable and gives new customer apps a sensible path to first launch. Core remains the runtime. The model remains replaceable. That separation is what keeps "batteries included" from turning into hard-coded policy.
