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

The lesson is structural:

- editorial config chooses the block and its stored settings
- runtime code resolves live records
- the template renders the combined contract

This is also the handoff point to the newer CMS/page-builder workflow:

- page settings stay on the page record
- ordered blocks remain the stored editorial shape
- shared blocks stay reusable
- runtime code supplies live block data without changing the editorial record

## Checkpoint

Run the app and verify one real block is now mixing:

```bash
cargo run -p tutorial-app-bin -- validate
cargo run -p tutorial-app-bin -- serve
```

- stored editorial config
- request-time live data
- fragment rendering

## What Comes Next

After this point the tutorial can move into richer product areas such as accounts, memberships,
events, and admin flows.
