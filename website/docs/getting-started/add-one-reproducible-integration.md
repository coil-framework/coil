---
title: Add One Reproducible Integration
---

# Add One Reproducible Integration

## Purpose

This chapter wires the tutorial app to one real external provider flow. In Shoppr, that flow is
Stripe Checkout plus the signed webhook that settles payment after the browser returns.

You will use the same files the checked-in app already uses:

- `apps/shoppr/platform.dev.toml` owns provider configuration and secrets
- `apps/shoppr/templates/commerce/checkout.html` owns the customer handoff page
- `apps/shoppr/templates/commerce/checkout-confirmation.html` owns the return screen
- `apps/shoppr/templates/account/orders.html` owns customer-visible order history
- `apps/shoppr/templates/commerce/orders.html` owns the operator order queue
- `apps/shoppr/templates/admin/integrations.html` owns the operator-visible integration inventory

The point of this chapter is not to invent a fake payment flow. It is to show the exact boundary:
the platform owns the Stripe module and webhook verification, while the customer app owns the
messages, the order support surfaces, and the local developer commands.

## Configure Stripe In Development

Start with the real Shoppr development config.

`apps/shoppr/platform.dev.toml`

```toml
[modules]
enabled = ["cms", "media", "commerce", "commerce-payments-stripe", "memberships", "events", "admin", "ops"]

[modules."commerce-payments-stripe"]
provider = "stripe"
checkout_mode = "hosted-checkout"
publishable_key = { kind = "env", var = "STRIPE_PUBLISHABLE_KEY" }
webhook_secret = { kind = "env", var = "STRIPE_WEBHOOK_SECRET" }

[wasm.secret_bindings]
commerce_payments_stripe_secret_key = { kind = "env", var = "STRIPE_SECRET_KEY" }
```

What each section does:

- `enabled = [...]` turns on the official Stripe payment module next to commerce, memberships,
  events, admin, and ops.
- `[modules."commerce-payments-stripe"]` tells Coil to use hosted Stripe Checkout and where to
  read the publishable key and webhook secret.
- `[wasm.secret_bindings]` exposes the Stripe secret key to the payment integration code without
  hard-coding it in the repository.

What you edit:

- `STRIPE_PUBLISHABLE_KEY`
- `STRIPE_SECRET_KEY`
- `STRIPE_WEBHOOK_SECRET`

What that enables:

- local hosted checkout configuration
- signed webhook verification
- customer-visible payment state that can move from pending to settled

## Show The Integration Boundary To Operators

Shoppr also exposes a dedicated integrations page in the admin shell.

`apps/shoppr/templates/admin/integrations.html`

```html
<section class="admin-panel">
  <p class="admin-panel__eyebrow">Integration summary</p>
  <div class="admin-card-grid">
    <article class="admin-card">
      <h2>Total integration points</h2>
      <p><strong coil:text="${integration_stats.total}">0</strong> integration points are declared across enabled modules.</p>
    </article>
    <article class="admin-card">
      <h2>Approved outbound endpoints</h2>
      <p><strong coil:text="${integration_stats.outbound_endpoints}">0</strong> outbound HTTP endpoints are explicitly approved in the runtime plan.</p>
    </article>
  </div>
</section>
```

What this page does:

- shows which integration seams are actually declared in the runtime
- separates request-time payment behavior from broader platform integration inventory
- gives operators one place to inspect endpoints and extension/plugin participation

## Use The Real Checkout Surface

The customer-facing handoff page lives in one file.

`apps/shoppr/templates/commerce/checkout.html`

```html
<!doctype html>
<html
  xmlns:coil="https://coil.rs"
  coil:with="page_title='Checkout'"
  lang="en-GB"
  coil:attr="lang=${locale}"
>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${page_title}">Checkout</title>
    <link rel="stylesheet" href="/theme/assets/site.css" coil:href="asset('theme/assets/site.css')" />
  </head>
  <body class="harbor">
    <header class="site-header">
      <a href="/" class="brand">Shoppr</a>
      <nav coil:replace="~{navigation/primary}"></nav>
    </header>
    <main class="site-main checkout-page">
      <section class="checkout-page__intro">
        <p class="checkout-page__eyebrow">Checkout</p>
        <h1>Review your order before the Stripe handoff.</h1>
        <p>
          Shoppr records the order details here, then hands the customer off to Stripe
          Checkout for payment collection.
        </p>
        <p>
          After Stripe sends the browser back, Shoppr keeps the order visible while the signed
          Stripe webhook settles the final payment state.
        </p>
        <ol class="checkout-list">
          <li>Confirm the receipt email and order summary.</li>
          <li>Place the order to open the Stripe Checkout handoff.</li>
          <li>Return to order status while Shoppr waits for the signed provider result.</li>
        </ol>
      </section>
      <section class="storefront-flash" coil:if="${has_flash_messages}">
        <article class="storefront-flash__message" coil:each="message : ${flash_messages}">
          <p class="storefront-flash__eyebrow" coil:text="${message.level}">error</p>
          <p coil:text="${message.text}">The checkout submission needs another look.</p>
        </article>
      </section>
      <section class="storefront-errors" coil:if="${checkout.has_errors}">
        <article class="storefront-errors__summary">
          <h2>Checkout details need attention</h2>
          <p coil:text="${checkout.error_summary}">
            Review the highlighted checkout fields and try again.
          </p>
          <ul class="storefront-errors__list">
            <li coil:each="error : ${checkout.errors}" coil:text="${error.message}">
              Update the highlighted field and try again.
            </li>
          </ul>
        </article>
      </section>

      <div class="checkout-page__grid">
        <section class="checkout-card">
          <h2>Contact</h2>
          <p coil:if="${checkout.has_checkout_email}" coil:text="${checkout.checkout_email}">hello@example.com</p>
          <p coil:unless="${checkout.has_checkout_email}">Guests can continue with email confirmation and order tracking.</p>

          <h2>Delivery</h2>
          <ul class="checkout-list">
            <li>Standard dispatch in 2-4 working days</li>
            <li>Collection notes supported for market-day pickup</li>
            <li>Gift messaging preserved on the order summary</li>
          </ul>

          <h2>Payment</h2>
          <p>
            This screen prepares the hosted Stripe Checkout handoff. Final payment confirmation is
            still resolved by the signed Stripe webhook after the hosted session completes.
          </p>
          <div class="checkout-provider">
            <p>
              <strong coil:text="${checkout.provider_label}">Stripe hosted checkout</strong>
            </p>
            <p coil:text="${checkout.provider_summary}">
              This checkout reserves the order in Coil, then redirects to Stripe Checkout for
              payment collection.
            </p>
            <p>
              Intent
              <strong coil:text="${checkout.payment_reference}">PAYMENT-PENDING</strong>
              is
              <span coil:text="${checkout.payment_status_label}">Ready for payment</span>.
            </p>
            <p>
              Shoppr keeps this payment reference and local order state visible so the return
              screen and account history can reconcile the final Stripe result after Stripe sends
              the customer back.
            </p>
            <p>
              If confirmation fails, the basket is restored so another checkout attempt can start
              from the cart.
            </p>
          </div>
        </section>

        <aside class="checkout-card">
          <h2>Order summary</h2>
          <ul class="checkout-summary" coil:if="${has_line_items}">
            <li coil:each="item : ${line_items}">
              <span coil:text="${item.title}">Harbor pantry set</span>
              <span coil:text="${item.quantity}">1</span>
              <strong coil:text="${item.total}">GBP 48</strong>
            </li>
          </ul>
          <div class="checkout-card__empty" coil:unless="${has_line_items}">
            <p>No items are in checkout for this browser session yet.</p>
            <p>Return to the cart or keep browsing to add products before placing an order.</p>
          </div>

          <div class="checkout-totals">
            <p><span>Subtotal</span><strong coil:text="${order_summary.subtotal}">GBP 70</strong></p>
            <p><span>Delivery</span><strong coil:text="${order_summary.shipping}">GBP 6</strong></p>
            <p><span>Total</span><strong coil:text="${order_summary.total}">GBP 76</strong></p>
          </div>

          <div class="checkout-actions" coil:if="${has_line_items}">
            <a class="button button--secondary" href="/cart">Return to cart</a>
            <form class="commerce-form checkout-form" action="/checkout/complete" method="post">
              <div class="checkout-form__grid">
                <label class="commerce-form__field">
                  <span>Receipt email</span>
                  <input
                    type="email"
                    name="checkout_email"
                    value="hello@example.com"
                    coil:attr="value=${checkout.checkout_email}"
                  />
                  <small class="commerce-form__error" coil:if="${checkout.has_checkout_email_error}" coil:text="${checkout.checkout_email_error}">
                    Enter the email address for order confirmation.
                  </small>
                </label>
                <label class="commerce-form__field">
                  <span>Customer session</span>
                  <input
                    type="text"
                    value="Shoppr Customer"
                    readonly="readonly"
                    coil:attr="value=${customer.display_name}"
                  />
                </label>
                <section class="commerce-form__field checkout-form__field--wide">
                  <span>Stripe handoff</span>
                  <p>
                    Method
                    <strong coil:text="${checkout.payment_method_label}">Card</strong>
                  </p>
                  <p>
                    Reference
                    <strong coil:text="${checkout.payment_reference}">PAYMENT-PENDING</strong>
                  </p>
                  <p>
                    Local status
                    <strong coil:text="${checkout.payment_status_label}">Ready for payment</strong>
                  </p>
                  <p coil:if="${checkout.has_payment_last4}">
                    Last confirmed card ending
                    <strong coil:text="${checkout.payment_last4}">4242</strong>.
                  </p>
                  <p>
                    Place the order to continue into the Stripe-backed confirmation flow. Shoppr
                    then returns you to the status page for this same browser session.
                  </p>
                  <small class="commerce-form__error" coil:if="${checkout.has_payment_method_error}" coil:text="${checkout.payment_method_error}">
                    Choose or confirm a payment method before placing the order.
                  </small>
                  <small class="commerce-form__error" coil:if="${checkout.has_payment_last4_error}" coil:text="${checkout.payment_last4_error}">
                    Enter the final 4 digits for the payment card.
                  </small>
                </section>
                <input
                  type="hidden"
                  name="payment_method"
                  value="card"
                  coil:attr="value=${checkout.payment_method}"
                />
                <input
                  type="hidden"
                  name="checkout_intent"
                  value="PAYMENT-PENDING"
                  coil:attr="value=${checkout.checkout_intent}"
                />
              </div>
              <p class="commerce-form__error" coil:if="${checkout.has_checkout_intent_error}" coil:text="${checkout.checkout_intent_error}">
                Refresh checkout and try again before placing the order.
              </p>
              <label class="commerce-form__checkbox" coil:if="${checkout.terms_accepted}">
                <input type="checkbox" name="terms_accepted" value="yes" checked="checked" />
                <span>I have reviewed the basket, receipt details, and the Stripe handoff summary.</span>
              </label>
              <label class="commerce-form__checkbox" coil:unless="${checkout.terms_accepted}">
                <input type="checkbox" name="terms_accepted" value="yes" />
                <span>I have reviewed the basket, receipt details, and the Stripe handoff summary.</span>
              </label>
              <p class="commerce-form__error" coil:if="${checkout.has_terms_accepted_error}" coil:text="${checkout.terms_accepted_error}">
                Review the basket and confirm the final total before placing the order.
              </p>
              <p class="checkout-form__consent">
                Placing the order keeps this browser on Shoppr while Stripe confirmation
                completes against the payment reference above.
              </p>
              <button class="button" type="submit" coil:text="${checkout.submit_label}">Place order</button>
            </form>
          </div>
          <div class="checkout-actions" coil:unless="${has_line_items}">
            <a class="button button--secondary" href="/cart">Return to cart</a>
            <a class="button" href="/en-GB/shop" coil:attr="href=${links.catalog}">
              Continue shopping
            </a>
          </div>
        </aside>
      </div>
    </main>
    <footer class="site-footer">
      <small>Shoppr</small>
    </footer>
  </body>
</html>
```

What this file owns:

- the honest explanation of the Stripe handoff
- the local order summary before the redirect
- the checkout form that posts into the payment flow
- the messages customers see while payment is still pending

Important sections:

- `checkout.provider_*` explains which provider the customer is about to use.
- `checkout.payment_reference` keeps the local payment intent visible before and after the
  redirect.
- `action="/checkout/complete"` is the handoff point that starts the provider flow.
- `customer.display_name`, `order_summary.*`, and `line_items` keep the local order state visible
  before payment has settled.

## Show The Real Return Screen

After Stripe sends the browser back, Shoppr uses the return screen below.

`apps/shoppr/templates/commerce/checkout-confirmation.html`

```html
<!doctype html>
<html
  xmlns:coil="https://coil.rs"
  coil:with="page_title='Order status'"
  lang="en-GB"
  coil:attr="lang=${locale}"
>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${page.title}">Order confirmed</title>
    <link rel="stylesheet" href="/theme/assets/site.css" coil:href="asset('theme/assets/site.css')" />
  </head>
  <body class="harbor">
    <header class="site-header">
      <a href="/" class="brand">Shoppr</a>
      <nav coil:replace="~{navigation/primary}"></nav>
    </header>
    <main class="site-main account-page">
      <section class="storefront-flash" coil:if="${has_flash_messages}">
        <article class="storefront-flash__message" coil:each="message : ${flash_messages}">
          <p class="storefront-flash__eyebrow" coil:text="${message.level}">info</p>
          <p coil:text="${message.text}">Your order is still moving through payment confirmation.</p>
        </article>
      </section>
      <section class="account-page__intro" coil:if="${has_confirmation}">
        <p class="account-page__eyebrow">Stripe return</p>
        <h1>Order and payment status</h1>
        <p>
          This is the return screen for the current browser session after checkout submission.
          Review the payment state here before leaving the order.
        </p>
        <p class="checkout-confirmation__order">
          Reference <strong coil:text="${confirmation.order_number}">HS-10482</strong>
        </p>
        <p>
          Status <strong coil:text="${confirmation.status}">Paid</strong>
          with total <strong coil:text="${confirmation.total}">£0.00</strong>.
        </p>
        <p coil:if="${confirmation.has_email}">
          Receipt email <strong coil:text="${confirmation.email}">member@example.com</strong>.
        </p>
        <p coil:text="${confirmation.next_step}">
          A confirmation email and membership activation will follow shortly.
        </p>
        <div class="checkout-confirmation__payment">
          <p>
            <strong coil:text="${confirmation.provider_label}">Stripe payment confirmation</strong>
          </p>
          <p>
            Shoppr has already recorded the order and is now waiting for the provider result
            tied to this payment route.
          </p>
          <p>
            Payment state
            <strong coil:text="${confirmation.payment_status}">Awaiting provider confirmation</strong>.
          </p>
          <p coil:text="${confirmation.payment_summary}">
            Card ending 4242, reference PAY-50001
          </p>
          <p>
            If you have just returned from Stripe, this page can remain in a pending state until
            the signed confirmation reaches Shoppr.
          </p>
        </div>
        <ol class="checkout-list">
          <li>Order recorded in Shoppr under the reference above.</li>
          <li>Stripe confirmation updates the payment state shown on this page.</li>
          <li>Order history and memberships in this same browser session reflect the final result.</li>
        </ol>
        <ul class="checkout-summary checkout-summary--stacked" coil:if="${confirmation.has_line_items}">
          <li coil:each="item : ${confirmation.line_items}">
            <span coil:text="${item.title}">Harbor pantry set</span>
            <span coil:text="${item.quantity}">1</span>
            <strong coil:text="${item.total}">GBP 48</strong>
          </li>
        </ul>
        <div class="checkout-actions">
          <a class="button" href="/account/orders">View order history</a>
          <a class="button button--secondary" href="/account">View account</a>
          <a class="button button--secondary" href="/account/memberships" coil:if="${confirmation.has_membership_items}">
            Check memberships
          </a>
          <a
            class="button button--secondary"
            href="/en-GB/shop"
            coil:attr="href=${links.catalog}"
          >
            Continue shopping
          </a>
        </div>
      </section>
      <section class="account-page__intro" coil:unless="${has_confirmation}">
        <p class="account-page__eyebrow">Stripe return</p>
        <h1>No recent provider return to review</h1>
        <p coil:text="${confirmation.next_step}">
          There is no recent checkout confirmation for this browser session yet.
        </p>
        <p>
          If you expected a Stripe return, reopen checkout or order history in the same browser
          session and verify that the latest payment reference is still present.
        </p>
        <div class="checkout-actions">
          <a class="button" href="/cart">Return to cart</a>
          <a class="button button--secondary" href="/account/orders">View order history</a>
          <a
            class="button button--secondary"
            href="/en-GB/shop"
            coil:attr="href=${links.catalog}"
          >
            Continue shopping
          </a>
        </div>
      </section>
      <div coil:replace="~{account/summary-panels :: panels}"></div>
    </main>
    <footer class="site-footer">
      <small>Shoppr</small>
    </footer>
  </body>
</html>
```

What this file owns:

- the browser return state after Stripe Checkout
- the customer-visible payment status
- the next actions after payment is pending or settled
- the link back into account, memberships, and catalog flows

Important sections:

- `confirmation.payment_status` and `confirmation.payment_summary` are the fields the customer
  checks first after returning from the provider.
- `confirmation.has_membership_items` decides whether the page needs to surface the memberships
  route as a next step.
- the `has_confirmation` branch keeps the return page honest when there is no recent provider
  session in the current browser.

## Keep Customer And Operator Order Views Separate

The customer-facing order history lives here.

`apps/shoppr/templates/account/orders.html`

```html
<!doctype html>
<html
  xmlns:coil="https://coil.rs"
  coil:with="page_title='Order history'"
  lang="en-GB"
  coil:attr="lang=${locale}"
>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${page_title}">Order history</title>
    <link rel="stylesheet" href="/theme/assets/site.css" coil:href="asset('theme/assets/site.css')" />
  </head>
  <body class="harbor">
    <header class="site-header">
      <a href="/" class="brand">Shoppr</a>
      <nav coil:replace="~{navigation/primary}"></nav>
    </header>
    <main class="site-main account-page">
      <section class="account-page__intro">
        <p class="account-page__eyebrow">Customer account</p>
        <h1>Order history</h1>
        <p coil:text="${account.state_summary}">
          Review completed purchases, payment details, and any membership-linked checkout history.
        </p>
        <p coil:if="${account.has_principal}">
          Signed in as <strong coil:text="${customer.display_name}">Member Live</strong>.
        </p>
        <p coil:unless="${account.has_principal}">
          This order history currently follows the browser session you are using right now. Orders
          placed from another browser will not appear here yet.
        </p>
        <p coil:if="${account.has_latest_order}">
          If you have just returned from the payment provider, the latest order can remain in
          <strong>Pending Payment</strong>
          until settlement is confirmed for this same browser session.
        </p>
        <p coil:if="${account.has_customer_email}">
          Receipt email for this account view:
          <strong coil:text="${customer.email}">member@example.com</strong>.
        </p>
      </section>
      <nav class="account-page__nav" coil:replace="~{account/nav :: nav}"></nav>

      <section class="account-orders" coil:if="${account.has_recent_orders}">
        <article class="account-panel">
          <p class="account-panel__eyebrow">Orders</p>
          <h2>Latest purchases</h2>
          <ol class="account-panel__list">
            <li coil:each="order : ${recent_orders}">
              <div>
                <strong coil:text="${order.reference}">ORD-10042</strong>
                <span coil:text="${order.status}">Paid</span>
                <span coil:text="${order.total}">GBP 84</span>
              </div>
              <p>
                <span coil:text="${order.line_count}">1</span>
                line items in this order.
              </p>
              <p coil:if="${order.has_payment_summary}" coil:text="${order.payment_summary}">
                Card ending 4242, reference PAY-50001
              </p>
              <p coil:if="${order.has_checkout_email}">
                Receipt email
                <strong coil:text="${order.checkout_email}">member@example.com</strong>
              </p>
            </li>
          </ol>
        </article>
      </section>

      <section class="account-orders" coil:unless="${account.has_recent_orders}">
        <article class="account-panel">
          <p class="account-panel__eyebrow">Orders</p>
          <h2>No completed orders yet</h2>
          <p coil:text="${account.orders_empty_text}">
            Completed storefront purchases will appear here once checkout has finished.
          </p>
          <a
            class="button button--secondary"
            href="/shop"
            coil:attr="href=${account.orders_cta_url}"
            coil:text="${account.orders_cta_label}"
          >
            Browse storefront
          </a>
        </article>
      </section>

      <section class="account-page__cards">
        <article class="account-card">
          <h2>Memberships</h2>
          <div coil:if="${account.has_membership}">
            <p>
              <strong coil:text="${membership_summary.tier_name}">Harbor Circle</strong>
              <span coil:text="${membership_summary.status}">Purchased</span>
            </p>
            <p coil:text="${membership_summary.renewal_text}">
              Renewal timing and entitlement state will appear here after sync.
            </p>
            <a class="button" href="/account/memberships">View memberships</a>
          </div>
          <div coil:if="${account.has_pending_membership_order}">
            <p>
              Latest order
              <strong coil:text="${account.latest_order_reference}">ORD-10042</strong>
              is
              <span coil:text="${account.latest_order_status}">Pending Payment</span>.
              Membership access only appears here after a qualifying membership purchase is
              captured for this account view. After a provider return, use this order history to
              confirm the latest status, then return to memberships once payment has settled.
            </p>
            <a class="button" href="/account/memberships">View memberships</a>
            <a class="button button--secondary" href="/account/orders">
              Review this order
            </a>
          </div>
          <div coil:if="${account.needs_membership_purchase}">
            <p coil:text="${account.membership_empty_text}">
              No active membership is attached yet.
            </p>
            <a class="button" href="/account/memberships">View memberships</a>
            <a
              class="button button--secondary"
              href="/shop/collections/memberships"
              coil:attr="href=${account.membership_cta_url}"
            >
              Explore memberships
            </a>
          </div>
        </article>
        <article class="account-card">
          <h2>Next steps</h2>
          <p coil:if="${account.has_recent_orders}">
            Review order status, then continue shopping or return to checkout if you still have
            work to finish on this browser session. If you bought membership access, the
            memberships page updates from the same session after payment capture. Pending Payment
            after a provider return means settlement is still completing.
          </p>
          <p coil:unless="${account.has_recent_orders}">
            Start from the storefront, then come back here once checkout has completed on this
            browser session.
          </p>
          <a class="button" href="/checkout">Open checkout</a>
          <a class="button button--secondary" href="/en-GB/shop" coil:attr="href=${links.catalog}">
            Continue shopping
          </a>
        </article>
      </section>
    </main>
    <footer class="site-footer">
      <small>Shoppr</small>
    </footer>
  </body>
</html>
```

This file owns the customer account view. It uses the storefront bundle and explains payment state
from the customer’s perspective.

The operator queue is a different file.

`apps/shoppr/templates/commerce/orders.html`

```html
<!doctype html>
<html
  xmlns:coil="https://coil.rs"
  coil:with="page_title='Orders'"
  lang="en-GB"
  coil:attr="lang=${locale}"
>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${page_title}">Orders</title>
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
        <p class="admin-page__eyebrow">Commerce operations</p>
        <h1 coil:text="${page.title}">Orders</h1>
        <p coil:text="${page.summary}">
          Operator order queue and payment history.
        </p>
        <p coil:if="${operator.has_principal}">
          Signed in as <strong coil:text="${operator.display_name}">Current Operator</strong>.
        </p>
        <p coil:if="${operator.has_principal}">
          Principal id <code coil:text="${operator.principal_id}">operator-live-1</code>.
        </p>
        <p>
          This queue is store-wide. Use it to confirm payment state, review checkout email and
          totals, and move into the per-order support detail view before escalating a checkout
          case.
        </p>
      </section>

      <section class="storefront-flash" coil:if="${has_flash_messages}">
        <article class="storefront-flash__message" coil:each="message : ${flash_messages}">
          <p class="storefront-flash__eyebrow" coil:text="${message.level}">info</p>
          <p coil:text="${message.text}">Order support update</p>
        </article>
      </section>

      <section class="admin-panel">
        <p class="admin-panel__eyebrow">Queue summary</p>
        <div class="admin-card-grid">
          <article class="admin-card">
            <h2>Total visible orders</h2>
            <p>
              <strong coil:text="${order_stats.total}">0</strong>
              orders are currently visible to support.
            </p>
          </article>
          <article class="admin-card">
            <h2>Pending confirmation</h2>
            <p>
              <strong coil:text="${order_stats.pending}">0</strong>
              orders are still awaiting payment confirmation or operator follow-up.
            </p>
          </article>
          <article class="admin-card">
            <h2>Refunded orders</h2>
            <p>
              <strong coil:text="${order_stats.refunded}">0</strong>
              orders have already moved through the refund workflow.
            </p>
          </article>
          <article class="admin-card">
            <h2>Stripe follow-up</h2>
            <p>
              <strong coil:text="${order_stats.payment_follow_up}">0</strong>
              orders still need provider confirmation before operators should treat payment as settled.
            </p>
          </article>
        </div>
      </section>

      <section class="admin-panel">
        <p class="admin-panel__eyebrow">Operator guidance</p>
        <div class="admin-card-grid">
          <article class="admin-card">
            <h2>Support first</h2>
            <p>
              Start in the detail view for customer email, payment reference, line items, and
              refund eligibility. That keeps escalations anchored to one order record.
            </p>
          </article>
          <article class="admin-card">
            <h2>Pending Payment is not failure</h2>
            <p>
              After a Stripe return, compare the customer account view and provider callback window
              before treating Pending Payment as a failed checkout.
            </p>
          </article>
          <article class="admin-card">
            <h2>Refund boundary</h2>
            <p>
              The checked-in workflow supports full remaining refunds from order detail. Use deeper
              provider diagnostics if reconciliation disagrees with this local order state.
            </p>
          </article>
        </div>
      </section>

      <section class="admin-panel" coil:if="${has_recent_orders}" data-admin-filter="">
        <p class="admin-panel__eyebrow">Recent orders</p>
        <label class="admin-filter">
          <span>Filter the support queue</span>
          <input
            type="search"
            placeholder="Filter by order, status, payment, or customer..."
            data-admin-filter-input=""
          />
        </label>
        <table class="admin-table">
          <thead>
            <tr>
              <th scope="col">Order</th>
              <th scope="col">Status</th>
              <th scope="col">Payment</th>
              <th scope="col">Support state</th>
              <th scope="col">Customer</th>
              <th scope="col">Total</th>
              <th scope="col">Actions</th>
            </tr>
          </thead>
          <tbody>
            <tr coil:each="order : ${recent_orders}" data-admin-filter-item="">
              <td>
                <div class="admin-copy-row">
                  <strong coil:text="${order.reference}">ORD-10042</strong>
                  <button
                    class="button button--secondary"
                    type="button"
                    coil:attr="data-copy-text=${order.reference}"
                  >
                    Copy
                  </button>
                </div>
              </td>
              <td coil:text="${order.status}">Paid</td>
              <td coil:text="${order.payment_status}">Captured</td>
              <td>
                <strong coil:text="${order.support_state_label}">No payment follow-up required</strong>
                <div coil:if="${order.has_payment_reference}">
                  <code coil:text="${order.payment_reference}">PAY-50001</code>
                </div>
              </td>
              <td>
                <span coil:if="${order.has_customer_email}" coil:text="${order.customer_email}">
                  member@example.com
                </span>
                <span coil:unless="${order.has_customer_email}">Receipt email pending</span>
              </td>
              <td coil:text="${order.total}">£0.00</td>
              <td>
                <a
                  class="button button--secondary"
                  href="/admin/orders/ORD-10042"
                  coil:attr="href=${order.detail_href}"
                >
                  View details
                </a>
              </td>
            </tr>
          </tbody>
        </table>
        <div class="admin-page__actions">
          <a class="button button--secondary" href="/admin" coil:attr="href=${links.admin_dashboard}">
            Back to dashboard
          </a>
          <a class="button button--secondary" href="/account/orders" coil:attr="href=${links.orders}">
            Open storefront order history
          </a>
          <a class="button button--secondary" href="/shop" coil:attr="href=${links.catalog}">
            Open storefront catalog
          </a>
        </div>
      </section>

      <section class="admin-panel" coil:unless="${has_recent_orders}">
        <p class="admin-panel__eyebrow">Recent orders</p>
        <h2>No completed orders yet</h2>
        <p coil:text="${orders_empty_text}">
          No completed orders have been captured in the checked-in sample app yet.
        </p>
        <p>
          Use the storefront checkout flow to generate the first supportable order, then return
          here to confirm that the order queue and payment handoff are visible to operators.
        </p>
        <div class="admin-page__actions">
          <a class="button" href="/shop" coil:attr="href=${links.catalog}">
            Open storefront
          </a>
          <a class="button button--secondary" href="/admin" coil:attr="href=${links.admin_dashboard}">
            Back to dashboard
          </a>
        </div>
      </section>
    </main>
    <footer class="site-footer">
      <small>Shoppr</small>
    </footer>
  </body>
</html>
```

This file owns the operator support queue. It uses the admin bundle because store-wide order work
belongs in the admin shell, not in the customer account shell.

## Run The Real Local Flow

From the Shoppr app root:

```bash
cd apps/shoppr
cp .env.example .env
docker compose up --build
```

The default `.env.example` values are enough to exercise the built-in local checkout stub. You do
not need live Stripe credentials to test the page flow.

To test a real Stripe webhook locally, run this in a separate terminal:

```bash
stripe listen --forward-to http://uk.localhost:8080/webhooks/commerce/payment-provider
```

Then copy the webhook signing secret into `.env` as `STRIPE_WEBHOOK_SECRET` and restart the stack.

## Runnable Checkpoint

Verify all of these in the running app:

- `/checkout` shows the Shoppr-owned Stripe handoff explanation from
  `templates/commerce/checkout.html`
- submitting checkout returns to the provider return screen in
  `templates/commerce/checkout-confirmation.html`
- `/account/orders` shows the customer-facing order history from
  `templates/account/orders.html`
- `/admin/orders` shows the operator queue from `templates/commerce/orders.html`
- if Stripe is connected, the webhook can settle the payment state after the browser returns
