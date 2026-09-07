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

## What Each File Is Doing

### `app.toml`

This file now defines both the page schema and the block schema.

The important new section is `[[block_types]]`.

Each block type gives the editor a reusable block contract:

- `hero` supports the main campaign banner content
- `promo_grid` supports a smaller structured list section
- `trust_strip` supports reusable reassurance content

This file only defines what kinds of blocks can exist. It does not place any blocks on a page by
itself.

### `content/shared-blocks/delivery-promises.json`

This file creates one reusable block instance that can be referenced from multiple pages.

The important fields are:

- `id`
  The stable identifier other pages will reference.
- `block_type`
  The schema contract this shared block uses.
- `fields`
  The actual editorial values for that reusable block.

This is how the CMS can store one shared editorial asset instead of copying the same footer-style
content into every page record.

### `content/pages/spring-sale.json`

This file shows the page-builder shape on a real page.

The important section is `blocks`.

That ordered array now mixes two kinds of block records:

- `kind = "instance"`
  A page-local block owned only by this page.
- `kind = "shared_reference"`
  A pointer to a reusable shared block stored elsewhere.

That is the core page-builder behavior. The page owns the order of the blocks, while some of the
content can still be shared across many pages.

## What Behavior This Enables

Once these files line up:

- the page becomes an ordered structured block list instead of one free-form body field
- editors can reuse common content without duplicating it across every page
- the runtime can resolve shared block references when composing the page
- the CMS admin can later expose block reorder, replace, and shared-block editing workflows
- page-level settings still remain separate from the block list

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

- [Add Dynamic Blocks](../add-dynamic-blocks/)
