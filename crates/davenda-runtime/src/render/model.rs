use super::*;
use crate::builder::RegisteredHookKind;
use crate::storefront::{
    StorefrontCartLine, StorefrontFormState, StorefrontOrderSnapshot, StorefrontPaymentSnapshot,
    StorefrontStateSnapshot, StorefrontStateStore,
};
use davenda_assets::AssetDeliveryTarget;
use davenda_commerce::{
    CheckoutId, CheckoutLine, CheckoutSession, CurrencyCode, EntitlementKey, Money, Order, OrderId,
    PricingPolicy, ProductId, ProductKind, Sku,
};
use davenda_customer_sdk::{
    AuditEntry, AuditFacade, AuthCheckRequest, AuthCheckResult, AuthExplainRequest,
    AuthExplanation, AuthFacade, BackendError, BackendErrorKind, CommerceFacade,
    CustomerAppContext as CustomerPluginAppContext, MoneyAmount, OrderDraft, OrderLineDraft,
    OrderReviewDecision, PrincipalContext as CustomerPluginPrincipalContext,
    PrincipalKind as CustomerPluginPrincipalKind, RequestContext as CustomerPluginRequestContext,
    TraceContext as CustomerPluginTraceContext,
};
use davenda_memberships::{
    BillingInterval, MemberAccountId, MembershipCatalog, MembershipInstant, MembershipModelError,
    MembershipTier, MembershipTierId, SubscriptionStatus, TierVisibility,
};
use davenda_template::{
    RenderModel, RenderValue, TemplateModelError, TemplateNamespace, TrustedHtml,
};
use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use url::form_urlencoded;

struct RuntimeCustomerCommerceFacade<'a> {
    catalog: &'a StorefrontCatalog,
    order_id: &'a str,
    review_notes: Arc<Mutex<Vec<String>>>,
}

impl CommerceFacade for RuntimeCustomerCommerceFacade<'_> {
    fn product(
        &self,
        sku: &str,
    ) -> Result<Option<davenda_customer_sdk::CommerceProduct>, BackendError> {
        Ok(self.catalog.product_by_sku_or_handle(sku).map(|product| {
            davenda_customer_sdk::CommerceProduct {
                sku: product.sku.clone(),
                handle: product.handle.clone(),
                title: product.title.clone(),
                current_price: MoneyAmount::new(product.currency.clone(), product.price_minor),
                collection_handle: Some(product.collection_handle.clone()),
                metadata: BTreeMap::new(),
            }
        }))
    }

    fn add_order_note(&self, order_id: &str, note: &str) -> Result<(), BackendError> {
        if order_id != self.order_id {
            return Err(BackendError::new(
                BackendErrorKind::InvalidInput,
                "commerce.order_note.order_mismatch",
                format!(
                    "Render-time customer review can only annotate the active order `{}`; received `{order_id}`.",
                    self.order_id
                ),
            ));
        }
        let note = note.trim();
        if note.is_empty() {
            return Err(BackendError::new(
                BackendErrorKind::InvalidInput,
                "commerce.order_note.empty",
                "Render-time customer review requires a non-empty order note.",
            ));
        }
        let mut notes = self.review_notes.lock().map_err(|_| {
            BackendError::new(
                BackendErrorKind::Internal,
                "commerce.order_note.state_poisoned",
                "Render-time customer review could not record the order note.",
            )
        })?;
        if !notes.iter().any(|existing| existing == note) {
            notes.push(note.to_string());
        }
        Ok(())
    }
}

struct RuntimeCustomerAuthFacade<'a> {
    plan: &'a RuntimePlan,
    principal_id: Option<&'a str>,
}

impl AuthFacade for RuntimeCustomerAuthFacade<'_> {
    fn check_capability(
        &self,
        request: &AuthCheckRequest,
    ) -> Result<AuthCheckResult, BackendError> {
        let capability = parse_customer_capability(request.capability.as_str())?;
        let object = parse_customer_auth_entity(request.object.as_str())?;
        let subject = customer_hook_auth_subject(self.principal_id);
        let data = self.plan.data.clone();
        let tenant_id = self.plan.tenant_id();
        let auth_package = self.plan.auth_package.clone();
        let allowed = run_customer_hook_future(async move {
            let client = data
                .connect_lazy_postgres()
                .map_err(|error| error.to_string())?;
            let engine = zanzibar::postgres::PostgresRebacEngine::new(client.pool.clone());
            let auth = davenda_auth::DavendaAuth::new(engine, tenant_id);
            auth.check_capability(auth_package.package(), &subject, capability, &object)
                .await
                .map_err(|error| error.to_string())
        })
        .map_err(customer_hook_auth_backend_error)?;

        Ok(AuthCheckResult {
            allowed,
            explanation: (!allowed).then(|| {
                format!(
                    "live auth denied `{}` for `{}`",
                    request.capability, request.object
                )
            }),
        })
    }

    fn explain_denial(
        &self,
        request: &AuthExplainRequest,
    ) -> Result<AuthExplanation, BackendError> {
        if !self.plan.config.auth.explain_api {
            return Err(BackendError::new(
                BackendErrorKind::Unsupported,
                "auth.explain.unavailable",
                "Runtime auth explanations are disabled for this installation.",
            ));
        }
        let capability = parse_customer_capability(request.capability.as_str())?;
        let object = parse_customer_auth_entity(request.object.as_str())?;
        let subject = customer_hook_auth_subject(self.principal_id);
        let config = self.plan.config.clone();
        let data = self.plan.data.clone();
        let auth_package = self.plan.auth_package.clone();
        let explanation = run_customer_hook_future(async move {
            let explainer =
                davenda_auth::LiveAuthExplainHost::from_runtime(&config, data, auth_package)
                    .map_err(|error| error.to_string())?;
            explainer
                .explain_capability(&davenda_auth::LiveAuthExplainRequest {
                    subject,
                    capability,
                    object,
                    options: davenda_auth::ExplainOptions::default(),
                })
                .await
                .map_err(|error| error.to_string())
        })
        .map_err(customer_hook_auth_backend_error)?;

        Ok(AuthExplanation {
            summary: format!(
                "{} `{}` on `{}`",
                if explanation.decision.is_allowed() {
                    "allow"
                } else {
                    "deny"
                },
                explanation.capability.as_str(),
                explanation.object
            ),
            traces: vec![format!("{:?}", explanation.trace)],
        })
    }
}

struct RuntimeCustomerAuditFacade<'a> {
    plan: &'a RuntimePlan,
    principal_id: Option<&'a str>,
}

impl AuditFacade for RuntimeCustomerAuditFacade<'_> {
    fn record(&self, entry: AuditEntry) -> Result<(), BackendError> {
        let mut serializer = form_urlencoded::Serializer::new(String::new());
        serializer
            .append_pair("action", entry.action.as_str())
            .append_pair("resource_kind", entry.resource_kind.as_str())
            .append_pair("resource_id", entry.resource_id.as_str())
            .append_pair("outcome", entry.outcome.as_str());
        if let Some(detail) = entry.detail.as_deref() {
            serializer.append_pair("detail", detail);
        }
        for (key, value) in &entry.metadata {
            serializer.append_pair(&format!("meta.{key}"), value);
        }
        record_admin_audit_entry(
            self.plan,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
            self.principal_id.unwrap_or("anonymous"),
            serializer.finish(),
        )
        .map_err(|reason| {
            BackendError::new(
                BackendErrorKind::Internal,
                "audit.record.failed",
                "Failed to persist the customer hook audit entry during render.",
            )
            .with_detail(reason)
        })
    }
}

impl RuntimePlan {
    pub(super) fn template_namespaces_for_execution(
        &self,
        execution: &RequestExecution,
    ) -> Vec<TemplateNamespace> {
        let module_namespace = self.module_template_namespace(execution);
        self.template.namespace_chain(module_namespace.as_ref())
    }

    pub(super) fn module_template_namespace(
        &self,
        execution: &RequestExecution,
    ) -> Option<TemplateNamespace> {
        self.http
            .routes
            .iter()
            .find(|route| route.name == execution.route.route_name)
            .and_then(|route| route.module.as_deref())
            .and_then(|module| TemplateNamespace::new(module.to_string()).ok())
    }

    pub(super) fn render_model_for_execution(
        &self,
        execution: &RequestExecution,
        template_name: &str,
        fragment_id: Option<&str>,
    ) -> Result<RenderModel, TemplateModelError> {
        let storefront_feedback = storefront_page_feedback(
            execution.route.route_name.as_str(),
            &execution.flash_messages,
        );
        let mut model = RenderModel::new()
            .with_value(
                "customer_app",
                RenderValue::text(execution.customer_app.clone()),
            )?
            .with_value(
                "route_name",
                RenderValue::text(execution.route.route_name.clone()),
            )?
            .with_value("path", RenderValue::text(execution.path.clone()))?
            .with_value("locale", RenderValue::text(execution.locale.clone()))?
            .with_value(
                "method",
                RenderValue::text(format!("{:?}", execution.method)),
            )?
            .with_value(
                "template_name",
                RenderValue::text(template_name.to_string()),
            )?
            .with_value(
                "route_area",
                RenderValue::text(format!("{:?}", execution.route_area)),
            )?
            .with_value(
                "request_id",
                RenderValue::text(execution.trace.request_id.clone()),
            )?
            .with_value(
                "transport_scheme",
                RenderValue::text(execution.trace.transport_scheme.clone()),
            )?
            .with_value(
                "principal_id",
                RenderValue::text(
                    execution
                        .principal
                        .principal_id
                        .clone()
                        .unwrap_or_else(|| "anonymous".to_string()),
                ),
            )?
            .with_value(
                "session_id",
                RenderValue::text(
                    execution
                        .session
                        .session_id
                        .clone()
                        .unwrap_or_else(|| "guest".to_string()),
                ),
            )?
            .with_value(
                "surface_id",
                RenderValue::text(
                    fragment_id
                        .map(str::to_string)
                        .unwrap_or_else(|| execution.route.route_name.clone()),
                ),
            )?
            .with_object("route_params", route_params_model(&execution.route.params))?
            .with_object("links", links_model(&execution.locale)?)?
            .with_object("navigation", navigation_model(Some(self))?)?
            .with_bool(
                "hasFlashMessages",
                !storefront_feedback.visible_flash_messages.is_empty(),
            )?
            .with_list(
                "flashMessages",
                flash_messages_model(&storefront_feedback.visible_flash_messages)?,
            )?
            .with_object(
                "page",
                page_model_for_route(execution, template_name, fragment_id),
            )?
            .with_bool(
                "hasLinkedCustomerPlugins",
                !self.linked_customer_plugins.is_empty(),
            )?
            .with_list(
                "linkedCustomerPlugins",
                linked_customer_plugins_model(&self.linked_customer_plugins)?,
            )?;

        if let Some(fragment_id) = fragment_id {
            model = model.with_value("fragment_id", RenderValue::text(fragment_id.to_string()))?;
        }

        if let Some(manifest) = &self.theme_asset_manifest {
            for (logical_path, published) in manifest.entries() {
                if let AssetDeliveryTarget::Cdn { public_url, .. } = published.delivery().target() {
                    model = model.with_asset_path(logical_path, public_url.clone())?;
                }
            }
        }

        apply_route_specific_bindings(
            Some(self),
            model,
            execution.route.route_name.as_str(),
            execution.locale.as_str(),
            &execution.route.params,
            &execution.query_params,
            storefront_feedback.form_state.as_ref(),
            Some(&execution.session),
            Some(&execution.principal),
        )
    }
}

fn linked_customer_plugins_model(
    plugins: &[crate::builder::LinkedCustomerPluginSummary],
) -> Result<Vec<RenderModel>, TemplateModelError> {
    plugins
        .iter()
        .map(|plugin| {
            RenderModel::new()
                .with_value("id", RenderValue::text(plugin.plugin_id.clone()))?
                .with_value(
                    "displayName",
                    RenderValue::text(plugin.display_name.clone()),
                )?
                .with_value("version", RenderValue::text(plugin.version.clone()))?
                .with_value(
                    "hooksSummary",
                    RenderValue::text(
                        plugin
                            .registered_hooks
                            .iter()
                            .map(registered_hook_label)
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                )
        })
        .collect()
}

fn registered_hook_label(kind: &RegisteredHookKind) -> &'static str {
    match kind {
        RegisteredHookKind::Checkout => "checkout",
        RegisteredHookKind::CmsPagePublish => "cms-page-publish",
        RegisteredHookKind::VerifiedWebhook => "verified-webhook",
    }
}

fn route_params_model(params: &BTreeMap<String, String>) -> RenderModel {
    let mut model = RenderModel::new();
    for (key, value) in params {
        model = model
            .with_value(key.clone(), RenderValue::text(value.clone()))
            .expect("route params are validated tokens");
    }
    model
}

fn navigation_model(plan: Option<&RuntimePlan>) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new().with_list("primary", primary_navigation_items(plan)?)
}

fn nav_item(label: &str, href: &str) -> RenderModel {
    RenderModel::new()
        .with_value("label", RenderValue::text(label))
        .and_then(|model| model.with_value("href", RenderValue::text(href)))
        .expect("navigation item keys are valid")
}

fn primary_navigation_items(
    plan: Option<&RuntimePlan>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    let items = if let Some(plan) = plan {
        cms_admin_workspace(plan)?
            .navigation
            .into_iter()
            .map(|item| nav_item(item.label.as_str(), item.href.as_str()))
            .collect::<Vec<_>>()
    } else {
        vec![
            nav_item("Home", "/"),
            nav_item("Shop", "/shop"),
            nav_item("Collections", "/shop/collections"),
            nav_item("Events", "/events"),
            nav_item("Cart", "/cart"),
            nav_item("Account", "/account"),
        ]
    };
    Ok(items)
}

fn links_model(locale: &str) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("home", RenderValue::text("/"))?
        .with_value("catalog", RenderValue::text(localized_shop_path(locale)))?
        .with_value(
            "collections",
            RenderValue::text(localized_collections_path(locale)),
        )?
        .with_value(
            "featuredCollection",
            RenderValue::text(localized_collection_path(locale, "featured")),
        )?
        .with_value(
            "membershipsCollection",
            RenderValue::text(localized_collection_path(locale, "memberships")),
        )?
        .with_value(
            "eventsCollection",
            RenderValue::text(localized_collection_path(locale, "events")),
        )?
        .with_value("cart", RenderValue::text("/cart"))?
        .with_value("checkout", RenderValue::text("/checkout"))?
        .with_value("account", RenderValue::text("/account"))?
        .with_value("orders", RenderValue::text("/account/orders"))?
        .with_value("memberships", RenderValue::text("/account/memberships"))?
        .with_value("adminDashboard", RenderValue::text("/admin"))?
        .with_value("adminAudit", RenderValue::text("/admin/audit"))?
        .with_value("adminOrders", RenderValue::text("/admin/orders"))?
        .with_value("adminCatalog", RenderValue::text("/admin/catalog/products"))?
        .with_value("adminPages", RenderValue::text("/admin/pages"))?
        .with_value("adminNavigation", RenderValue::text("/admin/navigation"))?
        .with_value("adminRedirects", RenderValue::text("/admin/redirects"))
}

fn page_model_for_route(
    execution: &RequestExecution,
    template_name: &str,
    fragment_id: Option<&str>,
) -> RenderModel {
    let title = match execution.route.route_name.as_str() {
        "home" => "Harbor Shop".to_string(),
        "commerce.catalog" => "Shop Harbor".to_string(),
        "commerce.collections" => "Shop Collections".to_string(),
        "commerce.collection-detail" => execution
            .route
            .params
            .get("collection_slug")
            .map(|slug| title_case_handle(slug))
            .unwrap_or_else(|| "Collection".to_string()),
        "commerce.product-detail" => execution
            .route
            .params
            .get("product_slug")
            .map(|slug| title_case_handle(slug))
            .unwrap_or_else(|| "Product".to_string()),
        "commerce.cart" => "Cart".to_string(),
        "commerce.checkout" => "Checkout".to_string(),
        "commerce.checkout-confirmation" => "Order Confirmed".to_string(),
        "commerce.account.orders" => "Order History".to_string(),
        "admin.dashboard" => "Harbor Shop Admin".to_string(),
        "admin.audit" => "Audit Log".to_string(),
        "commerce.orders" => "Orders".to_string(),
        "commerce.order-detail" => execution
            .route
            .params
            .get("order_id")
            .map(|order_id| format!("Order {order_id}"))
            .unwrap_or_else(|| "Order Detail".to_string()),
        "commerce.catalog-admin" => "Catalog Administration".to_string(),
        "cms.pages.index" => "Pages".to_string(),
        "cms.navigation.index" => "Navigation".to_string(),
        "cms.redirects.index" => "Redirects".to_string(),
        "memberships.account" | "memberships.account.dashboard" | "account.dashboard" => {
            "Your Account".to_string()
        }
        _ => execution.route.route_name.clone(),
    };

    let summary = match execution.route.route_name.as_str() {
        "commerce.catalog" => {
            "Browse the current assortment across apparel, memberships, and event-linked offers."
        }
        "commerce.collections" => {
            "Browse curated collection landings before moving into product detail and checkout."
        }
        "commerce.collection-detail" => {
            "A merchandising collection page with clear paths into products and checkout."
        }
        "commerce.product-detail" => {
            "Product detail, pricing, and purchase intent in the HTML-first storefront flow."
        }
        "commerce.cart" => "Review the basket before moving into checkout.",
        "commerce.checkout" => {
            "Confirm contact, delivery, and payment details before finalization."
        }
        "commerce.checkout-confirmation" => {
            "The customer-facing confirmation step after successful checkout."
        }
        "commerce.account.orders" => {
            "Review completed purchases, payment details, and membership-linked order history."
        }
        "admin.dashboard" => {
            "Operator dashboard for launch-day catalog, orders, and content checks."
        }
        "admin.audit" => {
            "Recent privileged actions, the acting operator, and the affected resources."
        }
        "commerce.orders" => {
            "Store-wide order queue with payment state, customer email, and refund visibility."
        }
        "commerce.order-detail" => {
            "Support and finance detail for a specific order, including payment, customer, and refund state."
        }
        "commerce.catalog-admin" => {
            "Live catalog copy, storefront visibility, list price, and collection management for the checked-in store."
        }
        "cms.pages.index" => "Draft, preview, publish, and unpublish live CMS pages.",
        "cms.navigation.index" => {
            "Edit the live primary navigation links rendered in the storefront shell."
        }
        "cms.redirects.index" => {
            "Manage live redirect rules for legacy and unmatched storefront routes."
        }
        "memberships.account" | "memberships.account.dashboard" | "account.dashboard" => {
            "Membership state, recent orders, and next actions for the signed-in customer."
        }
        _ => "Server-rendered storefront and account surface.",
    };

    RenderModel::new()
        .with_value("title", RenderValue::text(title))
        .and_then(|model| model.with_value("summary", RenderValue::text(summary)))
        .and_then(|model| model.with_value("template", RenderValue::text(template_name)))
        .and_then(|model| {
            model.with_value("fragment_mode", RenderValue::bool(fragment_id.is_some()))
        })
        .expect("page model keys are valid")
}

fn apply_route_specific_bindings(
    plan: Option<&RuntimePlan>,
    mut model: RenderModel,
    route_name: &str,
    locale: &str,
    params: &BTreeMap<String, String>,
    query_params: &RequestFieldMap,
    form_state: Option<&StorefrontFormState>,
    session: Option<&SessionContext>,
    principal: Option<&PrincipalContext>,
) -> Result<RenderModel, TemplateModelError> {
    let effective_catalog = effective_storefront_catalog(plan)?;
    let catalog = &effective_catalog;
    let fixture = storefront_fixture(locale, catalog, plan)?;

    match route_name {
        "home" | "commerce.catalog" | "commerce.collections" => {
            let has_catalog_sections = !fixture.catalog_sections.is_empty();
            let has_product_cards = !fixture.product_cards.is_empty();
            model = model
                .with_bool("hasCatalogSections", has_catalog_sections)?
                .with_bool("hasProductCards", has_product_cards)?
                .with_list("catalogSections", fixture.catalog_sections.clone())?
                .with_list("productCards", fixture.product_cards.clone())?;
        }
        "commerce.collection-detail" => {
            let slug = params
                .get("collection_slug")
                .map(String::as_str)
                .unwrap_or("featured");
            if let Some(_collection) = catalog.visible_collection(slug) {
                let product_cards = fixture.product_cards_for_collection(slug);
                model = model
                    .with_bool("hasCollection", true)?
                    .with_object("collection", fixture.collection_for(slug))?
                    .with_bool("hasProductCards", !product_cards.is_empty())?
                    .with_list("productCards", product_cards)?;
            } else {
                model = model
                    .with_bool("hasCollection", false)?
                    .with_bool("hasProductCards", false)?
                    .with_list("productCards", Vec::<RenderModel>::new())?
                    .with_value(
                        "missingCollectionHandle",
                        RenderValue::text(slug.to_string()),
                    )?;
            }
        }
        "commerce.product-detail" => {
            let slug = params
                .get("product_slug")
                .map(String::as_str)
                .unwrap_or("harbor-cap");
            if catalog.visible_product(slug).is_some() {
                let product_cards = fixture.related_product_cards_for_product(slug);
                model = model
                    .with_bool("hasProduct", true)?
                    .with_object("product", fixture.product_for(slug))?
                    .with_bool("hasProductCards", !product_cards.is_empty())?
                    .with_list("productCards", product_cards)?;
            } else {
                model = model
                    .with_bool("hasProduct", false)?
                    .with_bool("hasProductCards", false)?
                    .with_list("productCards", Vec::<RenderModel>::new())?
                    .with_value("missingProductHandle", RenderValue::text(slug.to_string()))?;
            }
        }
        "commerce.cart" => {
            if let Some(snapshot) = live_storefront_state(plan, session, principal)? {
                model = model
                    .with_bool("hasCartItems", !snapshot.cart.lines.is_empty())?
                    .with_list(
                        "cartItems",
                        cart_items_from_storefront(
                            catalog,
                            locale,
                            &snapshot.cart.lines,
                            form_state,
                        )?,
                    )?
                    .with_object("cartSummary", cart_summary_from_storefront(&snapshot)?)?
                    .with_object("cartForm", cart_form_model(form_state)?)?;
            } else {
                model = model
                    .with_bool("hasCartItems", !fixture.cart_items.is_empty())?
                    .with_list("cartItems", fixture.cart_items.clone())?
                    .with_object("cartSummary", fixture.cart_summary.clone())?
                    .with_object("cartForm", cart_form_model(form_state)?)?;
            }
        }
        "commerce.checkout" => {
            if let Some(snapshot) = live_storefront_state(plan, session, principal)? {
                let line_items =
                    cart_items_from_storefront(catalog, locale, &snapshot.cart.lines, form_state)?;
                model = model
                    .with_object("customer", checkout_customer(principal)?)?
                    .with_object(
                        "checkout",
                        checkout_form_from_storefront(
                            plan,
                            &snapshot.payment,
                            principal,
                            form_state,
                        )?,
                    )?
                    .with_bool("hasLineItems", !line_items.is_empty())?
                    .with_list("lineItems", line_items)?
                    .with_object("orderSummary", cart_summary_from_storefront(&snapshot)?)?;
            } else {
                let checkout = merge_checkout_form_feedback(fixture.checkout.clone(), form_state)?;
                model = model
                    .with_object("customer", fixture.customer.clone())?
                    .with_object("checkout", checkout)?
                    .with_bool("hasLineItems", !fixture.cart_items.is_empty())?
                    .with_list("lineItems", fixture.cart_items.clone())?
                    .with_object("orderSummary", fixture.cart_summary.clone())?;
            }
        }
        "commerce.checkout-confirmation" => {
            let account =
                account_surface_bindings(plan, &fixture, locale, session, principal, true)?;
            if let Some(snapshot) = live_storefront_state(plan, session, principal)? {
                let confirmation = snapshot
                    .latest_order
                    .as_ref()
                    .map(|order| confirmation_from_storefront(plan, order))
                    .transpose()?
                    .unwrap_or(empty_confirmation_model(plan)?);
                if snapshot.latest_order.is_some() {
                    model = model
                        .with_bool("hasConfirmation", true)?
                        .with_object("confirmation", confirmation)?
                        .with_object("account", account.account)?
                        .with_object("customer", account.customer)?
                        .with_list("recentOrders", account.recent_orders)?
                        .with_object("membershipSummary", account.membership_summary)?;
                } else {
                    model = model
                        .with_bool("hasConfirmation", false)?
                        .with_object("confirmation", confirmation)?
                        .with_object("account", account.account)?
                        .with_object("customer", account.customer)?
                        .with_list("recentOrders", account.recent_orders)?
                        .with_object("membershipSummary", account.membership_summary)?;
                }
            } else {
                model = model
                    .with_bool("hasConfirmation", true)?
                    .with_object("confirmation", fixture.confirmation.clone())?
                    .with_object("account", account.account)?
                    .with_object("customer", account.customer)?
                    .with_list("recentOrders", account.recent_orders)?
                    .with_object("membershipSummary", account.membership_summary)?;
            }
        }
        "commerce.account.orders"
        | "memberships.account"
        | "memberships.account.dashboard"
        | "account.dashboard" => {
            let include_pending_membership = route_name == "memberships.account";
            let account = account_surface_bindings(
                plan,
                &fixture,
                locale,
                session,
                principal,
                include_pending_membership,
            )?;
            model = model
                .with_object("account", account.account)?
                .with_object("customer", account.customer)?
                .with_list("recentOrders", account.recent_orders)?
                .with_object("membershipSummary", account.membership_summary)?;
        }
        "admin.dashboard" => {
            let live_recent_orders = recent_orders_from_storefront(
                live_storefront_state(plan, session, principal)?.as_ref(),
            )?;
            let order_count = if live_recent_orders.is_empty() {
                fixture.recent_orders.len()
            } else {
                live_recent_orders.len()
            };
            let content_count = if let Some(plan) = plan {
                cms_admin_workspace(plan)?.pages.len().to_string()
            } else {
                content_pages(locale)?.len().to_string()
            };
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("hasAdminPanels", true)?
                .with_list("adminPanels", admin_panels(locale, &fixture, order_count)?)?
                .with_value(
                    "catalogCount",
                    RenderValue::text(fixture.product_cards.len().to_string()),
                )?
                .with_value("orderCount", RenderValue::text(order_count.to_string()))?
                .with_value("contentCount", RenderValue::text(content_count))?;
        }
        "admin.audit" => {
            let audit_history = admin_audit_history(plan)?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("hasAuditEntries", !audit_history.entries.is_empty())?
                .with_list("auditEntries", audit_history.entries)?
                .with_value(
                    "auditEmptyText",
                    RenderValue::text(audit_history.empty_text),
                )?
                .with_value("auditBackend", RenderValue::text(audit_history.backend))?
                .with_value("auditLocation", RenderValue::text(audit_history.location))?
                .with_value(
                    "auditEntryCount",
                    RenderValue::text(audit_history.entry_count.to_string()),
                )?;
        }
        "commerce.orders" => {
            let (recent_orders, order_stats) = admin_orders_from_storefront(plan, &fixture)?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("hasRecentOrders", !recent_orders.is_empty())?
                .with_list("recentOrders", recent_orders)?
                .with_object("orderStats", order_stats)?
                .with_value(
                    "ordersEmptyText",
                    RenderValue::text(
                        "No completed orders have been captured in the checked-in sample app yet.",
                    ),
                )?;
        }
        "commerce.order-detail" => {
            let (recent_orders, _) = admin_orders_from_storefront(plan, &fixture)?;
            let selected_order = params
                .get("order_id")
                .and_then(|order_id| {
                    order_detail_from_storefront(plan, order_id, form_state, session, principal)
                        .transpose()
                })
                .transpose()?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("hasRecentOrders", !recent_orders.is_empty())?
                .with_list("recentOrders", recent_orders)?
                .with_bool("hasSelectedOrder", selected_order.is_some())?
                .with_bool(
                    "hasMissingOrder",
                    params.get("order_id").is_some() && selected_order.is_none(),
                )?;
            if let Some(order) = selected_order {
                model = model.with_object("selectedOrder", order)?;
            }
            model = merge_order_refund_form_feedback(model, form_state)?;
        }
        "commerce.catalog-admin" => {
            let product_cards = catalog_admin_products_model(locale, catalog, plan, form_state)?;
            let catalog_sections = catalog_admin_collections_model(locale, catalog, form_state)?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_object("catalogAdminForm", catalog_admin_form_model(form_state)?)?
                .with_bool("hasCatalogSections", !catalog_sections.is_empty())?
                .with_list("catalogSections", catalog_sections)?
                .with_bool("hasProductCards", !product_cards.is_empty())?
                .with_list("productCards", product_cards)?
                .with_value(
                    "catalogEmptyText",
                    RenderValue::text("No catalog entries are available in the sample app yet."),
                )?;
        }
        "cms.pages.index" => {
            let workspace = plan
                .map(cms_admin_workspace)
                .transpose()?
                .unwrap_or_else(default_cms_admin_workspace);
            let pages = cms_admin_pages_model(&workspace)?;
            let is_creating_page =
                query_flag(query_params, "new") || cms_admin_form_targets_new_page(form_state);
            let selected_page = (!is_creating_page)
                .then(|| {
                    workspace
                        .selected_page(query_first(query_params, "page"))
                        .cloned()
                })
                .flatten();
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("hasContentPages", !pages.is_empty())?
                .with_list("contentPages", pages)?
                .with_bool(
                    "hasSelectedContentPage",
                    selected_page.is_some() || is_creating_page,
                )?
                .with_bool("isCreatingContentPage", is_creating_page)?
                .with_bool("hasPersistedContentPage", selected_page.is_some())?
                .with_object(
                    "selectedContentPage",
                    cms_admin_selected_page_model_with_form_state(selected_page, form_state)?,
                )?
                .with_value(
                    "newContentPageHref",
                    RenderValue::text("/admin/pages?new=1"),
                )?
                .with_value(
                    "contentPageEditorTitle",
                    RenderValue::text(if is_creating_page {
                        "New page draft"
                    } else {
                        "Draft workflow"
                    }),
                )?
                .with_value(
                    "contentPageEditorSummary",
                    RenderValue::text(if is_creating_page {
                        "Start a new draft page, save it to create a stable preview, then publish it when the route is ready."
                    } else {
                        "Update the draft, review the preview, then publish it to the live route."
                    }),
                )?
                .with_value(
                    "contentPageSaveLabel",
                    RenderValue::text(if is_creating_page {
                        "Create draft"
                    } else {
                        "Save draft"
                    }),
                )?
                .with_bool(
                    "showPagesEmptyState",
                    workspace.pages.is_empty() && !is_creating_page,
                )?
                .with_value(
                    "pagesEmptyText",
                    RenderValue::text(
                        "Create or update a draft page, preview it below, then publish it to the live /pages/{slug} route.",
                    ),
                )?;
            model = merge_cms_page_form_feedback(model, form_state)?;
        }
        "cms.preview" => {
            let workspace = plan
                .map(cms_admin_workspace)
                .transpose()?
                .unwrap_or_else(default_cms_admin_workspace);
            let selected_page = workspace
                .selected_page(query_first(query_params, "page"))
                .cloned();
            model = model.with_object(
                "selectedContentPage",
                cms_admin_selected_page_model_with_form_state(selected_page, form_state)?,
            )?;
        }
        "cms.navigation.index" => {
            let workspace = plan
                .map(cms_admin_workspace)
                .transpose()?
                .unwrap_or_else(default_cms_admin_workspace);
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("hasNavigationItems", !workspace.navigation.is_empty())?
                .with_list(
                    "navigationItems",
                    cms_navigation_items_model(&workspace.navigation)?,
                )?
                .with_value(
                    "navigationEmptyText",
                    RenderValue::text("Add at least one primary navigation item before saving."),
                )?;
            model = merge_cms_navigation_form_feedback(model, form_state)?;
        }
        "cms.redirects.index" => {
            let workspace = plan
                .map(cms_admin_workspace)
                .transpose()?
                .unwrap_or_else(default_cms_admin_workspace);
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("hasRedirects", !workspace.redirects.is_empty())?
                .with_list("redirects", cms_redirects_model(&workspace.redirects)?)?
                .with_value(
                    "redirectsEmptyText",
                    RenderValue::text(
                        "Add redirect rules for unmatched legacy URLs before cutover.",
                    ),
                )?;
            model = merge_cms_redirect_form_feedback(model, form_state)?;
        }
        "cms.page" => {
            let workspace = plan
                .map(cms_admin_workspace)
                .transpose()?
                .unwrap_or_else(default_cms_admin_workspace);
            let slug = params.get("slug").map(String::as_str).unwrap_or_default();
            model = model.with_object("cmsPage", cms_live_page_model(&workspace, slug)?)?;
        }
        _ => {}
    }

    Ok(model)
}

fn effective_storefront_catalog(
    plan: Option<&RuntimePlan>,
) -> Result<StorefrontCatalog, TemplateModelError> {
    let Some(plan) = plan else {
        return Ok(StorefrontCatalog::default_sample());
    };
    StorefrontStateStore::open_for_plan(plan)
        .map_err(template_store_error)?
        .catalog()
        .map_err(template_store_error)
}

fn live_storefront_state(
    plan: Option<&RuntimePlan>,
    session: Option<&SessionContext>,
    principal: Option<&PrincipalContext>,
) -> Result<Option<StorefrontStateSnapshot>, TemplateModelError> {
    let Some(plan) = plan else {
        return Ok(None);
    };
    let Some(session_id) = session.and_then(|session| session.session_id.as_deref()) else {
        return Ok(None);
    };
    let store = StorefrontStateStore::open_for_plan(plan).map_err(template_store_error)?;
    let snapshot = store
        .snapshot(
            session_id,
            principal.and_then(|ctx| ctx.principal_id.as_deref()),
        )
        .map_err(template_store_error)?;
    Ok(Some(snapshot))
}

fn live_storefront_latest_order(
    plan: Option<&RuntimePlan>,
    session: Option<&SessionContext>,
    principal: Option<&PrincipalContext>,
) -> Result<Option<StorefrontOrderSnapshot>, TemplateModelError> {
    Ok(live_storefront_state(plan, session, principal)?.and_then(|snapshot| snapshot.latest_order))
}

fn cart_items_from_storefront(
    catalog: &StorefrontCatalog,
    locale: &str,
    lines: &[StorefrontCartLine],
    form_state: Option<&StorefrontFormState>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    lines
        .iter()
        .map(|line| cart_item_from_storefront(catalog, locale, line, form_state))
        .collect::<Result<Vec<_>, _>>()
}

fn cart_summary_from_storefront(
    snapshot: &StorefrontStateSnapshot,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value(
            "subtotal",
            RenderValue::text(snapshot.cart.subtotal.clone()),
        )?
        .with_value("shipping", RenderValue::text("£0.00"))?
        .with_value("total", RenderValue::text(snapshot.cart.subtotal.clone()))
}

fn confirmation_from_storefront(
    plan: Option<&RuntimePlan>,
    order: &StorefrontOrderSnapshot,
) -> Result<RenderModel, TemplateModelError> {
    let includes_membership = order
        .lines
        .iter()
        .any(|line| line.product_kind == "membership");
    let payment_is_final = matches!(order.status.as_str(), "paid" | "fulfilled");
    let next_step = if !payment_is_final {
        configured_payment_provider(plan)
            .map(|provider| provider.pending_next_step())
            .unwrap_or_else(|| {
                "Payment confirmation is pending. The order will move forward after the provider callback arrives.".to_string()
            })
    } else if includes_membership {
        "A confirmation email and membership activation will follow shortly.".to_string()
    } else {
        "A confirmation email and fulfillment summary are on the way.".to_string()
    };
    let payment_method = payment_method_label(order.payment.method.as_deref());
    let payment_status = payment_status_label(&order.payment.status);
    let payment_summary = payment_summary(
        order.payment.method.as_deref(),
        order.payment.last4.as_deref(),
        order.payment.reference.as_deref(),
    );
    RenderModel::new()
        .with_value("orderNumber", RenderValue::text(order.order_id.clone()))?
        .with_value(
            "email",
            RenderValue::text(order.payment.checkout_email.clone().unwrap_or_default()),
        )?
        .with_bool("hasEmail", order.payment.checkout_email.is_some())?
        .with_value("nextStep", RenderValue::text(next_step))?
        .with_value(
            "status",
            RenderValue::text(display_status_label(&order.status)),
        )?
        .with_value("subtotal", RenderValue::text(order.subtotal.clone()))?
        .with_value("total", RenderValue::text(order.total.clone()))?
        .with_value("paymentStatus", RenderValue::text(payment_status))?
        .with_value("paymentMethod", RenderValue::text(payment_method))?
        .with_value(
            "paymentReference",
            RenderValue::text(order.payment.reference.clone().unwrap_or_default()),
        )?
        .with_value(
            "paymentLast4",
            RenderValue::text(order.payment.last4.clone().unwrap_or_default()),
        )?
        .with_value("paymentSummary", RenderValue::text(payment_summary))?
        .with_value(
            "providerLabel",
            RenderValue::text(payment_provider_label(plan)),
        )?
        .with_bool("hasPaymentLast4", order.payment.last4.is_some())?
        .with_bool("hasPaymentReference", order.payment.reference.is_some())?
        .with_bool("hasMembershipItems", includes_membership)?
        .with_bool("hasLineItems", !order.lines.is_empty())?
        .with_list("lineItems", confirmation_line_items_from_storefront(order)?)
}

fn empty_confirmation_model(plan: Option<&RuntimePlan>) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("orderNumber", RenderValue::text(String::new()))?
        .with_value("status", RenderValue::text("No recent order".to_string()))?
        .with_value("total", RenderValue::text("£0.00".to_string()))?
        .with_bool("hasEmail", false)?
        .with_value("email", RenderValue::text(String::new()))?
        .with_value(
            "nextStep",
            RenderValue::text(
                "There is no recent checkout confirmation for this browser session yet.",
            ),
        )?
        .with_value(
            "providerLabel",
            RenderValue::text(payment_provider_label(plan)),
        )?
        .with_value(
            "paymentSummary",
            RenderValue::text("No payment has been submitted yet."),
        )?
        .with_bool("hasLineItems", false)?
        .with_list("lineItems", Vec::new())
}

fn account_order_from_storefront(
    order: &StorefrontOrderSnapshot,
) -> Result<RenderModel, TemplateModelError> {
    let payment_summary = payment_summary(
        order.payment.method.as_deref(),
        order.payment.last4.as_deref(),
        order.payment.reference.as_deref(),
    );
    RenderModel::new()
        .with_value("reference", RenderValue::text(order.order_id.clone()))?
        .with_value("total", RenderValue::text(order.total.clone()))?
        .with_value(
            "status",
            RenderValue::text(display_status_label(&order.status)),
        )
        .and_then(|model| {
            model.with_value("lineCount", RenderValue::text(order.line_count.to_string()))
        })
        .and_then(|model| {
            model.with_value(
                "checkoutEmail",
                RenderValue::text(order.payment.checkout_email.clone().unwrap_or_default()),
            )
        })
        .and_then(|model| model.with_value("paymentSummary", RenderValue::text(payment_summary)))
        .and_then(|model| {
            model.with_bool("hasCheckoutEmail", order.payment.checkout_email.is_some())
        })
        .and_then(|model| {
            model.with_bool(
                "hasPaymentSummary",
                order.payment.method.is_some()
                    || order.payment.reference.is_some()
                    || order.payment.last4.is_some(),
            )
        })
}

fn admin_orders_from_storefront(
    plan: Option<&RuntimePlan>,
    _fixture: &StorefrontFixture,
) -> Result<(Vec<RenderModel>, RenderModel), TemplateModelError> {
    let Some(plan) = plan else {
        return Ok((Vec::new(), admin_order_stats(0, 0, 0)?));
    };
    let store = StorefrontStateStore::open_for_plan(plan).map_err(template_store_error)?;
    let orders = store.admin_orders(50).map_err(template_store_error)?;
    if orders.is_empty() {
        Ok((Vec::new(), admin_order_stats(0, 0, 0)?))
    } else {
        let pending = orders
            .iter()
            .filter(|order| order.status == "pending_payment")
            .count();
        let refunded = orders
            .iter()
            .filter(|order| order.status == "refunded")
            .count();
        let rows = orders
            .iter()
            .map(admin_order_row_from_storefront)
            .collect::<Result<Vec<_>, _>>()?;
        Ok((rows, admin_order_stats(orders.len(), pending, refunded)?))
    }
}

fn order_detail_from_storefront(
    plan: Option<&RuntimePlan>,
    order_id: &str,
    form_state: Option<&StorefrontFormState>,
    session: Option<&SessionContext>,
    principal: Option<&PrincipalContext>,
) -> Result<Option<RenderModel>, TemplateModelError> {
    let Some(plan) = plan else {
        return Ok(None);
    };
    let store = StorefrontStateStore::open_for_plan(plan).map_err(template_store_error)?;
    let Some(order) = store.admin_order(order_id).map_err(template_store_error)? else {
        return Ok(None);
    };
    let payment_summary = payment_summary(
        order.payment.method.as_deref(),
        order.payment.last4.as_deref(),
        order.payment.reference.as_deref(),
    );
    let can_refund = matches!(
        order.status.as_str(),
        "paid" | "fulfilled" | "partially_refunded"
    ) && order.refundable_total_minor > 0;
    let refund_reason = form_state
        .and_then(|state| state.fields.get("reason"))
        .cloned()
        .unwrap_or_else(|| "customer_support".to_string());
    let refund_reason_error = storefront_field_error(form_state, "reason");
    let refund_action_summary = if can_refund {
        "Issue a full remaining refund from the checked-in order state. The order will move to Refunded."
            .to_string()
    } else if order.status == "pending_payment" {
        "This order is still awaiting provider confirmation. Confirm payment capture or failure before refunding."
            .to_string()
    } else if order.refundable_total_minor <= 0 {
        "This order has no remaining refundable balance in the checked-in order state.".to_string()
    } else {
        "This order is not currently eligible for an additional refund from the checked-in workflow."
            .to_string()
    };
    let can_fulfill = order.status == "paid";
    let fulfillment_action_summary = if can_fulfill {
        "Mark the order fulfilled once packing or dispatch is complete. The support queue and order history will show the fulfilled status after this action."
            .to_string()
    } else if order.status == "fulfilled" {
        "This order is already marked fulfilled in the checked-in workflow.".to_string()
    } else if order.status == "pending_payment" {
        "Capture payment before marking the order fulfilled.".to_string()
    } else {
        "This order is not currently eligible for fulfillment in the checked-in workflow."
            .to_string()
    };
    let payment_reference = order.payment.reference.clone().unwrap_or_default();
    let checkout_email = order.payment.checkout_email.clone().unwrap_or_default();
    let principal_id = order.principal_id.clone().unwrap_or_default();
    let line_items = order
        .lines
        .iter()
        .map(|line| {
            RenderModel::new()
                .with_value("title", RenderValue::text(line.title.clone()))?
                .with_value(
                    "variantTitle",
                    RenderValue::text(line.variant_title.clone()),
                )?
                .with_value("sku", RenderValue::text(line.sku.clone()))?
                .with_value("quantity", RenderValue::text(line.quantity.to_string()))?
                .with_value("total", RenderValue::text(line.total.clone()))?
                .with_bool(
                    "hasEntitlementKey",
                    line.entitlement_key
                        .as_deref()
                        .is_some_and(|value| !value.is_empty()),
                )?
                .with_value(
                    "entitlementKey",
                    RenderValue::text(line.entitlement_key.clone().unwrap_or_default()),
                )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let refunds = order
        .refunds
        .iter()
        .map(|refund| {
            RenderModel::new()
                .with_value("refundId", RenderValue::text(refund.refund_id.clone()))?
                .with_value("amount", RenderValue::text(refund.amount.clone()))?
                .with_value("reason", RenderValue::text(refund.reason.clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let customer_review = customer_order_review(plan, &order, session, principal)?;
    let has_customer_review = customer_review.is_some();
    let customer_review_model = customer_review
        .map(customer_order_review_model)
        .transpose()?;
    let mut model = RenderModel::new()
        .with_value("orderId", RenderValue::text(order.order_id.clone()))?
        .with_value("reference", RenderValue::text(order.order_id.clone()))?
        .with_value(
            "status",
            RenderValue::text(display_status_label(&order.status)),
        )?
        .with_value(
            "paymentStatus",
            RenderValue::text(payment_status_label(&order.payment.status)),
        )?
        .with_value("paymentSummary", RenderValue::text(payment_summary))?
        .with_value("paymentReference", RenderValue::text(payment_reference))?
        .with_bool("hasPaymentReference", order.payment.reference.is_some())?
        .with_value("checkoutEmail", RenderValue::text(checkout_email))?
        .with_bool("hasCheckoutEmail", order.payment.checkout_email.is_some())?
        .with_value("principalId", RenderValue::text(principal_id))?
        .with_bool("hasPrincipalId", order.principal_id.is_some())?
        .with_bool("canFulfill", can_fulfill)?
        .with_value(
            "fulfillmentActionSummary",
            RenderValue::text(fulfillment_action_summary),
        )?
        .with_value("sessionId", RenderValue::text(order.session_id.clone()))?
        .with_value("subtotal", RenderValue::text(order.subtotal.clone()))?
        .with_value("total", RenderValue::text(order.total.clone()))?
        .with_value(
            "refundedTotal",
            RenderValue::text(order.refunded_total.clone()),
        )?
        .with_value(
            "refundableTotal",
            RenderValue::text(order.refundable_total.clone()),
        )?
        .with_bool("canRefund", can_refund)?
        .with_value(
            "refundActionSummary",
            RenderValue::text(refund_action_summary),
        )?
        .with_value("refundReason", RenderValue::text(refund_reason))?
        .with_bool("hasRefundReasonError", refund_reason_error.is_some())?
        .with_value(
            "refundReasonError",
            RenderValue::text(refund_reason_error.unwrap_or_default()),
        )?
        .with_bool("hasRefunds", !order.refunds.is_empty())?
        .with_list("refunds", refunds)?
        .with_bool("hasLineItems", !order.lines.is_empty())?
        .with_list("lineItems", line_items)?
        .with_bool("hasCustomerReview", has_customer_review)?
        .with_value(
            "detailHref",
            RenderValue::text(format!("/admin/orders/{}", order.order_id)),
        )?;
    if let Some(customer_review_model) = customer_review_model {
        model = model.with_object("customerReview", customer_review_model)?;
    }
    Ok(Some(model))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CustomerOrderReviewOutcome {
    decision: OrderReviewDecision,
    notes: Vec<String>,
}

fn customer_order_review(
    plan: &RuntimePlan,
    order: &StorefrontOrderSnapshot,
    _session: Option<&SessionContext>,
    principal: Option<&PrincipalContext>,
) -> Result<Option<CustomerOrderReviewOutcome>, TemplateModelError> {
    if plan.customer_hooks.checkout.is_empty() {
        return Ok(None);
    }
    let request = CustomerPluginRequestContext::new(
        CustomerPluginAppContext::new(
            plan.config.app.name.clone(),
            format!("{:?}", plan.config.app.environment).to_ascii_lowercase(),
        ),
        customer_plugin_principal(principal),
        CustomerPluginTraceContext::new(format!("order-detail:{}", order.order_id)),
    );
    let review_notes = Arc::new(Mutex::new(Vec::new()));
    let commerce = RuntimeCustomerCommerceFacade {
        catalog: &plan.storefront_catalog,
        order_id: &order.order_id,
        review_notes: Arc::clone(&review_notes),
    };
    let auth = RuntimeCustomerAuthFacade {
        plan,
        principal_id: principal.and_then(|ctx| ctx.principal_id.as_deref()),
    };
    let audit = RuntimeCustomerAuditFacade {
        plan,
        principal_id: principal.and_then(|ctx| ctx.principal_id.as_deref()),
    };
    let order = customer_order_draft(order, &plan.storefront_catalog);
    let mut adjustment_messages = Vec::new();

    for hook in &plan.customer_hooks.checkout {
        match hook
            .review_order(&request, &order, &commerce, &auth, &audit)
            .map_err(customer_plugin_template_error)?
        {
            OrderReviewDecision::Approved => {}
            OrderReviewDecision::Adjusted(adjustment) => {
                adjustment_messages.push(adjustment.reason);
            }
            OrderReviewDecision::Rejected(rejection) => {
                return Ok(Some(CustomerOrderReviewOutcome {
                    decision: OrderReviewDecision::Rejected(rejection),
                    notes: customer_review_notes(&review_notes)?,
                }));
            }
        }
    }

    let decision = if adjustment_messages.is_empty() {
        OrderReviewDecision::Approved
    } else {
        OrderReviewDecision::Adjusted(davenda_customer_sdk::OrderAdjustment::new(
            adjustment_messages.join("; "),
        ))
    };

    Ok(Some(CustomerOrderReviewOutcome {
        decision,
        notes: customer_review_notes(&review_notes)?,
    }))
}

fn customer_order_draft(
    order: &StorefrontOrderSnapshot,
    catalog: &StorefrontCatalog,
) -> OrderDraft {
    let subtotal = MoneyAmount::new(order.currency.clone(), order.subtotal_minor);
    let total = MoneyAmount::new(order.currency.clone(), order.total_minor);
    let lines = order
        .lines
        .iter()
        .map(|line| OrderLineDraft {
            sku: line.sku.clone(),
            title: line.title.clone(),
            quantity: line.quantity,
            unit_price: MoneyAmount::new(line.currency.clone(), line.unit_price_minor),
            product_kind: line.product_kind.clone(),
            collection_handle: catalog
                .product_by_sku_or_handle(&line.sku)
                .map(|product| product.collection_handle.clone()),
            entitlement_key: line.entitlement_key.clone(),
            metadata: line.metadata.clone(),
        })
        .collect();
    let mut metadata = order.metadata.clone();
    metadata
        .entry("session_id".to_string())
        .or_insert_with(|| order.session_id.clone());
    metadata.entry("payment_method".to_string()).or_insert_with(|| {
        order
            .payment
            .method
            .clone()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
    });
    if let Some(checkout_email) = order.payment.checkout_email.as_ref() {
        metadata
            .entry("checkout_email".to_string())
            .or_insert_with(|| checkout_email.trim().to_string());
    }
    if let Some(principal_id) = order.principal_id.as_ref() {
        metadata
            .entry("order_principal_id".to_string())
            .or_insert_with(|| principal_id.clone());
    }
    OrderDraft {
        order_id: order.order_id.clone(),
        currency_code: order.currency.clone(),
        subtotal,
        total,
        lines,
        metadata,
    }
}

fn customer_plugin_principal(
    principal: Option<&PrincipalContext>,
) -> CustomerPluginPrincipalContext {
    if let Some(principal) = principal {
        if let Some(principal_id) = principal.principal_id.as_ref() {
            return CustomerPluginPrincipalContext::user(principal_id.clone());
        }
    }
    CustomerPluginPrincipalContext {
        kind: CustomerPluginPrincipalKind::Anonymous,
        id: None,
    }
}

fn customer_plugin_template_error(error: BackendError) -> TemplateModelError {
    TemplateModelError::TemplateRead {
        path: "linked customer hook".to_string(),
        message: error.to_string(),
    }
}

fn run_customer_hook_future<T>(
    future: impl Future<Output = Result<T, String>> + Send + 'static,
) -> Result<T, String>
where
    T: Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle)
            if matches!(
                handle.runtime_flavor(),
                tokio::runtime::RuntimeFlavor::MultiThread
            ) =>
        {
            tokio::task::block_in_place(|| handle.block_on(future))
        }
        Ok(_) => std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| error.to_string())?
                .block_on(future)
        })
        .join()
        .map_err(|_| "customer hook runtime bridge thread panicked".to_string())?,
        Err(_) => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?
            .block_on(future),
    }
}

fn customer_hook_auth_backend_error(reason: String) -> BackendError {
    BackendError::new(
        BackendErrorKind::Unavailable,
        "auth.live_check.failed",
        "Runtime could not complete the linked customer auth check.",
    )
    .with_detail(reason)
}

fn parse_customer_capability(value: &str) -> Result<davenda_auth::Capability, BackendError> {
    davenda_auth::Capability::from_str(value).ok_or_else(|| {
        BackendError::new(
            BackendErrorKind::InvalidInput,
            "auth.capability.invalid",
            format!("Unknown capability `{value}`."),
        )
    })
}

fn parse_customer_auth_entity(value: &str) -> Result<davenda_auth::Entity, BackendError> {
    let Some((namespace, id)) = value.split_once(':') else {
        return Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            "auth.object.invalid",
            format!("Invalid auth object `{value}`."),
        ));
    };
    if id.trim().is_empty() {
        return Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            "auth.object.invalid",
            format!("Invalid auth object `{value}`."),
        ));
    }
    match namespace {
        "tenant" => Ok(davenda_auth::Entity::tenant(id)),
        "site" => Ok(davenda_auth::Entity::site(id)),
        "brand" => Ok(davenda_auth::Entity::brand(id)),
        "storefront" => Ok(davenda_auth::Entity::storefront(id)),
        "user" => Ok(davenda_auth::Entity::user(id)),
        "group" => Ok(davenda_auth::Entity::group(id)),
        "team" => Ok(davenda_auth::Entity::team(id)),
        "service_account" => Ok(davenda_auth::Entity::service_account(id)),
        "page" => Ok(davenda_auth::Entity::page(id)),
        "navigation" => Ok(davenda_auth::Entity::navigation(id)),
        "product" => Ok(davenda_auth::Entity::product(id)),
        "collection" => Ok(davenda_auth::Entity::collection(id)),
        "order" => Ok(davenda_auth::Entity::order(id)),
        "subscription" => Ok(davenda_auth::Entity::subscription(id)),
        "membership_tier" => Ok(davenda_auth::Entity::membership_tier(id)),
        "event" => Ok(davenda_auth::Entity::event(id)),
        "event_slot" => Ok(davenda_auth::Entity::event_slot(id)),
        "booking" => Ok(davenda_auth::Entity::booking(id)),
        "media" => Ok(davenda_auth::Entity::media(id)),
        "media_library" => Ok(davenda_auth::Entity::media_library(id)),
        "asset" => Ok(davenda_auth::Entity::asset(id)),
        "asset_folder" => Ok(davenda_auth::Entity::asset_folder(id)),
        "theme_asset_bundle" => Ok(davenda_auth::Entity::theme_asset_bundle(id)),
        "admin_module" => Ok(davenda_auth::Entity::admin_module(id)),
        _ => Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            "auth.object.invalid",
            format!("Unknown auth object namespace `{namespace}`."),
        )),
    }
}

fn customer_hook_auth_subject(principal_id: Option<&str>) -> davenda_auth::DefaultSubject {
    match principal_id {
        Some(principal_id) => davenda_auth::DefaultSubject::entity(davenda_auth::Entity::user(
            principal_id.to_string(),
        )),
        None => davenda_auth::DefaultSubject::entity(davenda_auth::Entity::any_user()),
    }
}

fn customer_review_notes(
    review_notes: &Arc<Mutex<Vec<String>>>,
) -> Result<Vec<String>, TemplateModelError> {
    review_notes
        .lock()
        .map(|notes| notes.clone())
        .map_err(|_| TemplateModelError::TemplateRead {
            path: "linked customer hook".to_string(),
            message: "customer review note state was poisoned".to_string(),
        })
}

fn customer_order_review_model(
    review: CustomerOrderReviewOutcome,
) -> Result<RenderModel, TemplateModelError> {
    let notes = review
        .notes
        .iter()
        .map(|note| RenderModel::new().with_value("text", RenderValue::text(note.clone())))
        .collect::<Result<Vec<_>, _>>()?;
    let base = match review.decision {
        OrderReviewDecision::Approved => RenderModel::new()
            .with_value("status", RenderValue::text("Approved"))?
            .with_value(
                "summary",
                RenderValue::text(
                    "The linked Harbor customer backend approved this order without extra handling.",
                ),
            )?
            .with_value("code", RenderValue::text("approved"))?
            .with_bool("isApproved", true)?
            .with_bool("isRejected", false)?
            .with_bool("isAdjusted", false)?,
        OrderReviewDecision::Rejected(rejection) => RenderModel::new()
            .with_value("status", RenderValue::text("Rejected"))?
            .with_value("summary", RenderValue::text(rejection.message.clone()))?
            .with_value("code", RenderValue::text(rejection.code))?
            .with_bool("isApproved", false)?
            .with_bool("isRejected", true)?
            .with_bool("isAdjusted", false)?,
        OrderReviewDecision::Adjusted(adjustment) => RenderModel::new()
            .with_value("status", RenderValue::text("Adjusted"))?
            .with_value("summary", RenderValue::text(adjustment.reason.clone()))?
            .with_value("code", RenderValue::text("adjusted"))?
            .with_bool("isApproved", false)?
            .with_bool("isRejected", false)?
            .with_bool("isAdjusted", true)?,
    };
    base.with_bool("hasNotes", !notes.is_empty())?
        .with_value("noteCount", RenderValue::text(notes.len().to_string()))?
        .with_list("notes", notes)
}

fn admin_order_row_from_storefront(
    order: &StorefrontOrderSnapshot,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("reference", RenderValue::text(order.order_id.clone()))?
        .with_value(
            "status",
            RenderValue::text(display_status_label(&order.status)),
        )?
        .with_value(
            "paymentStatus",
            RenderValue::text(payment_status_label(&order.payment.status)),
        )?
        .with_value("total", RenderValue::text(order.total.clone()))?
        .with_value(
            "customerEmail",
            RenderValue::text(order.payment.checkout_email.clone().unwrap_or_default()),
        )?
        .with_bool("hasCustomerEmail", order.payment.checkout_email.is_some())?
        .with_value(
            "detailHref",
            RenderValue::text(format!("/admin/orders/{}", order.order_id)),
        )
}

fn admin_order_stats(
    total: usize,
    pending: usize,
    refunded: usize,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("total", RenderValue::text(total.to_string()))?
        .with_value("pending", RenderValue::text(pending.to_string()))?
        .with_value("refunded", RenderValue::text(refunded.to_string()))
}

fn operator_identity(
    principal: Option<&PrincipalContext>,
    session: Option<&SessionContext>,
) -> Result<RenderModel, TemplateModelError> {
    let principal_id = principal
        .and_then(|principal| principal.principal_id.as_deref())
        .unwrap_or_default();
    let display_name = if principal_id.is_empty() {
        "Current Operator".to_string()
    } else {
        display_name_from_principal_id(principal_id)
    };
    RenderModel::new()
        .with_value("displayName", RenderValue::text(display_name))?
        .with_value("principalId", RenderValue::text(principal_id.to_string()))?
        .with_bool("hasPrincipal", principal.is_some())?
        .with_bool(
            "hasSession",
            session
                .and_then(|session| session.session_id.as_deref())
                .is_some(),
        )
}

struct AdminAuditHistoryView {
    entries: Vec<RenderModel>,
    empty_text: String,
    backend: String,
    location: String,
    entry_count: usize,
}

#[derive(Default)]
struct ParsedOperatorAuditRecord {
    action: String,
    route: String,
    capability: String,
    resource_kind: String,
    resource_id: String,
    outcome: String,
    detail: String,
}

fn admin_audit_history(
    plan: Option<&RuntimePlan>,
) -> Result<AdminAuditHistoryView, TemplateModelError> {
    let Some(plan) = plan else {
        return Ok(AdminAuditHistoryView {
            entries: Vec::new(),
            empty_text:
                "Start the checked-in Harbor Shop runtime before expecting persisted operator audit history."
                    .to_string(),
            backend: "unavailable".to_string(),
            location: "runtime not started".to_string(),
            entry_count: 0,
        });
    };

    let snapshot = match RuntimeWasmHostServices::new(plan.clone()).metadata_snapshot(100) {
        Ok(snapshot) => snapshot,
        Err(reason) => {
            let fallback = AdminAuditLog::open(plan);
            let fallback_entries = fallback.recent_entries(100).map_err(|error| {
                TemplateModelError::TemplateRead {
                    path: "admin-audit-log".to_string(),
                    message: format!(
                        "audit history is currently unavailable: {reason}; local fallback also failed: {error}"
                    ),
                }
            })?;
            let entries = fallback_entries
                .iter()
                .map(|record| {
                    admin_audit_entry_model(
                        record.recorded_at_unix_seconds,
                        record.actor.as_str(),
                        record.kind.as_str(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(AdminAuditHistoryView {
                empty_text: format!(
                    "Audit history is using the local fallback log because the shared metadata backend is unavailable: {reason}"
                ),
                backend: "local-fallback".to_string(),
                location: fallback.location_label(),
                entry_count: fallback_entries.len(),
                entries,
            });
        }
    };

    let entries = snapshot
        .recent_records
        .iter()
        .map(|record| {
            admin_audit_entry_model(
                record.recorded_at_unix_seconds,
                record
                    .principal_id
                    .as_deref()
                    .unwrap_or(record.principal_kind.as_str()),
                record.kind.as_str(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AdminAuditHistoryView {
        empty_text: format!(
            "Audit backend `{}` at `{}` is live, but no privileged admin actions have been captured yet for this Harbor Shop runtime.",
            snapshot.backend.as_str(),
            snapshot.location
        ),
        backend: snapshot.backend.as_str().to_string(),
        location: snapshot.location,
        entry_count: snapshot.entry_count,
        entries,
    })
}

fn admin_audit_entry_model(
    recorded_at_unix_seconds: i64,
    actor: &str,
    kind: &str,
) -> Result<RenderModel, TemplateModelError> {
    let parsed = parse_operator_audit_record(kind);
    let resource = if parsed.resource_id.trim().is_empty() {
        parsed.resource_kind.clone()
    } else {
        format!("{}:{}", parsed.resource_kind, parsed.resource_id)
    };
    let detail = parsed.detail.trim().to_string();
    RenderModel::new()
        .with_value(
            "when",
            RenderValue::text(recorded_at_unix_seconds.to_string()),
        )?
        .with_value("actor", RenderValue::text(actor.to_string()))?
        .with_value(
            "action",
            RenderValue::text(admin_audit_action_label(parsed.action.as_str())),
        )?
        .with_value("capability", RenderValue::text(parsed.capability))?
        .with_value("resource", RenderValue::text(resource))?
        .with_value(
            "outcome",
            RenderValue::text(admin_audit_outcome_label(parsed.outcome.as_str())),
        )?
        .with_bool("hasDetail", !detail.is_empty())?
        .with_value("detail", RenderValue::text(detail))
}

fn parse_operator_audit_record(kind: &str) -> ParsedOperatorAuditRecord {
    if !kind.contains('=') {
        return ParsedOperatorAuditRecord {
            action: kind.to_string(),
            capability: admin_audit_capability_fallback(kind).to_string(),
            outcome: "succeeded".to_string(),
            ..ParsedOperatorAuditRecord::default()
        };
    }

    let mut parsed = ParsedOperatorAuditRecord::default();
    for (key, value) in form_urlencoded::parse(kind.as_bytes()) {
        match key.as_ref() {
            "action" => parsed.action = value.into_owned(),
            "route" => parsed.route = value.into_owned(),
            "capability" => parsed.capability = value.into_owned(),
            "resource_kind" => parsed.resource_kind = value.into_owned(),
            "resource_id" => parsed.resource_id = value.into_owned(),
            "outcome" => parsed.outcome = value.into_owned(),
            "detail" => parsed.detail = value.into_owned(),
            _ => {}
        }
    }

    if parsed.capability.is_empty() {
        parsed.capability = admin_audit_capability_fallback(parsed.action.as_str()).to_string();
    }
    if parsed.outcome.is_empty() {
        parsed.outcome = "succeeded".to_string();
    }
    parsed
}

fn admin_audit_capability_fallback(action: &str) -> &'static str {
    match action {
        "cms.pages.save-draft" => "cms.page.edit",
        "cms.pages.publish" | "cms.pages.unpublish" => "cms.page.publish",
        "cms.navigation.save" => "cms.navigation.edit",
        "cms.redirects.save" => "cms.page.edit",
        "commerce.catalog-admin-update.product"
        | "commerce.catalog-admin-update.collection"
        | "commerce.catalog-admin-update" => "catalog.product.edit",
        "commerce.order-refund" => "order.refund.issue",
        "commerce.order-fulfill" => "order.refund.issue",
        _ => "",
    }
}

fn admin_audit_action_label(action: &str) -> &'static str {
    match action {
        "cms.pages.save-draft" => "Save draft",
        "cms.pages.publish" => "Publish page",
        "cms.pages.unpublish" => "Unpublish page",
        "cms.navigation.save" => "Save navigation",
        "cms.redirects.save" => "Save redirects",
        "commerce.catalog-admin-update.product" => "Update product",
        "commerce.catalog-admin-update.collection" => "Update collection",
        "commerce.order-refund" => "Issue refund",
        "commerce.order-fulfill" => "Mark fulfilled",
        _ => "Privileged action",
    }
}

fn admin_audit_outcome_label(outcome: &str) -> &'static str {
    match outcome {
        "succeeded" => "Succeeded",
        "rejected" => "Rejected",
        _ => "Recorded",
    }
}

fn admin_panels(
    locale: &str,
    fixture: &StorefrontFixture,
    order_count: usize,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    let content_pages = content_pages(locale)?;
    Ok(vec![
        admin_panel(
            "Catalog",
            "Inspect products and collections",
            "/admin/catalog/products",
            &format!(
                "{} products and {} collections are currently represented in the sample app.",
                fixture.product_cards.len(),
                fixture.catalog_sections.len()
            ),
        )?,
        admin_panel(
            "Orders",
            "Review recent purchases",
            "/admin/orders",
            &format!("{order_count} completed orders are available for operator review."),
        )?,
        admin_panel(
            "Content",
            "Review live route inventory",
            "/admin/pages",
            &format!(
                "{} content routes are represented in the checked-in Harbor Shop sample app.",
                content_pages.len()
            ),
        )?,
    ])
}

fn admin_panel(
    title: &str,
    label: &str,
    href: &str,
    summary: &str,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("title", RenderValue::text(title))?
        .with_value("label", RenderValue::text(label))?
        .with_value("href", RenderValue::text(href))?
        .with_value("summary", RenderValue::text(summary))
}

fn content_pages(locale: &str) -> Result<Vec<RenderModel>, TemplateModelError> {
    Ok(vec![
        content_page(
            "Home",
            "/",
            "public",
            "The landing page for the Harbor Shop storefront.",
        )?,
        content_page(
            "Catalog",
            &localized_shop_path(locale),
            "public",
            "The main shopping entry point for products and collections.",
        )?,
        content_page(
            "Collections",
            &localized_collections_path(locale),
            "public",
            "Curated collection landing pages for merchandising journeys.",
        )?,
        content_page(
            "Account",
            "/account",
            "account",
            "Customer account hub for orders and membership state.",
        )?,
        content_page(
            "Order history",
            "/account/orders",
            "account",
            "Order history and post-checkout confirmation records.",
        )?,
        content_page(
            "Memberships",
            "/account/memberships",
            "account",
            "Membership state and entitlement guidance for signed-in customers.",
        )?,
    ])
}

fn content_page(
    title: &str,
    href: &str,
    surface: &str,
    summary: &str,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("title", RenderValue::text(title))?
        .with_value("href", RenderValue::text(href))?
        .with_value("surface", RenderValue::text(surface))?
        .with_value("summary", RenderValue::text(summary))
}

fn query_first<'a>(query_params: &'a RequestFieldMap, name: &str) -> Option<&'a str> {
    query_params
        .get(name)
        .and_then(|values| values.first())
        .map(String::as_str)
}

fn query_flag(query_params: &RequestFieldMap, name: &str) -> bool {
    query_first(query_params, name)
        .is_some_and(|value| matches!(value, "1" | "true" | "yes" | "on"))
}

fn cms_admin_form_targets_new_page(form_state: Option<&StorefrontFormState>) -> bool {
    let Some(form_state) = form_state else {
        return false;
    };
    let page_id = form_state
        .fields
        .get("page_id")
        .map(String::as_str)
        .unwrap_or_default()
        .trim();
    if !page_id.is_empty() {
        return false;
    }
    ["page_title", "page_slug", "page_summary", "page_body_html"]
        .iter()
        .any(|field| form_state.fields.contains_key(*field))
}

fn cms_admin_workspace(plan: &RuntimePlan) -> Result<CmsAdminWorkspace, TemplateModelError> {
    CmsAdminWorkspace::load(plan).map_err(|message| TemplateModelError::TemplateRead {
        path: "cms-admin-workspace".to_string(),
        message,
    })
}

fn default_cms_admin_workspace() -> CmsAdminWorkspace {
    crate::default_workspace()
}

fn cms_admin_pages_model(
    workspace: &CmsAdminWorkspace,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    workspace
        .pages
        .iter()
        .map(|page| {
            RenderModel::new()
                .with_value("id", RenderValue::text(page.id.clone()))?
                .with_value("title", RenderValue::text(page.draft.title.clone()))?
                .with_value("slug", RenderValue::text(page.draft.slug.clone()))?
                .with_value(
                    "statusLabel",
                    RenderValue::text(page.status_label().to_string()),
                )?
                .with_value("summary", RenderValue::text(page.draft.summary.clone()))?
                .with_value(
                    "editHref",
                    RenderValue::text(format!("/admin/pages?page={}", page.id)),
                )?
                .with_bool("hasLivePath", page.live_path().is_some())?
                .with_value(
                    "livePath",
                    RenderValue::text(page.live_path().unwrap_or_default()),
                )
        })
        .collect()
}

fn cms_admin_selected_page_model(page: CmsAdminPage) -> Result<RenderModel, TemplateModelError> {
    let preview_html = TrustedHtml::new(page.draft.body_html.clone())?;
    RenderModel::new()
        .with_value("id", RenderValue::text(page.id.clone()))?
        .with_value("title", RenderValue::text(page.draft.title.clone()))?
        .with_value("slug", RenderValue::text(page.draft.slug.clone()))?
        .with_value("summary", RenderValue::text(page.draft.summary.clone()))?
        .with_value(
            "bodySource",
            RenderValue::text(page.draft.body_html.clone()),
        )?
        .with_value("bodyHtml", RenderValue::trusted_html(preview_html))?
        .with_value(
            "statusLabel",
            RenderValue::text(page.status_label().to_string()),
        )?
        .with_bool("hasLivePath", page.live_path().is_some())?
        .with_value(
            "livePath",
            RenderValue::text(page.live_path().unwrap_or_default()),
        )?
        .with_bool("hasPreviewPath", true)?
        .with_value("previewPath", RenderValue::text(page.preview_path()))
}

fn cms_admin_selected_page_model_with_form_state(
    page: Option<CmsAdminPage>,
    form_state: Option<&StorefrontFormState>,
) -> Result<RenderModel, TemplateModelError> {
    let page_id = form_state
        .and_then(|state| state.fields.get("page_id"))
        .cloned()
        .or_else(|| page.as_ref().map(|page| page.id.clone()))
        .unwrap_or_default();
    let title = form_state
        .and_then(|state| state.fields.get("page_title"))
        .cloned()
        .or_else(|| page.as_ref().map(|page| page.draft.title.clone()))
        .unwrap_or_default();
    let slug = form_state
        .and_then(|state| state.fields.get("page_slug"))
        .cloned()
        .or_else(|| page.as_ref().map(|page| page.draft.slug.clone()))
        .unwrap_or_default();
    let summary = form_state
        .and_then(|state| state.fields.get("page_summary"))
        .cloned()
        .or_else(|| page.as_ref().map(|page| page.draft.summary.clone()))
        .unwrap_or_default();
    let body_html = form_state
        .and_then(|state| state.fields.get("page_body_html"))
        .cloned()
        .or_else(|| page.as_ref().map(|page| page.draft.body_html.clone()))
        .unwrap_or_else(|| "<p>Create a draft page to preview it.</p>".to_string());
    let status_label = page
        .as_ref()
        .map(|page| page.status_label().to_string())
        .unwrap_or_else(|| "Draft only".to_string());
    let live_path = page
        .as_ref()
        .and_then(|page| page.live_path())
        .unwrap_or_default();
    let has_live_path = !live_path.is_empty();
    let preview_path = page
        .as_ref()
        .map(|page| page.preview_path())
        .or_else(|| (!page_id.is_empty()).then(|| format!("/admin/pages/preview?page={page_id}")))
        .unwrap_or_default();
    RenderModel::new()
        .with_value("id", RenderValue::text(page_id))?
        .with_value("title", RenderValue::text(title))?
        .with_value("slug", RenderValue::text(slug.clone()))?
        .with_value("summary", RenderValue::text(summary))?
        .with_value("bodySource", RenderValue::text(body_html.clone()))?
        .with_value(
            "bodyHtml",
            RenderValue::trusted_html(TrustedHtml::new(body_html)?),
        )?
        .with_value("statusLabel", RenderValue::text(status_label))?
        .with_bool("hasLivePath", has_live_path)?
        .with_value("livePath", RenderValue::text(live_path))?
        .with_bool("hasPreviewPath", !preview_path.is_empty())?
        .with_value("previewPath", RenderValue::text(preview_path))
}

fn empty_cms_admin_selected_page_model() -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("id", RenderValue::text(String::new()))?
        .with_value("title", RenderValue::text(String::new()))?
        .with_value("slug", RenderValue::text(String::new()))?
        .with_value("summary", RenderValue::text(String::new()))?
        .with_value("bodySource", RenderValue::text(String::new()))?
        .with_value(
            "bodyHtml",
            RenderValue::trusted_html(TrustedHtml::new(
                "<p>Create a draft page to preview it.</p>",
            )?),
        )?
        .with_value("statusLabel", RenderValue::text("Draft only"))?
        .with_bool("hasLivePath", false)?
        .with_value("livePath", RenderValue::text(String::new()))?
        .with_bool("hasPreviewPath", false)?
        .with_value("previewPath", RenderValue::text(String::new()))
}

fn cms_navigation_items_model(
    items: &[CmsAdminNavigationItem],
) -> Result<Vec<RenderModel>, TemplateModelError> {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            RenderModel::new()
                .with_value("label", RenderValue::text(item.label.clone()))?
                .with_value("href", RenderValue::text(item.href.clone()))?
                .with_value(
                    "labelField",
                    RenderValue::text(format!("nav_label_{index}")),
                )?
                .with_value("hrefField", RenderValue::text(format!("nav_href_{index}")))
        })
        .collect()
}

fn cms_redirects_model(
    redirects: &[CmsAdminRedirect],
) -> Result<Vec<RenderModel>, TemplateModelError> {
    redirects
        .iter()
        .enumerate()
        .map(|(index, redirect)| {
            RenderModel::new()
                .with_value("from", RenderValue::text(redirect.from.clone()))?
                .with_value("to", RenderValue::text(redirect.to.clone()))?
                .with_bool("permanent", redirect.permanent)?
                .with_value(
                    "fromField",
                    RenderValue::text(format!("redirect_from_{index}")),
                )?
                .with_value("toField", RenderValue::text(format!("redirect_to_{index}")))?
                .with_value(
                    "permanentField",
                    RenderValue::text(format!("redirect_permanent_{index}")),
                )
        })
        .collect()
}

fn cms_live_page_model(
    workspace: &CmsAdminWorkspace,
    slug: &str,
) -> Result<RenderModel, TemplateModelError> {
    if let Some(page) = workspace.live_page_by_slug(slug) {
        let live = page
            .live
            .as_ref()
            .expect("live page should have a live revision");
        return RenderModel::new()
            .with_bool("isPublished", true)?
            .with_value("title", RenderValue::text(live.title.clone()))?
            .with_value("summary", RenderValue::text(live.summary.clone()))?
            .with_value(
                "bodyHtml",
                RenderValue::trusted_html(TrustedHtml::new(live.body_html.clone())?),
            )?
            .with_value("slug", RenderValue::text(live.slug.clone()));
    }

    RenderModel::new()
        .with_bool("isPublished", false)?
        .with_value("title", RenderValue::text("Page unavailable"))?
        .with_value(
            "summary",
            RenderValue::text(
                "This CMS page is not published yet. Use the Harbor Shop admin workflow to publish it before linking customers to this path.",
            ),
        )?
        .with_value(
            "bodyHtml",
            RenderValue::trusted_html(TrustedHtml::new(
                "<p>The requested CMS page is not live yet.</p>",
            )?),
        )?
        .with_value("slug", RenderValue::text(slug.to_string()))
}

fn merge_cms_page_form_feedback(
    model: RenderModel,
    form_state: Option<&StorefrontFormState>,
) -> Result<RenderModel, TemplateModelError> {
    let errors = form_errors_model(form_state)?;
    let has_errors = form_state.is_some() || !errors.is_empty();
    model
        .with_bool("hasErrors", has_errors)?
        .with_value(
            "errorSummary",
            RenderValue::text(
                form_state
                    .map(|state| state.summary.clone())
                    .unwrap_or_else(|| "Update the page draft and save again.".to_string()),
            ),
        )?
        .with_list("errors", errors)
}

fn merge_cms_navigation_form_feedback(
    model: RenderModel,
    form_state: Option<&StorefrontFormState>,
) -> Result<RenderModel, TemplateModelError> {
    let errors = form_errors_model(form_state)?;
    let has_errors = form_state.is_some() || !errors.is_empty();
    model
        .with_bool("hasErrors", has_errors)?
        .with_value(
            "errorSummary",
            RenderValue::text(
                form_state
                    .map(|state| state.summary.clone())
                    .unwrap_or_else(|| "Update the navigation items and save again.".to_string()),
            ),
        )?
        .with_list("errors", errors)
}

fn merge_cms_redirect_form_feedback(
    model: RenderModel,
    form_state: Option<&StorefrontFormState>,
) -> Result<RenderModel, TemplateModelError> {
    let errors = form_errors_model(form_state)?;
    let has_errors = form_state.is_some() || !errors.is_empty();
    model
        .with_bool("hasErrors", has_errors)?
        .with_value(
            "errorSummary",
            RenderValue::text(
                form_state
                    .map(|state| state.summary.clone())
                    .unwrap_or_else(|| "Update the redirect rules and save again.".to_string()),
            ),
        )?
        .with_list("errors", errors)
}

fn merge_order_refund_form_feedback(
    model: RenderModel,
    form_state: Option<&StorefrontFormState>,
) -> Result<RenderModel, TemplateModelError> {
    let errors = form_errors_model(form_state)?;
    model
        .with_bool("hasRefundErrors", !errors.is_empty())?
        .with_value(
            "refundErrorSummary",
            RenderValue::text(
                form_state
                    .map(|state| state.summary.clone())
                    .unwrap_or_else(|| {
                        "Review the refund request before issuing another order change.".to_string()
                    }),
            ),
        )?
        .with_list("refundErrors", errors)
}

fn cart_item_from_storefront(
    catalog: &StorefrontCatalog,
    locale: &str,
    line: &StorefrontCartLine,
    form_state: Option<&StorefrontFormState>,
) -> Result<RenderModel, TemplateModelError> {
    let quantity_field = format!("quantity_{}", line.sku);
    let quantity_value = form_state
        .and_then(|state| state.fields.get(&quantity_field))
        .cloned()
        .unwrap_or_else(|| line.quantity.to_string());
    let quantity_error = form_state.and_then(|state| state.field_errors.get(&quantity_field));
    let model = cart_item(
        &line.title,
        &line.variant_title,
        &quantity_value,
        &line.total,
    )?
    .with_value("quantityField", RenderValue::text(quantity_field))
    .and_then(|model| model.with_bool("hasQuantityError", quantity_error.is_some()))
    .and_then(|model| {
        model.with_value(
            "quantityError",
            RenderValue::text(quantity_error.cloned().unwrap_or_default()),
        )
    })?;

    decorate_cart_item_with_catalog_context(model, catalog, locale, &line.sku, &line.title)
}

fn decorate_cart_item_with_catalog_context(
    model: RenderModel,
    catalog: &StorefrontCatalog,
    locale: &str,
    sku_or_handle: &str,
    title: &str,
) -> Result<RenderModel, TemplateModelError> {
    let Some(product) = catalog_product_for_cart_item(catalog, sku_or_handle, title) else {
        return model
            .with_bool("hasProductLink", false)?
            .with_value("productUrl", RenderValue::text(String::new()))?
            .with_value("collectionUrl", RenderValue::text(String::new()))?
            .with_value("collectionName", RenderValue::text(String::new()));
    };

    let collection_name = catalog
        .collection(&product.collection_handle)
        .map(|collection| collection.title.as_str())
        .unwrap_or("Collection");

    model
        .with_bool("hasProductLink", true)?
        .with_value(
            "productUrl",
            RenderValue::text(localized_product_path(locale, &product.handle)),
        )?
        .with_value(
            "collectionUrl",
            RenderValue::text(localized_collection_path(
                locale,
                &product.collection_handle,
            )),
        )?
        .with_value("collectionName", RenderValue::text(collection_name))
}

fn catalog_product_for_cart_item<'a>(
    catalog: &'a StorefrontCatalog,
    sku_or_handle: &str,
    title: &str,
) -> Option<&'a StorefrontProductDefinition> {
    catalog.product_by_sku_or_handle(sku_or_handle).or_else(|| {
        catalog
            .products
            .iter()
            .find(|product| product.title == title)
    })
}

fn checkout_customer(
    principal: Option<&PrincipalContext>,
) -> Result<RenderModel, TemplateModelError> {
    let email = principal
        .and_then(|principal| principal.principal_id.clone())
        .filter(|candidate| looks_like_email(candidate))
        .unwrap_or_default();
    let display_name = principal
        .and_then(|principal| principal.principal_id.as_deref())
        .map(display_name_from_principal_id)
        .unwrap_or_else(|| "Guest Checkout".to_string());
    RenderModel::new()
        .with_value("displayName", RenderValue::text(display_name))?
        .with_value("email", RenderValue::text(email))
}

fn checkout_form_from_storefront(
    plan: Option<&RuntimePlan>,
    payment: &StorefrontPaymentSnapshot,
    principal: Option<&PrincipalContext>,
    form_state: Option<&StorefrontFormState>,
) -> Result<RenderModel, TemplateModelError> {
    let provider_code = payment_provider_code(plan);
    let provider_label = payment_provider_label(plan);
    let provider_summary = payment_provider_summary(plan);
    let submit_label = payment_submit_label(plan);
    let payment_method = form_state
        .and_then(|state| state.fields.get("payment_method"))
        .cloned()
        .filter(|value| !value.is_empty())
        .or_else(|| payment.method.clone())
        .unwrap_or_else(|| "card".to_string());
    let checkout_email = form_state
        .and_then(|state| state.fields.get("checkout_email"))
        .cloned()
        .or_else(|| {
            payment.checkout_email.clone().or_else(|| {
                principal
                    .and_then(|principal| principal.principal_id.clone())
                    .filter(|candidate| looks_like_email(candidate))
            })
        })
        .unwrap_or_default();
    let payment_reference = payment
        .reference
        .clone()
        .unwrap_or_else(|| "PAYMENT-PENDING".to_string());
    let checkout_intent = form_state
        .and_then(|state| state.fields.get("checkout_intent"))
        .cloned()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| payment_reference.clone());
    let payment_last4 = form_state
        .and_then(|state| state.fields.get("payment_last4"))
        .cloned()
        .or_else(|| payment.last4.clone())
        .unwrap_or_default();
    let delivery_name = form_state
        .and_then(|state| state.fields.get("delivery_name"))
        .cloned()
        .unwrap_or_default();
    let delivery_note = form_state
        .and_then(|state| state.fields.get("delivery_note"))
        .cloned()
        .unwrap_or_default();
    let terms_accepted = form_state
        .and_then(|state| state.fields.get("terms_accepted"))
        .is_some();
    let has_checkout_email = !checkout_email.is_empty();
    let model = RenderModel::new()
        .with_value("paymentReference", RenderValue::text(payment_reference))?
        .with_value("paymentMethod", RenderValue::text(payment_method.clone()))?
        .with_value("checkoutEmail", RenderValue::text(checkout_email))?
        .with_bool("hasCheckoutEmail", has_checkout_email)?
        .with_value("paymentLast4", RenderValue::text(payment_last4))?
        .with_value("checkoutIntent", RenderValue::text(checkout_intent))?
        .with_value("deliveryName", RenderValue::text(delivery_name))?
        .with_value("deliveryNote", RenderValue::text(delivery_note))?
        .with_bool("termsAccepted", terms_accepted)?
        .with_value(
            "paymentMethodLabel",
            RenderValue::text(payment_method_label(Some(payment_method.as_str()))),
        )?
        .with_value("paymentStatus", RenderValue::text(payment.status.clone()))?
        .with_value(
            "paymentStatusLabel",
            RenderValue::text(payment_status_label(&payment.status)),
        )?
        .with_value("providerCode", RenderValue::text(provider_code.to_string()))?
        .with_value(
            "providerLabel",
            RenderValue::text(provider_label.to_string()),
        )?
        .with_value("providerSummary", RenderValue::text(provider_summary))?
        .with_value("submitLabel", RenderValue::text(submit_label))?
        .with_bool("hasPaymentReference", payment.reference.is_some())?
        .with_bool("hasPaymentLast4", payment.last4.is_some())?;
    merge_checkout_form_feedback(model, form_state)
}

fn payment_provider_code(plan: Option<&RuntimePlan>) -> String {
    configured_payment_provider(plan)
        .map(|provider| provider.code.clone())
        .unwrap_or_else(|| "platform_fallback".to_string())
}

fn payment_provider_label(plan: Option<&RuntimePlan>) -> String {
    configured_payment_provider(plan)
        .map(|provider| provider.label())
        .unwrap_or_else(|| "Platform fallback payment path".to_string())
}

fn payment_provider_summary(plan: Option<&RuntimePlan>) -> String {
    configured_payment_provider(plan)
        .map(|provider| provider.summary())
        .unwrap_or_else(|| {
            "This checkout is using the platform fallback payment path until a provider-backed handoff is installed.".to_string()
        })
}

fn payment_submit_label(plan: Option<&RuntimePlan>) -> String {
    configured_payment_provider(plan)
        .map(|provider| provider.submit_label())
        .unwrap_or_else(|| "Place order".to_string())
}

fn configured_payment_provider(
    plan: Option<&RuntimePlan>,
) -> Option<crate::server::CommercePaymentProviderConfig> {
    plan.and_then(|plan| crate::server::configured_commerce_payment_provider(&plan.config))
}

fn payment_method_label(method: Option<&str>) -> String {
    match method.unwrap_or_default() {
        "card" => "Card".to_string(),
        "gift_credit" => "Gift credit".to_string(),
        "manual" => "Manual capture".to_string(),
        "" => "Payment method pending".to_string(),
        other => display_status_label(other),
    }
}

fn payment_status_label(status: &str) -> String {
    match status {
        "not_started" => "Not started".to_string(),
        "ready_for_payment" => "Ready for payment".to_string(),
        "provider_pending" => "Awaiting provider confirmation".to_string(),
        "captured" => "Captured".to_string(),
        "authorized" => "Authorized".to_string(),
        "failed" => "Failed".to_string(),
        "refunded" => "Refunded".to_string(),
        other => display_status_label(other),
    }
}

fn payment_summary(method: Option<&str>, last4: Option<&str>, reference: Option<&str>) -> String {
    let method = payment_method_label(method);
    match (last4, reference) {
        (Some(last4), Some(reference)) => format!("{method} ending {last4}, reference {reference}"),
        (Some(last4), None) => format!("{method} ending {last4}"),
        (None, Some(reference)) => format!("{method}, reference {reference}"),
        (None, None) => method,
    }
}

fn template_store_error(error: crate::storefront::StorefrontStateError) -> TemplateModelError {
    TemplateModelError::TemplateRead {
        path: "storefront-state".to_string(),
        message: error.to_string(),
    }
}

fn template_membership_error(error: MembershipModelError) -> TemplateModelError {
    TemplateModelError::TemplateRead {
        path: "membership-projection".to_string(),
        message: error.to_string(),
    }
}

fn template_commerce_error(error: davenda_commerce::CommerceModelError) -> TemplateModelError {
    TemplateModelError::TemplateRead {
        path: "membership-projection".to_string(),
        message: error.to_string(),
    }
}

fn confirmation_line_items_from_storefront(
    order: &StorefrontOrderSnapshot,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    order
        .lines
        .iter()
        .map(|line| {
            RenderModel::new()
                .with_value("title", RenderValue::text(line.title.clone()))?
                .with_value("quantity", RenderValue::text(line.quantity.to_string()))?
                .with_value("total", RenderValue::text(line.total.clone()))
        })
        .collect::<Result<Vec<_>, _>>()
}

fn title_case_handle(handle: &str) -> String {
    handle
        .split('-')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => {
                    let mut word = first.to_uppercase().collect::<String>();
                    word.push_str(chars.as_str());
                    word
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn localized_shop_path(locale: &str) -> String {
    format!("/{}/shop", locale.trim_matches('/'))
}

fn localized_collections_path(locale: &str) -> String {
    format!("/{}/shop/collections", locale.trim_matches('/'))
}

fn localized_collection_path(locale: &str, slug: &str) -> String {
    format!(
        "/{}/shop/collections/{}",
        locale.trim_matches('/'),
        slug.trim_matches('/')
    )
}

fn localized_product_path(locale: &str, slug: &str) -> String {
    format!(
        "/{}/shop/products/{}",
        locale.trim_matches('/'),
        slug.trim_matches('/')
    )
}

#[derive(Clone)]
struct AccountSurfaceBindings {
    account: RenderModel,
    customer: RenderModel,
    recent_orders: Vec<RenderModel>,
    membership_summary: RenderModel,
}

fn flash_messages_model(messages: &[FlashMessage]) -> Result<Vec<RenderModel>, TemplateModelError> {
    messages
        .iter()
        .map(|message| {
            RenderModel::new()
                .with_value(
                    "level",
                    RenderValue::text(format!("{:?}", message.level).to_ascii_lowercase()),
                )?
                .with_value("text", RenderValue::text(message.text.clone()))
        })
        .collect::<Result<Vec<_>, _>>()
}

fn account_surface_bindings(
    plan: Option<&RuntimePlan>,
    fixture: &StorefrontFixture,
    locale: &str,
    session: Option<&SessionContext>,
    principal: Option<&PrincipalContext>,
    include_pending_membership: bool,
) -> Result<AccountSurfaceBindings, TemplateModelError> {
    let Some(session) = session else {
        return fixture_account_surface_bindings(fixture, locale);
    };
    if session.session_id.is_none() {
        return fixture_account_surface_bindings(fixture, locale);
    }

    live_account_surface_bindings(plan, locale, session, principal, include_pending_membership)
}

fn fixture_account_surface_bindings(
    fixture: &StorefrontFixture,
    locale: &str,
) -> Result<AccountSurfaceBindings, TemplateModelError> {
    let latest_preview_order = sample_completed_order();
    let has_recent_orders = !fixture.recent_orders.is_empty();
    let orders_cta_url = if has_recent_orders {
        "/account/orders".to_string()
    } else {
        localized_shop_path(locale)
    };
    let orders_cta_label = if has_recent_orders {
        "View order history"
    } else {
        "Browse storefront"
    };
    Ok(AccountSurfaceBindings {
        account: RenderModel::new()
            .with_bool("hasLiveSession", false)?
            .with_bool("hasPrincipal", false)?
            .with_bool("hasCustomerEmail", true)?
            .with_bool("hasRecentOrders", has_recent_orders)?
            .with_bool("hasMembership", true)?
            .with_bool("hasLatestOrder", true)?
            .with_bool("hasPendingMembershipOrder", false)?
            .with_bool("needsMembershipPurchase", false)?
            .with_value("stateSource", RenderValue::text("fixture-preview"))?
            .with_value(
                "stateSummary",
                RenderValue::text(
                    "Previewing deterministic account content until a live storefront session is resolved.",
                ),
            )?
            .with_value(
                "ordersEmptyText",
                RenderValue::text(
                    "Recent orders will appear here once the customer has completed checkout.",
                ),
            )?
            .with_value(
                "membershipEmptyText",
                RenderValue::text(
                    "No membership is attached yet. Join to unlock early-access drops and concierge support.",
                ),
            )?
            .with_value("ordersCtaUrl", RenderValue::text(orders_cta_url))?
            .with_value("ordersCtaLabel", RenderValue::text(orders_cta_label))?
            .with_value(
                "membershipCtaUrl",
                RenderValue::text(localized_collection_path(locale, "memberships")),
            )?
            .with_value(
                "latestOrderReference",
                RenderValue::text(latest_preview_order.id.to_string()),
            )?
            .with_value(
                "latestOrderStatus",
                RenderValue::text(latest_preview_order.history_status_label()),
            )?,
        customer: fixture.customer.clone(),
        recent_orders: fixture.recent_orders.clone(),
        membership_summary: fixture.membership_summary.clone(),
    })
}

fn live_account_surface_bindings(
    plan: Option<&RuntimePlan>,
    locale: &str,
    session: &SessionContext,
    principal: Option<&PrincipalContext>,
    include_pending_membership: bool,
) -> Result<AccountSurfaceBindings, TemplateModelError> {
    let snapshot = live_storefront_state(plan, Some(session), principal)?;
    let principal_id = principal.and_then(|principal| principal.principal_id.as_deref());
    let recent_orders = recent_orders_from_storefront(snapshot.as_ref())?;
    let has_recent_orders = !recent_orders.is_empty();
    let latest_order = snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.recent_orders.first().cloned());
    let email = principal_id
        .filter(|candidate| looks_like_email(candidate))
        .map(str::to_string)
        .or_else(|| {
            latest_order
                .as_ref()
                .and_then(|order| order.payment.checkout_email.clone())
        })
        .unwrap_or_default();
    let display_name = principal_id
        .map(display_name_from_principal_id)
        .or_else(|| {
            if email.is_empty() {
                None
            } else {
                Some(display_name_from_principal_id(&email))
            }
        })
        .unwrap_or_else(|| "Current Browser Session".to_string());
    let membership_summary =
        membership_summary_from_storefront(snapshot.as_ref(), include_pending_membership)?;
    let has_membership = membership_summary.is_some();
    let has_latest_order = latest_order.is_some();
    let has_pending_membership_order = !has_membership && has_latest_order;
    let needs_membership_purchase = !has_membership && !has_latest_order;
    let latest_order_reference = latest_order
        .as_ref()
        .map(|order| order.order_id.clone())
        .unwrap_or_default();
    let latest_order_status = latest_order
        .as_ref()
        .map(|order| display_status_label(&order.status))
        .unwrap_or_default();
    let state_summary = account_state_summary(
        if principal_id.is_some() {
            "Using the live storefront session identity for this account view. Order history and membership state render from the current signed-in browser session."
        } else {
            "This account area follows the current browser session. Completed checkouts from this browser become the order history shown here, and any qualifying membership purchase from this browser appears here after payment capture."
        },
        latest_order.as_ref(),
    );
    let orders_cta_url = if has_recent_orders {
        "/account/orders".to_string()
    } else {
        localized_shop_path(locale)
    };
    let orders_cta_label = if has_recent_orders {
        "View order history"
    } else {
        "Browse storefront"
    };

    Ok(AccountSurfaceBindings {
        account: RenderModel::new()
            .with_bool("hasLiveSession", session.session_id.is_some())?
            .with_bool("hasPrincipal", principal_id.is_some())?
            .with_bool("hasCustomerEmail", !email.is_empty())?
            .with_bool("hasRecentOrders", has_recent_orders)?
            .with_bool("hasMembership", has_membership)?
            .with_bool("hasLatestOrder", has_latest_order)?
            .with_bool("hasPendingMembershipOrder", has_pending_membership_order)?
            .with_bool("needsMembershipPurchase", needs_membership_purchase)?
            .with_value("stateSource", RenderValue::text("storefront-session"))?
            .with_value("stateSummary", RenderValue::text(state_summary))?
            .with_value(
                "ordersEmptyText",
                RenderValue::text(
                    if principal_id.is_some() {
                        "No order history is attached to this signed-in account yet. Completed storefront purchases will appear here once live account history is available."
                    } else {
                        "This browser session has no completed orders yet. Orders placed from this browser will appear here automatically after checkout."
                    },
                ),
            )?
            .with_value(
                "membershipEmptyText",
                RenderValue::text(
                    if principal_id.is_some() {
                        "No active membership is attached to this signed-in account yet. Join from the storefront to unlock early access and renewal visibility."
                    } else {
                        "No active membership is attached to this browser session yet. A qualifying membership purchase completed here will appear after payment capture."
                    },
                ),
            )?
            .with_value("ordersCtaUrl", RenderValue::text(orders_cta_url))?
            .with_value("ordersCtaLabel", RenderValue::text(orders_cta_label))?
            .with_value(
                "membershipCtaUrl",
                RenderValue::text(localized_collection_path(locale, "memberships")),
            )?
            .with_value(
                "latestOrderReference",
                RenderValue::text(latest_order_reference),
            )?
            .with_value("latestOrderStatus", RenderValue::text(latest_order_status))?,
        customer: RenderModel::new()
            .with_value("displayName", RenderValue::text(display_name))?
            .with_value("email", RenderValue::text(email))?,
        recent_orders,
        membership_summary: membership_summary.unwrap_or(empty_membership_summary()?),
    })
}

fn recent_orders_from_storefront(
    snapshot: Option<&StorefrontStateSnapshot>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    let Some(snapshot) = snapshot else {
        return Ok(Vec::new());
    };

    snapshot
        .recent_orders
        .iter()
        .map(account_order_from_storefront)
        .collect::<Result<Vec<_>, _>>()
}

fn membership_summary_from_storefront(
    snapshot: Option<&StorefrontStateSnapshot>,
    include_pending_membership: bool,
) -> Result<Option<RenderModel>, TemplateModelError> {
    let Some(snapshot) = snapshot else {
        return Ok(None);
    };

    let Some(projected) = projected_membership_state(snapshot, include_pending_membership)? else {
        return Ok(None);
    };

    membership_summary(
        &projected.tier_name,
        projected.status_label(),
        &projected.renewal_text,
    )
    .map(Some)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectedMembershipState {
    tier_name: String,
    status: SubscriptionStatus,
    renewal_text: String,
}

impl ProjectedMembershipState {
    fn status_label(&self) -> &'static str {
        match self.status {
            SubscriptionStatus::PendingActivation => "Pending activation",
            SubscriptionStatus::Active => "Active",
            SubscriptionStatus::InGracePeriod => "In grace period",
            SubscriptionStatus::Paused => "Paused",
            SubscriptionStatus::Cancelled => "Cancelled",
            SubscriptionStatus::Expired => "Expired",
        }
    }
}

fn projected_membership_state(
    snapshot: &StorefrontStateSnapshot,
    include_pending_membership: bool,
) -> Result<Option<ProjectedMembershipState>, TemplateModelError> {
    if let Some(active) = projected_membership_state_for_statuses(snapshot, &["paid", "fulfilled"])?
    {
        return Ok(Some(active));
    }

    if include_pending_membership {
        return projected_membership_state_for_statuses(snapshot, &["pending_payment"]);
    }

    Ok(None)
}

fn projected_membership_state_for_statuses(
    snapshot: &StorefrontStateSnapshot,
    eligible_statuses: &[&str],
) -> Result<Option<ProjectedMembershipState>, TemplateModelError> {
    for order in &snapshot.recent_orders {
        if !eligible_statuses
            .iter()
            .any(|status| order.status.as_str() == *status)
        {
            continue;
        }
        if let Some(projected) = projected_membership_state_for_order(snapshot, order)? {
            return Ok(Some(projected));
        }
    }
    Ok(None)
}

fn projected_membership_state_for_order(
    snapshot: &StorefrontStateSnapshot,
    order: &StorefrontOrderSnapshot,
) -> Result<Option<ProjectedMembershipState>, TemplateModelError> {
    let outcomes = storefront_membership_outcomes(order)?;
    if outcomes.is_empty() {
        return Ok(None);
    }

    let order_id = OrderId::new(order.order_id.clone()).map_err(template_commerce_error)?;
    let member_id = storefront_member_account_id(snapshot)?;
    let starts_at = MembershipInstant::from_unix_seconds(order.created_at_unix_seconds);
    let catalog = storefront_membership_catalog(order)?;
    let mut provisioned = catalog
        .provision_from_order_outcomes(order_id, member_id, &outcomes, starts_at)
        .map_err(template_membership_error)?;
    let Some(mut provisioned) = provisioned.drain(..).next() else {
        return Ok(None);
    };

    if matches!(order.status.as_str(), "paid" | "fulfilled") {
        provisioned
            .subscription
            .activate(starts_at)
            .map_err(template_membership_error)?;
    }

    let tier_name = catalog
        .tier(&provisioned.subscription.tier_id)
        .map(|tier| tier.title.clone())
        .unwrap_or_else(|| {
            order
                .lines
                .iter()
                .find(|line| line.product_kind == "membership")
                .map(|line| line.title.clone())
                .unwrap_or_else(|| "Membership".to_string())
        });
    let renewal_text = match provisioned.subscription.status {
        SubscriptionStatus::PendingActivation => format!(
            "Included with order {}. Membership access will activate automatically after payment capture for this order.",
            order.order_id
        ),
        SubscriptionStatus::Active => format!(
            "Activated from order {}. Membership access is live for this account view.",
            order.order_id
        ),
        SubscriptionStatus::InGracePeriod => format!(
            "Membership from order {} is in its grace period while renewal is resolved.",
            order.order_id
        ),
        SubscriptionStatus::Paused => format!(
            "Membership from order {} is currently paused.",
            order.order_id
        ),
        SubscriptionStatus::Cancelled => format!(
            "Membership from order {} has been cancelled.",
            order.order_id
        ),
        SubscriptionStatus::Expired => {
            format!("Membership from order {} has expired.", order.order_id)
        }
    };

    Ok(Some(ProjectedMembershipState {
        tier_name,
        status: provisioned.subscription.status,
        renewal_text,
    }))
}

fn storefront_membership_catalog(
    order: &StorefrontOrderSnapshot,
) -> Result<MembershipCatalog, TemplateModelError> {
    let mut catalog = MembershipCatalog::new();
    for line in &order.lines {
        if line.product_kind != "membership" {
            continue;
        }
        let Some(entitlement_key) = line.entitlement_key.as_deref() else {
            continue;
        };
        let tier = MembershipTier::new(
            MembershipTierId::new(format!(
                "tier-{}",
                sanitize_membership_token(entitlement_key)
            ))
            .map_err(template_membership_error)?,
            line.title.clone(),
            EntitlementKey::new(entitlement_key.to_string()).map_err(template_commerce_error)?,
            10,
            infer_membership_interval(&line.variant_title),
            0,
            TierVisibility::Public,
            Vec::new(),
        )
        .map_err(template_membership_error)?;
        if catalog
            .tier_for_entitlement(&tier.entitlement_key)
            .is_none()
        {
            catalog
                .register_tier(tier)
                .map_err(template_membership_error)?;
        }
    }
    Ok(catalog)
}

fn storefront_membership_outcomes(
    order: &StorefrontOrderSnapshot,
) -> Result<Vec<davenda_commerce::OrderOutcome>, TemplateModelError> {
    order
        .lines
        .iter()
        .filter(|line| line.product_kind == "membership")
        .filter_map(|line| {
            line.entitlement_key.as_deref().map(|entitlement_key| {
                EntitlementKey::new(entitlement_key.to_string())
                    .map(
                        |entitlement_key| davenda_commerce::OrderOutcome::GrantMembership {
                            entitlement_key,
                            quantity: line.quantity,
                        },
                    )
                    .map_err(template_commerce_error)
            })
        })
        .collect()
}

fn storefront_member_account_id(
    snapshot: &StorefrontStateSnapshot,
) -> Result<MemberAccountId, TemplateModelError> {
    let raw = snapshot
        .principal_id
        .clone()
        .unwrap_or_else(|| format!("session-{}", snapshot.session_id));
    MemberAccountId::new(sanitize_membership_token(&raw)).map_err(template_membership_error)
}

fn sanitize_membership_token(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn infer_membership_interval(variant_title: &str) -> BillingInterval {
    let normalized = variant_title.to_ascii_lowercase();
    if normalized.contains("month") {
        BillingInterval::Monthly
    } else if normalized.contains("quarter") {
        BillingInterval::Quarterly
    } else {
        BillingInterval::Annual
    }
}

fn account_state_summary(base: &str, latest_order: Option<&StorefrontOrderSnapshot>) -> String {
    match latest_order {
        Some(order) => format!(
            "{base} Latest order {} is currently {}.",
            order.order_id,
            display_status_label(&order.status)
        ),
        None => base.to_string(),
    }
}

fn display_status_label(status: &str) -> String {
    status
        .split(|ch: char| matches!(ch, '-' | '_' | ' '))
        .filter(|segment| !segment.is_empty())
        .map(capitalize_token)
        .collect::<Vec<_>>()
        .join(" ")
}

fn looks_like_email(candidate: &str) -> bool {
    matches!(candidate.split_once('@'), Some((local, domain)) if !local.is_empty() && !domain.is_empty())
}

fn display_name_from_principal_id(principal_id: &str) -> String {
    let base = principal_id
        .split_once('@')
        .map(|(local, _)| local)
        .unwrap_or(principal_id);
    let words = base
        .split(|ch: char| matches!(ch, '-' | '_' | '.' | '+' | '/'))
        .filter(|segment| !segment.is_empty())
        .map(capitalize_token)
        .collect::<Vec<_>>();
    if words.is_empty() {
        "Member Account".to_string()
    } else {
        words.join(" ")
    }
}

fn capitalize_token(segment: &str) -> String {
    let mut chars = segment.chars();
    match chars.next() {
        Some(first) => {
            let mut word = first.to_uppercase().collect::<String>();
            word.push_str(chars.as_str());
            word
        }
        None => String::new(),
    }
}

#[derive(Clone)]
struct StorefrontFixture {
    catalog_sections: Vec<RenderModel>,
    product_cards: Vec<RenderModel>,
    product_cards_by_collection: BTreeMap<String, Vec<RenderModel>>,
    product_collection_handles: BTreeMap<String, String>,
    cart_items: Vec<RenderModel>,
    cart_summary: RenderModel,
    checkout: RenderModel,
    confirmation: RenderModel,
    customer: RenderModel,
    recent_orders: Vec<RenderModel>,
    membership_summary: RenderModel,
    collections: BTreeMap<String, RenderModel>,
    products: BTreeMap<String, RenderModel>,
}

impl StorefrontFixture {
    fn collection_for(&self, handle: &str) -> RenderModel {
        self.collections
            .get(handle)
            .cloned()
            .unwrap_or_else(|| self.collections["featured"].clone())
    }

    fn product_for(&self, handle: &str) -> RenderModel {
        self.products
            .get(handle)
            .cloned()
            .unwrap_or_else(|| self.products["harbor-cap"].clone())
    }

    fn product_cards_for_collection(&self, handle: &str) -> Vec<RenderModel> {
        if handle == "featured" {
            return self.product_cards.clone();
        }

        self.product_cards_by_collection
            .get(handle)
            .cloned()
            .unwrap_or_default()
    }

    fn related_product_cards_for_product(&self, handle: &str) -> Vec<RenderModel> {
        let collection_handle = self
            .product_collection_handles
            .get(handle)
            .map(String::as_str)
            .unwrap_or("featured");
        self.product_cards_for_collection(collection_handle)
    }
}

fn product_cards_by_collection(
    locale: &str,
    products: &[ProductFixture],
    plan: Option<&RuntimePlan>,
) -> Result<BTreeMap<String, Vec<RenderModel>>, TemplateModelError> {
    let mut grouped: BTreeMap<String, Vec<RenderModel>> = BTreeMap::new();
    for product in products {
        grouped
            .entry(product.collection_handle.to_string())
            .or_default()
            .push(product_model_for_locale(locale, product, plan)?);
    }
    Ok(grouped)
}

fn storefront_fixture(
    locale: &str,
    catalog: &StorefrontCatalog,
    plan: Option<&RuntimePlan>,
) -> Result<StorefrontFixture, TemplateModelError> {
    let products_data = catalog
        .products
        .iter()
        .filter(|product| catalog.visible_product(product.handle.as_str()).is_some())
        .map(|product| ProductFixture {
            handle: product.handle.clone(),
            title: product.title.clone(),
            summary: product.summary.clone(),
            price: money_display_minor(product.price_minor, &product.currency),
            collection_handle: product.collection_handle.clone(),
            collection_name: catalog
                .collection(&product.collection_handle)
                .map(|collection| collection.title.clone())
                .unwrap_or_else(|| "Collection".to_string()),
        })
        .collect::<Vec<_>>();
    let product_cards = products_data
        .iter()
        .map(|product| product_model_for_locale(locale, product, plan))
        .collect::<Result<Vec<_>, _>>()?;
    let product_cards_by_collection = product_cards_by_collection(locale, &products_data, plan)?;
    let product_collection_handles = products_data
        .iter()
        .map(|product| {
            (
                product.handle.to_string(),
                product.collection_handle.to_string(),
            )
        })
        .collect();

    let collections_data = catalog
        .collections
        .iter()
        .filter(|collection| collection.is_visible)
        .map(|collection| CollectionFixture {
            handle: collection.handle.clone(),
            title: collection.title.clone(),
            href: localized_collection_path(locale, &collection.handle),
            summary: collection.summary.clone(),
            label: collection.label.clone(),
        })
        .collect::<Vec<_>>();
    let catalog_sections = collections_data
        .iter()
        .map(|collection| collection_section_model(locale, collection))
        .collect::<Result<Vec<_>, _>>()?;
    let collections = collections_data
        .iter()
        .map(|collection| collection_detail_model(locale, collection, &products_data))
        .collect::<Result<Vec<_>, _>>()?;

    let current_order = sample_completed_order();
    let previous_order = sample_previous_order();

    let cart_items = current_order
        .lines
        .iter()
        .map(|line| cart_item_from_line(catalog, locale, line))
        .collect::<Result<Vec<_>, _>>()?;
    let cart_summary = RenderModel::new()
        .with_value(
            "subtotal",
            RenderValue::text(money_display(&current_order.totals.subtotal)),
        )?
        .with_value("shipping", RenderValue::text("£0.00"))?
        .with_value(
            "total",
            RenderValue::text(money_display(&current_order.totals.total)),
        )?;
    let checkout = RenderModel::new()
        .with_value("paymentReference", RenderValue::text("card-on-file"))?
        .with_value("paymentMethod", RenderValue::text("card"))?
        .with_value("paymentMethodLabel", RenderValue::text("Card"))?
        .with_value("paymentStatus", RenderValue::text("ready_for_payment"))?
        .with_value("paymentStatusLabel", RenderValue::text("Ready for payment"))?
        .with_value(
            "providerCode",
            RenderValue::text(payment_provider_code(None)),
        )?
        .with_value(
            "providerLabel",
            RenderValue::text(payment_provider_label(None).to_string()),
        )?
        .with_value(
            "providerSummary",
            RenderValue::text(payment_provider_summary(None)),
        )?
        .with_value("submitLabel", RenderValue::text("Place order"))?
        .with_value("checkoutEmail", RenderValue::text("member@example.com"))?
        .with_bool("hasCheckoutEmail", true)?
        .with_value("paymentLast4", RenderValue::text("4242"))?
        .with_bool("hasPaymentReference", true)?
        .with_bool("hasPaymentLast4", true)?;

    let confirmation = confirmation_model(&current_order)?;

    let customer = RenderModel::new()
        .with_value("displayName", RenderValue::text("Alex Mariner"))?
        .with_value("email", RenderValue::text("member@example.com"))?;

    let recent_orders = vec![
        account_order_from_order(&current_order)?,
        account_order_from_order(&previous_order)?,
    ];
    let membership_summary = membership_summary("Harbor Circle", "Active", "Renews on 18 April")?;

    Ok(StorefrontFixture {
        catalog_sections,
        product_cards: product_cards.clone(),
        product_cards_by_collection,
        product_collection_handles,
        cart_items,
        cart_summary,
        checkout,
        confirmation,
        customer,
        recent_orders,
        membership_summary,
        collections: collections
            .into_iter()
            .zip(
                collections_data
                    .iter()
                    .map(|collection| collection.handle.to_string()),
            )
            .map(|(collection, handle)| (handle, collection))
            .collect(),
        products: product_cards
            .into_iter()
            .zip(
                products_data
                    .iter()
                    .map(|product| product.handle.to_string()),
            )
            .map(|(product, handle)| (handle, product))
            .collect(),
    })
}

fn catalog_admin_form_model(
    form_state: Option<&StorefrontFormState>,
) -> Result<RenderModel, TemplateModelError> {
    let errors = form_errors_model(form_state)?;
    let has_errors = !errors.is_empty();
    RenderModel::new()
        .with_bool("hasErrors", has_errors)?
        .with_value(
            "errorSummary",
            RenderValue::text(
                form_state
                    .map(|state| state.summary.clone())
                    .unwrap_or_else(|| {
                        "Fix the highlighted catalog fields and save again.".to_string()
                    }),
            ),
        )?
        .with_list("errors", errors)
}

fn catalog_admin_products_model(
    locale: &str,
    catalog: &StorefrontCatalog,
    plan: Option<&RuntimePlan>,
    form_state: Option<&StorefrontFormState>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    catalog
        .products
        .iter()
        .map(|product| {
            let collection_name = catalog
                .collection(product.collection_handle.as_str())
                .map(|collection| collection.title.clone())
                .unwrap_or_else(|| "Collection".to_string());
            let fixture = ProductFixture {
                handle: product.handle.clone(),
                title: product.title.clone(),
                summary: product.summary.clone(),
                price: money_display_minor(product.price_minor, &product.currency),
                collection_handle: product.collection_handle.clone(),
                collection_name,
            };
            let is_active_form = catalog_admin_form_targets_product(form_state, &product.handle);
            let title_error = catalog_admin_form_error(form_state, is_active_form, "product_title");
            let summary_error =
                catalog_admin_form_error(form_state, is_active_form, "product_summary");
            let price_error = catalog_admin_form_error(form_state, is_active_form, "product_price");
            let collection_error =
                catalog_admin_form_error(form_state, is_active_form, "product_collection_handle");
            let visibility_input = catalog_admin_checkbox_value(
                form_state,
                is_active_form,
                "product_visible",
                product.is_visible,
            );
            product_model_for_locale(locale, &fixture, plan)?
                .with_bool("isVisible", product.is_visible)?
                .with_bool("visibilityInput", visibility_input)?
                .with_value(
                    "visibilityLabel",
                    RenderValue::text(if product.is_visible {
                        "Visible in storefront"
                    } else {
                        "Hidden from storefront"
                    }),
                )?
                .with_value(
                    "titleInput",
                    RenderValue::text(catalog_admin_form_value(
                        form_state,
                        is_active_form,
                        "product_title",
                        &product.title,
                    )),
                )?
                .with_value(
                    "summaryInput",
                    RenderValue::text(catalog_admin_form_value(
                        form_state,
                        is_active_form,
                        "product_summary",
                        &product.summary,
                    )),
                )?
                .with_value(
                    "priceInput",
                    RenderValue::text(catalog_admin_form_value(
                        form_state,
                        is_active_form,
                        "product_price",
                        &decimal_money_input_minor(product.price_minor),
                    )),
                )?
                .with_value(
                    "collectionHandleInput",
                    RenderValue::text(catalog_admin_form_value(
                        form_state,
                        is_active_form,
                        "product_collection_handle",
                        &product.collection_handle,
                    )),
                )?
                .with_bool("hasTitleError", title_error.is_some())?
                .with_value(
                    "titleError",
                    RenderValue::text(title_error.unwrap_or_default()),
                )?
                .with_bool("hasSummaryError", summary_error.is_some())?
                .with_value(
                    "summaryError",
                    RenderValue::text(summary_error.unwrap_or_default()),
                )?
                .with_bool("hasPriceError", price_error.is_some())?
                .with_value(
                    "priceError",
                    RenderValue::text(price_error.unwrap_or_default()),
                )?
                .with_bool("hasCollectionError", collection_error.is_some())?
                .with_value(
                    "collectionError",
                    RenderValue::text(collection_error.unwrap_or_default()),
                )?
                .with_list(
                    "collectionOptions",
                    catalog_admin_collection_options(
                        catalog,
                        catalog_admin_form_value(
                            form_state,
                            is_active_form,
                            "product_collection_handle",
                            &product.collection_handle,
                        )
                        .as_str(),
                    )?,
                )
        })
        .collect()
}

fn catalog_admin_collections_model(
    locale: &str,
    catalog: &StorefrontCatalog,
    form_state: Option<&StorefrontFormState>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    catalog
        .collections
        .iter()
        .map(|collection| {
            let fixture = CollectionFixture {
                handle: collection.handle.clone(),
                title: collection.title.clone(),
                href: localized_collection_path(locale, &collection.handle),
                summary: collection.summary.clone(),
                label: collection.label.clone(),
            };
            let is_active_form =
                catalog_admin_form_targets_collection(form_state, &collection.handle);
            let title_error =
                catalog_admin_form_error(form_state, is_active_form, "collection_title");
            let label_error =
                catalog_admin_form_error(form_state, is_active_form, "collection_label");
            let summary_error =
                catalog_admin_form_error(form_state, is_active_form, "collection_summary");
            let visibility_input = catalog_admin_checkbox_value(
                form_state,
                is_active_form,
                "collection_visible",
                collection.is_visible,
            );
            collection_section_model(locale, &fixture)?
                .with_bool("isVisible", collection.is_visible)?
                .with_bool("visibilityInput", visibility_input)?
                .with_value(
                    "visibilityLabel",
                    RenderValue::text(if collection.is_visible {
                        "Visible in storefront"
                    } else {
                        "Hidden from storefront"
                    }),
                )?
                .with_value(
                    "titleInput",
                    RenderValue::text(catalog_admin_form_value(
                        form_state,
                        is_active_form,
                        "collection_title",
                        &collection.title,
                    )),
                )?
                .with_value(
                    "labelInput",
                    RenderValue::text(catalog_admin_form_value(
                        form_state,
                        is_active_form,
                        "collection_label",
                        &collection.label,
                    )),
                )?
                .with_value(
                    "summaryInput",
                    RenderValue::text(catalog_admin_form_value(
                        form_state,
                        is_active_form,
                        "collection_summary",
                        &collection.summary,
                    )),
                )?
                .with_bool("hasTitleError", title_error.is_some())?
                .with_value(
                    "titleError",
                    RenderValue::text(title_error.unwrap_or_default()),
                )?
                .with_bool("hasLabelError", label_error.is_some())?
                .with_value(
                    "labelError",
                    RenderValue::text(label_error.unwrap_or_default()),
                )?
                .with_bool("hasSummaryError", summary_error.is_some())?
                .with_value(
                    "summaryError",
                    RenderValue::text(summary_error.unwrap_or_default()),
                )
        })
        .collect()
}

fn catalog_admin_collection_options(
    catalog: &StorefrontCatalog,
    selected_handle: &str,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    catalog
        .collections
        .iter()
        .map(|collection| {
            RenderModel::new()
                .with_value("handle", RenderValue::text(collection.handle.clone()))?
                .with_value("title", RenderValue::text(collection.title.clone()))?
                .with_bool("selected", collection.handle == selected_handle)
        })
        .collect()
}

fn catalog_admin_form_targets_product(
    form_state: Option<&StorefrontFormState>,
    handle: &str,
) -> bool {
    catalog_admin_form_matches(form_state, "product", "product_handle", handle)
}

fn catalog_admin_form_targets_collection(
    form_state: Option<&StorefrontFormState>,
    handle: &str,
) -> bool {
    catalog_admin_form_matches(form_state, "collection", "collection_handle", handle)
}

fn catalog_admin_form_matches(
    form_state: Option<&StorefrontFormState>,
    entity: &str,
    handle_field: &str,
    handle: &str,
) -> bool {
    form_state.is_some_and(|state| {
        state.fields.get("catalog_entity").map(String::as_str) == Some(entity)
            && state.fields.get(handle_field).map(String::as_str) == Some(handle)
    })
}

fn catalog_admin_form_value(
    form_state: Option<&StorefrontFormState>,
    is_active_form: bool,
    field: &str,
    default: &str,
) -> String {
    if !is_active_form {
        return default.to_string();
    }
    form_state
        .and_then(|state| state.fields.get(field))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

fn catalog_admin_checkbox_value(
    form_state: Option<&StorefrontFormState>,
    is_active_form: bool,
    field: &str,
    default: bool,
) -> bool {
    if !is_active_form {
        return default;
    }
    form_state
        .and_then(|state| state.fields.get(field))
        .is_some_and(|value| value == "yes")
}

fn catalog_admin_form_error(
    form_state: Option<&StorefrontFormState>,
    is_active_form: bool,
    field: &str,
) -> Option<String> {
    if !is_active_form {
        return None;
    }
    form_state.and_then(|state| state.field_errors.get(field).cloned())
}

struct CollectionFixture {
    handle: String,
    title: String,
    href: String,
    summary: String,
    label: String,
}

struct ProductFixture {
    handle: String,
    title: String,
    summary: String,
    price: String,
    collection_handle: String,
    collection_name: String,
}

fn collection_section_model(
    locale: &str,
    collection: &CollectionFixture,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("handle", RenderValue::text(collection.handle.as_str()))?
        .with_value("label", RenderValue::text(collection.label.as_str()))?
        .with_value("title", RenderValue::text(collection.title.as_str()))?
        .with_value("summary", RenderValue::text(collection.summary.as_str()))?
        .with_value(
            "url",
            RenderValue::text(localized_collection_path(
                locale,
                collection.handle.as_str(),
            )),
        )
}

fn collection_detail_model(
    locale: &str,
    collection: &CollectionFixture,
    products: &[ProductFixture],
) -> Result<RenderModel, TemplateModelError> {
    let filtered_products = products
        .iter()
        .filter(|product| {
            product.collection_handle == collection.handle || collection.handle == "featured"
        })
        .map(|product| product_model_for_locale(locale, product, None))
        .collect::<Vec<_>>();
    let filtered_products = filtered_products
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    RenderModel::new()
        .with_value("title", RenderValue::text(collection.title.as_str()))?
        .with_value("summary", RenderValue::text(collection.summary.as_str()))?
        .with_value(
            "url",
            RenderValue::text(localized_collection_path(
                locale,
                collection.handle.as_str(),
            )),
        )?
        .with_list("products", filtered_products)
}

fn product_model(product: &ProductFixture) -> Result<RenderModel, TemplateModelError> {
    product_model_for_locale("en-GB", product, None)
}

fn product_model_for_locale(
    locale: &str,
    product: &ProductFixture,
    plan: Option<&RuntimePlan>,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("handle", RenderValue::text(product.handle.as_str()))?
        .with_value("slug", RenderValue::text(product.handle.as_str()))?
        .with_value("sku", RenderValue::text(product.handle.as_str()))?
        .with_value("name", RenderValue::text(product.title.as_str()))?
        .with_value("summary", RenderValue::text(product.summary.as_str()))?
        .with_value("price", RenderValue::text(product.price.as_str()))?
        .with_value(
            "url",
            RenderValue::text(localized_product_path(locale, product.handle.as_str())),
        )?
        .with_value("addToCartUrl", RenderValue::text("/cart/items"))?
        .with_value(
            "imageUrl",
            RenderValue::text(theme_asset_url(plan, "theme/assets/logo.svg")),
        )?
        .with_value("imageAlt", RenderValue::text(product.title.as_str()))?
        .with_value(
            "collectionHandle",
            RenderValue::text(product.collection_handle.as_str()),
        )?
        .with_value(
            "collectionUrl",
            RenderValue::text(localized_collection_path(
                locale,
                product.collection_handle.as_str(),
            )),
        )?
        .with_value(
            "collectionName",
            RenderValue::text(product.collection_name.as_str()),
        )
}

fn theme_asset_url(plan: Option<&RuntimePlan>, logical_path: &str) -> String {
    plan.and_then(|runtime| runtime.theme_asset_manifest.as_ref())
        .and_then(|manifest| manifest.resolve(logical_path))
        .and_then(|published| match published.delivery().target() {
            AssetDeliveryTarget::Cdn { public_url, .. } => Some(public_url.clone()),
            _ => None,
        })
        .unwrap_or_else(|| format!("/{logical_path}"))
}

fn cart_item(
    title: &str,
    variant: &str,
    quantity: &str,
    total: &str,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("title", RenderValue::text(title))?
        .with_value("variant", RenderValue::text(variant))?
        .with_value("quantity", RenderValue::text(quantity))?
        .with_value(
            "quantityField",
            RenderValue::text(format!(
                "quantity_{}",
                title.to_lowercase().replace(' ', "-")
            )),
        )?
        .with_value("total", RenderValue::text(total))
}

fn account_order(
    reference: &str,
    total: &str,
    status: &str,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("reference", RenderValue::text(reference))?
        .with_value("total", RenderValue::text(total))?
        .with_value("status", RenderValue::text(status))
}

fn cart_item_from_line(
    catalog: &StorefrontCatalog,
    locale: &str,
    line: &CheckoutLine,
) -> Result<RenderModel, TemplateModelError> {
    let model = cart_item(
        &line.product_title,
        &line.variant_title,
        &line.quantity.to_string(),
        &money_display(&line.subtotal().expect("sample checkout line is valid")),
    )?;
    decorate_cart_item_with_catalog_context(
        model,
        catalog,
        locale,
        line.sku.as_str(),
        &line.product_title,
    )
}

fn confirmation_model(order: &Order) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("orderNumber", RenderValue::text(order.id.to_string()))?
        .with_value("email", RenderValue::text("member@example.com"))?
        .with_bool("hasEmail", true)?
        .with_value("nextStep", RenderValue::text(order.confirmation_message()))
        .and_then(|model| {
            model.with_value("status", RenderValue::text(order.history_status_label()))
        })
        .and_then(|model| {
            model.with_value(
                "subtotal",
                RenderValue::text(money_display(&order.totals.subtotal)),
            )
        })
        .and_then(|model| model.with_value("total", RenderValue::text(order.display_total())))
        .and_then(|model| model.with_value("paymentStatus", RenderValue::text("Captured")))
        .and_then(|model| model.with_value("paymentMethod", RenderValue::text("Card")))
        .and_then(|model| model.with_value("paymentReference", RenderValue::text("PAY-50001")))
        .and_then(|model| model.with_value("paymentLast4", RenderValue::text("4242")))
        .and_then(|model| {
            model.with_value(
                "paymentSummary",
                RenderValue::text("Card ending 4242, reference PAY-50001"),
            )
        })
        .and_then(|model| {
            model.with_value(
                "providerLabel",
                RenderValue::text(payment_provider_label(None).to_string()),
            )
        })
        .and_then(|model| model.with_bool("hasPaymentLast4", true))
        .and_then(|model| model.with_bool("hasPaymentReference", true))
        .and_then(|model| {
            model.with_bool(
                "hasMembershipItems",
                order.outcomes().iter().any(|outcome| {
                    matches!(
                        outcome,
                        davenda_commerce::OrderOutcome::GrantMembership {
                            entitlement_key: _,
                            quantity: _
                        }
                    )
                }),
            )
        })
        .and_then(|model| model.with_bool("hasLineItems", !order.lines.is_empty()))
        .and_then(|model| {
            model.with_list(
                "lineItems",
                order
                    .lines
                    .iter()
                    .map(|line| {
                        RenderModel::new()
                            .with_value("title", RenderValue::text(line.product_title.clone()))?
                            .with_value("quantity", RenderValue::text(line.quantity.to_string()))?
                            .with_value(
                                "total",
                                RenderValue::text(money_display(
                                    &line.subtotal().expect("fixture confirmation subtotal"),
                                )),
                            )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )
        })
}

fn account_order_from_order(order: &Order) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("reference", RenderValue::text(order.id.to_string()))?
        .with_value("total", RenderValue::text(order.display_total()))?
        .with_value("status", RenderValue::text(order.history_status_label()))
}

fn membership_summary(
    tier_name: &str,
    status: &str,
    renewal_text: &str,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("tierName", RenderValue::text(tier_name))?
        .with_value("status", RenderValue::text(status))?
        .with_value("renewalText", RenderValue::text(renewal_text))
}

fn sample_completed_order() -> Order {
    let currency = CurrencyCode::new("GBP").unwrap();
    let pricing = PricingPolicy::new(currency.clone());
    let mut checkout =
        CheckoutSession::new(CheckoutId::new("chk-10042").unwrap(), currency.clone());
    checkout
        .add_line(
            CheckoutLine::new(
                ProductId::new("product-harbor-cap").unwrap(),
                ProductKind::Physical,
                "Harbor Cap",
                Sku::new("sku-harbor-cap").unwrap(),
                "Canvas cap",
                1,
                Money::new(currency.clone(), 2_900).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    checkout
        .add_line(
            CheckoutLine::new(
                ProductId::new("product-gold-membership").unwrap(),
                ProductKind::Membership {
                    entitlement_key: EntitlementKey::new("membership.gold").unwrap(),
                },
                "Gold Membership",
                Sku::new("sku-gold-membership").unwrap(),
                "Annual plan",
                1,
                Money::new(currency.clone(), 8_900).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    checkout.ready_for_payment().unwrap();
    checkout.awaiting_payment().unwrap();
    checkout.mark_paid().unwrap();
    checkout
        .finalize(OrderId::new("ORD-10042").unwrap(), &pricing)
        .unwrap()
}

fn sample_previous_order() -> Order {
    let currency = CurrencyCode::new("GBP").unwrap();
    let pricing = PricingPolicy::new(currency.clone());
    let mut checkout = CheckoutSession::new(CheckoutId::new("chk-0998").unwrap(), currency.clone());
    checkout
        .add_line(
            CheckoutLine::new(
                ProductId::new("product-spring-tasting-pass").unwrap(),
                ProductKind::Service,
                "Spring Tasting Pass",
                Sku::new("sku-tasting-pass").unwrap(),
                "Single event pass",
                1,
                Money::new(currency.clone(), 4_500).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    checkout.ready_for_payment().unwrap();
    checkout.awaiting_payment().unwrap();
    checkout.mark_paid().unwrap();
    let mut order = checkout
        .finalize(OrderId::new("ORD-0998").unwrap(), &pricing)
        .unwrap();
    order.fulfill().unwrap();
    order
}

fn money_display(money: &Money) -> String {
    money_display_minor(money.amount_minor(), money.currency().as_str())
}

fn decimal_money_input_minor(amount_minor: i64) -> String {
    let major = amount_minor / 100;
    let remainder = amount_minor.abs() % 100;
    format!("{major}.{remainder:02}")
}

fn money_display_minor(amount_minor: i64, currency: &str) -> String {
    let major = amount_minor / 100;
    let remainder = amount_minor % 100;
    match currency {
        "GBP" => format!("£{major}.{remainder:02}"),
        code => format!("{code} {major}.{remainder:02}"),
    }
}

fn empty_membership_summary() -> Result<RenderModel, TemplateModelError> {
    membership_summary(
        "Membership unavailable",
        "Not active",
        "Join from the storefront to manage renewals and entitlements here.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::RuntimeBuilder;
    use davenda_auth::DefaultAuthModelPackage;
    use davenda_config::PlatformConfig;
    use davenda_customer_sdk::{
        AuditFacade, AuthFacade, BackendError, CheckoutHooks, CommerceFacade,
        CustomerBackendPlugin, CustomerHookRegistry, CustomerPluginDescriptor, OrderDraft,
        OrderReviewDecision, RequestContext,
    };
    use davenda_template::{
        DocumentRenderRequest, TemplateName, TemplateNamespace, TemplateRegistry, TemplateRuntime,
        TemplateSelector, TemplateSourceParser,
    };
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    const RENDER_TEST_CONFIG: &str = r#"
[app]
name = "showcase-events"
environment = "production"

[server]
bind = "0.0.0.0:8080"
trusted_proxies = ["10.0.0.0/8"]

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
mode = "acme"
challenge = "dns-01"
provider = "cloudflare-dns"

[storage]
default_class = "public_upload"
single_node_escape_hatch = "explicit_single_node"
object_store = "s3"
object_store_secret = { kind = "env", var = "OBJECT_STORE_URL" }
local_root = "/tmp/davenda-runtime-tests"
deployment = "single_node"

[cache]
l1 = "moka"
l2 = "redis"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]
fallback_locale = "en-GB"
localized_routes = true

[seo]
canonical_host = "www.example.com"
emit_json_ld = true

[auth]
package = "platform-default-auth"
explain_api = false
tenant_id = 101

[modules]
enabled = ["cms-pages", "admin-shell"]

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
"#;

    #[derive(Debug)]
    struct NoteRecordingCheckoutPlugin;

    #[derive(Debug)]
    struct NoteRecordingCheckoutHooks;

    impl CheckoutHooks for NoteRecordingCheckoutHooks {
        fn review_order(
            &self,
            _ctx: &RequestContext,
            order: &OrderDraft,
            commerce: &dyn CommerceFacade,
            _auth: &dyn AuthFacade,
            _audit: &dyn AuditFacade,
        ) -> Result<OrderReviewDecision, BackendError> {
            commerce.add_order_note(&order.order_id, "Flag for finance follow-up")?;
            commerce.add_order_note(&order.order_id, "Flag for finance follow-up")?;
            Ok(OrderReviewDecision::approved())
        }
    }

    impl CustomerBackendPlugin for NoteRecordingCheckoutPlugin {
        fn descriptor(&self) -> CustomerPluginDescriptor {
            CustomerPluginDescriptor::new(
                "render-order-note-recorder",
                "Render Order Note Recorder",
                "0.1.0",
            )
        }

        fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
            registry.register_checkout_hooks(Arc::new(NoteRecordingCheckoutHooks))
        }
    }

    #[derive(Debug)]
    struct WrongOrderNoteCheckoutPlugin;

    #[derive(Debug)]
    struct WrongOrderNoteCheckoutHooks;

    #[derive(Debug)]
    struct MetadataReplayCheckoutPlugin;

    #[derive(Debug)]
    struct MetadataReplayCheckoutHooks;

    impl CheckoutHooks for WrongOrderNoteCheckoutHooks {
        fn review_order(
            &self,
            _ctx: &RequestContext,
            _order: &OrderDraft,
            commerce: &dyn CommerceFacade,
            _auth: &dyn AuthFacade,
            _audit: &dyn AuditFacade,
        ) -> Result<OrderReviewDecision, BackendError> {
            commerce.add_order_note("ORD-OTHER", "This should fail closed")?;
            Ok(OrderReviewDecision::approved())
        }
    }

    impl CustomerBackendPlugin for WrongOrderNoteCheckoutPlugin {
        fn descriptor(&self) -> CustomerPluginDescriptor {
            CustomerPluginDescriptor::new(
                "render-order-note-mismatch",
                "Render Order Note Mismatch",
                "0.1.0",
            )
        }

        fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
            registry.register_checkout_hooks(Arc::new(WrongOrderNoteCheckoutHooks))
        }
    }

    impl CheckoutHooks for MetadataReplayCheckoutHooks {
        fn review_order(
            &self,
            _ctx: &RequestContext,
            order: &OrderDraft,
            _commerce: &dyn CommerceFacade,
            _auth: &dyn AuthFacade,
            _audit: &dyn AuditFacade,
        ) -> Result<OrderReviewDecision, BackendError> {
            let membership_tier = order.metadata.get("membership_tier").map(String::as_str);
            let shipping_country = order.metadata.get("shipping_country").map(String::as_str);
            let order_principal_id = order.metadata.get("order_principal_id").map(String::as_str);
            if membership_tier != Some("gold")
                || shipping_country != Some("GB")
                || order_principal_id != Some("member@example.com")
            {
                return Err(BackendError::new(
                    BackendErrorKind::InvalidInput,
                    "checkout.metadata.missing",
                    "Render replay lost linked checkout metadata.",
                ));
            }
            Ok(OrderReviewDecision::approved())
        }
    }

    impl CustomerBackendPlugin for MetadataReplayCheckoutPlugin {
        fn descriptor(&self) -> CustomerPluginDescriptor {
            CustomerPluginDescriptor::new(
                "render-order-metadata-replay",
                "Render Order Metadata Replay",
                "0.1.0",
            )
        }

        fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
            registry.register_checkout_hooks(Arc::new(MetadataReplayCheckoutHooks))
        }
    }

    fn render_test_plan_with_customer_plugin<C>(plugin: C) -> RuntimePlan
    where
        C: CustomerBackendPlugin,
    {
        let app_name = format!(
            "render-customer-hooks-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let storage_root = std::env::temp_dir().join(&app_name).display().to_string();
        let config = PlatformConfig::from_toml_str(
            &RENDER_TEST_CONFIG
                .replace(
                    "name = \"showcase-events\"",
                    &format!("name = \"{app_name}\""),
                )
                .replace(
                    "local_root = \"var/storage\"",
                    &format!("local_root = \"{storage_root}\""),
                ),
        )
        .unwrap();
        RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_customer_plugin(plugin)
            .build()
            .unwrap()
    }

    fn sample_storefront_order_snapshot() -> StorefrontOrderSnapshot {
        StorefrontOrderSnapshot {
            order_id: "ORD-10042".to_string(),
            session_id: "session-order-detail".to_string(),
            principal_id: Some("member@example.com".to_string()),
            metadata: BTreeMap::from([
                ("membership_tier".to_string(), "gold".to_string()),
                ("shipping_country".to_string(), "GB".to_string()),
                ("expedited_requested".to_string(), "false".to_string()),
            ]),
            status: "paid".to_string(),
            payment: StorefrontPaymentSnapshot {
                status: "captured".to_string(),
                method: Some("card".to_string()),
                reference: Some("PAY-50001".to_string()),
                last4: Some("4242".to_string()),
                checkout_email: Some("member@example.com".to_string()),
            },
            currency: "GBP".to_string(),
            line_count: 1,
            subtotal_minor: 8_900,
            total_minor: 8_900,
            refunded_total_minor: 0,
            refundable_total_minor: 8_900,
            subtotal: "£89.00".to_string(),
            total: "£89.00".to_string(),
            refunded_total: "£0.00".to_string(),
            refundable_total: "£89.00".to_string(),
            created_at_unix_seconds: 0,
            lines: vec![StorefrontOrderLine {
                sku: "gold-membership".to_string(),
                title: "Gold Membership".to_string(),
                variant_title: "Annual".to_string(),
                product_kind: "membership".to_string(),
                entitlement_key: Some("membership.gold".to_string()),
                metadata: BTreeMap::from([("variant_title".to_string(), "Annual".to_string())]),
                quantity: 1,
                unit_price_minor: 8_900,
                total_minor: 8_900,
                currency: "GBP".to_string(),
                total: "£89.00".to_string(),
            }],
            refunds: Vec::new(),
        }
    }

    fn fixture_model(route_name: &str) -> RenderModel {
        apply_route_specific_bindings(
            None,
            RenderModel::new(),
            route_name,
            "en-GB",
            &BTreeMap::new(),
            &RequestFieldMap::new(),
            None,
            None,
            None,
        )
        .unwrap()
    }

    fn live_account_model(principal_id: &str) -> RenderModel {
        let session = SessionContext {
            session_id: Some("session-live-123".to_string()),
            resolved_from_cookie: true,
        };
        let principal = PrincipalContext {
            principal_id: Some(principal_id.to_string()),
            granted_capabilities: HashSet::new(),
        };
        apply_route_specific_bindings(
            None,
            RenderModel::new(),
            "memberships.account",
            "en-GB",
            &BTreeMap::new(),
            &RequestFieldMap::new(),
            None,
            Some(&session),
            Some(&principal),
        )
        .unwrap()
    }

    fn session_scoped_account_model() -> RenderModel {
        let session = SessionContext {
            session_id: Some("session-live-guest".to_string()),
            resolved_from_cookie: true,
        };
        apply_route_specific_bindings(
            None,
            RenderModel::new(),
            "memberships.account.dashboard",
            "en-GB",
            &BTreeMap::new(),
            &RequestFieldMap::new(),
            None,
            Some(&session),
            None,
        )
        .unwrap()
    }

    fn render_fixture(route_name: &str, template_body: &str) -> String {
        let namespace = TemplateNamespace::new("customer-app").unwrap();
        let template = TemplateSourceParser::new()
            .parse_layout(
                namespace.clone(),
                TemplateName::new("page").unwrap(),
                template_body,
            )
            .unwrap();
        let mut registry = TemplateRegistry::new();
        registry.register(template).unwrap();
        TemplateRuntime::new(registry)
            .render_document(
                &[namespace],
                DocumentRenderRequest::new(
                    TemplateSelector::new(TemplateName::new("page").unwrap()),
                    fixture_model(route_name),
                ),
            )
            .unwrap()
            .html
    }

    #[test]
    fn route_specific_model_populates_catalog_listing() {
        let html = render_fixture(
            "commerce.catalog",
            r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <ul>
      <li dv:each="section : catalogSections" dv:text="${section.title}">Fallback</li>
    </ul>
  </body>
</html>"#,
        );

        assert!(html.contains("Featured"));
        assert!(html.contains("Memberships"));
    }

    #[test]
    fn route_specific_model_populates_checkout_intent_fields() {
        let html = render_fixture(
            "commerce.checkout",
            r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <p class="provider" dv:text="${checkout.providerLabel}">Provider</p>
    <p class="status" dv:text="${checkout.paymentStatusLabel}">Ready</p>
    <p class="reference" dv:text="${checkout.paymentReference}">PAYMENT-PENDING</p>
    <p class="summary" dv:text="${checkout.providerSummary}">Summary</p>
    <input type="hidden" name="payment_method" dv:attr="value=${checkout.paymentMethod}" />
  </body>
</html>"#,
        );

        assert!(html.contains("Platform fallback payment path"));
        assert!(html.contains("Ready for payment"));
        assert!(html.contains("card-on-file"));
        assert!(html.contains("provider-backed handoff"));
        assert!(html.contains("value=\"card\""));
    }

    #[test]
    fn route_specific_model_populates_checkout_confirmation_payment_fields() {
        let html = render_fixture(
            "commerce.checkout-confirmation",
            r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <p class="status" dv:text="${confirmation.status}">Paid</p>
    <p class="total" dv:text="${confirmation.total}">£0.00</p>
    <p class="payment-summary" dv:text="${confirmation.paymentSummary}">Summary</p>
    <p class="provider" dv:text="${confirmation.providerLabel}">Provider</p>
  </body>
</html>"#,
        );

        assert!(html.contains("Paid"));
        assert!(html.contains("£118.00"));
        assert!(html.contains("Card ending 4242, reference PAY-50001"));
        assert!(html.contains("Platform fallback payment path"));
    }

    #[test]
    fn route_specific_model_populates_account_surface() {
        let html = render_fixture(
            "memberships.account",
            r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <h1 dv:text="${customer.displayName}">Fallback</h1>
    <p dv:text="${membershipSummary.tierName}">Tier</p>
  </body>
</html>"#,
        );

        assert!(html.contains("Alex Mariner"));
        assert!(html.contains("Harbor Circle"));
    }

    #[test]
    fn route_specific_model_populates_checkout_surface() {
        let html = render_fixture(
            "commerce.checkout",
            r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <input type="text" dv:attr="value=${checkout.paymentReference}" value="fallback" />
    <strong dv:text="${customer.email}">Fallback</strong>
  </body>
</html>"#,
        );

        assert!(html.contains("card-on-file"));
        assert!(html.contains("member@example.com"));
    }

    #[test]
    fn route_specific_model_populates_product_slug_and_related_cards() {
        let html = render_fixture(
            "commerce.product-detail",
            r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <input type="hidden" dv:attr="value=${product.slug}" value="fallback" />
    <ul>
      <li dv:each="product : ${productCards}" dv:text="${product.slug}">fallback</li>
    </ul>
  </body>
</html>"#,
        );

        assert!(html.contains("value=\"harbor-cap\""), "{html}");
        assert!(html.contains("gold-membership"), "{html}");
    }

    #[test]
    fn route_specific_model_populates_cart_links_from_fixture_catalog() {
        let html = render_fixture(
            "commerce.cart",
            r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <ul>
      <li dv:each="item : ${cartItems}">
        <a class="collection" dv:if="${item.hasProductLink}" dv:attr="href=${item.collectionUrl}" dv:text="${item.collectionName}">Collection</a>
        <a class="product" dv:if="${item.hasProductLink}" dv:attr="href=${item.productUrl}" dv:text="${item.title}">Product</a>
      </li>
    </ul>
  </body>
</html>"#,
        );

        assert!(html.contains("/en-GB/shop/products/harbor-cap"), "{html}");
        assert!(html.contains("/en-GB/shop/collections/featured"), "{html}");
        assert!(
            html.contains("/en-GB/shop/collections/memberships"),
            "{html}"
        );
        assert!(html.contains("Gold Membership"), "{html}");
    }

    #[test]
    fn live_account_surface_prefers_session_backed_customer_state() {
        let namespace = TemplateNamespace::new("customer-app").unwrap();
        let template = TemplateSourceParser::new()
            .parse_layout(
                namespace.clone(),
                TemplateName::new("page").unwrap(),
                r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <h1 dv:text="${customer.displayName}">Fallback</h1>
    <p class="summary" dv:text="${account.stateSummary}">State</p>
    <p class="email" dv:if="${account.hasCustomerEmail}" dv:text="${customer.email}">Email</p>
    <p class="latest-order" dv:if="${account.hasLatestOrder}">
      <strong dv:text="${account.latestOrderReference}">Order</strong>
      <span dv:text="${account.latestOrderStatus}">Status</span>
    </p>
    <ul class="orders">
      <li dv:each="order : ${recentOrders}">
        <strong dv:text="${order.reference}">Order</strong>
        <span dv:text="${order.status}">Status</span>
        <span dv:text="${order.total}">Total</span>
      </li>
    </ul>
    <p class="membership" dv:text="${membershipSummary.tierName}">Membership</p>
    <p class="membership-status" dv:text="${membershipSummary.status}">Active</p>
  </body>
</html>"#,
            )
            .unwrap();
        let mut registry = TemplateRegistry::new();
        registry.register(template).unwrap();
        let html = TemplateRuntime::new(registry)
            .render_document(
                &[namespace],
                DocumentRenderRequest::new(
                    TemplateSelector::new(TemplateName::new("page").unwrap()),
                    live_account_model("sea.member@example.com"),
                ),
            )
            .unwrap()
            .html;

        assert!(html.contains("Sea Member"));
        assert!(html.contains("sea.member@example.com"));
        assert!(html.contains("live storefront session identity"));
        assert!(!html.contains("ORD-10042"));
        assert!(!html.contains("Paid"));
        assert!(!html.contains("Gold Membership"));
        assert!(html.contains("Membership unavailable"));
    }

    #[test]
    fn session_scoped_account_surface_uses_honest_browser_session_guidance() {
        let namespace = TemplateNamespace::new("customer-app").unwrap();
        let template = TemplateSourceParser::new()
            .parse_layout(
                namespace.clone(),
                TemplateName::new("page").unwrap(),
                r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <p class="summary" dv:text="${account.stateSummary}">State</p>
    <p class="orders-empty" dv:text="${account.ordersEmptyText}">Orders</p>
    <p class="membership-empty" dv:text="${account.membershipEmptyText}">Membership</p>
  </body>
</html>"#,
            )
            .unwrap();
        let mut registry = TemplateRegistry::new();
        registry.register(template).unwrap();
        let html = TemplateRuntime::new(registry)
            .render_document(
                &[namespace],
                DocumentRenderRequest::new(
                    TemplateSelector::new(TemplateName::new("page").unwrap()),
                    session_scoped_account_model(),
                ),
            )
            .unwrap()
            .html;

        assert!(
            html.contains("follows the current browser session"),
            "{html}"
        );
        assert!(html.contains("no completed orders yet"), "{html}");
        assert!(html.contains("qualifying membership purchase"), "{html}");
    }

    #[test]
    fn customer_order_review_surfaces_render_recorded_notes() {
        let plan = render_test_plan_with_customer_plugin(NoteRecordingCheckoutPlugin);
        let review = customer_order_review(&plan, &sample_storefront_order_snapshot(), None, None)
            .unwrap()
            .expect("checkout hook review should exist");

        assert_eq!(review.notes, vec!["Flag for finance follow-up".to_string()]);

        let namespace = TemplateNamespace::new("customer-app").unwrap();
        let template = TemplateSourceParser::new()
            .parse_layout(
                namespace.clone(),
                TemplateName::new("page").unwrap(),
                r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <p class="status" dv:text="${review.status}">Approved</p>
    <ul dv:if="${review.hasNotes}">
      <li dv:each="note : ${review.notes}" dv:text="${note.text}">note</li>
    </ul>
  </body>
</html>"#,
            )
            .unwrap();
        let mut registry = TemplateRegistry::new();
        registry.register(template).unwrap();
        let html = TemplateRuntime::new(registry)
            .render_document(
                &[namespace],
                DocumentRenderRequest::new(
                    TemplateSelector::new(TemplateName::new("page").unwrap()),
                    RenderModel::new()
                        .with_object("review", customer_order_review_model(review).unwrap())
                        .unwrap(),
                ),
            )
            .unwrap()
            .html;

        assert!(html.contains("Approved"), "{html}");
        assert!(html.contains("Flag for finance follow-up"), "{html}");
    }

    #[test]
    fn customer_order_review_fails_closed_on_mismatched_render_order_note_target() {
        let plan = render_test_plan_with_customer_plugin(WrongOrderNoteCheckoutPlugin);
        let error = customer_order_review(&plan, &sample_storefront_order_snapshot(), None, None)
            .expect_err("mismatched note target should fail the render hook");
        let message = error.to_string();

        assert!(message.contains("order_mismatch"), "{message}");
        assert!(message.contains("ORD-10042"), "{message}");
    }

    #[test]
    fn customer_order_review_replays_persisted_order_metadata_into_linked_hooks() {
        let plan = render_test_plan_with_customer_plugin(MetadataReplayCheckoutPlugin);
        let review = customer_order_review(&plan, &sample_storefront_order_snapshot(), None, None)
            .unwrap()
            .expect("metadata-aware review should exist");

        assert!(matches!(review.decision, OrderReviewDecision::Approved));
    }
}
#[derive(Clone)]
struct StorefrontPageFeedback {
    visible_flash_messages: Vec<FlashMessage>,
    form_state: Option<StorefrontFormState>,
}

fn storefront_page_feedback(route_name: &str, messages: &[FlashMessage]) -> StorefrontPageFeedback {
    let mut visible_flash_messages = Vec::new();
    let mut form_state = None;
    for message in messages {
        if let Some(decoded) = StorefrontFormState::decode(&message.text) {
            if decoded.route_name == route_name && form_state.is_none() {
                form_state = Some(decoded);
            }
            continue;
        }
        visible_flash_messages.push(message.clone());
    }
    StorefrontPageFeedback {
        visible_flash_messages,
        form_state,
    }
}

fn cart_form_model(
    form_state: Option<&StorefrontFormState>,
) -> Result<RenderModel, TemplateModelError> {
    let errors = form_errors_model(form_state)?;
    let has_errors = !errors.is_empty();
    RenderModel::new()
        .with_bool("hasErrors", has_errors)?
        .with_value(
            "errorSummary",
            RenderValue::text(
                form_state
                    .map(|state| state.summary.clone())
                    .unwrap_or_else(|| "Fix the highlighted cart lines and try again.".to_string()),
            ),
        )?
        .with_list("errors", errors)
}

fn merge_checkout_form_feedback(
    model: RenderModel,
    form_state: Option<&StorefrontFormState>,
) -> Result<RenderModel, TemplateModelError> {
    let errors = form_errors_model(form_state)?;
    let has_errors = form_state.is_some() || !errors.is_empty();
    let checkout_email_error = storefront_field_error(form_state, "checkout_email");
    let payment_method_error = storefront_field_error(form_state, "payment_method");
    let payment_last4_error = storefront_field_error(form_state, "payment_last4");
    let checkout_intent_error = storefront_field_error(form_state, "checkout_intent");
    let terms_accepted_error = storefront_field_error(form_state, "terms_accepted");
    model
        .with_bool("hasErrors", has_errors)?
        .with_value(
            "errorSummary",
            RenderValue::text(
                form_state
                    .map(|state| state.summary.clone())
                    .unwrap_or_else(|| {
                        "Review the highlighted checkout fields and try again.".to_string()
                    }),
            ),
        )?
        .with_list("errors", errors)?
        .with_bool("hasCheckoutEmailError", checkout_email_error.is_some())?
        .with_value(
            "checkoutEmailError",
            RenderValue::text(checkout_email_error.unwrap_or_default()),
        )?
        .with_bool("hasPaymentMethodError", payment_method_error.is_some())?
        .with_value(
            "paymentMethodError",
            RenderValue::text(payment_method_error.unwrap_or_default()),
        )?
        .with_bool("hasPaymentLast4Error", payment_last4_error.is_some())?
        .with_value(
            "paymentLast4Error",
            RenderValue::text(payment_last4_error.unwrap_or_default()),
        )?
        .with_bool("hasCheckoutIntentError", checkout_intent_error.is_some())?
        .with_value(
            "checkoutIntentError",
            RenderValue::text(checkout_intent_error.unwrap_or_default()),
        )?
        .with_bool("hasTermsAcceptedError", terms_accepted_error.is_some())?
        .with_value(
            "termsAcceptedError",
            RenderValue::text(terms_accepted_error.unwrap_or_default()),
        )
}

fn storefront_field_error(form_state: Option<&StorefrontFormState>, field: &str) -> Option<String> {
    form_state.and_then(|state| state.field_errors.get(field).cloned())
}

fn form_errors_model(
    form_state: Option<&StorefrontFormState>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    form_state
        .map(|state| {
            state
                .field_errors
                .iter()
                .map(|(field, message)| {
                    RenderModel::new()
                        .with_value("field", RenderValue::text(field.clone()))?
                        .with_value("message", RenderValue::text(message.clone()))
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap_or_else(|| Ok(Vec::new()))
}
