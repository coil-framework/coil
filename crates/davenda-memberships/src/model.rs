use std::collections::BTreeSet;
use std::fmt;
use std::time::Duration;

use davenda_commerce::{EntitlementKey, OrderId};

use crate::MembershipModelError;
use crate::validation::{require_non_empty, validate_token};

pub const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

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
        let mut seen = BTreeSet::new();
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
