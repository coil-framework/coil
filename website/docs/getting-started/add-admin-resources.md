---
title: Add Admin Resources
---

This chapter uses the real Shoppr admin and CMS editor files to show how Coil keeps operator
surfaces on separate bundles from the public storefront.

## Purpose

At the end of this chapter:

- the app has an admin dashboard template
- the app has a CMS editor template
- the app has dedicated jobs and integrations templates under `templates/admin/`
- the app has event operator pages under `templates/events/admin/`
- the app has ops pages under `templates/ops/`
- admin pages load `admin.css` and `admin.js`
- CMS editor pages load `cms-editor.css` and `cms-editor.js`
- the public storefront continues to load only `site.css` and `site.js`

## `apps/shoppr/theme/frontend/admin.ts`

`admin.ts` is the general operator bundle. Any admin page can load it without also pulling in CMS-editor-specific behavior.

```ts title="apps/shoppr/theme/frontend/admin.ts"
import "@hotwired/turbo";
import { Application, Controller } from "@hotwired/stimulus";

class AdminInteractiveController extends Controller<HTMLElement> {
  connect() {
    this.bindFilters();
    this.bindCopyButtons();
  }

  private bindFilters() {
    this.element.querySelectorAll<HTMLElement>("[data-admin-filter]").forEach((scope) => {
      const input = scope.querySelector<HTMLInputElement>("[data-admin-filter-input]");
      if (!input) return;

      const applyFilter = () => {
        const query = input.value.trim().toLowerCase();
        scope.querySelectorAll<HTMLElement>("[data-admin-filter-item]").forEach((item) => {
          const matches = item.textContent?.toLowerCase().includes(query) ?? false;
          item.toggleAttribute("hidden", !matches);
        });
      };

      input.addEventListener("input", applyFilter);
      applyFilter();
    });
  }

  private bindCopyButtons() {
    this.element.querySelectorAll<HTMLButtonElement>("[data-copy-text]").forEach((button) => {
      button.addEventListener("click", async () => {
        const value = button.dataset.copyText;
        if (!value) return;
        try {
          await navigator.clipboard.writeText(value);
          const original = button.textContent;
          button.textContent = "Copied";
          window.setTimeout(() => {
            if (original) button.textContent = original;
          }, 1200);
        } catch {
          button.textContent = "Copy failed";
        }
      });
    });
  }
}

document.body.dataset.controller = [document.body.dataset.controller, "admin--interactive"]
  .filter(Boolean)
  .join(" ");

const app = Application.start();
app.register("admin--interactive", AdminInteractiveController);
```

What this file is responsible for:

- importing Turbo so admin links and forms can progressively enhance cleanly
- starting Stimulus for operator pages
- attaching one controller namespace to the page body
- binding the exact filter and copy helpers used by the checked-in admin templates

## `apps/shoppr/theme/frontend/cms-editor.ts`

`cms-editor.ts` is the CMS-specific bundle. It keeps the general admin behavior and adds editor-only controls on top.

```ts title="apps/shoppr/theme/frontend/cms-editor.ts"
import "@hotwired/turbo";
import { Application, Controller } from "@hotwired/stimulus";

class AdminInteractiveController extends Controller<HTMLElement> {
  connect() {
    this.bindFilters();
    this.bindCopyButtons();
  }

  private bindFilters() {
    this.element.querySelectorAll<HTMLElement>("[data-admin-filter]").forEach((scope) => {
      const input = scope.querySelector<HTMLInputElement>("[data-admin-filter-input]");
      if (!input) return;

      const applyFilter = () => {
        const query = input.value.trim().toLowerCase();
        scope.querySelectorAll<HTMLElement>("[data-admin-filter-item]").forEach((item) => {
          const matches = item.textContent?.toLowerCase().includes(query) ?? false;
          item.toggleAttribute("hidden", !matches);
        });
      };

      input.addEventListener("input", applyFilter);
      applyFilter();
    });
  }

  private bindCopyButtons() {
    this.element.querySelectorAll<HTMLButtonElement>("[data-copy-text]").forEach((button) => {
      button.addEventListener("click", async () => {
        const value = button.dataset.copyText;
        if (!value) return;
        try {
          await navigator.clipboard.writeText(value);
          const original = button.textContent;
          button.textContent = "Copied";
          window.setTimeout(() => {
            if (original) button.textContent = original;
          }, 1200);
        } catch {
          button.textContent = "Copy failed";
        }
      });
    });
  }
}

class CmsEditorController extends Controller<HTMLElement> {
  connect() {
    this.updateSummary();
  }

  toggleBlock(event: Event) {
    const target = event.currentTarget;
    if (!(target instanceof HTMLButtonElement)) return;
    const card = target.closest<HTMLElement>("[data-block-card]");
    if (!card) return;
    const collapsed = card.dataset.collapsed === "true";
    card.dataset.collapsed = collapsed ? "false" : "true";
    card.classList.toggle("admin-card--collapsed", !collapsed);
    target.textContent = collapsed ? "Collapse" : "Expand";
  }

  expandAll() {
    this.setAllCollapsed(false);
  }

  collapseAll() {
    this.setAllCollapsed(true);
  }

  private setAllCollapsed(collapsed: boolean) {
    this.element.querySelectorAll<HTMLElement>("[data-block-card]").forEach((card) => {
      card.dataset.collapsed = collapsed ? "true" : "false";
      card.classList.toggle("admin-card--collapsed", collapsed);
      const button = card.querySelector<HTMLButtonElement>("[data-block-toggle]");
      if (button) {
        button.textContent = collapsed ? "Expand" : "Collapse";
      }
    });
    this.updateSummary();
  }

  private updateSummary() {
    const cards = Array.from(this.element.querySelectorAll<HTMLElement>("[data-block-card]"));
    const enabled = cards.filter((card) => card.dataset.blockEnabled === "true").length;
    const disabled = cards.length - enabled;
    const summary = this.element.querySelector<HTMLElement>("[data-cms-block-summary]");
    if (summary) {
      summary.textContent = `${cards.length} blocks, ${enabled} enabled, ${disabled} disabled`;
    }
  }
}

document.body.dataset.controller = [document.body.dataset.controller, "admin--interactive cms--editor"]
  .filter(Boolean)
  .join(" ");

const app = Application.start();
app.register("admin--interactive", AdminInteractiveController);
app.register("cms--editor", CmsEditorController);
```

What this file is responsible for:

- reusing the shared admin behavior
- adding block-editor affordances that only belong on CMS pages
- attaching both controllers to the body so the editor gets the shared admin helpers plus its own controls

## `apps/shoppr/templates/admin/dashboard.html`

The dashboard template shows the main pattern to copy: admin pages load the admin bundle in the
`<head>` and then declare small HTML hooks the controller can enhance.

```html title="apps/shoppr/templates/admin/dashboard.html"
<!doctype html>
<html
  xmlns:coil="https://coil.rs"
  coil:with="page_title='Shoppr Admin'"
  lang="en-GB"
  coil:attr="lang=${locale}"
>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${page_title}">Shoppr Admin</title>
    <link rel="stylesheet" href="/theme/assets/admin.css" coil:href="asset('theme/assets/admin.css')" />
    <script src="/theme/assets/admin.js" coil:src="asset('theme/assets/admin.js')" defer="defer"></script>
  </head>
  <body class="harbor harbor--admin">
    <header class="site-header">
      <a href="/" class="brand">Shoppr</a>
      <nav coil:replace="~{admin/nav}"></nav>
    </header>
    <main class="site-main admin-page" data-admin-filter="">
      <section class="admin-page__intro">
        <p class="admin-page__eyebrow">Operator dashboard</p>
        <h1 coil:text="${page.title}">Shoppr Admin</h1>
        <p coil:text="${page.summary}">
          Operator overview for catalog, orders, and content.
        </p>
      </section>

      <label class="admin-filter">
        <span>Filter tasks and control-room cards</span>
        <input
          type="search"
          placeholder="Filter by orders, catalog, content, launch, storefront..."
          data-admin-filter-input=""
        />
      </label>

      <div class="admin-card-grid">
        <article class="admin-card" data-admin-filter-item="">
          <h2>Review storefront orders</h2>
          <p>
            Check whether checkout is producing a visible order queue with the same reference,
            status, and total that customers see in the storefront account area.
          </p>
          <a class="button" href="/admin/orders" coil:attr="href=${links.admin_orders}">
            Open orders
          </a>
        </article>
        <article class="admin-card" data-admin-filter-item="">
          <h2>Manage live catalog copy</h2>
          <p>
            Update product titles, summaries, prices, and collection placement from the admin
            catalog screen, then verify the same browse loop customers will use.
          </p>
          <a class="button" href="/admin/catalog/products" coil:attr="href=${links.admin_catalog}">
            Open catalog
          </a>
        </article>
      </div>
    </main>
  </body>
</html>
```

The dashboard is not the whole admin shell. The checked-in Shoppr app now routes five operator
lanes through the same bundle and navigation contract:

- commerce support in `apps/shoppr/templates/commerce/orders.html` and `apps/shoppr/templates/commerce/payments.html`
- CRM and memberships in `apps/shoppr/templates/admin/customers.html` and `apps/shoppr/templates/memberships/`
- event operations in `apps/shoppr/templates/events/admin/`
- CMS/editor work in `apps/shoppr/templates/cms/`
- ops surfaces in `apps/shoppr/templates/ops/search.html` and `apps/shoppr/templates/ops/reports.html`
- job and integration inventory in `apps/shoppr/templates/admin/jobs.html` and `apps/shoppr/templates/admin/integrations.html`

That is the pattern to preserve:

- one shared admin bundle for normal operator pages
- one separate CMS-editor bundle for block-editor behavior
- route-specific templates that stay small and HTML-first

What this file proves:

- admin pages load `admin.css` and `admin.js`
- operator UI can use separate controllers and styles from the public storefront
- the rest of the document is still normal SSR HTML, not a client-rendered admin app

What you edit versus what the platform owns:

- You edit route templates like `templates/admin/dashboard.html`, `templates/ops/search.html`, and
  `templates/ops/reports.html` to explain the workflow in business terms.
- You edit `templates/admin/nav.html` to decide which operator lanes belong in the shared shell.
- The platform owns the route plumbing, request handling, render-model shaping, and any real queue
  or export mechanics behind those pages.
- The shared operator shell is where bulk actions, exports, and recovery lanes should appear. Do
  not fork a second browser app just because one page talks about reindex or report export.

## `apps/shoppr/templates/events/admin/bookings.html`

This file shows the next step after a general dashboard: a route-specific operator queue that still
uses the shared admin bundle instead of creating a second browser app for event workflows.

```html title="apps/shoppr/templates/events/admin/bookings.html"
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
        <h1>Event Bookings</h1>
        <p>
          Operator view of live event bookings, held reservations, and waitlist pressure.
        </p>
      </section>

      <section class="admin-panel" coil:if="${has_event_booking_rows}" data-admin-filter="">
        <p class="admin-panel__eyebrow">Booking queue</p>
        <label class="admin-filter">
          <span>Filter by booking, customer, event, or state</span>
          <input type="search" placeholder="Filter booking queue..." data-admin-filter-input="" />
        </label>
        <table class="admin-table">
          <thead>
            <tr>
              <th scope="col">Booking</th>
              <th scope="col">Customer</th>
              <th scope="col">Event</th>
              <th scope="col">State</th>
              <th scope="col">Actions</th>
            </tr>
          </thead>
          <tbody>
            <tr coil:each="booking : ${event_booking_rows}" data-admin-filter-item="">
              <td>
                <div class="admin-copy-row">
                  <strong coil:text="${booking.reference}">EVT-2001</strong>
                  <button class="button button--secondary" type="button" coil:attr="data-copy-text=${booking.reference}">
                    Copy ref
                  </button>
                </div>
                <div coil:text="${booking.booking_state_label}">Reservation held</div>
              </td>
              <td>
                <strong coil:text="${booking.customer_name}">Morgan Rowe</strong>
                <div class="admin-copy-row">
                  <span coil:text="${booking.customer_email}">morgan@example.com</span>
                  <button class="button button--secondary" type="button" coil:attr="data-copy-text=${booking.customer_email}">
                    Copy email
                  </button>
                </div>
              </td>
              <td>
                <strong coil:text="${booking.event_title}">Spring Tasting Evening</strong>
                <div coil:text="${booking.slot_label}">Early tasting</div>
              </td>
              <td coil:text="${booking.support_note}">Needs confirmation before hold expiry</td>
              <td>
                <a class="button button--secondary" href="/admin/events/check-in" coil:attr="href=${booking.check_in_href}">
                  Open check-in
                </a>
                <a class="button button--secondary" href="/events/spring-tasting" coil:attr="href=${booking.event_preview_href}">
                  Preview event
                </a>
              </td>
            </tr>
          </tbody>
        </table>
      </section>
    </main>
  </body>
</html>
```

What this file proves:

- route-specific operator screens can stay on the shared admin bundle
- the queue remains server-rendered HTML instead of client-rendered table state
- copy and filter behavior still comes from HTML hooks plus `admin.js`
- event operations stay inside the same admin shell as commerce and CMS
- operator follow-up can move from event state into bulk or recovery work without changing shells

## `apps/shoppr/templates/admin/nav.html`

This fragment owns the shared operator route map for commerce, memberships, events, content, and
operations.

```html title="apps/shoppr/templates/admin/nav.html"
<nav class="admin-nav" xmlns:coil="https://coil.rs" coil:fragment="primary" aria-label="Admin navigation">
  <div class="admin-nav__group">
    <p class="admin-nav__label">Commerce</p>
    <a class="admin-nav__link" href="/admin" coil:attr="href=${links.admin_dashboard}">Dashboard</a>
    <a class="admin-nav__link" href="/admin/customers" coil:attr="href=${links.admin_customers}">
      Customers
    </a>
    <a class="admin-nav__link" href="/admin/orders" coil:attr="href=${links.admin_orders}">
      Orders
    </a>
    <a
      class="admin-nav__link"
      href="/admin/catalog/products"
      coil:attr="href=${links.admin_catalog}"
    >
      Catalog
    </a>
  </div>
  <div class="admin-nav__group">
    <p class="admin-nav__label">Memberships</p>
    <a
      class="admin-nav__link"
      href="/admin/memberships/subscriptions"
      coil:attr="href=${links.admin_membership_subscriptions}"
    >
      Subscriptions
    </a>
    <a
      class="admin-nav__link"
      href="/admin/memberships/tiers"
      coil:attr="href=${links.admin_membership_tiers}"
    >
      Tiers
    </a>
  </div>
  <div class="admin-nav__group">
    <p class="admin-nav__label">Events</p>
    <a class="admin-nav__link" href="/admin/events" coil:attr="href=${links.admin_events}">
      Events
    </a>
    <a
      class="admin-nav__link"
      href="/admin/events/bookings"
      coil:attr="href=${links.admin_event_bookings}"
    >
      Bookings
    </a>
    <a
      class="admin-nav__link"
      href="/admin/events/check-in"
      coil:attr="href=${links.admin_event_check_in}"
    >
      Check-in
    </a>
  </div>
  <div class="admin-nav__group">
    <p class="admin-nav__label">Content</p>
    <a class="admin-nav__link" href="/admin/pages" coil:attr="href=${links.admin_pages}">
      Pages
    </a>
    <a
      class="admin-nav__link"
      href="/admin/navigation"
      coil:attr="href=${links.admin_navigation}"
    >
      Navigation
    </a>
    <a
      class="admin-nav__link"
      href="/admin/redirects"
      coil:attr="href=${links.admin_redirects}"
    >
      Redirects
    </a>
    <a
      class="admin-nav__link"
      href="/admin/options"
      coil:attr="href=${links.admin_options}"
    >
      Settings
    </a>
    <a class="admin-nav__link" href="/admin/audit" coil:attr="href=${links.admin_audit}">
      Audit
    </a>
  </div>
  <div class="admin-nav__group">
    <p class="admin-nav__label">Operations</p>
    <a class="admin-nav__link" href="/admin/search" coil:attr="href=${links.admin_search}">
      Search
    </a>
    <a class="admin-nav__link" href="/admin/reports" coil:attr="href=${links.admin_reports}">
      Reports
    </a>
  </div>
</nav>
```

What this file does:

- keeps event operator routes inside the same admin shell as commerce and CMS
- gives `/admin`, `/admin/events`, `/admin/search`, and `/admin/reports` one shared navigation
  fragment
- makes bulk operational actions discoverable in the same shell operators already use for support
  and publishing
- avoids a second standalone operator shell for search, recovery, or report work

## `apps/shoppr/templates/ops/search.html`

This page is the checked-in reindex and recovery seam. It does not blindly start a rebuild from
the browser. It shows operators whether they should trigger one.

```html title="apps/shoppr/templates/ops/search.html"
<!doctype html>
<html
  xmlns:coil="https://coil.rs"
  coil:with="page_title='Search Operations'"
  lang="en-GB"
  coil:attr="lang=${locale}"
>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${page_title}">Search Operations</title>
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
        <p class="admin-page__eyebrow">Operations</p>
        <h1 coil:text="${page.title}">Search Operations</h1>
        <p coil:text="${page.summary}">
          Search index freshness, rebuild pressure, and projection drift across the operator-visible catalog.
        </p>
        <p>
          Use this surface before a bulk reindex. The goal is to prove whether browse drift exists,
          not to rebuild search blindly.
        </p>
      </section>

      <section class="admin-panel">
        <p class="admin-panel__eyebrow">Search health</p>
        <div class="admin-card-grid">
          <article class="admin-card">
            <h2>Total indexes</h2>
            <p><strong coil:text="${search_stats.total}">0</strong> operator-visible search indexes are in scope.</p>
          </article>
          <article class="admin-card">
            <h2>Healthy</h2>
            <p><strong coil:text="${search_stats.healthy}">0</strong> indexes currently look ready.</p>
          </article>
          <article class="admin-card">
            <h2>Watching</h2>
            <p><strong coil:text="${search_stats.watching}">0</strong> indexes are waiting on publication or rebuild follow-up.</p>
          </article>
        </div>
      </section>

      <section class="admin-panel" coil:if="${has_search_rows}" data-admin-filter="">
        <p class="admin-panel__eyebrow">Index detail</p>
        <label class="admin-filter">
          <span>Filter by index or state</span>
          <input type="search" placeholder="Filter by index, freshness, or action..." data-admin-filter-input="" />
        </label>
        <table class="admin-table">
          <thead>
            <tr>
              <th scope="col">Index</th>
              <th scope="col">Freshness</th>
              <th scope="col">Drift summary</th>
              <th scope="col">Trigger</th>
              <th scope="col">Operator note</th>
            </tr>
          </thead>
          <tbody>
            <tr coil:each="row : ${search_rows}" data-admin-filter-item="">
              <td><strong coil:text="${row.index_name}">catalog.products</strong></td>
              <td coil:text="${row.freshness_label}">Fresh</td>
              <td coil:text="${row.drift_summary}">0 products need rebuild review.</td>
              <td coil:text="${row.trigger_summary}">Rebuilt by publication events.</td>
              <td coil:text="${row.action_label}">Rebuild only if browse drift is confirmed.</td>
            </tr>
          </tbody>
        </table>
      </section>
    </main>
    <footer class="site-footer">
      <small>Shoppr</small>
    </footer>
  </body>
</html>
```

What this file does:

- keeps recovery and reindex guidance in the shared operator shell
- explains search freshness and drift in operator language
- gives the platform room to own the actual reindex action later without changing the shell

What you edit:

- the copy around freshness, drift, and action labels
- any extra columns or route-level hints your operators need

What the platform owns:

- the search status model
- any rebuild or recovery endpoint behind this page
- the data that determines whether a bulk reindex is safe or necessary

## `apps/shoppr/templates/ops/reports.html`

This page is the checked-in export seam. It keeps report exports inside the shared shell and makes
it clear that exports are operator-triggered work, not background noise.

```html title="apps/shoppr/templates/ops/reports.html"
<!doctype html>
<html
  xmlns:coil="https://coil.rs"
  coil:with="page_title='Reports'"
  lang="en-GB"
  coil:attr="lang=${locale}"
>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${page_title}">Reports</title>
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
        <p class="admin-page__eyebrow">Operations</p>
        <h1 coil:text="${page.title}">Reports</h1>
        <p coil:text="${page.summary}">
          Operational report exports, delivery state, and queued follow-up work for system operators.
        </p>
        <p>
          This surface keeps report exports tied to actual operator needs. Queue them when support,
          audit, or launch workflows need an artifact, not as background noise.
        </p>
      </section>

      <section class="admin-panel">
        <p class="admin-panel__eyebrow">Export summary</p>
        <div class="admin-card-grid">
          <article class="admin-card">
            <h2>Total reports</h2>
            <p><strong coil:text="${report_stats.total}">0</strong> report definitions are currently surfaced to operators.</p>
          </article>
          <article class="admin-card">
            <h2>Ready to export</h2>
            <p><strong coil:text="${report_stats.ready}">0</strong> definitions are ready for operator-triggered export.</p>
          </article>
          <article class="admin-card">
            <h2>Queued follow-up</h2>
            <p><strong coil:text="${report_stats.queued}">0</strong> definitions have active operational reasons to export now.</p>
          </article>
        </div>
      </section>

      <section class="admin-panel" coil:if="${has_report_rows}" data-admin-filter="">
        <p class="admin-panel__eyebrow">Report definitions</p>
        <label class="admin-filter">
          <span>Filter by report, delivery, or state</span>
          <input type="search" placeholder="Filter by report, delivery, or operator note..." data-admin-filter-input="" />
        </label>
        <table class="admin-table">
          <thead>
            <tr>
              <th scope="col">Report</th>
              <th scope="col">Delivery</th>
              <th scope="col">Job state</th>
              <th scope="col">Scope</th>
              <th scope="col">Operator note</th>
            </tr>
          </thead>
          <tbody>
            <tr coil:each="row : ${report_rows}" data-admin-filter-item="">
              <td><strong coil:text="${row.report_name}">Search health</strong></td>
              <td coil:text="${row.delivery_mode}">Signed URL</td>
              <td coil:text="${row.job_state_label}">Ready to export</td>
              <td>
                <p coil:text="${row.scope_summary}">Use this before launch or migration cutover.</p>
                <p coil:text="${row.output_summary}">The ops module stores generated report artifacts behind signed delivery.</p>
              </td>
              <td coil:text="${row.action_label}">Queue export when operators need an external handoff.</td>
            </tr>
          </tbody>
        </table>
      </section>
    </main>
    <footer class="site-footer">
      <small>Shoppr</small>
    </footer>
  </body>
</html>
```

What this file does:

- keeps export work in the same operator shell as payments, events, and audit work
- explains delivery mode and follow-up in one place
- leaves room for bulk export actions without moving operators into another app

What you edit:

- report labels, scope summaries, and operator notes
- any page-level guidance around when an export should be queued

What the platform owns:

- report generation
- signed delivery
- queued export execution and recovery

## `apps/shoppr/templates/cms/pages.html`

This is the real Shoppr CMS editor surface. Again, the first thing to notice is the distinct
bundle load in the `<head>`.

```html title="apps/shoppr/templates/cms/pages.html"
<!doctype html>
<html
  xmlns:coil="https://coil.rs"
  coil:with="page_title='Pages'"
  lang="en-GB"
  coil:attr="lang=${locale}"
>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${page_title}">Pages</title>
    <link rel="stylesheet" href="/theme/assets/cms-editor.css" coil:href="asset('theme/assets/cms-editor.css')" />
    <script src="/theme/assets/cms-editor.js" coil:src="asset('theme/assets/cms-editor.js')" defer="defer"></script>
  </head>
  <body class="harbor harbor--admin">
    <header class="site-header">
      <a href="/" class="brand">Shoppr</a>
      <nav coil:replace="~{admin/nav}"></nav>
    </header>
    <main class="site-main admin-page">
      <section class="admin-page__intro">
        <p class="admin-page__eyebrow">Content operations</p>
        <h1 coil:text="${page.title}">Pages</h1>
        <p coil:text="${page.summary}">Live page inventory and publication state.</p>
      </section>

      <section class="admin-panel" coil:if="${has_content_pages}" data-admin-filter="">
        <p class="admin-panel__eyebrow">Managed pages</p>
        <label class="admin-filter">
          <span>Filter pages</span>
          <input
            type="search"
            placeholder="Filter by title, route, summary, or status..."
            data-admin-filter-input=""
          />
        </label>
      </section>
    </main>
  </body>
</html>
```

What this file proves:

- CMS editor pages load `cms-editor.css` and `cms-editor.js`
- editor-specific Stimulus behavior stays off the public storefront
- the shared operator shell is still where bulk, export, and recovery lanes belong; the CMS editor
  bundle is only for editorial interactions

## `apps/shoppr/templates/events/admin/*`

Event operations and ops surfaces are part of the admin-resource story because they reuse the same shell and the
same admin bundle.

The checked-in event operator pages are:

- `apps/shoppr/templates/events/admin/index.html`
- `apps/shoppr/templates/events/admin/bookings.html`
- `apps/shoppr/templates/events/admin/check-in.html`

They all follow the same pattern:

- load `theme/assets/admin.css`
- load `theme/assets/admin.js`
- render `~{admin/nav}`
- keep the page body on the operator/admin surface rather than the public storefront surface

This means the event operator pages do not need a separate frontend build entry yet. They are
ordinary admin-resource pages that happen to serve event operations.

## Rebuild The Admin And Editor Bundles

Use the real Shoppr frontend and runtime commands:

```bash
cd apps/shoppr
npm run build
./scripts/prepare-local-dev.sh
COIL_COOKIE_SECRET=01234567012345670123456701234567 \
COIL_CSRF_SECRET=76543210765432107654321076543210 \
cargo run -p shoppr -- up --config platform.dev.toml
```

Those commands do three things:

- rebuild the admin and editor bundles
- prepare the in-repo Shoppr workspace for local runs
- boot the real Shoppr runtime with the checked-in admin and CMS routes

The build step compiles:

- `theme/frontend/admin.ts` -> `theme/assets/admin.js`
- `theme/frontend/admin.css` -> `theme/assets/admin.css`
- `theme/frontend/cms-editor.ts` -> `theme/assets/cms-editor.js`
- `theme/frontend/cms-editor.css` -> `theme/assets/cms-editor.css`

## Runnable Checkpoint

Run:

```bash
cd apps/shoppr
npm run build
./scripts/prepare-local-dev.sh
cargo run -p shoppr -- validate
COIL_COOKIE_SECRET=01234567012345670123456701234567 \
COIL_CSRF_SECRET=76543210765432107654321076543210 \
cargo run -p shoppr -- up --config platform.dev.toml
```

Verify:

- `/admin` loads the admin bundle
- `/admin/pages` loads the CMS editor bundle
- `/admin/events`, `/admin/events/bookings`, and `/admin/events/check-in` all load the admin bundle
- public storefront pages still load only `site.css` and `site.js`
- admin filter controls and CMS block controls attach through their own bundles
