use crate::error::CommerceModelError;
use crate::identifiers::CurrencyCode;
use crate::model::{AdjustmentDirection, AdjustmentKind, Money};
use crate::validation::require_non_empty;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceAdjustment {
    pub kind: AdjustmentKind,
    pub direction: AdjustmentDirection,
    pub label: String,
    pub amount: Money,
}

impl PriceAdjustment {
    pub fn discount(
        kind: AdjustmentKind,
        label: impl Into<String>,
        amount: Money,
    ) -> Result<Self, CommerceModelError> {
        Ok(Self {
            kind,
            direction: AdjustmentDirection::Discount,
            label: require_non_empty("adjustment_label", label.into())?,
            amount,
        })
    }

    pub fn surcharge(
        kind: AdjustmentKind,
        label: impl Into<String>,
        amount: Money,
    ) -> Result<Self, CommerceModelError> {
        Ok(Self {
            kind,
            direction: AdjustmentDirection::Surcharge,
            label: require_non_empty("adjustment_label", label.into())?,
            amount,
        })
    }

    pub fn signed_minor_delta(&self) -> i64 {
        match self.direction {
            AdjustmentDirection::Discount => -self.amount.amount_minor(),
            AdjustmentDirection::Surcharge => self.amount.amount_minor(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscountRule {
    Percentage {
        kind: AdjustmentKind,
        label: String,
        basis_points: u32,
    },
    Fixed {
        kind: AdjustmentKind,
        label: String,
        amount: Money,
    },
}

impl DiscountRule {
    fn build_adjustment(&self, subtotal: &Money) -> Result<PriceAdjustment, CommerceModelError> {
        match self {
            Self::Percentage {
                kind,
                label,
                basis_points,
            } => {
                validate_basis_points("discount_basis_points", *basis_points)?;
                let amount = subtotal
                    .amount_minor()
                    .checked_mul(i64::from(*basis_points))
                    .ok_or(CommerceModelError::AmountOverflow {
                        field: "discount_amount",
                    })?
                    / 10_000;
                PriceAdjustment::discount(
                    *kind,
                    label.clone(),
                    Money::new(subtotal.currency().clone(), amount)?,
                )
            }
            Self::Fixed {
                kind,
                label,
                amount,
            } => PriceAdjustment::discount(*kind, label.clone(), amount.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PricingPolicy {
    pub currency: CurrencyCode,
    tax_rate_basis_points: u32,
    discounts: Vec<DiscountRule>,
    surcharges: Vec<PriceAdjustment>,
}

impl PricingPolicy {
    pub fn new(currency: CurrencyCode) -> Self {
        Self {
            currency,
            tax_rate_basis_points: 0,
            discounts: Vec::new(),
            surcharges: Vec::new(),
        }
    }

    pub fn with_membership_discount_basis_points(
        mut self,
        basis_points: u32,
    ) -> Result<Self, CommerceModelError> {
        validate_basis_points("membership_discount_basis_points", basis_points)?;
        self.discounts.push(DiscountRule::Percentage {
            kind: AdjustmentKind::MembershipBenefit,
            label: "Member pricing".to_string(),
            basis_points,
        });
        Ok(self)
    }

    pub fn with_fixed_discount(
        mut self,
        kind: AdjustmentKind,
        label: impl Into<String>,
        amount: Money,
    ) -> Result<Self, CommerceModelError> {
        ensure_same_currency(&self.currency, amount.currency())?;
        self.discounts.push(DiscountRule::Fixed {
            kind,
            label: require_non_empty("discount_label", label.into())?,
            amount,
        });
        Ok(self)
    }

    pub fn with_shipping(self, amount: Money) -> Result<Self, CommerceModelError> {
        self.with_surcharge(AdjustmentKind::Shipping, "Shipping", amount)
    }

    pub fn with_surcharge(
        mut self,
        kind: AdjustmentKind,
        label: impl Into<String>,
        amount: Money,
    ) -> Result<Self, CommerceModelError> {
        ensure_same_currency(&self.currency, amount.currency())?;
        self.surcharges
            .push(PriceAdjustment::surcharge(kind, label, amount)?);
        Ok(self)
    }

    pub fn with_tax_rate_basis_points(
        mut self,
        basis_points: u32,
    ) -> Result<Self, CommerceModelError> {
        validate_basis_points("tax_rate_basis_points", basis_points)?;
        self.tax_rate_basis_points = basis_points;
        Ok(self)
    }

    pub(crate) fn adjustments_for_subtotal(
        &self,
        subtotal: &Money,
    ) -> Result<Vec<PriceAdjustment>, CommerceModelError> {
        ensure_same_currency(&self.currency, subtotal.currency())?;

        let mut adjustments = Vec::new();
        for rule in &self.discounts {
            adjustments.push(rule.build_adjustment(subtotal)?);
        }
        adjustments.extend(self.surcharges.clone());

        let pre_tax_quote = PriceQuote::new(subtotal.clone(), adjustments.clone())?;
        if self.tax_rate_basis_points > 0 && pre_tax_quote.total.amount_minor() > 0 {
            let tax_minor = pre_tax_quote
                .total
                .amount_minor()
                .checked_mul(i64::from(self.tax_rate_basis_points))
                .ok_or(CommerceModelError::AmountOverflow {
                    field: "tax_amount",
                })?
                / 10_000;
            adjustments.push(PriceAdjustment::surcharge(
                AdjustmentKind::Tax,
                "Tax",
                Money::new(self.currency.clone(), tax_minor)?,
            )?);
        }

        Ok(adjustments)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceQuote {
    pub subtotal: Money,
    pub adjustments: Vec<PriceAdjustment>,
    pub total: Money,
}

impl PriceQuote {
    pub fn new(
        subtotal: Money,
        adjustments: Vec<PriceAdjustment>,
    ) -> Result<Self, CommerceModelError> {
        let mut total_minor = i128::from(subtotal.amount_minor());
        for adjustment in &adjustments {
            ensure_same_currency(subtotal.currency(), adjustment.amount.currency())?;
            total_minor += i128::from(adjustment.signed_minor_delta());
        }

        if total_minor < 0 {
            return Err(CommerceModelError::TotalWouldBecomeNegative {
                total_minor: total_minor.try_into().unwrap_or(i64::MIN),
            });
        }

        let total_minor: i64 =
            total_minor
                .try_into()
                .map_err(|_| CommerceModelError::AmountOverflow {
                    field: "priced_total",
                })?;

        Ok(Self {
            subtotal: subtotal.clone(),
            adjustments,
            total: Money::new(subtotal.currency().clone(), total_minor)?,
        })
    }
}

pub(crate) fn ensure_same_currency(
    expected: &CurrencyCode,
    actual: &CurrencyCode,
) -> Result<(), CommerceModelError> {
    if expected == actual {
        Ok(())
    } else {
        Err(CommerceModelError::CurrencyMismatch {
            expected: expected.clone(),
            actual: actual.clone(),
        })
    }
}

fn validate_basis_points(field: &'static str, basis_points: u32) -> Result<(), CommerceModelError> {
    if basis_points <= 10_000 {
        Ok(())
    } else {
        Err(CommerceModelError::BasisPointsOutOfRange {
            field,
            basis_points,
        })
    }
}

pub(crate) fn default_retry_policy() -> davenda_jobs::RetryPolicy {
    davenda_jobs::RetryPolicy::new(
        3,
        std::time::Duration::from_secs(15),
        std::time::Duration::from_secs(300),
    )
    .expect("constant retry policy is valid")
}
