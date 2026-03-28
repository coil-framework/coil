---
title: Theme Structure
---

This page documents the concrete theme and asset structure Davenda supports today.

## What Is This?

In Davenda, a theme is the customer-owned presentation layer made up of:

- template namespaces
- published asset roots
- customer HTML templates
- CSS, JavaScript, images, icons, and similar frontend assets

Themes live in the customer app, not in the runtime binary.

## Why Does It Exist?

Davenda needs a stable way for customer apps to:

- own their document shell
- restyle official module surfaces
- publish hashed assets safely
- keep frontend behavior inside the customer workspace

That is why theming is a manifest-level concept instead of “put some CSS somewhere and hope.”

## When Should I Use It?

You configure the theme whenever the customer app needs to:

- declare which templates should win in namespace resolution
- publish frontend assets
- supply its own layouts, fragments, or visual shell

If you are building a real customer app, you are using the theme system whether the visual design is
minimal or highly branded.

## How Do I Configure It?

Theme settings live under `[theme]` in `app.toml`.

Example from Shoppr:

```toml
[theme]
active = "harbor"
template_namespaces = ["customer-app", "harbor"]
asset_roots = ["theme/assets"]
```

Example from Gitly:

```toml
[theme]
active = "gitly"
template_namespaces = ["customer-app", "gitly"]
asset_roots = ["theme/assets"]
```

## Field Reference

### `active`

- Required: yes
- Type: string token
- Default: none
- Allowed values: any valid theme id token

What it means:

- the customer-facing theme identity for the app

Practical guidance:

- keep it stable
- treat it like app-owned configuration, not a temporary folder alias

### `template_namespaces`

- Required: yes
- Type: array of namespace tokens
- Default: none
- Constraints:
  - must not be empty
  - values must be unique

What it means:

- ordered template lookup precedence for this customer app

Practical guidance:

- put the customer-owned namespace first
- put lower-priority module or sample namespaces later

### `asset_roots`

- Required: no
- Type: array of relative paths
- Default: empty array
- Constraints:
  - paths must be relative
  - no absolute paths
  - no `..` traversal segments

What it means:

- which folders should be published through the theme asset pipeline

Practical guidance:

- keep it narrow
- most customer apps only need `["theme/assets"]`

## Which Exact Files Are Involved?

The important concrete files are:

- customer manifest: `app.toml`
- customer templates: `templates/**/*.html`
- theme assets: `theme/assets/**`
- optional design-token file: `theme/tokens.toml`
- runtime asset publication settings: `platform.toml` and `platform.dev.toml`

Checked-in examples:

- `apps/shoppr/app.toml`
- `apps/shoppr/theme/assets/site.css`
- `apps/shoppr/theme/assets/site.js`
- `apps/shoppr/theme/tokens.toml`
- `apps/gitly/app.toml`
- `apps/gitly/theme/assets/site.css`
- `apps/gitly/theme/assets/site.js`
- `apps/gitly/theme/tokens.toml`

## Recommended Theme Tree

```text
customer-app/
  app.toml
  templates/
    layouts/
    pages/
    components/
    commerce/
    account/
    admin/
  theme/
    tokens.toml
    assets/
      site.css
      site.js
      logo.svg
      images/
      fonts/
```

What each area is for:

- `templates/`
  - HTML structure, layouts, fragments, and module-facing page markup
- `theme/assets/`
  - publishable frontend files
- `theme/tokens.toml`
  - optional but useful customer convention for semantic design tokens

Important boundary:

- Davenda supports the asset-root and namespace mechanics directly
- a file like `theme/tokens.toml` is an app convention, not a magic runtime input

## How Asset Publication Works

Davenda expects templates to reference assets by logical path, not hardcoded final URLs.

Typical template usage:

```html
<link rel="stylesheet" href="/theme/assets/site.css" dv:href="asset('theme/assets/site.css')" />
<script src="/theme/assets/site.js" dv:src="asset('theme/assets/site.js')" defer="defer"></script>
```

The flow is:

1. the customer app declares `asset_roots`
2. assets are published
3. the runtime loads the asset manifest
4. templates resolve logical asset paths to the published URL

Why this matters:

- hashed asset filenames work in production
- templates stay readable
- CDN and same-origin serving use the same logical asset names

## Dark, Light, And System Mode

Davenda does not impose a framework-global theme mode switch.

Today’s honest model is:

- the runtime publishes templates and assets
- the customer app decides how light, dark, and system mode work
- the theme should remain accessible in every mode

Gitly is the canonical checked-in example:

- theme controls live in templates such as `apps/gitly/templates/gitly/home.html`
- the translation and theme-switching behavior lives in `apps/gitly/theme/assets/site.js`

Recommended approach:

- use semantic CSS variables
- default to `prefers-color-scheme`
- add explicit user controls only when the product benefits from them
- keep the page usable before JavaScript applies persisted preferences

## Multi-Site And Theme Structure

Do not fork the theme tree per site unless the information architecture is genuinely different.

The common Davenda pattern is:

- one theme tree
- site-aware values from the render model
- different branding, links, catalog visibility, and SEO per site

Use these runtime values in templates instead of cloning entire themes:

- `site.id`
- `site.displayName`
- `site.brandName`
- `locale`
- `links.*`

## Working Example

Shoppr shows the canonical commerce-oriented shape:

- `apps/shoppr/app.toml`
- `apps/shoppr/templates/`
- `apps/shoppr/theme/assets/site.css`
- `apps/shoppr/theme/assets/site.js`
- `apps/shoppr/theme/tokens.toml`

Gitly shows a non-commerce but still customer-root example:

- `apps/gitly/app.toml`
- `apps/gitly/templates/gitly/`
- `apps/gitly/theme/assets/site.css`
- `apps/gitly/theme/assets/site.js`

Use both when deciding whether a file belongs in templates, assets, or linked customer Rust.

## Common Mistakes

### Treating `theme/` as only a CSS folder

The theme contract is wider: namespace precedence, asset publication, and customer-owned shell
behavior all live here.

### Hardcoding final asset URLs

That breaks hashed publication and environment portability immediately.

### Turning `site.js` into a SPA shell

Use it for progressive enhancement, not for rebuilding the page model in the browser.

### Assuming `tokens.toml` is a framework-mandated schema

It is useful, but today it is still a customer convention.

## What Should I Read Next?

- [Template Language](./template-language.md)
- [Internationalization](./internationalization.md)
- [Accessibility](./accessibility.md)
- [SEO](./seo.md)
- `apps/shoppr/theme/`
- `apps/gitly/theme/`
