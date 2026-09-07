---
title: Add Memberships and Audience Gating
---

This chapter stops talking about memberships as an abstract future capability and uses the checked-in
Shoppr app as it actually works today.

The important seam is:

- account pages use `account.*` and `membership_summary.*`
- public CMS pages use `cms_page.*`, `audience.*`, and the runtime booleans
  `show_membership_gated_content`, `show_membership_pending_gate`, and
  `show_membership_teaser_gate`

That split matters because memberships affect two different parts of the product:

- the signed-in customer account journey
- public editorial pages that should only reveal full content to eligible customers

This chapter shows both.

## Purpose

By the end of this chapter, you should understand:

- where active, pending, and missing membership state comes from
- how the account area renders that state
- how a CMS page becomes membership-gated
- where the runtime decides whether to show the real page, a pending-payment message, or a teaser

The checked-in Shoppr app already contains the right surfaces. The gap was that the tutorial was
teaching a different model.

## Account Navigation

This file is the small but important bridge between the general account hub and the detailed
memberships surface. It puts memberships into the signed-in customer journey instead of treating
them as a one-off page.

`apps/shoppr/templates/account/nav.html`

```html
<nav class="account-nav" xmlns:coil="https://coil.rs" coil:fragment="nav">
  <a class="account-nav__link" href="/account" coil:attr="href=${links.account}">
    Account overview
  </a>
  <a class="account-nav__link" href="/account/orders" coil:attr="href=${links.orders}">
    Order history
  </a>
  <a
    class="account-nav__link"
    href="/account/memberships"
    coil:attr="href=${links.memberships}"
  >
    Memberships
  </a>
  <button class="account-nav__link" type="submit" form="coil-account-session-end">
    End browser session
  </button>
</nav>
```

What this file does:

- `links.account` keeps the overview route locale- and site-aware
- `links.orders` keeps order history in the same account flow
- `links.memberships` gives membership state its own first-class account surface
- the session-end button makes it explicit that Shoppr can also project account state from a live
  browser session, not only from a traditional sign-in flow

## Account Overview

This page is the high-level customer account hub. It shows how membership state already participates
in the same product loop as orders and storefront return journeys.

`apps/shoppr/templates/pages/account.html`

```html
<!doctype html>
<html
  xmlns:coil="https://coil.rs"
  coil:with="page_title='Account'"
  lang="en-GB"
  coil:attr="lang=${locale}"
>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${page_title}">Account</title>
    <link rel="stylesheet" href="/theme/assets/site.css" coil:href="asset('theme/assets/site.css')" />
  </head>
  <body class="harbor">
    <header class="site-header">
      <a href="/" class="brand">Shoppr</a>
      <nav coil:replace="~{navigation/primary}"></nav>
    </header>
    <main class="site-main">
      <section class="account-page">
        <section class="storefront-flash" coil:if="${has_flash_messages}">
          <article class="storefront-flash__message" coil:each="message : ${flash_messages}">
            <p coil:text="${message.text}">Account session updated.</p>
          </article>
        </section>
        <div class="account-page__intro">
          <p class="account-page__eyebrow">Customer account</p>
          <h1 coil:text="${customer.display_name}">Welcome back to Shoppr.</h1>
          <p coil:text="${account.state_summary}">
            Use this account hub for orders, membership access, and repeat-purchase shortcuts that
            point straight back into the storefront.
          </p>
          <p coil:if="${account.has_principal}">
            Signed in as <strong coil:text="${customer.display_name}">Member Live</strong>.
          </p>
          <p coil:unless="${account.has_principal}">
            This account currently follows the browser session you are using right now. Keep this
            browser active if you want the same order history and membership state to remain
            visible.
          </p>
          <p coil:if="${account.has_latest_order}">
            Latest order
            <strong coil:text="${account.latest_order_reference}">ORD-10042</strong>
            is
            <span coil:text="${account.latest_order_status}">Paid</span>.
          </p>
          <p coil:if="${account.has_latest_order}">
            If you have just returned from the payment provider, this account can keep that order
            in
            <strong>Pending Payment</strong>
            until the provider callback settles for this same browser session.
          </p>
          <div coil:if="${account.has_customer_email}">
            <p coil:if="${account.has_principal}">
              Account email: <strong coil:text="${customer.email}">member@example.com</strong>.
            </p>
            <p coil:unless="${account.has_principal}">
              Latest receipt email for this browser session:
              <strong coil:text="${customer.email}">member@example.com</strong>.
            </p>
          </div>
        </div>

        <nav class="account-page__nav" coil:replace="~{account/nav :: nav}"></nav>

        <div class="account-page__cards">
          <article class="account-card">
            <h2>Memberships</h2>
            <p coil:if="${account.has_membership}">
              <strong coil:text="${membership_summary.tier_name}">Harbor Circle</strong>
              <span coil:text="${membership_summary.status}">Active</span>
            </p>
            <p coil:if="${account.has_membership}" coil:text="${membership_summary.renewal_text}">
              Renewing on 18 April with market-day priority access.
            </p>
            <p coil:unless="${account.has_membership}" coil:text="${account.membership_empty_text}">
              No active membership is attached yet. Start from the storefront collection to join.
            </p>
            <a class="button" href="/account/memberships">View memberships</a>
          </article>
          <article class="account-card">
            <h2>Orders and storefront</h2>
            <p coil:if="${account.has_recent_orders}">
              Review payment status, receipt details, and post-checkout next steps before heading
              back into the storefront. Pending Payment after a provider return means settlement is
              still in flight.
            </p>
            <p coil:unless="${account.has_recent_orders}" coil:text="${account.orders_empty_text}">
              Continue browsing the public catalog and landing content.
            </p>
            <a class="button" href="/account/orders">View order history</a>
            <a
              class="button button--secondary"
              href="/en-GB/shop"
              coil:attr="href=${account.orders_cta_url}"
              coil:text="${account.orders_cta_label}"
            >
              Browse storefront
            </a>
          </article>
        </div>

        <div coil:replace="~{account/summary-panels :: panels}"></div>
      </section>
    </main>
    <footer class="site-footer">
      <small>Shoppr</small>
    </footer>
  </body>
</html>
```

What this page teaches:

- `account.has_membership` decides whether the membership card shows live entitlement state or the
  empty-state copy
- `membership_summary.tier_name`, `membership_summary.status`, and
  `membership_summary.renewal_text` are the account-facing membership projection
- `account.has_latest_order` and `account.latest_order_status` matter because a qualifying
  membership purchase can still be pending after the customer returns from the payment provider
- `account.membership_empty_text` and `account.orders_cta_url` are not hard-coded page decisions;
  they come from runtime shaping

This page is the overview. The detailed membership state lives in the dedicated memberships page.

## Membership Account Surface

This file is the clearest expression of the three membership states Shoppr currently supports in the
customer account:

- active membership
- pending activation after checkout
- no membership yet

`apps/shoppr/templates/memberships/account.html`

```html
<!doctype html>
<html
  xmlns:coil="https://coil.rs"
  coil:with="page_title='Memberships'"
  lang="en-GB"
  coil:attr="lang=${locale}"
>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${page_title}">Memberships</title>
    <link rel="stylesheet" href="/theme/assets/site.css" coil:href="asset('theme/assets/site.css')" />
  </head>
  <body class="harbor">
    <header class="site-header">
      <a href="/" class="brand">Shoppr</a>
      <nav coil:replace="~{navigation/primary}"></nav>
    </header>
    <main class="site-main memberships-page">
      <section class="memberships-page__intro">
        <p class="memberships-page__eyebrow">Account access</p>
        <h1>Memberships</h1>
        <p coil:text="${account.state_summary}">
          This page gives customers a direct answer to what they have, when it renews, and what
          support or priority access comes with it.
        </p>
        <p coil:if="${account.has_principal}">
          Signed in as <strong coil:text="${customer.display_name}">Member Live</strong>.
        </p>
        <p coil:unless="${account.has_principal}">
          This membership view currently follows the browser session you are using right now.
        </p>
        <p coil:if="${account.has_latest_order}">
          Latest order
          <strong coil:text="${account.latest_order_reference}">ORD-10042</strong>
          is
          <span coil:text="${account.latest_order_status}">Paid</span>.
        </p>
        <p coil:if="${account.has_latest_order}">
          After returning from the payment provider, this page can keep that order in
          <strong>Pending Payment</strong>
          until settlement updates the same browser session.
        </p>
        <div coil:if="${account.has_customer_email}">
          <p coil:if="${account.has_principal}">
            Signed in as <strong coil:text="${customer.email}">member@example.com</strong>.
          </p>
          <p coil:unless="${account.has_principal}">
            Receipt email on this browser session:
            <strong coil:text="${customer.email}">member@example.com</strong>.
          </p>
        </div>
      </section>
      <nav class="account-page__nav" coil:replace="~{account/nav :: nav}"></nav>

      <section class="memberships-page__summary" coil:if="${account.has_membership}">
        <article class="membership-card">
          <p class="membership-card__eyebrow">Current tier</p>
          <h2 coil:text="${membership_summary.tier_name}">Harbor Circle</h2>
          <p coil:text="${membership_summary.status}">Active</p>
          <p coil:text="${membership_summary.renewal_text}">
            Renewing on 18 April with early-access reservations.
          </p>
          <a class="button" href="/account/orders">View order history</a>
        </article>
      </section>

      <section class="memberships-page__summary" coil:if="${account.has_pending_membership_order}">
        <article class="membership-card">
          <p class="membership-card__eyebrow">Current tier</p>
          <h2>Pending activation</h2>
          <p>
            Your latest order
            <strong coil:text="${account.latest_order_reference}">ORD-10042</strong>
            is
            <span coil:text="${account.latest_order_status}">Pending Payment</span>.
            Membership access only appears here after a qualifying membership purchase is captured
            for this account view. After a Stripe return, check order history for the latest
            payment state, then return here once this browser session shows the order as captured.
          </p>
          <a class="button button--secondary" href="/account/orders">
            View order history
          </a>
        </article>
      </section>

      <section class="memberships-page__summary" coil:if="${account.needs_membership_purchase}">
        <article class="membership-card">
          <p class="membership-card__eyebrow">Current tier</p>
          <h2>Membership not active yet</h2>
          <p coil:text="${account.membership_empty_text}">
            Join to unlock early access, subscriber-only bundles, and concierge support.
          </p>
          <a
            class="button"
            href="/en-GB/shop/collections/memberships"
            coil:attr="href=${account.membership_cta_url}"
          >
            Explore memberships
          </a>
        </article>
      </section>

      <section class="memberships-page__summary" coil:if="${account.has_latest_order}">
        <article class="membership-card">
          <p class="membership-card__eyebrow">Latest order</p>
          <h2 coil:text="${account.latest_order_reference}">ORD-10042</h2>
          <p>
            Status
            <strong coil:text="${account.latest_order_status}">Paid</strong>
          </p>
          <p coil:text="${account.state_summary}">
            Order and membership state are drawn from the live storefront session.
          </p>
          <a class="button button--secondary" href="/account/orders">Review order history</a>
        </article>
      </section>

      <div coil:replace="~{account/summary-panels :: panels}"></div>
    </main>
    <footer class="site-footer">
      <small>Shoppr</small>
    </footer>
  </body>
</html>
```

What this page does:

- `account.has_membership` shows the active state
- `account.has_pending_membership_order` shows the pending-payment state
- `account.needs_membership_purchase` shows the upgrade or acquisition state
- `account.membership_cta_url` points back to the memberships collection in the storefront
- `membership_summary.*` stays focused on the active entitlement rather than trying to represent all
  states at once

This page is where the customer gets a precise answer to the question “Do I actually have access
yet?”

## Public Membership-Gated CMS Pages

Account state is only half of the story. Shoppr also needs public editorial pages whose full content
is only available to members.

This is the checked-in public CMS page template after wiring it to the real runtime gate booleans:

`apps/shoppr/templates/cms/page.html`

```html
<!doctype html>
<html
  xmlns:coil="https://coil.rs"
  lang="en-GB"
  coil:attr="lang=${locale}"
>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${page.title}">Shoppr</title>
    <link rel="stylesheet" href="/theme/assets/site.css" coil:href="asset('theme/assets/site.css')" />
  </head>
  <body class="harbor">
    <header class="site-header">
      <a href="/" class="brand">Shoppr</a>
      <nav coil:replace="~{navigation/primary}"></nav>
    </header>
    <main class="site-main">
      <article class="page-shell">
        <p
          class="catalog-section__eyebrow"
          coil:text="${cms_page.requires_membership ? 'Members guide' : 'CMS Page'}"
        >
          CMS Page
        </p>
        <h1 coil:text="${cms_page.content.title}">Shoppr</h1>
        <p coil:text="${cms_page.content.summary}">
          Server-rendered CMS page.
        </p>
        <section class="page-shell" coil:if="${cms_page.requires_membership and show_membership_gated_content}">
          <p class="catalog-section__eyebrow">Membership access</p>
          <h2>Member access unlocked</h2>
          <p>
            Current membership state:
            <strong coil:text="${audience.membership_state_label}">Active</strong>
          </p>
          <p coil:text="${audience.membership_summary}">
            This page now uses the active membership entitlement attached to the current request.
          </p>
        </section>
        <section class="page-shell" coil:if="${show_membership_pending_gate}">
          <p class="catalog-section__eyebrow">Membership access</p>
          <h2 coil:text="${audience.membership_title}">Membership activation pending</h2>
          <p coil:text="${audience.membership_summary}">
            Membership access only becomes available after the qualifying order finishes payment
            capture.
          </p>
          <div class="checkout-actions">
            <a class="button" href="/account/orders" coil:attr="href=${links.orders}">
              Review order history
            </a>
            <a
              class="button button--secondary"
              href="/account/memberships"
              coil:attr="href=${links.memberships}"
            >
              View memberships
            </a>
          </div>
        </section>
        <section class="page-shell" coil:if="${show_membership_teaser_gate}">
          <p class="catalog-section__eyebrow">Membership required</p>
          <h2 coil:text="${audience.membership_title}">Membership preview</h2>
          <p coil:text="${audience.membership_summary}">
            Join from the storefront to unlock the full guide and member-only editorial content.
          </p>
          <div class="checkout-actions">
            <a
              class="button"
              href="/en-GB/shop/collections/memberships"
              coil:attr="href=${audience.membership_cta_url}"
              coil:text="${audience.membership_cta_label}"
            >
              Explore memberships
            </a>
            <a class="button button--secondary" href="/account" coil:attr="href=${links.account}">
              Open account
            </a>
          </div>
        </section>
        <div
          class="cms-page-blocks"
          coil:if="${cms_page.has_structured_blocks and (show_membership_gated_content or not cms_page.requires_membership)}"
        >
          <coil:block coil:each="block : ${cms_page.blocks}">
            <coil:block coil:switch="${block.type_id}">
              <div coil:case="'hero'" coil:replace="~{cms/blocks/hero :: block}"></div>
              <div coil:case="'rich_text'" coil:replace="~{cms/blocks/rich_text :: block}"></div>
              <div coil:case="'legacy_html_body'" coil:replace="~{cms/blocks/rich_text :: block}"></div>
              <div coil:case="'callout'" coil:replace="~{cms/blocks/callout :: block}"></div>
              <div coil:case="'editorial_callout'" coil:replace="~{cms/blocks/callout :: block}"></div>
              <section coil:default class="page-shell">
                <p class="catalog-section__eyebrow" coil:text="${block.type_id}">Block</p>
                <h2 coil:if="${block.has_label}" coil:text="${block.label}">Block label</h2>
                <div coil:if="${block.has_html}" coil:utext="${block.html}">
                  <p>Block body.</p>
                </div>
                <dl coil:unless="${block.has_html}">
                  <div coil:each="field : ${block.field_entries}">
                    <dt coil:text="${field.key}">field</dt>
                    <dd coil:text="${field.value}">value</dd>
                  </div>
                </dl>
              </section>
            </coil:block>
          </coil:block>
        </div>
        <div
          coil:if="${(not cms_page.has_structured_blocks) and (show_membership_gated_content or not cms_page.requires_membership)}"
          coil:utext="${cms_page.body_html}"
        >
          <p>Page body.</p>
        </div>
        <div class="checkout-actions">
          <a class="button" href="/" coil:attr="href=${links.home}">Return home</a>
          <a class="button button--secondary" href="/en-GB/shop" coil:attr="href=${links.catalog}">
            Open catalog
          </a>
        </div>
      </article>
    </main>
    <footer class="site-footer">
      <small>Shoppr</small>
    </footer>
  </body>
</html>
```

What this file owns:

- it decides whether a public CMS page shows the real content or a gated alternative
- it does not decide who is a member; that comes from the runtime
- it uses `cms_page.requires_membership` to know whether this page participates in the gating flow
- it uses:
  - `show_membership_gated_content`
  - `show_membership_pending_gate`
  - `show_membership_teaser_gate`
  to choose which branch to render
- it uses `audience.*` for the customer-facing explanation and CTAs

That is the right split. Templates stay declarative. Runtime code owns the entitlement decision.

## Runtime Gating for Public CMS Pages

The public page route already computes the gate booleans before rendering the template.

Relevant excerpt from `crates/coil-runtime/src/render/model.rs`:

```rust
"cms.page" => {
    let workspace = plan
        .map(cms_admin_workspace)
        .transpose()?
        .unwrap_or_else(default_cms_admin_workspace);
    let slug = params.get("slug").map(String::as_str).unwrap_or_default();
    let requires_membership = workspace
        .live_page_by_slug(slug)
        .and_then(|page| page.live.as_ref())
        .is_some_and(|revision| revision.settings.page_type == "membership_guide");
    model = model
        .with_object("audience", audience.audience.clone())?
        .with_bool(
            "show_membership_gated_content",
            !requires_membership || audience.has_membership,
        )?
        .with_bool(
            "show_membership_pending_gate",
            requires_membership && audience.has_pending_membership_order,
        )?
        .with_bool(
            "show_membership_teaser_gate",
            requires_membership && audience.needs_membership_purchase,
        )?
        .with_object("cms_page", cms_live_page_model(&workspace, slug)?)?;
}
```

What this excerpt means:

- the current checked-in membership-gated convention is `page_type == "membership_guide"`
- `audience.has_membership` reveals the page
- `audience.has_pending_membership_order` shows the pending-payment gate
- `audience.needs_membership_purchase` shows the teaser or upgrade gate

The template does not infer this logic. The runtime hands it the answer directly.

## Runtime Membership Projection

The runtime also shapes the audience summary that the public CMS page uses.

Relevant excerpt from `crates/coil-runtime/src/render/model.rs`:

```rust
Ok(AudienceSurfaceBindings {
    audience: RenderModel::new()
        .with_bool("has_membership", has_membership)?
        .with_bool("has_pending_membership_order", has_pending_membership_order)?
        .with_bool("needs_membership_purchase", needs_membership_purchase)?
        .with_bool("has_membership_tier_name", !membership_tier_name.is_empty())?
        .with_value("membership_tier_name", RenderValue::text(membership_tier_name))?
        .with_value("membership_state", RenderValue::text(membership_state))?
        .with_value(
            "membership_state_label",
            RenderValue::text(membership_state_label),
        )?
        .with_value("membership_title", RenderValue::text(membership_title))?
        .with_value("membership_summary", RenderValue::text(membership_summary))?
        .with_value(
            "membership_cta_label",
            RenderValue::text(membership_cta_label),
        )?
        .with_value("membership_cta_url", RenderValue::text(membership_cta_url))?,
    has_membership,
    has_pending_membership_order,
    needs_membership_purchase,
})
```

This is why the CMS page template can stay simple:

- `audience.membership_state_label` gives the active or pending label
- `audience.membership_title` gives the headline for the gate state
- `audience.membership_summary` gives the explanatory copy
- `audience.membership_cta_label` and `audience.membership_cta_url` give the right primary CTA

The account surfaces use `account.*` and `membership_summary.*`. Public gated CMS pages use
`audience.*`. Those are different contracts for different parts of the product.

## What Changed In The Running App

After wiring the public CMS page template to the real gate booleans, the checked-in membership guide
now has three visible states:

- anonymous or not entitled
  - the page shows a teaser and upgrade path
- pending qualifying membership order
  - the page explains that activation is still waiting for payment capture
- active membership
  - the page reveals the real editorial content

That behavior is now verified by runtime tests instead of being assumed.

## Checkpoint

Run the app and verify both surfaces.

```bash
cargo run -p shoppr-app -- validate
cargo run -p shoppr-app -- serve
```

Then check:

1. `/account`
   You should see the account overview using `account.*` and `membership_summary.*`.
2. `/account/memberships`
   You should see one of:
   - active membership
   - pending activation
   - no membership yet
3. `/en-GB/pages/membership-guide`
   After publishing the checked-in Membership Guide from the CMS admin and setting its page type to
   `membership_guide`, you should see:
   - a teaser gate when there is no qualifying membership
   - a pending-payment gate after a qualifying membership checkout but before capture
   - the full page after membership activation

What this chapter proves:

- membership state already exists as a real platform projection
- public audience gating and account-facing membership views are separate but coherent surfaces
- the right seam is runtime shaping plus declarative templates, not a tutorial-only content-model
  flag

## Next

The next chapter uses this same entitlement-aware foundation to make event visibility and timeslot
surfaces membership-aware where needed.

- [Add Events and Timeslots](../add-events-and-timeslots/)
