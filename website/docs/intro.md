---
title: Introduction
---

Davenda is a Rust web framework for teams that want the framework, the product shape, and the deployment model to line up.

It is not a generic bag of middleware. It starts from the assumption that most real products need a coherent answer to the same questions:

- where application code lives
- how public pages, account areas, and admin surfaces fit together
- how product-specific code differs from reusable batteries
- how multi-site, locale, auth, and operational concerns are carried through the whole runtime

That makes Davenda a better fit for Rust web developers who want to build a product, not just wire an HTTP stack.

## What Davenda Is

Davenda gives you:

- a customer-root workspace model, where the customer binary owns composition
- HTML-first rendering with progressive enhancement
- first-party official modules for common product batteries such as CMS, commerce, memberships, admin, media, and ops
- a stable linked Rust path for customer-owned backend logic
- a bounded WASM path for third-party or lower-trust extensions

In practice, that means a Davenda application usually looks like a product codebase with a clear runtime model, not a thin shell around a collection of unrelated libraries.

## What Davenda Is Not

Davenda is probably the wrong choice if you want:

- a minimal unopinionated HTTP toolkit
- a framework where routing, rendering, and product structure are entirely ad hoc
- a plugin model where third-party code runs with the same trust level as the core runtime
- a frontend-first architecture where the browser owns most application behaviour

## How To Read These Docs

If you are evaluating Davenda for the first time:

1. Start with the [Quickstart](getting-started/quickstart.md).
2. Read the [Core Concepts overview](core-concepts/index.md).
3. Follow the concept pages in order if you want the full mental model.
4. Use Shoppr and Gitly as concrete reference applications while reading.

If you are already comfortable with Rust web stacks and want the shortest path to understanding Davenda's architecture:

1. Read [Glossary and mental model](core-concepts/glossary-and-mental-model.md).
2. Read [Customer-root workspace](core-concepts/customer-root-workspace.md).
3. Read [Runtime and module composition](core-concepts/runtime-and-module-composition.md).
4. Read [Request and render lifecycle](core-concepts/request-and-render-lifecycle.md).

## Two Example Apps

The repo currently uses two reference applications to make the model concrete:

- `apps/shoppr` shows the commerce-oriented path: storefront, cart, checkout, CMS, admin, and operations.
- `apps/gitly` shows the same platform shape applied to a non-commerce product.

Those apps are not marketing demos bolted onto the side. They are the fastest way to see how the runtime, modules, templates, and customer-owned Rust code fit together.

## Where To Go Next

- [Quickstart](getting-started/quickstart.md) for a practical first run
- [Customer project layout](getting-started/customer-project-layout.md) for the repository shape
- [Linked Rust backends](getting-started/linked-rust-backends.md) for the supported customization model
- [Core Concepts](core-concepts/index.md) for the architecture narrative
