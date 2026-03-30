---
title: Add Events and Timeslots
---

This chapter uses the real checked-in Shoppr events seam.

There is no `content/events/` directory in this app. There is no fake tutorial-only event loader.
The events route exists because the customer app enables the official `events` module, Shoppr
provides real `events/list` and `events/detail` templates, and the runtime shapes the event cards
and timeslots for those routes at request time.

The same module also contributes operator routes under `/admin/events`. Those pages reuse the
shared admin shell and the shared admin bundle rather than inventing a separate event-ops frontend.

## What This Chapter Adds

At the end of this chapter, the app has:

- a real localized `/events` route
- a real localized `/events/{event_slug}` route
- a public event listing template
- a public event detail template with timeslot rows
- event operator pages under `/admin/events`, `/admin/events/bookings`, and `/admin/events/check-in`
- runtime model shaping that provides `events`, `event`, and `event.timeslots`

This chapter does **not** add a separate checked-in event content store. The current Shoppr seam is
honest: the official module contributes the route contract, the customer app owns the templates,
and the runtime currently supplies the event records used by those templates.

## 1. Enable The Official Events Module

The first step is not to create event JSON files. It is to enable the official module in
`app.toml`.

### `apps/shoppr/app.toml`

```toml
[app]
name = "shoppr"
display_name = "Shoppr"

[domains]
canonical = "uk.localhost"
additional = ["www.localhost", "fr.localhost", "pl.localhost", "shop.example.com", "www.example.com"]

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR", "pl-PL"]
localized_routes = true

[translations]

[[translations.catalogs]]
locale = "en-GB"
path = "translations/en-GB.toml"

[[translations.catalogs]]
locale = "fr-FR"
path = "translations/fr-FR.toml"

[[translations.catalogs]]
locale = "pl-PL"
path = "translations/pl-PL.toml"

[[sites]]
id = "shoppr-uk"
display_name = "Shoppr UK"
brand_name = "Shoppr"
canonical_domain = "uk.localhost"
additional_domains = ["www.localhost", "shop.example.com", "www.example.com"]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR", "pl-PL"]

[[sites]]
id = "shoppr-fr"
display_name = "Shoppr France"
brand_name = "Shoppr Paris"
canonical_domain = "fr.localhost"
additional_domains = ["fr-store.localhost"]
default_locale = "fr-FR"
supported_locales = ["en-GB", "fr-FR", "pl-PL"]

[[sites]]
id = "shoppr-pl"
display_name = "Shoppr Polska"
brand_name = "Shoppr Studio"
canonical_domain = "pl.localhost"
additional_domains = ["pl-store.localhost"]
default_locale = "pl-PL"
supported_locales = ["en-GB", "fr-FR", "pl-PL"]

[theme]
active = "harbor"
template_namespaces = ["customer-app", "harbor"]
asset_roots = ["theme/assets"]

[auth]
mode = "extend"
package = "shoppr-auth"

[modules]
enabled = ["cms", "media", "commerce", "commerce-payments-stripe", "memberships", "events", "admin", "ops"]

[[extensions]]
id = "shoppr-waitlist-tools"
package_version = "0.1.0"
artifact_sha256 = "3ad7b44218d04a3eba602051cbcb991bdd1ab69fd55ad995cd688af26ca6d067"
customer_app_id = "shoppr"

[[extensions.handlers]]
id = "home.waitlist.banner"
grants = []
```

The important line in this chapter is:

```toml
[modules]
enabled = ["cms", "media", "commerce", "commerce-payments-stripe", "memberships", "events", "admin", "ops"]
```

That one module entry is what turns on the official event route contract. The sites and locales in
the same file matter because the event routes are localized. Once `events` is enabled, the same
host and locale rules that already apply to storefront pages also apply to `/events` and
`/events/{event_slug}`.

## 2. Use The Existing Catalog For Event-Led Discovery

Shoppr already has an event-led collection in its catalog. That is how the checked-in app connects
public event pages back into the rest of the storefront.

### `apps/shoppr/catalog.toml`

```toml
[[collections]]
handle = "featured"
title = "Featured"
label = "Featured edit"
summary = "Current campaign picks spanning merch, memberships, and event offers."
site_ids = ["shoppr-uk", "shoppr-fr", "shoppr-pl"]

[[collections]]
handle = "memberships"
title = "Memberships"
label = "Recurring value"
summary = "Recurring and premium access products that unlock customer benefits."
site_ids = ["shoppr-uk", "shoppr-pl"]

[[collections]]
handle = "events"
title = "Events"
label = "Event-led offer"
summary = "Bookable offers and event-linked passes surfaced alongside editorial content."
site_ids = ["shoppr-uk", "shoppr-fr"]

[[products]]
sku = "harbor-cap"
handle = "harbor-cap"
title = "Harbor Cap"
summary = "A classic canvas cap with embroidered harbor mark."
price_minor = 2900
currency = "GBP"
collection_handle = "featured"
variant_title = "Standard"
product_kind = "physical"
site_ids = ["shoppr-uk", "shoppr-pl"]
inventory_locations = ["uk-warehouse", "de-warehouse"]

[[products]]
sku = "membership-gold"
handle = "gold-membership"
title = "Gold Membership"
summary = "Priority event booking, exclusive offers, and member-only access."
price_minor = 8900
currency = "GBP"
collection_handle = "memberships"
variant_title = "Annual"
product_kind = "membership"
entitlement_key = "membership.gold"
site_ids = ["shoppr-uk", "shoppr-pl"]
inventory_locations = ["uk-digital", "de-digital"]

[[products]]
sku = "tasting-pass"
handle = "tasting-pass"
title = "Spring Tasting Pass"
summary = "An event-linked pass for the next seasonal tasting series."
price_minor = 4500
currency = "GBP"
collection_handle = "events"
variant_title = "Single pass"
product_kind = "physical"
site_ids = ["shoppr-uk", "shoppr-fr"]
inventory_locations = ["uk-events", "us-events"]

[[products]]
sku = "harbor-scarf"
handle = "harbor-scarf"
title = "Harbor Scarf"
summary = "A cold-weather staple reserved for the Poland storefront assortment."
price_minor = 3900
currency = "EUR"
collection_handle = "featured"
variant_title = "Winter weave"
product_kind = "physical"
site_ids = ["shoppr-pl"]
inventory_locations = ["de-warehouse"]

[[products]]
sku = "brooklyn-night-pass"
handle = "brooklyn-night-pass"
title = "Brooklyn Night Pass"
summary = "A France-only after-hours event pass reserved for the Paris edit."
price_minor = 6500
currency = "USD"
collection_handle = "events"
variant_title = "Evening entry"
product_kind = "physical"
site_ids = ["shoppr-fr"]
inventory_locations = ["us-events"]
```

The important section is:

```toml
[[collections]]
handle = "events"
title = "Events"
label = "Event-led offer"
summary = "Bookable offers and event-linked passes surfaced alongside editorial content."
site_ids = ["shoppr-uk", "shoppr-fr"]
```

That collection means the public event pages do not sit in isolation. The events route can link the
customer back into a real storefront collection that already contains event-linked offers.

## 3. Let The Events Module Contribute The Public Routes

The `events` module owns the route contract. Shoppr does not hand-roll these routes in its own app
crate.

### `crates/coil-events/src/module/platform/surfaces.rs`

```rust
use super::*;

pub(super) fn route_surfaces() -> Vec<RouteSurface> {
    vec![
        RouteSurface::new("events.list", RouteSurfaceKind::FrontendPage, "/events").localized(),
        RouteSurface::new(
            "events.detail",
            RouteSurfaceKind::FrontendPage,
            "/events/{event_slug}",
        )
        .localized(),
        RouteSurface::new(
            "events.book",
            RouteSurfaceKind::FrontendAction,
            "/events/{event_slug}/book",
        )
        .gated_by(Capability::EventsBookingCreate),
        RouteSurface::new(
            "events.admin.index",
            RouteSurfaceKind::AdminPage,
            "/admin/events",
        )
        .gated_by(Capability::EventsEventPublish),
        RouteSurface::new(
            "events.admin.bookings",
            RouteSurfaceKind::AdminPage,
            "/admin/events/bookings",
        )
        .gated_by(Capability::EventsBookingCreate),
        RouteSurface::new(
            "events.admin.check-in",
            RouteSurfaceKind::AdminPage,
            "/admin/events/check-in",
        )
        .gated_by(Capability::EventsBookingCheckIn),
    ]
}
```

The key point is that the module provides the route names and paths:

- `events.list`
- `events.detail`
- `events.book`

This is the boundary between the official module and the customer app:

- the module defines the route surface
- the customer app provides the template override
- the runtime shapes the request model for that route

## 4. Replace The Runtime Fallback With Real Shoppr Templates

If the customer app did nothing, the runtime would inject honest fallback event templates. Shoppr
does not rely on those fallbacks anymore. It overrides the public event pages directly.

### `apps/shoppr/templates/events/list.html`

```html
<!doctype html>
<html xmlns:coil="https://coil.rs" coil:attr="lang=${locale}" lang="en-GB">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Events · Shoppr</title>
    <link rel="stylesheet" href="/theme/assets/site.css" coil:href="asset('theme/assets/site.css')" />
  </head>
  <body class="harbor events">
    <header class="site-header">
      <a href="/" class="brand">Shoppr</a>
      <nav coil:replace="~{navigation/primary}"></nav>
    </header>
    <main class="site-main">
      <section class="home-page events-page">
        <article class="catalog-section">
          <p class="catalog-section__eyebrow">Events</p>
          <h1>Event discovery now lives in the same product as memberships and editorial pages.</h1>
          <p>
            This route is no longer a placeholder. It now exposes real event cards, venue context,
            and timeslot summaries so the customer can understand what happens before booking begins.
          </p>
          <div class="checkout-actions">
            <a class="button" href="/account/memberships" coil:attr="href=${links.memberships}">
              Review memberships
            </a>
            <a
              class="button button--secondary"
              href="/en-GB/shop/collections/events"
              coil:attr="href=${links.events_collection}"
            >
              Browse event-linked offers
            </a>
          </div>
        </article>

        <section class="collection-grid__list" coil:if="${has_events}">
          <article class="catalog-section collection-grid__item" coil:each="event : ${events}">
            <p class="catalog-section__eyebrow" coil:text="${event.eyebrow}">Event</p>
            <h2>
              <a href="/en-GB/events/spring-tasting" coil:attr="href=${event.href}" coil:text="${event.title}">
                Spring Tasting Evening
              </a>
            </h2>
            <p coil:text="${event.summary}">
              Event summary.
            </p>
            <p>
              <strong coil:text="${event.day_label}">Thursday 11 April</strong>
              ·
              <span coil:text="${event.time_range_label}">18:30 to 20:30</span>
            </p>
            <p>
              <span coil:text="${event.venue_name}">Shoppr Townhouse</span>
              ·
              <span coil:text="${event.venue_city}">London</span>
              ·
              <span coil:text="${event.venue_mode}">In store</span>
            </p>
            <p coil:text="${event.availability_label}">
              Priority booking window open
            </p>
            <p coil:text="${event.audience_label}">
              Gold members book first
            </p>
            <p coil:text="${event.priority_note}">
              Priority note.
            </p>
            <div class="checkout-actions">
              <a class="button" href="/en-GB/events/spring-tasting" coil:attr="href=${event.href}">
                View event
              </a>
              <a
                class="button button--secondary"
                href="/account/memberships"
                coil:attr="href=${links.memberships}"
              >
                Membership access
              </a>
            </div>
          </article>
        </section>

        <article class="catalog-section" coil:unless="${has_events}">
          <p class="catalog-section__eyebrow">Events</p>
          <h2>No events are currently visible</h2>
          <p>
            This route is live, but there are no current event cards to render for this audience.
          </p>
        </article>
      </section>
    </main>
  </body>
</html>
```

### `apps/shoppr/templates/events/detail.html`

```html
<!doctype html>
<html xmlns:coil="https://coil.rs" coil:attr="lang=${locale}" lang="en-GB">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Event detail · Shoppr</title>
    <link rel="stylesheet" href="/theme/assets/site.css" coil:href="asset('theme/assets/site.css')" />
  </head>
  <body class="harbor events">
    <header class="site-header">
      <a href="/" class="brand">Shoppr</a>
      <nav coil:replace="~{navigation/primary}"></nav>
    </header>
    <main class="site-main">
      <section class="home-page events-page" coil:if="${has_event}">
        <article class="catalog-section">
          <p class="catalog-section__eyebrow" coil:text="${event.eyebrow}">Event</p>
          <h1 coil:text="${event.title}">Spring Tasting Evening</h1>
          <p coil:text="${event.summary}">
            Event summary.
          </p>
          <p>
            <strong coil:text="${event.day_label}">Thursday 11 April</strong>
            ·
            <span coil:text="${event.time_range_label}">18:30 to 20:30</span>
          </p>
          <p>
            <span coil:text="${event.venue_name}">Shoppr Townhouse</span>
            ·
            <span coil:text="${event.venue_city}">London</span>
            ·
            <span coil:text="${event.venue_mode}">In store</span>
          </p>
          <p coil:text="${event.availability_label}">
            Priority booking window open
          </p>
          <p coil:text="${event.audience_label}">
            Gold members book first
          </p>
          <p coil:text="${event.priority_note}">
            Priority note.
          </p>
          <div class="checkout-actions">
            <a class="button" href="/en-GB/events" coil:attr="href=${links.events}">Back to events</a>
            <a class="button button--secondary" href="/account" coil:attr="href=${links.account}">
              Open account
            </a>
            <a class="button button--secondary" href="/account/memberships" coil:attr="href=${links.memberships}">
              Review memberships
            </a>
          </div>
        </article>

        <article class="catalog-section">
          <p class="catalog-section__eyebrow">Timeslots</p>
          <h2>Choose the right session before booking starts</h2>
          <ol class="account-panel__list" coil:if="${event.has_timeslots}">
            <li coil:each="timeslot : ${event.timeslots}">
              <div>
                <strong coil:text="${timeslot.label}">Early tasting</strong>
                <span coil:text="${timeslot.starts_at_label}">18:30</span>
                to
                <span coil:text="${timeslot.ends_at_label}">19:15</span>
              </div>
              <p coil:text="${timeslot.availability_label}">
                4 seats remaining
              </p>
              <p coil:text="${timeslot.audience_label}">
                Gold members
              </p>
              <p coil:text="${timeslot.capacity_note}">
                Capacity note.
              </p>
            </li>
          </ol>
        </article>
      </section>

      <section class="home-page events-page" coil:unless="${has_event}">
        <article class="catalog-section">
          <p class="catalog-section__eyebrow">Event detail</p>
          <h1>Event unavailable</h1>
          <p>
            The requested event slug is not currently available for this site or locale.
          </p>
          <p>
            Missing slug:
            <strong coil:text="${missing_event_slug}">unknown-event</strong>
          </p>
          <div class="checkout-actions">
            <a class="button" href="/en-GB/events" coil:attr="href=${links.events}">Back to events</a>
            <a class="button button--secondary" href="/en-GB/shop/collections/events" coil:attr="href=${links.events_collection}">
              Browse event-linked offers
            </a>
          </div>
        </article>
      </section>
    </main>
  </body>
</html>
```

These two files are the public event UI.

What owns them:

- `apps/shoppr/templates/events/list.html`
  Owns the public event listing page.
- `apps/shoppr/templates/events/detail.html`
  Owns the public event detail page and timeslot section.

What bundle they use:

- both templates load `theme/assets/site.css`
- both are storefront pages, so they stay on the public frontend surface
- they do not load `admin.js` or `cms-editor.js`

What the list template needs from the runtime:

- `has_events`
- `events[]`
- `event.href`
- `event.title`
- `event.summary`
- `event.day_label`
- `event.time_range_label`
- `event.venue_name`
- `event.venue_city`
- `event.venue_mode`
- `event.availability_label`
- `event.audience_label`
- `event.priority_note`
- `links.memberships`
- `links.events_collection`

What the detail template needs from the runtime:

- `has_event`
- `event`
- `event.timeslots[]`
- `missing_event_slug`
- `links.events`
- `links.account`
- `links.memberships`
- `links.events_collection`

## 5. Event Operator Pages Reuse The Admin Shell

Shoppr also has real operator-facing event pages. They do not create a fourth bundle. They reuse
the shared admin shell, the shared admin navigation fragment, and the shared `admin.css` /
`admin.js` bundle pair.

### `apps/shoppr/templates/events/admin/index.html`

```html
<!doctype html>
<html
  xmlns:coil="https://coil.rs"
  coil:with="page_title='Event Operations'"
  lang="en-GB"
  coil:attr="lang=${locale}"
>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${page_title}">Event Operations</title>
    <link rel="stylesheet" href="/theme/assets/admin.css" coil:href="asset('theme/assets/admin.css')" />
    <script src="/theme/assets/admin.js" coil:src="asset('theme/assets/admin.js')" defer="defer"></script>
  </head>
  <body class="harbor harbor--admin">
    <header class="site-header">
      <a href="/" class="brand">Shoppr</a>
      <nav coil:replace="~{admin/nav}"></nav>
    </header>
    <main class="site-main admin-page">
      <section class="admin-page__intro">
        <p class="admin-page__eyebrow">Event operations</p>
        <h1 coil:text="${page.title}">Event Operations</h1>
        <p coil:text="${page.summary}">
          Operator overview of live events, slot pressure, and booking or waitlist actions.
        </p>
      </section>
    </main>
  </body>
</html>
```

### `apps/shoppr/templates/events/admin/bookings.html`

```html
<!doctype html>
<html
  xmlns:coil="https://coil.rs"
  coil:with="page_title='Event Bookings'"
  lang="en-GB"
  coil:attr="lang=${locale}"
>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${page_title}">Event Bookings</title>
    <link rel="stylesheet" href="/theme/assets/admin.css" coil:href="asset('theme/assets/admin.css')" />
    <script src="/theme/assets/admin.js" coil:src="asset('theme/assets/admin.js')" defer="defer"></script>
  </head>
  <body class="harbor harbor--admin">
    <header class="site-header">
      <a href="/" class="brand">Shoppr</a>
      <nav coil:replace="~{admin/nav}"></nav>
    </header>
    <main class="site-main admin-page">
      <section class="admin-page__intro">
        <p class="admin-page__eyebrow">Event operations</p>
        <h1 coil:text="${page.title}">Event Bookings</h1>
        <p coil:text="${page.summary}">
          Operator booking queue for held reservations, confirmed attendees, and waitlist follow-up.
        </p>
      </section>
    </main>
  </body>
</html>
```

### `apps/shoppr/templates/events/admin/check-in.html`

```html
<!doctype html>
<html
  xmlns:coil="https://coil.rs"
  coil:with="page_title='Event Check-In'"
  lang="en-GB"
  coil:attr="lang=${locale}"
>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${page_title}">Event Check-In</title>
    <link rel="stylesheet" href="/theme/assets/admin.css" coil:href="asset('theme/assets/admin.css')" />
    <script src="/theme/assets/admin.js" coil:src="asset('theme/assets/admin.js')" defer="defer"></script>
  </head>
  <body class="harbor harbor--admin">
    <header class="site-header">
      <a href="/" class="brand">Shoppr</a>
      <nav coil:replace="~{admin/nav}"></nav>
    </header>
    <main class="site-main admin-page">
      <section class="admin-page__intro">
        <p class="admin-page__eyebrow">Event operations</p>
        <h1 coil:text="${page.title}">Event Check-In</h1>
        <p coil:text="${page.summary}">
          Operator check-in lane for attendance readiness, reconciliation, and on-site follow-up.
        </p>
      </section>
    </main>
  </body>
</html>
```

What these operator files do:

- `events/admin/index.html`
  Owns the operator event slate.
- `events/admin/bookings.html`
  Owns the booking queue for reservations, confirmed rows, and waitlist pressure.
- `events/admin/check-in.html`
  Owns the on-site attendance lane.

Why they all load the same bundle:

- they are operator pages inside the shared admin shell
- they reuse `apps/shoppr/templates/admin/nav.html`
- they reuse the same filter and copy helpers already provided by `apps/shoppr/theme/frontend/admin.ts`
- the checked-in app does not need a separate event-ops bundle yet because the shared admin surface
  already covers the required interaction pattern

## 6. See Where The Runtime Populates The Event Model

The route surface comes from `coil-events`. The request-time page model comes from
`coil-runtime/src/render/model.rs`.

### `crates/coil-runtime/src/render/model.rs`

```rust
"events.list" => {
    let events = event_fixtures(locale, site_id, plan)
        .into_iter()
        .map(|event| event_model(&event))
        .collect::<Result<Vec<_>, _>>()?;
    model = model
        .with_object("audience", audience.audience.clone())?
        .with_bool("has_events", !events.is_empty())?
        .with_list("events", events)?;
}
"events.detail" => {
    let slug = params
        .get("event_slug")
        .map(String::as_str)
        .unwrap_or("spring-tasting");
    let event = event_fixtures(locale, site_id, plan)
        .into_iter()
        .find(|event| event.slug == slug);
    model = model.with_object("audience", audience.audience.clone())?;
    if let Some(event) = event {
        model = model
            .with_bool("has_event", true)?
            .with_object("event", event_model(&event)?)?;
    } else {
        model = model
            .with_bool("has_event", false)?
            .with_value("missing_event_slug", RenderValue::text(slug.to_string()))?;
    }
}
```

That is the runtime seam:

- the route name is already known
- the runtime decides whether this is a list or detail request
- the runtime prepares the exact keys that the template expects

The actual event card and timeslot values come from the event fixtures used by the checked-in
Shoppr sample:

### `crates/coil-runtime/src/render/model.rs`

```rust
fn event_model(event: &EventFixture) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("slug", RenderValue::text(event.slug.as_str()))?
        .with_value("title", RenderValue::text(event.title.as_str()))?
        .with_value("summary", RenderValue::text(event.summary.as_str()))?
        .with_value("eyebrow", RenderValue::text(event.eyebrow.as_str()))?
        .with_value("venue_name", RenderValue::text(event.venue_name.as_str()))?
        .with_value("venue_city", RenderValue::text(event.venue_city.as_str()))?
        .with_value("venue_mode", RenderValue::text(event.venue_mode.as_str()))?
        .with_value("day_label", RenderValue::text(event.day_label.as_str()))?
        .with_value(
            "time_range_label",
            RenderValue::text(event.time_range_label.as_str()),
        )?
        .with_value(
            "availability_label",
            RenderValue::text(event.availability_label.as_str()),
        )?
        .with_value("audience_label", RenderValue::text(event.audience_label.as_str()))?
        .with_value("priority_note", RenderValue::text(event.priority_note.as_str()))?
        .with_value("href", RenderValue::text(event.detail_href.as_str()))?
        .with_bool("has_timeslots", !event.timeslots.is_empty())?
        .with_value("timeslot_count", RenderValue::text(event.timeslots.len().to_string()))?
        .with_list(
            "timeslots",
            event
                .timeslots
                .iter()
                .map(event_slot_model)
                .collect::<Result<Vec<_>, _>>()?,
        )
}

fn event_fixtures(
    locale: &str,
    site_id: Option<&str>,
    plan: Option<&RuntimePlan>,
) -> Vec<EventFixture> {
    let detail_href = |slug: &str| {
        let fallback = format!("/{locale}/events/{slug}");
        plan.map_or_else(
            || fallback.clone(),
            |plan| {
                route_link(
                    plan,
                    site_id,
                    "events.detail",
                    &BTreeMap::from([("event_slug".to_string(), slug.to_string())]),
                    Some(locale),
                    &fallback,
                )
            },
        )
    };

    vec![
        EventFixture {
            slug: "spring-tasting".to_string(),
            title: "Spring Tasting Evening".to_string(),
            summary:
                "A guided tasting and edit preview built around event-linked products and member-first booking."
                    .to_string(),
            eyebrow: "Member event".to_string(),
            venue_name: "Shoppr Townhouse".to_string(),
            venue_city: "London".to_string(),
            venue_mode: "In store".to_string(),
            day_label: "Thursday 11 April".to_string(),
            time_range_label: "18:30 to 20:30".to_string(),
            availability_label: "Priority booking window open".to_string(),
            audience_label: "Gold members book first".to_string(),
            priority_note:
                "Gold members can secure early slots before the wider event-linked edit opens."
                    .to_string(),
            detail_href: detail_href("spring-tasting"),
            timeslots: vec![
                EventSlotFixture {
                    label: "Early tasting".to_string(),
                    starts_at_label: "18:30".to_string(),
                    ends_at_label: "19:15".to_string(),
                    availability_label: "4 seats remaining".to_string(),
                    audience_label: "Gold members".to_string(),
                    capacity_note: "Small-group seating keeps this tasting intimate.".to_string(),
                },
                EventSlotFixture {
                    label: "Main salon tasting".to_string(),
                    starts_at_label: "19:30".to_string(),
                    ends_at_label: "20:30".to_string(),
                    availability_label: "8 seats remaining".to_string(),
                    audience_label: "All active members".to_string(),
                    capacity_note:
                        "The later session opens after the priority allocation clears."
                            .to_string(),
                },
            ],
        },
    ]
}
```

This is the exact seam you need to understand:

- `app.toml` enables the official events module
- `coil-events` contributes the event routes
- Shoppr provides the full page templates
- `coil-runtime` shapes the event card and timeslot model for those templates

In the current checked-in app, the event records are runtime-provided sample state. The customer
templates are real. The routes are real. The locale-aware links are real. The timeslot rendering is
real. What is still sample-backed is where the event record data itself comes from.

## 7. Understand The Fallback Boundary

The runtime still keeps honest fallback templates for `events/list` and `events/detail` in
`crates/coil-runtime/src/builder/templates.rs`. Those fallbacks exist so a customer app can enable
the module before it has provided custom templates.

Shoppr does not use those fallback pages for the public event experience anymore because:

- `apps/shoppr/templates/events/list.html` exists
- `apps/shoppr/templates/events/detail.html` exists

That is the right customer-app boundary:

- the platform guarantees the route and runtime model seam
- the customer app owns the visible page design and copy

## 7. Runnable Checkpoint

Run the app:

```bash
cargo run --manifest-path apps/shoppr/Cargo.toml -p shoppr -- validate
cargo run --manifest-path apps/shoppr/Cargo.toml -p shoppr -- serve
```

Then verify:

- `/en-GB/events` renders event cards
- `/fr-FR/events/spring-tasting` renders the detail page
- the detail page shows real timeslot rows
- the route is localized through the same site/locale configuration as the rest of the storefront
- the page links back into memberships, account, and the event-linked catalog collection

You can also run the server-level proof that the checked-in app renders these routes end to end:

```bash
cargo test -p coil-runtime --lib server_host_renders_honest_checked_in_harbor_shop_events_surfaces
```

That test exercises the actual Shoppr event pages and asserts that the rendered responses include:

- event discovery copy
- concrete event titles
- venue and timeslot data
- locale-aware HTML output

## What This Chapter Proved

You did not add a fake `content/events` directory.

You used the actual checked-in Coil seam:

- official event routes from `coil-events`
- customer-owned event templates in `apps/shoppr/templates/events`
- request-time model shaping in `coil-runtime`
- existing event-linked product discovery in `catalog.toml`

That is the correct starting point for the next chapter. Booking can now build on top of a real
public event surface instead of a placeholder page.
