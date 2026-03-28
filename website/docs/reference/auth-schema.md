---
title: Auth Schema
---

The auth schema is the part of an auth package that defines:

- resource types
- relations
- derived permissions

In Davenda, that schema lives in `model.auth`.

## Why The Schema Exists

The schema defines what authorization means for one deployment.

It answers questions like:

- what kinds of resources can appear in auth checks?
- which relations can be stored directly?
- which permissions are derived from those relations?

Tuple storage alone cannot answer that. The tuple engine needs semantic rules.

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

## Common Mistakes

- Treating schema permissions as the same thing as module capabilities.
- Assuming arbitrary relation names will load because the design docs discuss replaceable semantics. The current file-backed implementation is still bounded.
- Writing complex permission expressions in `model.auth` when the current loader only supports single-source assignments.
- Changing schema relation names when the real module contract lives in `capabilities.toml`.
