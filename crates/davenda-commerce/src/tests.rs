use super::*;
use davenda_auth::Capability;
use davenda_core::{CoreServiceDependency, ExtensionSlotKind, PlatformModule, ServiceRegistry};
use davenda_data::{
    MigrationOwner, PublicationVisibility, QueryCacheScope, TransactionIsolation,
};

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
    assert_eq!(manifest.route_surfaces.len(), 4);
    assert_eq!(manifest.http_surfaces.len(), 4);
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
}
