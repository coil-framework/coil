---
title: Add Sites, Markets, and Locales
---

This chapter moves the app from one host and one locale to a site-first configuration with a
storefront shell that exposes site and locale switching.

## Purpose

At the end of this chapter:

- the app has two sites
- each site has its own canonical host
- each site has its own locale contract
- locale-prefixed routes are enabled
- the storefront shell exposes site and locale switching
- the storefront bundle owns the panel interaction for those switches

## Replace `app.toml`

`app.toml` now becomes site-first:

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

What each section does:

- `[i18n]`
  Declares the full locale set the app supports.
- first `[[sites]]`
  Creates the UK site and limits it to `en-GB`.
- second `[[sites]]`
  Creates the France site and limits it to `fr-FR`.

## Replace `platform.dev.toml`

The local runtime config needs matching site definitions with local hosts:

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

What each section does:

- `[i18n].localized_routes = true`
  Turns on locale-prefixed URLs.
- `[[sites]]`
  Maps each site id to a local host and locale set.

## Replace `templates/layouts/base.html`

Update the storefront shell so users can see and use the site and locale model.

```html
<!doctype html>
<html xmlns:coil="https://coil.rs" lang="en-GB" coil:attr="lang=${locale}">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${page_title}">Tutorial App</title>
    <link rel="stylesheet" href="/theme/assets/site.css" coil:href="asset('theme/assets/site.css')" />
    <script src="/theme/assets/site.js" coil:src="asset('theme/assets/site.js')" defer="defer"></script>
  </head>
  <body class="tutorial-shell">
    <a class="skip-link" href="#main">Skip to content</a>
    <header class="site-header">
      <div class="site-header__main">
        <a class="brand" href="/" coil:attr="href=${links.home}">
          <span class="brand__mark">T</span>
          <span class="brand__wordmark">Tutorial App</span>
        </a>
        <nav aria-label="Primary">
          <a href="/" coil:attr="href=${links.home}">Home</a>
          <a href="/shop" coil:attr="href=${links.catalog}">Shop</a>
          <a href="/account" coil:attr="href=${links.account}">Account</a>
        </nav>
        <div class="utility-nav">
          <button type="button" class="button button--secondary" data-panel-toggle="market-panel" aria-expanded="false">
            Markets
          </button>
          <button type="button" class="button button--secondary" data-panel-toggle="locale-panel" aria-expanded="false">
            Language
          </button>
        </div>
      </div>
      <div class="site-header__panels">
        <div class="switcher-panel" id="market-panel" hidden="hidden">
          <p class="eyebrow">Market</p>
          <ul>
            <li coil:each="item : ${links.site_switches}">
              <a href="/" coil:attr="href=${item.href}" coil:text="${item.label}">Tutorial UK</a>
              <span coil:if="${item.active}">Current</span>
            </li>
          </ul>
        </div>
        <div class="switcher-panel" id="locale-panel" hidden="hidden">
          <p class="eyebrow">Language</p>
          <ul>
            <li coil:each="item : ${links.locale_switches}">
              <a href="/" coil:attr="href=${item.href}" coil:text="${item.label}">English</a>
              <span coil:if="${item.active}">Current</span>
            </li>
          </ul>
        </div>
      </div>
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

What each section does:

- the head still loads the storefront bundle, not a new site-specific bundle
- `${links.site_switches}`
  Renders runtime-generated site links
- `${links.locale_switches}`
  Renders runtime-generated locale links
- `data-panel-toggle`
  Gives `site.ts` a stable controller seam for switcher-panel behavior

## Update `theme/frontend/site.ts`

The storefront controller now owns panel toggles for site and locale switching.

```ts
import "@hotwired/turbo";
import { Application, Controller } from "@hotwired/stimulus";

class SiteInteractiveController extends Controller<HTMLElement> {
  connect() {
    this.bindPanelToggles();
  }

  private bindPanelToggles() {
    this.element.querySelectorAll<HTMLElement>("[data-panel-toggle]").forEach((button) => {
      button.addEventListener("click", () => {
        const panelId = button.getAttribute("data-panel-toggle");
        const panel = panelId ? document.getElementById(panelId) : null;
        if (!panel) return;

        const isOpen = !panel.hasAttribute("hidden");
        document.querySelectorAll<HTMLElement>(".switcher-panel").forEach((entry) => {
          entry.setAttribute("hidden", "");
        });
        document.querySelectorAll<HTMLElement>("[data-panel-toggle]").forEach((entry) => {
          entry.setAttribute("aria-expanded", "false");
        });

        if (!isOpen) {
          panel.removeAttribute("hidden");
          button.setAttribute("aria-expanded", "true");
        }
      });
    });

    document.addEventListener("click", (event) => {
      const target = event.target;
      if (!(target instanceof HTMLElement)) return;
      if (target.closest(".switcher-panel") || target.closest("[data-panel-toggle]")) return;
      document.querySelectorAll<HTMLElement>(".switcher-panel").forEach((entry) => {
        entry.setAttribute("hidden", "");
      });
      document.querySelectorAll<HTMLElement>("[data-panel-toggle]").forEach((entry) => {
        entry.setAttribute("aria-expanded", "false");
      });
    });
  }
}

document.body.dataset.controller = [document.body.dataset.controller, "site--interactive"]
  .filter(Boolean)
  .join(" ");

const app = Application.start();
app.register("site--interactive", SiteInteractiveController);
```

What this file now enables:

- the site and locale controls still work as plain links without JavaScript
- when JavaScript is present, the bundle opens and closes the switcher panels

## Rebuild The Storefront Bundle

Run:

```bash
npm run build
```

## Runnable Checkpoint

Run:

```bash
npm run build
cargo run -p tutorial-app-bin -- validate
cargo run -p tutorial-app-bin -- serve
```

Then verify:

- `http://uk.127.0.0.1.nip.io:8080/en-GB/` loads
- `http://fr.127.0.0.1.nip.io:8080/fr-FR/` loads
- the header renders site-switch and locale-switch links
- the panel buttons open and close through the storefront bundle

## What To Read Next

- [Add a Real Content Model](../add-a-real-content-model/)
