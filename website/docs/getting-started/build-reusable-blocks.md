---
title: Build Reusable Blocks
---

This chapter turns the page model into a page-builder model.

## Replace `app.toml`

At this checkpoint the manifest should contain the site model, the page content model, and the
block schemas together:

```toml
name = "tutorial-app"
display_name = "Tutorial App"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]

[[sites]]
id = "tutorial-uk"
display_name = "Tutorial UK"
brand_name = "Tutorial"
canonical_domain = "uk.127.0.0.1.nip.io"
additional_domains = ["www.127.0.0.1.nip.io"]
default_locale = "en-GB"
supported_locales = ["en-GB"]

[[sites]]
id = "tutorial-fr"
display_name = "Tutorial France"
brand_name = "Tutorial"
canonical_domain = "fr.127.0.0.1.nip.io"
additional_domains = []
default_locale = "fr-FR"
supported_locales = ["fr-FR"]

[[content_models]]
name = "landing_page"
label = "Landing page"

[[content_models.fields]]
name = "title"
type = "string"
required = true

[[content_models.fields]]
name = "slug"
type = "slug"
required = true

[[content_models.fields]]
name = "summary"
type = "text"
required = true

[[content_models.fields]]
name = "page_type"
type = "string"
required = true

[[content_models.fields]]
name = "template"
type = "string"
required = false

[[content_models.fields]]
name = "seo_title"
type = "string"
required = false

[[content_models.fields]]
name = "seo_description"
type = "text"
required = false

[[block_types]]
id = "hero"
label = "Hero"

[[block_types.fields]]
name = "heading"
type = "string"
required = true

[[block_types.fields]]
name = "body"
type = "text"
required = true

[[block_types]]
id = "promo_grid"
label = "Promo grid"

[[block_types.fields]]
name = "intro"
type = "string"
required = false

[[block_types]]
id = "trust_strip"
label = "Trust strip"

[[block_types.fields]]
name = "heading"
type = "string"
required = true

[[block_types.fields]]
name = "body"
type = "text"
required = true
```

## Add One Shared Block File

### `content/shared-blocks/delivery-promises.json`

```json
{
  "id": "shared-delivery-promises",
  "label": "Delivery promises",
  "block_type": "trust_strip",
  "fields": {
    "heading": "Delivery and returns",
    "body": "Tracked shipping, straightforward returns, and local support."
  }
}
```

## Replace The Page Record With A Block-Based Version

### `content/pages/spring-sale.json`

```json
{
  "id": "page-spring-sale",
  "type": "landing_page",
  "title": "Spring Sale",
  "slug": "spring-sale",
  "summary": "Fresh arrivals and seasonal offers for the new quarter.",
  "settings": {
    "page_type": "landing_page",
    "template": "pages/landing-page",
    "show_in_navigation": true,
    "allow_indexing": true
  },
  "blocks": [
    {
      "kind": "instance",
      "id": "hero-spring-sale",
      "block_type": "hero",
      "label": "Spring Sale Hero",
      "fields": {
        "heading": "Spring layers and trail essentials",
        "body": "Start the season with new arrivals, member offers, and event signups."
      }
    },
    {
      "kind": "instance",
      "id": "promo-grid-spring-sale",
      "block_type": "promo_grid",
      "label": "Campaign cards",
      "fields": {
        "intro": "Shop by priority"
      }
    },
    {
      "kind": "shared_reference",
      "id": "shared-delivery-promises-reference",
      "shared_block_id": "shared-delivery-promises",
      "label": "Delivery promises"
    }
  ]
}
```

At this point you should have three concrete files in agreement:

```text
app.toml
content/shared-blocks/delivery-promises.json
content/pages/spring-sale.json
```

That set teaches the boundary you need:

- block type is schema
- inline block is page-local content
- shared block is reusable content
- shared reference is how the page points at that reusable content

It also lines up with the later CMS workflow:

- page settings still live at the page level
- ordered blocks become the editable page-builder surface
- shared blocks become reusable editorial assets

## Checkpoint

Run the app and confirm:

```bash
cargo run -p tutorial-app-bin -- validate
cargo run -p tutorial-app-bin -- serve
```

- the page has an ordered block list
- one block is a shared reference
- the app still validates and serves

## What To Read Next

- [Add Dynamic Blocks](add-dynamic-blocks.md)
