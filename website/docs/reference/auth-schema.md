---
title: Auth Schema
---

The auth schema is the part of an auth package that defines:

- resource types
- relations
- derived permissions

In Coil, that schema lives in `model.auth`.

## When You Should Use This Page

Read this page when you are:

- editing `model.auth`
- adding a relation or permission for a customer app
- trying to understand why a capability binding points to one permission and not another
- checking whether the current loader can express the rule you want

## Why The Schema Exists

The schema defines what authorisation means for one deployment.

It answers questions like:

- what kinds of resources can appear in auth checks?
- which relations can be stored directly?
- which permissions are derived from those relations?

Tuple storage alone cannot answer that. The tuple engine needs semantic rules.

## Where The Schema Lives

In a customer app, the exact files are:

- `apps/<app>/auth/<package>/model.auth`
- `apps/<app>/auth/<package>/capabilities.toml`

The schema and bindings must be read together. `model.auth` without `capabilities.toml` does not tell you what official modules can actually do.

## Current `model.auth` Shape

The current file-backed loader supports a deliberately small syntax:

```text
type product
  relations
    merchandiser: user | group#member
  permissions
    featured_edit = merchandiser
```

Current parsing model:

- `type <resource>`
- `relations`
- relation entries using `<relation>: ...`
- `permissions`
- permission entries using `<permission> = <relation>`

## Required Versus Optional

Required in `model.auth`:

- at least one `type`
- at least one valid `relations` or `permissions` block where needed by the package

Practical expectation:

- every permission you intend modules to use should normally have a matching capability binding in `capabilities.toml`

## What The Current Loader Supports

The current loader supports:

- declaring resource types
- declaring supported relation names
- declaring single-source derived permissions
- extending the shipped default schema with additional rules

The current loader does not support every theoretical Zanzibar-style expression.

Known current limits:

- multi-source permission expressions are rejected
- unsupported relation names are rejected
- file-backed full `replace` mode is not yet supported by the current loader path

That means this page documents both:

- the design intent of the auth schema
- the current implementation boundary you have to stay inside today

## Relation Vocabulary

The current runtime supports a bounded relation vocabulary rather than arbitrary names.

Examples currently recognized by the auth layer include:

- `tenant`
- `site`
- `brand`
- `storefront`
- `member`
- `owner`
- `admin`
- `editor`
- `viewer`
- `support`
- `merchandiser`
- `view`
- `edit`
- `publish`
- `manage`
- `featured_edit`
- `checkout`
- `refund`
- `read`
- `read_public`
- `replace`
- `delete`
- `unpublish`
- `manage_storage`
- `book`
- `check_in`

That is the current implementation boundary, not an abstract promise of arbitrary user-defined relation tokens.

## How To Read A Schema

Use this order:

1. identify the resource type
2. identify the directly stored relations
3. identify derived permissions
4. open `capabilities.toml`
5. check which capabilities map onto those permissions

If you skip step 4, you are looking at internal semantics, not the module-facing contract.

## Schema Versus Capability Bindings

The schema defines permissions such as:

- `publish`
- `featured_edit`
- `refund`

Capability bindings map stable module capabilities onto those permissions.

Example:

- schema permission: `featured_edit`
- capability binding: `catalog.featured.edit -> product#featured_edit`

The schema is the semantic layer. The capability file is the module contract layer.

## Practical Repo Example

Shoppr's package adds one customer-specific rule:

```text
type product
  relations
    merchandiser: user | group#member
  permissions
    featured_edit = merchandiser
```

That schema becomes operational only because `capabilities.toml` binds:

```toml
[bindings."catalog.featured.edit"]
resource_type = "product"
permission = "featured_edit"
```

Without that binding, the schema addition would not help first-party module code.

## Example

`model.auth`:

```text
type product
  relations
    merchandiser: user | group#member
  permissions
    featured_edit = merchandiser
```

`capabilities.toml`:

```toml
[bindings."catalog.featured.edit"]
resource_type = "product"
permission = "featured_edit"
```

That gives customer code or modules a stable capability without exposing them to custom relation names.

## How To Change A Schema Safely

For a normal customer-specific addition:

1. add one relation
2. derive one permission from it
3. bind one capability to that permission
4. validate the package
5. use explain tooling to confirm the grant path is what you intended

Keep changes small. Small auth changes are easier to explain, test, and roll back.

## Common Mistakes

- Treating schema permissions as the same thing as module capabilities.
- Assuming arbitrary relation names will load because the design docs discuss replaceable semantics. The current file-backed implementation is still bounded.
- Writing complex permission expressions in `model.auth` when the current loader only supports single-source assignments.
- Changing schema relation names when the real module contract lives in `capabilities.toml`.

## Read Next

- [Auth Packages](./auth-packages/)
- [Custom Auth Schema Guidance](./custom-auth-schema/)
