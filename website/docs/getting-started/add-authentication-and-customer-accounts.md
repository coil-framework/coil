---
title: Add Authentication and Customer Accounts
---

This chapter adds the first durable customer identity loop: registration, sign-in, session
continuity, and account pages.

The earlier chapters built public browsing, editorial content, dynamic blocks, and discovery. This
chapter adds the first personalized surface so the tutorial app can support memberships, bookings,
and operator-visible customer history later.

## Purpose

The tutorial app now needs:

- registration and sign-in routes
- a real auth package
- session-backed account pages
- profile editing
- a sign-out path

By the end of this chapter, the app should have a real account loop instead of only anonymous
public pages.

## Replace `app.toml`

The app manifest needs to make the auth package explicit and add account-facing routes to the
product shape:

```toml
name = "tutorial-app"
display_name = "Tutorial App"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]

[theme]
asset_roots = ["theme/assets"]

[auth]
package = "tutorial-auth"

[[modules]]
name = "admin"

[[modules]]
name = "cms"

[[modules]]
name = "commerce"

[[modules]]
name = "memberships"
```

### What matters in `app.toml`

The important sections are:

- `[auth]`
  This is what tells the app which auth package the runtime should load.
- `[[modules]] name = "memberships"`
  This is the first sign that the app is moving beyond public pages into account-aware flows.

This file does not define session cookie internals. It just declares that the app owns an auth
package and the module set needed for account features.

## Replace `platform.dev.toml`

The local runtime config needs to be explicit about the account/session environment:

```toml
[app]
name = "tutorial-app"
environment = "development"

[server]
bind = "127.0.0.1:8080"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]
localized_routes = true

[seo]
canonical_host = "www.127.0.0.1.nip.io:8080"

[database]
mode = "postgres"
url = "postgres://postgres:postgres@127.0.0.1:5432/tutorial_app"

[cache]
mode = "redis"
url = "redis://127.0.0.1:6379"

[jobs]
mode = "postgres"

[storage]
mode = "local"
local_root = ".coil/state"
```

### What matters in `platform.dev.toml`

This file still does the same runtime job as before, but now that job matters more:

- the database and cache backends need to be stable because sessions and account state depend on
  them
- the localized route setting matters because sign-in and account routes will inherit locale-aware
  URLs

The public auth contract comes from `app.toml`. The actual runtime environment still comes from
`platform.dev.toml`.

## Add An Auth Package Shape

Create `auth/tutorial-auth/schema.toml`:

```toml
[subjects.customer]
label = "Customer"

[[subjects.customer.fields]]
name = "customer_id"
type = "string"
required = true

[[subjects.customer.fields]]
name = "email"
type = "string"
required = true
```

Create `auth/tutorial-auth/policy.toml`:

```toml
[roles.customer]
subjects = ["customer"]

[capabilities.account.view]
label = "View account"

[capabilities.account.edit]
label = "Edit account"

[[grants]]
role = "customer"
capability = "account.view"

[[grants]]
role = "customer"
capability = "account.edit"
```

### What these auth files do

`schema.toml` defines the identity shape the app cares about:

- every signed-in customer has a `customer_id`
- every signed-in customer has an `email`

`policy.toml` defines the first account-facing capability boundary:

- customers can view their account
- customers can edit their account

This is the first chapter where the tutorial explicitly adds user identity as structured app data.

## Replace `templates/pages/sign-in.html`

Create a real sign-in page:

```html
<!doctype html>
<html xmlns:coil="https://coil.rs" coil:replace="~{layouts/base}">
  <body>
    <section class="card">
      <h1>Sign in</h1>
      <form method="post" action="/sign-in">
        <label>
          Email
          <input type="email" name="email" autocomplete="email" />
        </label>
        <label>
          Password
          <input type="password" name="password" autocomplete="current-password" />
        </label>
        <button class="button" type="submit">Sign in</button>
      </form>
    </section>
  </body>
</html>
```

## Replace `templates/pages/sign-up.html`

Create a real sign-up page:

```html
<!doctype html>
<html xmlns:coil="https://coil.rs" coil:replace="~{layouts/base}">
  <body>
    <section class="card">
      <h1>Create account</h1>
      <form method="post" action="/sign-up">
        <label>
          Full name
          <input type="text" name="full_name" autocomplete="name" />
        </label>
        <label>
          Email
          <input type="email" name="email" autocomplete="email" />
        </label>
        <label>
          Password
          <input type="password" name="password" autocomplete="new-password" />
        </label>
        <button class="button" type="submit">Create account</button>
      </form>
    </section>
  </body>
</html>
```

## Replace `templates/pages/account.html`

Create an account overview page:

```html
<!doctype html>
<html xmlns:coil="https://coil.rs" coil:replace="~{layouts/base}">
  <body>
    <section class="hero">
      <p class="eyebrow">Account</p>
      <h1 coil:text="${account.full_name}">Customer name</h1>
      <p coil:text="${account.email}">customer@example.com</p>
    </section>

    <section class="content-rail">
      <article class="card">
        <h2>Profile</h2>
        <form method="post" action="/account/profile">
          <label>
            Full name
            <input type="text" name="full_name" coil:attr="value=${account.full_name}" />
          </label>
          <label>
            Phone
            <input type="text" name="phone" coil:attr="value=${account.phone}" />
          </label>
          <button class="button" type="submit">Save profile</button>
        </form>
      </article>

      <article class="card">
        <h2>Session</h2>
        <form method="post" action="/sign-out">
          <button class="button button--secondary" type="submit">Sign out</button>
        </form>
      </article>
    </section>
  </body>
</html>
```

### What these templates do

`sign-in.html` and `sign-up.html` introduce the public account entry points.

The important parts are:

- `method="post"`
  Auth actions should be mutations, not GET links.
- concrete field names like `email`, `password`, and `full_name`
  These make the workflow readable and give the tutorial a real form contract.

`account.html` introduces the first principal-aware page.

The important fields are:

- `${account.full_name}`
  This proves the runtime is resolving customer-specific state for the signed-in principal.
- the profile form
  This makes account state mutable, not only viewable.
- the sign-out form
  This gives the session a concrete end point.

## Replace `crates/tutorial-app-backend/src/lib.rs`

The customer backend now needs to support account-aware behavior alongside the earlier discovery and
dynamic-block examples.

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

pub fn account_overview_model(
    _request: &RequestContext,
) -> Result<BTreeMap<String, String>, BackendError> {
    Ok(BTreeMap::from([
        ("full_name".to_string(), "Alex Parker".to_string()),
        ("email".to_string(), "alex@example.com".to_string()),
        ("phone".to_string(), "+44 20 0000 0000".to_string()),
    ]))
}
```

### What matters in the backend file

The important new function is `account_overview_model(...)`.

It is the first explicit example of principal-aware account shaping in the tutorial. In a real app,
that model would come from authenticated customer state. For this step, it is enough to make the
account template contract clear:

- the account page is not static
- it needs per-customer runtime data
- the customer backend is the right place for app-specific account shaping

## What Behavior This Enables

Once these files exist together:

- the app has real sign-in and sign-up surfaces
- the account area becomes a concrete personalized destination
- auth package ownership is visible in committed files
- the tutorial now has a stable base for membership gating, bookings, passes, and operator-visible
  customer state later

This is the chapter where the tutorial stops being only a public site and starts becoming a real
customer product.

## Checkpoint

Run:

```bash
cargo run -p tutorial-app-bin -- validate
cargo run -p tutorial-app-bin -- serve
```

Then verify:

- the app still validates with the auth package in place
- the sign-in and sign-up pages render
- the account page renders principal-aware state
- the account page includes both profile editing and sign-out actions

## What To Read Next

- [Add Memberships and Audience Gating](add-memberships-and-audience-gating.md)
