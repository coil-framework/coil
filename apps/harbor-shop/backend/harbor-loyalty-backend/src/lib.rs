//! Harbor Shop linked customer backend example.
//!
//! This crate is the chapter 96 path for customer-owned first-party Rust logic: a linked library
//! that a customer workspace composes into its own binary and registers at explicit Davenda hook
//! points.
//!
//! High-level shape:
//!
//! ```rust,ignore
//! fn main() -> Result<(), anyhow::Error> {
//!     davenda_all::builder()
//!         .with_customer_plugin(harbor_loyalty_backend::plugin())
//!         .run_from_env()
//! }
//! ```
//!
//! The optional Axum service in `src/http.rs` is intentionally secondary. It adapts the same
//! customer-owned Rust rules to a sidecar process only when a separate HTTP/process boundary is
//! genuinely useful.

mod http;

use davenda_customer_sdk::{
    AuditEntry, AuditFacade, AuthFacade, BackendError, BackendErrorKind, CheckoutHooks,
    CommerceFacade, CustomerBackendPlugin, CustomerHookRegistry, CustomerPluginDescriptor,
    JobsFacade, OrderAdjustment, OrderDraft, OrderReviewDecision, RegisteredHookKind,
    RequestContext, VerifiedWebhook, VerifiedWebhookHooks, WebhookHandlingResult,
};
pub use http::{BackendConfig, build_router};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HarborCustomerBackend;

pub fn plugin() -> HarborCustomerBackend {
    HarborCustomerBackend
}

pub fn plugin_descriptor() -> CustomerPluginDescriptor {
    CustomerPluginDescriptor::new(
        "harbor-loyalty-backend",
        "Harbor Shop Loyalty Backend",
        env!("CARGO_PKG_VERSION"),
    )
    .with_documentation_url("apps/harbor-shop/backend/harbor-loyalty-backend/README.md")
}

pub fn registered_hook_kinds() -> Vec<RegisteredHookKind> {
    vec![
        RegisteredHookKind::Checkout,
        RegisteredHookKind::VerifiedWebhook,
    ]
}

impl HarborCustomerBackend {
    pub fn preview_loyalty(&self, request: &LoyaltyPreviewRequest) -> LoyaltyPreviewResponse {
        compute_loyalty_preview(request)
    }

    pub fn review_checkout_order(&self, request: &OrderReviewRequest) -> OrderReviewResponse {
        review_order(request)
    }

    pub fn route_crm_contact_update(&self, update: &CrmContactUpdate) -> CrmContactRoute {
        route_crm_contact(update)
    }
}

impl CustomerBackendPlugin for HarborCustomerBackend {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        plugin_descriptor()
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        let hooks = Arc::new(*self);
        registry.register_checkout_hooks(hooks.clone())?;
        registry.register_verified_webhook_hooks(hooks)?;
        Ok(())
    }
}

impl CheckoutHooks for HarborCustomerBackend {
    fn review_order(
        &self,
        _ctx: &RequestContext,
        order: &OrderDraft,
        _commerce: &dyn CommerceFacade,
        _auth: &dyn AuthFacade,
        audit: &dyn AuditFacade,
    ) -> Result<OrderReviewDecision, BackendError> {
        let request = review_request_from_order(order);
        let review = self.review_checkout_order(&request);
        audit.record(
            AuditEntry::new(
                "customer-plugin.checkout-review",
                "order",
                order.order_id.clone(),
                if review.review_required {
                    "manual-review"
                } else {
                    "approved"
                },
            )
            .with_detail(review.operator_note.clone()),
        )?;

        if review.review_required {
            Ok(OrderReviewDecision::Adjusted(
                OrderAdjustment::new(review.operator_note.clone()).with_metadata_entries([
                    ("assigned_queue", review.assigned_queue.clone()),
                    ("service_level", review.service_level.clone()),
                    ("review_required", "true".to_string()),
                    ("tags", review.tags.join(",")),
                ]),
            ))
        } else {
            Ok(OrderReviewDecision::approved())
        }
    }
}

impl VerifiedWebhookHooks for HarborCustomerBackend {
    fn handle_verified_webhook(
        &self,
        _ctx: &RequestContext,
        webhook: &VerifiedWebhook,
        _http: &dyn davenda_customer_sdk::OutboundHttpFacade,
        _jobs: &dyn JobsFacade,
        audit: &dyn AuditFacade,
    ) -> Result<WebhookHandlingResult, BackendError> {
        if webhook.source != "crm" || webhook.event != "contact-updated" {
            return Ok(WebhookHandlingResult::rejected(
                "unsupported_webhook",
                "Harbor Shop only handles verified CRM contact-updated webhooks in this example",
            ));
        }

        let update: CrmContactUpdate =
            serde_json::from_slice(&webhook.payload).map_err(|error| {
                BackendError::new(
                    BackendErrorKind::InvalidInput,
                    "crm_contact_update_payload",
                    "verified webhook payload could not be parsed as a Harbor CRM contact update",
                )
                .with_detail(error.to_string())
            })?;
        let route = self.route_crm_contact_update(&update);
        audit.record(
            AuditEntry::new(
                "customer-plugin.verified-webhook",
                "crm-contact",
                update.customer_email.clone(),
                if route.follow_up_required {
                    "follow-up-required"
                } else {
                    "accepted"
                },
            )
            .with_detail(route.follow_up_reason.clone()),
        )?;

        Ok(WebhookHandlingResult::accepted(Some(format!(
            "{} [{}]",
            route.follow_up_reason,
            route.tags.join(", ")
        ))))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MembershipTier {
    Guest,
    Standard,
    Gold,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoyaltyPreviewRequest {
    pub customer_email: String,
    pub membership_tier: MembershipTier,
    pub subtotal_gbp: f64,
    #[serde(default)]
    pub cart_skus: Vec<String>,
    pub collection_handle: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoyaltyPreviewResponse {
    pub segment: String,
    pub discount_bps: u16,
    pub free_shipping: bool,
    pub priority_fulfilment: bool,
    pub concierge_note: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrmContactUpdate {
    pub customer_email: String,
    pub membership_tier: MembershipTier,
    pub lifecycle_stage: String,
    pub last_order_total_gbp: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrmContactRoute {
    pub segment: String,
    pub follow_up_required: bool,
    pub follow_up_reason: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderReviewRequest {
    pub customer_email: String,
    pub membership_tier: MembershipTier,
    pub subtotal_gbp: f64,
    #[serde(default)]
    pub cart_skus: Vec<String>,
    pub shipping_country: String,
    #[serde(default)]
    pub expedited_requested: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderReviewResponse {
    pub review_required: bool,
    pub assigned_queue: String,
    pub service_level: String,
    pub operator_note: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceOverview {
    pub service: String,
    pub brand: String,
    pub endpoints: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub service: String,
    pub status: String,
}

pub fn service_overview(brand: &str) -> ServiceOverview {
    ServiceOverview {
        service: "harbor-loyalty-backend".to_string(),
        brand: brand.to_string(),
        endpoints: vec![
            "GET /".to_string(),
            "GET /health".to_string(),
            "POST /api/loyalty/preview".to_string(),
            "POST /api/orders/review".to_string(),
            "POST /webhooks/crm/contact-updated".to_string(),
        ],
    }
}

pub fn health_response() -> HealthResponse {
    HealthResponse {
        service: "harbor-loyalty-backend".to_string(),
        status: "ok".to_string(),
    }
}

pub fn compute_loyalty_preview(request: &LoyaltyPreviewRequest) -> LoyaltyPreviewResponse {
    let is_event_order = request
        .collection_handle
        .as_deref()
        .map(|handle| handle == "events")
        .unwrap_or(false)
        || request.cart_skus.iter().any(|sku| sku == "tasting-pass");
    let is_high_value = request.subtotal_gbp >= 150.0;

    let (segment, discount_bps, priority_fulfilment) = match request.membership_tier {
        MembershipTier::Gold => {
            let discount_bps = if is_high_value { 1500 } else { 1000 };
            ("harbor-vip".to_string(), discount_bps, true)
        }
        MembershipTier::Standard => {
            let discount_bps = if request.subtotal_gbp >= 100.0 {
                500
            } else {
                250
            };
            ("harbor-member".to_string(), discount_bps, is_event_order)
        }
        MembershipTier::Guest => ("harbor-guest".to_string(), 0, is_event_order),
    };

    let free_shipping =
        !matches!(request.membership_tier, MembershipTier::Guest) || request.subtotal_gbp >= 80.0;
    let concierge_note = if matches!(request.membership_tier, MembershipTier::Gold) && is_high_value
    {
        "Offer quay-side pickup and same-day concierge follow-up.".to_string()
    } else if is_event_order {
        "Flag this order for the events desk so arrival instructions stay in sync.".to_string()
    } else if free_shipping {
        "Customer qualifies for Harbor Shop free delivery.".to_string()
    } else {
        "Standard storefront fulfillment rules apply.".to_string()
    };

    let mut tags = vec![
        "customer-app:harbor-shop".to_string(),
        format!("segment:{segment}"),
    ];
    if free_shipping {
        tags.push("perk:free-shipping".to_string());
    }
    if priority_fulfilment {
        tags.push("perk:priority-fulfilment".to_string());
    }
    if discount_bps > 0 {
        tags.push(format!("perk:discount-{}bps", discount_bps));
    }

    LoyaltyPreviewResponse {
        segment,
        discount_bps,
        free_shipping,
        priority_fulfilment,
        concierge_note,
        tags,
    }
}

pub fn route_crm_contact(update: &CrmContactUpdate) -> CrmContactRoute {
    let high_value = update.last_order_total_gbp.unwrap_or_default() >= 150.0;
    let (segment, follow_up_required, follow_up_reason) = match update.membership_tier {
        MembershipTier::Gold if high_value => (
            "harbor-vip".to_string(),
            true,
            "High-value gold member should receive manual concierge follow-up.".to_string(),
        ),
        MembershipTier::Gold => (
            "harbor-vip".to_string(),
            false,
            "Gold member remains in the premium nurture track.".to_string(),
        ),
        MembershipTier::Standard if update.lifecycle_stage == "winback" => (
            "harbor-member".to_string(),
            true,
            "Winback member should receive the Harbor Shop retention sequence.".to_string(),
        ),
        MembershipTier::Standard => (
            "harbor-member".to_string(),
            false,
            "Standard member stays in the default lifecycle automation.".to_string(),
        ),
        MembershipTier::Guest => (
            "harbor-guest".to_string(),
            false,
            "Guest contact remains in the public storefront lead funnel.".to_string(),
        ),
    };

    CrmContactRoute {
        segment: segment.clone(),
        follow_up_required,
        follow_up_reason,
        tags: vec![
            "customer-app:harbor-shop".to_string(),
            format!("segment:{segment}"),
            format!("lifecycle:{}", update.lifecycle_stage),
        ],
    }
}

pub fn review_order(request: &OrderReviewRequest) -> OrderReviewResponse {
    let international = !matches!(request.shipping_country.as_str(), "GB" | "UK");
    let contains_event_pass = request
        .cart_skus
        .iter()
        .any(|sku| sku == "tasting-pass" || sku == "cellar-tour-pass");
    let high_value = request.subtotal_gbp >= 200.0;
    let review_required =
        international || high_value || (request.expedited_requested && contains_event_pass);

    let assigned_queue = if review_required {
        "ops-manual-review".to_string()
    } else if matches!(request.membership_tier, MembershipTier::Gold) {
        "vip-fulfilment".to_string()
    } else {
        "storefront-standard".to_string()
    };

    let service_level = if review_required {
        "manual-clearance".to_string()
    } else if matches!(request.membership_tier, MembershipTier::Gold) {
        "priority".to_string()
    } else if request.expedited_requested {
        "expedited".to_string()
    } else {
        "standard".to_string()
    };

    let operator_note = if international {
        "Check customs-safe packing and confirm the carrier lane before capture.".to_string()
    } else if high_value && matches!(request.membership_tier, MembershipTier::Gold) {
        "Gold high-value order: route to concierge packing and same-day follow-up.".to_string()
    } else if request.expedited_requested && contains_event_pass {
        "Expedited event order needs manual arrival coordination before release.".to_string()
    } else if matches!(request.membership_tier, MembershipTier::Gold) {
        "Gold member order qualifies for the priority fulfilment lane.".to_string()
    } else {
        "Standard storefront fulfilment rules apply.".to_string()
    };

    let mut tags = vec![
        "customer-app:harbor-shop".to_string(),
        format!("queue:{assigned_queue}"),
        format!("service-level:{service_level}"),
    ];
    if review_required {
        tags.push("ops:manual-review".to_string());
    }
    if international {
        tags.push("shipping:international".to_string());
    }
    if contains_event_pass {
        tags.push("catalog:event-pass".to_string());
    }

    OrderReviewResponse {
        review_required,
        assigned_queue,
        service_level,
        operator_note,
        tags,
    }
}

pub fn webhook_secret_matches(expected: &str, provided: Option<&str>) -> bool {
    match provided {
        Some(value) => value == expected,
        None => false,
    }
}

fn review_request_from_order(order: &OrderDraft) -> OrderReviewRequest {
    OrderReviewRequest {
        customer_email: order_metadata(order, &["customer_email", "checkout_email"])
            .unwrap_or("guest@harbor.local")
            .to_string(),
        membership_tier: order_metadata(order, &["membership_tier"])
            .map(parse_membership_tier)
            .unwrap_or(MembershipTier::Guest),
        subtotal_gbp: order.subtotal.minor_units as f64 / 100.0,
        cart_skus: order.lines.iter().map(|line| line.sku.clone()).collect(),
        shipping_country: order_metadata(order, &["shipping_country", "country"])
            .unwrap_or("GB")
            .to_string(),
        expedited_requested: order_metadata(order, &["expedited_requested"])
            .map(parse_bool)
            .unwrap_or(false),
    }
}

fn order_metadata<'a>(order: &'a OrderDraft, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| order.metadata.get(*key).map(String::as_str))
}

fn parse_membership_tier(value: &str) -> MembershipTier {
    match value.trim().to_ascii_lowercase().as_str() {
        "gold" => MembershipTier::Gold,
        "standard" => MembershipTier::Standard,
        _ => MembershipTier::Guest,
    }
}

fn parse_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "y" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use davenda_customer_sdk::{
        AuditFacade, AuthCheckRequest, AuthCheckResult, AuthExplainRequest, AuthExplanation,
        CommerceProduct, CustomerAppContext, CustomerHookRegistry, MoneyAmount, OrderLineDraft,
        OutboundHttpRequest, OutboundHttpResponse, PrincipalContext, TraceContext,
    };
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct RecordingRegistry {
        hook_kinds: Vec<RegisteredHookKind>,
    }

    impl CustomerHookRegistry for RecordingRegistry {
        fn register_checkout_hooks(
            &mut self,
            _hooks: Arc<dyn CheckoutHooks>,
        ) -> Result<(), BackendError> {
            self.hook_kinds.push(RegisteredHookKind::Checkout);
            Ok(())
        }

        fn register_cms_hooks(
            &mut self,
            _hooks: Arc<dyn davenda_customer_sdk::CmsHooks>,
        ) -> Result<(), BackendError> {
            unreachable!("Harbor loyalty backend should not register CMS hooks")
        }

        fn register_verified_webhook_hooks(
            &mut self,
            _hooks: Arc<dyn VerifiedWebhookHooks>,
        ) -> Result<(), BackendError> {
            self.hook_kinds.push(RegisteredHookKind::VerifiedWebhook);
            Ok(())
        }
    }

    struct NoopCommerce;

    impl CommerceFacade for NoopCommerce {
        fn product(&self, _sku: &str) -> Result<Option<CommerceProduct>, BackendError> {
            Ok(None)
        }

        fn add_order_note(&self, _order_id: &str, _note: &str) -> Result<(), BackendError> {
            Ok(())
        }
    }

    struct NoopAuth;

    impl AuthFacade for NoopAuth {
        fn check_capability(
            &self,
            _request: &AuthCheckRequest,
        ) -> Result<AuthCheckResult, BackendError> {
            Ok(AuthCheckResult {
                allowed: true,
                explanation: None,
            })
        }

        fn explain_denial(
            &self,
            _request: &AuthExplainRequest,
        ) -> Result<AuthExplanation, BackendError> {
            Ok(AuthExplanation {
                summary: "allowed".to_string(),
                traces: Vec::new(),
            })
        }
    }

    #[derive(Default)]
    struct RecordingAudit {
        actions: Mutex<Vec<String>>,
    }

    impl AuditFacade for RecordingAudit {
        fn record(&self, entry: AuditEntry) -> Result<(), BackendError> {
            self.actions.lock().unwrap().push(entry.action);
            Ok(())
        }
    }

    struct NoopHttp;

    impl davenda_customer_sdk::OutboundHttpFacade for NoopHttp {
        fn send(
            &self,
            _request: OutboundHttpRequest,
        ) -> Result<OutboundHttpResponse, BackendError> {
            Ok(OutboundHttpResponse {
                status: 200,
                headers: BTreeMap::new(),
                body: Vec::new(),
            })
        }
    }

    struct NoopJobs;

    impl JobsFacade for NoopJobs {
        fn enqueue(
            &self,
            _request: davenda_customer_sdk::JobRequest,
        ) -> Result<davenda_customer_sdk::JobReceipt, BackendError> {
            Ok(davenda_customer_sdk::JobReceipt {
                queue: "customer-backend".to_string(),
                job_id: "job-1".to_string(),
            })
        }
    }

    fn request_context() -> RequestContext {
        RequestContext::new(
            CustomerAppContext::new("harbor-shop", "development").with_locale("en-GB"),
            PrincipalContext::user("operator-live-1"),
            TraceContext::new("trace-harbor-1"),
        )
    }

    #[test]
    fn gold_member_high_value_order_gets_vip_rules() {
        let response = compute_loyalty_preview(&LoyaltyPreviewRequest {
            customer_email: "captain@harbor.test".to_string(),
            membership_tier: MembershipTier::Gold,
            subtotal_gbp: 175.0,
            cart_skus: vec!["harbor-cap".to_string()],
            collection_handle: Some("featured".to_string()),
        });

        assert_eq!(response.segment, "harbor-vip");
        assert_eq!(response.discount_bps, 1500);
        assert!(response.free_shipping);
        assert!(response.priority_fulfilment);
        assert!(
            response.concierge_note.contains("quay-side pickup"),
            "{}",
            response.concierge_note
        );
    }

    #[test]
    fn standard_event_order_gets_priority_but_not_vip_discount() {
        let response = compute_loyalty_preview(&LoyaltyPreviewRequest {
            customer_email: "member@harbor.test".to_string(),
            membership_tier: MembershipTier::Standard,
            subtotal_gbp: 64.0,
            cart_skus: vec!["tasting-pass".to_string()],
            collection_handle: Some("events".to_string()),
        });

        assert_eq!(response.segment, "harbor-member");
        assert_eq!(response.discount_bps, 250);
        assert!(response.priority_fulfilment);
        assert!(
            response.concierge_note.contains("events desk"),
            "{}",
            response.concierge_note
        );
    }

    #[test]
    fn crm_winback_member_requires_follow_up() {
        let route = route_crm_contact(&CrmContactUpdate {
            customer_email: "member@harbor.test".to_string(),
            membership_tier: MembershipTier::Standard,
            lifecycle_stage: "winback".to_string(),
            last_order_total_gbp: Some(42.0),
        });

        assert_eq!(route.segment, "harbor-member");
        assert!(route.follow_up_required);
        assert!(
            route.follow_up_reason.contains("retention sequence"),
            "{}",
            route.follow_up_reason
        );
    }

    #[test]
    fn international_order_requires_manual_review() {
        let review = review_order(&OrderReviewRequest {
            customer_email: "captain@harbor.test".to_string(),
            membership_tier: MembershipTier::Standard,
            subtotal_gbp: 88.0,
            cart_skus: vec!["harbor-cap".to_string()],
            shipping_country: "IE".to_string(),
            expedited_requested: false,
        });

        assert!(review.review_required);
        assert_eq!(review.assigned_queue, "ops-manual-review");
        assert!(review.operator_note.contains("customs-safe packing"));
    }

    #[test]
    fn gold_member_domestic_order_uses_priority_lane() {
        let review = review_order(&OrderReviewRequest {
            customer_email: "gold@harbor.test".to_string(),
            membership_tier: MembershipTier::Gold,
            subtotal_gbp: 110.0,
            cart_skus: vec!["harbor-cap".to_string()],
            shipping_country: "GB".to_string(),
            expedited_requested: false,
        });

        assert!(!review.review_required);
        assert_eq!(review.assigned_queue, "vip-fulfilment");
        assert_eq!(review.service_level, "priority");
    }

    #[test]
    fn webhook_secret_fails_closed() {
        assert!(webhook_secret_matches(
            "harbor-backend-dev-secret",
            Some("harbor-backend-dev-secret")
        ));
        assert!(!webhook_secret_matches(
            "harbor-backend-dev-secret",
            Some("wrong-secret")
        ));
        assert!(!webhook_secret_matches("harbor-backend-dev-secret", None));
    }

    #[test]
    fn plugin_surface_wraps_the_same_customer_rules() {
        let backend = plugin();
        let review = backend.review_checkout_order(&OrderReviewRequest {
            customer_email: "captain@harbor.test".to_string(),
            membership_tier: MembershipTier::Gold,
            subtotal_gbp: 220.0,
            cart_skus: vec!["harbor-cap".to_string()],
            shipping_country: "GB".to_string(),
            expedited_requested: false,
        });

        assert!(review.review_required);
        assert_eq!(review.assigned_queue, "ops-manual-review");

        let route = backend.route_crm_contact_update(&CrmContactUpdate {
            customer_email: "member@harbor.test".to_string(),
            membership_tier: MembershipTier::Standard,
            lifecycle_stage: "winback".to_string(),
            last_order_total_gbp: Some(42.0),
        });

        assert!(route.follow_up_required);
    }

    #[test]
    fn plugin_descriptor_and_registered_hooks_stay_stable() {
        let descriptor = plugin_descriptor();

        assert_eq!(descriptor.id, "harbor-loyalty-backend");
        assert_eq!(descriptor.display_name, "Harbor Shop Loyalty Backend");
        assert_eq!(descriptor.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(
            descriptor.documentation_url.as_deref(),
            Some("apps/harbor-shop/backend/harbor-loyalty-backend/README.md")
        );
        assert_eq!(
            registered_hook_kinds(),
            vec![
                RegisteredHookKind::Checkout,
                RegisteredHookKind::VerifiedWebhook
            ]
        );
    }

    #[test]
    fn plugin_registers_sdk_hook_kinds() {
        let backend = plugin();
        let mut registry = RecordingRegistry::default();

        backend.register(&mut registry).unwrap();

        assert_eq!(
            registry.hook_kinds,
            vec![
                RegisteredHookKind::Checkout,
                RegisteredHookKind::VerifiedWebhook
            ]
        );
    }

    #[test]
    fn checkout_hook_maps_order_draft_to_harbor_review_logic() {
        let backend = plugin();
        let audit = RecordingAudit::default();
        let decision = CheckoutHooks::review_order(
            &backend,
            &request_context(),
            &OrderDraft {
                order_id: "ORD-HARBOR-1".to_string(),
                currency_code: "GBP".to_string(),
                subtotal: MoneyAmount::new("GBP", 8_800),
                total: MoneyAmount::new("GBP", 8_800),
                lines: vec![OrderLineDraft {
                    sku: "harbor-cap".to_string(),
                    title: "Harbor Cap".to_string(),
                    quantity: 1,
                    unit_price: MoneyAmount::new("GBP", 8_800),
                    product_kind: "merchandise".to_string(),
                    collection_handle: Some("featured".to_string()),
                    entitlement_key: None,
                    metadata: BTreeMap::new(),
                }],
                metadata: BTreeMap::from([
                    (
                        "customer_email".to_string(),
                        "captain@harbor.test".to_string(),
                    ),
                    ("membership_tier".to_string(), "standard".to_string()),
                    ("shipping_country".to_string(), "IE".to_string()),
                ]),
            },
            &NoopCommerce,
            &NoopAuth,
            &audit,
        )
        .unwrap();

        assert_eq!(
            decision,
            OrderReviewDecision::Adjusted(
                OrderAdjustment::new(
                    "Check customs-safe packing and confirm the carrier lane before capture."
                )
                .with_metadata_entries([
                    ("assigned_queue", "ops-manual-review"),
                    ("service_level", "manual-clearance"),
                    ("review_required", "true"),
                    (
                        "tags",
                        "customer-app:harbor-shop,queue:ops-manual-review,service-level:manual-clearance,ops:manual-review,shipping:international"
                    ),
                ])
            )
        );
        assert_eq!(
            audit.actions.lock().unwrap().as_slice(),
            ["customer-plugin.checkout-review"]
        );
    }

    #[test]
    fn verified_webhook_hook_routes_crm_updates_through_customer_rules() {
        let backend = plugin();
        let audit = RecordingAudit::default();
        let result = VerifiedWebhookHooks::handle_verified_webhook(
            &backend,
            &request_context(),
            &VerifiedWebhook {
                source: "crm".to_string(),
                event: "contact-updated".to_string(),
                headers: BTreeMap::new(),
                content_type: Some("application/json".to_string()),
                payload: serde_json::to_vec(&CrmContactUpdate {
                    customer_email: "member@harbor.test".to_string(),
                    membership_tier: MembershipTier::Standard,
                    lifecycle_stage: "winback".to_string(),
                    last_order_total_gbp: Some(42.0),
                })
                .unwrap(),
            },
            &NoopHttp,
            &NoopJobs,
            &audit,
        )
        .unwrap();

        assert_eq!(
            result,
            WebhookHandlingResult::accepted(Some(
                "Winback member should receive the Harbor Shop retention sequence. [customer-app:harbor-shop, segment:harbor-member, lifecycle:winback]".to_string()
            ))
        );
        assert_eq!(
            audit.actions.lock().unwrap().as_slice(),
            ["customer-plugin.verified-webhook"]
        );
    }
}
