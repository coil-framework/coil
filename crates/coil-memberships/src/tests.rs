use super::*;
use coil_auth::Capability;
use coil_commerce::{EntitlementKey, OrderId, OrderOutcome};
use coil_core::{
    CoreServiceDependency, HttpResponseContract, HttpSurfaceArea, PlatformModule, ServiceRegistry,
};

fn instant(days: u64) -> MembershipInstant {
    MembershipInstant::from_days(days)
}

fn tier(
    id: &str,
    entitlement_key: &str,
    rank: u16,
    interval: BillingInterval,
    grace_period_days: u16,
) -> MembershipTier {
    MembershipTier::new(
        MembershipTierId::new(id).unwrap(),
        format!("{id} tier"),
        EntitlementKey::new(entitlement_key).unwrap(),
        rank,
        interval,
        grace_period_days,
        TierVisibility::Public,
        vec![
            MembershipBenefit::new(
                BenefitKey::new(format!("{id}.content")).unwrap(),
                BenefitKind::ContentAccess,
                "Member content access",
            )
            .unwrap(),
            MembershipBenefit::new(
                BenefitKey::new(format!("{id}.events")).unwrap(),
                BenefitKind::EventEligibility,
                "Event booking eligibility",
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn activate_subscription(
    id: &str,
    member_id: &str,
    tier: &MembershipTier,
    starts_at: MembershipInstant,
) -> Subscription {
    let mut subscription = Subscription::from_order(
        SubscriptionId::new(id).unwrap(),
        MemberAccountId::new(member_id).unwrap(),
        tier,
        OrderId::new("order-100").unwrap(),
        starts_at,
    )
    .unwrap();
    subscription.activate(starts_at).unwrap();
    subscription
}

#[test]
fn tier_rejects_duplicate_benefits() {
    let benefit_key = BenefitKey::new("members.gold").unwrap();
    let error = MembershipTier::new(
        MembershipTierId::new("gold").unwrap(),
        "Gold",
        EntitlementKey::new("membership.gold").unwrap(),
        10,
        BillingInterval::Monthly,
        7,
        TierVisibility::Public,
        vec![
            MembershipBenefit::new(
                benefit_key.clone(),
                BenefitKind::ContentAccess,
                "Primary benefit",
            )
            .unwrap(),
            MembershipBenefit::new(benefit_key, BenefitKind::MemberPricing, "Duplicate key")
                .unwrap(),
        ],
    )
    .unwrap_err();

    assert_eq!(
        error,
        MembershipModelError::DuplicateBenefit {
            key: "members.gold".to_string()
        }
    );
}

#[test]
fn activation_grants_entitlement_for_the_current_term() {
    let tier = tier("gold", "membership.gold", 10, BillingInterval::Monthly, 7);
    let subscription = activate_subscription("sub-1", "member-1", &tier, instant(10));

    assert_eq!(subscription.status, SubscriptionStatus::Active);
    assert_eq!(subscription.entitlements().len(), 1);
    assert_eq!(
        subscription.entitlements()[0],
        EntitlementGrant {
            key: EntitlementKey::new("membership.gold").unwrap(),
            subscription_id: SubscriptionId::new("sub-1").unwrap(),
            active_from: instant(10),
            active_until: instant(40),
            status: EntitlementStatus::Active,
            revoked_at: None,
        }
    );
    assert!(subscription.is_active_for_access());
}

#[test]
fn renewal_failure_enters_grace_then_expires() {
    let tier = tier("gold", "membership.gold", 10, BillingInterval::Monthly, 7);
    let mut subscription = activate_subscription("sub-1", "member-1", &tier, instant(0));

    subscription
        .apply_renewal_failure(subscription.current_term_end, &tier)
        .unwrap();
    assert_eq!(subscription.status, SubscriptionStatus::InGracePeriod);
    assert_eq!(subscription.grace_period_ends_at, Some(instant(37)));
    assert!(subscription.entitlements()[1].is_active_at(instant(35)));

    let expired = subscription.expire_if_grace_elapsed(instant(37)).unwrap();
    assert!(expired);
    assert_eq!(subscription.status, SubscriptionStatus::Expired);
    assert!(subscription.entitlements().last().unwrap().status == EntitlementStatus::Revoked);
}

#[test]
fn scheduled_cancellation_ends_at_the_period_boundary() {
    let tier = tier("gold", "membership.gold", 10, BillingInterval::Monthly, 7);
    let mut subscription = activate_subscription("sub-1", "member-1", &tier, instant(0));

    subscription.schedule_cancellation(instant(15)).unwrap();
    let settled = subscription.settle_period_boundary(instant(30)).unwrap();

    assert!(settled);
    assert_eq!(subscription.status, SubscriptionStatus::Cancelled);
    assert!(!subscription.is_active_for_access());
}

#[test]
fn changing_tier_replaces_the_active_entitlement() {
    let gold = tier("gold", "membership.gold", 10, BillingInterval::Monthly, 7);
    let platinum = tier(
        "platinum",
        "membership.platinum",
        20,
        BillingInterval::Annual,
        14,
    );
    let mut subscription = activate_subscription("sub-1", "member-1", &gold, instant(0));

    let change = subscription
        .change_tier(&platinum, instant(5), gold.rank)
        .unwrap();

    assert_eq!(change, TierChangeKind::Upgrade);
    assert_eq!(subscription.tier_id, platinum.id);
    assert_eq!(subscription.entitlement_key, platinum.entitlement_key);
    assert_eq!(subscription.entitlements().len(), 2);
    assert_eq!(
        subscription.entitlements().last().unwrap().key,
        EntitlementKey::new("membership.platinum").unwrap()
    );
    assert_eq!(
        subscription.history().last().unwrap().kind,
        SubscriptionEventKind::TierChanged {
            from: MembershipTierId::new("gold").unwrap(),
            to: MembershipTierId::new("platinum").unwrap(),
            kind: TierChangeKind::Upgrade,
        }
    );
}

#[test]
fn catalog_provisions_membership_subscriptions_from_commerce_outcomes() {
    let mut catalog = MembershipCatalog::new();
    catalog
        .register_tier(tier(
            "gold",
            "membership.gold",
            10,
            BillingInterval::Monthly,
            7,
        ))
        .unwrap();
    catalog
        .register_tier(tier(
            "silver",
            "membership.silver",
            5,
            BillingInterval::Quarterly,
            3,
        ))
        .unwrap();

    let order_id = OrderId::new("order-500").unwrap();
    let subscriptions = catalog
        .provision_from_order_outcomes(
            order_id.clone(),
            MemberAccountId::new("member-1").unwrap(),
            &[
                OrderOutcome::DeliverDigital {
                    sku: coil_commerce::Sku::new("ebook").unwrap(),
                    quantity: 1,
                },
                OrderOutcome::GrantMembership {
                    entitlement_key: EntitlementKey::new("membership.gold").unwrap(),
                    quantity: 2,
                },
            ],
            instant(100),
        )
        .unwrap();

    assert_eq!(subscriptions.len(), 2);
    assert_eq!(subscriptions[0].source_order_id, order_id);
    assert_eq!(
        subscriptions[0].subscription.history()[0].kind,
        SubscriptionEventKind::CreatedFromOrder {
            order_id: OrderId::new("order-500").unwrap(),
        }
    );
    assert_eq!(
        subscriptions[1].subscription.id,
        SubscriptionId::new("sub-order-500-2").unwrap()
    );
}

#[test]
fn module_manifest_and_service_registration_match_capability_contracts() {
    let module = MembershipsModule::new();
    let manifest = module.manifest();
    assert_eq!(manifest.name, "memberships");
    assert_eq!(
        manifest.required_capabilities,
        vec![
            Capability::MembershipSubscriptionManage,
            Capability::MembershipTierEdit,
        ]
    );
    assert!(
        manifest
            .optional_capabilities
            .contains(&Capability::AdminShellAccess)
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
            .contains(&CoreServiceDependency::Jobs)
    );
    assert_eq!(manifest.migrations.len(), 4);
    assert_eq!(manifest.route_surfaces.len(), 4);
    assert_eq!(manifest.http_surfaces.len(), 4);
    assert_eq!(manifest.jobs.len(), 2);
    assert_eq!(manifest.event_subscriptions.len(), 2);
    assert_eq!(manifest.admin_resources.len(), 2);
    assert_eq!(manifest.search_contributions.len(), 1);
    assert_eq!(manifest.report_definitions.len(), 1);
    assert!(
        manifest
            .route_surfaces
            .iter()
            .any(|surface| surface.name == "memberships.account.dashboard"
                && surface.path == "/account"
                && surface.capability.is_none())
    );
    assert!(
        manifest
            .route_surfaces
            .iter()
            .any(|surface| surface.name == "memberships.account"
                && surface.path == "/account/memberships"
                && surface.capability.is_none())
    );
    assert!(manifest.http_surfaces.iter().any(|surface| surface.name
        == "memberships.account.dashboard"
        && surface.area == HttpSurfaceArea::Account
        && surface.path == "/account"
        && matches!(
            &surface.response,
            HttpResponseContract::Page { template, status }
                if template == "account/dashboard" && *status == 200
        )
        && surface.capability.is_none()));
    assert!(
        manifest
            .http_surfaces
            .iter()
            .any(|surface| surface.name == "memberships.account"
                && surface.area == HttpSurfaceArea::Account
                && surface.path == "/account/memberships"
                && matches!(
                    &surface.response,
                    HttpResponseContract::Page { template, status }
                        if template == "memberships/account" && *status == 200
                )
                && surface.capability.is_none())
    );
    assert_eq!(
        module
            .install_migration_plan()
            .expect("memberships migration plan")
            .ordered_steps()
            .len(),
        4
    );

    let mut registry = ServiceRegistry::new();
    module.register(&mut registry).unwrap();

    assert!(
        registry
            .services()
            .any(|service| service.id == "module.memberships.entitlements")
    );
    assert!(
        registry
            .services()
            .any(|service| service.id == "module.memberships.commerce_bridge")
    );
}

#[test]
fn memberships_module_exposes_private_customer_account_surfaces_without_capability_gates() {
    let manifest = MembershipsModule::new().manifest();

    let dashboard = manifest
        .http_surfaces
        .iter()
        .find(|surface| surface.name == "memberships.account.dashboard")
        .expect("memberships account dashboard surface");
    assert_eq!(dashboard.area, HttpSurfaceArea::Account);
    assert!(dashboard.capability.is_none());
    assert!(matches!(
        &dashboard.response,
        HttpResponseContract::Page { template, status }
            if template == "account/dashboard" && *status == 200
    ));

    let memberships = manifest
        .http_surfaces
        .iter()
        .find(|surface| surface.name == "memberships.account")
        .expect("memberships account detail surface");
    assert_eq!(memberships.area, HttpSurfaceArea::Account);
    assert!(memberships.capability.is_none());
    assert!(matches!(
        &memberships.response,
        HttpResponseContract::Page { template, status }
            if template == "memberships/account" && *status == 200
    ));
}
