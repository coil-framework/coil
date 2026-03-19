use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::time::Duration;

use davenda_auth::Capability;
use davenda_commerce::{EntitlementKey, OrderId, OrderOutcome};
use davenda_core::{
    AdminContributionKind, AdminNavigationSection, AdminResourceContribution, CapabilityContract,
    CoreServiceDependency, EventSubscription, ExtensionSlotDescriptor, ExtensionSlotKind,
    HttpSurfaceArea, HttpSurfaceContribution, IntegrationKind, IntegrationPoint, JobContract,
    JobTriggerKind, MigrationContract, ModuleBehavior, ModuleDependency, ModuleManifest,
    PlatformModule, RegistrationError, ReportDefinition, ReportDeliveryMode, ReportFormat,
    ReportSensitivity, RouteSurface, RouteSurfaceKind, SearchDocumentKind, SearchFieldContribution,
    SearchFieldRole, SearchIndexContribution, SearchInvalidationRule, SearchInvalidationTrigger,
    SearchRebuildStrategy, SearchVisibility, ServiceRegistry,
};
use davenda_data::{MigrationId, MigrationOwner, MigrationPlan, MigrationStep};
use davenda_jobs::RetryPolicy;

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

mod module;
pub use module::MembershipsModule;

fn default_retry_policy() -> RetryPolicy {
    RetryPolicy::new(3, Duration::from_secs(15), Duration::from_secs(300))
        .expect("constant retry policy is valid")
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
mod tests;
