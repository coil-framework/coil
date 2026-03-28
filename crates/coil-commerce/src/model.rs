use crate::error::CommerceModelError;
use crate::identifiers::CurrencyCode;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductKind {
    Physical,
    Digital,
    Service,
    Membership {
        entitlement_key: crate::EntitlementKey,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductStatus {
    Draft,
    Active,
    Archived,
}

impl fmt::Display for ProductStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Draft => f.write_str("draft"),
            Self::Active => f.write_str("active"),
            Self::Archived => f.write_str("archived"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckoutStatus {
    Draft,
    ReadyForPayment,
    AwaitingPayment,
    Paid,
    Completed,
    Cancelled,
}

impl fmt::Display for CheckoutStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Draft => f.write_str("draft"),
            Self::ReadyForPayment => f.write_str("ready_for_payment"),
            Self::AwaitingPayment => f.write_str("awaiting_payment"),
            Self::Paid => f.write_str("paid"),
            Self::Completed => f.write_str("completed"),
            Self::Cancelled => f.write_str("cancelled"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    PendingPayment,
    Paid,
    Fulfilled,
    PartiallyRefunded,
    Refunded,
    Cancelled,
}

impl fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PendingPayment => f.write_str("pending_payment"),
            Self::Paid => f.write_str("paid"),
            Self::Fulfilled => f.write_str("fulfilled"),
            Self::PartiallyRefunded => f.write_str("partially_refunded"),
            Self::Refunded => f.write_str("refunded"),
            Self::Cancelled => f.write_str("cancelled"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdjustmentKind {
    Promotion,
    Voucher,
    MembershipBenefit,
    Shipping,
    Tax,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdjustmentDirection {
    Discount,
    Surcharge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Money {
    currency: CurrencyCode,
    amount_minor: i64,
}

impl Money {
    pub fn new(currency: CurrencyCode, amount_minor: i64) -> Result<Self, CommerceModelError> {
        if amount_minor < 0 {
            return Err(CommerceModelError::NegativeAmount {
                field: "amount_minor",
                amount_minor,
            });
        }

        Ok(Self {
            currency,
            amount_minor,
        })
    }

    pub fn zero(currency: CurrencyCode) -> Self {
        Self {
            currency,
            amount_minor: 0,
        }
    }

    pub fn currency(&self) -> &CurrencyCode {
        &self.currency
    }

    pub fn amount_minor(&self) -> i64 {
        self.amount_minor
    }

    pub fn checked_add(&self, other: &Self) -> Result<Self, CommerceModelError> {
        crate::pricing::ensure_same_currency(&self.currency, &other.currency)?;
        let amount = self.amount_minor.checked_add(other.amount_minor).ok_or(
            CommerceModelError::AmountOverflow {
                field: "money_addition",
            },
        )?;
        Self::new(self.currency.clone(), amount)
    }

    pub fn checked_mul(&self, quantity: u32) -> Result<Self, CommerceModelError> {
        let amount = self.amount_minor.checked_mul(i64::from(quantity)).ok_or(
            CommerceModelError::AmountOverflow {
                field: "money_multiplication",
            },
        )?;
        Self::new(self.currency.clone(), amount)
    }
}
