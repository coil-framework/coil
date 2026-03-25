use super::*;
use davenda_commerce::{
    CheckoutId, CheckoutLine, CheckoutSession, CurrencyCode, EntitlementKey, Money, Order, OrderId,
    PricingPolicy, ProductId, ProductKind, Sku,
};
use davenda_template::{RenderModel, RenderValue, TemplateModelError, TemplateNamespace};
use std::collections::BTreeMap;

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
            .with_object("navigation", navigation_model())?
            .with_object(
                "page",
                page_model_for_route(execution, template_name, fragment_id),
            )?;

        if let Some(fragment_id) = fragment_id {
            model = model.with_value("fragment_id", RenderValue::text(fragment_id.to_string()))?;
        }

        apply_route_specific_bindings(
            model,
            execution.route.route_name.as_str(),
            &execution.route.params,
            Some(&execution.session),
            Some(&execution.principal),
        )
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

fn navigation_model() -> RenderModel {
    RenderModel::new()
        .with_list(
            "primary",
            vec![
                nav_item("Home", "/"),
                nav_item("Shop", "/shop"),
                nav_item("Collections", "/shop/collections/featured"),
                nav_item("Events", "/events"),
                nav_item("Cart", "/cart"),
                nav_item("Account", "/account"),
            ],
        )
        .expect("navigation keys are valid")
}

fn nav_item(label: &str, href: &str) -> RenderModel {
    RenderModel::new()
        .with_value("label", RenderValue::text(label))
        .and_then(|model| model.with_value("href", RenderValue::text(href)))
        .expect("navigation item keys are valid")
}

fn page_model_for_route(
    execution: &RequestExecution,
    template_name: &str,
    fragment_id: Option<&str>,
) -> RenderModel {
    let title = match execution.route.route_name.as_str() {
        "home" => "Harbor Shop".to_string(),
        "commerce.catalog" => "Shop Harbor".to_string(),
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
        "memberships.account" | "memberships.account.dashboard" | "account.dashboard" => {
            "Your Account".to_string()
        }
        _ => execution.route.route_name.clone(),
    };

    let summary = match execution.route.route_name.as_str() {
        "commerce.catalog" => {
            "Browse the current assortment across apparel, memberships, and event-linked offers."
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
    mut model: RenderModel,
    route_name: &str,
    params: &BTreeMap<String, String>,
    session: Option<&SessionContext>,
    principal: Option<&PrincipalContext>,
) -> Result<RenderModel, TemplateModelError> {
    let fixture = storefront_fixture()?;

    match route_name {
        "home" | "commerce.catalog" => {
            model = model
                .with_list("catalogSections", fixture.catalog_sections.clone())?
                .with_list("productCards", fixture.product_cards.clone())?;
        }
        "commerce.collection-detail" => {
            let slug = params
                .get("collection_slug")
                .map(String::as_str)
                .unwrap_or("featured");
            model = model
                .with_object("collection", fixture.collection_for(slug))?
                .with_list("productCards", fixture.product_cards_for_collection(slug))?;
        }
        "commerce.product-detail" => {
            let slug = params
                .get("product_slug")
                .map(String::as_str)
                .unwrap_or("harbor-cap");
            model = model.with_object("product", fixture.product_for(slug))?;
        }
        "commerce.cart" => {
            model = model
                .with_list("cartItems", fixture.cart_items.clone())?
                .with_object("cartSummary", fixture.cart_summary.clone())?;
        }
        "commerce.checkout" => {
            model = model
                .with_object("customer", fixture.customer.clone())?
                .with_list("lineItems", fixture.cart_items.clone())?
                .with_object("orderSummary", fixture.cart_summary.clone())?;
        }
        "commerce.checkout-confirmation" => {
            model = model
                .with_object("confirmation", fixture.confirmation.clone())?
                .with_object("customer", fixture.customer.clone())?
                .with_list("recentOrders", fixture.recent_orders.clone())?
                .with_object("membershipSummary", fixture.membership_summary.clone())?;
        }
        "memberships.account" | "memberships.account.dashboard" | "account.dashboard" => {
            let account = account_surface_bindings(&fixture, session, principal)?;
            model = model
                .with_object("account", account.account)?
                .with_object("customer", account.customer)?
                .with_list("recentOrders", account.recent_orders)?
                .with_object("membershipSummary", account.membership_summary)?;
        }
        _ => {}
    }

    Ok(model)
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

#[derive(Clone)]
struct AccountSurfaceBindings {
    account: RenderModel,
    customer: RenderModel,
    recent_orders: Vec<RenderModel>,
    membership_summary: RenderModel,
}

fn account_surface_bindings(
    fixture: &StorefrontFixture,
    session: Option<&SessionContext>,
    principal: Option<&PrincipalContext>,
) -> Result<AccountSurfaceBindings, TemplateModelError> {
    let Some(session) = session else {
        return fixture_account_surface_bindings(fixture);
    };
    let Some(principal) = principal else {
        return fixture_account_surface_bindings(fixture);
    };

    if session.session_id.is_none() && principal.principal_id.is_none() {
        return fixture_account_surface_bindings(fixture);
    }

    live_account_surface_bindings(fixture, session, principal)
}

fn fixture_account_surface_bindings(
    fixture: &StorefrontFixture,
) -> Result<AccountSurfaceBindings, TemplateModelError> {
    Ok(AccountSurfaceBindings {
        account: RenderModel::new()
            .with_bool("hasLiveSession", false)?
            .with_bool("hasPrincipal", false)?
            .with_bool("hasCustomerEmail", true)?
            .with_bool("hasRecentOrders", !fixture.recent_orders.is_empty())?
            .with_bool("hasMembership", true)?
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
            .with_value("ordersCtaUrl", RenderValue::text("/shop"))?
            .with_value(
                "membershipCtaUrl",
                RenderValue::text("/shop/collections/memberships"),
            )?,
        customer: fixture.customer.clone(),
        recent_orders: fixture.recent_orders.clone(),
        membership_summary: fixture.membership_summary.clone(),
    })
}

fn live_account_surface_bindings(
    fixture: &StorefrontFixture,
    session: &SessionContext,
    principal: &PrincipalContext,
) -> Result<AccountSurfaceBindings, TemplateModelError> {
    let principal_id = principal.principal_id.as_deref();
    let email = principal_id
        .filter(|candidate| looks_like_email(candidate))
        .unwrap_or_default()
        .to_string();
    let display_name = principal_id
        .map(display_name_from_principal_id)
        .unwrap_or_else(|| "Signed-in Customer".to_string());
    let state_summary = if principal_id.is_some() {
        "Using the live storefront session identity for this account view. Order history and membership state will render here when the storefront state path supplies them."
    } else {
        "Using a resolved storefront session for this account view. Order history and membership state will render here when the storefront state path supplies them."
    };

    Ok(AccountSurfaceBindings {
        account: RenderModel::new()
            .with_bool("hasLiveSession", session.session_id.is_some())?
            .with_bool("hasPrincipal", principal_id.is_some())?
            .with_bool("hasCustomerEmail", !email.is_empty())?
            .with_bool("hasRecentOrders", !fixture.recent_orders.is_empty())?
            .with_bool("hasMembership", true)?
            .with_value("stateSource", RenderValue::text("storefront-session"))?
            .with_value("stateSummary", RenderValue::text(state_summary))?
            .with_value(
                "ordersEmptyText",
                RenderValue::text(
                    "No order history is attached to this signed-in account yet. Completed storefront purchases will appear here once live account history is available.",
                ),
            )?
            .with_value(
                "membershipEmptyText",
                RenderValue::text(
                    "No active membership is attached to this signed-in account yet. Join from the storefront to unlock early access and renewal visibility.",
                ),
            )?
            .with_value("ordersCtaUrl", RenderValue::text("/shop"))?
            .with_value(
                "membershipCtaUrl",
                RenderValue::text("/shop/collections/memberships"),
            )?,
        customer: RenderModel::new()
            .with_value("displayName", RenderValue::text(display_name))?
            .with_value("email", RenderValue::text(email))?,
        recent_orders: fixture.recent_orders.clone(),
        membership_summary: fixture.membership_summary.clone(),
    })
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
    cart_items: Vec<RenderModel>,
    cart_summary: RenderModel,
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
}

fn product_cards_by_collection(
    products: &[ProductFixture<'_>],
) -> Result<BTreeMap<String, Vec<RenderModel>>, TemplateModelError> {
    let mut grouped: BTreeMap<String, Vec<RenderModel>> = BTreeMap::new();
    for product in products {
        grouped
            .entry(product.collection_handle.to_string())
            .or_default()
            .push(product_model(product)?);
    }
    Ok(grouped)
}

fn storefront_fixture() -> Result<StorefrontFixture, TemplateModelError> {
    let products_data = vec![
        ProductFixture {
            handle: "harbor-cap",
            title: "Harbor Cap",
            summary: "A classic canvas cap with embroidered harbor mark.",
            price: "£29.00",
            collection_handle: "featured",
            collection_name: "Featured",
        },
        ProductFixture {
            handle: "gold-membership",
            title: "Gold Membership",
            summary: "Priority event booking, exclusive offers, and member-only access.",
            price: "£89.00",
            collection_handle: "memberships",
            collection_name: "Memberships",
        },
        ProductFixture {
            handle: "tasting-pass",
            title: "Spring Tasting Pass",
            summary: "An event-linked pass for the next seasonal tasting series.",
            price: "£45.00",
            collection_handle: "events",
            collection_name: "Events",
        },
    ];
    let product_cards = products_data
        .iter()
        .map(product_model)
        .collect::<Result<Vec<_>, _>>()?;
    let product_cards_by_collection = product_cards_by_collection(&products_data)?;

    let collections_data = [
        CollectionFixture {
            handle: "featured",
            title: "Featured",
            href: "/shop/collections/featured",
            summary: "Current campaign picks spanning merch, memberships, and event offers.",
            label: "Featured edit",
        },
        CollectionFixture {
            handle: "memberships",
            title: "Memberships",
            href: "/shop/collections/memberships",
            summary: "Recurring and premium access products that unlock customer benefits.",
            label: "Recurring value",
        },
        CollectionFixture {
            handle: "events",
            title: "Events",
            href: "/shop/collections/events",
            summary:
                "Bookable offers and event-linked passes surfaced alongside editorial content.",
            label: "Event-led offer",
        },
    ];
    let catalog_sections = collections_data
        .iter()
        .map(collection_section_model)
        .collect::<Result<Vec<_>, _>>()?;
    let collections = collections_data
        .iter()
        .map(|collection| collection_detail_model(collection, &products_data))
        .collect::<Result<Vec<_>, _>>()?;

    let current_order = sample_completed_order();
    let previous_order = sample_previous_order();

    let cart_items = current_order
        .lines
        .iter()
        .map(cart_item_from_line)
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
        cart_items,
        cart_summary,
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

struct CollectionFixture<'a> {
    handle: &'a str,
    title: &'a str,
    href: &'a str,
    summary: &'a str,
    label: &'a str,
}

struct ProductFixture<'a> {
    handle: &'a str,
    title: &'a str,
    summary: &'a str,
    price: &'a str,
    collection_handle: &'a str,
    collection_name: &'a str,
}

fn collection_section_model(
    collection: &CollectionFixture<'_>,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("label", RenderValue::text(collection.label))?
        .with_value("title", RenderValue::text(collection.title))?
        .with_value("summary", RenderValue::text(collection.summary))?
        .with_value("url", RenderValue::text(collection.href))
}

fn collection_detail_model(
    collection: &CollectionFixture<'_>,
    products: &[ProductFixture<'_>],
) -> Result<RenderModel, TemplateModelError> {
    let filtered_products = products
        .iter()
        .filter(|product| {
            product.collection_handle == collection.handle || collection.handle == "featured"
        })
        .map(product_model)
        .collect::<Vec<_>>();
    let filtered_products = filtered_products
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    RenderModel::new()
        .with_value("title", RenderValue::text(collection.title))?
        .with_value("summary", RenderValue::text(collection.summary))?
        .with_value("url", RenderValue::text(collection.href))?
        .with_list("products", filtered_products)
}

fn product_model(product: &ProductFixture<'_>) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("handle", RenderValue::text(product.handle))?
        .with_value("name", RenderValue::text(product.title))?
        .with_value("summary", RenderValue::text(product.summary))?
        .with_value("price", RenderValue::text(product.price))?
        .with_value(
            "url",
            RenderValue::text(format!("/shop/products/{}", product.handle)),
        )?
        .with_value("addToCartUrl", RenderValue::text("/cart"))?
        .with_value("imageUrl", RenderValue::text("/theme/assets/logo.svg"))?
        .with_value("imageAlt", RenderValue::text(product.title))?
        .with_value(
            "collectionName",
            RenderValue::text(product.collection_name.to_string()),
        )
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

fn cart_item_from_line(line: &CheckoutLine) -> Result<RenderModel, TemplateModelError> {
    cart_item(
        &line.product_title,
        &line.variant_title,
        &line.quantity.to_string(),
        &money_display(&line.subtotal().expect("sample checkout line is valid")),
    )
}

fn confirmation_model(order: &Order) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("orderNumber", RenderValue::text(order.id.to_string()))?
        .with_value("email", RenderValue::text("member@example.com"))?
        .with_value("nextStep", RenderValue::text(order.confirmation_message()))
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
    let amount_minor = money.amount_minor();
    let major = amount_minor / 100;
    let remainder = amount_minor % 100;
    match money.currency().as_str() {
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
    use davenda_template::{
        DocumentRenderRequest, TemplateName, TemplateNamespace, TemplateRegistry, TemplateRuntime,
        TemplateSelector, TemplateSourceParser,
    };
    use std::collections::HashSet;

    fn fixture_model(route_name: &str) -> RenderModel {
        apply_route_specific_bindings(RenderModel::new(), route_name, &BTreeMap::new(), None, None)
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
            RenderModel::new(),
            "memberships.account",
            &BTreeMap::new(),
            Some(&session),
            Some(&principal),
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
        assert!(html.contains("ORD-10042"));
        assert!(html.contains("Paid"));
        assert!(html.contains("Harbor Circle"));
    }
}
