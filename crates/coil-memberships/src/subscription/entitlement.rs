use coil_commerce::EntitlementKey;

use crate::{MembershipInstant, SubscriptionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntitlementStatus {
    Active,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitlementGrant {
    pub key: EntitlementKey,
    pub subscription_id: SubscriptionId,
    pub active_from: MembershipInstant,
    pub active_until: MembershipInstant,
    pub status: EntitlementStatus,
    pub revoked_at: Option<MembershipInstant>,
}

impl EntitlementGrant {
    pub(super) fn active(
        key: EntitlementKey,
        subscription_id: SubscriptionId,
        active_from: MembershipInstant,
        active_until: MembershipInstant,
    ) -> Self {
        Self {
            key,
            subscription_id,
            active_from,
            active_until,
            status: EntitlementStatus::Active,
            revoked_at: None,
        }
    }

    pub(super) fn revoke(&mut self, revoked_at: MembershipInstant) {
        self.status = EntitlementStatus::Revoked;
        self.active_until = revoked_at;
        self.revoked_at = Some(revoked_at);
    }

    pub fn is_active_at(&self, now: MembershipInstant) -> bool {
        self.status == EntitlementStatus::Active
            && now >= self.active_from
            && now < self.active_until
    }
}
