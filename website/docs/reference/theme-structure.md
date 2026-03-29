---
title: Theme Structure
---

Coil themes are customer-owned UI packages made of templates, published assets, and a small
amount of manifest configuration.

The easiest way to understand the structure is to start with a real shape, then map each part back
to the framework contract.

## Start With A Working Shape

```text
customer-app/
  app.toml
  templates/
    layouts/
      base.html
    pages/
      home.html
      account.html
    components/
      nav.html
  theme/
    tokens.toml
    assets/
      site.css
      site.js
      logo.svg
```

What each part is doing:

- `templates/`
  - owns HTML structure, layout shells, fragments, and page composition
- `theme/assets/`
  - owns files that the asset pipeline publishes and templates reference through `asset(...)`
- `theme/tokens.toml`
  - optional customer convention for semantic design tokens
- `app.toml`
  - tells Coil which template namespaces and asset roots belong to the active theme

The key point is that Coil does not treat the theme as “just CSS.” The theme is the customer
app’s presentation boundary.

## The Manifest Contract

The framework-supported theme configuration lives under `[theme]` in `app.toml`:

```toml
[theme]
active = "harbor"
template_namespaces = ["customer-app", "harbor"]
asset_roots = ["theme/assets"]
```

Annotated:

- `active`
  - stable theme id for the customer app
- `template_namespaces`
  - ordered namespace precedence for template lookup
- `asset_roots`
  - relative directories that Coil publishes as theme assets

## Field Reference

### `active`

- Required: yes
- Type: string token
- Default: none
- Allowed values: any valid theme id token

Use it for:

- naming the active customer theme profile

Do not use it for:

- temporary folder aliases
- environment-specific behaviour

### `template_namespaces`

- Required: yes
- Type: array of namespace tokens
- Default: none
- Constraints:
  - must not be empty
  - values must be unique

Use it for:

- deciding which templates win when multiple layers define the same template name

The practical rule is simple:

- put the customer-owned namespace first
- put lower-priority fallback namespaces later

### `asset_roots`

- Required: no
- Type: array of relative paths
- Default: empty array
- Constraints:
  - must be relative
  - no absolute paths
  - no `..` traversal

Use it for:

- telling Coil which folders should be published and exposed through `asset(...)`

For most apps, `["theme/assets"]` is enough.

## What Belongs In `templates/` Versus `theme/assets/`

This is the split that developers most often blur.

Put it in `templates/` when it is:

- HTML structure
- semantic landmarks
- page composition
- fragment composition
- render-model bindings

Put it in `theme/assets/` when it is:

- CSS
- enhancement JS
- static images, icons, and fonts

If you find yourself putting route logic or data-fetching decisions into `site.js`, the split has
already broken down.

## Dark, Light, And System Mode

Coil does not provide a framework-global dark-mode switch.

The current framework contract is narrower:

- Coil publishes templates and assets
- the customer app decides how theme mode works
- the result still has to remain accessible

The strongest checked-in pattern is a customer-owned control like this:

```html
<div class="theme-switcher" role="group" aria-labelledby="theme-switcher-label">
  <span class="sr-only" id="theme-switcher-label">Theme</span>
  <button type="button" data-theme-option="light">Light</button>
  <button type="button" data-theme-option="dark">Dark</button>
  <button type="button" data-theme-option="system">System</button>
</div>
```

What this teaches:

- the server-rendered HTML already contains the control
- the enhancement script can persist the preference later
- semantics are visible in the template, not hidden in JS

Recommended theme-mode guidance:

- start from semantic CSS variables
- respect `prefers-color-scheme`
- keep explicit controls optional, not mandatory
- verify focus, contrast, and reduced motion in every mode

## Multi-Site Themes

Most multi-site apps should keep one theme tree and branch on runtime values such as:

- `site.id`
- `site.display_name`
- `site.brand_name`
- `locale`

That is usually better than cloning the entire theme per site.

A good multi-site template looks like this:

```html
<html xmlns:coil="https://coil.rs" coil:attr="lang=${locale}">
  <body>
    <a class="brand" coil:attr="href=${links.home}">
      <span coil:text="${site.brand_name}">Brand</span>
    </a>
  </body>
</html>
```

This keeps one template tree while allowing site-aware branding and links.

## Common Mistakes

### Treating `theme/` as only a CSS folder

The actual theme contract includes namespace precedence, published assets, and customer-owned shell
behaviour.

### Hardcoding final asset URLs

Templates should use logical asset paths such as `asset('theme/assets/site.css')`, not guessed CDN
paths.

### Turning `site.js` into a second application runtime

Use enhancement JS to improve the HTML-first path, not replace it.

### Assuming `tokens.toml` is a framework-mandated schema

It is useful and recommended, but today it is still a customer convention rather than a required
runtime file.

## Supporting Implementation And Repo Examples

Concrete supporting files:

- `apps/shoppr/app.toml`
- `apps/shoppr/theme/assets/site.css`
- `apps/shoppr/theme/assets/site.js`
- `apps/shoppr/theme/tokens.toml`
- `apps/gitly/app.toml`
- `apps/gitly/theme/assets/site.css`
- `apps/gitly/theme/assets/site.js`
- `crates/coil-app/src/types/theme.rs`
- `crates/coil-app/src/manifest/document.rs`

## What Should I Read Next?

- [Theme Asset Delivery](./theme-asset-delivery.md)
- [Template Language](./template-language.md)
- [Template Models](./template-models.md)
- [Themes, Rendering, And Assets](../core-concepts/themes-rendering-and-assets.md)
