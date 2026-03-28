---
title: Custom Auth Schema Guidance
---

Custom auth schemas are supported by design, but they need to be done with discipline.

This page explains the safe path.

## Start With The Real Question

Before changing auth, decide which of these problems you actually have:

- you need one or two extra customer-specific permissions
- you need extra resources or operator roles
- the default relation graph is fundamentally wrong for the deployment

Those are different problems, and they should not all lead to a full replacement.

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

## How To Keep A Custom Schema Safe

Rules that matter:

- keep official modules bound to stable capabilities, never to your custom relation names
- change as little as possible
- version storage, schema semantics, and capability bindings separately
- ship explainable tests for important decisions
- fail validation early if installed modules do not have the capability bindings they need

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
