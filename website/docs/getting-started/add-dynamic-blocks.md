---
title: Add Dynamic Blocks
---

This chapter connects stored editorial blocks to request-time data.

## Replace The Page Record With A Dynamic Block

At this checkpoint, the page file should explicitly contain a dynamic block configuration:

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
      "id": "featured-events-spring",
      "block_type": "featured_events",
      "label": "Upcoming events",
      "fields": {
        "heading": "Try the new range in person",
        "limit": "3",
        "city": "Bristol"
      }
    }
  ]
}
```

## Replace `crates/tutorial-app-backend/src/lib.rs`

The customer backend should now turn that stored configuration into runtime data:

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
```

## Add A Real Block Fragment

Keep the template contract explicit:

### `templates/blocks/featured-events.html`

```html
<section xmlns:coil="https://coil.rs" coil:fragment="block">
  <p class="eyebrow" coil:text="${block.fields.heading}">Upcoming events</p>
  <ul>
    <li coil:each="event : ${block.runtime.events}">
      <a href="#" coil:attr="href=${event.href}" coil:text="${event.title}">Event</a>
    </li>
  </ul>
</section>
```

At this point the dynamic-block checkpoint should be visible as three concrete files:

```text
content/pages/spring-sale.json
crates/tutorial-app-backend/src/lib.rs
templates/blocks/featured-events.html
```

## What Each File Is Doing

### `content/pages/spring-sale.json`

This file still stores the editorial block instance.

The important fields are:

- `block_type = "featured_events"`
  This tells the runtime which kind of block it is dealing with.
- `fields.heading`
  Editor-owned display copy.
- `fields.limit`
  Editor-owned configuration for how much live data to request.
- `fields.city`
  Editor-owned filter criteria.

This file does not store the actual event list. It stores the configuration needed to ask runtime
code for one.

### `crates/tutorial-app-backend/src/lib.rs`

This file resolves request-time data.

The important function is `featured_events_block_model(...)`.

That function takes the stored block configuration and returns live values that should only exist at
request time:

- event titles
- event links
- any other derived runtime fields you need later

This is the point where customer-owned Rust turns stored editorial intent into live page data.

### `templates/blocks/featured-events.html`

This file renders the combined contract.

The important split in the template is:

- `${block.fields.heading}`
  editor-owned stored content
- `${block.runtime.events}`
  runtime-owned live data

That split is the key dynamic-block seam. The template does not query for events itself. It renders
what the runtime has already prepared.

## What Behavior This Enables

Once these files match:

- editors can place a dynamic block on a page without hard-coding live records into CMS content
- customer backend code can fetch or derive live data per request
- templates can render one stable block contract that mixes stored fields and runtime fields
- the app gains a clear schema/content/render-model handoff instead of hiding dynamic behavior in
  templates

## Checkpoint

Run the app and verify one real block is now mixing:

```bash
cargo run -p tutorial-app-bin -- validate
cargo run -p tutorial-app-bin -- serve
```

- stored editorial config
- request-time live data
- fragment rendering
