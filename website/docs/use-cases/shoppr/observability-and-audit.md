---
title: Observability And Audit
---

Shoppr is the canonical example of how a customer app should use Davenda's operational surfaces
rather than treating logs and audit as an afterthought.

## What To Read In The Repo

Start with:

- `apps/shoppr/crates/shoppr-backend/src/lib.rs`
- `apps/shoppr/backend/shoppr-loyalty-backend/src/lib.rs`
- `apps/shoppr/platform.toml`
- `apps/shoppr/platform.dev.toml`

These files show where Shoppr:

- records customer-owned audit entries
- uses verified webhook hooks
- drives runtime services through linked Rust
- keeps platform and app configuration aligned

## Concrete Audit Example

Shoppr's linked Rust backend receives an `AuditFacade` in checkout and verified-webhook hooks.

That means customer-owned logic can do more than reject or approve a decision. It can also record
why that decision happened.

This is the pattern to copy for:

- order review decisions
- webhook acceptance or rejection
- loyalty or entitlement policy checks
- admin-side operator actions implemented through customer code

## What The Developer Learns From Shoppr

The key Davenda lesson is:

- audit evidence is not a vague platform promise
- it is a facade your customer-owned code can call directly

If your store has a custom business rule, you should record evidence at the point where that rule
executes.

## Metrics And Runtime Signals

Shoppr's runtime also uses the platform observability surface described in
[Observability](../../operations/observability.md).

Use Shoppr when you want to connect the abstract platform story to a commerce-shaped app:

- checkout changes
- order intake
- webhook processing
- asset publication
- site and locale serving

## Practical Workflow

When adding a new customer rule in Shoppr:

1. implement the hook in linked Rust
2. record an audit entry when the rule accepts, rewrites, or rejects work
3. make sure the decision is visible in the operator surfaces that depend on it
4. document the rule in the store docs if it is part of the public app story

## Read Next

- [Observability](../../operations/observability.md)
- [Shoppr Linked Rust Backend](./linked-rust-backend.md)
- [Jobs, Webhooks, And Background Work](./jobs-webhooks-and-background-work.md)
