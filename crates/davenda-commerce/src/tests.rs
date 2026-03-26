use super::*;
use davenda_auth::Capability;
use davenda_config::{PlatformConfig, SecretRef};
use davenda_core::{
    CoreServiceDependency, ExtensionSlotKind, HttpResponseContract, HttpSurfaceArea,
    HttpSurfaceMethod, PlatformModule, RouteSurfaceKind, ServiceRegistry,
};
use davenda_data::{MigrationOwner, PublicationVisibility, QueryCacheScope, TransactionIsolation};
use std::collections::BTreeMap;

fn gbp(value: i64) -> Money {
    Money::new(CurrencyCode::new("GBP").unwrap(), value).unwrap()
}

fn membership_product() -> CatalogProduct {
    CatalogProduct::new(
        ProductId::new("product-gold").unwrap(),
        ProductHandle::new("gold-membership").unwrap(),
        "Gold Membership",
        ProductKind::Membership {
            entitlement_key: EntitlementKey::new("membership.gold").unwrap(),
        },
    )
    .unwrap()
    .with_variant(
        ProductVariant::new(
            Sku::new("membership-gold").unwrap(),
            "Gold Membership",
            gbp(10_000),
        )
        .unwrap(),
    )
    .unwrap()
    .activate()
}

fn tshirt_product() -> CatalogProduct {
    CatalogProduct::new(
        ProductId::new("product-shirt").unwrap(),
        ProductHandle::new("davenda-shirt").unwrap(),
        "Davenda Shirt",
        ProductKind::Physical,
    )
    .unwrap()
    .with_variant(ProductVariant::new(Sku::new("shirt-m").unwrap(), "Medium", gbp(2_500)).unwrap())
    .unwrap()
    .activate()
}

#[test]
fn commerce_module_manifest_declares_expected_capabilities_and_registers_services() {
    let module = CommerceModule::new();
    let manifest = module.manifest();
    let mut registry = ServiceRegistry::new();

    module.register(&mut registry).unwrap();

    assert_eq!(manifest.name, "commerce");
    assert_eq!(manifest.config_namespace.as_deref(), Some("commerce"));
    assert_eq!(
        manifest.required_capabilities,
        vec![
            Capability::CatalogProductRead,
            Capability::CatalogProductEdit,
            Capability::CatalogCollectionEdit,
            Capability::CheckoutSessionCreate,
            Capability::OrderRead,
            Capability::OrderRefundIssue,
        ]
    );
    assert!(
        manifest
            .optional_capabilities
            .contains(&Capability::AdminShellAccess)
    );
    assert!(
        manifest
            .optional_capabilities
            .contains(&Capability::AssetRead)
    );
    assert_eq!(manifest.migrations.len(), 3);
    assert_eq!(manifest.route_surfaces.len(), 19);
    assert_eq!(manifest.http_surfaces.len(), 19);
    assert_eq!(manifest.jobs.len(), 2);
    assert_eq!(manifest.event_subscriptions.len(), 2);
    assert_eq!(manifest.search_contributions.len(), 2);
    assert_eq!(manifest.report_definitions.len(), 1);
    assert!(
        manifest
            .module_dependencies
            .iter()
            .any(|dependency| dependency.module == "memberships")
    );
    assert!(
        manifest
            .core_service_dependencies
            .contains(&CoreServiceDependency::Jobs)
    );
    assert!(
        manifest
            .extension_slots
            .iter()
            .any(|slot| slot.kind == ExtensionSlotKind::Webhook)
    );
    assert_eq!(manifest.admin_resources.len(), 4);
    assert!(
        registry
            .services()
            .any(|service| service.id == "module.commerce.membership_bridge")
    );
    assert_eq!(module.admin_resources().len(), 4);
}

#[test]
fn commerce_payments_stripe_module_declares_concrete_provider_installation_shape() {
    let module = CommercePaymentsStripeModule::new();
    let manifest = module.manifest();
    let mut registry = ServiceRegistry::new();

    module.register(&mut registry).unwrap();

    assert_eq!(manifest.name, "commerce-payments-stripe");
    assert_eq!(
        manifest.config_namespace.as_deref(),
        Some("commerce_payments_stripe")
    );
    assert!(
        manifest
            .module_dependencies
            .iter()
            .any(|dependency| dependency.module == "commerce")
    );
    assert!(
        manifest
            .core_service_dependencies
            .contains(&CoreServiceDependency::Observability)
    );
    assert!(
        manifest
            .behaviors
            .contains(&davenda_core::ModuleBehavior::AsyncJobs)
    );
    assert!(
        registry
            .services()
            .any(|service| service.id == "module.commerce.payments.stripe")
    );

    let metadata = CommercePaymentsStripeModule::provider_metadata(&CommercePaymentsStripeConfig {
        provider: "stripe".to_string(),
        checkout_mode: StripeCheckoutMode::WebhookConfirmation,
        publishable_key: SecretRef::Env {
            var: "STRIPE_PUBLISHABLE_KEY".to_string(),
        },
        webhook_secret: SecretRef::Env {
            var: "STRIPE_WEBHOOK_SECRET".to_string(),
        },
    });
    assert_eq!(metadata.provider_code, "stripe");
    assert_eq!(metadata.provider_label, "Stripe");
    assert_eq!(
        metadata.checkout_mode,
        StripeCheckoutMode::WebhookConfirmation
    );
    assert_eq!(
        metadata.webhook_route,
        "/webhooks/commerce/payment-provider"
    );
    assert_eq!(metadata.publishable_key_ref, "env:STRIPE_PUBLISHABLE_KEY");
    assert_eq!(metadata.webhook_secret_ref, "env:STRIPE_WEBHOOK_SECRET");
}

#[test]
fn commerce_payments_stripe_config_requires_publishable_key_and_webhook_secret() {
    let config = PlatformConfig::from_toml_str(
        r#"
[app]
name = "shop"
environment = "production"

[server]
bind = "0.0.0.0:8080"
trusted_proxies = []

[http.session]
store = "redis"
idle_timeout_secs = 3600
absolute_timeout_secs = 86400

[http.session_cookie]
name = "davenda_session"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.flash_cookie]
name = "davenda_flash"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.csrf]
enabled = true
field_name = "_csrf"
header_name = "x-csrf-token"

[tls]
mode = "external"

[storage]
default_class = "public_upload"
local_root = "/tmp/davenda"

[cache]
l1 = "moka"
l2 = "redis"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB"]
fallback_locale = "en-GB"

[seo]
canonical_host = "shop.example.com"
emit_json_ld = true

[auth]
package = "platform-default-auth"
explain_api = false
tenant_id = 101

[modules]
enabled = ["commerce", "commerce-payments-stripe"]

[modules."commerce-payments-stripe"]
provider = "stripe"
checkout_mode = "webhook-confirmation"
publishable_key = { kind = "env", var = "STRIPE_PUBLISHABLE_KEY" }
webhook_secret = { kind = "env", var = "STRIPE_WEBHOOK_SECRET" }

[wasm]
directory = "extensions"
default_time_limit_ms = 50
allow_network = false

[jobs]
backend = "redis"

[observability]
metrics = true
tracing = true

[assets]
publish_manifest = true
cdn_base_url = "https://cdn.example.com"
"#,
    )
    .unwrap();

    let stripe = CommercePaymentsStripeConfig::from_platform_config(&config)
        .unwrap()
        .expect("stripe config");
    assert_eq!(stripe.provider, "stripe");
    assert_eq!(
        stripe.checkout_mode,
        StripeCheckoutMode::WebhookConfirmation
    );
    assert_eq!(
        stripe.publishable_key,
        SecretRef::Env {
            var: "STRIPE_PUBLISHABLE_KEY".to_string(),
        }
    );
    assert_eq!(
        stripe.webhook_secret,
        SecretRef::Env {
            var: "STRIPE_WEBHOOK_SECRET".to_string(),
        }
    );
}

#[test]
fn commerce_payments_stripe_config_accepts_hosted_checkout_mode() {
    let config = PlatformConfig::from_toml_str(
        r#"
[app]
name = "shop"
environment = "production"

[server]
bind = "0.0.0.0:8080"
trusted_proxies = []

[http.session]
store = "redis"
idle_timeout_secs = 3600
absolute_timeout_secs = 86400

[http.session_cookie]
name = "davenda_session"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.flash_cookie]
name = "davenda_flash"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.csrf]
enabled = true
field_name = "_csrf"
header_name = "x-csrf-token"

[tls]
mode = "external"

[storage]
default_class = "public_upload"
local_root = "/tmp/davenda"

[cache]
l1 = "moka"
l2 = "redis"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB"]
fallback_locale = "en-GB"

[seo]
canonical_host = "shop.example.com"
emit_json_ld = true

[auth]
package = "platform-default-auth"
explain_api = false
tenant_id = 101

[modules]
enabled = ["commerce", "commerce-payments-stripe"]

[modules."commerce-payments-stripe"]
provider = "stripe"
checkout_mode = "hosted-checkout"
publishable_key = { kind = "env", var = "STRIPE_PUBLISHABLE_KEY" }
webhook_secret = { kind = "env", var = "STRIPE_WEBHOOK_SECRET" }

[wasm]
directory = "extensions"
default_time_limit_ms = 50
allow_network = false

[jobs]
backend = "redis"

[observability]
metrics = true
tracing = true

[assets]
publish_manifest = true
cdn_base_url = "https://cdn.example.com"
"#,
    )
    .unwrap();

    let stripe = CommercePaymentsStripeConfig::from_platform_config(&config)
        .unwrap()
        .expect("stripe config");
    assert_eq!(stripe.provider, "stripe");
    assert_eq!(stripe.checkout_mode, StripeCheckoutMode::HostedCheckout);
}

#[test]
fn commerce_payments_stripe_config_fails_closed_when_handoff_contract_is_incomplete() {
    let config = PlatformConfig::from_toml_str(
        r#"
[app]
name = "shop"
environment = "production"

[server]
bind = "0.0.0.0:8080"
trusted_proxies = []

[http.session]
store = "redis"
idle_timeout_secs = 3600
absolute_timeout_secs = 86400

[http.session_cookie]
name = "davenda_session"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.flash_cookie]
name = "davenda_flash"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.csrf]
enabled = true
field_name = "_csrf"
header_name = "x-csrf-token"

[tls]
mode = "external"

[storage]
default_class = "public_upload"
local_root = "/tmp/davenda"

[cache]
l1 = "moka"
l2 = "redis"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB"]
fallback_locale = "en-GB"

[seo]
canonical_host = "shop.example.com"
emit_json_ld = true

[auth]
package = "platform-default-auth"
explain_api = false
tenant_id = 101

[modules]
enabled = ["commerce", "commerce-payments-stripe"]

[modules."commerce-payments-stripe"]
provider = "stripe"
checkout_mode = "webhook-confirmation"
webhook_secret = { kind = "env", var = "STRIPE_WEBHOOK_SECRET" }

[wasm]
directory = "extensions"
default_time_limit_ms = 50
allow_network = false

[jobs]
backend = "redis"

[observability]
metrics = true
tracing = true

[assets]
publish_manifest = true
cdn_base_url = "https://cdn.example.com"
"#,
    )
    .unwrap();

    let error = CommercePaymentsStripeConfig::from_platform_config(&config).unwrap_err();
    assert_eq!(
        error.to_string(),
        "module `commerce-payments-stripe` requires setting `publishable_key`"
    );
}

#[test]
fn checked_in_harbor_shop_declares_the_stripe_handoff_contract() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("apps/harbor-shop/platform.toml");
    let config = PlatformConfig::from_file(root).unwrap();
    let stripe = CommercePaymentsStripeConfig::from_platform_config(&config)
        .unwrap()
        .expect("checked-in stripe config");

    assert_eq!(stripe.provider, "stripe");
    assert_eq!(stripe.checkout_mode, StripeCheckoutMode::HostedCheckout);
    assert_eq!(
        stripe.publishable_key.redacted(),
        "env:STRIPE_PUBLISHABLE_KEY"
    );
    assert_eq!(
        stripe.webhook_secret.redacted(),
        "env:STRIPE_WEBHOOK_SECRET"
    );
    assert_eq!(
        config
            .wasm
            .secret_bindings
            .get("commerce_payments_stripe_secret_key")
            .expect("hosted checkout should declare a Stripe secret-key binding")
            .redacted(),
        "env:STRIPE_SECRET_KEY"
    );
}

#[test]
fn commerce_module_manifest_exposes_basic_storefront_listing_detail_cart_and_completion_surfaces() {
    let module = CommerceModule::new();
    let manifest = module.manifest();

    let catalog = manifest
        .route_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.catalog")
        .expect("catalog surface should exist");
    assert_eq!(catalog.kind, RouteSurfaceKind::FrontendPage);
    assert_eq!(catalog.path, "/shop");
    assert!(catalog.localized);
    assert_eq!(catalog.capability, None);

    let collections = manifest
        .route_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.collections")
        .expect("collections surface should exist");
    assert_eq!(collections.kind, RouteSurfaceKind::FrontendPage);
    assert_eq!(collections.path, "/shop/collections");
    assert!(collections.localized);
    assert_eq!(collections.capability, None);

    let collection_detail = manifest
        .route_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.collection-detail")
        .expect("collection detail surface should exist");
    assert_eq!(collection_detail.kind, RouteSurfaceKind::FrontendPage);
    assert_eq!(
        collection_detail.path,
        "/shop/collections/{collection_slug}"
    );
    assert!(collection_detail.localized);
    assert_eq!(collection_detail.capability, None);

    let product_detail = manifest
        .route_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.product-detail")
        .expect("product detail surface should exist");
    assert_eq!(product_detail.kind, RouteSurfaceKind::FrontendPage);
    assert_eq!(product_detail.path, "/shop/products/{product_slug}");
    assert!(product_detail.localized);
    assert_eq!(product_detail.capability, None);

    let cart = manifest
        .route_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.cart")
        .expect("cart surface should exist");
    assert_eq!(cart.kind, RouteSurfaceKind::FrontendPage);
    assert_eq!(cart.path, "/cart");
    assert_eq!(cart.capability, None);

    let checkout = manifest
        .route_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.checkout")
        .expect("checkout surface should exist");
    assert_eq!(checkout.kind, RouteSurfaceKind::FrontendPage);
    assert_eq!(checkout.path, "/checkout");
    assert_eq!(checkout.capability, None);

    let add_to_cart = manifest
        .route_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.add-to-cart")
        .expect("add-to-cart surface should exist");
    assert_eq!(add_to_cart.kind, RouteSurfaceKind::FrontendAction);
    assert_eq!(add_to_cart.path, "/cart/items");
    assert_eq!(add_to_cart.capability, None);

    let cart_update = manifest
        .route_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.cart-update")
        .expect("cart update surface should exist");
    assert_eq!(cart_update.kind, RouteSurfaceKind::FrontendAction);
    assert_eq!(cart_update.path, "/cart");
    assert_eq!(cart_update.capability, None);

    let checkout_start = manifest
        .route_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.checkout-start")
        .expect("checkout start surface should exist");
    assert_eq!(checkout_start.kind, RouteSurfaceKind::FrontendAction);
    assert_eq!(checkout_start.path, "/checkout/start");
    assert_eq!(checkout_start.capability, None);

    let checkout_complete = manifest
        .route_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.checkout-complete")
        .expect("checkout complete surface should exist");
    assert_eq!(checkout_complete.kind, RouteSurfaceKind::FrontendAction);
    assert_eq!(checkout_complete.path, "/checkout/complete");
    assert_eq!(checkout_complete.capability, None);

    let payment_webhook = manifest
        .route_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.payment-provider-webhook")
        .expect("payment webhook surface should exist");
    assert_eq!(payment_webhook.kind, RouteSurfaceKind::Webhook);
    assert_eq!(payment_webhook.path, "/webhooks/commerce/payment-provider");
    assert_eq!(payment_webhook.capability, None);

    let checkout_confirmation = manifest
        .route_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.checkout-confirmation")
        .expect("checkout confirmation surface should exist");
    assert_eq!(checkout_confirmation.kind, RouteSurfaceKind::FrontendPage);
    assert_eq!(checkout_confirmation.path, "/checkout/confirmation");
    assert_eq!(checkout_confirmation.capability, None);

    let account_orders = manifest
        .route_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.account.orders")
        .expect("account orders surface should exist");
    assert_eq!(account_orders.kind, RouteSurfaceKind::FrontendPage);
    assert_eq!(account_orders.path, "/account/orders");
    assert_eq!(account_orders.capability, None);

    let account_session_end = manifest
        .route_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.account-session-end")
        .expect("account session end surface should exist");
    assert_eq!(account_session_end.kind, RouteSurfaceKind::FrontendAction);
    assert_eq!(account_session_end.path, "/account/session/end");
    assert_eq!(account_session_end.capability, None);

    let catalog_admin_update = manifest
        .route_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.catalog-admin-update")
        .expect("catalog admin update surface should exist");
    assert_eq!(catalog_admin_update.kind, RouteSurfaceKind::AdminAction);
    assert_eq!(catalog_admin_update.path, "/admin/catalog/products");
    assert_eq!(
        catalog_admin_update.capability,
        Some(Capability::CatalogProductEdit)
    );

    let order_detail = manifest
        .route_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.order-detail")
        .expect("order detail surface should exist");
    assert_eq!(order_detail.kind, RouteSurfaceKind::AdminPage);
    assert_eq!(order_detail.path, "/admin/orders/{order_id}");
    assert_eq!(order_detail.capability, Some(Capability::OrderRead));

    let order_refund = manifest
        .route_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.order-refund")
        .expect("order refund surface should exist");
    assert_eq!(order_refund.kind, RouteSurfaceKind::AdminAction);
    assert_eq!(order_refund.path, "/admin/orders/refund");
    assert_eq!(order_refund.capability, Some(Capability::OrderRefundIssue));
}

#[test]
fn commerce_module_http_surfaces_match_storefront_route_contracts() {
    let module = CommerceModule::new();
    let manifest = module.manifest();

    let collection_detail = manifest
        .http_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.collection-detail")
        .expect("collection detail http surface should exist");
    assert_eq!(collection_detail.area, HttpSurfaceArea::Public);
    assert_eq!(
        collection_detail.path,
        "/shop/collections/{collection_slug}"
    );
    assert!(collection_detail.localized);
    assert_eq!(collection_detail.capability, None);
    assert_eq!(
        collection_detail.response,
        HttpResponseContract::Page {
            template: "commerce/collection-detail".to_string(),
            status: 200,
        }
    );

    let collections = manifest
        .http_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.collections")
        .expect("collections http surface should exist");
    assert_eq!(collections.area, HttpSurfaceArea::Public);
    assert_eq!(collections.path, "/shop/collections");
    assert!(collections.localized);
    assert_eq!(collections.capability, None);
    assert_eq!(
        collections.response,
        HttpResponseContract::Page {
            template: "commerce/collections".to_string(),
            status: 200,
        }
    );

    let product_detail = manifest
        .http_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.product-detail")
        .expect("product detail http surface should exist");
    assert_eq!(product_detail.area, HttpSurfaceArea::Public);
    assert_eq!(product_detail.path, "/shop/products/{product_slug}");
    assert!(product_detail.localized);
    assert_eq!(product_detail.capability, None);
    assert_eq!(
        product_detail.response,
        HttpResponseContract::Page {
            template: "commerce/product-detail".to_string(),
            status: 200,
        }
    );

    let cart = manifest
        .http_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.cart")
        .expect("cart http surface should exist");
    assert_eq!(cart.area, HttpSurfaceArea::Public);
    assert_eq!(cart.path, "/cart");
    assert_eq!(cart.capability, None);
    assert_eq!(
        cart.response,
        HttpResponseContract::Page {
            template: "commerce/cart".to_string(),
            status: 200,
        }
    );

    let checkout = manifest
        .http_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.checkout")
        .expect("checkout http surface should exist");
    assert_eq!(checkout.area, HttpSurfaceArea::Public);
    assert_eq!(checkout.path, "/checkout");
    assert_eq!(checkout.capability, None);
    assert_eq!(
        checkout.response,
        HttpResponseContract::Page {
            template: "commerce/checkout".to_string(),
            status: 200,
        }
    );

    let add_to_cart = manifest
        .http_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.add-to-cart")
        .expect("add-to-cart http surface should exist");
    assert_eq!(add_to_cart.area, HttpSurfaceArea::Public);
    assert_eq!(add_to_cart.method, HttpSurfaceMethod::Post);
    assert_eq!(add_to_cart.path, "/cart/items");
    assert_eq!(add_to_cart.capability, None);
    assert_eq!(
        add_to_cart.response,
        HttpResponseContract::Redirect {
            location: "/cart".to_string(),
            status: 303,
        }
    );

    let cart_update = manifest
        .http_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.cart-update")
        .expect("cart update http surface should exist");
    assert_eq!(cart_update.area, HttpSurfaceArea::Public);
    assert_eq!(cart_update.method, HttpSurfaceMethod::Post);
    assert_eq!(cart_update.path, "/cart");
    assert_eq!(cart_update.capability, None);
    assert_eq!(
        cart_update.response,
        HttpResponseContract::Redirect {
            location: "/cart".to_string(),
            status: 303,
        }
    );

    let catalog_admin_update = manifest
        .http_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.catalog-admin-update")
        .expect("catalog admin update http surface should exist");
    assert_eq!(catalog_admin_update.area, HttpSurfaceArea::Admin);
    assert_eq!(catalog_admin_update.method, HttpSurfaceMethod::Post);
    assert_eq!(catalog_admin_update.path, "/admin/catalog/products");
    assert_eq!(
        catalog_admin_update.capability,
        Some(Capability::CatalogProductEdit)
    );
    assert_eq!(
        catalog_admin_update.response,
        HttpResponseContract::Redirect {
            location: "/admin/catalog/products".to_string(),
            status: 303,
        }
    );

    let order_detail = manifest
        .http_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.order-detail")
        .expect("order detail http surface should exist");
    assert_eq!(order_detail.area, HttpSurfaceArea::Admin);
    assert_eq!(order_detail.method, HttpSurfaceMethod::Get);
    assert_eq!(order_detail.path, "/admin/orders/{order_id}");
    assert_eq!(order_detail.capability, Some(Capability::OrderRead));
    assert_eq!(
        order_detail.response,
        HttpResponseContract::Page {
            template: "commerce/order-detail".to_string(),
            status: 200,
        }
    );

    let order_refund = manifest
        .http_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.order-refund")
        .expect("order refund http surface should exist");
    assert_eq!(order_refund.area, HttpSurfaceArea::Admin);
    assert_eq!(order_refund.method, HttpSurfaceMethod::Post);
    assert_eq!(order_refund.path, "/admin/orders/refund");
    assert_eq!(order_refund.capability, Some(Capability::OrderRefundIssue));
    assert_eq!(
        order_refund.response,
        HttpResponseContract::Redirect {
            location: "/admin/orders".to_string(),
            status: 303,
        }
    );

    let checkout_start = manifest
        .http_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.checkout-start")
        .expect("checkout start http surface should exist");
    assert_eq!(checkout_start.area, HttpSurfaceArea::Public);
    assert_eq!(checkout_start.method, HttpSurfaceMethod::Post);
    assert_eq!(checkout_start.path, "/checkout/start");
    assert_eq!(checkout_start.capability, None);
    assert_eq!(
        checkout_start.response,
        HttpResponseContract::Redirect {
            location: "/checkout".to_string(),
            status: 303,
        }
    );

    let checkout_complete = manifest
        .http_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.checkout-complete")
        .expect("checkout complete http surface should exist");
    assert_eq!(checkout_complete.area, HttpSurfaceArea::Public);
    assert_eq!(checkout_complete.method, HttpSurfaceMethod::Post);
    assert_eq!(checkout_complete.path, "/checkout/complete");
    assert_eq!(checkout_complete.capability, None);
    assert_eq!(
        checkout_complete.response,
        HttpResponseContract::Redirect {
            location: "/checkout/confirmation".to_string(),
            status: 303,
        }
    );

    let payment_webhook = manifest
        .http_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.payment-provider-webhook")
        .expect("payment webhook http surface should exist");
    assert_eq!(payment_webhook.area, HttpSurfaceArea::Api);
    assert_eq!(payment_webhook.method, HttpSurfaceMethod::Post);
    assert_eq!(payment_webhook.path, "/webhooks/commerce/payment-provider");
    assert_eq!(payment_webhook.capability, None);
    assert_eq!(
        payment_webhook.response,
        HttpResponseContract::Json {
            status: 200,
            payload: BTreeMap::from([("status".to_string(), "accepted".to_string())]),
        }
    );

    let checkout_confirmation = manifest
        .http_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.checkout-confirmation")
        .expect("checkout confirmation http surface should exist");
    assert_eq!(checkout_confirmation.area, HttpSurfaceArea::Public);
    assert_eq!(checkout_confirmation.path, "/checkout/confirmation");
    assert_eq!(checkout_confirmation.capability, None);
    assert_eq!(
        checkout_confirmation.response,
        HttpResponseContract::Page {
            template: "commerce/checkout-confirmation".to_string(),
            status: 200,
        }
    );

    let account_orders = manifest
        .http_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.account.orders")
        .expect("account orders http surface should exist");
    assert_eq!(account_orders.area, HttpSurfaceArea::Account);
    assert_eq!(account_orders.path, "/account/orders");
    assert_eq!(account_orders.capability, None);
    assert_eq!(
        account_orders.response,
        HttpResponseContract::Page {
            template: "account/orders".to_string(),
            status: 200,
        }
    );

    let account_session_end = manifest
        .http_surfaces
        .iter()
        .find(|surface| surface.name == "commerce.account-session-end")
        .expect("account session end http surface should exist");
    assert_eq!(account_session_end.area, HttpSurfaceArea::Account);
    assert_eq!(account_session_end.path, "/account/session/end");
    assert_eq!(account_session_end.capability, None);
    assert_eq!(
        account_session_end.response,
        HttpResponseContract::Redirect {
            location: "/account".to_string(),
            status: 303,
        }
    );
}

#[test]
fn commerce_module_public_action_surfaces_stay_in_lockstep_between_route_and_http_contracts() {
    let module = CommerceModule::new();
    let manifest = module.manifest();

    let expected = [
        ("commerce.add-to-cart", "/cart/items", "/cart"),
        ("commerce.cart-update", "/cart", "/cart"),
        ("commerce.checkout-start", "/checkout/start", "/checkout"),
        (
            "commerce.checkout-complete",
            "/checkout/complete",
            "/checkout/confirmation",
        ),
    ];

    for (name, path, redirect_to) in expected {
        let route_surface = manifest
            .route_surfaces
            .iter()
            .find(|surface| surface.name == name)
            .unwrap_or_else(|| panic!("route surface {name} should exist"));
        assert_eq!(route_surface.kind, RouteSurfaceKind::FrontendAction);
        assert_eq!(route_surface.path, path);
        assert_eq!(route_surface.capability, None);

        let http_surface = manifest
            .http_surfaces
            .iter()
            .find(|surface| surface.name == name)
            .unwrap_or_else(|| panic!("http surface {name} should exist"));
        assert_eq!(http_surface.method, HttpSurfaceMethod::Post);
        assert_eq!(http_surface.area, HttpSurfaceArea::Public);
        assert_eq!(http_surface.path, path);
        assert_eq!(http_surface.capability, None);
        assert_eq!(
            http_surface.response,
            HttpResponseContract::Redirect {
                location: redirect_to.to_string(),
                status: 303,
            }
        );
    }
}

#[test]
fn commerce_module_public_browse_pages_form_a_coherent_catalog_loop() {
    let module = CommerceModule::new();
    let manifest = module.manifest();

    let expected = [
        ("commerce.catalog", "/shop", "commerce/catalog", true),
        (
            "commerce.collections",
            "/shop/collections",
            "commerce/collections",
            true,
        ),
        (
            "commerce.collection-detail",
            "/shop/collections/{collection_slug}",
            "commerce/collection-detail",
            true,
        ),
        (
            "commerce.product-detail",
            "/shop/products/{product_slug}",
            "commerce/product-detail",
            true,
        ),
        ("commerce.cart", "/cart", "commerce/cart", false),
    ];

    for (name, path, template, localized) in expected {
        let route_surface = manifest
            .route_surfaces
            .iter()
            .find(|surface| surface.name == name)
            .unwrap_or_else(|| panic!("route surface {name} should exist"));
        assert_eq!(route_surface.kind, RouteSurfaceKind::FrontendPage);
        assert_eq!(route_surface.path, path);
        assert_eq!(route_surface.localized, localized);
        assert_eq!(route_surface.capability, None);

        let http_surface = manifest
            .http_surfaces
            .iter()
            .find(|surface| surface.name == name)
            .unwrap_or_else(|| panic!("http surface {name} should exist"));
        assert_eq!(http_surface.area, HttpSurfaceArea::Public);
        assert_eq!(http_surface.path, path);
        assert_eq!(http_surface.localized, localized);
        assert_eq!(http_surface.capability, None);
        assert_eq!(
            http_surface.response,
            HttpResponseContract::Page {
                template: template.to_string(),
                status: 200,
            }
        );
    }
}

#[test]
fn commerce_module_storefront_integration_point_describes_the_customer_browse_loop() {
    let module = CommerceModule::new();
    let manifest = module.manifest();

    let storefront_catalog = manifest
        .integration_points
        .iter()
        .find(|point| point.surface == "storefront.catalog")
        .expect("storefront catalog integration point should exist");

    assert!(
        storefront_catalog
            .description
            .contains("collection listing")
    );
    assert!(storefront_catalog.description.contains("collection detail"));
    assert!(storefront_catalog.description.contains("product detail"));
    assert!(storefront_catalog.description.contains("cart"));
    assert!(storefront_catalog.description.contains("checkout"));
    assert!(storefront_catalog.description.contains("confirmation"));
}

#[test]
fn catalog_tracks_products_and_sellable_checkout_lines() {
    let membership = membership_product();
    let shirt = tshirt_product();

    let mut catalog = Catalog::new();
    catalog.insert_product(membership.clone()).unwrap();
    catalog.insert_product(shirt.clone()).unwrap();

    let collection = CatalogCollection::new(
        CollectionId::new("featured").unwrap(),
        CollectionHandle::new("featured").unwrap(),
        "Featured",
    )
    .unwrap()
    .include_product(membership.id.clone())
    .include_product(shirt.id.clone());
    catalog.insert_collection(collection).unwrap();

    let resolved = catalog
        .collection_products(&CollectionId::new("featured").unwrap())
        .unwrap();
    assert_eq!(resolved.len(), 2);

    let line = membership
        .checkout_line(&Sku::new("membership-gold").unwrap(), 1)
        .unwrap();
    assert_eq!(line.product_title, "Gold Membership");

    let draft_product = CatalogProduct::new(
        ProductId::new("draft-product").unwrap(),
        ProductHandle::new("draft").unwrap(),
        "Draft Product",
        ProductKind::Digital,
    )
    .unwrap()
    .with_variant(ProductVariant::new(Sku::new("draft-sku").unwrap(), "Default", gbp(500)).unwrap())
    .unwrap();

    let err = draft_product
        .checkout_line(&Sku::new("draft-sku").unwrap(), 1)
        .unwrap_err();
    assert_eq!(
        err,
        CommerceModelError::ProductNotSellable {
            product_id: "draft-product".to_string(),
            status: ProductStatus::Draft,
        }
    );
}

#[test]
fn pricing_policy_applies_membership_discounts_shipping_vouchers_and_tax() {
    let membership = membership_product();
    let mut checkout = CheckoutSession::new(
        CheckoutId::new("chk-1").unwrap(),
        CurrencyCode::new("GBP").unwrap(),
    );
    checkout
        .add_line(
            membership
                .checkout_line(&Sku::new("membership-gold").unwrap(), 2)
                .unwrap(),
        )
        .unwrap();

    let pricing = PricingPolicy::new(CurrencyCode::new("GBP").unwrap())
        .with_membership_discount_basis_points(1_000)
        .unwrap()
        .with_fixed_discount(AdjustmentKind::Voucher, "Voucher", gbp(1_500))
        .unwrap()
        .with_shipping(gbp(500))
        .unwrap()
        .with_tax_rate_basis_points(2_000)
        .unwrap();

    let quote = checkout.price(&pricing).unwrap();
    assert_eq!(quote.subtotal.amount_minor(), 20_000);
    assert_eq!(quote.adjustments.len(), 4);
    assert_eq!(
        quote.adjustments[0].direction,
        AdjustmentDirection::Discount
    );
    assert_eq!(quote.adjustments[0].amount.amount_minor(), 2_000);
    assert_eq!(quote.adjustments[1].kind, AdjustmentKind::Voucher);
    assert_eq!(quote.adjustments[2].kind, AdjustmentKind::Shipping);
    assert_eq!(quote.adjustments[3].kind, AdjustmentKind::Tax);
    assert_eq!(quote.total.amount_minor(), 20_400);
}

#[test]
fn pricing_rejects_discounts_that_overdraw_the_total() {
    let quote = PriceQuote::new(
        gbp(500),
        vec![PriceAdjustment::discount(AdjustmentKind::Voucher, "Big voucher", gbp(600)).unwrap()],
    )
    .unwrap_err();

    assert_eq!(
        quote,
        CommerceModelError::TotalWouldBecomeNegative { total_minor: -100 }
    );
}

#[test]
fn membership_products_materialize_to_paid_orders_with_entitlement_outcomes() {
    let membership = membership_product();
    let pricing = PricingPolicy::new(CurrencyCode::new("GBP").unwrap());
    let mut checkout = CheckoutSession::new(
        CheckoutId::new("chk-2").unwrap(),
        CurrencyCode::new("GBP").unwrap(),
    );
    checkout
        .add_line(
            membership
                .checkout_line(&Sku::new("membership-gold").unwrap(), 1)
                .unwrap(),
        )
        .unwrap();

    checkout.ready_for_payment().unwrap();
    checkout.awaiting_payment().unwrap();
    checkout.mark_paid().unwrap();
    checkout.complete().unwrap();

    let order = checkout
        .to_order(OrderId::new("ord-100").unwrap(), &pricing)
        .unwrap();
    assert_eq!(order.status, OrderStatus::Paid);
    assert_eq!(
        order.outcomes(),
        vec![OrderOutcome::GrantMembership {
            entitlement_key: EntitlementKey::new("membership.gold").unwrap(),
            quantity: 1,
        }]
    );
}

#[test]
fn refunds_are_bounded_by_captured_total_and_drive_order_status() {
    let shirt = tshirt_product();
    let pricing = PricingPolicy::new(CurrencyCode::new("GBP").unwrap());
    let mut checkout = CheckoutSession::new(
        CheckoutId::new("chk-3").unwrap(),
        CurrencyCode::new("GBP").unwrap(),
    );
    checkout
        .add_line(
            shirt
                .checkout_line(&Sku::new("shirt-m").unwrap(), 2)
                .unwrap(),
        )
        .unwrap();
    checkout.ready_for_payment().unwrap();
    checkout.awaiting_payment().unwrap();
    checkout.mark_paid().unwrap();

    let mut order = checkout
        .to_order(OrderId::new("ord-200").unwrap(), &pricing)
        .unwrap();
    order.fulfill().unwrap();
    order
        .issue_refund(
            Refund::new(RefundId::new("refund-1").unwrap(), gbp(2_000), "partial").unwrap(),
        )
        .unwrap();
    assert_eq!(order.status, OrderStatus::PartiallyRefunded);

    let err = order
        .issue_refund(
            Refund::new(RefundId::new("refund-2").unwrap(), gbp(4_000), "too much").unwrap(),
        )
        .unwrap_err();
    assert_eq!(
        err,
        CommerceModelError::RefundExceedsCaptured {
            order_id: "ord-200".to_string(),
            captured_minor: 5_000,
            refunded_minor: 2_000,
            requested_minor: 4_000,
        }
    );

    order
        .issue_refund(Refund::new(RefundId::new("refund-3").unwrap(), gbp(3_000), "final").unwrap())
        .unwrap();
    assert_eq!(order.status, OrderStatus::Refunded);
}

#[test]
fn commerce_module_exposes_queries_migrations_and_transaction_plans() {
    let membership = membership_product();
    let shirt = tshirt_product();
    let mut catalog = Catalog::new();
    catalog.insert_product(membership.clone()).unwrap();
    catalog.insert_product(shirt.clone()).unwrap();

    let collection = CatalogCollection::new(
        CollectionId::new("featured").unwrap(),
        CollectionHandle::new("featured").unwrap(),
        "Featured",
    )
    .unwrap()
    .include_product(membership.id.clone())
    .include_product(shirt.id.clone());
    catalog.insert_collection(collection).unwrap();

    let listing = catalog
        .storefront_listing_query(
            Some("en-GB"),
            Some(&CollectionHandle::new("featured").unwrap()),
        )
        .unwrap();
    assert_eq!(
        listing.query.context.publication_visibility,
        PublicationVisibility::PublishedOnly
    );
    assert_eq!(
        listing.query.context.cache_scope,
        QueryCacheScope::LocaleScoped
    );
    assert_eq!(
        listing.query.filters[1].values,
        vec!["featured".to_string()]
    );

    let admin_listing = catalog
        .admin_catalog_query("user-42", Some("en-GB"))
        .unwrap();
    assert_eq!(
        admin_listing.query.context.publication_visibility,
        PublicationVisibility::IncludeDrafts
    );
    assert_eq!(
        admin_listing.query.context.principal_id.as_deref(),
        Some("user-42")
    );

    let pricing = PricingPolicy::new(CurrencyCode::new("GBP").unwrap());
    let mut checkout = CheckoutSession::new(
        CheckoutId::new("chk-ops").unwrap(),
        CurrencyCode::new("GBP").unwrap(),
    );
    checkout
        .add_line(
            membership
                .checkout_line(&Sku::new("membership-gold").unwrap(), 1)
                .unwrap(),
        )
        .unwrap();
    checkout.ready_for_payment().unwrap();
    checkout.awaiting_payment().unwrap();
    checkout.mark_paid().unwrap();
    checkout.complete().unwrap();

    let order = checkout
        .to_order(OrderId::new("ord-ops").unwrap(), &pricing)
        .unwrap();
    let completion = checkout.completion_transaction_plan(&order).unwrap();
    assert_eq!(completion.isolation, TransactionIsolation::Serializable);
    assert_eq!(completion.writes.len(), 4);
    assert!(
        completion
            .after_commit_events
            .iter()
            .any(|event| event == "commerce.order.created:ord-ops")
    );

    let fulfillment = order.fulfillment_transaction_plan().unwrap();
    assert_eq!(fulfillment.writes[0].resource, "order");

    let refund = Refund::new(RefundId::new("refund-ops").unwrap(), gbp(500), "partial").unwrap();
    let refund_plan = order.refund_transaction_plan(&refund).unwrap();
    assert_eq!(refund_plan.writes[0].resource, "order_refund");
    assert!(
        refund_plan
            .after_commit_jobs
            .iter()
            .any(|job| job == "commerce.jobs.refund.reconcile:refund-ops")
    );

    let module = CommerceModule::new();
    let migrations = module.migration_plan().unwrap();
    assert_eq!(migrations.ordered_steps().len(), 4);
    assert_eq!(
        migrations.ordered_steps()[0].owner,
        MigrationOwner::Module("commerce".to_string())
    );
    assert!(
        migrations
            .ordered_steps()
            .iter()
            .all(|step| !step.statements.is_empty())
    );
}
