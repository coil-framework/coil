use crate::{
    EntitlementStatus, MembershipInstant, MembershipModelError, MembershipTier, SubscriptionEvent,
    SubscriptionEventKind, SubscriptionStatus, TierChangeKind,
};

use super::Subscription;

impl Subscription {
    pub fn activate(
        &mut self,
        activated_at: MembershipInstant,
    ) -> Result<(), MembershipModelError> {
        match self.status {
            SubscriptionStatus::PendingActivation => {
                self.status = SubscriptionStatus::Active;
                self.provision_entitlement(activated_at, self.current_term_end)?;
                self.history.push(SubscriptionEvent {
                    at: activated_at,
                    kind: SubscriptionEventKind::Activated,
                });
                Ok(())
            }
            _ => Err(MembershipModelError::InvalidStatusTransition {
                from: self.status,
                to: SubscriptionStatus::Active,
            }),
        }
    }

    pub fn schedule_cancellation(
        &mut self,
        scheduled_at: MembershipInstant,
    ) -> Result<(), MembershipModelError> {
        match self.status {
            SubscriptionStatus::PendingActivation
            | SubscriptionStatus::Active
            | SubscriptionStatus::InGracePeriod
            | SubscriptionStatus::Paused => {
                self.cancel_at_period_end = true;
                self.history.push(SubscriptionEvent {
                    at: scheduled_at,
                    kind: SubscriptionEventKind::CancellationScheduled,
                });
                Ok(())
            }
            _ => Err(MembershipModelError::InvalidStatusTransition {
                from: self.status,
                to: SubscriptionStatus::Cancelled,
            }),
        }
    }

    pub fn cancel_immediately(
        &mut self,
        cancelled_at: MembershipInstant,
    ) -> Result<(), MembershipModelError> {
        match self.status {
            SubscriptionStatus::PendingActivation
            | SubscriptionStatus::Active
            | SubscriptionStatus::InGracePeriod
            | SubscriptionStatus::Paused => {
                self.status = SubscriptionStatus::Cancelled;
                self.cancel_at_period_end = false;
                self.grace_period_ends_at = None;
                self.revoke_entitlements(cancelled_at);
                self.history.push(SubscriptionEvent {
                    at: cancelled_at,
                    kind: SubscriptionEventKind::Cancelled,
                });
                Ok(())
            }
            _ => Err(MembershipModelError::InvalidStatusTransition {
                from: self.status,
                to: SubscriptionStatus::Cancelled,
            }),
        }
    }

    pub fn pause(&mut self, paused_at: MembershipInstant) -> Result<(), MembershipModelError> {
        match self.status {
            SubscriptionStatus::Active | SubscriptionStatus::InGracePeriod => {
                self.status = SubscriptionStatus::Paused;
                self.revoke_entitlements(paused_at);
                self.history.push(SubscriptionEvent {
                    at: paused_at,
                    kind: SubscriptionEventKind::Paused,
                });
                Ok(())
            }
            _ => Err(MembershipModelError::InvalidStatusTransition {
                from: self.status,
                to: SubscriptionStatus::Paused,
            }),
        }
    }

    pub fn resume(&mut self, resumed_at: MembershipInstant) -> Result<(), MembershipModelError> {
        match self.status {
            SubscriptionStatus::Paused => {
                self.status = SubscriptionStatus::Active;
                self.provision_entitlement(resumed_at, self.current_term_end)?;
                self.history.push(SubscriptionEvent {
                    at: resumed_at,
                    kind: SubscriptionEventKind::Resumed,
                });
                Ok(())
            }
            _ => Err(MembershipModelError::InvalidStatusTransition {
                from: self.status,
                to: SubscriptionStatus::Active,
            }),
        }
    }

    pub fn apply_renewal_success(
        &mut self,
        renewed_at: MembershipInstant,
        tier: &MembershipTier,
    ) -> Result<(), MembershipModelError> {
        match self.status {
            SubscriptionStatus::Active | SubscriptionStatus::InGracePeriod => {
                self.status = SubscriptionStatus::Active;
                self.term_started_at = self.current_term_end;
                self.current_term_end = self
                    .current_term_end
                    .checked_add("current_term_end", tier.interval.term_duration())?;
                self.renewal_due_at = self.current_term_end;
                self.grace_period_ends_at = None;
                self.cancel_at_period_end = false;
                self.provision_entitlement(renewed_at, self.current_term_end)?;
                self.history.push(SubscriptionEvent {
                    at: renewed_at,
                    kind: SubscriptionEventKind::Renewed,
                });
                Ok(())
            }
            _ => Err(MembershipModelError::InvalidStatusTransition {
                from: self.status,
                to: SubscriptionStatus::Active,
            }),
        }
    }

    pub fn apply_renewal_failure(
        &mut self,
        failed_at: MembershipInstant,
        tier: &MembershipTier,
    ) -> Result<(), MembershipModelError> {
        match self.status {
            SubscriptionStatus::Active => {
                if tier.grace_period_days == 0 {
                    self.expire(failed_at)?;
                    return Ok(());
                }

                let grace_ends_at = failed_at
                    .checked_add_days("grace_period_ends_at", u64::from(tier.grace_period_days))?;
                self.status = SubscriptionStatus::InGracePeriod;
                self.grace_period_ends_at = Some(grace_ends_at);
                self.provision_entitlement(failed_at, grace_ends_at)?;
                self.history.push(SubscriptionEvent {
                    at: failed_at,
                    kind: SubscriptionEventKind::EnteredGracePeriod,
                });
                Ok(())
            }
            _ => Err(MembershipModelError::InvalidStatusTransition {
                from: self.status,
                to: SubscriptionStatus::InGracePeriod,
            }),
        }
    }

    pub fn expire_if_grace_elapsed(
        &mut self,
        now: MembershipInstant,
    ) -> Result<bool, MembershipModelError> {
        match (self.status, self.grace_period_ends_at) {
            (SubscriptionStatus::InGracePeriod, Some(grace_ends_at)) if now >= grace_ends_at => {
                self.expire(now)?;
                Ok(true)
            }
            (SubscriptionStatus::InGracePeriod, Some(_)) => Ok(false),
            _ => Ok(false),
        }
    }

    pub fn settle_period_boundary(
        &mut self,
        now: MembershipInstant,
    ) -> Result<bool, MembershipModelError> {
        if now < self.current_term_end {
            return Ok(false);
        }

        if self.cancel_at_period_end {
            self.cancel_immediately(now)?;
            return Ok(true);
        }

        Ok(false)
    }

    pub fn change_tier(
        &mut self,
        new_tier: &MembershipTier,
        changed_at: MembershipInstant,
        previous_rank: u16,
    ) -> Result<TierChangeKind, MembershipModelError> {
        let kind = match new_tier.rank.cmp(&previous_rank) {
            std::cmp::Ordering::Greater => TierChangeKind::Upgrade,
            std::cmp::Ordering::Less => TierChangeKind::Downgrade,
            std::cmp::Ordering::Equal => TierChangeKind::Lateral,
        };

        let previous_tier = self.tier_id.clone();
        self.tier_id = new_tier.id.clone();
        self.entitlement_key = new_tier.entitlement_key.clone();
        self.interval = new_tier.interval;
        self.grace_period_ends_at = None;
        self.revoke_entitlements(changed_at);

        if matches!(
            self.status,
            SubscriptionStatus::Active | SubscriptionStatus::InGracePeriod
        ) {
            self.status = SubscriptionStatus::Active;
            self.provision_entitlement(changed_at, self.current_term_end)?;
        }

        self.history.push(SubscriptionEvent {
            at: changed_at,
            kind: SubscriptionEventKind::TierChanged {
                from: previous_tier,
                to: new_tier.id.clone(),
                kind,
            },
        });

        Ok(kind)
    }

    fn provision_entitlement(
        &mut self,
        active_from: MembershipInstant,
        active_until: MembershipInstant,
    ) -> Result<(), MembershipModelError> {
        if !matches!(
            self.status,
            SubscriptionStatus::PendingActivation
                | SubscriptionStatus::Active
                | SubscriptionStatus::InGracePeriod
                | SubscriptionStatus::Paused
        ) {
            return Err(MembershipModelError::SubscriptionNotProvisionable {
                subscription_id: self.id.to_string(),
                status: self.status,
            });
        }

        self.revoke_entitlements(active_from);
        self.entitlements.push(super::EntitlementGrant::active(
            self.entitlement_key.clone(),
            self.id.clone(),
            active_from,
            active_until,
        ));
        Ok(())
    }

    fn revoke_entitlements(&mut self, revoked_at: MembershipInstant) {
        for entitlement in &mut self.entitlements {
            if entitlement.status == EntitlementStatus::Active {
                entitlement.revoke(revoked_at);
            }
        }
    }

    fn expire(&mut self, expired_at: MembershipInstant) -> Result<(), MembershipModelError> {
        match self.status {
            SubscriptionStatus::Active
            | SubscriptionStatus::InGracePeriod
            | SubscriptionStatus::Paused => {
                self.status = SubscriptionStatus::Expired;
                self.grace_period_ends_at = None;
                self.cancel_at_period_end = false;
                self.revoke_entitlements(expired_at);
                self.history.push(SubscriptionEvent {
                    at: expired_at,
                    kind: SubscriptionEventKind::Expired,
                });
                Ok(())
            }
            _ => Err(MembershipModelError::InvalidStatusTransition {
                from: self.status,
                to: SubscriptionStatus::Expired,
            }),
        }
    }
}
