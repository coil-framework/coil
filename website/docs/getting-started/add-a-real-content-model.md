---
title: Add a Real Content Model
---

This chapter stops treating “a page” as one HTML field and starts modeling editorial state
explicitly.

## Replace `app.toml` With A Real Content Model

At this stage the manifest should not only declare sites and modules. It should also declare the
page shape the CMS is allowed to store.

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
```

## Add One Real Page Record

Do not stop at schema. Check in one real page instance as tutorial content.

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
    "seo_title": "Spring Sale | Tutorial App",
    "seo_description": "Fresh arrivals and seasonal offers for the new quarter.",
    "show_in_navigation": true,
    "allow_indexing": true
  }
}
```

This is the minimum useful distinction to make explicit:

- content fields
- page settings
- future ordered blocks

That distinction is what later block and runtime chapters depend on.

In a full CMS flow, this is the state that later maps onto:

- page settings forms
- block replacement/editing
- shared block references
- draft and publish validation

## Checkpoint

Run:

```bash
cargo run -p tutorial-app-bin -- validate
cargo run -p tutorial-app-bin -- serve
```

Then verify:

- the schema exists
- one real page instance exists
- the page record has settings as well as content
- the app still validates and serves with the new content model

## What To Read Next

- [Build Reusable Blocks](build-reusable-blocks.md)
