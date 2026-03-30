---
title: Where to Go Next
---

At this point the tutorial app should feel familiar. You have touched the same product surfaces the
checked-in Shoppr app uses:

- storefront pages under `apps/shoppr/templates/pages/**`
- CMS pages under `apps/shoppr/templates/cms/**`
- account pages under `apps/shoppr/templates/account/**` and
  `apps/shoppr/templates/memberships/**`
- admin pages under `apps/shoppr/templates/admin/**`
- customer-owned backend logic in `apps/shoppr/crates/shoppr-backend/src/lib.rs`

This final chapter should not be a generic wrap-up. It should tell the reader exactly where to go
when they want to keep working on one of those seams.

## If You Are Changing Templates, Theme, I18n, Or SEO

Look at these docs first:

- `website/docs/reference/template-language.md`
- `website/docs/reference/template-models.md`
- `website/docs/reference/theme-structure.md`
- `website/docs/reference/theme-asset-delivery.md`
- `website/docs/reference/internationalization.md`
- `website/docs/reference/seo.md`

Then look at these Shoppr files:

- `apps/shoppr/templates/layouts/base.html`
- `apps/shoppr/templates/pages/home.html`
- `apps/shoppr/templates/cms/page.html`
- `apps/shoppr/templates/account/nav.html`

Why:

- the reference pages explain the template and render-model contract
- the Shoppr files show how that contract is actually used in a full app shell

## If You Are Changing CMS Pages Or Block Rendering

Look at these docs first:

- `website/docs/reference/cms-page-builder-model.md`
- `website/docs/reference/render-model-hooks.md`
- `website/docs/core-concepts/themes-rendering-and-assets.md`

Then look at these Shoppr files:

- `apps/shoppr/templates/cms/page.html`
- `apps/shoppr/templates/cms/pages.html`
- `apps/shoppr/templates/cms/blocks/**`

Why:

- the docs explain the block model and render-model handoff
- the Shoppr files show the live frontend and admin sides of that same block system

## If You Are Changing Accounts, Memberships, Or Entitlements

Look at these docs first:

- `website/docs/reference/modules/commerce.md`
- `website/docs/reference/modules/memberships.md`
- `website/docs/reference/linked-rust-hook-apis.md`

Then look at these Shoppr files:

- `apps/shoppr/templates/account/dashboard.html`
- `apps/shoppr/templates/account/nav.html`
- `apps/shoppr/templates/account/passes.html`
- `apps/shoppr/templates/account/orders.html`
- `apps/shoppr/templates/memberships/account.html`
- `apps/shoppr/templates/memberships/passes.html`
- `apps/shoppr/crates/shoppr-backend/src/lib.rs`

Why:

- the module docs explain what the platform already provides
- the backend API docs explain where customer-owned validation and policy belong
- the Shoppr files show the account flow as customers and operators actually see it

## If You Are Changing Admin Resources

Look at these docs first:

- `website/docs/reference/modules/admin.md`
- `website/docs/reference/modules/cms.md`
- `website/docs/reference/modules/commerce.md`

Then look at these Shoppr files:

- `apps/shoppr/templates/admin/dashboard.html`
- `apps/shoppr/templates/admin/nav.html`
- `apps/shoppr/templates/admin/audit.html`
- `apps/shoppr/templates/cms/pages.html`
- `apps/shoppr/templates/commerce/catalog-admin.html`

Why:

- the module docs explain the admin resource contracts
- the Shoppr templates show the checked-in operator shell, tables, actions, and audit surface

## If You Are Changing Jobs, Webhooks, Or Integrations

Look at these docs first:

- `website/docs/reference/linked-rust-hook-apis.md`
- `website/docs/reference/wasm-host-apis.md`
- `website/docs/operations/jobs-and-schedulers.md`
- `website/docs/operations/webhooks-and-integrations.md`
- `website/docs/reference/modules/commerce-payments-stripe.md`

Then look at these Shoppr files:

- `apps/shoppr/crates/shoppr-backend/src/lib.rs`
- `apps/shoppr/templates/commerce/checkout.html`
- `apps/shoppr/templates/commerce/checkout-confirmation.html`

Why:

- the docs explain which integration and background seams are framework-owned
- the Shoppr files show how customer-facing pages reflect those flows honestly

## If You Are Changing Local Or Production Runtime Shape

Look at these docs first:

- `website/docs/reference/platform-config.md`
- `website/docs/reference/environment-variables.md`
- `website/docs/operations/build-and-deploy.md`
- `website/docs/operations/configuration-and-secrets.md`
- `website/docs/operations/database-migrations.md`
- `website/docs/operations/cache-tls-cutover-and-rollback.md`

Then look at these Shoppr files:

- `apps/shoppr/platform.dev.toml`
- `apps/shoppr/platform.toml`
- `apps/shoppr/docker-compose.yml`
- `apps/shoppr/Dockerfile`

Why:

- the docs explain the runtime contract
- the Shoppr files show the current customer-root deployment shape in a real app

## Keep The Ownership Boundary Straight

If you are unsure where a change belongs, use this rule:

- if it is product policy, account logic, booking validation, or entitlement logic, start in the
  customer backend crate
- if it is HTML structure or customer-facing copy, start in templates
- if it is admin presentation, start in admin templates
- if it is jobs, webhook handling, or integrations, start in linked backend hooks and the relevant
  operations/reference docs
- if it is core module behavior, check the official module docs before adding customer code

That is the same boundary the tutorial has been teaching all the way through.

## Final Checkpoint

Run the app one last time:

```bash
docker compose up -d
cargo run -p tutorial-app-bin -- validate
cargo run -p tutorial-app-bin -- serve
```

Open and verify:

1. `/`
   The branded shell, CMS content, and navigation still render.
2. `/account`
   The account and entitlement surfaces still work.
3. `/admin`
   The operator shell still renders.
4. one integration-backed or job-backed flow
   The request path and follow-up path still agree.

If that all works, the tutorial is no longer just a sequence of chapters. It is now a map of the
actual code and docs you need to keep building.
