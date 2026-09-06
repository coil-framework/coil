# Contributing To Coil

Coil is built for product teams shipping serious web applications in Rust. Contributions are welcome, but they need to move the platform forward in a disciplined way.

## Before You Open A PR

- read the relevant chapters in `docs/design/`
- confirm the change fits the product shape instead of adding a one-off escape hatch
- include tests for the behavior you are changing
- update docs when the user-facing or operator-facing story changes

## Good Contributions

- fixes that close a real product gap
- performance or reliability improvements with verification
- documentation that removes ambiguity for adopters
- improvements to the demo apps that make Coil easier to understand
- changes that strengthen the boundary between core, official modules, customer apps, and WASM extensions

## Changes That Need More Care

Please open an issue or discussion before sending a large PR that changes:

- the extension model
- authentication semantics
- storage or cache contracts
- the runtime or rendering architecture
- customer app composition
- publish and release workflows

These are platform-shaping areas and they need a documented argument, not just a patch.

## Development Workflow

The root workspace uses the Fission source checkout at
`../../fission/fission`. Keep that checkout available rather than substituting
a second UI runtime or vendored copy.

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
cargo run -p coil-website -- build --project-dir website
```

## Pull Request Expectations

- keep commits focused and explain why the change exists
- prefer multiple small commits over one opaque diff
- mention tests run in the PR description
- call out any tradeoffs or intentionally deferred work
- do not mix unrelated refactors into feature PRs

## Review Standard

Coil is not trying to accept every possible approach. Reviews will prioritize:

- architectural coherence
- security and operational safety
- maintainability over cleverness
- clarity of extension boundaries
- quality of docs and tests

## Communication

Be direct, technical, and respectful. Strong disagreement is fine. Vague drive-by criticism is not useful.

## Licensing

By contributing to Coil, you agree that your contributions are licensed under the repository license.
