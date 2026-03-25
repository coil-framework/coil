use super::*;
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

        apply_route_specific_bindings(model, execution.route.route_name.as_str(), &execution.route.params)
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
        .and_then(|model| model.with_value("fragment_mode", RenderValue::bool(fragment_id.is_some())))
        .expect("page model keys are valid")
}

fn apply_route_specific_bindings(
    mut model: RenderModel,
    route_name: &str,
    params: &BTreeMap<String, String>,
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
            model = model.with_object("confirmation", fixture.confirmation.clone())?;
        }
        "memberships.account" | "memberships.account.dashboard" | "account.dashboard" => {
            model = model
                .with_object("customer", fixture.customer.clone())?
                .with_list("recentOrders", fixture.recent_orders.clone())?
                .with_object("membershipSummary", fixture.membership_summary.clone())?;
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
            summary: "Bookable offers and event-linked passes surfaced alongside editorial content.",
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

    let cart_items = vec![
        cart_item("Harbor Cap", "Canvas cap", "1", "£29.00")?,
        cart_item("Gold Membership", "Annual plan", "1", "£89.00")?,
    ];
    let cart_summary = RenderModel::new()
        .with_value("subtotal", RenderValue::text("£118.00"))?
        .with_value("shipping", RenderValue::text("£0.00"))?
        .with_value("total", RenderValue::text("£118.00"))?;

    let confirmation = RenderModel::new()
        .with_value("orderNumber", RenderValue::text("ORD-10042"))?
        .with_value("email", RenderValue::text("member@example.com"))?
        .with_value(
            "nextStep",
            RenderValue::text("A confirmation email and membership activation will follow shortly."),
        )?;

    let customer = RenderModel::new()
        .with_value("displayName", RenderValue::text("Alex Mariner"))?
        .with_value("email", RenderValue::text("member@example.com"))?;

    let recent_orders = vec![
        account_order("HS-1048", "£118.00", "Packed")?,
        account_order("HS-0998", "£45.00", "Fulfilled")?,
    ];
    let membership_summary =
        membership_summary("Harbor Circle", "Active", "Renews on 18 April")?;

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
            .zip(collections_data.iter().map(|collection| collection.handle.to_string()))
            .map(|(collection, handle)| (handle, collection))
            .collect(),
        products: product_cards
            .into_iter()
            .zip(products_data.iter().map(|product| product.handle.to_string()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use davenda_template::{
        DocumentRenderRequest, TemplateName, TemplateNamespace, TemplateRegistry,
        TemplateRuntime, TemplateSelector, TemplateSourceParser,
    };

    fn fixture_model(route_name: &str) -> RenderModel {
        apply_route_specific_bindings(RenderModel::new(), route_name, &BTreeMap::new()).unwrap()
    }

    fn render_fixture(route_name: &str, template_body: &str) -> String {
        let namespace = TemplateNamespace::new("customer-app").unwrap();
        let template = TemplateSourceParser::new()
            .parse_layout(namespace.clone(), TemplateName::new("page").unwrap(), template_body)
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
}
