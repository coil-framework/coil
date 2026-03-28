---
title: Auth Packages
---

An auth package is the deployable unit of authorization semantics in Davenda.

It packages:

- auth package metadata
- the auth schema
- capability bindings
- migrations and seeds owned by the auth layer

It does not replace the core auth engine. Core still owns tuple storage, checks, explanation, and query execution.

## Why Auth Packages Exist

Davenda separates three concerns that are often collapsed together:

- tuple storage
- authorization semantics
- module capability contracts

The package exists so a customer app can extend or replace auth semantics without forking official modules or modifying the engine.

## When You Should Care About This Page

Read this page when you are:

- selecting an auth package in `app.toml`
- creating a new customer auth package under `auth/<package>/`
- versioning auth changes for release
- trying to understand what is package-owned versus engine-owned
- validating whether a customer-specific auth change belongs in schema, bindings, seeds, or migrations

## Package Layout

Typical layout:

```text
auth/
  shoppr-auth/
    package.toml
    model.auth
    capabilities.toml
    migrations/
    seeds/
```

The current docs/design also reserve `tests/` for package-owned auth decision tests.

## Required Versus Optional Files

Required package files:

- `package.toml`
- `model.auth`
- `capabilities.toml`

Optional but normal directories:

- `migrations/`
- `seeds/`
- `tests/`

Practical rule:

- if the package changes auth semantics, it should at least have the first three
- if the package needs bootstrap tuples or auth-owned evolution, add migrations and seeds

## `package.toml`

`package.toml` describes package identity and version boundaries.

Current manifest fields:

- `name`
- `version`
- `mode`
- `storage_schema_version`
- `model_version`
- `capability_binding_version`
- `imports`

Example:

```toml
name = "shoppr-auth"
version = "0.1.0"
mode = "extend"
storage_schema_version = 1
model_version = 1
capability_binding_version = 1
imports = ["platform-default-auth"]
```

What the version fields mean:

- `storage_schema_version`: tuple-storage-facing version boundary
- `model_version`: relation/permission semantic version boundary
- `capability_binding_version`: capability-to-schema mapping version boundary

Keep them separate. A binding change is not the same thing as a storage change.

## Exact Repo Example

Shoppr's checked-in auth package is:

```text
apps/shoppr/auth/shoppr-auth/
  package.toml
  model.auth
  capabilities.toml
  migrations/
  seeds/
```

That is the canonical repo example for a customer-specific extending package.

## `mode`

Current package mode values:

- `extend`
- `replace`

Semantics:

- `extend`: import a base package and add or refine customer-specific schema/bindings
- `replace`: define a fully custom package with its own complete capability coverage

Important current implementation note:

The file-backed auth loader in the current runtime supports the shipped default package plus `extend` mode packages. It currently rejects file-backed `replace` mode packages.

That means `replace` is part of the design contract, but not yet fully supported by the current loader path.

## `imports`

`imports` names the base package(s) an extending package builds on.

Current loader constraint:

- an extend-mode package must import exactly one base package
- multiple imported base packages are rejected by the current loader

## How Packages Are Used In Davenda

The normal path is:

1. declare the package in `app.toml`
2. select the same package at runtime in `platform.toml`
3. validate or inspect it with auth tooling
4. run the app with that package active

In practice that means these files move together:

- `apps/<app>/app.toml`
- `apps/<app>/platform.toml`
- `apps/<app>/platform.dev.toml`
- `apps/<app>/auth/<package>/...`

If those disagree, the app/runtime contract is inconsistent.

## `model.auth`

`model.auth` defines resource types, relations, and derived permissions.

See [Auth Schema](./auth-schema.md) for the syntax and constraints.

## `capabilities.toml`

`capabilities.toml` maps stable capability names to the auth schema.

Example:

```toml
[bindings."catalog.featured.edit"]
resource_type = "product"
permission = "featured_edit"
```

This is the layer that official modules depend on. Modules do not inspect relation names from `model.auth`.

## Practical Workflow

For a normal customer-specific addition:

1. start from an extending package
2. add or refine schema in `model.auth`
3. bind the needed capability in `capabilities.toml`
4. add bootstrap or migration material only if the change needs it
5. validate the package before deployment

This is the safer path because capability bindings keep official modules stable while letting customer apps change policy.

## `migrations/` and `seeds/`

Use these for auth-owned bootstrap and evolution:

- schema-bearing auth changes
- initial tuple/bootstrap state
- auth-specific data backfills

Do not use them as a general-purpose app migration bucket.

## When To Extend Vs Replace

Choose `extend` when:

- the default platform concepts are mostly right
- you need extra roles or extra resources
- you want to preserve default capability coverage and add customer-specific behavior

Choose `replace` only when:

- the default relation graph is materially wrong for the deployment
- you are prepared to supply complete capability coverage for installed modules
- you can own the migration and operational cost of replacing the model

## Common Working Example

A small package-level customization usually looks like this:

```toml
# package.toml
name = "shoppr-auth"
version = "0.1.0"
mode = "extend"
storage_schema_version = 1
model_version = 1
capability_binding_version = 1
imports = ["platform-default-auth"]
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

That is the shape to copy first unless you have a strong reason to do something more invasive.

## Common Mistakes

- Treating the package as a role list instead of a full semantic boundary.
- Changing relation names and assuming official modules will notice. They will not; they look at capabilities.
- Using `replace` without planning full capability coverage for installed modules.
- Bumping all three package version numbers together by habit instead of by actual change type.
- Assuming the current file-backed loader supports every design-time package feature. It does not yet.

## Read Next

- [Auth Schema](./auth-schema.md)
- [Custom Auth Schema Guidance](./custom-auth-schema.md)
