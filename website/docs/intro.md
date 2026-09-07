---
title: Introduction
---

Coil is a Rust web framework for teams that want the framework, the product shape, and the deployment model to line up.

It is not a generic bag of middleware. It starts from the assumption that most real products need a coherent answer to the same questions:

- where application code lives
- how public pages, account areas, and admin surfaces fit together
- how product-specific code differs from reusable batteries
- how multi-site, locale, auth, and operational concerns are carried through the whole runtime

That makes Coil a better fit for Rust web developers who want to build a product, not just wire an HTTP stack.

## What Coil Is

Coil gives you:

- a customer-root workspace model, where the customer binary owns composition
- Fission SSR with focused islands and full Web applications where the workflow requires them
- first-party official modules for common product batteries such as CMS, commerce, memberships, admin, media, and ops
- a stable linked Rust path for customer-owned backend logic
- a bounded WASM path for third-party or lower-trust extensions

In practice, that means a Coil application usually looks like a product codebase with a clear runtime model, not a thin shell around a collection of unrelated libraries.

## What Coil Is Not

Coil is probably the wrong choice if you want:

- a minimal unopinionated HTTP toolkit
- a framework where routing, rendering, and product structure are entirely ad hoc
- a plugin model where third-party code runs with the same trust level as the core runtime
- a frontend-first architecture where the browser owns most application behaviour

## How To Read These Docs

If you are evaluating Coil for the first time:

1. Start with the [Quickstart](/docs/getting-started/quickstart/).
2. Read the [Core Concepts overview](/docs/core-concepts/).
3. Follow the concept pages in order if you want the full mental model.
4. Use Shoppr and Gitly as concrete reference applications while reading.

If you are already comfortable with Rust web stacks and want the shortest path to understanding Coil's architecture:

1. Read [Glossary and mental model](/docs/core-concepts/glossary-and-mental-model/).
2. Read [Customer-root workspace](/docs/core-concepts/customer-root-workspace/).
3. Read [Runtime and module composition](/docs/core-concepts/runtime-and-module-composition/).
4. Read [Request and render lifecycle](/docs/core-concepts/request-and-render-lifecycle/).

## Two Example Apps

The repo currently uses two reference applications to make the model concrete:

- `apps/shoppr` shows the commerce-oriented path: storefront, cart, checkout, CMS, admin, and operations.
- `apps/gitly` shows the same platform shape applied to a non-commerce product.

Those apps are not marketing demos bolted onto the side. They are the fastest way to see how the runtime, modules, templates, and customer-owned Rust code fit together.

## Where To Go Next

- [Quickstart](/docs/getting-started/quickstart/) for a practical first run
- [Customer project layout](/docs/getting-started/customer-project-layout/) for the repository shape
- [Linked Rust backends](/docs/getting-started/linked-rust-backends/) for the supported customization model
- [Core Concepts](/docs/core-concepts/) for the architecture narrative
