---
title: Model Brands, Categories, and Discovery
---

This chapter turns the tutorial app from a small set of editorial pages into a real browse surface.

The dynamic block chapter already proved that the runtime can combine stored content with live
data. This chapter adds a customer-owned domain model for brands and categories, then uses it to
build route-aware discovery pages.

## Purpose

The app needs more than a single `/shop` link. It needs:

- brand landing pages
- category discovery pages
- reusable taxonomy records
- listing pages that respond to the route and query string

By the end of this chapter, the tutorial app should have a real discovery layer that later account,
membership, and event chapters can plug into.

## Replace `app.toml`

At this point the manifest should include brand and category models alongside the editorial and
block models introduced earlier.

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

[[content_models]]
name = "brand"
label = "Brand"

[[content_models.fields]]
name = "name"
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
name = "hero_heading"
type = "string"
required = false

[[content_models]]
name = "category"
label = "Category"

[[content_models.fields]]
name = "name"
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
name = "parent_category_slug"
type = "string"
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
id = "featured_events"
label = "Featured events"

[[block_types.fields]]
name = "heading"
type = "string"
required = true

[[block_types.fields]]
name = "limit"
type = "string"
required = true

[[block_types.fields]]
name = "city"
type = "string"
required = false
```

### What matters in `app.toml`

The important new sections are:

- `[[content_models]] name = "brand"`
  This creates a first-class brand record shape instead of burying brand names inside templates.
- `[[content_models]] name = "category"`
  This creates a first-class category record shape for route-driven browsing.
- the brand and category fields
  These fields make taxonomy content explicit and editable instead of hard-coded.

This file defines what kinds of records the app supports. It still does not create any actual brand
or category instances. That comes next.

## Add Brand Records

Create `content/brands/ridgefield.json`:

```json
{
  "id": "brand-ridgefield",
  "type": "brand",
  "name": "Ridgefield",
  "slug": "ridgefield",
  "summary": "Outerwear and layering gear for cold and wet conditions.",
  "hero_heading": "Ridgefield seasonal layers"
}
```

Create `content/brands/ember-trail.json`:

```json
{
  "id": "brand-ember-trail",
  "type": "brand",
  "name": "Ember Trail",
  "slug": "ember-trail",
  "summary": "Trail accessories, workshop tools, and lightweight travel gear.",
  "hero_heading": "Ember Trail workshop picks"
}
```

### What these files do

These are content instances, not schema.

They provide the actual data that later discovery routes will render:

- a stable `slug` for route lookup
- a display `name`
- descriptive copy for the discovery page
- a hero heading for the landing surface

## Add Category Records

Create `content/categories/layers.json`:

```json
{
  "id": "category-layers",
  "type": "category",
  "name": "Layers",
  "slug": "layers",
  "summary": "Base, mid, and outer layers for mixed weather conditions.",
  "parent_category_slug": null
}
```

Create `content/categories/workshop-tools.json`:

```json
{
  "id": "category-workshop-tools",
  "type": "category",
  "name": "Workshop tools",
  "slug": "workshop-tools",
  "summary": "Repair kits, care tools, and tuning essentials.",
  "parent_category_slug": null
}
```

### What these files do

These files provide the taxonomy records the app can browse by.

The important fields are:

- `slug`
  This becomes the route-facing identifier.
- `summary`
  This gives category landing pages real copy.
- `parent_category_slug`
  This is the starting point for nested taxonomy later if the app needs it.

## Replace `crates/tutorial-app-backend/src/lib.rs`

The tutorial backend now needs to do two jobs:

- keep the dynamic block example from the previous chapter
- add request-time discovery shaping for brands and categories

Replace the backend file with this:

```rust
use coil_customer_sdk::{
    BackendError, CustomerBackendPlugin, CustomerHookRegistry, RequestContext,
};
use std::collections::BTreeMap;

pub struct TutorialAppPlugin;

impl CustomerBackendPlugin for TutorialAppPlugin {
    fn register(
        &self,
        _registry: &mut dyn CustomerHookRegistry,
    ) -> Result<(), coil_customer_sdk::BackendError> {
        Ok(())
    }
}

pub fn featured_events_block_model(
    _request: &RequestContext,
) -> Result<Vec<BTreeMap<String, String>>, BackendError> {
    Ok(vec![
        BTreeMap::from([
            ("title".to_string(), "Bristol trail evening".to_string()),
            ("href".to_string(), "/events/bristol-trail-evening".to_string()),
        ]),
        BTreeMap::from([
            ("title".to_string(), "Lake district gear clinic".to_string()),
            ("href".to_string(), "/events/lake-district-gear-clinic".to_string()),
        ]),
    ])
}

pub fn brand_discovery_model(
    _request: &RequestContext,
    slug: &str,
) -> Result<BTreeMap<String, String>, BackendError> {
    let model = match slug {
        "ridgefield" => BTreeMap::from([
            ("title".to_string(), "Ridgefield".to_string()),
            (
                "summary".to_string(),
                "Outerwear and layering gear for cold and wet conditions.".to_string(),
            ),
            ("hero_heading".to_string(), "Ridgefield seasonal layers".to_string()),
        ]),
        "ember-trail" => BTreeMap::from([
            ("title".to_string(), "Ember Trail".to_string()),
            (
                "summary".to_string(),
                "Trail accessories, workshop tools, and lightweight travel gear.".to_string(),
            ),
            (
                "hero_heading".to_string(),
                "Ember Trail workshop picks".to_string(),
            ),
        ]),
        _ => BTreeMap::from([
            ("title".to_string(), "Unknown brand".to_string()),
            ("summary".to_string(), "No brand record matched this route.".to_string()),
            ("hero_heading".to_string(), "Brand not found".to_string()),
        ]),
    };

    Ok(model)
}

pub fn category_discovery_model(
    _request: &RequestContext,
    slug: &str,
    query: Option<&str>,
) -> Result<BTreeMap<String, String>, BackendError> {
    let base = match slug {
        "layers" => BTreeMap::from([
            ("title".to_string(), "Layers".to_string()),
            (
                "summary".to_string(),
                "Base, mid, and outer layers for mixed weather conditions.".to_string(),
            ),
        ]),
        "workshop-tools" => BTreeMap::from([
            ("title".to_string(), "Workshop tools".to_string()),
            (
                "summary".to_string(),
                "Repair kits, care tools, and tuning essentials.".to_string(),
            ),
        ]),
        _ => BTreeMap::from([
            ("title".to_string(), "Unknown category".to_string()),
            ("summary".to_string(), "No category matched this route.".to_string()),
        ]),
    };

    let mut model = base;
    model.insert(
        "query".to_string(),
        query.unwrap_or_default().to_string(),
    );
    Ok(model)
}
```

### What matters in the backend file

The important additions are:

- `brand_discovery_model(...)`
  This is the route-aware shape for brand landing pages.
- `category_discovery_model(...)`
  This shows the pattern for category routes and query-string-aware listings.
- `query.unwrap_or_default()`
  This is the first explicit example of route/query state becoming request-time render data.

These functions are deliberately simple, but they show the ownership boundary clearly:

- taxonomy records are stored content
- route and query state are request-time input
- the backend turns those inputs into a concrete discovery model

## Add A Brand Page Template

Create `templates/pages/brand.html`:

```html
<!doctype html>
<html xmlns:coil="https://coil.rs" coil:replace="~{layouts/base}">
  <body>
    <section class="hero">
      <p class="eyebrow">Brand</p>
      <h1 coil:text="${brand.title}">Brand title</h1>
      <p coil:text="${brand.summary}">
        Brand summary
      </p>
    </section>

    <section class="content-rail">
      <article class="card">
        <h2 coil:text="${brand.hero_heading}">Seasonal picks</h2>
        <p>
          This page is route-aware. The backend shaped the brand model from the requested slug.
        </p>
      </article>
    </section>
  </body>
</html>
```

### What this template does

This page renders one stable brand-facing contract:

- `${brand.title}`
  Route-resolved brand name
- `${brand.summary}`
  Stored brand content rendered into the page
- `${brand.hero_heading}`
  Request-time model data prepared by the backend

## Add A Category Page Template

Create `templates/pages/category.html`:

```html
<!doctype html>
<html xmlns:coil="https://coil.rs" coil:replace="~{layouts/base}">
  <body>
    <section class="hero">
      <p class="eyebrow">Category</p>
      <h1 coil:text="${category.title}">Category title</h1>
      <p coil:text="${category.summary}">
        Category summary
      </p>
    </section>

    <section class="card">
      <h2>Discovery state</h2>
      <p>
        Active search:
        <strong coil:text="${category.query}">none</strong>
      </p>
      <p>
        This page should later render a real filtered listing. For now it proves that route and
        query state are reaching the template through a shaped model.
      </p>
    </section>
  </body>
</html>
```

### What this template does

The category page is the first route-aware discovery template in the tutorial.

The important fields are:

- `${category.title}` and `${category.summary}`
  Stored category content
- `${category.query}`
  Request-time route/query input resolved by backend code

This is the discovery version of the same schema/content/runtime split the CMS chapters introduced.

## What Behavior This Enables

Once these files exist together:

- the app has real brand and category records
- discovery pages stop being one hard-coded `/shop` page
- route slug and query-string state become visible in template rendering
- the tutorial app now has a clear bridge from editorial landing pages into browse and discovery

That matters because later chapters need somewhere credible to add:

- signed-in customer state
- membership-aware discovery
- event and booking entry points

## Checkpoint

Run:

```bash
cargo run -p tutorial-app-bin -- validate
cargo run -p tutorial-app-bin -- serve
```

Then verify:

- the app still validates after adding the new models
- at least one brand landing page renders from a route-aware model
- at least one category page renders and shows query state
- discovery is now driven by real domain records instead of only editorial pages

## What To Read Next

- [Add Authentication and Customer Accounts](add-authentication-and-customer-accounts.md)
