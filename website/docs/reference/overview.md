---
title: Reference Overview
---

The reference section is where Davenda is documented generically rather than through a specific product lens.

Use it when you need:

- module boundaries
- composition guidance
- extension model rules
- platform vocabulary
- app manifest rules
- runtime config contracts
- auth package and schema guidance

## How To Use This Section

The reference section is the exactness layer of the docs.

Use it when you already know roughly what you are trying to do and need the concrete contract:

- field names
- supported values
- structure rules
- configuration examples
- packaging rules
- integration boundaries

If you are new to the idea, start in:

- [Core Concepts](../core-concepts/index.md)

If you want a worked example, jump to:

- [Shoppr use cases](../use-cases/shoppr/overview.md)
- [Gitly use cases](../use-cases/gitly/overview.md)

## What Lives Here

### Product And Runtime Configuration

- [app.toml](./app-toml.md)
- [platform.toml And platform.dev.toml](./platform-config.md)

Use these when you need to know exactly which blocks exist, what keys are supported, and how product composition differs from runtime operations.

### Auth

- [Auth overview](./auth-overview.md)
- [Zanzibar and Davenda auth](./auth-zanzibar.md)
- [Auth schema](./auth-schema.md)
- [Auth packages](./auth-packages.md)
- [Custom auth schema guidance](./custom-auth-schema.md)

Use this subsection when you need to understand how Davenda expresses authorisation and how to extend or replace the shipped auth model.

### Rendering, Themes, Locales, Accessibility, And SEO

- [Template Language](./template-language.md)
- [Theme Structure](./theme-structure.md)
- [Internationalisation](./internationalization.md)
- [Accessibility](./accessibility.md)
- [SEO](./seo.md)

Use these when you need the exact template syntax, theme contract, locale wiring, accessibility expectations, or SEO controls.

### Official Batteries And Composition

- [Official modules](./modules.md)
- [Composition and davenda-all](./composition.md)
- [Customer Rust vs third-party WASM](./customer-vs-wasm.md)

Use these when you need to know what the official batteries provide, how to compose them, and how linked customer code differs from runtime-installed extensions.

## Practical Reading Strategy

When you are implementing something, the most useful order is usually:

1. find the relevant use-case page
2. read the corresponding reference page
3. return to the Shoppr or Gitly example
4. apply the pattern in your own app

That is how the docs are intended to work together:

- concepts explain the model
- use cases show the model in a real app
- reference pages define the exact contract

## Deep Internals

For architectural internals, invariants, and design history, continue into the architecture section under `docs/design`.
