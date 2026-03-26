mod http;

pub use http::{BackendConfig, build_router};

use serde::{Deserialize, Serialize};

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

pub fn webhook_secret_matches(expected: &str, provided: Option<&str>) -> bool {
    match provided {
        Some(value) => value == expected,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
