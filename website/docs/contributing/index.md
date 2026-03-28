---
title: Contributing
---

Davenda welcomes contributions, but the bar is product and architecture quality, not raw activity.

This page is the docs-site version of the contribution contract. It should be enough for a new
contributor to understand how to propose, build, and ship a useful change.

## What Good Contributions Look Like

Strong contributions usually improve at least one of these:

- product credibility
- architectural clarity
- operational safety
- test coverage for a real boundary
- documentation that removes ambiguity for adopters
- demo apps that make the platform easier to understand

Examples:

- closing a real platform gap with tests
- tightening a customer-root lifecycle story
- expanding public docs with file-grounded examples
- improving an official module boundary without adding a hidden escape hatch

## Before You Start

Read these in order:

1. the relevant chapter in `docs/design/`
2. the closest demo app, usually `apps/shoppr` or `apps/gitly`
3. the root `CONTRIBUTING.md`
4. the reference docs for the subsystem you want to change

Do not start from code alone if the change affects:

- auth
- storage
- cache
- rendering
- linked Rust and WASM boundaries
- customer-root lifecycle
- publish, release, or cutover workflows

Those areas need an explicit architectural argument, not just a patch.

## Development Workflow

### Core workspace

```bash
cargo test --workspace
```

### Shoppr

```bash
cd apps/shoppr
./scripts/prepare-local-dev.sh
cargo test --workspace
```

### Gitly

```bash
cd apps/gitly
./scripts/prepare-local-dev.sh
cargo test --workspace
```

### Public docs

```bash
cd website
npm install
npm run build
```

If your change affects a demo app, run that app's relevant tests too. If your change affects docs,
build the docs site.

## Pull Request Expectations

A good PR should:

- stay focused on one real problem
- explain why the change exists
- call out tests run
- mention intentional tradeoffs or deferred work
- update docs when the behavior or story changed

Avoid:

- mixing unrelated refactors into one patch
- adding escape hatches without a design reason
- changing a demo app in a way that weakens the teaching story

## Standards Reviewers Will Apply

Reviews prioritize:

- architectural coherence
- security and operational safety
- maintainability over cleverness
- honesty of the demo apps
- clarity of customer-vs-core-vs-WASM boundaries
- quality of docs and tests

This means “works locally” is not enough if the change weakens one of those boundaries.

## When To Open An Issue Or Discussion First

Please discuss before opening a large PR that reshapes:

- the extension model
- auth semantics
- storage or cache contracts
- customer app composition
- runtime and rendering architecture
- cutover, release, or lifecycle commands

Those are platform-shaping changes and need a written argument.

## Issue Guidance

When filing an issue, include:

- what you were trying to do
- the exact files, commands, or docs pages involved
- whether the problem is product behavior, docs clarity, or a demo gap
- the smallest credible fix you can see

The best issues are concrete and reproducible. “This area feels wrong” is usually too vague to be
actionable.

## Docs Contributions

Docs work is first-class work in Davenda.

Good docs changes:

- explain both why and how
- cite exact files and config keys
- use Shoppr or Gitly as the concrete example
- remove repo archaeology for the next developer

The current public-docs backlog lives in `docs/public-docs-expansion-backlog.md`.

## Security And Community

For conduct expectations, follow the repository code of conduct.

For security reporting, follow the repository security policy instead of opening a public exploit
issue.

## Licensing

By contributing to Davenda, you agree that your contributions are licensed under the repository
license.

## Read Next

- [Official Modules](../reference/modules.md)
- [Composition And `davenda-all`](../reference/composition.md)
- [Shoppr Overview](../use-cases/shoppr/overview.md)
- [Gitly Overview](../use-cases/gitly/overview.md)
