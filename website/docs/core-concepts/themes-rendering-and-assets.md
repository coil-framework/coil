---
title: Themes, Rendering, And Assets
---

This page explains how customer-owned templates, theme assets, and runtime rendering fit together in
Davenda.

## What Is This?

In Davenda, the customer-facing UI layer is the combination of:

- templates
- theme configuration
- published assets
- runtime render models
- document decoration such as SEO metadata injection

This is broader than “CSS plus some HTML.”

## Why Does It Exist?

Davenda needs a rendering model that can do all of these at once:

- let the customer app own the visual shell
- let official modules render inside that shell
- keep HTML readable and reviewable
- publish hashed assets safely
- keep public rendering, account flows, admin surfaces, and fragment composition under one model

That is why themes and rendering are documented as a first-class subsystem.

## When Should I Use This Mental Model?

Use this page when you are deciding:

- where a new layout belongs
- where a reusable fragment belongs
- where a customer asset belongs
- whether something belongs in templates, CSS, JS, or linked Rust
- how a page ends up with the final `<head>` metadata and asset URLs it serves

## Annotated Theme Tree

The checked-in customer apps use a structure like this:

```text
app/
  app.toml
  templates/
    layouts/
      base.html
      storefront.html
    pages/
      home.html
      account.html
    components/
      ...
    commerce/
      cart.html
      checkout.html
      checkout-confirmation.html
  theme/
    tokens.toml
    assets/
      site.css
      site.js
      logo.svg
```

What each area is for:

- `templates/layouts/`
  - full document shells or page wrappers
- `templates/pages/`
  - page-specific HTML
- `templates/components/`, `templates/fragments/`, or module folders
  - reusable partials
- `theme/assets/`
  - publishable frontend files
- `theme/tokens.toml`
  - optional design-token convention owned by the customer app

Concrete examples:

- Shoppr layouts: `apps/shoppr/templates/layouts/base.html`, `apps/shoppr/templates/layouts/storefront.html`
- Shoppr pages: `apps/shoppr/templates/pages/home.html`, `apps/shoppr/templates/pages/account.html`
- Gitly pages: `apps/gitly/templates/gitly/home.html`, `apps/gitly/templates/gitly/repository.html`

## Layouts, Fragments, Pages, And Assets In Practice

### Layouts

Layouts own document-level structure such as:

- `<!DOCTYPE html>`
- `<html>`
- `<head>`
- shared navigation
- footer
- named slots

Shoppr’s layout files are the best checked-in example because they show the full customer-owned
storefront shell.

### Fragments

Fragments exist so that reusable markup stays explicit and server-rendered.

Use them for:

- navigation sections
- summary panels
- collection grids
- reusable promotional or account blocks

### Pages

Pages assemble layouts and fragments around route-specific render-model data.

A page is where you usually see:

- the route-specific title
- the main heading
- the body content
- form or list rendering for that route

### Assets

Assets provide presentation and enhancement:

- CSS for layout and components
- JS for enhancement, not for recreating the entire page model
- images, icons, and similar published files

## Why Some Templates Carry Full HTML Structure

New Davenda developers often expect the runtime to hide the outer document shell completely.

The checked-in apps do not do that, and that is intentional.

Why:

- the customer app owns the actual product shell
- SEO and asset references are still customer-facing concerns
- the customer app often wants explicit control over header, main, footer, language controls, and
  navigation

So in Davenda, it is normal for a customer layout to contain:

- `<html lang=...>`
- `<head>`
- CSS and JS asset references
- document landmarks

That is not duplication. That is ownership.

## How Hashed Assets And Publication Work

Davenda publishes theme assets as runtime-managed artifacts.

Practical flow:

1. the customer app declares `[theme].asset_roots`
2. `assets publish` or the equivalent customer lifecycle publishes the asset tree
3. the runtime loads the publication manifest
4. templates resolve logical asset paths through `asset('...')`
5. rendered HTML receives the current published URL

Why this is better than hardcoded filenames:

- cache busting is deterministic
- local and production behavior stay aligned
- templates do not need to know the final hashed filename

Canonical examples:

- `apps/shoppr/theme/assets/site.css`
- `apps/shoppr/theme/assets/site.js`
- `apps/gitly/theme/assets/site.css`
- `apps/gitly/theme/assets/site.js`

## JSON-LD And Head Metadata Injection

Document-level metadata is not just whatever the template hardcodes into `<head>`.

Davenda’s render layer injects:

- meta description
- canonical URL
- robots policy
- alternate locale links
- Open Graph fields
- JSON-LD page nodes when enabled

That logic lives in:

- `crates/davenda-runtime/src/render/seo.rs`

This is why some templates stay focused on page structure and content while the runtime supplies
search-facing metadata automatically.

## Working Example

If you want one concrete “trace” through the system, use Shoppr home:

1. layout shell in `apps/shoppr/templates/layouts/base.html`
2. page in `apps/shoppr/templates/pages/home.html`
3. asset references via `asset('theme/assets/site.css')` and `asset('theme/assets/site.js')`
4. runtime model from `crates/davenda-runtime/src/render/model.rs`
5. head metadata injection from `crates/davenda-runtime/src/render/seo.rs`

That is the full rendering stack in one example.

## Constraints And Common Mistakes

### Treating the theme as only CSS

The theme contract includes namespace precedence, assets, and the customer-owned document shell.

### Recreating application state in `site.js`

JavaScript should enhance the HTML-first path, not replace it.

### Hardcoding final asset URLs

Use the asset helper and publication manifest path.

### Forking whole module screens instead of using customer-owned shell and fragments

That usually means the customer app is fighting the composition model.

## What Should I Read Next?

- [Template Language](../reference/template-language.md)
- [Theme Structure](../reference/theme-structure.md)
- [SEO](../reference/seo.md)
- `apps/shoppr/templates/`
- `apps/gitly/templates/`
