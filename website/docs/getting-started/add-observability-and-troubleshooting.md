---
title: Add Observability and Troubleshooting
---

This chapter turns the checked-in Shoppr operator shell into something you can actually debug.

It is not a generic monitoring chapter. It is a concrete walkthrough of the real files that expose
health, readiness, metrics, and operator diagnostics in the current app.

## Purpose

Use this chapter to answer four practical questions:

- is the runtime alive?
- is it ready to serve traffic?
- are metrics and trace capture enabled?
- where should an operator look before dropping into raw logs or provider dashboards?

The real files for this chapter are:

- `apps/shoppr/platform.dev.toml`
- `apps/shoppr/templates/admin/diagnostics.html`
- `apps/shoppr/templates/admin/dashboard.html`
- `crates/coil-runtime/src/tests/server.rs`

## Start With The Local Runtime Config

The checked-in development config already enables the observability features this chapter needs:

```toml
[observability]
metrics = true
tracing = true
```

This block lives in `apps/shoppr/platform.dev.toml`.

Why this file exists:

- `app.toml` describes the app and module structure
- `platform.dev.toml` describes how the local runtime behaves
- `[observability]` is where you turn operator-visible diagnostics on for local work

What this section does:

- enables the metrics surface
- enables trace capture in the runtime
- gives the admin diagnostics page enough real state to summarize

Exact next effect:

- `/metrics` becomes meaningful
- the diagnostics page can report whether metrics and tracing are on instead of hardcoding a guess

## Read The Diagnostics Template As The Operator Contract

The main file for this chapter is
`apps/shoppr/templates/admin/diagnostics.html`.

The first section teaches operators what kind of state they should trust:

```html
<section class="admin-metrics">
  <article class="admin-metric">
    <p class="admin-metric__label">Liveness</p>
    <strong coil:text="${diagnostics.health.status}">ok</strong>
  </article>
  <article class="admin-metric">
    <p class="admin-metric__label">Readiness</p>
    <strong coil:text="${diagnostics.readiness.status}">ready</strong>
  </article>
  <article class="admin-metric">
    <p class="admin-metric__label">Metrics enabled</p>
    <strong coil:text="${diagnostics.metrics_enabled_label}">yes</strong>
  </article>
  <article class="admin-metric">
    <p class="admin-metric__label">Trace capture</p>
    <strong coil:text="${diagnostics.tracing_enabled_label}">yes</strong>
  </article>
</section>
```

What these fields mean:

- `diagnostics.health.status` is basic process liveness
- `diagnostics.readiness.status` is the traffic-readiness answer
- `diagnostics.metrics_enabled_label` tells the operator whether the metrics endpoint is expected to work
- `diagnostics.tracing_enabled_label` tells the operator whether trace summaries should appear

Exact next effect:

- one operator page answers the “is the app up?” and “is instrumentation on?” questions without
  requiring shell access

## Keep The Raw Endpoints Reachable From The Admin Surface

The diagnostics template also exposes the raw endpoints directly:

```html
<div class="admin-copy-row">
  <a class="button button--secondary" href="/health">Open /health</a>
  <a class="button button--secondary" href="/ready">Open /ready</a>
  <a class="button button--secondary" href="/metrics">Open /metrics</a>
  <a class="button button--secondary" href="/diagnostics">Open /diagnostics</a>
</div>
```

Why this section exists:

- operators often need both the summary page and the raw payload
- the admin page gives them the safe entry point
- the raw endpoints stay inspectable without inventing a second debug tool

The important boundary is that these endpoints are not all equally public.

The server test already proves the intended split in
`crates/coil-runtime/src/tests/server.rs`:

```rust
async fn server_router_keeps_public_probes_open_and_diagnostics_privileged()
```

Exact next effect:

- `/health`, `/ready`, and `/metrics` stay usable as probe targets
- `/diagnostics` remains an operator-only diagnostics surface

## Show Operators What To Inspect Next

The template does more than show probe status. It tells the operator where to look next.

For metrics:

```html
<table class="admin-table">
  <tbody>
    <tr coil:each="metric : ${diagnostics.metrics}">
      <td><strong coil:text="${metric.name}">http.requests</strong></td>
      <td coil:text="${metric.kind}">counter</td>
      <td coil:text="${metric.description}">Total incoming HTTP requests</td>
    </tr>
  </tbody>
</table>
```

For recent traces:

```html
<tr coil:each="trace : ${diagnostics.traces}">
  <td><code coil:text="${trace.trace_id}">trace-1</code></td>
  <td coil:text="${trace.span}">request</td>
  <td coil:text="${trace.outcome}">ok</td>
  <td coil:text="${trace.duration_ms}">12</td>
</tr>
```

What these sections do:

- the metrics table tells the operator which signals the runtime knows about
- the traces table gives a fast recent-history view before you reach for deeper logs or diagnostics

Exact next effect:

- a new developer can move from “the app looks broken” to “the readiness probe failed” or “trace
  capture is off” without guessing which subsystem to inspect first

## Wire Diagnostics Into The Shared Admin Shell

The diagnostics page is not meant to stand alone. The dashboard already routes operators to it:

```html
<article class="admin-card" data-admin-filter-item="">
  <h2>Open diagnostics</h2>
  <p>
    Review readiness, metrics, and recent traces before dropping into raw health or
    diagnostics endpoints.
  </p>
  <a class="button" href="/admin/diagnostics" coil:attr="href=${links.admin_diagnostics}">
    Open diagnostics
  </a>
</article>
```

This lives in `apps/shoppr/templates/admin/dashboard.html`.

Why this file matters:

- operators should reach diagnostics from the same shell as orders, content, events, and payments
- observability is part of the product’s operating model, not an afterthought hidden in a runbook

Exact next effect:

- your admin dashboard becomes the first stop for troubleshooting
- probe links and diagnostics summaries live next to real operator workflows

## What Coil Does And Does Not Do Automatically

Coil gives you:

- health and readiness endpoints
- metrics and trace toggles in platform config
- a render-model path for diagnostics pages
- tests proving public probes and privileged diagnostics behavior

Coil does not automatically decide:

- which failures matter most to your operators
- how to explain payment, search, or queue failures in your own admin shell
- what troubleshooting drill your team should follow first

That operator explanation is customer-app work. Shoppr does it in the diagnostics template and the
dashboard card that links to it.

## Runnable Checkpoint

Run the targeted server tests that already cover the probe split and admin shell:

```bash
cargo test -p coil-runtime server_router_keeps_public_probes_open_and_diagnostics_privileged -- --nocapture
cargo test -p coil-runtime server_host_renders_checked_in_harbor_shop_admin_surfaces -- --nocapture
```

Then run the app locally and verify these routes manually:

```bash
cargo run --manifest-path apps/shoppr/Cargo.toml -p shoppr -- --config platform.dev.toml up
```

Check:

- `/health`
- `/ready`
- `/metrics`
- `/admin/diagnostics`

Exact next effect:

- after this chapter, your tutorial app has a real troubleshooting entry point
- the production-prep chapter can now talk about deployment and startup behavior using the same
  concrete files instead of generic operations advice
