---
title: Core Concepts
---

This section is the architecture narrative for Rust web developers who want to understand how Coil is put together.

Each page focuses on one concept and answers five questions:

- what it is
- why it exists
- how it works
- what people commonly get wrong
- what to read next

## How To Use This Chapter

Use **Core Concepts** when you want the architectural model first.

- Start here if you are asking "how is Coil shaped?"
- Switch to **Reference** when you need exact fields, APIs, or supported surfaces.
- Switch to **Use Cases** when you want to see how the concepts appear in a real customer app.
- Switch to **Operations** when you are ready to run or ship an app rather than just understand its design.

In practice, most readers should move through these sections in a loop:

1. read the concept page
2. open the matching Shoppr or Gitly example
3. jump to the exact reference or operations page when you need mechanics

## Where To See This In Practice

- Shoppr is the main reference app for multi-site commerce, CMS, checkout, and admin:
  [Shoppr overview](../use-cases/shoppr/overview.md)
- Gitly is the main reference app for a non-commerce product shape, linked Rust, and bounded WASM:
  [Gitly overview](../use-cases/gitly/overview.md)

If you want the exactness layer while reading:

- [Reference overview](../reference/overview.md)
- [Composition and coil](../reference/composition.md)
- [Official modules](../reference/modules.md)

Recommended order:

1. [Glossary and mental model](glossary-and-mental-model.md)
2. [Customer-root workspace](customer-root-workspace.md)
3. [Runtime and module composition](runtime-and-module-composition.md)
4. [Request and render lifecycle](request-and-render-lifecycle.md)
5. [Sites, locales, and markets](sites-locales-and-markets.md)
6. [Customer apps vs official modules](customer-apps-vs-official-modules.md)
7. [Themes, rendering, and assets](themes-rendering-and-assets.md)
8. [Internationalisation, localisation, and content](internationalization-localization-and-content.md)
9. [Accessibility as a platform contract](accessibility-as-a-platform-contract.md)
10. [SEO and discoverability](seo-and-discoverability.md)

If you prefer to start from a running app first, go back to the [Quickstart](../getting-started/quickstart.md) and return here after you have looked at Shoppr or Gitly.

## Suggested Follow-On Paths

Choose one of these after the concept chapter:

- Product composition path:
  [Customer project layout](../getting-started/customer-project-layout.md),
  [Composition and coil](../reference/composition.md),
  [Shoppr overview](../use-cases/shoppr/overview.md)
- Runtime and request path:
  [Request and render lifecycle](request-and-render-lifecycle.md),
  [Theme structure](../reference/theme-structure.md),
  [Build and deploy](../operations/build-and-deploy.md)
- Extension and customization path:
  [Linked Rust backends](../getting-started/linked-rust-backends.md),
  [Customer Rust vs third-party WASM](../reference/customer-vs-wasm.md),
  [Gitly overview](../use-cases/gitly/overview.md)
