---
title: Composition And davenda-all
---

Davenda gives you two composition styles:

- broad convenience through `davenda-all`
- explicit crate-by-crate selection in your customer workspace

Both are valid. The right choice depends on how much control you need now.

## What This Page Helps You Decide

Use this page when you want to answer:

- should my customer binary depend on `davenda-all`
- when should I link only selected modules
- how do compile-time linking and runtime enablement differ
- what do Shoppr and Gitly actually do

## The Two Layers Of Composition

There are always two decisions:

1. What your customer workspace links into the binary.
2. What your manifest and config enable at runtime.

Those are not the same thing.

Concrete example:

- `apps/shoppr/Cargo.toml` links a broad workspace with `davenda-all`.
- `apps/shoppr/app.toml` and `apps/shoppr/platform.dev.toml` decide the actual installed modules.

## Fastest Path: `davenda-all`

Use `davenda-all` when you want a believable product quickly.

This is the pattern in both demo apps:

```toml title="apps/shoppr/crates/shoppr-app/Cargo.toml"
[dependencies]
davenda-all.workspace = true
davenda-app.workspace = true
davenda-auth.workspace = true
davenda-runtime.workspace = true
```

Why teams start here:

- shortest learning path
- broad official module availability
- less crate-selection friction while the product shape is still moving

## Narrower Path: Selective Linking

Use explicit linking when:

- you already know the exact product surface
- you want a smaller dependency graph
- you want hard compile-time limits on which modules may be enabled

A selective customer app still needs:

- `davenda-app`
- `davenda-auth`
- `davenda-runtime`
- whichever official modules you genuinely intend to support
- any customer-owned linked backend crates

## Runtime Enablement Still Matters

Even with `davenda-all`, modules are not installed automatically.

Shoppr still has to say this explicitly in `apps/shoppr/app.toml`:

```toml
[modules]
enabled = ["cms", "media", "commerce", "commerce-payments-stripe", "memberships", "events", "admin", "ops"]
```

Gitly does the same in `apps/gitly/app.toml`:

```toml
[modules]
enabled = ["admin", "cms", "media", "gitly-showcase"]
```

That is why “linked” and “enabled” must stay separate in your mental model.

## Common Composition Patterns

### Broad product app

Use a broad convenience stack when the product is still discovering its boundaries.

Shoppr is the example:

- broad official stack
- linked Rust backend
- runtime-installed WASM
- multiple sites and locales

### Narrow non-commerce app

Gitly shows the second pattern:

- broad runtime battery still linked for convenience
- only a narrow enabled module set
- customer-owned routes doing most of the product work

### Controlled platform app

This is the pattern to adopt later:

- explicit module dependencies
- explicit customer crates
- manifest enablement limited to what the binary truly supports

## Failure Mode To Avoid

Do not enable a module in `app.toml` that the binary did not link.

The customer-root runtime builder explicitly checks for this. That is part of what keeps customer
apps honest and upgradeable.

## Choosing Intentionally

Choose `davenda-all` if:

- you are learning Davenda
- you want the fastest path to a real app
- you expect to use many official modules

Choose selective composition if:

- you already know the product boundary
- you want tighter release control
- you want the binary to prove the supported module set

## Read Next

- [Official Modules](./modules.md)
- [Customer Rust Vs Third-Party WASM](./customer-vs-wasm.md)
- [Shoppr Overview](../use-cases/shoppr/overview.md)
- [Gitly Overview](../use-cases/gitly/overview.md)
