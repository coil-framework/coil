---
title: Build the Base Theme
---

This chapter is the first visible UI step. The goal is to establish a real shell that later pages,
accounts, and CMS content can live inside.

## Replace `templates/layouts/base.html`

Create or replace `templates/layouts/base.html` with a layout that already has usable landmarks:

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
    </header>
    <main id="main" class="site-main">
      <coil:block coil:insert="${content}">
        <p>Page content</p>
      </coil:block>
    </main>
    <footer class="site-footer">
      <nav aria-label="Footer">
        <a href="/pages/about">About</a>
        <a href="/pages/shipping">Shipping</a>
        <a href="/pages/support">Support</a>
      </nav>
      <small>Tutorial App</small>
    </footer>
  </body>
</html>
```

## Replace `templates/pages/home.html`

The home page should already look product-owned:

```html
<!doctype html>
<html xmlns:coil="https://coil.rs" coil:replace="~{layouts/base}">
  <body>
    <section class="hero">
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

## Replace `theme/assets/site.css`

Create or replace `theme/assets/site.css`:

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

At the end of this chapter, these three files should exist as real files in the tutorial app:

```text
templates/layouts/base.html
templates/pages/home.html
theme/assets/site.css
```

## Checkpoint

Run:

```bash
cargo run -p tutorial-app-bin -- validate
cargo run -p tutorial-app-bin -- serve
```

At the end of this chapter, a reviewer should be able to open the app and see a branded shell, a
usable home page, and layout landmarks that later pages can inherit.

## What To Read Next

- [Add Sites, Markets, and Locales](add-sites-markets-and-locales.md)
