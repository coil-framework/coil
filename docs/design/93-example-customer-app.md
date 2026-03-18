# Example Customer App

**Part:** Appendices  
**Chapter:** 93

This appendix shows a small but realistic customer app built on the platform. The example is intentionally modest: a branded site with CMS pages, a small catalog, a media library, and selected admin tools. It demonstrates the boundary between the reusable platform and customer-owned code.

## What Lives Where

The customer app composes:

- core runtime and host services
- selected official modules
- customer templates, theme tokens, translations, and content model
- customer auth extensions where the default model is not sufficient
- optional WASM extensions for bespoke behavior

It does not reimplement cache, storage, TLS, auth execution, or module internals.

## Reference Layout

```text
apps/harbor-shop/
  app.toml
  templates/
    layouts/
    pages/
    components/
  theme/
    tokens.toml
    assets/
  content/
    page-types/
  auth/
    harbor-auth/
      package.toml
      model.auth
      capabilities.toml
  extensions/
    loyalty-widget.wasm
```

The customer app is a separate deployable package. It can be built, versioned, and released independently while still pinning compatible versions of core and official modules.

## Installed Modules

For this example the app enables:

- `cms-pages`
- `media-library`
- `commerce-catalog`
- `admin-shell`
- `admin-content`

That module set is enough for a content-led storefront without committing to the full commerce stack.

## Reference Manifest

```toml
[app]
name = "harbor-shop"

[modules]
enabled = ["cms-pages", "media-library", "commerce-catalog", "admin-shell", "admin-content"]

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]

[theme]
active = "harbor"
```

This is the level of configuration the app should own directly: composition, locale policy, and theme selection. Product data, page content, and media records stay in managed module data.

## Theme And Rendering

Templates are HTML-first and use the platform’s fragment model for shared layout, navigation, cards, forms, and partial updates. The theme layer supplies:

- brand tokens such as typography, spacing, and color
- static build assets that publish through the asset pipeline as public deployment artifacts
- locale-aware templates and copy choices

Because the template engine is a core service, the customer app only owns template content and composition. It does not own the rendering runtime.

## Auth Binding

The example app imports the default auth package and extends it with one customer-specific concept: a merchandising team that may edit catalog presentation without gaining broad admin rights. That is implemented by:

- adding a `merchandiser` relation for the relevant catalog resources
- binding a customer capability such as `catalog.featured.edit`
- leaving canonical first-party capabilities unchanged so module upgrades remain clean

This is the expected customization path. The app changes semantics it owns while continuing to consume stable capability contracts from official modules.

## Runtime And Policy Choices

The app chooses:

- ACME with DNS-01 for TLS
- `moka` plus `redis` for cache
- object storage for uploads and build-asset publication
- `public_upload` for catalog media and `private_shared` for internal documents
- English and French locales with locale-aware routing
- JSON-LD enabled for pages and product summaries

These are app-level policy choices expressed through config. The actual cache engine, storage adapters, TLS lifecycle, and SEO rendering primitives remain part of the platform.

## Extension Example

The `loyalty-widget.wasm` extension contributes a customer-specific account widget and a small admin dashboard card. It consumes host APIs for rendering, auth checks, and data access, but it does not access tuple tables, raw object-store credentials, or internal module tables directly. That is a useful example of the intended extension boundary: bespoke behavior at the edge, stable services in the host.

## Why This App Stays Upgradeable

This app remains maintainable because it customizes composition, not foundations. It does not fork official modules, it binds to capability names instead of relation names, and it keeps customer-specific behavior inside app-owned templates, config, and extensions. That is exactly the boundary the platform is designed to protect.
