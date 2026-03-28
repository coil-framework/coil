---
title: CMS Module
---

The CMS module gives a customer app editable pages, navigation trees, redirects, preview, and
publish workflow.

Primary implementation files:

- `crates/coil-cms/src/module/platform/manifest.rs`
- `apps/shoppr/templates/cms/page.html`
- `apps/shoppr/templates/cms/pages.html`
- `apps/shoppr/templates/cms/preview.html`
- `apps/shoppr/templates/cms/navigation.html`
- `apps/shoppr/templates/cms/redirects.html`

## Why It Exists

Coil keeps editorial publishing in a first-party module so pages, redirects, SEO, cache
invalidation, and publish scheduling stay coherent.

## What It Provides

From `crates/coil-cms/src/module/platform/manifest.rs`, CMS contributes:

- migrations for pages, navigation, and redirects
- localized public page routes such as `/pages/{slug}`
- admin routes for pages, navigation, and redirects
- admin actions for save draft, publish, unpublish, and redirect save
- scheduled and domain-event jobs for publish and cache invalidation
- render-hook and admin-widget extension slots

## How To Enable It

Add the module id in both places:

```toml title="app.toml"
[modules]
enabled = ["cms"]
```

```toml title="platform.dev.toml"
[modules]
enabled = ["cms"]
```

Shoppr shows the real shape in `apps/shoppr/app.toml` and `apps/shoppr/platform.dev.toml`.

## How To Disable It

Remove `cms` from both module lists, then remove or replace any CMS-owned templates and admin
navigation that your app depended on.

If you keep pages like `templates/cms/pages.html` without the module enabled, the templates are
just files. The runtime surface comes from the module manifest, not from the template tree alone.

## Config Expectations

CMS does not add a deep module-specific config table in the checked-in demos. It relies on shared
platform services:

- database
- cache
- jobs
- template loading
- auth package bindings
- i18n and SEO services

That means the main configuration work is usually:

- enabling the module
- ensuring the auth package satisfies CMS capabilities
- providing page templates under `templates/cms/` and public page templates under `templates/pages/`

## Routes And Surfaces

The manifest currently declares these important surfaces:

- public pages: `/pages/{slug}`
- preview: `/admin/pages/preview`
- page inventory: `/admin/pages`
- navigation: `/admin/navigation`
- redirects: `/admin/redirects`
- publish actions: `/admin/pages/publish`, `/admin/pages/unpublish`

Use Shoppr to see how those surfaces look in practice.

## Required Auth Capabilities

CMS requires:

- `cms.page.read`
- `cms.page.edit`
- `cms.page.publish`
- `cms.navigation.edit`

Optional but useful bindings include:

- `admin.shell.access`
- `seo.metadata.edit`
- `i18n.translation.edit`
- asset-read and asset-publication capabilities when CMS pages reference media

## How Customer Apps Extend It

There are two clean extension seams in the module manifest:

- admin widget slot: `cms.page.editor.sidebar`
- render hook slot: `cms.page.render`

Customer apps can also change CMS behaviour through:

- linked CMS hooks
- customer templates
- auth package bindings
- theme assets

Concrete example:

```html title="templates/cms/page.html"
<article xmlns:coil="https://coil.rs">
  <header>
    <h1 coil:text="${page.title}">Page title</h1>
  </header>
  <section coil:utext="${page.body}"></section>
  <aside coil:insert="~{fragments/editorial-signpost}"></aside>
</article>
```

That is a real customer-app extension point in practice:

- CMS still owns the route and publication model
- the customer app owns the rendered page shape
- the customer app can add fragments or render-hook output without forking the module

The practical sequence is:

1. enable `cms`
2. provide `templates/cms/*.html` and `templates/pages/*.html`
3. optionally register a linked CMS publish hook
4. optionally install a render hook against `cms.page.render`

## Where To See It

Shoppr is the canonical example:

- `apps/shoppr/templates/cms/pages.html`
- `apps/shoppr/templates/cms/page.html`
- `apps/shoppr/templates/cms/preview.html`
- `apps/shoppr/templates/cms/navigation.html`
- `apps/shoppr/templates/cms/redirects.html`

## Common Mistakes

- Enabling CMS without the required auth capabilities.
- Treating redirects as a reverse-proxy-only concern instead of part of publication.
- Forgetting that publish workflow also drives cache invalidation and scheduled jobs.

## Read Next

- [Media](./media.md)
- [Shoppr Custom Pages And CMS](../../use-cases/shoppr/custom-pages-and-cms.md)
