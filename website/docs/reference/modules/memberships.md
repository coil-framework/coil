---
title: Memberships Module
---

The memberships module owns tiers, subscriptions, entitlements, and the customer account
memberships surface.

Primary implementation files:

- `crates/coil-memberships/src/module/manifest.rs`
- `apps/shoppr/templates/memberships/account.html`

## Why It Exists

Memberships are not just products with different copy. They need recurring lifecycle state,
entitlements, renewal jobs, and account visibility.

## What It Provides

From `crates/coil-memberships/src/module/manifest.rs`, memberships adds:

- migrations for member accounts, tiers, subscriptions, and entitlements
- account routes `/account` and `/account/memberships`
- admin routes for tier and subscription management
- scheduled renewals and entitlement-sync jobs
- a commerce bridge from paid orders into ongoing membership state

## How To Enable It

Enable both commerce and memberships:

```toml title="app.toml"
[modules]
enabled = ["commerce", "memberships"]
```

Shoppr uses this pattern because memberships has a required module dependency on commerce.

## How To Disable It

Remove `memberships` from the enabled lists and remove or replace account and admin surfaces that
assume `/account/memberships` and membership operator workflows exist.

## Config Expectations

The checked-in module does not require a large dedicated `[modules."memberships"]` block in the
demos. Most setup is structural:

- enable the module
- satisfy its auth capabilities
- provide customer-facing and operator-facing templates

## Routes And Surfaces

Important routes:

- `/account`
- `/account/memberships`
- `/admin/memberships/tiers`
- `/admin/memberships/subscriptions`

## Required Auth Capabilities

Memberships requires:

- `membership.subscription.manage`
- `membership.tier.edit`

It also optionally integrates with:

- `order.read`
- `admin.shell.access`
- `i18n.translation.edit`
- `asset.read`

## How Customer Apps Extend It

Memberships exposes an admin widget slot:

- `memberships.subscription.summary`

Customer apps typically extend the module by:

- selling membership products through commerce
- shaping the account UX in templates
- using linked hooks for post-purchase policy

Concrete example:

```html title="templates/memberships/account.html"
<section xmlns:coil="https://coil.rs">
  <h2>Your membership</h2>
  <p coil:text="${membership.tierName}">Founders</p>
  <p coil:text="${membership.stateLabel}">Active</p>
</section>
```

Memberships still owns entitlement and renewal state. The customer app owns how that state is
explained to members.

The practical sequence is:

1. enable `commerce` and `memberships`
2. sell membership products through commerce
3. provide account templates that explain tier and entitlement state
4. add linked checkout or webhook hooks if membership activation needs customer-specific policy

## Where To See It

Shoppr uses memberships in:

- `apps/shoppr/templates/memberships/account.html`
- `apps/shoppr/templates/account/dashboard.html`

## Common Mistakes

- Enabling memberships without commerce.
- Treating entitlement state as just account copy rather than a real async lifecycle.
- Forgetting that renewals and entitlement sync are jobs, not request-only logic.

## Read Next

- [Commerce](./commerce.md)
- [Events](./events.md)
