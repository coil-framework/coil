---
title: Build the Base Theme
---

This chapter replaces the generated placeholder shell with a real storefront layout, a real home
page, and the first storefront frontend bundle.

## Purpose

At the end of this chapter:

- every public page renders inside one shared storefront layout
- the layout loads the compiled storefront CSS and JavaScript bundles
- the app has one concrete storefront controller entrypoint
- the home page has real public entry points into commerce and membership flows

## Replace `templates/layouts/base.html`

This file defines the storefront shell and loads the storefront bundle.

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

- `asset('theme/assets/site.css')`
  Loads the compiled storefront stylesheet.
- `asset('theme/assets/site.js')`
  Loads the compiled storefront JavaScript bundle.
- `${links.home}`, `${links.catalog}`, `${links.account}`
  Keep navigation server-owned.
- `<coil:block coil:insert="${content}">`
  Marks where child pages render.

What you should edit:

- brand label
- primary navigation
- shell-wide layout and semantics

What this enables:

- one shared storefront shell
- one consistent place for the public bundle load

## Replace `templates/pages/home.html`

This file becomes the first real public route in the app.

```html
<!doctype html>
<html xmlns:coil="https://coil.rs" coil:replace="~{layouts/base}">
  <body>
    <section class="hero" data-controller="site--interactive">
      <p class="eyebrow">New season</p>
      <h1>Retail, memberships, and editorial content in one customer app.</h1>
      <p>
        Start with a branded shell now so later CMS pages, commerce flows, and reusable blocks have
        a real home.
      </p>
      <div class="hero-actions">
        <a class="button" href="/shop">Shop now</a>
        <a class="button button--secondary" href="/pages/membership">Memberships</a>
      </div>
    </section>

    <section class="content-rail">
      <article class="card">
        <h2>Editorial landing pages</h2>
        <p>Later chapters replace hardcoded sections with structured CMS pages and reusable blocks.</p>
      </article>
      <article class="card">
        <h2>Multiple sites and locales</h2>
        <p>The shell is simple now, but it is ready for site and locale switching.</p>
      </article>
      <article class="card">
        <h2>Linked customer Rust</h2>
        <p>The app crate and backend crate stay visible from the start instead of becoming afterthoughts.</p>
      </article>
    </section>
  </body>
</html>
```

What each section does:

- `coil:replace="~{layouts/base}"`
  Renders the home page inside the storefront shell.
- `data-controller="site--interactive"`
  Gives the storefront bundle a stable place to attach.
- `.hero`
  Creates the first public content block.
- `.hero-actions`
  Establishes public route entry points the rest of the tutorial will fill in.

## Replace `theme/frontend/site.ts`

This file defines the first storefront controller bundle.

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
  }
}

document.body.dataset.controller = [document.body.dataset.controller, "site--interactive"]
  .filter(Boolean)
  .join(" ");

const app = Application.start();
app.register("site--interactive", SiteInteractiveController);
```

What each section does:

- `import "@hotwired/turbo"`
  Enables Turbo on public pages.
- `Application.start()`
  Starts Stimulus for the storefront shell.
- `SiteInteractiveController`
  Owns small storefront-only interaction.
- `document.body.dataset.controller = ...`
  Attaches the storefront controller to the rendered page.

What you should edit:

- public page interaction only
- keep business state on the server and use this file for browser behavior

## Replace `theme/frontend/site.css`

This file defines the source stylesheet for the storefront bundle.

```css
:root {
  --bg: #f5f1e8;
  --panel: #fffaf2;
  --ink: #1e1c18;
  --muted: #6b6458;
  --accent: #1f5f4a;
  --accent-contrast: #f7fff9;
  --border: #d8cfbf;
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
  background: var(--bg);
  color: var(--ink);
  font: 16px/1.5 Georgia, serif;
}

.skip-link {
  position: absolute;
  left: -9999px;
}

.skip-link:focus {
  left: 1rem;
  top: 1rem;
  background: var(--panel);
  padding: 0.75rem 1rem;
}

.site-header,
.site-footer,
.site-main {
  width: min(72rem, calc(100% - 2rem));
  margin: 0 auto;
}

.site-header,
.site-footer {
  display: flex;
  justify-content: space-between;
  gap: 1rem;
  padding: 1.25rem 0;
}

.hero,
.card {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 1rem;
}

.hero {
  padding: 2rem;
  margin: 2rem 0;
}

.content-rail {
  display: grid;
  gap: 1rem;
  grid-template-columns: repeat(auto-fit, minmax(16rem, 1fr));
  margin-bottom: 2rem;
}

.card {
  padding: 1.25rem;
}

.button {
  display: inline-block;
  padding: 0.75rem 1rem;
  border-radius: 999px;
  background: var(--accent);
  color: var(--accent-contrast);
  text-decoration: none;
}

.button--secondary {
  background: transparent;
  color: var(--ink);
  border: 1px solid var(--border);
}
```

What each section does:

- `:root`
  Defines reusable design tokens.
- `.site-header`, `.site-footer`, `.site-main`
  Create the base layout width.
- `.hero`, `.card`, `.button`
  Create the first public UI primitives.

Important boundary:

- edit `theme/frontend/site.css`
- build to produce `theme/assets/site.css`
- do not edit `theme/assets/site.css` directly

## Build The Storefront Bundle

Run:

```bash
npm run build
```

That command compiles:

- `theme/frontend/site.ts` -> `theme/assets/site.js`
- `theme/frontend/site.css` -> `theme/assets/site.css`

## Runnable Checkpoint

Run:

```bash
npm run build
cargo run -p tutorial-app-bin -- validate
cargo run -p tutorial-app-bin -- serve
```

Then verify:

- the header and footer render on the page
- the home page shows the hero and content cards
- the layout loads `site.css` and `site.js`
- the public page still works as a normal server-rendered page even if the browser-side behavior is minimal

## What To Read Next

- [Add Sites, Markets, and Locales](../add-sites-markets-and-locales/)
