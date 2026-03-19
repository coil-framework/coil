use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::time::Duration;

use davenda_auth::Capability;
use davenda_commerce::{EntitlementKey, OrderId, OrderOutcome};
use davenda_core::{
    CapabilityContract, CoreServiceDependency, EventSubscription, ExtensionSlotDescriptor,
    ExtensionSlotKind, IntegrationKind, IntegrationPoint, JobContract, JobTriggerKind,
    MigrationContract, ModuleBehavior, ModuleDependency, ModuleManifest, PlatformModule,
    RegistrationError, RouteSurface, RouteSurfaceKind, ServiceRegistry,
};
use davenda_data::{MigrationId, MigrationOwner, MigrationPlan, MigrationStep};

const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MembershipModelError {
    EmptyField {
        field: &'static str,
    },
    InvalidToken {
        field: &'static str,
        value: String,
    },
    InvalidRoute {
        field: &'static str,
        value: String,
    },
    DuplicateBenefit {
        key: String,
    },
    DuplicateTier {
        tier_id: String,
    },
    DuplicateEntitlementKey {
        entitlement_key: String,
    },
    MissingTierForEntitlement {
        entitlement_key: String,
    },
    InvalidQuantity {
        field: &'static str,
        quantity: u32,
    },
    InvalidStatusTransition {
        from: SubscriptionStatus,
        to: SubscriptionStatus,
    },
    TimestampOverflow {
        field: &'static str,
        base: u64,
        offset_seconds: u64,
    },
    SubscriptionNotProvisionable {
        subscription_id: String,
        status: SubscriptionStatus,
    },
}

impl fmt::Display for MembershipModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::InvalidRoute { field, value } => {
                write!(f, "`{field}` must start with `/`, got `{value}`")
            }
            Self::DuplicateBenefit { key } => {
                write!(f, "membership benefit `{key}` is duplicated")
            }
            Self::DuplicateTier { tier_id } => {
                write!(f, "membership tier `{tier_id}` is duplicated")
            }
            Self::DuplicateEntitlementKey { entitlement_key } => write!(
                f,
                "membership entitlement key `{entitlement_key}` is already assigned to a tier"
            ),
            Self::MissingTierForEntitlement { entitlement_key } => write!(
                f,
                "no membership tier is registered for entitlement key `{entitlement_key}`"
            ),
            Self::InvalidQuantity { field, quantity } => {
                write!(f, "`{field}` must be greater than zero, got `{quantity}`")
            }
            Self::InvalidStatusTransition { from, to } => {
                write!(f, "cannot transition subscription from `{from}` to `{to}`")
            }
            Self::TimestampOverflow {
                field,
                base,
                offset_seconds,
            } => write!(
                f,
                "timestamp overflow while calculating `{field}` from `{base}` plus `{offset_seconds}` seconds"
            ),
            Self::SubscriptionNotProvisionable {
                subscription_id,
                status,
            } => write!(
                f,
                "subscription `{subscription_id}` cannot provision entitlements while `{status}`"
            ),
        }
    }
}

impl Error for MembershipModelError {}

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, MembershipModelError> {
                Ok(Self(validate_token($field, value.into())?))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

token_type!(MembershipTierId, "membership_tier_id");
token_type!(SubscriptionId, "subscription_id");
token_type!(MemberAccountId, "member_account_id");
token_type!(BenefitKey, "benefit_key");

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MembershipInstant(u64);

impl MembershipInstant {
    pub const fn from_unix_seconds(seconds: u64) -> Self {
        Self(seconds)
    }

    pub const fn as_unix_seconds(self) -> u64 {
        self.0
    }

    pub const fn from_days(days: u64) -> Self {
        Self(days * SECONDS_PER_DAY)
    }

    pub fn checked_add(
        self,
        field: &'static str,
        duration: Duration,
    ) -> Result<Self, MembershipModelError> {
        let offset_seconds = duration.as_secs();
        let next =
            self.0
                .checked_add(offset_seconds)
                .ok_or(MembershipModelError::TimestampOverflow {
                    field,
                    base: self.0,
                    offset_seconds,
                })?;
        Ok(Self(next))
    }

    pub fn checked_add_days(
        self,
        field: &'static str,
        days: u64,
    ) -> Result<Self, MembershipModelError> {
        self.checked_add(
            field,
            Duration::from_secs(days.saturating_mul(SECONDS_PER_DAY)),
        )
    }
}

impl fmt::Display for MembershipInstant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillingInterval {
    Monthly,
    Quarterly,
    Annual,
    CustomDays(u16),
}

impl BillingInterval {
    pub const fn term_days(self) -> u16 {
        match self {
            Self::Monthly => 30,
            Self::Quarterly => 90,
            Self::Annual => 365,
            Self::CustomDays(days) => days,
        }
    }

    pub fn term_duration(self) -> Duration {
        Duration::from_secs(u64::from(self.term_days()) * SECONDS_PER_DAY)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TierVisibility {
    Public,
    InviteOnly,
    StaffManaged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenefitKind {
    ContentAccess,
    EventEligibility,
    MemberPricing,
    MediaAccess,
    AccountExperience,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MembershipBenefit {
    pub key: BenefitKey,
    pub kind: BenefitKind,
    pub description: String,
}

impl MembershipBenefit {
    pub fn new(
        key: BenefitKey,
        kind: BenefitKind,
        description: impl Into<String>,
    ) -> Result<Self, MembershipModelError> {
        Ok(Self {
            key,
            kind,
            description: require_non_empty("benefit_description", description.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MembershipTier {
    pub id: MembershipTierId,
    pub title: String,
    pub entitlement_key: EntitlementKey,
    pub rank: u16,
    pub interval: BillingInterval,
    pub grace_period_days: u16,
    pub visibility: TierVisibility,
    benefits: Vec<MembershipBenefit>,
}

impl MembershipTier {
    pub fn new(
        id: MembershipTierId,
        title: impl Into<String>,
        entitlement_key: EntitlementKey,
        rank: u16,
        interval: BillingInterval,
        grace_period_days: u16,
        visibility: TierVisibility,
        benefits: Vec<MembershipBenefit>,
    ) -> Result<Self, MembershipModelError> {
        let mut seen = std::collections::BTreeSet::new();
        for benefit in &benefits {
            if !seen.insert(benefit.key.clone()) {
                return Err(MembershipModelError::DuplicateBenefit {
                    key: benefit.key.to_string(),
                });
            }
        }

        Ok(Self {
            id,
            title: require_non_empty("tier_title", title.into())?,
            entitlement_key,
            rank,
            interval,
            grace_period_days,
            visibility,
            benefits,
        })
    }

    pub fn benefits(&self) -> &[MembershipBenefit] {
        &self.benefits
    }

    pub fn benefit(&self, key: &BenefitKey) -> Option<&MembershipBenefit> {
        self.benefits.iter().find(|benefit| &benefit.key == key)
    }
}

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
    fn active(
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

    fn revoke(&mut self, revoked_at: MembershipInstant) {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionStatus {
    PendingActivation,
    Active,
    InGracePeriod,
    Paused,
    Cancelled,
    Expired,
}

impl fmt::Display for SubscriptionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PendingActivation => f.write_str("pending_activation"),
            Self::Active => f.write_str("active"),
            Self::InGracePeriod => f.write_str("in_grace_period"),
            Self::Paused => f.write_str("paused"),
            Self::Cancelled => f.write_str("cancelled"),
            Self::Expired => f.write_str("expired"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TierChangeKind {
    Upgrade,
    Downgrade,
    Lateral,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionEventKind {
    CreatedFromOrder {
        order_id: OrderId,
    },
    Activated,
    Renewed,
    EnteredGracePeriod,
    CancellationScheduled,
    Cancelled,
    Paused,
    Resumed,
    Expired,
    TierChanged {
        from: MembershipTierId,
        to: MembershipTierId,
        kind: TierChangeKind,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionEvent {
    pub at: MembershipInstant,
    pub kind: SubscriptionEventKind,
}

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
    entitlements: Vec<EntitlementGrant>,
    history: Vec<SubscriptionEvent>,
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
        self.entitlements.push(EntitlementGrant::active(
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminResourceDescriptor {
    pub route: String,
    pub capability: Capability,
    pub title: String,
}

impl AdminResourceDescriptor {
    pub fn new(
        route: impl Into<String>,
        capability: Capability,
        title: impl Into<String>,
    ) -> Result<Self, MembershipModelError> {
        Ok(Self {
            route: validate_route("admin_route", route.into())?,
            capability,
            title: require_non_empty("admin_title", title.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MembershipsModule {
    name: String,
    config_namespace: String,
    admin_resources: Vec<AdminResourceDescriptor>,
}

impl MembershipsModule {
    pub fn new() -> Self {
        Self {
            name: "memberships".to_string(),
            config_namespace: "memberships".to_string(),
            admin_resources: vec![
                AdminResourceDescriptor::new(
                    "/admin/memberships/tiers",
                    Capability::MembershipTierEdit,
                    "Membership tiers",
                )
                .expect("constant admin route is valid"),
                AdminResourceDescriptor::new(
                    "/admin/memberships/subscriptions",
                    Capability::MembershipSubscriptionManage,
                    "Subscriptions",
                )
                .expect("constant admin route is valid"),
            ],
        }
    }

    pub fn admin_resources(&self) -> &[AdminResourceDescriptor] {
        &self.admin_resources
    }
}

impl Default for MembershipsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformModule for MembershipsModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new(self.name.clone())
            .with_required_capabilities(vec![
                Capability::MembershipSubscriptionManage,
                Capability::MembershipTierEdit,
            ])
            .with_optional_capabilities(vec![
                Capability::AdminShellAccess,
                Capability::OrderRead,
                Capability::I18nTranslationEdit,
                Capability::AssetRead,
            ])
            .with_config_namespace(self.config_namespace.clone())
            .with_capability_contracts(vec![
                CapabilityContract::required(
                    Capability::MembershipSubscriptionManage,
                    ["subscription"],
                ),
                CapabilityContract::required(
                    Capability::MembershipTierEdit,
                    ["membership_tier"],
                ),
                CapabilityContract::optional(
                    Capability::AdminShellAccess,
                    ["admin_module"],
                ),
                CapabilityContract::optional(Capability::OrderRead, ["order"]),
                CapabilityContract::optional(
                    Capability::I18nTranslationEdit,
                    ["membership_tier"],
                ),
                CapabilityContract::optional(Capability::AssetRead, ["asset", "media"]),
            ])
            .with_module_dependencies(vec![
                ModuleDependency::required(
                    "commerce",
                    "Membership subscriptions are provisioned from order outcomes and billing lifecycles",
                ),
                ModuleDependency::optional(
                    "admin",
                    "Memberships contributes operator resources into the shared admin shell when installed",
                ),
                ModuleDependency::optional(
                    "events",
                    "Membership tiers can influence event eligibility and member-only booking workflows",
                ),
            ])
            .with_core_service_dependencies(vec![
                CoreServiceDependency::Auth,
                CoreServiceDependency::Data,
                CoreServiceDependency::Jobs,
                CoreServiceDependency::Observability,
                CoreServiceDependency::I18n,
            ])
            .with_migrations(vec![
                MigrationContract::new(
                    "memberships.tiers",
                    10,
                    "Creates membership tier, benefit, and merchandising policy tables",
                ),
                MigrationContract::new(
                    "memberships.subscriptions",
                    20,
                    "Creates subscription lifecycle state, term, and grace-period tables",
                ),
                MigrationContract::new(
                    "memberships.entitlements",
                    30,
                    "Creates entitlement grants and revocation audit rows linked to active subscriptions",
                ),
            ])
            .with_route_surfaces(vec![
                RouteSurface::new(
                    "memberships.account",
                    RouteSurfaceKind::FrontendPage,
                    "/account/memberships",
                )
                .gated_by(Capability::MembershipSubscriptionManage),
                RouteSurface::new(
                    "memberships.tiers",
                    RouteSurfaceKind::AdminPage,
                    "/admin/memberships/tiers",
                )
                .gated_by(Capability::MembershipTierEdit),
                RouteSurface::new(
                    "memberships.subscriptions",
                    RouteSurfaceKind::AdminPage,
                    "/admin/memberships/subscriptions",
                )
                .gated_by(Capability::MembershipSubscriptionManage),
            ])
            .with_jobs(vec![
                JobContract::new(
                    "memberships.renewals",
                    JobTriggerKind::Scheduled,
                    true,
                    "Processes scheduled renewals, grace-period transitions, and retry windows",
                ),
                JobContract::new(
                    "memberships.entitlements.sync",
                    JobTriggerKind::DomainEvent,
                    true,
                    "Reconciles auth-backed entitlements after subscription lifecycle changes",
                ),
            ])
            .with_event_subscriptions(vec![
                EventSubscription::new(
                    "commerce.order.paid",
                    Some("memberships.entitlements.sync"),
                    "Creates or extends subscription access after qualifying membership purchases complete",
                ),
                EventSubscription::new(
                    "membership.subscription.renewal-due",
                    Some("memberships.renewals"),
                    "Schedules renewal and grace-period maintenance work for active subscriptions",
                ),
            ])
            .with_integration_points(vec![
                IntegrationPoint::new(
                    IntegrationKind::AdminNavigation,
                    "admin.memberships",
                    "Adds tier and subscription management resources to the shared operator shell",
                ),
                IntegrationPoint::new(
                    IntegrationKind::CommerceBridge,
                    "commerce.orders",
                    "Projects order outcomes into recurring membership state and entitlement grants",
                ),
                IntegrationPoint::new(
                    IntegrationKind::FrontendRendering,
                    "account.memberships",
                    "Provides the member account experience and entitlement visibility surface",
                ),
            ])
            .with_behaviors(vec![
                ModuleBehavior::AccessibleAdminUi,
                ModuleBehavior::AsyncJobs,
                ModuleBehavior::AuditedBulkActions,
            ])
            .with_extension_slots(vec![ExtensionSlotDescriptor::new(
                ExtensionSlotKind::AdminWidget,
                "memberships.subscription.summary",
                "Allows customer app widgets to augment subscription detail views with bounded insights",
            )])
    }

    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError> {
        registry.register_module_service(
            self.name.clone(),
            "module.memberships.tiers",
            "Membership tiers, benefits, and plan configuration",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.memberships.subscriptions",
            "Subscription lifecycle, grace periods, pause and cancellation handling",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.memberships.entitlements",
            "Entitlement grants and revocation aligned with auth-backed member access",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.memberships.renewals",
            "Renewal scheduling, retry orchestration, and subscription follow-up work",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.memberships.commerce_bridge",
            "Commerce order outcomes translated into membership subscription state",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.memberships.admin",
            "Membership operator resources for tiers, subscriptions, and entitlement review",
        )
    }

    fn install_migration_plan(&self) -> Option<MigrationPlan> {
        let owner = MigrationOwner::Module(self.name.clone());
        let mut plan = MigrationPlan::new();
        plan.insert(
            MigrationStep::new(
                MigrationId::new("membership_tiers").expect("constant migration id is valid"),
                owner.clone(),
                10,
                "Create membership tier and benefit storage",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS membership_tiers (id TEXT PRIMARY KEY, name TEXT NOT NULL, status TEXT NOT NULL)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("membership migration ids are unique");
        plan.insert(
            MigrationStep::new(
                MigrationId::new("membership_subscriptions")
                    .expect("constant migration id is valid"),
                owner.clone(),
                20,
                "Create subscription lifecycle and renewal state storage",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS membership_subscriptions (id TEXT PRIMARY KEY, tier_id TEXT NOT NULL, status TEXT NOT NULL, renews_at BIGINT)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("membership migration ids are unique");
        plan.insert(
            MigrationStep::new(
                MigrationId::new("membership_entitlements")
                    .expect("constant migration id is valid"),
                owner,
                30,
                "Create entitlement grant and revocation audit storage",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS membership_entitlements (id TEXT PRIMARY KEY, subscription_id TEXT NOT NULL, entitlement_key TEXT NOT NULL, active BOOLEAN NOT NULL)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("membership migration ids are unique");
        Some(plan)
    }
}

fn validate_token(field: &'static str, value: String) -> Result<String, MembershipModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(MembershipModelError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(MembershipModelError::InvalidToken {
            field,
            value: trimmed.to_string(),
        })
    }
}

fn validate_route(field: &'static str, value: String) -> Result<String, MembershipModelError> {
    let route = require_non_empty(field, value)?;
    if route.starts_with('/') {
        Ok(route)
    } else {
        Err(MembershipModelError::InvalidRoute {
            field,
            value: route,
        })
    }
}

fn require_non_empty(field: &'static str, value: String) -> Result<String, MembershipModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(MembershipModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

fn ensure_positive_quantity(
    field: &'static str,
    quantity: u32,
) -> Result<(), MembershipModelError> {
    if quantity == 0 {
        Err(MembershipModelError::InvalidQuantity { field, quantity })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                        sku: davenda_commerce::Sku::new("ebook").unwrap(),
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
        assert!(manifest
            .module_dependencies
            .iter()
            .any(|dependency| dependency.module == "commerce"));
        assert!(manifest
            .core_service_dependencies
            .contains(&CoreServiceDependency::Jobs));
        assert_eq!(manifest.migrations.len(), 3);
        assert_eq!(manifest.route_surfaces.len(), 3);
        assert_eq!(manifest.jobs.len(), 2);
        assert_eq!(manifest.event_subscriptions.len(), 2);
        assert_eq!(
            module
                .install_migration_plan()
                .expect("memberships migration plan")
                .ordered_steps()
                .len(),
            3
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
}
