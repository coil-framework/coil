use std::collections::BTreeMap;

use davenda_commerce::{EntitlementKey, OrderId, OrderOutcome};

use crate::MembershipModelError;
use crate::model::{
    MemberAccountId, MembershipInstant, MembershipTier, MembershipTierId, SubscriptionId,
};
use crate::subscription::Subscription;
use crate::validation::ensure_positive_quantity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionedSubscription {
    pub subscription: Subscription,
    pub source_order_id: OrderId,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MembershipCatalog {
    tiers: BTreeMap<MembershipTierId, MembershipTier>,
    entitlement_index: BTreeMap<String, MembershipTierId>,
}

impl MembershipCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_tier(&mut self, tier: MembershipTier) -> Result<(), MembershipModelError> {
        if self.tiers.contains_key(&tier.id) {
            return Err(MembershipModelError::DuplicateTier {
                tier_id: tier.id.to_string(),
            });
        }

        if self
            .entitlement_index
            .contains_key(tier.entitlement_key.as_str())
        {
            return Err(MembershipModelError::DuplicateEntitlementKey {
                entitlement_key: tier.entitlement_key.to_string(),
            });
        }

        self.entitlement_index
            .insert(tier.entitlement_key.to_string(), tier.id.clone());
        self.tiers.insert(tier.id.clone(), tier);
        Ok(())
    }

    pub fn tier(&self, id: &MembershipTierId) -> Option<&MembershipTier> {
        self.tiers.get(id)
    }

    pub fn tier_for_entitlement(
        &self,
        entitlement_key: &EntitlementKey,
    ) -> Option<&MembershipTier> {
        let tier_id = self.entitlement_index.get(entitlement_key.as_str())?;
        self.tiers.get(tier_id)
    }

    pub fn provision_from_order_outcomes(
        &self,
        order_id: OrderId,
        member_id: MemberAccountId,
        outcomes: &[OrderOutcome],
        starts_at: MembershipInstant,
    ) -> Result<Vec<ProvisionedSubscription>, MembershipModelError> {
        let mut provisioned = Vec::new();
        let mut next_index = 1u32;

        for outcome in outcomes {
            if let OrderOutcome::GrantMembership {
                entitlement_key,
                quantity,
            } = outcome
            {
                ensure_positive_quantity("membership_quantity", *quantity)?;
                let tier = self.tier_for_entitlement(entitlement_key).ok_or_else(|| {
                    MembershipModelError::MissingTierForEntitlement {
                        entitlement_key: entitlement_key.to_string(),
                    }
                })?;

                for _ in 0..*quantity {
                    let subscription_id =
                        SubscriptionId::new(format!("sub-{}-{next_index}", order_id.as_str()))?;
                    next_index += 1;
                    let subscription = Subscription::from_order(
                        subscription_id,
                        member_id.clone(),
                        tier,
                        order_id.clone(),
                        starts_at,
                    )?;
                    provisioned.push(ProvisionedSubscription {
                        subscription,
                        source_order_id: order_id.clone(),
                    });
                }
            }
        }

        Ok(provisioned)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenewalWorkItem {
    pub subscription_id: SubscriptionId,
    pub scheduled_for: MembershipInstant,
}

impl RenewalWorkItem {
    pub fn new(subscription_id: SubscriptionId, scheduled_for: MembershipInstant) -> Self {
        Self {
            subscription_id,
            scheduled_for,
        }
    }
}
