---
title: Custom Auth Schema Guidance
---

Custom auth schemas are supported by design, but the safe path is narrower than "write whatever policy graph you want."

Start with the smallest useful customization:

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

That is a good first custom schema because it:

- changes one business rule
- keeps first-party capability names stable
- preserves the default package as the base
- is easy to validate and explain

## When You Should Use This Page

Use this page when:

- the default auth package is close but not quite right
- you need one customer-specific permission path
- you are deciding whether to extend or replace the default model
- you want a practical walkthrough instead of only conceptual guidance

## Start With The Real Question

Before changing auth, decide which of these problems you actually have:

- you need one or two extra customer-specific permissions
- you need extra resources or operator roles
- the default relation graph is fundamentally wrong for the deployment

Those are different problems, and they should not all lead to a full replacement.

## The Safest Davenda Path Today

Today, the safest practical path is:

1. keep the shipped default package as the base
2. extend it with one small schema change
3. bind one stable capability to that new permission
4. validate and explain the result

That is the pattern the checked-in Shoppr package already demonstrates.

## Preferred Path: Extend

For most customer apps, the correct first move is to extend the default auth package.

Use `extend` when:

- first-party module capability coverage should remain intact
- you need a few extra relations or permissions
- your customer app adds one domain-specific concept

Example pattern:

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

This is the safest custom-schema shape because it preserves first-party capability contracts while adding customer semantics.

## When Full Replacement Is Justified

Use `replace` only when:

- the default scope/resources are not an acceptable starting point
- approval chains or organization structure are fundamentally different
- you can supply complete capability bindings for every installed module

Full replacement is a product decision, not a small customization.

## Current Implementation Constraint

The design supports both `extend` and `replace`.

The current file-backed auth loader is more limited:

- it supports the shipped default package
- it supports file-backed `extend` packages over one imported base package
- it does not yet support file-backed `replace` packages on the same path

So the practical guidance today is:

- prefer `extend`
- treat `replace` as a deliberate future-facing contract unless your runtime path fully supports it

## Practical Walkthrough: Add One Customer-Specific Permission

This is the normal workflow for a bounded customization.

### 1. Decide the capability you need

Example:

- you want a merchandising team to edit featured catalog presentation
- the stable capability name is `catalog.featured.edit`

### 2. Add the schema rule

In `model.auth`:

```text
type product
  relations
    merchandiser: user | group#member
  permissions
    featured_edit = merchandiser
```

### 3. Bind the capability

In `capabilities.toml`:

```toml
[bindings."catalog.featured.edit"]
resource_type = "product"
permission = "featured_edit"
```

### 4. Select the package in the app

In `app.toml`:

```toml
[auth]
mode = "extend"
package = "shoppr-auth"
```

And in platform config:

```toml
[auth]
package = "shoppr-auth"
explain_api = false
tenant_id = 101
```

### 5. Validate before shipping

The operational rule is simple:

- do not wait to discover auth mistakes through a broken admin or storefront flow
- validate and inspect the package first

### 6. Explain a decision when behaviour is unclear

If the grant path is surprising, use auth explain tooling rather than guessing from relation names.

### 7. Stop if the change starts to sprawl

If you find yourself needing:

- many renamed first-party permissions
- many replaced base relations
- a new organization model across most resources

you are no longer doing a small extension. Re-evaluate whether you are planning a real replacement package.

## How To Keep A Custom Schema Safe

Rules that matter:

- keep official modules bound to stable capabilities, never to your custom relation names
- change as little as possible
- version storage, schema semantics, and capability bindings separately
- ship explainable tests for important decisions
- fail validation early if installed modules do not have the capability bindings they need

## What A Good First Custom Schema Looks Like

Good first custom schemas are:

- small
- capability-oriented
- specific to one business rule
- easy to explain
- easy to remove if the requirement changes

## What Not To Do

Bad pattern:

```text
type page
  relations
    custom_role_a: user
    custom_role_b: user
    custom_role_c: user
  permissions
    publish = custom_role_a
    edit = custom_role_b
    read = custom_role_c
```

Why this is a poor first customization:

- it throws away the default model too early
- it introduces local names with no clear capability story
- it increases migration and explain complexity immediately

Prefer small additions over broad rewrites.

## What To Customize First

Good first customizations:

- a merchandising role for featured catalog editing
- a compliance or approval role layered onto publishing
- a customer-specific resource type used only by linked customer logic

Bad first customizations:

- renaming large parts of the default relation vocabulary without a real need
- replacing the entire model before you know which module capabilities your app needs
- copying default capabilities under new names and assuming official modules will switch over

## Common Mistakes

- Using custom relations where a new capability binding is the real requirement.
- Replacing the model to add one small customer-specific permission.
- Forgetting that installed modules still need their published capability contracts satisfied.
- Assuming design-level replaceability means every runtime loader path already supports arbitrary custom schemas today.

## Decision Rule

If you can express the requirement by:

- adding one resource type
- adding one relation
- adding one derived permission
- binding one extra capability

use `extend`.

If the deployment needs a genuinely different organization model across the whole app, then plan a full replacement and treat it as a major auth change, not a small customization.

## Full Implementation

The canonical checked-in example is:

- `apps/shoppr/app.toml`
- `apps/shoppr/platform.toml`
- `apps/shoppr/platform.dev.toml`
- `apps/shoppr/auth/shoppr-auth/package.toml`
- `apps/shoppr/auth/shoppr-auth/model.auth`
- `apps/shoppr/auth/shoppr-auth/capabilities.toml`

## Read Next

- [Auth Packages](./auth-packages.md)
- [Auth Schema](./auth-schema.md)
