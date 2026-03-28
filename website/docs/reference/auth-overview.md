---
title: Auth
---

Auth in Coil is the contract between:

- the core relationship engine
- the app-selected auth package
- official module capability checks
- customer-specific authorisation semantics

Start with this concrete example:

```toml
# app.toml
[auth]
mode = "extend"
package = "shoppr-auth"
```

```text
# model.auth
type product
  relations
    merchandiser: user | group#member
  permissions
    featured_edit = merchandiser
```

```toml
# capabilities.toml
[bindings."catalog.featured.edit"]
resource_type = "product"
permission = "featured_edit"
```

That means:

- the app selects an auth package
- the package defines a schema rule
- the package binds a stable capability to that rule
- modules ask for `catalog.featured.edit`, not for `merchandiser` directly

That is the whole Coil auth model in miniature.

Coil auth has four layers:

- the Zanzibar-style core engine
- the auth package
- the auth schema and capability bindings
- runtime selection in platform config

## What This Section Covers

Use the auth reference when you need to understand:

- what `auth.package = "shoppr-auth"` actually selects
- what lives in `package.toml`, `model.auth`, and `capabilities.toml`
- how capabilities relate to relations and permissions
- when a customer app should extend the default auth model instead of replacing it
- which parts of the system are stable contracts and which parts are app-specific semantics

## When To Read Which Page

Start here:

1. [Zanzibar And Core Auth](./auth-zanzibar.md)
   - read this if you need the mental model
2. [Auth Packages](./auth-packages.md)
   - read this if you need to know the file layout and package contract
3. [Auth Schema](./auth-schema.md)
   - read this if you are editing `model.auth` or `capabilities.toml`
4. [Custom Auth Schema Guidance](./custom-auth-schema.md)
   - read this if you need to extend or eventually replace the default model

## Practical Rule

One rule matters throughout:

Official modules depend on capabilities, not relation names.

That rule is what makes custom auth models real instead of cosmetic.

## Working Mental Model

If you remember only one sequence, remember this one:

1. core stores and evaluates relationships
2. the auth package defines the schema and bindings
3. the app selects the package
4. modules ask for capabilities
5. the active package decides how those capabilities are satisfied

## What Lives Where

Use this split:

- core auth engine
  - stores tuples
  - executes checks
  - explains decisions
- auth package
  - defines resource types, relations, permissions, and bindings
- app config
  - selects which package runs
- official modules
  - consume stable capabilities

If a question is "who owns the meaning of this permission?", the answer is almost always "the auth package", not the module.

## Full Implementation

Canonical Shoppr example files:

- `apps/shoppr/app.toml`
- `apps/shoppr/platform.toml`
- `apps/shoppr/platform.dev.toml`
- `apps/shoppr/auth/shoppr-auth/package.toml`
- `apps/shoppr/auth/shoppr-auth/model.auth`
- `apps/shoppr/auth/shoppr-auth/capabilities.toml`

## Read Next

- [Zanzibar And Core Auth](./auth-zanzibar.md)
- [Auth Packages](./auth-packages.md)
