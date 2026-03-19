use std::error::Error;
use std::fmt;

use crate::SubscriptionStatus;

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
