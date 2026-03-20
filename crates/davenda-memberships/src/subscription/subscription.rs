use davenda_commerce::{EntitlementKey, OrderId};

use crate::{
    BillingInterval, MemberAccountId, MembershipInstant, MembershipModelError, MembershipTier,
    MembershipTierId, SubscriptionEvent, SubscriptionEventKind, SubscriptionId, SubscriptionStatus,
};

use super::EntitlementGrant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    pub id: SubscriptionId,
    pub member_id: MemberAccountId,
    pub tier_id: MembershipTierId,
    pub entitlement_key: EntitlementKey,
    pub interval: BillingInterval,
    pub status: SubscriptionStatus,
    pub term_started_at: MembershipInstant,
    pub current_term_end: MembershipInstant,
    pub renewal_due_at: MembershipInstant,
    pub grace_period_ends_at: Option<MembershipInstant>,
    pub cancel_at_period_end: bool,
    pub(super) entitlements: Vec<EntitlementGrant>,
    pub(super) history: Vec<SubscriptionEvent>,
}

impl Subscription {
    pub fn from_order(
        id: SubscriptionId,
        member_id: MemberAccountId,
        tier: &MembershipTier,
        order_id: OrderId,
        starts_at: MembershipInstant,
    ) -> Result<Self, MembershipModelError> {
        let current_term_end =
            starts_at.checked_add("current_term_end", tier.interval.term_duration())?;
        Ok(Self {
            id,
            member_id,
            tier_id: tier.id.clone(),
            entitlement_key: tier.entitlement_key.clone(),
            interval: tier.interval,
            status: SubscriptionStatus::PendingActivation,
            term_started_at: starts_at,
            current_term_end,
            renewal_due_at: current_term_end,
            grace_period_ends_at: None,
            cancel_at_period_end: false,
            entitlements: Vec::new(),
            history: vec![SubscriptionEvent {
                at: starts_at,
                kind: SubscriptionEventKind::CreatedFromOrder { order_id },
            }],
        })
    }

    pub fn entitlements(&self) -> &[EntitlementGrant] {
        &self.entitlements
    }

    pub fn history(&self) -> &[SubscriptionEvent] {
        &self.history
    }

    pub fn is_active_for_access(&self) -> bool {
        matches!(
            self.status,
            SubscriptionStatus::Active | SubscriptionStatus::InGracePeriod
        )
    }
}
