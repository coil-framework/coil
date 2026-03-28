---
title: Configuration And Secrets
---

Davenda separates customer composition, operational configuration, and secrets on purpose.

That split is one of the platform’s core safety properties.

## The Three Inputs

Most production problems become easier to reason about if teams keep these three inputs distinct:

### Customer App Manifest

The manifest describes product composition:

- app identity
- installed modules
- site and locale policy
- theme configuration
- linked customer backend and runtime-installed extension choices

This file should usually be committed and reviewed as product code.

### Platform Runtime Config

Runtime config describes deployment and operational controls:

- server bind and trusted proxy settings
- database, cache, jobs, and storage backends
- TLS mode and provider details
- observability controls
- asset publication settings
- payment/provider wiring

This should also be committed, but environment-specific values may vary across dev, staging, and
production configs.

### Secrets

Secrets resolve sensitive values at runtime:

- database URLs
- object-store credentials
- API keys
- webhook secrets
- payment provider secrets
- certificate automation credentials

Secrets should not be hard-coded in committed manifests or templates.

## Why This Separation Matters

Without this boundary, teams tend to create one giant config file that mixes:

- product behavior
- environment topology
- secrets
- temporary operational hacks

That makes review harder, drift easier, and incident recovery slower.

## Configuration Strategy

A strong Davenda setup should have:

- a committed app manifest
- a committed development platform config
- a committed production-shaped platform config
- environment-provided secrets
- explicit validation in CI and before deploy

The goal is reproducibility, not convenience by omission.

## Secret Resolution Guidance

Preferred secret sources are:

- environment variables
- a secret manager exposed to the runtime environment
- deployment tooling that writes environment-specific secret material at runtime

Avoid:

- committing secrets into the repo
- embedding secrets in templates or frontend assets
- hiding production-only secrets in undocumented local files

## Validation Expectations

Run validation whenever configuration changes:

- during CI
- before image or binary promotion
- before migrations
- before cutover

Validation should confirm:

- app manifest and runtime config align
- required modules are configured coherently
- locale and site policy is internally consistent
- required providers and secrets are present for enabled features
- unsafe storage or TLS modes are rejected outside intended environments

## Environment Parity

Development and production do not need identical infrastructure, but they should preserve the same
behavioral model.

In practice that means:

- same installed module set
- same auth package identity
- same routes and templates
- same job and webhook surfaces
- same storage and payment model, where feasible

Do not create a development config that hides the exact integration boundaries operators will have
to manage later.

## Rotation And Change Management

Production teams should treat secret rotation and config changes as planned operations.

At minimum:

- rotate provider credentials deliberately
- keep old and new credentials coordinated during transition windows when required
- record who changed production config and why
- validate before and after the change

If config changes are ad hoc and untracked, the platform may still work, but operations will not
be production-ready.

## Recommended Review Questions

For any config or secret-related change, ask:

- does this belong in app manifest, platform config, or secret storage
- does it change product behavior, operational behavior, or credentials
- does it need validation in CI
- does it need an operator playbook entry
- can a rollback restore the previous state cleanly

Those questions keep configuration from becoming an unowned risk surface.
