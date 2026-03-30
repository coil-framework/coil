---
title: Add Jobs, Notifications, and Scheduled Work
---

This chapter makes the operator side of your product honest.

The previous integration chapter showed request-time payment handoff and webhook reconciliation. This
chapter shows the next step: how Shoppr tells operators that some work is still waiting on a job,
provider callback, or export queue instead of pretending the request is the whole workflow.

## Purpose

Use this chapter to understand three different seams:

- request-time customer behavior
- background or deferred operational work
- operator-facing pages that explain what the system is waiting on

Shoppr already has those seams in the checked-in app. You do not need to invent a fake
`jobs.json` file or a custom scheduler demo. The real files to read are the Shoppr development
runtime config, the payment operations template, the search ops template, the reports template,
the dedicated jobs template, and the shared admin dashboard template.

## Start With The Runtime Config

The checked-in development config already turns on the queue backend you need locally:

```toml
[jobs]
backend = "redis"
```

This block lives in the Shoppr development runtime config file.

Why this file exists:

- `app.toml` describes the product and enabled modules
- `platform.dev.toml` describes how the local runtime actually runs that product
- the `[jobs]` section is where local deferred work infrastructure is selected

What this section does:

- tells the runtime to use Redis-backed job execution in local development
- keeps background work out of the request path
- makes later operator pages truthful when they say a task is queued or waiting on follow-up

Exact next effect:

- when you run the local stack, the runtime has a real queue backend available
- payment follow-up, report-export readiness, and similar operator guidance now points at an
  actual subsystem, not a pretend browser-only state machine

## Read The Payment Operations Page As A Jobs Surface

The clearest checked-in operator workflow is the payment queue in the Shoppr payment operations
template.

The most important part of the file is not the table markup by itself. It is what the table is
teaching the operator:

```html
<section class="admin-panel">
  <p class="admin-panel__eyebrow">Queue summary</p>
  <div class="admin-card-grid">
    <article class="admin-card">
      <h2>Awaiting confirmation</h2>
      <p>
        <strong coil:text="${payment_operation_stats.awaiting_confirmation}">0</strong>
        rows still need signed provider confirmation before the order can move forward.
      </p>
    </article>
    <article class="admin-card">
      <h2>Refund follow-up</h2>
      <p>
        <strong coil:text="${payment_operation_stats.refund_follow_up}">0</strong>
        rows already have refund history and still need reconciliation or customer messaging.
      </p>
    </article>
  </div>
</section>
```

The queue rows make the deferred-work seam explicit:

```html
<td>
  <strong coil:text="${payment.webhook_status_label}">
    Awaiting signed Stripe webhook
  </strong>
  <p coil:text="${payment.integration_note}">
    Payment confirmation is pending. The order will move forward after the provider callback arrives.
  </p>
</td>
<td>
  <p coil:text="${payment.next_job_label}">
    Hold customer messaging until the payment-provider webhook confirms capture.
  </p>
  <p coil:if="${payment.has_refund_history}" coil:text="${payment.refund_history_label}">
    1 refund event recorded
  </p>
</td>
```

What the important sections do:

- `payment_operation_stats.*` gives the operator a queue-level summary
- `payment.webhook_status_label` explains the current provider state
- `payment.integration_note` explains why the row is still waiting
- `payment.next_job_label` tells the operator what downstream work should happen next

Exact next effect:

- the operator can see that payment capture, refund reconciliation, and customer messaging are
  different stages
- the app stops implying that checkout is complete just because the HTTP request returned

## Use Search And Reports As The Other Two Operator Lanes

Shoppr also exposes two ops pages that keep background work inside the shared admin shell: the
search operations template and the reports template.

The search page teaches operators when not to reindex blindly:

```html
<section class="admin-panel">
  <p class="admin-panel__eyebrow">Search health</p>
  <div class="admin-card-grid">
    <article class="admin-card">
      <h2>Watching</h2>
      <p>
        <strong coil:text="${search_stats.watching}">0</strong>
        indexes are waiting on publication or rebuild follow-up.
      </p>
    </article>
  </div>
</section>
```

```html
<td coil:text="${row.trigger_summary}">Rebuilt by publication events.</td>
<td coil:text="${row.action_label}">
  Rebuild only if browse drift is confirmed.
</td>
```

The reports page teaches operators that exports are deliberate queued work, not a hidden script:

```html
<section class="admin-panel">
  <p class="admin-panel__eyebrow">Export summary</p>
  <div class="admin-card-grid">
    <article class="admin-card">
      <h2>Queued follow-up</h2>
      <p>
        <strong coil:text="${report_stats.queued}">0</strong>
        definitions have active operational reasons to export now.
      </p>
    </article>
  </div>
</section>
```

```html
<td coil:text="${row.job_state_label}">Ready to export</td>
<td coil:text="${row.action_label}">
  Queue export when operators need an external handoff.
</td>
```

What these files are doing:

- `search.html` exposes reindex and drift-review decisions as operator work
- `reports.html` exposes export readiness and delivery mode as operator work
- both pages keep background operations inside the same admin shell as orders, content, memberships,
  and events

Exact next effect:

- operators have one place to reason about payment follow-up, reindex pressure, and report exports
- jobs and scheduled work stop feeling like hidden infrastructure and become visible product behavior

## Keep The Operator Entry Point Honest

The shared admin dashboard template is the page that ties those workflows together.

This is the part that matters:

```html
<article class="admin-card" data-admin-filter-item="">
  <h2>Monitor payment handoff</h2>
  <a class="button" href="/admin/payments" coil:attr="href=${links.admin_payments}">
    Open payments
  </a>
</article>
<article class="admin-card" data-admin-filter-item="">
  <h2>Inspect search and reports</h2>
  <div class="admin-copy-row">
    <a class="button" href="/admin/search" coil:attr="href=${links.admin_search}">
      Open search
    </a>
    <a class="button button--secondary" href="/admin/reports" coil:attr="href=${links.admin_reports}">
      Open reports
    </a>
  </div>
</article>
```

Why this file exists:

- it is the operator control room
- it tells the reader which pages are part of the daily workflow
- it keeps ops pages discoverable from the same shell as commerce and CMS work

Exact next effect:

- a new operator does not need to know internal route names first
- payment follow-up, search review, and report export appear as normal admin tasks

## Read The Dedicated Jobs Page

Shoppr also has a dedicated jobs page in the shared admin shell.

`apps/shoppr/templates/admin/jobs.html`

```html
<section class="admin-panel">
  <p class="admin-panel__eyebrow">Queue summary</p>
  <div class="admin-card-grid">
    <article class="admin-card">
      <h2>Total jobs</h2>
      <p><strong coil:text="${job_stats.total}">0</strong> runtime jobs are registered.</p>
    </article>
    <article class="admin-card">
      <h2>Event reactions</h2>
      <p><strong coil:text="${job_stats.event_reactions}">0</strong> domain-event subscriptions currently route follow-up work into the jobs runtime.</p>
    </article>
  </div>
</section>
```

What this file does:

- shows the jobs runtime as operator-visible product state
- distinguishes direct operator jobs from event-driven follow-up work
- keeps queue and trigger visibility inside the shared admin shell

## What Coil Does And Does Not Do Automatically

Coil gives you:

- the runtime job backend
- module-level job contracts
- operator routes that can explain queued work
- render-model data that can surface queue state in templates

Coil does not automatically decide:

- when your product should send customer-facing follow-up messages
- when a reindex is actually warranted
- when a report export should happen for support, audit, or launch work
- what operator guidance text belongs on those pages

That last step is customer-app work. Shoppr does it in the templates you just read.

## Runnable Checkpoint

Run the focused server tests that already exercise these operator surfaces:

```bash
cargo test -p coil-runtime server_host_renders_checked_in_harbor_shop_payment_operations_surface -- --nocapture
cargo test -p coil-runtime server_host_supports_checked_in_harbor_shop_order_detail_and_refund_flow -- --nocapture
cargo test -p coil-runtime server_host_renders_checked_in_harbor_shop_admin_surfaces -- --nocapture
```

What each test proves:

- `server_host_renders_checked_in_harbor_shop_payment_operations_surface`
  proves the payment queue renders provider state and follow-up guidance
- `server_host_supports_checked_in_harbor_shop_order_detail_and_refund_flow`
  proves refund-related operator follow-up is part of the real order workflow
- `server_host_renders_checked_in_harbor_shop_admin_surfaces`
  proves the shared admin shell exposes the Shoppr operator routes

Exact next effect:

- after this chapter, your product has a real operator story for deferred work
- the next chapter can explain observability on top of those real admin and queue surfaces instead
  of introducing generic monitoring concepts in isolation
