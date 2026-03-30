---
title: Add Sites, Markets, and Locales
---

This chapter makes the tutorial app visibly multi-site and locale-aware.

## Replace `app.toml`

At this point, replace the earlier single-site manifest with a concrete site-first file:

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
```

## Replace `platform.dev.toml`

The runtime config must match the same site model:

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

[[sites]]
id = "tutorial-uk"
brand_name = "Tutorial"
canonical_host = "uk.127.0.0.1.nip.io:8080"
hosts = ["www.127.0.0.1.nip.io:8080"]
default_locale = "en-GB"
supported_locales = ["en-GB"]

[[sites]]
id = "tutorial-fr"
brand_name = "Tutorial"
canonical_host = "fr.127.0.0.1.nip.io:8080"
hosts = []
default_locale = "fr-FR"
supported_locales = ["fr-FR"]
```

## Replace `templates/layouts/base.html`

Now update the shell so the site and locale split is visible in the UI:

```html
<!doctype html>
<html xmlns:coil="https://coil.rs" lang="en-GB" coil:attr="lang=${locale}">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${page.title}">Tutorial App</title>
    <link rel="stylesheet" href="/theme/assets/site.css" coil:href="asset('theme/assets/site.css')" />
  </head>
  <body>
    <a class="skip-link" href="#main">Skip to content</a>
    <header class="site-header">
      <a class="brand" href="/" coil:attr="href=${links.home}">Tutorial App</a>
      <nav aria-label="Primary">
        <a href="/" coil:attr="href=${links.home}">Home</a>
        <a href="/shop" coil:attr="href=${links.catalog}">Shop</a>
        <a href="/account" coil:attr="href=${links.account}">Account</a>
      </nav>
      <nav aria-label="Sites and locales" class="utility-nav">
        <a
          coil:each="switch : ${links.site_switches}"
          coil:attr="href=${switch.href}"
          coil:text="${switch.label}"
        >UK</a>
        <a
          coil:each="switch : ${links.locale_switches}"
          coil:attr="href=${switch.href}"
          coil:text="${switch.label}"
        >English</a>
      </nav>
    </header>
    <main id="main" class="site-main">
      <coil:block coil:insert="${content}">
        <p>Page content</p>
      </coil:block>
    </main>
    <footer class="site-footer">
      <small>Tutorial App</small>
    </footer>
  </body>
</html>
```

At this checkpoint three files need to agree with each other:

- `app.toml` defines the public site model
- `platform.dev.toml` defines the local host map
- `templates/layouts/base.html` exposes site and locale switching in the shell

## Checkpoint

Run the app and verify:

- `http://uk.127.0.0.1.nip.io:8080/en-GB/` works
- `http://fr.127.0.0.1.nip.io:8080/fr-FR/` works
- the shell visibly acknowledges site and locale switching

## What To Read Next

- [Add a Real Content Model](add-a-real-content-model.md)
